use crate::audio::dsp::DspSnapshot;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const ORDINARY_VOICE: &str = "Ordinary Voice";

pub const BUILTIN: [&str; 6] = [
    "Ordinary Voice",
    "Deep Voice",
    "High Voice",
    "Radio Comms",
    "Telephone",
    "Monster",
];

pub fn is_builtin(name: &str) -> bool {
    BUILTIN.contains(&name)
}

pub fn builtin(name: &str) -> Option<DspSnapshot> {
    let base = DspSnapshot::default();
    let snap = match name {
        "Ordinary Voice" => base,
        "Deep Voice" => DspSnapshot {
            pitch_enabled: true,
            pitch_semitones: -4.0,
            formant_shift: 0.9,
            eq_low_db: 4.0,
            eq_high_db: -2.0,
            ..base
        },
        "High Voice" => DspSnapshot {
            pitch_enabled: true,
            pitch_semitones: 4.0,
            formant_shift: 1.1,
            eq_low_db: -3.0,
            eq_high_db: 2.0,
            ..base
        },
        "Radio Comms" => DspSnapshot {
            radio_enabled: true,
            radio_bp_low_hz: 400.0,
            radio_bp_high_hz: 3000.0,
            radio_drive: 0.4,
            radio_noise_db: -50.0,
            eq_low_db: -6.0,
            eq_mid_gain_db: 3.0,
            ..base
        },
        "Telephone" => DspSnapshot {
            radio_enabled: true,
            radio_bp_low_hz: 300.0,
            radio_bp_high_hz: 3400.0,
            radio_drive: 0.1,
            radio_noise_db: -60.0,
            eq_low_db: -8.0,
            eq_high_db: -4.0,
            ..base
        },
        "Monster" => DspSnapshot {
            pitch_enabled: true,
            pitch_semitones: -7.0,
            formant_shift: 0.8,
            eq_low_db: 6.0,
            radio_enabled: true,
            radio_drive: 0.2,
            radio_noise_db: -60.0,
            ..base
        },
        _ => return None,
    };
    Some(snap)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default = "one_u32")]
    pub version: u32,
}

fn one_u32() -> u32 {
    1
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            name: "Untitled Voice".into(),
            description: String::new(),
            author: "OpenWire".into(),
            version: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pitch {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub semitones: f32,
    #[serde(default = "one_f32")]
    pub formant_shift: f32,
}

fn one_f32() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Equalizer {
    #[serde(default)]
    pub low_shelf_gain_db: f32,
    #[serde(default = "mid_freq")]
    pub mid_freq_hz: f32,
    #[serde(default)]
    pub mid_gain_db: f32,
    #[serde(default = "mid_q")]
    pub mid_q: f32,
    #[serde(default)]
    pub high_shelf_gain_db: f32,
}

fn mid_freq() -> f32 {
    1200.0
}

fn mid_q() -> f32 {
    1.2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioSection {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "bp_low")]
    pub bandpass_low_hz: f32,
    #[serde(default = "bp_high")]
    pub bandpass_high_hz: f32,
    #[serde(default)]
    pub drive_saturation: f32,
    #[serde(default = "noise_floor")]
    pub noise_floor_db: f32,
}

fn bp_low() -> f32 {
    380.0
}

fn bp_high() -> f32 {
    3600.0
}

fn noise_floor() -> f32 {
    -96.0
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DspSection {
    #[serde(default)]
    pub pitch: Option<Pitch>,
    #[serde(default)]
    pub equalizer: Option<Equalizer>,
    #[serde(default)]
    pub radio_fx: Option<RadioSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Routing {
    #[serde(default = "duck_db")]
    pub ducking_attenuation_db: f32,
    #[serde(default = "release_ms")]
    pub sidechain_release_ms: f32,
}

fn duck_db() -> f32 {
    -12.0
}

fn release_ms() -> f32 {
    120.0
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VoicePreset {
    #[serde(default)]
    pub meta: Meta,
    #[serde(default)]
    pub dsp: DspSection,
    #[serde(default)]
    pub routing: Option<Routing>,
}

impl VoicePreset {
    pub fn from_snapshot(name: &str, s: &DspSnapshot) -> Self {
        Self {
            meta: Meta {
                name: name.to_string(),
                ..Meta::default()
            },
            dsp: DspSection {
                pitch: Some(Pitch {
                    enabled: s.pitch_enabled,
                    semitones: s.pitch_semitones,
                    formant_shift: s.formant_shift,
                }),
                equalizer: Some(Equalizer {
                    low_shelf_gain_db: s.eq_low_db,
                    mid_freq_hz: s.eq_mid_freq,
                    mid_gain_db: s.eq_mid_gain_db,
                    mid_q: s.eq_mid_q,
                    high_shelf_gain_db: s.eq_high_db,
                }),
                radio_fx: Some(RadioSection {
                    enabled: s.radio_enabled,
                    bandpass_low_hz: s.radio_bp_low_hz,
                    bandpass_high_hz: s.radio_bp_high_hz,
                    drive_saturation: s.radio_drive,
                    noise_floor_db: s.radio_noise_db,
                }),
            },
            routing: Some(Routing {
                ducking_attenuation_db: s.duck_atten_db,
                sidechain_release_ms: s.duck_release_ms,
            }),
        }
    }

    pub fn apply_to(&self, s: &mut DspSnapshot) {
        if let Some(p) = &self.dsp.pitch {
            s.pitch_enabled = p.enabled;
            s.pitch_semitones = p.semitones;
            s.formant_shift = p.formant_shift.max(0.01);
        }
        if let Some(eq) = &self.dsp.equalizer {
            s.eq_low_db = eq.low_shelf_gain_db;
            s.eq_mid_freq = eq.mid_freq_hz;
            s.eq_mid_gain_db = eq.mid_gain_db;
            s.eq_mid_q = eq.mid_q;
            s.eq_high_db = eq.high_shelf_gain_db;
        }
        if let Some(r) = &self.dsp.radio_fx {
            s.radio_enabled = r.enabled;
            s.radio_bp_low_hz = r.bandpass_low_hz;
            s.radio_bp_high_hz = r.bandpass_high_hz;
            s.radio_drive = r.drive_saturation;
            s.radio_noise_db = r.noise_floor_db;
        }
        if let Some(rt) = &self.routing {
            s.duck_atten_db = rt.ducking_attenuation_db;
            s.duck_release_ms = rt.sidechain_release_ms;
        }
    }
}

pub struct PresetStore {
    dir: PathBuf,
}

impl PresetStore {
    pub fn new(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)
            .with_context(|| format!("cannot create preset directory {}", dir.display()))?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    pub fn list(&self) -> Result<Vec<String>> {
        let mut names: Vec<String> = fs::read_dir(&self.dir)
            .with_context(|| format!("cannot read {}", self.dir.display()))?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("toml"))
            .filter_map(|path| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect();
        names.sort();
        Ok(names)
    }

    pub fn load(&self, name: &str) -> Result<VoicePreset> {
        let path = self.path_for(name)?;
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("cannot read preset {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("invalid preset format in {}", path.display()))
    }

    pub fn save(&self, name: &str, preset: &VoicePreset) -> Result<PathBuf> {
        let path = self.path_for(name)?;
        let pretty = toml::to_string_pretty(preset).context("preset serialization")?;
        fs::write(&path, pretty).with_context(|| format!("cannot write preset {}", path.display()))?;
        Ok(path)
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        let path = self.path_for(name)?;
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("cannot delete preset {}", path.display()))?;
        }
        Ok(())
    }

    pub fn path_for(&self, name: &str) -> Result<PathBuf> {
        let clean: String = name
            .chars()
            .map(|c| match c {
                '/' | '\\' | '\0' => '_',
                other => other,
            })
            .collect();
        let clean = clean.trim();
        let clean = clean.strip_suffix(".toml").unwrap_or(clean);
        if clean.is_empty() || clean == "." || clean == ".." {
            return Err(anyhow!("preset name must not be empty or a traversal"));
        }
        Ok(self.dir.join(format!("{clean}.toml")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_roundtrips_through_toml() {
        let snap = DspSnapshot {
            pitch_enabled: true,
            pitch_semitones: -3.5,
            formant_shift: 0.85,
            ..DspSnapshot::default()
        };
        let preset = VoicePreset::from_snapshot("Test", &snap);
        let text = toml::to_string_pretty(&preset).unwrap();
        let back: VoicePreset = toml::from_str(&text).unwrap();
        let mut applied = DspSnapshot::default();
        back.apply_to(&mut applied);
        assert!(applied.pitch_enabled);
        assert!((applied.pitch_semitones + 3.5).abs() < f32::EPSILON);
        assert_eq!(back.meta.name, "Test");
    }

    #[test]
    fn names_are_sanitised() {
        let dir = std::env::temp_dir().join("openwire_test_presets");
        let store = PresetStore::new(&dir).unwrap();
        let path = store.path_for("../evil/path").unwrap();
        assert_eq!(path.file_name().unwrap(), ".._evil_path.toml");
        assert!(path.starts_with(&dir));
        assert!(store.path_for("").is_err());
        assert!(store.path_for("..").is_err());
    }
}
