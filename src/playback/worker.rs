use super::{report, Sampler};
use crate::board::Pad;
use crate::config::settings::Transport;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

pub enum Action {
    Play(usize, usize),
    Stop(usize, usize),
    TogglePause(usize, usize),
    Transport(Transport),
    Select(usize, usize),
    Seek(usize, usize, f32),
    StopAll,
    Import {
        bank: usize,
        slot: usize,
        path: PathBuf,
        reply: Sender<Result<Pad, String>>,
    },
    Waveform {
        bank: usize,
        slot: usize,
        points: usize,
        reply: Sender<Result<Vec<f32>, String>>,
    },
}

pub fn run_worker(sampler: Arc<Sampler>, rx: Receiver<Action>) {
    let spawned = std::thread::Builder::new()
        .name("openwire-actions".into())
        .spawn(move || {
            while let Ok(action) = rx.recv() {
                match action {
                    Action::Play(bank, slot) => report(sampler.play(bank, slot)),
                    Action::Stop(bank, slot) => sampler.stop_pad(bank, slot),
                    Action::TogglePause(bank, slot) => sampler.toggle_pause(bank, slot),
                    Action::Seek(bank, slot, progress) => sampler.seek(bank, slot, progress),
                    Action::Transport(action) => sampler.transport(action),
                    Action::Select(bank, slot) => sampler.select(bank, slot),
                    Action::StopAll => sampler.stop_all(),
                    Action::Import {
                        bank,
                        slot,
                        path,
                        reply,
                    } => {
                        let result = sampler.import(bank, slot, path).map_err(|e| format!("{e:#}"));
                        let _ = reply.send(result);
                    }
                    Action::Waveform {
                        bank,
                        slot,
                        points,
                        reply,
                    } => {
                        let result = sampler
                            .waveform(bank, slot, points)
                            .map_err(|e| format!("{e:#}"));
                        let _ = reply.send(result);
                    }
                }
            }
        });
    if let Err(err) = spawned {
        eprintln!("[openwire][sampler] cannot start worker: {err}");
    }
}
