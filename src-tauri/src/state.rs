use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex, RwLock};

use tauri::{AppHandle, Manager};

use crate::audio::player::{start as start_player, PlayerHandle};
use crate::audio::recorder::RecorderHandle;
use crate::mcp::mock;
use crate::models::*;
use crate::store::{self, AppPaths};

/// mock 策略默认使用内置「紧急告警音」：全新安装也一定存在，便于直接验证播报
const MOCK_SOUND_ID: &str = "紧急告警音";

/// mock 开关打开时临时插入的匹配策略（固定 id，排在列表第一）
fn build_mock_strategy(sound_id: Option<String>) -> Strategy {
    let now = store::now_iso();
    // 游标停在 BASE_SEQ 之前：打开模拟邮箱后第一封模拟邮件即可命中播报，
    // 不必像新建策略那样先"初始化"一轮
    let mut cursor = CursorSeq::default();
    cursor.advance(mock::SOURCE, mock::BASE_SEQ.saturating_sub(1));
    Strategy {
        id: MOCK_STRATEGY_ID.into(),
        name: "[mock] 模拟邮箱告警".into(),
        enabled: true,
        sender: mock::EMAIL.into(),
        keyword: mock::KEYWORD.into(),
        match_mode: "contains".into(),
        filter_replies: true,
        require_confirm: false,
        sound_id,
        play_count: 1,
        cursor_seq: cursor,
        created_at: now.clone(),
        updated_at: now,
    }
}

/// 按 mock 开关维护临时策略：开启时确保存在且位于第一，关闭时移除。
/// 返回列表是否发生变化（用于通知前端刷新）。
fn sync_mock_strategy_list(
    strategies: &mut Vec<Strategy>,
    mock_on: bool,
    sound_id: Option<String>,
) -> bool {
    let position = strategies.iter().position(|s| s.id == MOCK_STRATEGY_ID);
    if !mock_on {
        if position.is_some() {
            strategies.retain(|s| s.id != MOCK_STRATEGY_ID);
            return true;
        }
        return false;
    }
    match position {
        Some(index) => {
            let mut changed = index != 0;
            let mut item = strategies.remove(index);
            if item.sender != mock::EMAIL || item.keyword != mock::KEYWORD {
                item.sender = mock::EMAIL.into();
                item.keyword = mock::KEYWORD.into();
                changed = true;
            }
            if item.sound_id.is_none() && sound_id.is_some() {
                item.sound_id = sound_id;
                changed = true;
            }
            if !item.enabled {
                item.enabled = true;
                changed = true;
            }
            strategies.insert(0, item);
            changed
        }
        None => {
            strategies.insert(0, build_mock_strategy(sound_id));
            true
        }
    }
}

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
        // mock 开关是持久化配置：启动时按它补入 / 清理临时策略
        let fallback_sound = sounds
            .iter()
            .find(|item| item.id == MOCK_SOUND_ID)
            .map(|item| item.id.clone())
            .or_else(|| {
                strategies
                    .iter()
                    .find(|item| item.id != MOCK_STRATEGY_ID)
                    .and_then(|item| item.sound_id.clone())
            })
            .or_else(|| sounds.first().map(|item| item.id.clone()));
        sync_mock_strategy_list(&mut strategies, config.behavior.mock, fallback_sound);
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

    /// mock 开关变化时调用：增删临时策略并通知前端刷新列表
    pub fn sync_mock_strategy(&self) -> Result<bool, String> {
        let mock_on = self
            .config
            .read()
            .map(|c| c.behavior.mock)
            .unwrap_or(false);
        let sound_id = self
            .sounds
            .read()
            .ok()
            .and_then(|list| {
                list.iter()
                    .find(|item| item.id == MOCK_SOUND_ID)
                    .map(|item| item.id.clone())
            })
            .or_else(|| {
                self.strategies
                    .read()
                    .ok()
                    .and_then(|list| {
                        list.iter()
                            .find(|item| item.id != MOCK_STRATEGY_ID)
                            .and_then(|item| item.sound_id.clone())
                    })
            })
            .or_else(|| {
                self.sounds
                    .read()
                    .ok()
                    .and_then(|list| list.first().map(|item| item.id.clone()))
            });
        let changed = {
            let mut strategies = self.strategies.write().map_err(|e| e.to_string())?;
            sync_mock_strategy_list(&mut strategies, mock_on, sound_id)
        };
        if changed {
            let _ = self.app.emit_all("strategies://changed", ());
        }
        Ok(changed)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn real_strategy(id: &str) -> Strategy {
        Strategy {
            id: id.into(),
            name: "真实策略".into(),
            enabled: true,
            sender: "a@b.com".into(),
            keyword: "告警".into(),
            match_mode: "contains".into(),
            filter_replies: true,
            require_confirm: false,
            sound_id: Some("snd".into()),
            play_count: 1,
            cursor_seq: CursorSeq::default(),
            created_at: "2026-01-01T00:00:00+08:00".into(),
            updated_at: "2026-01-01T00:00:00+08:00".into(),
        }
    }

    #[test]
    fn mock_strategy_is_inserted_first_and_removed_on_close() {
        let mut list = vec![real_strategy("stg_real")];
        assert!(sync_mock_strategy_list(&mut list, true, Some("snd".into())));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, MOCK_STRATEGY_ID);
        assert!(list[0].name.starts_with("[mock]"));
        assert_eq!(list[0].sender, mock::EMAIL);
        assert_eq!(list[0].keyword, mock::KEYWORD);
        assert_eq!(
            list[0].cursor_seq.get(mock::SOURCE),
            Some(mock::BASE_SEQ - 1)
        );

        assert!(sync_mock_strategy_list(&mut list, false, None));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "stg_real");
    }

    #[test]
    fn existing_mock_strategy_is_moved_to_front() {
        let mut list = vec![real_strategy("stg_real"), real_strategy(MOCK_STRATEGY_ID)];
        assert!(sync_mock_strategy_list(&mut list, true, Some("snd".into())));
        assert_eq!(list[0].id, MOCK_STRATEGY_ID);
        // 已存在于第一位时再同步不产生变化
        assert!(!sync_mock_strategy_list(&mut list, true, Some("snd".into())));
    }
}
