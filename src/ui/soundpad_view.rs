
use crate::input::keybinds as hotkeys;
use crate::playback::{Action, AUDIO_EXTENSIONS};
use eframe::egui::{self, Align2, RichText};
use super::app::{App, GroupAction, PadCell};
use super::theme::{DANGER, MUTED};
use super::widgets::{group_menu, sound_header, sound_row};

#[derive(Clone, Copy, PartialEq)]
enum RowAction {
    Play,
    Stop,
    Remove,
}

impl App {
    pub(crate) fn soundpad_view(&mut self, ui: &mut egui::Ui) {
        let rows: Vec<(String, usize)> = self
            .board
            .banks
            .iter()
            .map(|b| (b.name.clone(), b.pads.iter().filter(|p| p.is_loaded()).count()))
            .collect();

        let mut pick_category = None;
        let mut menu: Option<(usize, GroupAction)> = None;
        let mut add_category = false;
        let mut from_folder = false;
        let mut commit_rename = false;

        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(210.0);
                ui.label(RichText::new("CATEGORIES").small().color(MUTED));
                ui.add_space(4.0);
                let list_height = (ui.available_height() - 92.0).max(120.0);
                egui::ScrollArea::vertical()
                    .id_salt("categories")
                    .max_height(list_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, (name, count)) in rows.iter().enumerate() {
                            if self.renaming && self.bank == i {
                                let edit = ui.add(
                                    egui::TextEdit::singleline(&mut self.group_draft).desired_width(200.0),
                                );
                                if self.rename_focus {
                                    edit.request_focus();
                                    self.rename_focus = false;
                                }
                                if edit.lost_focus() {
                                    commit_rename = true;
                                }
                            } else {
                                let button = egui::Button::new(RichText::new(format!("{name}  ({count})")).strong())
                                    .selected(self.bank == i)
                                    .min_size(egui::vec2(200.0, 30.0));
                                let response = ui.add(button);
                                if response.clicked() {
                                    pick_category = Some(i);
                                }
                                response.context_menu(|ui| group_menu(ui, i, &mut menu));
                            }
                        }
                    });
                ui.add_space(6.0);
                let busy = self.import_job.is_some();
                if ui.add_sized([200.0, 26.0], egui::Button::new("+ Category")).clicked() {
                    add_category = true;
                }
                ui.add_space(4.0);
                if ui.add_enabled_ui(!busy, |ui| ui.add_sized([200.0, 26.0], egui::Button::new("From folder…"))).inner.clicked() {
                    from_folder = true;
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                self.sound_list(ui);
            });
        });

        if commit_rename {
            self.rename_group();
            self.renaming = false;
        }
        if let Some(bank) = pick_category {
            self.switch_bank(bank);
            self.renaming = false;
        }
        if add_category {
            self.new_category();
        }
        if from_folder {
            self.category_from_folder();
        }
        if let Some((bank, action)) = menu {
            match action {
                GroupAction::Rename => {
                    self.switch_bank(bank);
                    self.renaming = true;
                    self.rename_focus = true;
                }
                GroupAction::Delete => self.confirm_delete = Some(bank),
            }
        }
        self.confirm_delete_window(ui.ctx());
    }

    fn sound_list(&mut self, ui: &mut egui::Ui) {
        let cells: Vec<PadCell> = self.pad_cells();
        let query = self.search.trim().to_lowercase();
        let visible: Vec<usize> = cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.loaded && (query.is_empty() || c.title.to_lowercase().contains(&query)))
            .map(|(slot, _)| slot)
            .collect();
        let total = cells.iter().filter(|c| c.loaded).count();
        let selected = self.selected;
        let selected_loaded = cells.get(selected).is_some_and(|c| c.loaded);
        let busy = self.import_job.is_some();

        let mut nav_target: Option<usize> = None;
        let mut key_play = false;
        let mut key_remove = false;
        if !ui.ctx().wants_keyboard_input() && !visible.is_empty() {
            let (down, up, enter, delete) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowDown),
                    i.key_pressed(egui::Key::ArrowUp),
                    i.key_pressed(egui::Key::Enter),
                    i.key_pressed(egui::Key::Delete),
                )
            });
            if down || up {
                let pos = visible.iter().position(|&s| s == selected);
                nav_target = match pos {
                    None => visible.first().copied(),
                    Some(p) if down => visible.get(p + 1).copied(),
                    Some(p) => p.checked_sub(1).and_then(|q| visible.get(q).copied()),
                };
            }
            key_play = enter && selected_loaded;
            key_remove = delete && selected_loaded;
        }
        let effective = nav_target.unwrap_or(selected);
        let scroll = nav_target.is_some();

        let mut add_files = false;
        let mut add_folder = false;
        let mut clicked: Option<usize> = None;
        let mut double: Option<usize> = None;
        let mut row_menu: Option<(usize, RowAction)> = None;

        let button_w = 118.0;
        ui.horizontal(|ui| {
            let search_w = (ui.available_width() - 2.0 * button_w - 8.0).max(120.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search sounds")
                    .desired_width(search_w),
            );
            if ui.add_enabled(!busy, egui::Button::new("Add files…").min_size(egui::vec2(button_w, 0.0))).clicked() {
                add_files = true;
            }
            if ui.add_enabled(!busy, egui::Button::new("Add folder…").min_size(egui::vec2(button_w, 0.0))).clicked() {
                add_folder = true;
            }
        });
        ui.horizontal(|ui| {
            let sel = usize::from(selected_loaded);
            ui.label(RichText::new(format!("{total} sounds · {sel} selected")).small().color(MUTED));
        });
        ui.add_space(4.0);
        sound_header(ui);

        if visible.is_empty() {
            ui.add_space(24.0);
            let text = if total == 0 {
                "No sounds in this category yet.\nDrop files or folders anywhere in the window, or use Add files…"
            } else {
                "Nothing matches the search."
            };
            ui.label(RichText::new(text).color(MUTED));
        } else {
        egui::ScrollArea::vertical()
            .id_salt("sounds")
            .auto_shrink([false, false])
            .show_rows(ui, 28.0, visible.len(), |ui, range| {
                for row in range {
                    let slot = visible[row];
                    let cell = &cells[slot];
                    let is_selected = slot == effective;
                    let response = sound_row(ui, row + 1, cell, is_selected, scroll && is_selected);
                    if response.double_clicked() {
                        double = Some(slot);
                    } else if response.clicked() {
                        clicked = Some(slot);
                    }
                    response.context_menu(|ui| {
                        if ui.button("Play").clicked() {
                            row_menu = Some((slot, RowAction::Play));
                            ui.close_menu();
                        }
                        if ui.button("Stop").clicked() {
                            row_menu = Some((slot, RowAction::Stop));
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(RichText::new("Remove from category").color(DANGER)).clicked() {
                            row_menu = Some((slot, RowAction::Remove));
                            ui.close_menu();
                        }
                    });
                }
            });
        }

        if add_files {
            self.add_files_to_current();
        }
        if add_folder {
            self.add_folder_to_current();
        }
        if let Some(slot) = double.or(clicked).or(nav_target) {
            self.select(slot);
        }
        if let Some(slot) = double {
            let _ = self.actions.send(Action::Play(self.bank, slot));
        }
        if key_play {
            let _ = self.actions.send(Action::Play(self.bank, selected));
        }
        if key_remove {
            self.remove_sound(selected);
        }
        if let Some((slot, action)) = row_menu {
            match action {
                RowAction::Play => {
                    self.select(slot);
                    let _ = self.actions.send(Action::Play(self.bank, slot));
                }
                RowAction::Stop => {
                    let _ = self.actions.send(Action::Stop(self.bank, slot));
                }
                RowAction::Remove => self.remove_sound(slot),
            }
        }
    }

    fn remove_sound(&mut self, slot: usize) {
        match self.sampler.remove_pad(self.bank, slot) {
            Ok(()) => self.status = "Removed from category. The file on disk is kept.".into(),
            Err(err) => {
                self.status = format!("Error: {err:#}");
                return;
            }
        }
        self.refresh();
        hotkeys::rebuild(&self.bindings, &self.board, &self.settings.transport_keys);
        let len = self.board.banks.get(self.bank).map(|b| b.pads.len()).unwrap_or(0);
        self.select(slot.min(len.saturating_sub(1)));
    }

    fn new_category(&mut self) {
        let name = format!("Category {}", self.board.banks.len() + 1);
        match self.sampler.add_category(name.clone()) {
            Ok(index) => {
                self.refresh();
                self.switch_bank(index);
                self.group_draft = name;
                self.renaming = true;
                self.rename_focus = true;
                self.status = "Category added. Give it a name.".into();
            }
            Err(err) => self.status = format!("Error: {err:#}"),
        }
    }

    fn category_from_folder(&mut self) {
        let Some(dir) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Folder".into());
        match self.sampler.add_category(name) {
            Ok(index) => {
                self.refresh();
                self.switch_bank(index);
                self.import_paths(index, vec![dir]);
            }
            Err(err) => self.status = format!("Error: {err:#}"),
        }
    }

    fn add_files_to_current(&mut self) {
        let Some(files) = rfd::FileDialog::new()
            .add_filter("Audio", AUDIO_EXTENSIONS)
            .pick_files()
        else {
            return;
        };
        let bank = self.bank;
        self.import_paths(bank, files);
    }

    fn add_folder_to_current(&mut self) {
        let Some(dir) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        let bank = self.bank;
        self.import_paths(bank, vec![dir]);
    }

    fn confirm_delete_window(&mut self, ctx: &egui::Context) {
        let Some(bank) = self.confirm_delete else {
            return;
        };
        let name = self.board.banks.get(bank).map(|b| b.name.clone()).unwrap_or_default();
        let count = self
            .board
            .banks
            .get(bank)
            .map(|b| b.pads.iter().filter(|p| p.is_loaded()).count())
            .unwrap_or(0);
        let mut yes = false;
        let mut no = false;
        egui::Window::new("Delete category")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Delete \"{name}\" with {count} sounds? Sound files on disk are kept."));
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Delete category").color(DANGER)).clicked() {
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
    }
}
