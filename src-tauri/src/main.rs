// Windows 发布版不额外弹控制台窗口；dev 构建(debug)与其它平台行为不变
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod audio;
mod commands;
mod logfile;
mod mcp;
mod models;
mod scheduler;
mod state;
mod store;
mod system;

use std::path::PathBuf;

use tauri::{
    AppHandle, CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu,
    SystemTrayMenuItem, WindowEvent,
};

use state::AppState;

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn main() {
    let tray_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("show", "显示主窗口"))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("quit", "退出"));

    tauri::Builder::default()
        .on_window_event(|event| {
            if let WindowEvent::CloseRequested { api, .. } = event.event() {
                if event.window().label() == "main" {
                    api.prevent_close();
                    let _ = event.window().hide();
                    logfile::info("[窗口] 关闭请求，已隐藏到托盘");
                }
            }
        })
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "show" => show_main(app),
                "quit" => {
                    logfile::info("[托盘] 用户退出应用");
                    app.exit(0);
                }
                _ => {}
            },
            SystemTrayEvent::LeftClick { .. } => show_main(app),
            SystemTrayEvent::DoubleClick { .. } => show_main(app),
            _ => {}
        })
        .setup(|app| {
            let root = std::env::var_os("LOUDMAIL_DATA_DIR")
                .map(PathBuf::from)
                .or_else(|| app.path_resolver().app_data_dir())
                .unwrap_or_else(|| {
                    dirs::data_dir()
                        .unwrap_or_else(std::env::temp_dir)
                        .join("com.loudmail.app")
                });
            let paths = store::init(root).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })?;
            logfile::init(&paths.logs_dir, &app.package_info().version.to_string());
            logfile::install_panic_hook();
            logfile::info(format!("数据目录: {}", paths.root.display()));

            if !system::acquire_single_instance(app.handle().clone()) {
                logfile::info("[启动] 检测到已有实例，已请求其显示窗口");
                std::process::exit(0);
            }

            let state = AppState::load(app.handle().clone(), paths)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            app.manage(state.clone());
            scheduler::start(state.clone());

            // 三端统一：左键（单击/双击）呼出主界面，右键才弹菜单。
            // macOS 默认是左键弹菜单，Windows 默认也是，这里统一关掉。
            SystemTray::new()
                .with_menu(tray_menu)
                .with_menu_on_left_click(false)
                .build(app)?;

            if !std::env::args().any(|arg| arg == "--hidden") {
                show_main(&app.handle());
            }
            logfile::info("[启动] 完成");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::strategies::cmd_list_strategies,
            commands::strategies::cmd_create_strategy,
            commands::strategies::cmd_update_strategy,
            commands::strategies::cmd_delete_strategy,
            commands::strategies::cmd_set_strategy_enabled,
            commands::strategies::cmd_reorder_strategies,
            commands::strategies::cmd_export_strategies,
            commands::strategies::cmd_pick_strategies_file,
            commands::strategies::cmd_import_strategies,
            commands::history::cmd_list_history,
            commands::history::cmd_clear_history,
            commands::sounds::cmd_list_sounds,
            commands::sounds::cmd_pick_audio_file,
            commands::sounds::cmd_import_sound,
            commands::sounds::cmd_start_recording,
            commands::sounds::cmd_stop_recording,
            commands::sounds::cmd_save_recording,
            commands::sounds::cmd_rename_sound,
            commands::sounds::cmd_delete_sound,
            commands::sounds::cmd_export_sound,
            commands::sounds::cmd_preview_sound,
            commands::sounds::cmd_stop_playback,
            commands::sounds::cmd_get_playback_status,
            commands::sounds::cmd_test_playback,
            commands::sounds::cmd_tts_generate,
            commands::sounds::cmd_compose_audio,
            commands::sounds::cmd_save_composed_sound,
            commands::sounds::cmd_preview_temp,
            commands::settings::cmd_get_config,
            commands::settings::cmd_set_config,
            commands::settings::cmd_test_connection,
            commands::settings::cmd_test_search,
            commands::settings::cmd_tts_status,
            commands::settings::cmd_get_runtime_status,
            commands::settings::cmd_get_autostart,
            commands::settings::cmd_set_autostart,
            commands::settings::cmd_get_storage_files,
            commands::settings::cmd_get_log_path,
            commands::settings::cmd_open_path,
            commands::settings::cmd_get_app_info,
            commands::settings::cmd_log,
            commands::settings::cmd_hide_window,
            commands::settings::cmd_minimize_window,
            commands::settings::cmd_toggle_maximize_window,
            commands::settings::cmd_is_window_maximized,
            commands::settings::cmd_show_window,
            commands::settings::cmd_quit_app,
            commands::settings::cmd_get_broadcast_paused,
            commands::settings::cmd_set_broadcast_paused,
        ])
        .run(tauri::generate_context!())
        .expect("LoudMail 启动失败");
}
