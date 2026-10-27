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

// Linux 专用的 X11 EWMH 前置/聚焦(见 show_main 的 Linux 分支)
#[cfg(target_os = "linux")]
mod x11;

use std::path::PathBuf;

#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    AppHandle, CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu,
    SystemTrayMenuItem, WindowEvent,
};

use state::AppState;

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_window("main") {
        let _ = window.show();
        let _ = window.unminimize();

        // Linux 下(ukui-kwin 等 WM)tao 的 set_focus / set_always_on_top 都不可靠:
        // GTK 的激活请求会被「防抢焦点」机制忽略(窗口被其它应用盖住时提不到最前),
        // GTK 的 keep_above 在 UKUI 上同样不一定被理会。参考 kylinpaste 的实测做法:
        // show 之后等窗口进入 _NET_CLIENT_LIST,直接向根窗口发 EWMH _NET_ACTIVE_WINDOW
        // (source=2,用户主动请求,WM 会放行),由 activate_window 内部兜底 XSetInputFocus
        // (见 src/x11.rs);macOS / Windows 的 set_focus 本身就会前置,无需额外处理。
        #[cfg(target_os = "linux")]
        {
            // show() 走 GTK show_all,会把 tao 预分配的空菜单栏一并显示——透明
            // 无边框窗口下它就是顶部那条 22px 的透明带,每次唤出后都要补隐藏一次
            hide_empty_gtk_menubar(app);

            std::thread::spawn(move || {
                for _ in 0..10 {
                    std::thread::sleep(std::time::Duration::from_millis(80));
                    if let Some(xid) = x11::main_window_xid() {
                        if x11::activate_window(xid) {
                            logfile::info("[窗口] 已通过 EWMH 前置并聚焦主窗口");
                            return;
                        }
                    }
                }
                logfile::warn("[窗口] EWMH 前置主窗口未成功(桌面可能未响应激活请求)");
            });
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = window.set_focus();
        }
    }
}

/// 隐藏 tao 在 GTK 窗口顶部预分配的空菜单栏（消除顶部 22px 的透明带）。
///
/// tao 为「之后注入菜单」在每个 GTK 窗口的竖直 GtkBox 顶部 pack 了一个空
/// GtkMenuBar（tao window.rs：`window_box.pack_start(&menu_bar, ...)`），本应用
/// 从不设置菜单，它仍占约 22px 高度；透明无边框窗口下 tao 还把它刷成透明，
/// 表现就是窗口顶部永远有一条 22px 的透明带（内容比窗口矮 22px，透出底下的
/// 窗口），最大化贴屏时最显眼。tao 的 show() 走 show_all，每次唤出都会让它
/// 复活，所以每次 show_main 后都要补隐藏。macOS / Windows 后端没有这个结构，
/// 无需处理。
#[cfg(target_os = "linux")]
fn hide_empty_gtk_menubar(app: &AppHandle) {
    static LOGGED: AtomicBool = AtomicBool::new(false);
    // 闭包需要 'static，这里先克隆一份给闭包，app 本身只用来派发
    let handle = app.clone();
    // GTK 调用必须在主线程：托盘回调恰在主循环里，tauri 命令则在线程池里，
    // 统一经 run_on_main_thread 派发
    let _ = app.run_on_main_thread(move || {
        use gtk::prelude::*;

        let Some(window) = handle.get_window("main") else {
            return;
        };
        let Ok(gtk_window) = window.gtk_window() else {
            if !LOGGED.swap(true, Ordering::SeqCst) {
                logfile::warn("[窗口] 取 GTK 窗口句柄失败，顶部透明菜单栏未隐藏");
            }
            return;
        };
        // tao 的结构固定为 GtkWindow -> GtkBox(window_box) -> [空 GtkMenuBar, webview]
        let mut hidden = false;
        for top in gtk_window.children() {
            let Ok(vbox) = top.downcast::<gtk::Box>() else {
                continue;
            };
            for child in vbox.children() {
                if child.is::<gtk::MenuBar>() {
                    child.hide();
                    hidden = true;
                }
            }
        }
        // 首次成功/失败各落一条日志即可，唤出是高频动作，别刷屏
        if hidden && !LOGGED.swap(true, Ordering::SeqCst) {
            logfile::info("[窗口] 已隐藏 tao 预分配的空 GTK 菜单栏（消除顶部 22px 透明带）");
        }
    });
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

            // 托盘左键呼出主界面、右键弹菜单。
            // 注意：with_menu_on_left_click 只有 macOS 版本才有（tauri 里带
            // #[cfg(target_os = "macos")]），Windows/Linux 调用会编译失败。
            // Windows 左键本来就走 LeftClick 事件、右键弹菜单，无需干预；
            // Linux 由 libappindicator 决定，无法通过 API 控制。
            #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
            let mut tray = SystemTray::new().with_menu(tray_menu);
            #[cfg(target_os = "macos")]
            {
                tray = tray.with_menu_on_left_click(false);
            }
            tray.build(app)?;

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
            commands::strategies::cmd_set_strategy_require_confirm,
            commands::strategies::cmd_reorder_strategies,
            commands::strategies::cmd_export_strategies,
            commands::strategies::cmd_pick_strategies_file,
            commands::strategies::cmd_import_strategies,
            commands::history::cmd_list_history,
            commands::history::cmd_clear_history,
            commands::history::cmd_confirm_history,
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
