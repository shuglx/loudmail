//! 单实例（Unix socket）与开机自启（macOS 登录项 / Linux XDG autostart）

use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};

use tauri::AppHandle;

fn sock_path() -> std::path::PathBuf {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        let path = std::path::PathBuf::from(dir).join("loudmail.sock");
        if path.is_absolute() {
            return path;
        }
    }
    dirs::cache_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join("loudmail-instance.sock")
}

/// 返回 true 表示本进程继续运行；false 表示已有实例并已请求其显示窗口。
pub fn acquire_single_instance(app: AppHandle) -> bool {
    let path = sock_path();
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(first_error) => match UnixStream::connect(&path) {
            Ok(mut stream) => {
                let _ = stream.write_all(b"show");
                return false;
            }
            Err(_) => {
                let _ = std::fs::remove_file(&path);
                match UnixListener::bind(&path) {
                    Ok(listener) => listener,
                    Err(e) => {
                        eprintln!(
                            "[单实例] 绑定 {path:?} 失败({first_error} / {e})，本次禁用单实例"
                        );
                        return true;
                    }
                }
            }
        },
    };

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let _ = stream.write_all(b"show");
                crate::show_main(&app);
            }
        }
    });
    true
}

#[cfg(target_os = "macos")]
pub fn set_autostart(enable: bool) -> Result<bool, String> {
    use auto_launch::AutoLaunchBuilder;

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let launch = AutoLaunchBuilder::new()
        .set_app_name("LoudMail")
        .set_app_path(exe.to_string_lossy().as_ref())
        .build()
        .map_err(|e| e.to_string())?;
    if enable {
        launch.enable().map_err(|e| e.to_string())?;
    } else {
        launch.disable().map_err(|e| e.to_string())?;
    }
    launch.is_enabled().map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
pub fn is_autostart() -> bool {
    use auto_launch::AutoLaunchBuilder;

    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    AutoLaunchBuilder::new()
        .set_app_name("LoudMail")
        .set_app_path(exe.to_string_lossy().as_ref())
        .build()
        .ok()
        .and_then(|l| l.is_enabled().ok())
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn autostart_desktop_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("autostart").join("loudmail.desktop"))
}

#[cfg(not(target_os = "macos"))]
pub fn set_autostart(enable: bool) -> Result<bool, String> {
    let path = autostart_desktop_path().ok_or("无法定位用户配置目录")?;
    if enable {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let content = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=LoudMail\n\
             Comment=Mail alert notifier\n\
             Exec={} --hidden\n\
             Icon=loudmail\n\
             Terminal=false\n\
             X-GNOME-Autostart-enabled=true\n",
            exe.display()
        );
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
    } else {
        let _ = std::fs::remove_file(&path);
    }
    Ok(is_autostart())
}

#[cfg(not(target_os = "macos"))]
pub fn is_autostart() -> bool {
    autostart_desktop_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}
