use crate::config::paths;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BoardLayout {
    #[default]
    Grid,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Play,
    Pause,
    Next,
    Previous,
    Stop,
    StopAll,
}

impl Transport {
    pub const ALL: [Transport; 6] = [
        Transport::Play,
        Transport::Pause,
        Transport::Next,
        Transport::Previous,
        Transport::Stop,
        Transport::StopAll,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Transport::Play => "Play selected",
            Transport::Pause => "Pause / resume",
            Transport::Next => "Next sound",
            Transport::Previous => "Previous sound",
            Transport::Stop => "Stop last sound",
            Transport::StopAll => "Stop all sounds",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransportKeys {
    #[serde(default)]
    pub play: Option<String>,
    #[serde(default)]
    pub pause: Option<String>,
    #[serde(default)]
    pub next: Option<String>,
    #[serde(default)]
    pub previous: Option<String>,
    #[serde(default)]
    pub stop: Option<String>,
    #[serde(default)]
    pub stop_all: Option<String>,
}

impl TransportKeys {
    fn slot(&self, t: Transport) -> &Option<String> {
        match t {
            Transport::Play => &self.play,
            Transport::Pause => &self.pause,
            Transport::Next => &self.next,
            Transport::Previous => &self.previous,
            Transport::Stop => &self.stop,
            Transport::StopAll => &self.stop_all,
        }
    }

    fn slot_mut(&mut self, t: Transport) -> &mut Option<String> {
        match t {
            Transport::Play => &mut self.play,
            Transport::Pause => &mut self.pause,
            Transport::Next => &mut self.next,
            Transport::Previous => &mut self.previous,
            Transport::Stop => &mut self.stop,
            Transport::StopAll => &mut self.stop_all,
        }
    }

    pub fn get(&self, t: Transport) -> Option<&str> {
        self.slot(t).as_deref()
    }

    pub fn set(&mut self, t: Transport, label: Option<String>) {
        *self.slot_mut(t) = label;
    }

    pub fn unbind_label(&mut self, label: &str) {
        for t in Transport::ALL {
            if self.get(t) == Some(label) {
                self.set(t, None);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub data_dir: Option<PathBuf>,
    #[serde(default)]
    pub presets_dir: Option<PathBuf>,
    #[serde(default = "yes")]
    pub ducking_enabled: bool,
    #[serde(default = "no")]
    pub monitor_enabled: bool,
    #[serde(default = "yes")]
    pub close_to_tray: bool,
    #[serde(default)]
    pub input_device: Option<String>,
    #[serde(default)]
    pub soundboard_layout: BoardLayout,
    #[serde(default)]
    pub transport_keys: TransportKeys,
}

fn yes() -> bool {
    true
}

fn no() -> bool {
    false
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            data_dir: None,
            presets_dir: None,
            ducking_enabled: true,
            monitor_enabled: false,
            close_to_tray: true,
            input_device: None,
            soundboard_layout: BoardLayout::Grid,
            transport_keys: TransportKeys::default(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(paths::config_file())
            .ok()
            .and_then(|raw| toml::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = paths::config_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(self).context("settings serialization")?;
        std::fs::write(&path, raw).with_context(|| format!("cannot write {}", path.display()))
    }
}
