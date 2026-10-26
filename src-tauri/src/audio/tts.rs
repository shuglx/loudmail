use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use base64::Engine as _;
use edge_tts_rust::{EdgeTtsClient, SpeakOptions};
use serde_json::{json, Value};
use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, WebSocket};

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

/// 内网 TTS（vLLM-Omni Speech API / Qwen3-TTS）的连接参数
pub struct IntranetOptions<'a> {
    pub url: &'a str,
    pub key: &'a str,
    pub voice: &'a str,
    pub instructions: &'a str,
    pub speed: f32,
}

const INTRANET_MODEL: &str = "Qwen3-TTS-12Hz-1.7B-CustomVoice";
const DEFAULT_SAMPLE_RATE: u32 = 24_000;

/// 建立到内网 TTS 的 WebSocket 连接（地址解析 + 可选 Bearer 认证 + 超时）
fn connect_intranet(url: &str, key: &str) -> Result<WebSocket<TcpStream>, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("未配置内网 TTS 服务地址".into());
    }
    if url.starts_with("wss://") {
        return Err("内网 TTS 暂不支持 wss://，请填 ws:// 地址".into());
    }

    let mut request = url
        .into_client_request()
        .map_err(|e| format!("内网 TTS 地址无效: {e}"))?;
    let key = key.trim();
    if !key.is_empty() {
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {key}")
                .parse()
                .map_err(|_| "内网 TTS 认证信息非法".to_string())?,
        );
    }

    let host = request.uri().host().unwrap_or("").to_string();
    if host.is_empty() {
        return Err("内网 TTS 地址缺少主机名".into());
    }
    let port = request.uri().port_u16().unwrap_or(80);
    let addr = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("内网 TTS 地址解析失败: {e}"))?
        .next()
        .ok_or_else(|| "内网 TTS 地址解析为空".to_string())?;
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map_err(|e| format!("连接内网 TTS 失败: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let (socket, _) = tungstenite::client(request, stream)
        .map_err(|e| format!("内网 TTS 握手失败: {e}"))?;
    Ok(socket)
}

/// 「检测」用：真的连一次内网 TTS，WS 握手成功即认为服务可达。
pub fn probe_intranet(url: &str, key: &str) -> Result<(), String> {
    let mut socket = connect_intranet(url, key)?;
    let _ = socket.close(None);
    Ok(())
}

/// 「检测」用：联网服务（微软 Edge）只做 TCP 可达性探测。
pub fn probe_edge() -> Result<(), String> {
    let addr = ("speech.platform.bing.com", 443)
        .to_socket_addrs()
        .map_err(|e| format!("解析在线语音地址失败: {e}"))?
        .next()
        .ok_or_else(|| "在线语音地址解析为空".to_string())?;
    TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map(|_| ())
        .map_err(|e| format!("连接在线语音服务失败: {e}"))
}

/// 内网合成：连 WebSocket，按 session.config → input.text → input.done 发三帧，
/// 收集二进制 PCM（加 audio.chunk 的 base64 帧），收到 session.done 结束。
pub fn synthesize_intranet(opt: &IntranetOptions<'_>, text: &str) -> Result<(Vec<f32>, u32), String> {
    let url = opt.url.trim();
    if url.is_empty() {
        return Err("未配置内网 TTS 服务地址".into());
    }
    if url.starts_with("wss://") {
        return Err("内网 TTS 暂不支持 wss://，请填 ws:// 地址".into());
    }

    let mut socket = connect_intranet(url, opt.key)?;

    let voice = if opt.voice.trim().is_empty() {
        "vivian"
    } else {
        opt.voice.trim()
    };
    let config = json!({
        "type": "session.config",
        "voice": voice,
        "language": "Chinese",
        "task_type": "CustomVoice",
        "response_format": "pcm",
        "stream_audio": true,
        "split_granularity": "sentence",
        "speed": opt.speed,
        "model": INTRANET_MODEL,
        "instructions": opt.instructions,
    });
    for frame in [
        config,
        json!({ "type": "input.text", "text": text }),
        json!({ "type": "input.done" }),
    ] {
        socket
            .send(Message::Text(frame.to_string()))
            .map_err(|e| format!("内网 TTS 发送失败: {e}"))?;
    }

    let mut pcm: Vec<u8> = Vec::new();
    let mut sample_rate = DEFAULT_SAMPLE_RATE;
    loop {
        match socket.read() {
            Ok(Message::Binary(data)) => pcm.extend_from_slice(&data),
            Ok(Message::Text(text)) => {
                let Ok(msg) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if let Some(rate) = msg.get("sample_rate").and_then(Value::as_u64) {
                    if rate > 0 {
                        sample_rate = rate as u32;
                    }
                }
                match msg.get("type").and_then(Value::as_str) {
                    Some("error") => {
                        let message = msg
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("未知错误");
                        return Err(format!("内网 TTS 返回错误: {message}"));
                    }
                    Some("audio.chunk") => {
                        if let Some(data) = msg.get("audio_b64").and_then(Value::as_str) {
                            let bytes = base64::engine::general_purpose::STANDARD
                                .decode(data)
                                .map_err(|e| format!("内网 TTS 音频解码失败: {e}"))?;
                            pcm.extend_from_slice(&bytes);
                        }
                    }
                    Some("session.done") => break,
                    _ => {}
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(e) => return Err(format!("内网 TTS 读取失败: {e}")),
        }
    }

    if pcm.is_empty() {
        return Err("内网 TTS 未返回音频数据（检查服务地址与音色）".into());
    }
    let samples: Vec<f32> = pcm
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32768.0)
        .collect();
    logfile::info(format!(
        "内网 TTS 合成完成: {} 采样点, {} Hz, voice={voice}",
        samples.len(),
        sample_rate
    ));
    Ok((samples, sample_rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_i16_le_converts_to_float() {
        let mut pcm = Vec::new();
        pcm.extend_from_slice(&0i16.to_le_bytes());
        pcm.extend_from_slice(&16384i16.to_le_bytes());
        pcm.extend_from_slice(&(-32768i16).to_le_bytes());
        let samples: Vec<f32> = pcm
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32768.0)
            .collect();
        assert_eq!(samples, vec![0.0, 0.5, -1.0]);
    }

    #[test]
    fn rejects_wss_and_empty_url() {
        let opt = IntranetOptions {
            url: "",
            key: "",
            voice: "vivian",
            instructions: "",
            speed: 1.0,
        };
        assert!(synthesize_intranet(&opt, "测试").is_err());
        let opt = IntranetOptions { url: "wss://x/y", ..opt };
        assert!(synthesize_intranet(&opt, "测试").is_err());
    }
}
