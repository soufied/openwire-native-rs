use crate::input::keybinds as hotkeys;
use std::sync::mpsc;
use eframe::egui;
use eframe::egui::{RichText};
use crate::config::settings::Transport;
use crate::input::keybinds::{KEY_ESC, Target, key_label};
use super::app::App;
use super::theme::{MUTED, WARN};
use super::widgets::{card, keycap};

impl App {
    pub(crate) fn start_capture(&mut self, target: Target) {
        let (tx, rx) = mpsc::channel();
        if let Ok(mut slot) = self.capture_slot.lock() {
            *slot = Some(tx);
        }
        self.capture = Some((target, rx));
        self.status = "Press a key or numpad key (ESC removes the keybind)".into();
    }

    pub(crate) fn stop_capture(&mut self) {
        if let Ok(mut slot) = self.capture_slot.lock() {
            *slot = None;
        }
        self.capture = None;
    }

    pub(crate) fn capturing(&self, target: Target) -> bool {
        self.capture.as_ref().is_some_and(|(t, _)| *t == target)
    }

    pub(crate) fn poll_capture(&mut self) {
        let received = self.capture.as_ref().and_then(|(_, rx)| rx.try_recv().ok());
        let Some(code) = received else {
            return;
        };
        let target = self.capture.as_ref().map(|(t, _)| *t);
        self.stop_capture();
        if let Some(target) = target {
            let label = if code == KEY_ESC { None } else { Some(key_label(code)) };
            self.bind(target, label);
        }
    }

    pub(crate) fn bind(&mut self, target: Target, label: Option<String>) {
        if let Some(wanted) = label.as_deref() {
            let board = self.sampler.snapshot();
            for bank in &board.banks {
                for pad in &bank.pads {
                    if pad.hotkey.as_deref() == Some(wanted) && Target::Pad(pad.bank, pad.id) != target {
                        let mut cleared = pad.clone();
                        cleared.hotkey = None;
                        let _ = self.sampler.update_pad(pad.bank, cleared);
                    }
                }
            }
            self.settings.transport_keys.unbind_label(wanted);
        }
        match target {
            Target::Pad(bank, slot) => {
                if let Some(mut pad) = self.board.pad(bank, slot).cloned() {
                    pad.hotkey = label.clone();
                    if let Err(err) = self.sampler.update_pad(bank, pad) {
                        self.status = format!("Error: {err:#}");
                    }
                }
            }
            Target::Transport(t) => self.settings.transport_keys.set(t, label.clone()),
        }
        self.refresh();
        self.save_settings();
        hotkeys::rebuild(&self.bindings, &self.board, &self.settings.transport_keys);
        self.select(self.selected);
        self.status = match label {
            Some(name) => format!("Keybind set to {name}"),
            None => "Keybind removed".into(),
        };
    }

    pub(crate) fn keybinds_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Keybinds");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.capture.is_some() {
                    if ui.button("Cancel capture").clicked() {
                        self.stop_capture();
                    }
                    ui.label(RichText::new("Waiting for a key...").color(WARN));
                }
            });
        });
        ui.label(
            RichText::new("Press Assign, then press the key or numpad key. ESC removes the keybind.")
                .small()
                .color(MUTED),
        );
        ui.add_space(10.0);

        let mut start = None;
        let mut clear = None;
        card(ui, "Playback", |ui| {
            egui::Grid::new("transport_keys")
                .striped(true)
                .spacing([24.0, 10.0])
                .min_col_width(180.0)
                .show(ui, |ui| {
                    ui.label(RichText::new("Action").small().color(MUTED));
                    ui.label(RichText::new("Keybind").small().color(MUTED));
                    ui.label("");
                    ui.end_row();
                    for t in Transport::ALL {
                        ui.label(RichText::new(t.label()).strong());
                        if self.capturing(Target::Transport(t)) {
                            ui.label(RichText::new("Press a key...").color(WARN));
                        } else {
                            keycap(ui, self.settings.transport_keys.get(t));
                        }
                        ui.horizontal(|ui| {
                            if ui.button("Assign").clicked() {
                                start = Some(Target::Transport(t));
                            }
                            let bound = self.settings.transport_keys.get(t).is_some();
                            if ui.add_enabled(bound, egui::Button::new("Clear")).clicked() {
                                clear = Some(Target::Transport(t));
                            }
                        });
                        ui.end_row();
                    }
                });
        });

        ui.add_space(12.0);
        let board = self.board.clone();
        let total = board
            .banks
            .iter()
            .flat_map(|b| b.pads.iter())
            .filter(|p| p.hotkey.is_some())
            .count();
        card(ui, &format!("Sound keybinds ({total})"), |ui| {
            if total == 0 {
                ui.label(
                    RichText::new("No sounds have a keybind yet. Select a pad in Soundboard and press Assign keybind.")
                        .color(MUTED),
                );
                return;
            }
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                egui::Grid::new("sound_keybinds")
                    .striped(true)
                    .spacing([24.0, 8.0])
                    .min_col_width(140.0)
                    .show(ui, |ui| {
                        for (bank_idx, bank) in board.banks.iter().enumerate() {
                            for pad in &bank.pads {
                                let Some(key) = pad.hotkey.as_deref() else {
                                    continue;
                                };
                                ui.label(RichText::new(bank.name.as_str()).color(MUTED));
                                let name = if pad.name.is_empty() { "empty".to_string() } else { pad.name.clone() };
                                ui.label(RichText::new(name).strong());
                                if self.capturing(Target::Pad(bank_idx, pad.id)) {
                                    ui.label(RichText::new("Press a key...").color(WARN));
                                } else {
                                    keycap(ui, Some(key));
                                }
                                ui.horizontal(|ui| {
                                    if ui.button("Assign").clicked() {
                                        start = Some(Target::Pad(bank_idx, pad.id));
                                    }
                                    if ui.button("Clear").clicked() {
                                        clear = Some(Target::Pad(bank_idx, pad.id));
                                    }
                                });
                                ui.end_row();
                            }
                        }
                    });
            });
        });

        if let Some(target) = start {
            self.start_capture(target);
        }
        if let Some(target) = clear {
            self.bind(target, None);
        }
    }
}
