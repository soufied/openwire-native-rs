use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PADS_PER_GROUP: usize = 5;
pub const DEFAULT_GROUPS: [&str; 2] = ["Group 1", "Group 2"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PlayMode {
    #[default]
    OneShot,
    Loop,
    Toggle,
}

impl PlayMode {
    pub fn is_looping(self) -> bool {
        matches!(self, PlayMode::Loop | PlayMode::Toggle)
    }

    pub fn label(self) -> &'static str {
        match self {
            PlayMode::OneShot => "One-shot",
            PlayMode::Loop => "Loop",
            PlayMode::Toggle => "Toggle",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pad {
    pub id: usize,
    #[serde(default)]
    pub bank: usize,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub hotkey: Option<String>,
    #[serde(default = "full")]
    pub monitor_volume: f32,
    #[serde(default = "full")]
    pub stream_volume: f32,
    #[serde(default)]
    pub trim_start_ms: u32,
    #[serde(default)]
    pub trim_end_ms: u32,
    #[serde(default)]
    pub duration_ms: u32,
    #[serde(default)]
    pub bass_boost: bool,
    #[serde(default)]
    pub play_mode: PlayMode,
}

fn full() -> f32 {
    1.0
}

impl Pad {
    pub fn empty(bank: usize, id: usize) -> Self {
        Self {
            id,
            bank,
            name: String::new(),
            path: None,
            hotkey: None,
            monitor_volume: 1.0,
            stream_volume: 1.0,
            trim_start_ms: 0,
            trim_end_ms: 0,
            duration_ms: 0,
            bass_boost: false,
            play_mode: PlayMode::OneShot,
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.path.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bank {
    pub name: String,
    pub pads: Vec<Pad>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Soundboard {
    pub banks: Vec<Bank>,
}

impl Default for Soundboard {
    fn default() -> Self {
        Self {
            banks: DEFAULT_GROUPS
                .iter()
                .enumerate()
                .map(|(bank, name)| Bank {
                    name: name.to_string(),
                    pads: (0..PADS_PER_GROUP).map(|slot| Pad::empty(bank, slot)).collect(),
                })
                .collect(),
        }
    }
}

impl Soundboard {
    pub fn load_or_default(path: &Path) -> Self {
        let mut board = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| toml::from_str::<Soundboard>(&raw).ok())
            .unwrap_or_default();
        if board.banks.is_empty() {
            board = Soundboard::default();
        }
        board.renumber();
        board
    }

    pub fn renumber(&mut self) {
        for (bank_idx, bank) in self.banks.iter_mut().enumerate() {
            for (slot, pad) in bank.pads.iter_mut().enumerate() {
                pad.bank = bank_idx;
                pad.id = slot;
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let raw = toml::to_string_pretty(self).context("soundboard serialization")?;
        std::fs::write(path, raw).with_context(|| format!("cannot write {}", path.display()))
    }

    pub fn pad(&self, bank: usize, slot: usize) -> Option<&Pad> {
        self.banks.get(bank).and_then(|b| b.pads.get(slot))
    }

    pub fn pad_mut(&mut self, bank: usize, slot: usize) -> Option<&mut Pad> {
        self.banks.get_mut(bank).and_then(|b| b.pads.get_mut(slot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_board_has_two_groups_of_five_pads_without_keybinds() {
        let board = Soundboard::default();
        assert_eq!(board.banks.len(), 2);
        assert!(board.banks.iter().all(|b| b.pads.len() == PADS_PER_GROUP));
        assert!(board.pad(0, 0).unwrap().hotkey.is_none());
    }

    #[test]
    fn roundtrip_keeps_pad_identity() {
        let mut board = Soundboard::default();
        board.banks[1].pads[3].name = "horn".into();
        board.banks[1].pads[3].play_mode = PlayMode::Loop;
        let text = toml::to_string_pretty(&board).unwrap();
        let back: Soundboard = toml::from_str(&text).unwrap();
        let pad = back.pad(1, 3).unwrap();
        assert_eq!(pad.name, "horn");
        assert_eq!(pad.play_mode, PlayMode::Loop);
        assert_eq!(pad.id, 3);
    }
}
