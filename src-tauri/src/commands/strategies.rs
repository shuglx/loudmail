use std::fs;
use std::path::Path;
use std::sync::Arc;

use tauri::api::dialog::blocking::FileDialogBuilder;
use tauri::{AppHandle, Manager, State};

use crate::logfile;
use crate::models::{
    Strategy, StrategyExportFile, StrategyExportItem, StrategyImportPreview, StrategyInput,
};
use crate::state::AppState;
use crate::store;

/// 导出文件的标识，导入时用于校验文件类型
const EXPORT_KIND: &str = "loudmail.strategies";

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
        filter_replies: input.filter_replies,
        sound_id,
        play_count: input.play_count.clamp(1, 3),
        enabled: input.enabled,
    })
}

fn build_export(state: &AppState) -> Result<StrategyExportFile, String> {
    let strategies = state.strategies.read().map_err(|e| e.to_string())?;
    let sounds = state.sounds.read().map_err(|e| e.to_string())?;
    let items = strategies
        .iter()
        .map(|item| StrategyExportItem {
            name: item.name.clone(),
            sender: item.sender.clone(),
            keyword: item.keyword.clone(),
            filter_replies: item.filter_replies,
            sound_id: item.sound_id.clone(),
            sound_name: item
                .sound_id
                .as_ref()
                .and_then(|id| sounds.iter().find(|s| &s.id == id))
                .map(|s| s.name.clone()),
            play_count: item.play_count,
            enabled: item.enabled,
        })
        .collect();
    Ok(StrategyExportFile {
        kind: EXPORT_KIND.into(),
        version: 1,
        exported_at: store::now_iso(),
        items,
    })
}

fn read_export_file(path: &Path) -> Result<StrategyExportFile, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("读取文件失败: {e}"))?;
    let file: StrategyExportFile =
        serde_json::from_str(&raw).map_err(|e| format!("解析策略文件失败: {e}"))?;
    if !file.kind.is_empty() && file.kind != EXPORT_KIND {
        return Err("不是 LoudMail 策略文件".into());
    }
    if file.items.is_empty() {
        return Err("文件中没有策略".into());
    }
    Ok(file)
}

#[tauri::command]
pub async fn cmd_export_strategies(app: AppHandle) -> Result<Option<String>, String> {
    let state = app.state::<Arc<AppState>>();
    let payload = build_export(&state)?;
    let json = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    if let Some(window) = app.get_window("main") {
        let _ = window.set_focus();
    }
    let default_name = format!(
        "loudmail-strategies-{}.json",
        chrono::Local::now().format("%Y%m%d")
    );
    let Some(path) = FileDialogBuilder::new()
        .set_title("导出匹配策略")
        .add_filter("JSON", &["json"])
        .set_file_name(&default_name)
        .save_file()
    else {
        return Ok(None);
    };
    fs::write(&path, json).map_err(|e| format!("写入文件失败: {e}"))?;
    logfile::info(format!(
        "导出匹配策略 {} 条 -> {}",
        payload.items.len(),
        path.display()
    ));
    Ok(Some(path.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn cmd_pick_strategies_file(app: AppHandle) -> Result<Option<StrategyImportPreview>, String> {
    if let Some(window) = app.get_window("main") {
        let _ = window.set_focus();
    }
    let Some(path) = FileDialogBuilder::new()
        .set_title("导入匹配策略")
        .add_filter("JSON", &["json"])
        .pick_file()
    else {
        return Ok(None);
    };
    let file = read_export_file(&path)?;
    Ok(Some(StrategyImportPreview {
        path: path.to_string_lossy().to_string(),
        count: file.items.len(),
    }))
}

#[tauri::command]
pub fn cmd_import_strategies(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<usize, String> {
    let file = read_export_file(Path::new(&path))?;
    let sound_ids: Vec<String> = state
        .sounds
        .read()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|s| s.id.clone())
        .collect();

    let now = store::now_iso();
    let mut imported: Vec<Strategy> = Vec::new();
    for item in &file.items {
        let name = item.name.trim().to_string();
        let keyword = item.keyword.trim().to_string();
        if name.is_empty() || keyword.is_empty() {
            continue;
        }
        // 发件人格式不合法时按「任意发件人」处理
        let sender = normalize_sender(&item.sender).unwrap_or_default();
        let sound_id = item
            .sound_id
            .clone()
            .filter(|id| !id.is_empty() && sound_ids.contains(id));
        imported.push(Strategy {
            id: store::new_id("stg"),
            name,
            enabled: item.enabled,
            sender,
            keyword,
            filter_replies: item.filter_replies,
            sound_id,
            play_count: item.play_count.clamp(1, 3),
            // 游标留空：下一轮扫描只做初始化，不会补播历史邮件
            cursor_seq: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        });
    }

    if imported.is_empty() {
        return Err("文件里没有可用的策略".into());
    }
    let count = imported.len();
    *state.strategies.write().map_err(|e| e.to_string())? = imported;
    state.save_strategies()?;
    logfile::info(format!("导入匹配策略 {count} 条 <- {path}"));
    Ok(count)
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
        filter_replies: input.filter_replies,
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
    item.filter_replies = input.filter_replies;
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
