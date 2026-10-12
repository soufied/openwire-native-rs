use crate::presets;
use eframe::egui;
use crate::audio::dsp::DspSnapshot;
use crate::presets::{ORDINARY_VOICE, VoicePreset};
use super::app::App;
use super::widgets::{card, chain_strip, slider};

pub(crate) fn pitch_controls(ui: &mut egui::Ui, s: &mut DspSnapshot) -> bool {
    let mut c = ui.checkbox(&mut s.pitch_enabled, "Enable pitch shift").changed();
    c |= slider(ui, &mut s.pitch_semitones, -12.0..=12.0, "Semitones");
    c |= slider(ui, &mut s.formant_shift, 0.5..=2.0, "Formant");
    c
}

pub(crate) fn eq_controls(ui: &mut egui::Ui, s: &mut DspSnapshot) -> bool {
    let mut c = slider(ui, &mut s.eq_low_db, -24.0..=24.0, "Low, dB");
    c |= slider(ui, &mut s.eq_mid_freq, 60.0..=12_000.0, "Mid, Hz");
    c |= slider(ui, &mut s.eq_mid_gain_db, -24.0..=24.0, "Mid, dB");
    c |= slider(ui, &mut s.eq_mid_q, 0.2..=8.0, "Q factor");
    c |= slider(ui, &mut s.eq_high_db, -24.0..=24.0, "High, dB");
    c
}

pub(crate) fn radio_controls(ui: &mut egui::Ui, s: &mut DspSnapshot) -> bool {
    let mut c = ui.checkbox(&mut s.radio_enabled, "Enable radio effect").changed();
    c |= slider(ui, &mut s.radio_bp_low_hz, 80.0..=2000.0, "Band low, Hz");
    c |= slider(ui, &mut s.radio_bp_high_hz, 1500.0..=12_000.0, "Band high, Hz");
    c |= slider(ui, &mut s.radio_drive, 0.0..=1.0, "Drive");
    c |= slider(ui, &mut s.radio_noise_db, -96.0..=-20.0, "Noise, dB");
    c
}

pub(crate) fn gate_routing_controls(ui: &mut egui::Ui, s: &mut DspSnapshot) -> bool {
    let mut c = ui.checkbox(&mut s.noise_gate_enabled, "Enable noise gate").changed();
    c |= slider(ui, &mut s.noise_gate_db, -90.0..=-20.0, "Gate threshold, dB");
    c |= slider(ui, &mut s.duck_atten_db, -60.0..=0.0, "Ducking, dB");
    c |= slider(ui, &mut s.duck_release_ms, 20.0..=2000.0, "Release, ms");
    c
}

pub(crate) fn monitor_controls(ui: &mut egui::Ui, s: &mut DspSnapshot) -> bool {
    let mut c = ui.checkbox(&mut s.monitor_enabled, "Hear myself").changed();
    c |= slider(ui, &mut s.monitor_volume, 0.0..=1.0, "Monitor volume");
    c |= slider(ui, &mut s.stream_volume, 0.0..=2.0, "Virtual mic volume");
    c
}

impl App {
    pub(crate) fn preset_names(&self) -> Vec<String> {
        let mut names: Vec<String> = presets::BUILTIN.iter().map(|s| s.to_string()).collect();
        for name in self.presets.list().unwrap_or_default() {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }

    pub(crate) fn load_preset(&self, name: &str) -> anyhow::Result<VoicePreset> {
        match presets::builtin(name) {
            Some(snap) => Ok(VoicePreset::from_snapshot(name, &snap)),
            None => self.presets.load(name),
        }
    }

    pub(crate) fn apply_preset(&mut self) {
        let base = DspSnapshot {
            monitor_enabled: self.snap.monitor_enabled,
            monitor_volume: self.snap.monitor_volume,
            stream_volume: self.snap.stream_volume,
            ..DspSnapshot::default()
        };
        match self.load_preset(&self.preset_choice) {
            Ok(preset) => {
                let mut next = base;
                preset.apply_to(&mut next);
                self.snap = next;
                self.params.apply(&self.snap);
                self.status = format!("Preset: {}", self.preset_choice);
            }
            Err(err) => self.status = format!("Preset error: {err:#}"),
        }
    }

    pub(crate) fn save_preset(&mut self) {
        let name = self.preset_name.trim().to_string();
        if name.is_empty() {
            self.status = "Enter a preset name".into();
            return;
        }
        if presets::is_builtin(&name) {
            self.status = "That name is reserved for a built-in preset".into();
            return;
        }
        let preset = VoicePreset::from_snapshot(&name, &self.snap);
        match self.presets.save(&name, &preset) {
            Ok(_) => {
                self.status = format!("Preset saved: {name}");
                self.preset_choice = name;
                self.preset_name.clear();
            }
            Err(err) => self.status = format!("Save error: {err:#}"),
        }
    }

    pub(crate) fn delete_preset(&mut self) {
        if presets::is_builtin(&self.preset_choice) {
            self.status = "Built-in presets cannot be deleted".into();
            return;
        }
        match self.presets.delete(&self.preset_choice) {
            Ok(()) => {
                self.status = format!("Deleted: {}", self.preset_choice);
                self.preset_choice = ORDINARY_VOICE.to_string();
                self.apply_preset();
            }
            Err(err) => self.status = format!("Delete error: {err:#}"),
        }
    }

    pub(crate) fn cycle_preset(&mut self) {
        let names = self.preset_names();
        let current = names.iter().position(|n| *n == self.preset_choice).unwrap_or(0);
        self.preset_choice = names[(current + 1) % names.len()].clone();
        self.apply_preset();
    }

    pub(crate) fn disable_effects(&mut self) {
        self.preset_choice = ORDINARY_VOICE.to_string();
        self.apply_preset();
    }

    pub(crate) fn voice_view(&mut self, ui: &mut egui::Ui) {
        card(ui, "Presets", |ui| {
            let names = self.preset_names();
            ui.horizontal_wrapped(|ui| {
                for name in &names {
                    let active = self.preset_choice == *name;
                    if ui.add(egui::Button::new(name.as_str()).selected(active)).clicked() {
                        self.preset_choice = name.clone();
                        self.apply_preset();
                    }
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.preset_name)
                        .hint_text("New preset name")
                        .desired_width(200.0),
                );
                let can_save = !self.preset_name.trim().is_empty();
                if ui.add_enabled(can_save, egui::Button::new("Save current")).clicked() {
                    self.save_preset();
                }
                let deletable = !presets::is_builtin(&self.preset_choice);
                if ui.add_enabled(deletable, egui::Button::new("Delete selected")).clicked() {
                    self.delete_preset();
                }
            });
        });

        ui.add_space(8.0);
        card(ui, "Signal chain", |ui| chain_strip(ui, &self.snap));
        ui.add_space(8.0);
        let s = &mut self.snap;
        let mut changed = false;
        ui.columns(2, |cols| {
            changed |= card(&mut cols[0], "Pitch & formant", |ui| pitch_controls(ui, s));
            changed |= card(&mut cols[0], "Equalizer", |ui| eq_controls(ui, s));
            changed |= card(&mut cols[1], "Radio effect", |ui| radio_controls(ui, s));
            changed |= card(&mut cols[1], "Noise gate & routing", |ui| gate_routing_controls(ui, s));
        });
        ui.add_space(8.0);
        changed |= card(ui, "Monitoring", |ui| monitor_controls(ui, s));
        if changed {
            self.params.apply(&self.snap);
        }
    }
}
