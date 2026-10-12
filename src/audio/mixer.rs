use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub const MAX_VOICES: usize = 32;
const COMMAND_CAPACITY: usize = 256;
const GRAVEYARD_CAPACITY: usize = 256;

pub struct PlayRequest {
    pub pad_id: usize,
    pub samples: Arc<Vec<f32>>,
    pub monitor_gain: f32,
    pub stream_gain: f32,
    pub looping: bool,
}

enum Command {
    Play(PlayRequest),
    StopPad(usize),
    StopAll,
    SetPaused(usize, bool),
    Seek(usize, f32),
    SetMasterMonitor(f32),
}

struct Voice {
    pad_id: usize,
    samples: Arc<Vec<f32>>,
    pos: usize,
    monitor_gain: f32,
    stream_gain: f32,
    looping: bool,
    paused: bool,
}

#[derive(Clone)]
pub struct MixerControl {
    tx: Arc<Mutex<HeapProd<Command>>>,
    active: Arc<AtomicUsize>,
}

impl MixerControl {
    pub fn play(&self, req: PlayRequest) {
        self.push(Command::Play(req));
    }

    pub fn stop_pad(&self, pad_id: usize) {
        self.push(Command::StopPad(pad_id));
    }

    pub fn stop_all(&self) {
        self.push(Command::StopAll);
    }

    pub fn set_paused(&self, pad_id: usize, paused: bool) {
        self.push(Command::SetPaused(pad_id, paused));
    }

    pub fn seek(&self, pad_id: usize, progress: f32) {
        self.push(Command::Seek(pad_id, progress));
    }

    pub fn set_master_monitor(&self, gain: f32) {
        self.push(Command::SetMasterMonitor(gain));
    }

    pub fn active_voices(&self) -> usize {
        self.active.load(Ordering::Relaxed)
    }

    fn push(&self, cmd: Command) {
        if let Ok(mut tx) = self.tx.lock() {
            let _ = tx.try_push(cmd);
        }
    }
}

pub struct MixerBus {
    rx: HeapCons<Command>,
    voices: Vec<Voice>,
    active: Arc<AtomicUsize>,
    graveyard: HeapProd<Arc<Vec<f32>>>,
    master_monitor: f32,
}

pub struct Reaper {
    rx: HeapCons<Arc<Vec<f32>>>,
}

impl Reaper {
    pub fn drain(&mut self) {
        while self.rx.try_pop().is_some() {}
    }
}

impl MixerBus {
    pub fn split() -> (MixerControl, MixerBus, Reaper) {
        let (cmd_tx, cmd_rx) = HeapRb::<Command>::new(COMMAND_CAPACITY).split();
        let (grave_tx, grave_rx) = HeapRb::<Arc<Vec<f32>>>::new(GRAVEYARD_CAPACITY).split();
        let active = Arc::new(AtomicUsize::new(0));
        let control = MixerControl {
            tx: Arc::new(Mutex::new(cmd_tx)),
            active: active.clone(),
        };
        let bus = MixerBus {
            rx: cmd_rx,
            voices: Vec::with_capacity(MAX_VOICES),
            active,
            graveyard: grave_tx,
            master_monitor: 1.0,
        };
        (control, bus, Reaper { rx: grave_rx })
    }

    fn retire_where(&mut self, mut remove: impl FnMut(&Voice) -> bool) {
        let mut i = 0;
        while i < self.voices.len() {
            if remove(&self.voices[i]) {
                let voice = self.voices.swap_remove(i);
                let _ = self.graveyard.try_push(voice.samples);
            } else {
                i += 1;
            }
        }
    }

    fn poll_commands(&mut self) {
        while let Some(cmd) = self.rx.try_pop() {
            match cmd {
                Command::Play(req) => {
                    self.retire_where(|v| v.pad_id == req.pad_id);
                    if self.voices.len() < MAX_VOICES {
                        self.voices.push(Voice {
                            pad_id: req.pad_id,
                            samples: req.samples,
                            pos: 0,
                            monitor_gain: req.monitor_gain,
                            stream_gain: req.stream_gain,
                            looping: req.looping,
                            paused: false,
                        });
                    }
                }
                Command::StopPad(id) => self.retire_where(|v| v.pad_id == id),
                Command::StopAll => self.retire_where(|_| true),
                Command::SetPaused(id, paused) => {
                    for voice in self.voices.iter_mut().filter(|v| v.pad_id == id) {
                        voice.paused = paused;
                    }
                }
                Command::Seek(id, progress) => {
                    let progress = progress.clamp(0.0, 1.0);
                    for voice in self.voices.iter_mut().filter(|v| v.pad_id == id) {
                        let len = voice.samples.len();
                        voice.pos = ((progress * len as f32) as usize).min(len);
                    }
                }
                Command::SetMasterMonitor(gain) => self.master_monitor = gain.clamp(0.0, 1.0),
            }
        }
    }

    pub fn render(&mut self, frames: usize, monitor: &mut [f32], stream: &mut [f32]) -> bool {
        self.poll_commands();
        monitor[..frames].fill(0.0);
        stream[..frames].fill(0.0);

        let master = self.master_monitor;
        let mut audible = false;
        for voice in self.voices.iter_mut() {
            if voice.paused {
                continue;
            }
            let len = voice.samples.len();
            if len == 0 {
                continue;
            }
            let monitor_gain = voice.monitor_gain * master;
            let mut i = 0;
            while i < frames {
                if voice.pos >= len {
                    if voice.looping {
                        voice.pos = 0;
                    } else {
                        break;
                    }
                }
                let n = (len - voice.pos).min(frames - i);
                audible = true;
                for j in 0..n {
                    let s = voice.samples[voice.pos + j];
                    monitor[i + j] += s * monitor_gain;
                    stream[i + j] += s * voice.stream_gain;
                }
                voice.pos += n;
                i += n;
            }
        }

        self.retire_where(|v| !v.looping && v.pos >= v.samples.len());
        self.active.store(self.voices.len(), Ordering::Relaxed);
        audible
    }
}
