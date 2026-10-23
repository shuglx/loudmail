use std::io::Cursor;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const MAX_INPUT_BYTES: usize = 20 * 1024 * 1024;
const MAX_DURATION_MS: u64 = 120_000;

/// 将任意受支持的音频解码为单声道浮点采样。
pub fn decode_to_mono(input: &[u8]) -> Result<(Vec<f32>, u32), String> {
    if input.is_empty() {
        return Err("音频文件为空".into());
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err("音频文件超过 20MB".into());
    }

    let mss = MediaSourceStream::new(Box::new(Cursor::new(input.to_vec())), Default::default());
    let hint = Hint::new();
    let format_opts = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let metadata_opts = MetadataOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| format!("无法识别音频格式: {e}"))?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| "音频中没有可解码的轨道".to_string())?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("创建解码器失败: {e}"))?;

    let mut mono: Vec<f32> = Vec::new();
    let mut sample_rate = codec_params.sample_rate.unwrap_or(44_100);

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(format!("读取音频失败: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("解码音频失败: {e}")),
        };

        let spec = *decoded.spec();
        sample_rate = spec.rate;
        let channels = spec.channels.count().max(1);
        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        for frame in buf.samples().chunks(channels) {
            let sum: f32 = frame.iter().copied().sum();
            mono.push(sum / channels as f32);
        }
    }

    if mono.is_empty() {
        return Err("音频没有可用的采样数据".into());
    }

    let duration_ms = (mono.len() as u64 * 1000) / sample_rate.max(1) as u64;
    if duration_ms > MAX_DURATION_MS {
        return Err("音频超过 120 秒".into());
    }

    Ok((mono, sample_rate))
}

/// 将任意受支持的音频解码为单声道 16-bit PCM WAV。
/// 返回 (wav bytes, duration_ms)。
pub fn decode_bytes_to_wav(input: &[u8]) -> Result<(Vec<u8>, u64), String> {
    let (mono, sample_rate) = decode_to_mono(input)?;
    let duration_ms = (mono.len() as u64 * 1000) / sample_rate.max(1) as u64;
    let wav_spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut writer =
            hound::WavWriter::new(&mut cursor, wav_spec).map_err(|e| format!("写入 WAV 失败: {e}"))?;
        for sample in &mono {
            let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer
                .write_sample(value)
                .map_err(|e| format!("写入 WAV 失败: {e}"))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("完成 WAV 写入失败: {e}"))?;
    }

    Ok((cursor.into_inner(), duration_ms))
}

pub fn convert_file(input: &Path, output: &Path) -> Result<u64, String> {
    let bytes = std::fs::read(input).map_err(|e| format!("读取音频失败: {e}"))?;
    let (wav, duration_ms) = decode_bytes_to_wav(&bytes)?;
    std::fs::write(output, wav).map_err(|e| format!("保存 WAV 失败: {e}"))?;
    Ok(duration_ms)
}
