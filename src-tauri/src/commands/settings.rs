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
pub fn cmd_open_path(app: AppHandle, path: String) -> Result<(), String> {
    tauri::api::shell::open(&app.shell_scope(), path, None).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_get_app_info(state: State<'_, Arc<AppState>>) -> AppInfo {
    AppInfo {
        name: "LoudMail".into(),
        version: env!("CARGO_PKG_VERSION").into(),
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
pub fn cmd_show_window(app: AppHandle) {
    crate::show_main(&app);
}

#[tauri::command]
pub fn cmd_quit_app(app: AppHandle) {
    app.exit(0);
}
