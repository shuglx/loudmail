//! 单实例与开机自启
//!
//! - 单实例：Unix 用 Unix socket；Windows 用 127.0.0.1 回环 TCP。语义一致：
//!   第二个实例连上已运行的实例后，让它显示主窗口，自己退出。
//! - 开机自启：macOS 登录项、Windows 注册表 Run 键（都走 auto-launch）；
//!   Linux / 麒麟走 XDG autostart（.desktop 文件）。

use tauri::AppHandle;

// --------------------------------------------------------------- 单实例通道(Unix)

#[cfg(unix)]
mod chan {
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
    pub fn acquire(app: AppHandle) -> bool {
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
}

// ------------------------------------------------------------- 单实例通道(Windows)

#[cfg(windows)]
mod chan {
    use std::io::Write;
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::time::Duration;

    use tauri::AppHandle;

    /// 只监听本机回环，用于"唤起已运行实例"，不对外暴露服务。
    const PORT: u16 = 11329;

    /// 返回 true 表示本进程继续运行；false 表示已有实例并已请求其显示窗口。
    pub fn acquire(app: AppHandle) -> bool {
        let addr = SocketAddr::from(([127, 0, 0, 1], PORT));
        match TcpListener::bind(addr) {
            Ok(listener) => {
                std::thread::spawn(move || {
                    for stream in listener.incoming() {
                        if stream.is_ok() {
                            crate::show_main(&app);
                        }
                    }
                });
                true
            }
            Err(bind_error) => match TcpStream::connect_timeout(&addr, Duration::from_millis(800))
            {
                Ok(mut stream) => {
                    let _ = stream.write_all(b"show");
                    false
                }
                Err(_) => {
                    // 端口被其它程序占用等极端情况：放行，避免应用起不来
                    eprintln!(
                        "[单实例] 回环端口 {PORT} 不可用({bind_error})，本次禁用单实例"
                    );
                    true
                }
            },
        }
    }
}

/// 返回 true 表示本进程继续运行；false 表示已有实例并已请求其显示窗口。
pub fn acquire_single_instance(app: AppHandle) -> bool {
    chan::acquire(app)
}

// ------------------------------------------------------- 开机自启(macOS / Windows)

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn autostart_entry() -> Result<auto_launch::AutoLaunch, String> {
    use auto_launch::AutoLaunchBuilder;

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // Windows 的 Run 键值是"路径 + 参数"的裸字符串，路径含空格时必须自带引号，
    // 否则登录时会按空格截断导致启动失败；macOS 走登录项不需要加引号。
    #[cfg(target_os = "windows")]
    let app_path = format!("\"{}\"", exe.to_string_lossy());
    #[cfg(not(target_os = "windows"))]
    let app_path = exe.to_string_lossy().to_string();

    let mut builder = AutoLaunchBuilder::new();
    builder.set_app_name("LoudMail").set_app_path(&app_path);
    // Windows 登录时静默启动到托盘，与 Linux 的 .desktop 行为保持一致
    #[cfg(target_os = "windows")]
    builder.set_args(&["--hidden"]);

    builder.build().map_err(|e| e.to_string())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn set_autostart(enable: bool) -> Result<bool, String> {
    let launch = autostart_entry()?;
    if enable {
        launch.enable().map_err(|e| e.to_string())?;
    } else {
        launch.disable().map_err(|e| e.to_string())?;
    }
    launch.is_enabled().map_err(|e| e.to_string())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn is_autostart() -> bool {
    autostart_entry()
        .ok()
        .and_then(|l| l.is_enabled().ok())
        .unwrap_or(false)
}

// -------------------------------------------------------- 开机自启(Linux / 麒麟)

#[cfg(all(unix, not(target_os = "macos")))]
fn autostart_desktop_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("autostart").join("loudmail.desktop"))
}

#[cfg(all(unix, not(target_os = "macos")))]
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

#[cfg(all(unix, not(target_os = "macos")))]
pub fn is_autostart() -> bool {
    autostart_desktop_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}
