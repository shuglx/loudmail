use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::api::dialog::blocking::FileDialogBuilder;
use tauri::{Manager, State};

use crate::audio::{convert, mix, tts};
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
pub async fn cmd_pick_audio_file(app: tauri::AppHandle) -> Result<Option<PickedFile>, String> {
    if let Some(window) = app.get_window("main") {
        let _ = window.set_focus();
    }
    let Some(path) = FileDialogBuilder::new()
        .add_filter("音频文件", &["wav", "mp3", "ogg"])
        .pick_file()
    else {
        return Ok(None);
    };
    let meta = std::fs::metadata(&path).map_err(|e| format!("读取文件失败: {e}"))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "音频".into());
    Ok(Some(PickedFile {
        path: path.to_string_lossy().to_string(),
        name,
        size: meta.len(),
    }))
}

#[tauri::command]
pub async fn cmd_import_sound(
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

fn temp_wav_path(state: &AppState, prefix: &str) -> PathBuf {
    state
        .paths
        .recordings_dir
        .join(format!("{prefix}_{}.wav", uuid::Uuid::new_v4().simple()))
}

/// 校验路径位于 recordings 临时目录内
fn ensure_temp_path(state: &AppState, raw: &str) -> Result<PathBuf, String> {
    let recordings = state
        .paths
        .recordings_dir
        .canonicalize()
        .map_err(|e| format!("录音临时目录不存在: {e}"))?;
    let canonical = PathBuf::from(raw)
        .canonicalize()
        .map_err(|e| format!("临时音频不存在: {e}"))?;
    if !canonical.starts_with(&recordings) {
        return Err("临时音频路径不合法".into());
    }
    Ok(canonical)
}

#[tauri::command]
pub async fn cmd_tts_generate(
    state: State<'_, Arc<AppState>>,
    text: String,
) -> Result<RecordingResult, String> {
    let content = text.trim().to_string();
    if content.is_empty() {
        return Err("请输入要合成的文本".into());
    }
    if content.chars().count() > 50 {
        return Err("文本不超过 50 字".into());
    }
    let config = state
        .config
        .read()
        .map(|c| c.tts.clone())
        .unwrap_or_default();
    if !config.enabled {
        return Err("TTS 未启用，请先在设置中开启".into());
    }
    let (samples, rate) = tts::synthesize(&config.model_dir, &content, config.speed)?;
    let path = temp_wav_path(&state, "tts");
    let duration_ms = mix::write_wav_at(&path, &samples, rate)?;
    if duration_ms > mix::MAX_TOTAL_MS {
        let _ = std::fs::remove_file(&path);
        return Err("生成的语音超过 120 秒，请缩短文本".into());
    }
    logfile::info(format!("TTS 生成完成: {} ({} ms)", path.display(), duration_ms));
    Ok(RecordingResult {
        path: path.to_string_lossy().to_string(),
        duration_ms,
    })
}

#[tauri::command]
pub async fn cmd_compose_audio(
    state: State<'_, Arc<AppState>>,
    voice_path: String,
    ringtone_id: Option<String>,
    loops: u8,
) -> Result<RecordingResult, String> {
    let voice_file = ensure_temp_path(&state, &voice_path)?;
    let voice = mix::read_wav(&voice_file)?;

    let ringtone = match ringtone_id.as_deref() {
        Some(id) if !id.is_empty() => {
            let sound = state
                .sound(id)
                .ok_or_else(|| "所选铃声不存在".to_string())?;
            if sound.source != "builtin" {
                return Err("仅支持内置铃声作为拼接素材".into());
            }
            let path = state.paths.sounds_dir.join(&sound.file);
            Some(mix::read_wav(&path)?)
        }
        _ => None,
    };

    let (samples, duration_ms) = mix::compose(ringtone.as_ref(), loops, &voice)?;
    let path = temp_wav_path(&state, "mix");
    mix::write_wav(&path, &samples)?;
    logfile::info(format!("合成语音完成: {} ({} ms)", path.display(), duration_ms));
    Ok(RecordingResult {
        path: path.to_string_lossy().to_string(),
        duration_ms,
    })
}

#[tauri::command]
pub async fn cmd_save_composed_sound(
    state: State<'_, Arc<AppState>>,
    temp_path: String,
    name: String,
    source: String,
) -> Result<SoundItem, String> {
    let display_name = name.trim().to_string();
    if display_name.is_empty() || display_name.chars().count() > 40 {
        return Err("请填写 1~40 字的语音名称".into());
    }
    let source = match source.as_str() {
        "record" => "record",
        "tts" => "tts",
        _ => return Err("语音来源不合法".into()),
    };
    let canonical = ensure_temp_path(&state, &temp_path)?;
    let wav = mix::read_wav(&canonical)?;
    let duration_ms = (wav.samples.len() as u64 * 1000) / wav.rate.max(1) as u64;

    let id = store::new_id("snd");
    let target = state.paths.sounds_dir.join(format!("{id}.wav"));
    if std::fs::rename(&canonical, &target).is_err() {
        std::fs::copy(&canonical, &target).map_err(|e| format!("保存语音失败: {e}"))?;
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
        source: source.into(),
        created_at: store::now_iso(),
    };
    state
        .sounds
        .write()
        .map_err(|e| e.to_string())?
        .push(sound.clone());
    if let Err(e) = state.save_sounds() {
        let _ = std::fs::remove_file(&target);
        return Err(e);
    }
    logfile::info(format!("保存合成语音: {}", target.display()));
    Ok(sound)
}

#[tauri::command]
pub fn cmd_preview_temp(state: State<'_, Arc<AppState>>, path: String) -> Result<(), String> {
    let canonical = ensure_temp_path(&state, &path)?;
    state.player.preview(canonical);
    Ok(())
}
