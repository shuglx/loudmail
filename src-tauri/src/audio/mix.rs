use std::path::Path;

/// 语音库统一使用 44.1kHz / 16-bit / 单声道 WAV
pub const TARGET_RATE: u32 = 44_100;
/// 合成语音总时长上限
pub const MAX_TOTAL_MS: u64 = 120_000;
/// 每段音频之间的静音间隔（铃声每段之间、铃声与人声之间）
pub const GAP_MS: u64 = 500;

pub struct Pcm {
    pub samples: Vec<f32>,
    pub rate: u32,
}

pub fn read_wav(path: &Path) -> Result<Pcm, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("读取音频失败: {e}"))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let bits = spec.bits_per_sample.max(1);
    let mut mono: Vec<f32> = Vec::new();
    let mut frame = vec![0f32; channels];

    match spec.sample_format {
        hound::SampleFormat::Int => {
            let max = (1i64 << (bits - 1)) as f32;
            for (index, sample) in reader.samples::<i32>().enumerate() {
                let value = sample.map_err(|e| format!("读取音频失败: {e}"))? as f32 / max;
                frame[index % channels] = value;
                if index % channels == channels - 1 {
                    mono.push(frame.iter().copied().sum::<f32>() / channels as f32);
                }
            }
        }
        hound::SampleFormat::Float => {
            for (index, sample) in reader.samples::<f32>().enumerate() {
                let value = sample.map_err(|e| format!("读取音频失败: {e}"))?;
                frame[index % channels] = value;
                if index % channels == channels - 1 {
                    mono.push(frame.iter().copied().sum::<f32>() / channels as f32);
                }
            }
        }
    }

    if mono.is_empty() {
        return Err("音频没有可用的采样数据".into());
    }
    Ok(Pcm {
        samples: mono,
        rate: spec.sample_rate.max(1),
    })
}

/// 线性插值重采样
pub fn resample(pcm: &Pcm, target: u32) -> Vec<f32> {
    if pcm.rate == target || pcm.samples.is_empty() {
        return pcm.samples.clone();
    }
    let ratio = target as f64 / pcm.rate as f64;
    let out_len = (pcm.samples.len() as f64 * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 / ratio;
        let index = pos.floor() as usize;
        let frac = (pos - index as f64) as f32;
        let a = pcm.samples[index.min(pcm.samples.len() - 1)];
        let b = pcm.samples[(index + 1).min(pcm.samples.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    out
}

/// 铃声在第 N 段后和人声之前都会插入 GAP_MS 静音，避免相邻片段黏连
/// 例如：铃声 → 静音 → 铃声 → 静音 → 人声
pub fn compose(ringtone: Option<&Pcm>, loops: u8, voice: &Pcm) -> Result<(Vec<f32>, u64), String> {
    let voice_samples = resample(voice, TARGET_RATE);
    let gap = vec![0f32; (TARGET_RATE as u64 * GAP_MS / 1000) as usize];
    let mut out: Vec<f32> = Vec::new();

    if let Some(ringtone) = ringtone {
        let ring = resample(ringtone, TARGET_RATE);
        for _ in 0..loops.clamp(1, 5) {
            out.extend_from_slice(&ring);
            out.extend_from_slice(&gap);
        }
    }
    out.extend_from_slice(&voice_samples);

    let duration_ms = (out.len() as u64 * 1000) / TARGET_RATE as u64;
    if duration_ms > MAX_TOTAL_MS {
        return Err("合成语音超过 120 秒，请减少铃声循环次数".into());
    }
    Ok((out, duration_ms))
}

pub fn write_wav_at(path: &Path, samples: &[f32], rate: u32) -> Result<u64, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| format!("写入 WAV 失败: {e}"))?;
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer
            .write_sample(value)
            .map_err(|e| format!("写入 WAV 失败: {e}"))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("完成 WAV 写入失败: {e}"))?;
    Ok((samples.len() as u64 * 1000) / rate.max(1) as u64)
}

pub fn write_wav(path: &Path, samples: &[f32]) -> Result<u64, String> {
    write_wav_at(path, samples, TARGET_RATE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcm(rate: u32, count: usize) -> Pcm {
        Pcm {
            samples: vec![0.5; count],
            rate,
        }
    }

    #[test]
    fn resample_changes_length() {
        let source = pcm(22_050, 22_050);
        let out = resample(&source, TARGET_RATE);
        assert!((out.len() as i64 - 44_100).abs() <= 1);
    }

    #[test]
    fn compose_adds_gap_and_loop() {
        let ring = pcm(TARGET_RATE, 1000);
        let voice = pcm(TARGET_RATE, 2000);
        let (out, ms) = compose(Some(&ring), 2, &voice).unwrap();
        let gap = (TARGET_RATE as u64 * GAP_MS / 1000) as usize;
        // 每段铃声后都插入间隔，最后一段与人声之间也有一段
        assert_eq!(out.len(), 1000 * 2 + gap * 2 + 2000);
        assert!(ms > 0);
    }
}
