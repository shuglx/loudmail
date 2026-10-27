use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex, RwLock};

use tauri::{AppHandle, Manager};

use crate::audio::player::{start as start_player, PlayerHandle};
use crate::audio::recorder::RecorderHandle;
use crate::models::*;
use crate::store::{self, AppPaths};

pub struct AppState {
    pub app: AppHandle,
    pub paths: AppPaths,
    pub config: RwLock<AppConfig>,
    pub strategies: RwLock<Vec<Strategy>>,
    pub history: RwLock<Vec<HistoryRecord>>,
    pub sounds: RwLock<Vec<SoundItem>>,
    pub runtime: RwLock<RuntimeStatus>,
    pub seen: Mutex<HashSet<String>>,
    pub config_version: AtomicU64,
    pub paused: AtomicBool,
    /// 恢复播报后的首轮扫描跳过暂停期间的积压邮件
    pub skip_backlog: AtomicBool,
    pub player: PlayerHandle,
    pub recorder: RecorderHandle,
}

impl AppState {
    pub fn load(app: AppHandle, paths: AppPaths) -> Result<Arc<Self>, String> {
        let config = store::load_config(&paths);
        let mut strategies = store::load_strategies(&paths);
        let history = store::load_history(&paths);
        let sounds = store::load_sounds(&paths);
        // 语音被移除（如内置铃声下线）时，停用引用它的策略，避免无声报警
        for item in &mut strategies {
            if let Some(id) = item.sound_id.as_deref() {
                if !sounds.iter().any(|sound| sound.id == id) {
                    item.sound_id = None;
                    item.enabled = false;
                }
            }
        }
        let mut seen = HashSet::new();
        for record in &history {
            seen.insert(record.mail_id.clone());
        }
        let runtime = RuntimeStatus {
            connected: false,
            server_name: None,
            server_version: None,
            last_error: None,
            mock: config.behavior.mock,
        };
        let recorder = RecorderHandle::new(paths.recordings_dir.clone());
        let player = start_player(app.clone());
        Ok(Arc::new(Self {
            app,
            paths,
            config: RwLock::new(config),
            strategies: RwLock::new(strategies),
            history: RwLock::new(history),
            sounds: RwLock::new(sounds),
            runtime: RwLock::new(runtime),
            seen: Mutex::new(seen),
            config_version: AtomicU64::new(0),
            paused: AtomicBool::new(false),
            skip_backlog: AtomicBool::new(false),
            player,
            recorder,
        }))
    }

    pub fn save_strategies(&self) -> Result<(), String> {
        store::save_strategies(&self.paths, &self.strategies.read().unwrap())
    }

    pub fn save_history(&self) -> Result<(), String> {
        let mut history = self.history.write().map_err(|e| e.to_string())?;
        // 这里再兜一次：某条记录被确认后，就该按滚动上限淘汰了
        store::cap_history(&mut history);
        store::save_history(&self.paths, &history)
    }

    pub fn save_sounds(&self) -> Result<(), String> {
        store::save_sounds(&self.paths, &self.sounds.read().unwrap())
    }

    pub fn save_config(&self) -> Result<(), String> {
        store::save_config(&self.paths, &self.config.read().unwrap())
    }

    /// 标记为不可达：保留上次的服务名/版本，仅更新连接状态与错误
    pub fn mark_disconnected(&self, error: Option<String>) {
        let mut next = self.runtime.read().map(|r| r.clone()).unwrap_or_default();
        next.connected = false;
        next.last_error = error;
        self.set_runtime(next);
    }

    pub fn set_runtime(&self, status: RuntimeStatus) {
        if let Ok(mut guard) = self.runtime.write() {
            *guard = status.clone();
        }
        let _ = self.app.emit_all("runtime://status", status);
    }

    pub fn emit_history(&self, record: &HistoryRecord) {
        let _ = self.app.emit_all("history://new", record.clone());
    }

    /// 记录被修改（人工确认）：前端原位替换该条
    pub fn emit_history_updated(&self, record: &HistoryRecord) {
        let _ = self.app.emit_all("history://updated", record.clone());
    }

    /// 未确认记录被重复播报：前端按数组顺序把它们移到列表最前
    pub fn emit_history_rebroadcast(&self, records: &[HistoryRecord]) {
        if records.is_empty() {
            return;
        }
        let _ = self.app.emit_all("history://rebroadcast", records.to_vec());
    }

    pub fn sound(&self, id: &str) -> Option<SoundItem> {
        self.sounds
            .read()
            .ok()?
            .iter()
            .find(|s| s.id == id)
            .cloned()
    }
}
