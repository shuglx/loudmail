use std::sync::Arc;

use chrono::Local;
use tauri::State;

use crate::logfile;
use crate::models::HistoryRecord;
use crate::state::AppState;

#[tauri::command]
pub fn cmd_list_history(state: State<'_, Arc<AppState>>) -> Vec<HistoryRecord> {
    state
        .history
        .read()
        .map(|h| h.clone())
        .unwrap_or_default()
}

/// 清空记录：只清「无需确认」和「已确认」的，待确认的记录必须保留到人工确认。
/// 返回保留下来的待确认条数。
#[tauri::command]
pub fn cmd_clear_history(state: State<'_, Arc<AppState>>) -> Result<usize, String> {
    let (kept, mail_ids) = {
        let mut history = state.history.write().map_err(|e| e.to_string())?;
        history.retain(|item| item.require_confirm && !item.confirmed);
        (
            history.len(),
            history
                .iter()
                .map(|item| item.mail_id.clone())
                .collect::<Vec<_>>(),
        )
    };
    // 保留的记录仍然占着 mailId 去重位，其余的随记录一起释放
    if let Ok(mut seen) = state.seen.lock() {
        seen.clear();
        seen.extend(mail_ids);
    }
    state.save_history()?;
    logfile::info(format!("清空播报记录，保留 {kept} 条待确认记录"));
    Ok(kept)
}

/// 人工确认：确认后该记录不再参与重复播报，队列里还没开始的重播一并撤掉
#[tauri::command]
pub fn cmd_confirm_history(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<HistoryRecord, String> {
    let updated = {
        let mut history = state.history.write().map_err(|e| e.to_string())?;
        let item = history
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| "播报记录不存在".to_string())?;
        if !item.require_confirm {
            return Err("该记录无需确认".into());
        }
        if !item.confirmed {
            item.confirmed = true;
            item.confirmed_at = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        }
        item.clone()
    };
    // 正在播的那一遍会播完，尚未开始的重播直接丢弃
    state.player.cancel_tag(&id);
    state.save_history()?;
    state.emit_history_updated(&updated);
    logfile::info(format!(
        "人工确认播报记录: [{}] {}",
        updated.strategy_name, updated.subject
    ));
    Ok(updated)
}
