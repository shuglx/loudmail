use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use include_dir::{include_dir, Dir};
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::logfile;
use crate::models::*;

/// 编译期嵌入源码里的 alert-sound 目录：目录中放什么音频，运行时就有哪些内置铃声
static ALERT_SOUNDS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../alert-sound");

/// 旧版内置铃声 ID → 新的文件名（文件名同时作为语音名与 ID）
const LEGACY_IDS: [(&str, &str); 5] = [
    ("snd_default", "轻微提示音"),
    ("snd_emergency", "普通告警音"),
    ("snd_notice", "紧急告警音"),
    ("snd_fire", "火灾告警音"),
    ("snd_gas", "气体告警音"),
];

/// 旧内置 ID 对应的新内置 ID（文件名），用于迁移历史策略引用
pub fn legacy_builtin_id(id: &str) -> Option<&'static str> {
    LEGACY_IDS
        .iter()
        .find(|(old, _)| *old == id)
        .map(|(_, new)| *new)
}

#[derive(Clone, Debug)]
pub struct BuiltinSound {
    /// 文件名（不含扩展名）同时作为语音 ID 与展示名
    pub id: String,
    pub name: String,
    pub bytes: &'static [u8],
    pub filename: String,
}

/// 扫描内置铃声目录：文件名（不含扩展名）即语音名与 ID
pub fn builtins() -> Vec<BuiltinSound> {
    let mut files: Vec<_> = ALERT_SOUNDS.files().collect();
    files.sort_by(|a, b| a.path().cmp(b.path()));

    let mut found: Vec<BuiltinSound> = Vec::new();
    for file in files {
        let path = file.path();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !matches!(ext.as_str(), "ogg" | "wav" | "mp3") {
            continue;
        }
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        if found.iter().any(|item| item.id == stem) {
            continue;
        }
        found.push(BuiltinSound {
            filename: format!("{stem}.wav"),
            name: stem.clone(),
            id: stem,
            bytes: file.contents(),
        });
    }

    // 早期 5 个固定铃声保持原有顺序，其余按文件名排序
    let rank = |id: &str| {
        LEGACY_IDS
            .iter()
            .position(|(_, new)| *new == id)
            .unwrap_or(LEGACY_IDS.len())
    };
    found.sort_by(|a, b| rank(&a.id).cmp(&rank(&b.id)).then_with(|| a.id.cmp(&b.id)));
    found
}

/// 内置铃声由源码目录统一管理：不可删除、不可重命名，且是录音 / TTS 合成时的唯一铃声素材
pub fn is_protected_builtin(id: &str) -> bool {
    builtins().iter().any(|item| item.id == id)
}

const SOUNDS_VERSION: u32 = 7;

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
    // 初始化时补齐数据文件，保证「设置 → 存储」中的条目始终可打开
    if !paths.config.exists() {
        save_config(&paths, &AppConfig::default())?;
    }
    if !paths.strategies.exists() {
        save_strategies(&paths, &[])?;
    }
    if !paths.history.exists() {
        save_history(&paths, &[])?;
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
        cfg.scan.lookback_min = 10;
    }
    if !["light", "dark"].contains(&cfg.theme.as_str()) {
        cfg.theme = "light".into();
    }
    cfg
}

pub fn save_config(paths: &AppPaths, cfg: &AppConfig) -> Result<(), String> {
    write_json(&paths.config, cfg)
}

pub fn load_strategies(paths: &AppPaths) -> Vec<Strategy> {
    let mut items = read_json::<StrategiesFile>(&paths.strategies)
        .map(|f| f.items)
        .unwrap_or_default();
    // 内置铃声 ID 改为文件名后，同步迁移历史策略里的引用
    let mut migrated = false;
    for item in items.iter_mut() {
        if let Some(new_id) = item.sound_id.as_deref().and_then(legacy_builtin_id) {
            item.sound_id = Some(new_id.to_string());
            migrated = true;
        }
        // 非法或历史遗留的匹配方式统一归为模糊匹配
        if !["contains", "prefix", "suffix"].contains(&item.match_mode.as_str()) {
            item.match_mode = "contains".into();
            migrated = true;
        }
    }
    if migrated {
        let _ = save_strategies(paths, &items);
    }
    items
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

/// 播报记录滚动上限；待确认（开启人工确认且未确认）的记录不受该上限约束
pub const HISTORY_LIMIT: usize = 1000;

/// 超出上限时从尾部淘汰「无需确认 / 已确认」的记录，待确认的一律保留
pub fn cap_history(items: &mut Vec<HistoryRecord>) {
    if items.len() <= HISTORY_LIMIT {
        return;
    }
    let mut index = items.len();
    while items.len() > HISTORY_LIMIT && index > 0 {
        index -= 1;
        let pending = items[index].require_confirm && !items[index].confirmed;
        if !pending {
            items.remove(index);
        }
    }
}

pub fn load_history(paths: &AppPaths) -> Vec<HistoryRecord> {
    let mut items = read_json::<HistoryFile>(&paths.history)
        .map(|f| f.items)
        .unwrap_or_default();
    cap_history(&mut items);
    items
}

pub fn save_history(paths: &AppPaths, items: &[HistoryRecord]) -> Result<(), String> {
    let mut capped = items.to_vec();
    cap_history(&mut capped);
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
    let Some(mut file) = read_json::<SoundsFile>(&paths.sounds_json) else {
        return Vec::new();
    };
    let mut changed = false;

    if file.version < 5 {
        // v5：移除蜂鸣告警音
        let removed = file.items.iter().any(|item| item.id == "snd_buzzer");
        file.items.retain(|item| item.id != "snd_buzzer");
        if removed {
            let _ = fs::remove_file(paths.sounds_dir.join("snd_buzzer.wav"));
        }
        changed = true;
    }

    if file.version < SOUNDS_VERSION {
        // v7：内置铃声 ID 改为文件名，迁移旧固定 ID
        for item in file.items.iter_mut() {
            if item.source != "builtin" {
                continue;
            }
            if let Some(new_id) = legacy_builtin_id(&item.id) {
                item.id = new_id.to_string();
            }
        }
        file.version = SOUNDS_VERSION;
        changed = true;
    }

    let defs = builtins();

    // 目录中已删除的内置铃声同步移除（含测试残留）
    let mut index = 0;
    while index < file.items.len() {
        let stale = file.items[index].source == "builtin"
            && !defs.iter().any(|def| def.id == file.items[index].id);
        if stale {
            let removed = file.items.remove(index);
            let _ = fs::remove_file(paths.sounds_dir.join(&removed.file));
            changed = true;
        } else {
            index += 1;
        }
    }

    // 以目录为准生成 / 刷新内置铃声：新增文件自动注册，素材替换后时长同步更新
    for def in &defs {
        let Some(mut item) = build_builtin_sound(paths, def) else {
            continue;
        };
        match file.items.iter_mut().find(|existing| existing.id == item.id) {
            Some(existing) => {
                item.created_at = existing.created_at.clone();
                let duration_ms = item.duration_ms;
                let old_item = std::mem::replace(existing, item);
                if old_item.file != def.filename {
                    let _ = fs::remove_file(paths.sounds_dir.join(old_item.file));
                }
                if old_item.name != def.name || old_item.duration_ms != duration_ms {
                    changed = true;
                }
            }
            None => {
                file.items.push(item);
                changed = true;
            }
        }
    }

    // 排序：内置语音永远排在用户新增的语音之前；内置之间按 alert-sound 目录顺序
    let builtin_order: Vec<&str> = defs.iter().map(|def| def.id.as_str()).collect();
    let rank = |item: &SoundItem| -> (bool, usize) {
        if item.source != "builtin" {
            return (true, usize::MAX);
        }
        let position = builtin_order
            .iter()
            .position(|id| *id == item.id)
            .unwrap_or(usize::MAX);
        (false, position)
    };
    let before: Vec<String> = file.items.iter().map(|item| item.id.clone()).collect();
    file.items.sort_by_key(rank);
    if file
        .items
        .iter()
        .map(|item| item.id.as_str())
        .ne(before.iter().map(|id| id.as_str()))
    {
        changed = true;
    }

    if changed {
        let _ = save_sounds(paths, &file.items);
    }
    file.items
}

pub fn save_sounds(paths: &AppPaths, items: &[SoundItem]) -> Result<(), String> {
    write_json(
        &paths.sounds_json,
        &SoundsFile {
            version: SOUNDS_VERSION,
            items: items.to_vec(),
        },
    )
}

fn ensure_builtin_sounds(paths: &AppPaths) {
    // 仅在首次运行时写入内置语音；之后由 load_sounds 按目录补齐
    if paths.sounds_json.exists() {
        return;
    }
    let items: Vec<SoundItem> = builtins()
        .iter()
        .filter_map(|def| build_builtin_sound(paths, def))
        .collect();
    if let Err(e) = save_sounds(paths, &items) {
        logfile::error(format!("保存内置语音元数据失败: {e}"));
    }
}

fn build_builtin_sound(paths: &AppPaths, def: &BuiltinSound) -> Option<SoundItem> {
    let target = paths.sounds_dir.join(&def.filename);
    let duration_ms = match crate::audio::convert::decode_bytes_to_wav(def.bytes) {
        Ok((wav, ms)) => {
            if let Err(e) = fs::write(&target, wav) {
                logfile::error(format!("写入内置语音失败 {}: {e}", target.display()));
                return None;
            }
            ms
        }
        Err(e) => {
            logfile::error(format!("转换内置语音失败 {}: {e}", def.id));
            return None;
        }
    };
    Some(SoundItem {
        id: def.id.clone(),
        name: def.name.clone(),
        file: def.filename.clone(),
        duration_ms,
        source: "builtin".into(),
        created_at: now_iso(),
    })
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

#[cfg(test)]
mod tests {
    use super::{cap_history, HistoryRecord, HISTORY_LIMIT};

    fn record(id: usize, require_confirm: bool, confirmed: bool) -> HistoryRecord {
        HistoryRecord {
            id: format!("his_{id}"),
            time: "2026-10-01 10:00:00".into(),
            strategy_id: "stg_1".into(),
            strategy_name: "SOC".into(),
            mail_id: format!("mock:{id}"),
            sender: "a@b.com".into(),
            subject: format!("告警 {id}"),
            summary: String::new(),
            sound_name: "普通告警音".into(),
            play_count: 1,
            require_confirm,
            confirmed,
            confirmed_at: None,
            sound_id: None,
            rebroadcast_count: 0,
            last_broadcast_at: None,
        }
    }

    #[test]
    fn cap_removes_oldest_handled_records() {
        let mut items: Vec<HistoryRecord> = (0..HISTORY_LIMIT + 1)
            .map(|i| record(i, false, false))
            .collect();
        cap_history(&mut items);
        assert_eq!(items.len(), HISTORY_LIMIT);
        // 列表是新→旧，尾部（最老的一条）被淘汰
        assert_eq!(items.first().unwrap().id, "his_0");
        assert_eq!(items.last().unwrap().id, format!("his_{}", HISTORY_LIMIT - 1));
    }

    #[test]
    fn cap_keeps_pending_records_even_at_the_tail() {
        let mut items: Vec<HistoryRecord> = (0..HISTORY_LIMIT + 1)
            .map(|i| record(i, false, false))
            .collect();
        // 最老的一条仍是待确认：不能被滚动上限挤掉
        *items.last_mut().unwrap() = record(HISTORY_LIMIT, true, false);
        cap_history(&mut items);
        assert_eq!(items.len(), HISTORY_LIMIT);
        assert_eq!(
            items.last().unwrap().id,
            format!("his_{HISTORY_LIMIT}")
        );
        // 被淘汰的是倒数第二条（无需确认）
        assert!(items.iter().all(|item| item.id != format!("his_{}", HISTORY_LIMIT - 1)));
    }

    #[test]
    fn cap_can_exceed_limit_when_everything_is_pending() {
        let mut items: Vec<HistoryRecord> = (0..HISTORY_LIMIT + 5)
            .map(|i| record(i, true, false))
            .collect();
        cap_history(&mut items);
        assert_eq!(items.len(), HISTORY_LIMIT + 5);
    }

    #[test]
    fn confirmed_pending_record_becomes_evictable() {
        let mut items: Vec<HistoryRecord> = (0..HISTORY_LIMIT + 1)
            .map(|i| record(i, true, true))
            .collect();
        cap_history(&mut items);
        assert_eq!(items.len(), HISTORY_LIMIT);
    }
}
