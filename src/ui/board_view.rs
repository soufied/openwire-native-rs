use crate::input::keybinds as hotkeys;
use std::sync::mpsc;
use eframe::egui;
use eframe::egui::{Color32, RichText};
use crate::board::PlayMode;
use crate::config::settings::BoardLayout;
use crate::input::keybinds::Target;
use crate::playback::Action;
use super::app::{App, GroupAction};
use super::theme::{ACCENT, DANGER, MUTED, TEXT, WARN};
use super::widgets::{card, dot_label, group_menu, pad_row, pad_tile, table_header, waveform_ui};

impl App {
    pub(crate) fn import_pad(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Audio",
                &["mp3", "wav", "flac", "ogg", "oga", "m4a", "mp4", "aac", "aif", "aiff"],
            )
            .pick_file()
        else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        let request = Action::Import {
            bank: self.bank,
            slot: self.selected,
            path,
            reply: tx,
        };
        if self.actions.send(request).is_ok() {
            self.import_rx = Some(rx);
            self.status = "Importing…".into();
        } else {
            self.status = "Worker unavailable".into();
        }
    }

    pub(crate) fn board_view(&mut self, ui: &mut egui::Ui) {
        let mut layout_choice = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Layout").small().color(MUTED));
            let grid = egui::Button::new("Grid").selected(self.board_layout == BoardLayout::Grid);
            if ui.add(grid).clicked() {
                layout_choice = Some(BoardLayout::Grid);
            }
            let list = egui::Button::new("List").selected(self.board_layout == BoardLayout::List);
            if ui.add(list).clicked() {
                layout_choice = Some(BoardLayout::List);
            }
        });
        if let Some(layout) = layout_choice {
            self.board_layout = layout;
            self.settings.soundboard_layout = layout;
            self.save_settings();
        }
        ui.add_space(6.0);

        let names: Vec<String> = self.board.banks.iter().map(|b| b.name.clone()).collect();
        let cells = self.pad_cells();
        let mut switch = None;
        let mut add_pad = false;
        let mut add_group = false;
        let mut pick = None;
        let mut menu: Option<(usize, GroupAction)> = None;

        if let Some(bank) = self.confirm_delete {
            let name = names.get(bank).cloned().unwrap_or_default();
            let pads = self.board.banks.get(bank).map(|b| b.pads.len()).unwrap_or(0);
            let mut yes = false;
            let mut no = false;
            card(ui, "Delete group", |ui| {
                ui.label(format!("Delete \"{name}\" and its {pads} pads? Sound files on disk are kept."));
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Delete group").color(DANGER)).clicked() {
                        yes = true;
                    }
                    if ui.button("Cancel").clicked() {
                        no = true;
                    }
                });
            });
            if yes {
                self.delete_group(bank);
            }
            if no {
                self.confirm_delete = None;
            }
            ui.add_space(6.0);
        }

        match self.board_layout {
            BoardLayout::Grid => {
                ui.horizontal_wrapped(|ui| {
                    for (i, name) in names.iter().enumerate() {
                        let tab = egui::Button::new(RichText::new(name.as_str()).strong())
                            .selected(self.bank == i)
                            .min_size(egui::vec2(120.0, 30.0));
                        let response = ui.add(tab);
                        if response.clicked() {
                            switch = Some(i);
                        }
                        response.context_menu(|ui| group_menu(ui, i, &mut menu));
                    }
                    if ui.button("+ Group").clicked() {
                        add_group = true;
                    }
                });
                self.rename_row(ui);
                ui.add_space(8.0);
                let selected = self.selected;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("pads").spacing([10.0, 10.0]).show(ui, |ui| {
                        for (slot, cell) in cells.iter().enumerate() {
                            if pad_tile(ui, cell, selected == slot).clicked() {
                                pick = Some(slot);
                            }
                            if (slot + 1) % 5 == 0 {
                                ui.end_row();
                            }
                        }
                    });
                    ui.add_space(8.0);
                    if ui.button("+ Pad").clicked() {
                        add_pad = true;
                    }
                });
            }
            BoardLayout::List => {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(210.0);
                        ui.label(RichText::new("GROUPS").small().color(MUTED));
                        ui.add_space(4.0);
                        for (i, name) in names.iter().enumerate() {
                            let loaded = self
                                .board
                                .banks
                                .get(i)
                                .map(|b| b.pads.iter().filter(|p| p.is_loaded()).count())
                                .unwrap_or(0);
                            let entry = egui::Button::new(RichText::new(format!("{name}  ({loaded})")).strong())
                                .selected(self.bank == i)
                                .min_size(egui::vec2(200.0, 32.0));
                            let response = ui.add(entry);
                            if response.clicked() {
                                switch = Some(i);
                            }
                            response.context_menu(|ui| group_menu(ui, i, &mut menu));
                        }
                        ui.add_space(6.0);
                        if ui.button("+ Group").clicked() {
                            add_group = true;
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        self.rename_row(ui);
                        ui.add_space(6.0);
                        table_header(ui);
                        egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                            for (slot, cell) in cells.iter().enumerate() {
                                if pad_row(ui, slot, cell, self.selected == slot) {
                                    pick = Some(slot);
                                }
                            }
                        });
                        ui.add_space(6.0);
                        if ui.button("+ Pad").clicked() {
                            add_pad = true;
                        }
                    });
                });
            }
        }

        if let Some((bank, action)) = menu {
            match action {
                GroupAction::Rename => {
                    self.switch_bank(bank);
                    self.renaming = true;
                }
                GroupAction::Delete => self.confirm_delete = Some(bank),
            }
        }
        if let Some(bank) = switch {
            self.switch_bank(bank);
            self.renaming = false;
        }
        if add_group {
            self.add_group();
        }
        if add_pad {
            self.add_pad();
        }
        if let Some(slot) = pick {
            let Some(cell) = cells.get(slot) else {
                return;
            };
            let stop_instead = cell.playing && cell.looping;
            let loaded = cell.loaded;
            self.select(slot);
            if stop_instead {
                let _ = self.actions.send(Action::Stop(self.bank, slot));
            } else if loaded {
                let _ = self.actions.send(Action::Play(self.bank, slot));
            }
        }
    }

    pub(crate) fn rename_row(&mut self, ui: &mut egui::Ui) {
        if !self.renaming {
            return;
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("Group name").small().color(MUTED));
            let edit = ui.add(egui::TextEdit::singleline(&mut self.group_draft).desired_width(220.0));
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Save").clicked() || enter {
                self.rename_group();
                self.renaming = false;
            }
            if ui.button("Cancel").clicked() {
                self.renaming = false;
                self.group_draft = self
                    .board
                    .banks
                    .get(self.bank)
                    .map(|b| b.name.clone())
                    .unwrap_or_default();
            }
        });
    }

    pub(crate) fn add_group(&mut self) {
        let name = format!("Group {}", self.board.banks.len() + 1);
        match self.sampler.add_group(name.clone()) {
            Ok(index) => {
                self.refresh();
                self.switch_bank(index);
                self.group_draft = name;
                self.renaming = true;
                self.status = "Group added. Give it a name.".into();
            }
            Err(err) => self.status = format!("Error: {err:#}"),
        }
    }

    pub(crate) fn delete_group(&mut self, bank: usize) {
        self.confirm_delete = None;
        match self.sampler.delete_group(bank) {
            Ok(()) => {
                if bank < self.bank {
                    self.bank -= 1;
                }
                self.refresh();
                self.bank = self.bank.min(self.board.banks.len().saturating_sub(1));
                self.renaming = false;
                hotkeys::rebuild(&self.bindings, &self.board, &self.settings.transport_keys);
                self.select(0);
                self.status = "Group deleted".into();
            }
            Err(err) => self.status = format!("Error: {err:#}"),
        }
    }

    pub(crate) fn properties(&mut self, ui: &mut egui::Ui) {
        let Some(mut pad) = self.edit.clone() else {
            card(ui, "No pad selected", |ui| {
                ui.label(RichText::new("Click a pad to edit it.").color(MUTED));
            });
            return;
        };
        let mut import = false;
        let mut clear = false;
        let mut save = false;
        let mut stop = false;
        let mut play = false;
        let mut pause = false;
        let mut assign_key = false;
        let mut clear_key = false;
        let mut cancel_capture = false;
        let view = self.sampler.playback(self.bank, self.selected);

        let title = if pad.name.is_empty() { "Empty pad".to_string() } else { pad.name.clone() };
        ui.label(RichText::new(title).size(18.0).strong().color(TEXT));
        ui.label(
            RichText::new(format!("Group {} · pad {}", self.bank + 1, self.selected + 1))
                .small()
                .color(MUTED),
        );
        ui.add_space(6.0);

        let mut seek_to = None;
        card(ui, "Waveform", |ui| {
            seek_to = waveform_ui(ui, &self.waveform, view, pad.duration_ms);
        });
        if let Some(progress) = seek_to {
            let _ = self.actions.send(Action::Seek(self.bank, self.selected, progress));
        }

        ui.add_space(6.0);
        card(ui, "Transport", |ui| {
            ui.horizontal(|ui| {
                let play_btn = egui::Button::new(RichText::new("Play").strong().color(Color32::BLACK)).fill(ACCENT);
                play = ui.add_enabled(pad.is_loaded(), play_btn).clicked();
                let pause_label = if view.is_some_and(|v| v.paused) { "Resume" } else { "Pause" };
                pause = ui.add_enabled(view.is_some(), egui::Button::new(pause_label)).clicked();
                stop = ui.button("Stop").clicked();
                import = ui.button("Import file").clicked();
                clear = ui.button("Clear").clicked();
            });
        });

        ui.add_space(6.0);
        card(ui, "Playback", |ui| {
            egui::ComboBox::from_label("Mode")
                .selected_text(pad.play_mode.label())
                .show_ui(ui, |ui| {
                    for mode in [PlayMode::OneShot, PlayMode::Loop, PlayMode::Toggle] {
                        ui.selectable_value(&mut pad.play_mode, mode, mode.label());
                    }
                });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Keybind").color(MUTED));
                ui.label(
                    RichText::new(pad.hotkey.clone().unwrap_or_else(|| "none".into()))
                        .monospace()
                        .color(ACCENT),
                );
            });
            ui.horizontal(|ui| {
                if self.capturing(Target::Pad(self.bank, self.selected)) {
                    ui.label(RichText::new("Press a key... (ESC clears)").color(WARN));
                    if ui.button("Cancel").clicked() {
                        cancel_capture = true;
                    }
                } else {
                    if ui.button("Assign keybind").clicked() {
                        assign_key = true;
                    }
                    if ui.button("Clear").clicked() {
                        clear_key = true;
                    }
                }
            });
            ui.checkbox(&mut pad.bass_boost, "Bass boost");
        });

        ui.add_space(6.0);
        card(ui, "Levels", |ui| {
            ui.add(egui::Slider::new(&mut pad.monitor_volume, 0.0..=1.0).text("Monitor"));
            ui.add(egui::Slider::new(&mut pad.stream_volume, 0.0..=2.0).text("Virtual mic"));
        });

        ui.add_space(6.0);
        card(ui, "Trim", |ui| {
            let max_ms = pad.duration_ms.max(1);
            ui.add(egui::Slider::new(&mut pad.trim_start_ms, 0..=max_ms).text("Start, ms"));
            ui.add(egui::Slider::new(&mut pad.trim_end_ms, 0..=max_ms).text("End, ms (0 = to end)"));
        });

        let unsaved = self
            .board
            .pad(self.bank, self.selected)
            .is_some_and(|saved| format!("{saved:?}") != format!("{pad:?}"));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let save_btn = egui::Button::new(RichText::new("Save pad").strong().color(Color32::BLACK)).fill(ACCENT);
            save = ui.add(save_btn).clicked();
            if unsaved {
                dot_label(ui, WARN, "unsaved changes");
            }
        });

        self.edit = Some(pad.clone());
        if import {
            self.import_pad();
        }
        if clear {
            self.clear_pad();
        }
        if save {
            self.commit(pad);
        }
        if assign_key {
            self.start_capture(Target::Pad(self.bank, self.selected));
        }
        if clear_key {
            self.bind(Target::Pad(self.bank, self.selected), None);
        }
        if cancel_capture {
            self.stop_capture();
        }
        if play {
            let _ = self.actions.send(Action::Play(self.bank, self.selected));
        }
        if pause {
            let _ = self.actions.send(Action::TogglePause(self.bank, self.selected));
        }
        if stop {
            let _ = self.actions.send(Action::Stop(self.bank, self.selected));
        }
    }
}
