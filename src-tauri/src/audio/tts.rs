use std::path::{Path, PathBuf};

use edge_tts_rust::{EdgeTtsClient, SpeakOptions};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsVitsModelConfig,
};

use crate::logfile;

/// 在线 Edge 语音使用的音色与格式
const EDGE_VOICE_FEMALE: &str = "zh-CN-XiaoxiaoNeural";
const EDGE_RATE: &str = "+10%";
const EDGE_FORMAT: &str = "audio-24khz-48kbitrate-mono-mp3";
/// 边缘节点偶发 reset，需要重试
const EDGE_RETRIES: usize = 6;

/// 在线合成：走微软 Edge Read Aloud 接口，返回 (采样, 采样率)
pub async fn synthesize_edge(text: &str, voice: &str) -> Result<(Vec<f32>, u32), String> {
    let voice = if voice.trim().is_empty() {
        EDGE_VOICE_FEMALE
    } else {
        voice.trim()
    };
    let client = EdgeTtsClient::new().map_err(|e| format!("初始化在线语音失败: {e}"))?;
    let mut last_error = String::new();
    for attempt in 1..=EDGE_RETRIES {
        let options = SpeakOptions {
            voice: voice.into(),
            rate: EDGE_RATE.into(),
            output_format: EDGE_FORMAT.into(),
            ..Default::default()
        };
        match client.synthesize(text.to_string(), options).await {
            Ok(result) => return crate::audio::convert::decode_to_mono(&result.audio),
            Err(e) => {
                last_error = format!("在线语音合成失败: {e}");
                logfile::warn(format!(
                    "在线语音第 {attempt}/{EDGE_RETRIES} 次失败: {e}"
                ));
            }
        }
        if attempt < EDGE_RETRIES {
            std::thread::sleep(std::time::Duration::from_millis(1200 * attempt as u64));
        }
    }
    Err(last_error)
}

/// 外置 TTS 模型的组成：.onnx 与 tokens.txt 必需，
/// espeak-ng-data（piper 系）与 lexicon.txt（MeloTTS 等）按模型二选一。
pub struct TtsModel {
    pub onnx: PathBuf,
    pub tokens: PathBuf,
    pub lexicon: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    /// 目录里可选的中文文本规范化规则（MeloTTS 提供 number.fst / date.fst）
    pub rule_fsts: Option<String>,
}

/// 校验模型目录：取目录中的 .onnx，并要求 tokens.txt 与至少一个文本前端资源
pub fn resolve_model(model_dir: &str) -> Result<TtsModel, String> {
    let dir = Path::new(model_dir.trim());
    if model_dir.trim().is_empty() {
        return Err("未选择 TTS 模型目录".into());
    }
    if !dir.is_dir() {
        return Err("TTS 模型目录不存在".into());
    }
    // 取体积最大的 .onnx：目录里可能混有 int8 版本或 Git LFS 指针文件（几百字节）
    let onnx = std::fs::read_dir(dir)
        .map_err(|e| format!("读取模型目录失败: {e}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|candidate| {
            candidate
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("onnx"))
                .unwrap_or(false)
        })
        .max_by_key(|candidate| std::fs::metadata(candidate).map(|m| m.len()).unwrap_or(0))
        .ok_or_else(|| "模型目录中未找到 .onnx 模型文件".to_string())?;

    let tokens = dir.join("tokens.txt");
    if !tokens.is_file() {
        return Err("模型目录缺少 tokens.txt".into());
    }
    let lexicon = dir.join("lexicon.txt");
    let lexicon = lexicon.is_file().then_some(lexicon);
    let data_dir = dir.join("espeak-ng-data");
    let data_dir = data_dir.is_dir().then_some(data_dir);
    if lexicon.is_none() && data_dir.is_none() {
        return Err("模型目录缺少 espeak-ng-data 或 lexicon.txt".into());
    }
    let rule_fsts: Vec<String> = ["number.fst", "date.fst"]
        .iter()
        .map(|name| dir.join(name))
        .filter(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    Ok(TtsModel {
        onnx,
        tokens,
        lexicon,
        data_dir,
        rule_fsts: (!rule_fsts.is_empty()).then(|| rule_fsts.join(",")),
    })
}

/// 合成文本，返回 (采样, 采样率)
pub fn synthesize(model_dir: &str, text: &str, speed: f32) -> Result<(Vec<f32>, u32), String> {
    let model = resolve_model(model_dir)?;
    let config = OfflineTtsConfig {
        model: OfflineTtsModelConfig {
            vits: OfflineTtsVitsModelConfig {
                model: Some(model.onnx.to_string_lossy().into_owned()),
                tokens: Some(model.tokens.to_string_lossy().into_owned()),
                lexicon: model
                    .lexicon
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                data_dir: model
                    .data_dir
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                ..Default::default()
            },
            num_threads: 2,
            ..Default::default()
        },
        rule_fsts: model.rule_fsts,
        ..Default::default()
    };
    let tts = OfflineTts::create(&config).ok_or_else(|| "加载 TTS 模型失败".to_string())?;
    let sample_rate = tts.sample_rate().max(1) as u32;

    let clean = preprocess(text);
    let mut speech: Vec<f32> = Vec::new();
    for (is_acronym, seg) in segment(&clean) {
        if is_acronym {
            // 逐字母朗读：略放慢并留出明显间隔，避免字母之间连读
            for ch in seg.chars() {
                speech.extend(trim(
                    &synth(&tts, &letter_zh(ch), speed * LETTER_SPEED_RATIO),
                    sample_rate,
                ));
                speech.extend(silence(sample_rate, LETTER_GAP_SEC));
            }
            speech.extend(silence(sample_rate, 0.12));
        } else if !seg.trim().is_empty() {
            speech.extend(synth(&tts, seg.trim(), speed));
            speech.extend(silence(sample_rate, 0.08));
        }
    }
    if speech.iter().all(|s| *s == 0.0) {
        return Err("TTS 未生成有效音频".into());
    }
    Ok((speech, sample_rate))
}

const LETTER_ZH: &[(char, &str)] = &[
    ('A', "诶"), ('B', "必"), ('C', "西"), ('D', "迪"), ('E', "伊"), ('F', "艾弗"),
    ('G', "吉"), ('H', "艾尺"), ('I', "爱"), ('J', "杰"), ('K', "开"), ('L', "艾勒"),
    ('M', "艾姆"), ('N', "恩"), ('O', "欧"), ('P', "批"), ('Q', "扣"), ('R', "啊"),
    ('S', "艾斯"), ('T', "提"), ('U', "优"), ('V', "维"), ('W', "达不溜"), ('X', "艾克斯"),
    ('Y', "歪"), ('Z', "贼"),
];

/// 逐字母朗读时的语速系数（越小越慢、越清晰）
const LETTER_SPEED_RATIO: f32 = 0.85;
/// 相邻字母之间的静音时长
const LETTER_GAP_SEC: f32 = 0.20;

fn letter_zh(c: char) -> String {
    LETTER_ZH
        .iter()
        .find(|(key, _)| *key == c.to_ascii_uppercase())
        .map(|(_, value)| value.to_string())
        .unwrap_or_else(|| c.to_string())
}

/// 把英文串整体拆成逐字母片段，其余保持原文
fn segment(text: &str) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut last = 0usize;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0usize;

    while i < chars.len() {
        let (start, c) = chars[i];
        if c.is_ascii_alphabetic() {
            let mut j = i;
            while j < chars.len() && chars[j].1.is_ascii_alphabetic() {
                j += 1;
            }
            let end = if j < chars.len() { chars[j].0 } else { text.len() };
            if start > last {
                out.push((false, text[last..start].to_string()));
            }
            out.push((true, text[start..end].to_string()));
            last = end;
            i = j;
        } else {
            i += 1;
        }
    }
    if last < text.len() {
        out.push((false, text[last..].to_string()));
    }
    out
}

/// 百分号、温度、符号等中文化
fn preprocess(text: &str) -> String {
    let replaced = text.replace('℃', "摄氏度").replace('&', "和").replace('@', "艾特");
    let chars: Vec<char> = replaced.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' {
            let mut k = out.len();
            while k > 0 && (out.as_bytes()[k - 1].is_ascii_digit() || out.as_bytes()[k - 1] == b'.') {
                k -= 1;
            }
            let number = out[k..].to_string();
            out.truncate(k);
            out.push_str("百分之");
            out.push_str(&number);
        } else {
            out.push(chars[i]);
        }
        i += 1;
    }
    out
}

fn synth(tts: &OfflineTts, text: &str, speed: f32) -> Vec<f32> {
    let config = GenerationConfig {
        speed,
        sid: 0,
        ..Default::default()
    };
    tts.generate_with_config(text, &config, None::<fn(&[f32], f32) -> bool>)
        .map(|audio| audio.samples().to_vec())
        .unwrap_or_default()
}

fn trim(samples: &[f32], sample_rate: u32) -> Vec<f32> {
    let peak = samples.iter().fold(0.0f32, |max, value| max.max(value.abs()));
    if peak <= 0.0 {
        return samples.to_vec();
    }
    let threshold = peak * 0.02;
    let mut start = 0usize;
    let mut end = samples.len();
    while start < end && samples[start].abs() < threshold {
        start += 1;
    }
    while end > start && samples[end - 1].abs() < threshold {
        end -= 1;
    }
    let pad = (sample_rate as f32 * 0.012) as usize;
    let start = start.saturating_sub(pad);
    let end = (end + pad).min(samples.len());
    samples[start..end].to_vec()
}

fn silence(sample_rate: u32, seconds: f32) -> Vec<f32> {
    vec![0.0; (sample_rate as f32 * seconds) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_every_english_run_for_letter_by_letter() {
        let segments = segment("收到一条SOC系统的报警，Windows 正常");
        assert!(segments
            .iter()
            .any(|(spell, value)| *spell && value == "SOC"));
        assert!(segments
            .iter()
            .any(|(spell, value)| *spell && value == "Windows"));
    }

    #[test]
    fn preprocess_percent() {
        assert_eq!(preprocess("温度 95%"), "温度 百分之95");
    }
}
