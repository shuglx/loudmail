use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use crate::logfile;
use crate::models::RecordingResult;

struct ActiveRecording {
    stop_tx: Sender<()>,
    result_rx: Receiver<Result<RecordingResult, String>>,
}

#[derive(Clone)]
pub struct RecorderHandle {
    inner: Arc<Mutex<Option<ActiveRecording>>>,
    dir: PathBuf,
}

impl RecorderHandle {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            dir,
        }
    }

    pub fn start(&self) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Err("正在录音".into());
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (result_tx, result_rx) = mpsc::channel::<Result<RecordingResult, String>>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let dir = self.dir.clone();

        thread::spawn(move || record_thread(dir, stop_rx, result_tx, ready_tx));

        match ready_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(())) => {
                *guard = Some(ActiveRecording { stop_tx, result_rx });
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err("启动录音超时，请检查系统麦克风权限".into()),
        }
    }

    pub fn stop(&self) -> Result<RecordingResult, String> {
        let active = {
            let mut guard = self.inner.lock().map_err(|e| e.to_string())?;
            guard.take()
        };
        let Some(active) = active else {
            return Err("当前没有录音".into());
        };
        let _ = active.stop_tx.send(());
        match active.result_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(result) => result,
            Err(_) => Err("结束录音超时".into()),
        }
    }
}

fn record_thread(
    dir: PathBuf,
    stop_rx: Receiver<()>,
    result_tx: Sender<Result<RecordingResult, String>>,
    ready_tx: Sender<Result<(), String>>,
) {
    let result = (|| -> Result<RecordingResult, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "没有可用的麦克风设备".to_string())?;
        let config = device
            .default_input_config()
            .map_err(|e| format!("读取麦克风配置失败: {e}"))?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let sample_format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();

        let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let err_fn = |e| logfile::error(format!("录音流错误: {e}"));

        let stream = match sample_format {
            SampleFormat::F32 => {
                let buf = samples.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _| {
                        if let Ok(mut b) = buf.lock() {
                            b.extend_from_slice(data);
                        }
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let buf = samples.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _| {
                        if let Ok(mut b) = buf.lock() {
                            b.extend(data.iter().map(|&s| s as f32 / 32768.0));
                        }
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let buf = samples.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _| {
                        if let Ok(mut b) = buf.lock() {
                            b.extend(data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0));
                        }
                    },
                    err_fn,
                    None,
                )
            }
            other => return Err(format!("不支持的麦克风采样格式: {other:?}")),
        }
        .map_err(|e| format!("打开麦克风失败: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("启动麦克风失败: {e}"))?;
        let _ = ready_tx.send(Ok(()));

        let started = Instant::now();
        let _ = stop_rx.recv_timeout(Duration::from_secs(60));
        drop(stream);

        let interleaved = samples.lock().map(|b| b.clone()).unwrap_or_default();
        if interleaved.is_empty() {
            return Err("没有采集到音频，请检查麦克风权限".into());
        }

        let mono: Vec<f32> = interleaved
            .chunks(channels.max(1))
            .map(|frame| frame.iter().copied().sum::<f32>() / channels.max(1) as f32)
            .collect();
        let duration_ms = (mono.len() as u64 * 1000) / sample_rate.max(1) as u64;
        let path = dir.join(format!("rec_{}.wav", uuid::Uuid::new_v4().simple()));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer =
            hound::WavWriter::create(&path, spec).map_err(|e| format!("保存录音失败: {e}"))?;
        for sample in mono {
            let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer
                .write_sample(value)
                .map_err(|e| format!("写入录音失败: {e}"))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("完成录音写入失败: {e}"))?;

        logfile::info(format!(
            "录音完成: {} ({} ms, 实际时长 {} ms)",
            path.display(),
            duration_ms,
            started.elapsed().as_millis()
        ));
        Ok(RecordingResult {
            path: path.to_string_lossy().to_string(),
            duration_ms,
        })
    })();

    if let Err(e) = &result {
        let _ = ready_tx.send(Err(e.clone()));
    }
    let _ = result_tx.send(result);
}
