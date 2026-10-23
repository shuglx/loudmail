use std::sync::Arc;

use tauri::State;

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

#[tauri::command]
pub fn cmd_clear_history(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.history.write().map_err(|e| e.to_string())?.clear();
    if let Ok(mut seen) = state.seen.lock() {
        seen.clear();
    }
    state.save_history()
}
