
use crate::board::Pad;
use crate::config::settings::SoundSort;
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
    Favorite,
    Remove,
}

fn order_slots(pads: &[Pad], slots: &mut [usize], sort: SoundSort) {
    slots.sort_by(|&a, &b| {
        let (first, second) = (&pads[a], &pads[b]);
        second.favorite.cmp(&first.favorite).then_with(|| match sort {
            SoundSort::Manual => a.cmp(&b),
            SoundSort::MostPlayed => second.play_count.cmp(&first.play_count).then(a.cmp(&b)),
            SoundSort::DateAdded => second.added_at.cmp(&first.added_at).then(a.cmp(&b)),
            SoundSort::Name => first
                .name
                .to_lowercase()
                .cmp(&second.name.to_lowercase())
                .then(a.cmp(&b)),
        })
    });
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
        let mut visible: Vec<usize> = cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.loaded && (query.is_empty() || c.title.to_lowercase().contains(&query)))
            .map(|(slot, _)| slot)
            .collect();
        let pads: &[Pad] = self.board.banks.get(self.bank).map(|b| b.pads.as_slice()).unwrap_or(&[]);
        order_slots(pads, &mut visible, self.settings.sound_sort);
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
        let mut link_folder = false;
        let mut unlink_folder = false;
        let linked_folder = self.board.banks.get(self.bank).and_then(|b| b.folder.clone());
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
            if ui
                .add_enabled(!busy, egui::Button::new("Link folder…").min_size(egui::vec2(button_w, 0.0)))
                .on_hover_text("Link this category to a folder. Its audio files are added, and browser links save there.")
                .clicked()
            {
                link_folder = true;
            }
        });
        let mut sort_choice = self.settings.sound_sort;
        ui.horizontal(|ui| {
            let sel = usize::from(selected_loaded);
            ui.label(RichText::new(format!("{total} sounds · {sel} selected")).small().color(MUTED));
            ui.label(RichText::new("·").small().color(MUTED));
            match &linked_folder {
                Some(folder) => {
                    ui.label(RichText::new(format!("Folder: {}", folder.display())).small().color(MUTED));
                    if ui.add_enabled(!busy, egui::Button::new("Unlink").small()).clicked() {
                        unlink_folder = true;
                    }
                }
                None => {
                    ui.label(RichText::new("No folder linked, links save to Downloads").small().color(MUTED));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("sound_sort")
                    .selected_text(sort_choice.label())
                    .show_ui(ui, |ui| {
                        for option in SoundSort::ALL {
                            ui.selectable_value(&mut sort_choice, option, option.label());
                        }
                    });
                ui.label(RichText::new("Sort").small().color(MUTED));
            });
        });
        if sort_choice != self.settings.sound_sort {
            self.settings.sound_sort = sort_choice;
            self.save_settings();
        }
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
                        let favorite_label = if cell.favorite {
                            "Remove from favorites"
                        } else {
                            "Add to favorites"
                        };
                        if ui.button(favorite_label).clicked() {
                            row_menu = Some((slot, RowAction::Favorite));
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
        if link_folder {
            self.link_folder_to_current();
        }
        if unlink_folder {
            self.unlink_folder_from_current();
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
                RowAction::Favorite => self.toggle_favorite(slot),
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
                let linked = self.sampler.set_group_folder(index, Some(dir.clone()));
                self.refresh();
                self.switch_bank(index);
                match linked {
                    Ok(()) => self.import_paths(index, vec![dir]),
                    Err(err) => self.status = format!("Error: {err:#}"),
                }
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

    fn link_folder_to_current(&mut self) {
        let Some(dir) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        let bank = self.bank;
        match self.sampler.set_group_folder(bank, Some(dir.clone())) {
            Ok(()) => {
                self.refresh();
                self.import_paths(bank, vec![dir]);
            }
            Err(err) => self.status = format!("Error: {err:#}"),
        }
    }

    fn unlink_folder_from_current(&mut self) {
        match self.sampler.set_group_folder(self.bank, None) {
            Ok(()) => {
                self.refresh();
                self.status = "Folder unlinked. Browser links will save to Downloads.".into();
            }
            Err(err) => self.status = format!("Error: {err:#}"),
        }
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
