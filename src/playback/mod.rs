pub mod timing;
pub mod worker;

pub use timing::PlaybackView;
pub use worker::{run_worker, Action};
use timing::Playback;

use crate::board::{now_secs, Bank, Pad, Soundboard, PADS_PER_GROUP};
use crate::audio::decode::decode_file;
use crate::audio::dsp::{Biquad, SAMPLE_RATE};
use crate::audio::mixer::{MixerControl, PlayRequest};
use crate::config::settings::Transport;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

pub fn pad_id(bank: usize, slot: usize) -> usize {
    bank.saturating_mul(1_000_000).saturating_add(slot)
}

pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "ogg", "oga", "m4a", "mp4", "aac", "aif", "aiff"];

const MAX_SCAN_DEPTH: usize = 8;

pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| AUDIO_EXTENSIONS.iter().any(|known| known.eq_ignore_ascii_case(ext)))
}

pub fn collect_audio(inputs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for input in inputs {
        if input.is_dir() {
            scan_dir(input, &mut out, 0);
        } else if is_audio_file(input) {
            out.push(input.clone());
        }
    }
    out
}

fn scan_dir(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            scan_dir(&entry, out, depth + 1);
        } else if is_audio_file(&entry) {
            out.push(entry);
        }
    }
}

pub fn sound_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "sound".into())
}

#[derive(Debug, Default, Clone)]
pub struct ImportReport {
    pub added: usize,
    pub failed: Vec<String>,
}

fn poisoned<T>(_: T) -> anyhow::Error {
    anyhow!("internal lock poisoned")
}

pub struct Sampler {
    board: RwLock<Soundboard>,
    cache: Mutex<HashMap<PathBuf, Arc<Vec<f32>>>>,
    control: MixerControl,
    board_path: RwLock<PathBuf>,
    playing: Mutex<HashMap<(usize, usize), Playback>>,
    master_monitor: AtomicU32,
    multi_play: AtomicBool,
    current: Mutex<Option<(usize, usize)>>,
    last_played: Mutex<Option<(usize, usize)>>,
    revision: AtomicU64,
}

impl Sampler {
    pub fn new(board_path: PathBuf, control: MixerControl) -> Self {
        let board = Soundboard::load_or_default(&board_path);
        Self {
            board: RwLock::new(board),
            cache: Mutex::new(HashMap::new()),
            control,
            board_path: RwLock::new(board_path),
            playing: Mutex::new(HashMap::new()),
            master_monitor: AtomicU32::new(1.0_f32.to_bits()),
            multi_play: AtomicBool::new(false),
            current: Mutex::new(None),
            last_played: Mutex::new(None),
            revision: AtomicU64::new(0),
        }
    }

    pub fn relocate(&self, board_path: PathBuf) {
        let board = Soundboard::load_or_default(&board_path);
        if let Ok(mut current) = self.board.write() {
            *current = board;
        }
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
        if let Ok(mut path) = self.board_path.write() {
            *path = board_path;
        }
    }

    pub fn snapshot(&self) -> Soundboard {
        self.board.read().map(|b| b.clone()).unwrap_or_default()
    }

    pub fn add_group(&self, name: String) -> Result<usize> {
        let index = {
            let mut board = self.board.write().map_err(poisoned)?;
            let index = board.banks.len();
            board.banks.push(Bank {
                name,
                folder: None,
                pads: (0..PADS_PER_GROUP).map(|slot| Pad::empty(index, slot)).collect(),
            });
            index
        };
        self.persist()?;
        Ok(index)
    }

    pub fn delete_group(&self, bank: usize) -> Result<()> {
        {
            let mut board = self.board.write().map_err(poisoned)?;
            if board.banks.len() <= 1 {
                return Err(anyhow!("keep at least one group"));
            }
            if bank >= board.banks.len() {
                return Err(anyhow!("group {bank} does not exist"));
            }
            board.banks.remove(bank);
            board.renumber();
        }
        self.stop_all();
        if let Ok(mut current) = self.current.lock() {
            *current = None;
        }
        if let Ok(mut last) = self.last_played.lock() {
            *last = None;
        }
        self.persist()
    }

    pub fn add_category(&self, name: String) -> Result<usize> {
        let index = {
            let mut board = self.board.write().map_err(poisoned)?;
            let index = board.banks.len();
            board.banks.push(Bank {
                name,
                folder: None,
                pads: Vec::new(),
            });
            index
        };
        self.persist()?;
        Ok(index)
    }

    pub fn set_group_folder(&self, bank: usize, folder: Option<PathBuf>) -> Result<()> {
        {
            let mut board = self.board.write().map_err(poisoned)?;
            let group = board
                .banks
                .get_mut(bank)
                .ok_or_else(|| anyhow!("group {bank} does not exist"))?;
            group.folder = folder;
        }
        self.persist()
    }

    pub fn import_many(&self, bank: usize, paths: Vec<PathBuf>, done: &AtomicUsize) -> Result<ImportReport> {
        if self.board.read().map_err(poisoned)?.banks.get(bank).is_none() {
            return Err(anyhow!("group {bank} does not exist"));
        }
        let mut report = ImportReport::default();
        let mut batch = Vec::with_capacity(paths.len());
        for path in paths {
            match decode_file(&path, SAMPLE_RATE) {
                Ok(samples) => {
                    let duration_ms = (samples.len() as u64 * 1000 / SAMPLE_RATE as u64) as u32;
                    let name = sound_name(&path);
                    batch.push(Pad {
                        name,
                        path: Some(path),
                        duration_ms,
                        added_at: now_secs(),
                        ..Pad::empty(bank, 0)
                    });
                }
                Err(err) => report.failed.push(format!("{}: {err:#}", path.display())),
            }
            done.fetch_add(1, Ordering::Relaxed);
        }
        if batch.is_empty() {
            return Ok(report);
        }
        report.added = batch.len();
        {
            let mut board = self.board.write().map_err(poisoned)?;
            let group = board
                .banks
                .get_mut(bank)
                .ok_or_else(|| anyhow!("group {bank} does not exist"))?;
            group.pads.extend(batch);
            board.renumber();
        }
        self.persist()?;
        Ok(report)
    }

    pub fn remove_pad(&self, bank: usize, slot: usize) -> Result<()> {
        {
            let mut board = self.board.write().map_err(poisoned)?;
            let group = board
                .banks
                .get_mut(bank)
                .ok_or_else(|| anyhow!("group {bank} does not exist"))?;
            if slot >= group.pads.len() {
                return Err(anyhow!("pad {bank}/{slot} does not exist"));
            }
            group.pads.remove(slot);
            board.renumber();
        }
        self.stop_bank(bank);
        self.persist()
    }

    fn stop_bank(&self, bank: usize) {
        let stopped: Vec<(usize, usize)> = match self.playing.lock() {
            Ok(mut map) => {
                let keys: Vec<(usize, usize)> = map.keys().filter(|(b, _)| *b == bank).copied().collect();
                for key in &keys {
                    map.remove(key);
                }
                keys
            }
            Err(_) => return,
        };
        for (b, s) in stopped {
            self.control.stop_pad(pad_id(b, s));
        }
        if let Ok(mut current) = self.current.lock() {
            if matches!(*current, Some((b, _)) if b == bank) {
                *current = None;
            }
        }
        if let Ok(mut last) = self.last_played.lock() {
            if matches!(*last, Some((b, _)) if b == bank) {
                *last = None;
            }
        }
    }

    pub fn rename_bank(&self, bank: usize, name: String) -> Result<()> {
        {
            let mut board = self.board.write().map_err(poisoned)?;
            let group = board
                .banks
                .get_mut(bank)
                .ok_or_else(|| anyhow!("group {bank}"))?;
            group.name = name;
        }
        self.persist()
    }

    pub fn update_pad(&self, bank: usize, pad: Pad) -> Result<()> {
        {
            let mut board = self.board.write().map_err(poisoned)?;
            let slot = board
                .pad_mut(bank, pad.id)
                .ok_or_else(|| anyhow!("pad {bank}/{}", pad.id))?;
            *slot = pad;
        }
        self.persist()
    }

    pub fn add_pad(&self, bank: usize) -> Result<Pad> {
        let pad = {
            let mut board = self.board.write().map_err(poisoned)?;
            let b = board
                .banks
                .get_mut(bank)
                .ok_or_else(|| anyhow!("bank {bank}"))?;
            let pad = Pad::empty(bank, b.pads.len());
            b.pads.push(pad.clone());
            pad
        };
        self.persist()?;
        Ok(pad)
    }

    pub fn clear_pad(&self, bank: usize, slot: usize) -> Result<()> {
        {
            let mut board = self.board.write().map_err(poisoned)?;
            let pad = board
                .pad_mut(bank, slot)
                .ok_or_else(|| anyhow!("pad {bank}/{slot}"))?;
            pad.path = None;
            pad.name.clear();
            pad.duration_ms = 0;
            pad.trim_start_ms = 0;
            pad.trim_end_ms = 0;
        }
        self.persist()
    }

    pub fn import(&self, bank: usize, slot: usize, path: PathBuf) -> Result<Pad> {
        let samples = decode_file(&path, SAMPLE_RATE)?;
        let name = sound_name(&path);
        let duration_ms = (samples.len() as u64 * 1000 / SAMPLE_RATE as u64) as u32;
        let result = {
            let mut board = self.board.write().map_err(poisoned)?;
            let pad = board
                .pad_mut(bank, slot)
                .ok_or_else(|| anyhow!("pad {bank}/{slot}"))?;
            pad.name = name;
            pad.path = Some(path);
            pad.duration_ms = duration_ms;
            pad.added_at = now_secs();
            pad.trim_start_ms = 0;
            pad.trim_end_ms = 0;
            pad.clone()
        };
        self.persist()?;
        Ok(result)
    }

    pub fn waveform(&self, bank: usize, slot: usize, points: usize) -> Result<Vec<f32>> {
        let pad = self.pad_at(bank, slot)?;
        let samples = self.pad_samples(&pad)?;
        let points = points.max(1);
        if samples.is_empty() {
            return Ok(vec![0.0; points]);
        }
        let bucket = (samples.len() as f64 / points as f64).max(1.0);
        Ok((0..points)
            .map(|i| {
                let start = (i as f64 * bucket) as usize;
                let end = (((i + 1) as f64 * bucket) as usize)
                    .min(samples.len())
                    .max(start + 1)
                    .min(samples.len());
                samples[start..end]
                    .iter()
                    .fold(0.0f32, |acc, s| acc.max(s.abs()))
                    .min(1.0)
            })
            .collect())
    }

    pub fn playback(&self, bank: usize, slot: usize) -> Option<PlaybackView> {
        self.playing
            .lock()
            .ok()?
            .get(&(bank, slot))
            .filter(|p| !p.finished())
            .map(|p| p.view())
    }

    pub fn set_multi_play(&self, enabled: bool) {
        self.multi_play.store(enabled, Ordering::Relaxed);
    }

    pub fn multi_play(&self) -> bool {
        self.multi_play.load(Ordering::Relaxed)
    }

    fn stop_others(&self, keep: (usize, usize)) {
        let stopped: Vec<(usize, usize)> = match self.playing.lock() {
            Ok(mut map) => {
                let keys: Vec<(usize, usize)> = map.keys().filter(|k| **k != keep).copied().collect();
                for key in &keys {
                    map.remove(key);
                }
                keys
            }
            Err(_) => return,
        };
        for (b, s) in stopped {
            self.control.stop_pad(pad_id(b, s));
        }
    }

    pub fn playback_views(&self, bank: usize) -> HashMap<usize, PlaybackView> {
        self.playing
            .lock()
            .map(|map| {
                map.iter()
                    .filter(|(key, p)| key.0 == bank && !p.finished())
                    .map(|(key, p)| (key.1, p.view()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn play(&self, bank: usize, slot: usize) -> Result<()> {
        self.start(bank, slot, true)
    }

    fn start(&self, bank: usize, slot: usize, counted: bool) -> Result<()> {
        let pad = self.pad_at(bank, slot)?;
        if !pad.is_loaded() {
            return Ok(());
        }
        if !self.multi_play() {
            self.stop_others((bank, slot));
        }
        let samples = self.pad_samples(&pad)?;
        let start = (pad.trim_start_ms as usize * SAMPLE_RATE as usize / 1000).min(samples.len());
        let end = if pad.trim_end_ms > 0 {
            (pad.trim_end_ms as usize * SAMPLE_RATE as usize / 1000).clamp(start, samples.len())
        } else {
            samples.len()
        };
        let slice: Arc<Vec<f32>> = if pad.bass_boost {
            Arc::new(bass_boost(&samples[start..end]))
        } else if start == 0 && end == samples.len() {
            samples
        } else {
            Arc::new(samples[start..end].to_vec())
        };
        let length = Duration::from_micros((end - start) as u64 * 1_000_000 / SAMPLE_RATE as u64);
        let looping = pad.play_mode.is_looping();
        if let Ok(mut map) = self.playing.lock() {
            map.insert((bank, slot), Playback::new(length, looping));
        }
        if let Ok(mut last) = self.last_played.lock() {
            *last = Some((bank, slot));
        }
        self.control.play(PlayRequest {
            pad_id: pad_id(bank, slot),
            samples: slice,
            monitor_gain: pad.monitor_volume.clamp(0.0, 1.0),
            stream_gain: pad.stream_volume.clamp(0.0, 2.0),
            looping,
        });
        if counted {
            self.count_play(bank, slot);
        }
        Ok(())
    }

    fn count_play(&self, bank: usize, slot: usize) {
        let changed = match self.board.write() {
            Ok(mut board) => match board.pad_mut(bank, slot) {
                Some(pad) => {
                    pad.play_count = pad.play_count.saturating_add(1);
                    true
                }
                None => false,
            },
            Err(_) => false,
        };
        if changed {
            report(self.persist());
        }
    }

    pub fn set_pad_favorite(&self, bank: usize, slot: usize, favorite: bool) -> Result<()> {
        {
            let mut board = self.board.write().map_err(poisoned)?;
            let pad = board
                .pad_mut(bank, slot)
                .ok_or_else(|| anyhow!("pad {bank}/{slot} does not exist"))?;
            pad.favorite = favorite;
        }
        self.persist()
    }

    pub fn stop_pad(&self, bank: usize, slot: usize) {
        if let Ok(mut map) = self.playing.lock() {
            map.remove(&(bank, slot));
        }
        self.control.stop_pad(pad_id(bank, slot));
    }

    pub fn stop_all(&self) {
        if let Ok(mut map) = self.playing.lock() {
            map.clear();
        }
        self.control.stop_all();
    }

    pub fn toggle_pause(&self, bank: usize, slot: usize) {
        let mut now_paused = None;
        if let Ok(mut map) = self.playing.lock() {
            if let Some(playback) = map.get_mut(&(bank, slot)) {
                if playback.paused() {
                    playback.resume();
                    now_paused = Some(false);
                } else {
                    playback.pause();
                    now_paused = Some(true);
                }
            }
        }
        if let Some(paused) = now_paused {
            self.control.set_paused(pad_id(bank, slot), paused);
        }
    }

    pub fn seek(&self, bank: usize, slot: usize, progress: f32) {
        let previous_paused = self.playback(bank, slot).map(|v| v.paused);
        if self.start(bank, slot, false).is_err() {
            return;
        }
        let paused = previous_paused == Some(true);
        let id = pad_id(bank, slot);
        if let Ok(mut map) = self.playing.lock() {
            if let Some(playback) = map.get_mut(&(bank, slot)) {
                playback.seek(progress);
                if paused {
                    playback.pause();
                }
            }
        }
        self.control.seek(id, progress);
        if paused {
            self.control.set_paused(id, true);
        }
    }

    pub fn select(&self, bank: usize, slot: usize) {
        if let Ok(mut current) = self.current.lock() {
            *current = Some((bank, slot));
        }
    }

    pub fn transport(&self, action: Transport) {
        let current = self.current.lock().ok().and_then(|c| *c);
        let last = self.last_played.lock().ok().and_then(|l| *l);
        match action {
            Transport::Play => {
                if let Some((bank, slot)) = current {
                    report(self.play(bank, slot));
                }
            }
            Transport::Pause => {
                if let Some((bank, slot)) = last {
                    self.toggle_pause(bank, slot);
                }
            }
            Transport::Stop => {
                if let Some((bank, slot)) = last {
                    self.stop_pad(bank, slot);
                }
            }
            Transport::StopAll => self.stop_all(),
            Transport::Next => self.step(current.or(last), 1),
            Transport::Previous => self.step(current.or(last), -1),
        }
    }

    fn step(&self, from: Option<(usize, usize)>, delta: i32) {
        let (bank, slot) = from.unwrap_or((0, 0));
        let loaded: Vec<usize> = match self.board.read() {
            Ok(board) => board
                .banks
                .get(bank)
                .map(|group| group.pads.iter().filter(|p| p.is_loaded()).map(|p| p.id).collect())
                .unwrap_or_default(),
            Err(_) => return,
        };
        let next = if delta > 0 {
            loaded.iter().copied().find(|&id| id > slot).or_else(|| loaded.first().copied())
        } else {
            loaded.iter().copied().rev().find(|&id| id < slot).or_else(|| loaded.last().copied())
        };
        if let Some(id) = next {
            self.select(bank, id);
            report(self.play(bank, id));
        }
    }

    pub fn set_master_monitor(&self, gain: f32) {
        let gain = gain.clamp(0.0, 1.0);
        self.master_monitor.store(gain.to_bits(), Ordering::Relaxed);
        self.control.set_master_monitor(gain);
    }

    pub fn master_monitor(&self) -> f32 {
        f32::from_bits(self.master_monitor.load(Ordering::Relaxed))
    }

    fn pad_at(&self, bank: usize, slot: usize) -> Result<Pad> {
        self.board
            .read()
            .map_err(poisoned)?
            .pad(bank, slot)
            .cloned()
            .ok_or_else(|| anyhow!("pad {bank}/{slot} does not exist"))
    }

    fn pad_samples(&self, pad: &Pad) -> Result<Arc<Vec<f32>>> {
        let path = pad
            .path
            .clone()
            .ok_or_else(|| anyhow!("pad '{}' has no audio attached", pad.name))?;
        self.load_samples(&path)
    }

    fn load_samples(&self, path: &Path) -> Result<Arc<Vec<f32>>> {
        if let Some(hit) = self.cache.lock().map_err(poisoned)?.get(path) {
            return Ok(hit.clone());
        }
        let decoded = Arc::new(decode_file(path, SAMPLE_RATE)?);
        self.cache
            .lock()
            .map_err(poisoned)?
            .insert(path.to_path_buf(), decoded.clone());
        Ok(decoded)
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }

    fn persist(&self) -> Result<()> {
        self.revision.fetch_add(1, Ordering::Relaxed);
        let path = self.board_path.read().map_err(poisoned)?.clone();
        let board = self.board.read().map_err(poisoned)?;
        board.save(&path)
    }
}

fn bass_boost(samples: &[f32]) -> Vec<f32> {
    let mut shelf = Biquad::low_shelf(SAMPLE_RATE as f32, 200.0, 6.0);
    samples.iter().map(|&s| shelf.process(s)).collect()
}

pub(crate) fn report(result: Result<()>) {
    if let Err(err) = result {
        eprintln!("[openwire][sampler] {err:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_audio_extensions_case_insensitively() {
        assert!(is_audio_file(Path::new("horn.MP3")));
        assert!(is_audio_file(Path::new("/x/y/clip.flac")));
        assert!(!is_audio_file(Path::new("notes.txt")));
        assert!(!is_audio_file(Path::new("no_extension")));
    }

    #[test]
    fn sound_name_drops_extension_and_folders() {
        assert_eq!(sound_name(Path::new("/music/Bingo Bango.wav")), "Bingo Bango");
    }
}
