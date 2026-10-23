use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

use crate::logfile;

enum PlayerCommand {
    Play { path: PathBuf, count: u8 },
    Preview { path: PathBuf },
    Stop,
}

#[derive(Clone)]
pub struct PlayerHandle {
    tx: Sender<PlayerCommand>,
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

    pub fn stop(&self) {
        let _ = self.tx.send(PlayerCommand::Stop);
    }
}

pub fn start() -> PlayerHandle {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || run(rx));
    PlayerHandle { tx }
}

fn run(rx: Receiver<PlayerCommand>) {
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

    while let Ok(cmd) = rx.recv() {
        match cmd {
            PlayerCommand::Play { path, count } => {
                append_to_sink(&mut sink, &handle, &path, count);
            }
            PlayerCommand::Preview { path } => {
                sink.stop();
                match Sink::try_new(&handle) {
                    Ok(next) => {
                        sink = next;
                        append_to_sink(&mut sink, &handle, &path, 1);
                    }
                    Err(e) => logfile::error(format!("重建播放通道失败: {e}")),
                }
            }
            PlayerCommand::Stop => {
                sink.stop();
            }
        }
    }
}

fn append_to_sink(sink: &mut Sink, handle: &OutputStreamHandle, path: &PathBuf, count: u8) {
    let mut appended = 0u8;
    for _ in 0..count {
        match File::open(path) {
            Ok(file) => match Decoder::new(BufReader::new(file)) {
                Ok(decoder) => {
                    sink.append(decoder);
                    appended += 1;
                }
                Err(e) => logfile::error(format!("解码音频失败 {}: {e}", path.display())),
            },
            Err(e) => logfile::error(format!("打开音频失败 {}: {e}", path.display())),
        }
    }
    if appended > 0 {
        sink.play();
    } else {
        // 队列为空时保持可用，后续命令仍可继续使用
        let _ = handle;
    }
}
