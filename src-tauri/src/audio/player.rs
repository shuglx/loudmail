use std::collections::VecDeque;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rodio::source::Zero;
use rodio::{Decoder, OutputStream, Sink};
use tauri::{AppHandle, Manager};

use crate::logfile;
use crate::models::PlaybackStatus;

/// 待播队列上限（条），超出后新的命中会被丢弃并写日志
pub const MAX_QUEUE: usize = 50;
/// 每条完整语音之间的静音间隔
const GAP_MS: u64 = 500;

enum PlayerCommand {
    /// 加入播报队列（count = 同一条语音连续播放次数）
    Play { path: PathBuf, count: u8 },
    /// 试听：使用独立 Sink，不影响播报队列
    Preview { path: PathBuf },
    /// 只停止试听
    StopPreview,
    /// 停止播报并清空队列（托盘暂停使用）
    StopAll,
}

struct AlarmItem {
    path: PathBuf,
    count: u8,
}

#[derive(Clone)]
pub struct PlayerHandle {
    tx: Sender<PlayerCommand>,
    status: Arc<Mutex<PlaybackStatus>>,
}

impl PlayerHandle {
    pub fn play(&self, path: PathBuf, count: u8) {
        let _ = self.tx.send(PlayerCommand::Play {
            path,
            count: count.max(1),
        });
    }

    pub fn preview(&self, path: PathBuf) {
        let _ = self.tx.send(PlayerCommand::Preview { path });
    }

    pub fn stop_preview(&self) {
        let _ = self.tx.send(PlayerCommand::StopPreview);
    }

    pub fn stop_all(&self) {
        let _ = self.tx.send(PlayerCommand::StopAll);
    }

    pub fn status(&self) -> PlaybackStatus {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }
}

pub fn start(app: AppHandle) -> PlayerHandle {
    let (tx, rx) = mpsc::channel();
    let status = Arc::new(Mutex::new(PlaybackStatus::default()));
    let shared = status.clone();
    thread::spawn(move || run(rx, app, shared));
    PlayerHandle { tx, status }
}

fn run(rx: Receiver<PlayerCommand>, app: AppHandle, shared: Arc<Mutex<PlaybackStatus>>) {
    let (stream, handle) = match OutputStream::try_default() {
        Ok(v) => v,
        Err(e) => {
            logfile::error(format!("初始化音频输出失败: {e}"));
            while rx.recv().is_ok() {}
            return;
        }
    };
    let _stream = stream;

    let mut sink = match Sink::try_new(&handle) {
        Ok(s) => s,
        Err(e) => {
            logfile::error(format!("创建播放通道失败: {e}"));
            while rx.recv().is_ok() {}
            return;
        }
    };

    let mut queue: VecDeque<AlarmItem> = VecDeque::new();
    let mut playing = false;
    let mut previewing = false;
    // 试听走独立 Sink，与播报互不影响
    let mut preview_sink: Option<Sink> = None;

    loop {
        if preview_sink.as_ref().map(|s| s.empty()).unwrap_or(false) {
            if let Some(s) = preview_sink.take() {
                s.stop();
            }
            previewing = false;
        }

        if playing && sink.empty() {
            playing = false;
        }
        if !playing {
            if let Some(item) = queue.pop_front() {
                if append_to_sink(&sink, &item.path, item.count) {
                    playing = true;
                }
            }
        }

        publish(&app, &shared, playing, queue.len(), previewing);

        let idle = !playing && preview_sink.is_none();
        let timeout = if idle {
            Duration::from_millis(250)
        } else {
            Duration::from_millis(80)
        };

        match rx.recv_timeout(timeout) {
            Ok(PlayerCommand::Play { path, count }) => {
                if queue.len() >= MAX_QUEUE {
                    logfile::warn(format!(
                        "播报队列已满（上限 {MAX_QUEUE} 条），已丢弃: {}",
                        path.display()
                    ));
                } else {
                    queue.push_back(AlarmItem {
                        path,
                        count: count.max(1),
                    });
                }
            }
            Ok(PlayerCommand::Preview { path }) => {
                if let Some(old) = preview_sink.take() {
                    old.stop();
                }
                previewing = false;
                match Sink::try_new(&handle) {
                    Ok(s) => {
                        if append_to_sink(&s, &path, 1) {
                            preview_sink = Some(s);
                            previewing = true;
                        }
                    }
                    Err(e) => logfile::error(format!("创建试听通道失败: {e}")),
                }
            }
            Ok(PlayerCommand::StopPreview) => {
                if let Some(s) = preview_sink.take() {
                    s.stop();
                }
                previewing = false;
            }
            Ok(PlayerCommand::StopAll) => {
                sink.stop();
                queue.clear();
                playing = false;
                if let Some(s) = preview_sink.take() {
                    s.stop();
                }
                previewing = false;
                // rodio 的 stop 会清空队列，重建一个以便后续仍能播报
                match Sink::try_new(&handle) {
                    Ok(next) => sink = next,
                    Err(e) => logfile::error(format!("重建播放通道失败: {e}")),
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn publish(
    app: &AppHandle,
    shared: &Arc<Mutex<PlaybackStatus>>,
    playing: bool,
    pending: usize,
    previewing: bool,
) {
    let next = PlaybackStatus {
        playing,
        pending,
        previewing,
    };
    let changed = match shared.lock() {
        Ok(mut guard) => {
            if *guard == next {
                false
            } else {
                *guard = next.clone();
                true
            }
        }
        Err(_) => return,
    };
    if changed {
        let _ = app.emit_all("player://status", next);
    }
}

fn append_to_sink(sink: &Sink, path: &PathBuf, count: u8) -> bool {
    let mut appended = 0u8;
    for _ in 0..count {
        match File::open(path) {
            Ok(file) => match Decoder::new(BufReader::new(file)) {
                Ok(decoder) => {
                    sink.append(decoder);
                    // 每段完整语音后补一段静音，避免多条播报连在一起
                    push_gap(sink);
                    appended += 1;
                }
                Err(e) => logfile::error(format!("解码音频失败 {}: {e}", path.display())),
            },
            Err(e) => logfile::error(format!("打开音频失败 {}: {e}", path.display())),
        }
    }
    if appended > 0 {
        sink.play();
        true
    } else {
        false
    }
}

/// 追加一段 500ms 的静音，作为语音之间的间隔
fn push_gap(sink: &Sink) {
    let frames = (44_100u64 * GAP_MS / 1000) as usize;
    sink.append(Zero::<f32>::new_samples(2, 44_100, frames * 2));
}
