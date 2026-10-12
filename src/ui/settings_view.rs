use crate::config::paths;
use eframe::egui;
use eframe::egui::{RichText};
use crate::audio::engine::EngineCmd;
use crate::config::settings::Settings;
use crate::playback::Sampler;
use crate::presets::PresetStore;
use std::path::PathBuf;
use super::app::{App, PathAction};
use super::theme::MUTED;
use super::widgets::{card, display_path};

pub(crate) fn relocate_all(settings: &Settings, sampler: &Sampler) -> anyhow::Result<PresetStore> {
    let data = paths::data_dir(settings)?;
    paths::ensure(&data)?;
    sampler.relocate(data.join("soundboard.toml"));
    PresetStore::new(&paths::presets_dir(settings)?)
}

pub(crate) fn path_row(ui: &mut egui::Ui, label: &str, draft: &mut String) -> PathAction {
    let mut action = PathAction::None;
    ui.label(RichText::new(label).small().color(MUTED));
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(draft).desired_width(360.0));
        if ui.button("Browse…").clicked() {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                *draft = dir.display().to_string();
            }
        }
        if ui.button("Apply").clicked() {
            action = PathAction::Apply;
        }
        if ui.button("Default").clicked() {
            action = PathAction::Reset;
        }
    });
    action
}

impl App {
    pub(crate) fn set_input(&mut self, device: Option<String>) {
        self.settings.input_device = device.clone();
        if let Some(tx) = &self.engine_commands {
            let _ = tx.send(EngineCmd::SetInput(device));
        }
        self.save_settings();
    }

    pub(crate) fn apply_paths(&mut self) {
        match relocate_all(&self.settings, &self.sampler) {
            Ok(store) => {
                self.presets = store;
                self.refresh();
                self.select(self.selected);
                self.status = "Folders updated".into();
            }
            Err(err) => self.status = format!("Folder error: {err:#}"),
        }
        self.data_draft = display_path(paths::data_dir(&self.settings));
        self.presets_draft = display_path(paths::presets_dir(&self.settings));
        self.save_settings();
    }

    pub(crate) fn settings_view(&mut self, ui: &mut egui::Ui) {
        card(ui, "Audio input", |ui| {
            let devices = self.inputs.lock().map(|l| l.clone()).unwrap_or_default();
            let current = self.settings.input_device.clone();
            let label = match &current {
                None => "System default".to_string(),
                Some(name) => devices
                    .iter()
                    .find(|d| d.node_name == *name)
                    .map(|d| d.description.clone())
                    .unwrap_or_else(|| format!("{name} (not connected)")),
            };
            let mut choice = current.clone();
            egui::ComboBox::from_id_salt("input_pick")
                .width(360.0)
                .selected_text(label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut choice, None, "System default");
                    for d in &devices {
                        ui.selectable_value(&mut choice, Some(d.node_name.clone()), d.description.as_str());
                    }
                });
            if choice != current {
                self.set_input(choice);
            }
            if devices.is_empty() {
                ui.label(RichText::new("No microphones detected yet.").color(MUTED));
            }
            ui.label(
                RichText::new("Changes apply immediately. The virtual mic keeps its name.")
                    .small()
                    .color(MUTED),
            );
        });

        ui.add_space(8.0);
        card(ui, "Folders", |ui| {
            let data_action = path_row(ui, "Data folder (soundboard & board names)", &mut self.data_draft);
            let presets_action = path_row(ui, "Presets folder", &mut self.presets_draft);
            match data_action {
                PathAction::Apply => {
                    let text = self.data_draft.trim().to_string();
                    self.settings.data_dir = (!text.is_empty()).then(|| PathBuf::from(text));
                    self.apply_paths();
                }
                PathAction::Reset => {
                    self.settings.data_dir = None;
                    self.apply_paths();
                }
                PathAction::None => {}
            }
            match presets_action {
                PathAction::Apply => {
                    let text = self.presets_draft.trim().to_string();
                    self.settings.presets_dir = (!text.is_empty()).then(|| PathBuf::from(text));
                    self.apply_paths();
                }
                PathAction::Reset => {
                    self.settings.presets_dir = None;
                    self.apply_paths();
                }
                PathAction::None => {}
            }
        });

        ui.add_space(8.0);
        card(ui, "Behavior", |ui| {
            let mut changed = false;
            changed |= ui.checkbox(&mut self.snap.monitor_enabled, "Hear myself").changed();
            changed |= ui.checkbox(&mut self.snap.ducking_enabled, "Duck mic while pads play").changed();
            changed |= ui.checkbox(&mut self.settings.close_to_tray, "Closing the window (X) hides it to the tray").changed();
            if changed {
                self.params.apply(&self.snap);
                self.save_settings();
            }
            ui.label(
                RichText::new("The tray needs a StatusNotifier host (KDE, or GNOME with the AppIndicator extension).")
                    .small()
                    .color(MUTED),
            );
        });

        ui.add_space(8.0);
        card(ui, "PipeWire", |ui| {
            ui.label(format!("Server: {}", self.meters.version()));
            ui.label(format!("Quantum: {} samples", self.meters.quantum()));
            ui.label(format!("Latency: {:.2} ms", self.meters.latency_ms()));
            ui.label(
                RichText::new("Keybinds are read from /dev/input. Your user must be in the input group.")
                    .small()
                    .color(MUTED),
            );
        });
    }
}
