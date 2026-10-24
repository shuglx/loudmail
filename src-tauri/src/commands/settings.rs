use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

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
    config.tts.model_dir = config.tts.model_dir.trim().to_string();
    config.tts.speed = config.tts.speed.clamp(0.5, 2.0);
    if !["local", "edge"].contains(&config.tts.provider.as_str()) {
        return Err("语音来源仅支持 local / edge".into());
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
    *state.config.write().map_err(|e| e.to_string())? = config.clone();
    state.save_config()?;
    state.config_version.fetch_add(1, Ordering::Relaxed);
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
    mcp::test_connection(&host, port, mock)
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
pub async fn cmd_pick_tts_model_dir(app: AppHandle) -> Result<Option<String>, String> {
    if let Some(window) = app.get_window("main") {
        let _ = window.set_focus();
    }
    Ok(tauri::api::dialog::blocking::FileDialogBuilder::new()
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn cmd_tts_status(state: State<'_, Arc<AppState>>) -> TtsStatus {
    let config = state
        .config
        .read()
        .map(|c| c.tts.clone())
        .unwrap_or_default();
    if !config.enabled {
        return TtsStatus {
            ready: false,
            message: "TTS 未启用，请先开启".into(),
        };
    }
    if config.provider == "edge" {
        return TtsStatus {
            ready: true,
            message: "在线 Edge 语音，需联网".into(),
        };
    }
    match crate::audio::tts::resolve_model(&config.model_dir) {
        Ok(_) => TtsStatus {
            ready: true,
            message: "模型已就绪".into(),
        },
        Err(e) => TtsStatus {
            ready: false,
            message: e,
        },
    }
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
    let maximized = window.is_maximized().map_err(|e| e.to_string())?;
    if maximized {
        window.unmaximize().map_err(|e| e.to_string())?;
        logfile::info("[窗口] 已还原窗口大小");
    } else {
        window.maximize().map_err(|e| e.to_string())?;
        logfile::info("[窗口] 已最大化");
    }
    Ok(!maximized)
}

#[tauri::command]
pub fn cmd_is_window_maximized(app: AppHandle) -> Result<bool, String> {
    let window = app.get_window("main").ok_or("主窗口不存在")?;
    window.is_maximized().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_show_window(app: AppHandle) {
    crate::show_main(&app);
}

#[tauri::command]
pub fn cmd_quit_app(app: AppHandle) {
    app.exit(0);
}
