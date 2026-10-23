use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Local;
use once_cell::sync::Lazy;

const LOG_FILE: &str = "loudmail.log";
const MAX_BYTES: u64 = 2 * 1024 * 1024;

static SINK: Lazy<Mutex<Option<File>>> = Lazy::new(|| Mutex::new(None));
static LOG_PATH: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));

pub fn init(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join(LOG_FILE);
    rotate_if_needed(&path);
    *SINK.lock().unwrap() = OpenOptions::new().create(true).append(true).open(&path).ok();
    *LOG_PATH.lock().unwrap() = Some(path.clone());
    info(format!(
        "======== 启动 LoudMail {} ========",
        env!("CARGO_PKG_VERSION")
    ));
    info(format!("日志文件: {}", path.display()));
}

pub fn path() -> Option<PathBuf> {
    LOG_PATH.lock().ok().and_then(|p| p.clone())
}

pub fn info(msg: impl AsRef<str>) {
    write_line("INFO", msg.as_ref());
}

pub fn warn(msg: impl AsRef<str>) {
    write_line("WARN", msg.as_ref());
}

pub fn error(msg: impl AsRef<str>) {
    write_line("ERROR", msg.as_ref());
}

pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        error(format!("panic: {info}"));
        default(info);
    }));
}

fn write_line(level: &str, msg: &str) {
    let line = format!(
        "{} [{}] {}",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        level,
        msg
    );
    eprintln!("{line}");
    let Ok(mut guard) = SINK.lock() else {
        return;
    };
    if let Some(file) = guard.as_mut() {
        let _ = writeln!(file, "{line}");
        let _ = file.flush();
    }
    if let Ok(path) = LOG_PATH.lock() {
        if let Some(path) = path.as_ref() {
            if rotate_if_needed(path) {
                *guard = OpenOptions::new().create(true).append(true).open(path).ok();
            }
        }
    }
}

fn rotate_if_needed(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() <= MAX_BYTES {
        return false;
    }
    let old = path.with_extension("log.1");
    let _ = std::fs::remove_file(&old);
    let _ = std::fs::rename(path, old);
    true
}
