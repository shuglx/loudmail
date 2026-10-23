use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::api::dialog::blocking::FileDialogBuilder;
use tauri::State;

use crate::audio::convert;
use crate::logfile;
use crate::models::{PickedFile, RecordingResult, SoundItem};
use crate::state::AppState;
use crate::store;

const MAX_BYTES: u64 = 20 * 1024 * 1024;

#[tauri::command]
pub fn cmd_list_sounds(state: State<'_, Arc<AppState>>) -> Vec<SoundItem> {
    state
        .sounds
        .read()
        .map(|s| s.clone())
        .unwrap_or_default()
}

#[tauri::command]
pub fn cmd_pick_audio_file() -> Option<PickedFile> {
    let path = FileDialogBuilder::new()
        .add_filter("音频文件", &["wav", "mp3", "ogg"])
        .pick_file()?;
    let meta = std::fs::metadata(&path).ok()?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "音频".into());
    Some(PickedFile {
        path: path.to_string_lossy().to_string(),
        name,
        size: meta.len(),
    })
}

#[tauri::command]
pub fn cmd_import_sound(
    state: State<'_, Arc<AppState>>,
    path: String,
    name: String,
) -> Result<SoundItem, String> {
    let input = PathBuf::from(&path);
    let meta = std::fs::metadata(&input).map_err(|e| format!("读取文件失败: {e}"))?;
    if meta.len() > MAX_BYTES {
        return Err("音频文件超过 20MB".into());
    }
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !["wav", "mp3", "ogg"].contains(&ext.as_str()) {
        return Err("仅支持 WAV / MP3 / OGG".into());
    }
    let mut display_name = name.trim().to_string();
    if display_name.is_empty() {
        display_name = input
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "新语音".into());
    }
    if display_name.chars().count() > 40 {
        return Err("语音名称不超过 40 字".into());
    }

    let id = store::new_id("snd");
    let output = state.paths.sounds_dir.join(format!("{id}.wav"));
    let duration_ms = match convert::convert_file(&input, &output) {
        Ok(ms) => ms,
        Err(e) => {
            let _ = std::fs::remove_file(&output);
            return Err(e);
        }
    };
    let sound = SoundItem {
        id,
        name: display_name,
        file: output
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        duration_ms,
        source: "upload".into(),
        created_at: store::now_iso(),
    };
    state
        .sounds
        .write()
        .map_err(|e| e.to_string())?
        .push(sound.clone());
    if let Err(e) = state.save_sounds() {
        let _ = std::fs::remove_file(&output);
        return Err(e);
    }
    logfile::info(format!("导入语音 {} -> {}", path, output.display()));
    Ok(sound)
}

#[tauri::command]
pub fn cmd_start_recording(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.recorder.start()
}

#[tauri::command]
pub fn cmd_stop_recording(state: State<'_, Arc<AppState>>) -> Result<RecordingResult, String> {
    state.recorder.stop()
}

#[tauri::command]
pub fn cmd_save_recording(
    state: State<'_, Arc<AppState>>,
    temp_path: String,
    name: String,
    duration_ms: u64,
) -> Result<SoundItem, String> {
    let display_name = name.trim().to_string();
    if display_name.is_empty() || display_name.chars().count() > 40 {
        return Err("请填写 1~40 字的语音名称".into());
    }
    let source = PathBuf::from(&temp_path);
    let recordings = state
        .paths
        .recordings_dir
        .canonicalize()
        .unwrap_or_else(|_| state.paths.recordings_dir.clone());
    let canonical = source
        .canonicalize()
        .map_err(|e| format!("录音临时文件不存在: {e}"))?;
    if !canonical.starts_with(&recordings) {
        return Err("录音文件路径不合法".into());
    }

    let id = store::new_id("snd");
    let target = state.paths.sounds_dir.join(format!("{id}.wav"));
    if std::fs::rename(&canonical, &target).is_err() {
        std::fs::copy(&canonical, &target).map_err(|e| format!("保存录音失败: {e}"))?;
        let _ = std::fs::remove_file(&canonical);
    }
    let sound = SoundItem {
        id,
        name: display_name,
        file: target
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        duration_ms,
        source: "record".into(),
        created_at: store::now_iso(),
    };
    state
        .sounds
        .write()
        .map_err(|e| e.to_string())?
        .push(sound.clone());
    state.save_sounds()?;
    Ok(sound)
}

#[tauri::command]
pub fn cmd_rename_sound(
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
) -> Result<(), String> {
    let display_name = name.trim().to_string();
    if display_name.is_empty() || display_name.chars().count() > 40 {
        return Err("请填写 1~40 字的语音名称".into());
    }
    if let Some(item) = state
        .sounds
        .write()
        .map_err(|e| e.to_string())?
        .iter_mut()
        .find(|s| s.id == id)
    {
        item.name = display_name;
    } else {
        return Err("语音不存在".into());
    }
    state.save_sounds()
}

#[tauri::command]
pub fn cmd_delete_sound(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let removed = {
        let mut sounds = state.sounds.write().map_err(|e| e.to_string())?;
        let index = sounds.iter().position(|s| s.id == id);
        index.map(|i| sounds.remove(i))
    };
    let Some(sound) = removed else {
        return Err("语音不存在".into());
    };
    if let Some(file_name) = Path::new(&sound.file).file_name() {
        let path = state.paths.sounds_dir.join(file_name);
        let _ = std::fs::remove_file(path);
    }
    if let Ok(mut strategies) = state.strategies.write() {
        for item in strategies.iter_mut() {
            if item.sound_id.as_deref() == Some(id.as_str()) {
                item.sound_id = None;
                item.enabled = false;
                item.updated_at = store::now_iso();
            }
        }
    }
    state.save_sounds()?;
    state.save_strategies()?;
    Ok(())
}

#[tauri::command]
pub fn cmd_preview_sound(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let sound = state.sound(&id).ok_or_else(|| "语音不存在".to_string())?;
    let path = state.paths.sounds_dir.join(&sound.file);
    if !path.exists() {
        return Err("语音文件不存在".into());
    }
    state.player.preview(path);
    Ok(())
}

#[tauri::command]
pub fn cmd_stop_playback(state: State<'_, Arc<AppState>>) {
    state.player.stop();
}
