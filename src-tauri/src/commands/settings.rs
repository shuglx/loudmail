use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use chrono::{Duration as ChronoDuration, Local};

use crate::logfile;
use crate::mcp;
use crate::models::*;
use crate::state::AppState;
use crate::store;
use crate::system;

#[tauri::command]
pub fn cmd_get_config(state: State<'_, Arc<AppState>>) -> AppConfig {
    state.config.read().map(|c| c.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn cmd_set_config(
    state: State<'_, Arc<AppState>>,
    config: AppConfig,
) -> Result<AppConfig, String> {
    let mut config = config;
    config.mcp.host = config.mcp.host.trim().to_string();
    if config.mcp.host.is_empty() {
        return Err("MCP 地址不能为空".into());
    }
    if config.mcp.port == 0 {
        return Err("端口需为 1~65535".into());
    }
    if ![60, 120, 180].contains(&config.scan.interval_sec) {
        return Err("扫描间隔仅支持 1 / 2 / 3 分钟".into());
    }
    if ![10, 30, 60].contains(&config.scan.lookback_min) {
        return Err("检索时间窗仅支持 10 / 30 / 60 分钟".into());
    }
    if !["light", "dark"].contains(&config.theme.as_str()) {
        return Err("主题仅支持 light / dark".into());
    }
    config.tts.intranet_url = config.tts.intranet_url.trim().to_string();
    config.tts.intranet_key = config.tts.intranet_key.trim().to_string();
    config.tts.intranet_instructions = config.tts.intranet_instructions.trim().to_string();
    if config.tts.intranet_voice != "ryan" {
        config.tts.intranet_voice = "vivian".into();
    }
    config.tts.speed = config.tts.speed.clamp(0.5, 2.0);
    // 本地模型已下线：老配置里的 local 统一迁移到联网服务
    if config.tts.provider == "local" {
        config.tts.provider = "edge".into();
    }
    if !["edge", "intranet"].contains(&config.tts.provider.as_str()) {
        return Err("语音来源仅支持 edge / intranet".into());
    }
    config.tts.edge_voice = config.tts.edge_voice.trim().to_string();
    if config.tts.edge_voice.is_empty() {
        config.tts.edge_voice = "zh-CN-XiaoxiaoNeural".into();
    }
    if !["zh-CN-XiaoxiaoNeural", "zh-CN-YunyangNeural"]
        .contains(&config.tts.edge_voice.as_str())
    {
        return Err("在线音色仅支持晓晓 / 云扬".into());
    }
    // 端点 / 模拟开关变化时，旧连接立即失效，并在后台立刻探一次，
    // 让状态马上反映"改完之后到底通不通"，不必等下一轮（也避免连续两次失败才翻红）
    let endpoint_changed = {
        let current = state.config.read().map(|c| c.clone()).ok();
        match current {
            Some(old) => {
                old.mcp.host != config.mcp.host
                    || old.mcp.port != config.mcp.port
                    || old.behavior.mock != config.behavior.mock
            }
            None => false,
        }
    };

    *state.config.write().map_err(|e| e.to_string())? = config.clone();
    state.save_config()?;

    if endpoint_changed {
        // 只有端点 / 模拟开关变了才需要作废旧连接并立刻重扫；
        // 改主题、TTS 这类设置若也递增版本，会顺带触发一次多余的轮询
        state.config_version.fetch_add(1, Ordering::Relaxed);
        let state = state.inner().clone();
        let host = config.mcp.host.clone();
        let port = config.mcp.port;
        let mock = config.behavior.mock;
        std::thread::spawn(move || {
            let mut next = RuntimeStatus::default();
            next.mock = mock;
            if mock {
                next.connected = true;
                next.server_name = Some("Mock MailBox".into());
                logfile::info("[连接] 已切换到模拟邮箱");
            } else {
                let result = mcp::test_connection(&host, port, false);
                next.connected = result.overall;
                if let Some(info) = result.handshake.server_info.as_ref() {
                    next.server_name = Some(info.name.clone());
                    next.server_version = Some(info.version.clone());
                }
                next.last_error = if result.overall {
                    None
                } else {
                    result
                        .handshake
                        .error
                        .clone()
                        .or_else(|| result.tcp.error.clone())
                };
                logfile::info(format!(
                    "[连接] 端点变更后立即探测 {host}:{port} -> {}",
                    if result.overall { "可达" } else { "不可达" }
                ));
            }
            state.set_runtime(next);
        });
    }

    Ok(config)
}

#[tauri::command]
pub fn cmd_test_connection(
    state: State<'_, Arc<AppState>>,
    host: String,
    port: u16,
) -> TestConnectionResult {
    let mock = state
        .config
        .read()
        .map(|c| c.behavior.mock)
        .unwrap_or(false);
    let result = mcp::test_connection(&host, port, mock);
    // 显式「检查连接」：把结果写进运行状态
    let mut next = state
        .runtime
        .read()
        .map(|r| r.clone())
        .unwrap_or_default();
    next.mock = mock;
    next.connected = result.overall;
    if result.overall {
        next.last_error = None;
        if let Some(info) = result.handshake.server_info.as_ref() {
            next.server_name = Some(info.name.clone());
            next.server_version = Some(info.version.clone());
        }
    } else {
        next.last_error = result
            .handshake
            .error
            .clone()
            .or_else(|| result.tcp.error.clone());
    }
    state.set_runtime(next);
    result
}

/// 调试用：用 MCP 接口查一次邮件（limit 3），返回收发报文
#[tauri::command]
pub fn cmd_test_search(
    state: State<'_, Arc<AppState>>,
    keyword: String,
    limit: u32,
    all_time: bool,
) -> DebugSearchResult {
    let (host, port, lookback_min, mock) = match state.config.read() {
        Ok(config) => (
            config.mcp.host.clone(),
            config.mcp.port,
            config.scan.lookback_min,
            config.behavior.mock,
        ),
        Err(_) => ("127.0.0.1".to_string(), 11328, 30, false),
    };
    let end = Local::now();
    let start = end - ChronoDuration::minutes(lookback_min as i64);
    // all_time=true 表示「不限」：下界给一个足够早的时间，仍显式传起止时间
    let start_time = if all_time {
        "20000101000000".to_string()
    } else {
        start.format("%Y%m%d%H%M%S").to_string()
    };
    let end_time = end.format("%Y%m%d%H%M%S").to_string();
    logfile::info(format!(
        "[测试接口] keyword={keyword} limit={limit} window={}",
        if all_time {
            format!("不限~{end_time}")
        } else {
            format!("{start_time}~{end_time}")
        }
    ));
    let result = mcp::search_debug(&host, port, &keyword, &start_time, &end_time, limit, mock);
    // 显式「发起测试」：同样以这次结果刷新运行状态
    let mut next = state
        .runtime
        .read()
        .map(|r| r.clone())
        .unwrap_or_default();
    next.mock = mock;
    next.connected = result.ok;
    next.last_error = result.error.clone();
    state.set_runtime(next);
    result
}

#[tauri::command]
pub fn cmd_get_runtime_status(state: State<'_, Arc<AppState>>) -> RuntimeStatus {
    state
        .runtime
        .read()
        .map(|r| r.clone())
        .unwrap_or_default()
}

#[tauri::command]
pub fn cmd_get_autostart() -> bool {
    system::is_autostart()
}

#[tauri::command]
pub fn cmd_set_autostart(
    state: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<bool, String> {
    let actual = system::set_autostart(enabled)?;
    if let Ok(mut config) = state.config.write() {
        config.behavior.autostart = actual;
    }
    let _ = state.save_config();
    Ok(actual)
}

#[tauri::command]
pub fn cmd_get_storage_files(state: State<'_, Arc<AppState>>) -> Vec<StorageFile> {
    store::storage_files(&state.paths)
}

#[tauri::command]
pub fn cmd_get_log_path() -> Option<String> {
    logfile::path().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn cmd_open_path(path: String) -> Result<(), String> {
    if path.starts_with("http://") || path.starts_with("https://") {
        return open::that(&path).map_err(|e| format!("打开失败: {e}"));
    }
    let target = std::path::Path::new(&path);
    if !target.exists() {
        logfile::warn(format!("打开失败，文件不存在: {path}"));
        return Err(format!("文件不存在：{path}"));
    }
    logfile::info(format!("打开路径: {path}"));
    open::that(target).map_err(|e| format!("打开失败: {e}"))
}


#[tauri::command]
pub async fn cmd_tts_status(state: State<'_, Arc<AppState>>) -> Result<TtsStatus, String> {
    let config = state
        .config
        .read()
        .map(|c| c.tts.clone())
        .unwrap_or_default();
    if !config.enabled {
        return Ok(TtsStatus {
            ready: false,
            message: "TTS 未启用，请先开启".into(),
        });
    }

    // 真实探测：内网走 WS 握手，联网只探 TCP 443
    let result = tauri::async_runtime::spawn_blocking(move || {
        if config.provider == "intranet" {
            match crate::audio::tts::probe_intranet(&config.intranet_url, &config.intranet_key) {
                Ok(_) => TtsStatus {
                    ready: true,
                    message: format!("内网服务可达 · 音色 {}", config.intranet_voice),
                },
                Err(e) => TtsStatus {
                    ready: false,
                    message: e,
                },
            }
        } else {
            match crate::audio::tts::probe_edge() {
                Ok(_) => TtsStatus {
                    ready: true,
                    message: "联网服务可达（微软 Edge）".into(),
                },
                Err(e) => TtsStatus {
                    ready: false,
                    message: e,
                },
            }
        }
    })
    .await;
    Ok(result.unwrap_or(TtsStatus {
        ready: false,
        message: "检测失败".into(),
    }))
}

#[tauri::command]
pub fn cmd_get_app_info(app: AppHandle, state: State<'_, Arc<AppState>>) -> AppInfo {
    // 版本统一取 tauri.conf.json 的 package.version，前端标题栏与设置页共用
    let package = app.package_info();
    AppInfo {
        name: package.name.clone(),
        version: package.version.to_string(),
        tauri_version: "1.8".into(),
        data_dir: state.paths.root.to_string_lossy().to_string(),
        storage_files: store::storage_files(&state.paths),
        log_path: state.paths.log_file.to_string_lossy().to_string(),
    }
}

#[tauri::command]
pub fn cmd_log(level: String, message: String) {
    match level.as_str() {
        "error" => logfile::error(message),
        "warn" => logfile::warn(message),
        _ => logfile::info(message),
    }
}

#[tauri::command]
pub fn cmd_hide_window(app: AppHandle) -> Result<(), String> {
    let window = app.get_window("main").ok_or("主窗口不存在")?;
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_minimize_window(app: AppHandle) -> Result<(), String> {
    let window = app.get_window("main").ok_or("主窗口不存在")?;
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_toggle_maximize_window(app: AppHandle) -> Result<bool, String> {
    let window = app.get_window("main").ok_or("主窗口不存在")?;

    // Linux（麒麟/GTK）上 maximize() 会按「工作区」铺，顶部若被算作面板区域就会留一条缝。
    // 这里改成自己按显示器几何铺满，并在还原时恢复原位置与尺寸。
    #[cfg(target_os = "linux")]
    {
        let mut slot = LINUX_RESTORE
            .lock()
            .map_err(|_| "窗口状态锁失败".to_string())?;
        if let Some((x, y, w, h)) = *slot {
            window
                .set_size(tauri::Size::Physical(tauri::PhysicalSize::new(w, h)))
                .map_err(|e| e.to_string())?;
            window
                .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(x, y)))
                .map_err(|e| e.to_string())?;
            *slot = None;
            logfile::info("[窗口] 已还原窗口大小");
            let _ = window.set_focus();
            return Ok(false);
        }

        let pos = window.outer_position().map_err(|e| e.to_string())?;
        let size = window.inner_size().map_err(|e| e.to_string())?;
        let monitor = window
            .current_monitor()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "未找到当前显示器".to_string())?;
        *slot = Some((pos.x, pos.y, size.width, size.height));
        // 先改尺寸再改位置：部分 WM 会按"当前尺寸必须落在工作区内"来夹取位置，
        // 反过来设置时位置请求会被 WM 拒绝/改写
        window
            .set_size(tauri::Size::Physical(*monitor.size()))
            .map_err(|e| e.to_string())?;
        window
            .set_position(tauri::Position::Physical(*monitor.position()))
            .map_err(|e| e.to_string())?;
        // 部分 WM 在 resize 之后会重新摆放窗口，等它稳定后再定一次位
        std::thread::sleep(std::time::Duration::from_millis(150));
        let _ = window.set_position(tauri::Position::Physical(*monitor.position()));
        logfile::info(format!(
            "[窗口] 已最大化（铺满显示器 {}x{}）",
            monitor.size().width,
            monitor.size().height
        ));
        let _ = window.set_focus();
        return Ok(true);
    }

    #[cfg(not(target_os = "linux"))]
    {
        let maximized = window.is_maximized().map_err(|e| e.to_string())?;
        if maximized {
            window.unmaximize().map_err(|e| e.to_string())?;
            logfile::info("[窗口] 已还原窗口大小");
        } else {
            window.maximize().map_err(|e| e.to_string())?;
            logfile::info("[窗口] 已最大化");
        }
        // macOS 上 zoom 之后 WKWebView 可能不再重绘（标题栏图标 hover 不出来、
        // 画面停在旧帧），让它重新成为 key window 即可恢复。
        let _ = window.set_focus();
        return Ok(!maximized);
    }
}

/// Linux 上自己实现最大化，需要记住还原用的原始几何
#[cfg(target_os = "linux")]
static LINUX_RESTORE: std::sync::Mutex<Option<(i32, i32, u32, u32)>> = std::sync::Mutex::new(None);

#[tauri::command]
pub fn cmd_is_window_maximized(app: AppHandle) -> Result<bool, String> {
    let window = app.get_window("main").ok_or("主窗口不存在")?;
    window.is_maximized().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_show_window(app: AppHandle) {
    crate::show_main(&app);
}

/// 是否已暂停全部播报
#[tauri::command]
pub fn cmd_get_broadcast_paused(state: State<'_, Arc<AppState>>) -> bool {
    state.paused.load(Ordering::Relaxed)
}

/// 停用 / 恢复全部播报（替代原托盘菜单里的「暂停播报」）
#[tauri::command]
pub fn cmd_set_broadcast_paused(state: State<'_, Arc<AppState>>, paused: bool) -> bool {
    state.paused.store(paused, Ordering::Relaxed);
    if paused {
        // 停用：立即停止当前播报并清空待播队列
        state.player.stop_all();
        logfile::info("[播报] 已暂停全部播报，停止轮询匹配");
    } else {
        // 恢复：只播报今后新命中的邮件，跳过停用期间积压的
        state.skip_backlog.store(true, Ordering::Relaxed);
        logfile::info("[播报] 已恢复播报，跳过停用期间的积压邮件");
    }
    paused
}

#[tauri::command]
pub fn cmd_quit_app(app: AppHandle) {
    app.exit(0);
}
