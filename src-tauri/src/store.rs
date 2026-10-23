use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::logfile;
use crate::models::*;

const BUILTINS: [(&str, &str, &[u8], &str); 3] = [
    ("snd_default", "默认警报", include_bytes!("../../alert-sound/1.ogg"), "snd_default.wav"),
    ("snd_emergency", "紧急警报", include_bytes!("../../alert-sound/2.ogg"), "snd_emergency.wav"),
    ("snd_notice", "提示音", include_bytes!("../../alert-sound/3.ogg"), "snd_notice.wav"),
];

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub root: PathBuf,
    pub config: PathBuf,
    pub strategies: PathBuf,
    pub history: PathBuf,
    pub sounds_json: PathBuf,
    pub sounds_dir: PathBuf,
    pub recordings_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub log_file: PathBuf,
}

pub fn init(root: PathBuf) -> Result<AppPaths, String> {
    let paths = AppPaths {
        config: root.join("config.json"),
        strategies: root.join("strategies.json"),
        history: root.join("history.json"),
        sounds_json: root.join("sounds.json"),
        sounds_dir: root.join("sounds"),
        recordings_dir: root.join("recordings"),
        logs_dir: root.join("logs"),
        log_file: root.join("logs").join("loudmail.log"),
        root,
    };
    for dir in [&paths.root, &paths.sounds_dir, &paths.recordings_dir, &paths.logs_dir] {
        fs::create_dir_all(dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    }
    Ok(paths)
}

pub fn load_config(paths: &AppPaths) -> AppConfig {
    let mut cfg: AppConfig = read_json(&paths.config).unwrap_or_default();
    if cfg.version == 0 {
        cfg.version = 1;
    }
    cfg.mcp.host = cfg.mcp.host.trim().to_string();
    if cfg.mcp.host.is_empty() {
        cfg.mcp.host = "127.0.0.1".into();
    }
    if cfg.mcp.port == 0 {
        cfg.mcp.port = 11328;
    }
    if ![60, 120, 180].contains(&cfg.scan.interval_sec) {
        cfg.scan.interval_sec = 60;
    }
    if ![10, 30, 60].contains(&cfg.scan.lookback_min) {
        cfg.scan.lookback_min = 60;
    }
    cfg
}

pub fn save_config(paths: &AppPaths, cfg: &AppConfig) -> Result<(), String> {
    write_json(&paths.config, cfg)
}

pub fn load_strategies(paths: &AppPaths) -> Vec<Strategy> {
    read_json::<StrategiesFile>(&paths.strategies)
        .map(|f| f.items)
        .unwrap_or_default()
}

pub fn save_strategies(paths: &AppPaths, items: &[Strategy]) -> Result<(), String> {
    write_json(
        &paths.strategies,
        &StrategiesFile {
            version: 1,
            items: items.to_vec(),
        },
    )
}

pub fn load_history(paths: &AppPaths) -> Vec<HistoryRecord> {
    let mut items = read_json::<HistoryFile>(&paths.history)
        .map(|f| f.items)
        .unwrap_or_default();
    items.truncate(500);
    items
}

pub fn save_history(paths: &AppPaths, items: &[HistoryRecord]) -> Result<(), String> {
    let mut capped = items.to_vec();
    capped.truncate(500);
    write_json(
        &paths.history,
        &HistoryFile {
            version: 1,
            items: capped,
        },
    )
}

pub fn load_sounds(paths: &AppPaths) -> Vec<SoundItem> {
    ensure_builtin_sounds(paths);
    read_json::<SoundsFile>(&paths.sounds_json)
        .map(|f| f.items)
        .unwrap_or_default()
}

pub fn save_sounds(paths: &AppPaths, items: &[SoundItem]) -> Result<(), String> {
    write_json(
        &paths.sounds_json,
        &SoundsFile {
            version: 1,
            items: items.to_vec(),
        },
    )
}

fn ensure_builtin_sounds(paths: &AppPaths) {
    // 仅在首次运行时写入内置语音；用户之后删除内置语音也不会被重新加回来
    if paths.sounds_json.exists() {
        return;
    }
    let mut items = Vec::new();
    for (id, name, bytes, filename) in BUILTINS {
        let target = paths.sounds_dir.join(filename);
        let duration_ms = match crate::audio::convert::decode_bytes_to_wav(bytes) {
            Ok((wav, ms)) => {
                if let Err(e) = fs::write(&target, wav) {
                    logfile::error(format!("写入内置语音失败 {}: {e}", target.display()));
                    continue;
                }
                ms
            }
            Err(e) => {
                logfile::error(format!("转换内置语音失败 {id}: {e}"));
                continue;
            }
        };
        items.push(SoundItem {
            id: id.to_string(),
            name: name.to_string(),
            file: filename.to_string(),
            duration_ms,
            source: "builtin".into(),
            created_at: now_iso(),
        });
    }
    if let Err(e) = save_sounds(paths, &items) {
        logfile::error(format!("保存内置语音元数据失败: {e}"));
    }
}

pub fn storage_files(paths: &AppPaths) -> Vec<StorageFile> {
    vec![
        storage("配置文件", &paths.config, "file"),
        storage("策略文件", &paths.strategies, "file"),
        storage("播报记录", &paths.history, "file"),
        storage("语音元数据", &paths.sounds_json, "file"),
        storage("语音目录", &paths.sounds_dir, "dir"),
        storage("日志", &paths.log_file, "file"),
    ]
}

fn storage(label: &str, path: &Path, kind: &str) -> StorageFile {
    StorageFile {
        label: label.into(),
        path: path.to_string_lossy().to_string(),
        kind: kind.into(),
    }
}

pub fn now_iso() -> String {
    Local::now().to_rfc3339()
}

pub fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    match serde_json::from_str::<T>(&text) {
        Ok(value) => Some(value),
        Err(e) => {
            logfile::error(format!("读取 {} 失败: {e}，已备份并重建", path.display()));
            let backup = path.with_extension("bak");
            let _ = fs::remove_file(&backup);
            let _ = fs::rename(path, backup);
            None
        }
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}
