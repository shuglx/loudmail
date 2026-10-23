use std::sync::Arc;

use tauri::State;

use crate::models::{Strategy, StrategyInput};
use crate::state::AppState;
use crate::store;

fn normalize_sender(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    let email = if let (Some(start), Some(end)) = (value.rfind('<'), value.rfind('>')) {
        if end > start {
            value[start + 1..end].trim()
        } else {
            value
        }
    } else {
        value
    };
    if email.contains('@')
        && email.contains('.')
        && !email.chars().any(char::is_whitespace)
        && email.len() <= 254
    {
        Ok(email.to_lowercase())
    } else {
        Err("发件人邮箱格式不正确".into())
    }
}

fn validate(state: &AppState, input: &StrategyInput) -> Result<StrategyInput, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 40 {
        return Err("策略名称必填，且不超过 40 字".into());
    }
    let keyword = input.keyword.trim().to_string();
    if keyword.is_empty() || keyword.chars().count() > 100 {
        return Err("标题关键词必填，且不超过 100 字".into());
    }
    let sender = normalize_sender(&input.sender)?;
    let sound_id = input.sound_id.clone().filter(|s| !s.is_empty());
    if sound_id.is_none() {
        return Err("请选择报警音频".into());
    }
    if state.sound(sound_id.as_deref().unwrap_or_default()).is_none() {
        return Err("所选语音不存在，请先在语音库添加".into());
    }
    Ok(StrategyInput {
        name,
        sender,
        keyword,
        sound_id,
        play_count: input.play_count.clamp(1, 3),
        enabled: input.enabled,
    })
}

#[tauri::command]
pub fn cmd_list_strategies(state: State<'_, Arc<AppState>>) -> Vec<Strategy> {
    state
        .strategies
        .read()
        .map(|s| s.clone())
        .unwrap_or_default()
}

#[tauri::command]
pub fn cmd_create_strategy(
    state: State<'_, Arc<AppState>>,
    input: StrategyInput,
) -> Result<Strategy, String> {
    let input = validate(&state, &input)?;
    let now = store::now_iso();
    let strategy = Strategy {
        id: store::new_id("stg"),
        name: input.name,
        enabled: input.enabled,
        sender: input.sender,
        keyword: input.keyword,
        sound_id: input.sound_id,
        play_count: input.play_count,
        cursor_seq: None,
        created_at: now.clone(),
        updated_at: now,
    };
    state
        .strategies
        .write()
        .map_err(|e| e.to_string())?
        .push(strategy.clone());
    state.save_strategies()?;
    Ok(strategy)
}

#[tauri::command]
pub fn cmd_update_strategy(
    state: State<'_, Arc<AppState>>,
    id: String,
    input: StrategyInput,
) -> Result<Strategy, String> {
    let input = validate(&state, &input)?;
    let mut strategies = state.strategies.write().map_err(|e| e.to_string())?;
    let item = strategies
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| "策略不存在".to_string())?;

    let cond_changed = item.sender != input.sender || item.keyword != input.keyword;
    item.name = input.name;
    item.enabled = input.enabled;
    item.sender = input.sender;
    item.keyword = input.keyword;
    item.sound_id = input.sound_id;
    item.play_count = input.play_count;
    item.updated_at = store::now_iso();
    if cond_changed {
        item.cursor_seq = None;
    }
    let result = item.clone();
    drop(strategies);
    state.save_strategies()?;
    Ok(result)
}

#[tauri::command]
pub fn cmd_delete_strategy(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state
        .strategies
        .write()
        .map_err(|e| e.to_string())?
        .retain(|s| s.id != id);
    state.save_strategies()
}

#[tauri::command]
pub fn cmd_set_strategy_enabled(
    state: State<'_, Arc<AppState>>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    if let Some(item) = state
        .strategies
        .write()
        .map_err(|e| e.to_string())?
        .iter_mut()
        .find(|s| s.id == id)
    {
        item.enabled = enabled;
        item.updated_at = store::now_iso();
    }
    state.save_strategies()
}

#[tauri::command]
pub fn cmd_reorder_strategies(
    state: State<'_, Arc<AppState>>,
    ids: Vec<String>,
) -> Result<Vec<Strategy>, String> {
    let mut strategies = state.strategies.write().map_err(|e| e.to_string())?;
    let mut ordered = Vec::with_capacity(strategies.len());
    for id in &ids {
        if let Some(item) = strategies.iter().find(|s| &s.id == id) {
            ordered.push(item.clone());
        }
    }
    for item in strategies.iter() {
        if !ids.contains(&item.id) {
            ordered.push(item.clone());
        }
    }
    *strategies = ordered;
    let result = strategies.clone();
    drop(strategies);
    state.save_strategies()?;
    Ok(result)
}
