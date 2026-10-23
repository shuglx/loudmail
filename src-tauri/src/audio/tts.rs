use std::path::{Path, PathBuf};

use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsVitsModelConfig,
};

/// 外置 TTS 模型的三个必要组成部分
pub struct TtsModel {
    pub onnx: PathBuf,
    pub tokens: PathBuf,
    pub data_dir: PathBuf,
}

/// 校验模型目录：取第一个 .onnx，并要求 tokens.txt 与 espeak-ng-data
pub fn resolve_model(model_dir: &str) -> Result<TtsModel, String> {
    let dir = Path::new(model_dir.trim());
    if model_dir.trim().is_empty() {
        return Err("未配置 TTS 模型目录".into());
    }
    if !dir.is_dir() {
        return Err("TTS 模型目录不存在".into());
    }
    let onnx = std::fs::read_dir(dir)
        .map_err(|e| format!("读取模型目录失败: {e}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .map(|ext| ext.eq_ignore_ascii_case("onnx"))
                .unwrap_or(false)
        })
        .ok_or_else(|| "模型目录中未找到 .onnx 模型文件".to_string())?;

    let tokens = dir.join("tokens.txt");
    if !tokens.is_file() {
        return Err("模型目录缺少 tokens.txt".into());
    }
    let data_dir = dir.join("espeak-ng-data");
    if !data_dir.is_dir() {
        return Err("模型目录缺少 espeak-ng-data".into());
    }
    Ok(TtsModel {
        onnx,
        tokens,
        data_dir,
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
                data_dir: Some(model.data_dir.to_string_lossy().into_owned()),
                ..Default::default()
            },
            num_threads: 2,
            ..Default::default()
        },
        ..Default::default()
    };
    let tts = OfflineTts::create(&config).ok_or_else(|| "加载 TTS 模型失败".to_string())?;
    let sample_rate = tts.sample_rate().max(1) as u32;

    let clean = preprocess(text);
    let mut speech: Vec<f32> = Vec::new();
    for (is_acronym, seg) in segment(&clean) {
        if is_acronym {
            for ch in seg.chars() {
                speech.extend(trim(&synth(&tts, &letter_zh(ch), speed), sample_rate));
                speech.extend(silence(sample_rate, 0.10));
            }
            speech.extend(silence(sample_rate, 0.08));
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

const WORD_EXCEPTIONS: &[&str] = &[
    "WINDOWS", "LINUX", "ANDROID", "ORACLE", "PYTHON", "KAFKA", "REDIS", "NGINX", "MACOS",
    "DOCKER",
];

fn letter_zh(c: char) -> String {
    LETTER_ZH
        .iter()
        .find(|(key, _)| *key == c.to_ascii_uppercase())
        .map(|(_, value)| value.to_string())
        .unwrap_or_else(|| c.to_string())
}

fn is_acronym(word: &str) -> bool {
    let first_upper = word
        .chars()
        .next()
        .map(|c| c.is_ascii_uppercase())
        .unwrap_or(false);
    let uppers = word.chars().filter(|c| c.is_ascii_uppercase()).count();
    first_upper && uppers >= 2
}

/// 把缩写拆成独立片段，其余保持原文
fn segment(text: &str) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut last = 0usize;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0usize;

    while i < chars.len() {
        let (start, c) = chars[i];
        if c.is_ascii_alphabetic() {
            let mut j = i;
            while j < chars.len() && chars[j].1.is_ascii_alphanumeric() {
                j += 1;
            }
            let end = if j < chars.len() { chars[j].0 } else { text.len() };
            let word = &text[start..end];
            let prev_ok = start == 0
                || !text[..start]
                    .chars()
                    .last()
                    .map(|p| p.is_ascii_alphanumeric())
                    .unwrap_or(false);
            let next_ok = j >= chars.len()
                || !text[end..]
                    .chars()
                    .next()
                    .map(|n| n.is_ascii_alphanumeric())
                    .unwrap_or(false);

            if word.len() >= 2
                && word.len() <= 6
                && prev_ok
                && next_ok
                && is_acronym(word)
                && !WORD_EXCEPTIONS.contains(&word.to_uppercase().as_str())
            {
                if start > last {
                    out.push((false, text[last..start].to_string()));
                }
                out.push((true, word.to_string()));
                last = end;
            }
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
    fn splits_acronyms_and_keeps_words() {
        let segments = segment("收到一条SOC系统的报警，WINDOWS 正常");
        assert!(segments.iter().any(|(acronym, value)| *acronym && value == "SOC"));
        assert!(!segments
            .iter()
            .any(|(acronym, value)| *acronym && value == "WINDOWS"));
    }

    #[test]
    fn preprocess_percent() {
        assert_eq!(preprocess("温度 95%"), "温度 百分之95");
    }
}
