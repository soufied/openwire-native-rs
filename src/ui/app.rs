use crate::input::keybinds as hotkeys;
use crate::config::paths;
use crate::links;
use std::sync::mpsc;
use eframe::egui;
use eframe::egui::{Align2, Color32, FontId, RichText, ViewportCommand};
use std::path::{Path, PathBuf};
use crate::audio::dsp::{DspSnapshot, Params};
use crate::audio::engine::{CommandSender, InputDevice, Meters};
use crate::audio::mixer::MixerControl;
use crate::board::{Pad, Soundboard};
use crate::config::settings::{AppMode, BoardLayout, Settings};
use crate::input::keybinds::{Bindings, CaptureSlot, Target};
use crate::playback::{is_audio_file, Action, ImportReport, Sampler};
use crate::presets::{ORDINARY_VOICE, PresetStore};
use crate::system::instance::Message as IpcMessage;
use crate::system::tray::{TrayCmd, Wake};
use crate::system::window::{hide_to_tray, show_window};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use super::theme::{ACCENT, ACCENT_SOFT, BG, CARD, CARD_HI, DANGER, MUTED, PANEL, TEXT, apply_style};
use super::widgets::{display_path, dot_label, fmt_time, format_duration, meter, nav_button, pill, section_label};

pub struct Wiring {
    pub sampler: Arc<Sampler>,
    pub actions: Sender<Action>,
    pub params: Arc<Params>,
    pub meters: Arc<Meters>,
    pub control: MixerControl,
    pub bindings: Bindings,
    pub settings: Settings,
    pub presets: PresetStore,
    pub tray_rx: Receiver<TrayCmd>,
    pub ipc_rx: Receiver<IpcMessage>,
    pub engine_commands: Option<CommandSender>,
    pub inputs: Arc<Mutex<Vec<InputDevice>>>,
    pub engine_running: bool,
    pub capture: CaptureSlot,
}

pub(crate) struct ImportJob {
    pub(crate) label: String,
    pub(crate) total: Arc<AtomicUsize>,
    pub(crate) done: Arc<AtomicUsize>,
    pub(crate) rx: Receiver<Result<ImportReport, String>>,
}

pub(crate) struct LinkJob {
    pub(crate) label: String,
    pub(crate) group: String,
    pub(crate) rx: Receiver<Result<String, String>>,
}

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum View {
    Board,
    Voice,
    KeyBinds,
    Settings,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum GroupAction {
    Rename,
    Delete,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum PathAction {
    None,
    Apply,
    Reset,
}

pub(crate) struct PadCell {
    pub(crate) title: String,
    pub(crate) favorite: bool,
    pub(crate) plays: u64,
    pub(crate) key: String,
    pub(crate) duration: String,
    pub(crate) loaded: bool,
    pub(crate) looping: bool,
    pub(crate) playing: bool,
    pub(crate) progress: f32,
}

pub struct App {
    pub(crate) sampler: Arc<Sampler>,
    pub(crate) actions: Sender<Action>,
    pub(crate) params: Arc<Params>,
    pub(crate) meters: Arc<Meters>,
    pub(crate) control: MixerControl,
    pub(crate) bindings: Bindings,
    pub(crate) settings: Settings,
    pub(crate) presets: PresetStore,
    pub(crate) tray_rx: Receiver<TrayCmd>,
    pub(crate) ipc_rx: Receiver<IpcMessage>,
    pub(crate) engine_commands: Option<CommandSender>,
    pub(crate) inputs: Arc<Mutex<Vec<InputDevice>>>,
    pub(crate) engine_running: bool,
    pub(crate) board: Soundboard,
    pub(crate) view: View,
    pub(crate) bank: usize,
    pub(crate) selected: usize,
    pub(crate) edit: Option<Pad>,
    pub(crate) waveform: Vec<f32>,
    pub(crate) waveform_rx: Option<Receiver<Result<Vec<f32>, String>>>,
    pub(crate) import_rx: Option<Receiver<Result<Pad, String>>>,
    pub(crate) snap: DspSnapshot,
    pub(crate) preset_choice: String,
    pub(crate) preset_name: String,
    pub(crate) data_draft: String,
    pub(crate) presets_draft: String,
    pub(crate) group_draft: String,
    pub(crate) status: String,
    pub(crate) quitting: bool,
    pub(crate) board_layout: BoardLayout,
    pub(crate) capture_slot: CaptureSlot,
    pub(crate) capture: Option<(Target, Receiver<u16>)>,
    pub(crate) renaming: bool,
    pub(crate) rename_focus: bool,
    pub(crate) confirm_delete: Option<usize>,
    pub(crate) app_mode: AppMode,
    pub(crate) search: String,
    pub(crate) import_job: Option<ImportJob>,
    pub(crate) link_jobs: Vec<LinkJob>,
    pub(crate) board_revision: u64,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(33));
        self.handle_tray(ctx);
        self.handle_ipc(ctx);
        self.sync_board();
        self.poll_background();
        self.poll_capture();

        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.settings.close_to_tray {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            hide_to_tray(ctx);
        }

        self.handle_dropped(ctx);
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::default().fill(PANEL).inner_margin(egui::Margin::symmetric(14, 10)))
            .show(ctx, |ui| self.header(ui));
        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::default().fill(PANEL).inner_margin(egui::Margin::symmetric(14, 6)))
            .show(ctx, |ui| {
                ui.label(RichText::new(self.status.as_str()).small().color(MUTED));
            });
        egui::SidePanel::left("nav")
            .exact_width(190.0)
            .frame(egui::Frame::default().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 8)))
            .show(ctx, |ui| self.sidebar(ui));
        if self.view == View::Board {
            egui::SidePanel::right("props")
                .default_width(372.0)
                .frame(egui::Frame::default().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 10)))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.properties(ui));
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(BG).inner_margin(egui::Margin::symmetric(18, 14)))
            .show(ctx, |ui| {
                self.import_banner(ui);
                if self.view == View::Board && self.app_mode == AppMode::Soundpad {
                    self.soundpad_view(ui);
                    return;
                }
                egui::ScrollArea::vertical().show(ui, |ui| match self.view {
                    View::Board => self.board_view(ui),
                    View::Voice => self.voice_view(ui),
                    View::KeyBinds => self.keybinds_view(ui),
                    View::Settings => self.settings_view(ui),
                });
            });
    }
}

impl App {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>, w: Wiring, wake: Wake) -> Self {
        apply_style(&cc.egui_ctx);
        if let Ok(mut slot) = wake.lock() {
            *slot = Some(cc.egui_ctx.clone());
        }
        let board = w.sampler.snapshot();
        let data_draft = display_path(paths::data_dir(&w.settings));
        let presets_draft = display_path(paths::presets_dir(&w.settings));
        let snap = w.params.snapshot();
        let board_layout = w.settings.soundboard_layout;
        let group_draft = board.banks.first().map(|b| b.name.clone()).unwrap_or_default();
        let app_mode = w.settings.app_mode;
        let mut app = Self {
            sampler: w.sampler,
            actions: w.actions,
            params: w.params,
            meters: w.meters,
            control: w.control,
            bindings: w.bindings,
            settings: w.settings,
            presets: w.presets,
            tray_rx: w.tray_rx,
            ipc_rx: w.ipc_rx,
            engine_commands: w.engine_commands,
            inputs: w.inputs,
            engine_running: w.engine_running,
            board,
            view: View::Board,
            bank: 0,
            selected: 0,
            edit: None,
            waveform: Vec::new(),
            waveform_rx: None,
            import_rx: None,
            snap,
            preset_choice: ORDINARY_VOICE.to_string(),
            preset_name: String::new(),
            data_draft,
            presets_draft,
            group_draft,
            status: "Ready".into(),
            quitting: false,
            board_layout,
            capture_slot: w.capture,
            capture: None,
            renaming: false,
            rename_focus: false,
            confirm_delete: None,
            app_mode,
            search: String::new(),
            import_job: None,
            link_jobs: Vec::new(),
            board_revision: 0,
        };
        app.select(0);
        app
    }

    pub(crate) fn refresh(&mut self) {
        self.board = self.sampler.snapshot();
    }

    pub(crate) fn sync_board(&mut self) {
        let revision = self.sampler.revision();
        if revision == self.board_revision {
            return;
        }
        self.board_revision = revision;
        self.refresh();
        if let (Some(edit), Some(saved)) = (self.edit.as_mut(), self.board.pad(self.bank, self.selected)) {
            edit.play_count = saved.play_count;
            edit.favorite = saved.favorite;
            edit.added_at = saved.added_at;
        }
    }

    pub(crate) fn select(&mut self, slot: usize) {
        self.selected = slot;
        let _ = self.actions.send(Action::Select(self.bank, slot));
        self.edit = self.board.pad(self.bank, slot).cloned();
        self.waveform.clear();
        self.waveform_rx = None;
        if self.edit.as_ref().is_some_and(|p| p.is_loaded()) {
            let (tx, rx) = mpsc::channel();
            let request = Action::Waveform {
                bank: self.bank,
                slot,
                points: 160,
                reply: tx,
            };
            if self.actions.send(request).is_ok() {
                self.waveform_rx = Some(rx);
            }
        }
    }

    pub(crate) fn switch_bank(&mut self, bank: usize) {
        self.bank = bank;
        self.group_draft = self.board.banks.get(bank).map(|b| b.name.clone()).unwrap_or_default();
        self.refresh();
        self.select(0);
    }

    pub(crate) fn rename_group(&mut self) {
        let name = self.group_draft.trim().to_string();
        if name.is_empty() {
            self.status = "Group name cannot be empty".into();
            return;
        }
        match self.sampler.rename_bank(self.bank, name.clone()) {
            Ok(()) => self.status = format!("Renamed group to {name}"),
            Err(err) => self.status = format!("Error: {err:#}"),
        }
        self.refresh();
    }

    pub(crate) fn add_pad(&mut self) {
        match self.sampler.add_pad(self.bank) {
            Ok(pad) => {
                self.status = format!("Added pad {}", pad.id + 1);
                self.refresh();
                self.select(pad.id);
            }
            Err(err) => self.status = format!("Error: {err:#}"),
        }
    }

    pub(crate) fn poll_background(&mut self) {
        let outcome = self.import_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = outcome {
            self.import_rx = None;
            self.status = match result {
                Ok(pad) => format!("Imported: {}", pad.name),
                Err(err) => format!("Import error: {err}"),
            };
            self.refresh();
            self.select(self.selected);
        }
        let finished = self.import_job.as_ref().and_then(|job| job.rx.try_recv().ok());
        if let Some(result) = finished {
            self.import_job = None;
            self.status = match result {
                Ok(report) if report.added == 0 && report.failed.is_empty() => {
                    "No supported audio files found".into()
                }
                Ok(report) => match report.failed.first() {
                    None => format!("Added {} sound(s)", report.added),
                    Some(first) => format!(
                        "Added {} sound(s), {} failed. First error: {first}",
                        report.added,
                        report.failed.len()
                    ),
                },
                Err(err) => format!("Import error: {err}"),
            };
            self.refresh();
            self.select(self.selected);
        }
        let outcome = self.waveform_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = outcome {
            self.waveform_rx = None;
            self.waveform = result.unwrap_or_default();
        }
        self.poll_links();
    }

    pub(crate) fn handle_ipc(&mut self, ctx: &egui::Context) {
        while let Ok(message) = self.ipc_rx.try_recv() {
            match message {
                IpcMessage::Show => show_window(ctx),
                IpcMessage::Open(raw) => self.open_link(&raw),
            }
        }
    }

    pub(crate) fn open_link(&mut self, raw: &str) {
        let Some(url) = links::parse_soundpad_link(raw) else {
            self.status = "Unsupported link".into();
            return;
        };
        let file = links::file_name_from_url(&url);
        if !is_audio_file(Path::new(&file)) {
            self.status = format!("Link is not an audio file: {file}");
            return;
        }
        let bank = self.bank;
        let Some(group) = self.board.banks.get(bank) else {
            self.status = "No group selected".into();
            return;
        };
        let group_name = group.name.clone();
        let folder = match group.folder.clone() {
            Some(folder) => folder,
            None => match paths::downloads_dir(&self.settings) {
                Ok(folder) => folder,
                Err(err) => {
                    self.status = format!("Error: {err:#}");
                    return;
                }
            },
        };
        let (tx, rx) = mpsc::channel();
        let sampler = self.sampler.clone();
        let url_bg = url.clone();
        let spawned = std::thread::Builder::new()
            .name("openwire-link".into())
            .spawn(move || {
                let outcome = links::fetch_into_group(&sampler, bank, &url_bg, &folder)
                    .map_err(|err| format!("{err:#}"));
                let _ = tx.send(outcome);
            });
        match spawned {
            Ok(_) => {
                self.status = format!("Downloading {file} to {group_name}…");
                self.link_jobs.push(LinkJob {
                    label: file,
                    group: group_name,
                    rx,
                });
            }
            Err(err) => self.status = format!("Cannot start download: {err}"),
        }
    }

    pub(crate) fn toggle_favorite(&mut self, slot: usize) {
        let favorite = self.board.pad(self.bank, slot).is_some_and(|p| p.favorite);
        match self.sampler.set_pad_favorite(self.bank, slot, !favorite) {
            Ok(()) => {
                self.status = if favorite { "Removed from favorites".into() } else { "Added to favorites".into() };
                self.refresh();
            }
            Err(err) => self.status = format!("Error: {err:#}"),
        }
    }

    pub(crate) fn poll_links(&mut self) {
        let mut index = 0;
        while index < self.link_jobs.len() {
            let outcome = match self.link_jobs[index].rx.try_recv() {
                Ok(outcome) => outcome,
                Err(mpsc::TryRecvError::Empty) => {
                    index += 1;
                    continue;
                }
                Err(mpsc::TryRecvError::Disconnected) => Err("download stopped unexpectedly".to_string()),
            };
            let job = self.link_jobs.remove(index);
            match outcome {
                Ok(sound) => {
                    self.status = format!("Added \u{201C}{sound}\u{201D} to {}", job.group);
                    self.refresh();
                    self.select(self.selected);
                }
                Err(err) => self.status = format!("Link \u{201C}{}\u{201D} failed: {err}", job.label),
            }
        }
    }

    pub(crate) fn commit(&mut self, pad: Pad) {
        match self.sampler.update_pad(self.bank, pad) {
            Ok(()) => self.status = "Pad saved".into(),
            Err(err) => self.status = format!("Save error: {err:#}"),
        }
        self.refresh();
        hotkeys::rebuild(&self.bindings, &self.board, &self.settings.transport_keys);
        self.select(self.selected);
    }

    pub(crate) fn clear_pad(&mut self) {
        match self.sampler.clear_pad(self.bank, self.selected) {
            Ok(()) => self.status = "Pad cleared".into(),
            Err(err) => self.status = format!("Error: {err:#}"),
        }
        self.refresh();
        self.select(self.selected);
    }

    pub(crate) fn save_settings(&mut self) {
        self.settings.monitor_enabled = self.snap.monitor_enabled;
        self.settings.ducking_enabled = self.snap.ducking_enabled;
        self.settings.voice_enabled = self.snap.voice_enabled;
        if let Err(err) = self.settings.save() {
            self.status = format!("Settings save error: {err:#}");
        }
    }

    pub(crate) fn import_banner(&self, ui: &mut egui::Ui) {
        let Some(job) = &self.import_job else {
            return;
        };
        let total = job.total.load(Ordering::Relaxed);
        let done = job.done.load(Ordering::Relaxed);
        egui::Frame::default()
            .fill(CARD)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    if total == 0 {
                        ui.label(RichText::new(format!("Scanning {}…", job.label)).color(TEXT));
                    } else {
                        ui.label(
                            RichText::new(format!("Importing {} · {done} of {total}", job.label)).color(TEXT),
                        );
                    }
                });
                let bar = if total == 0 {
                    egui::ProgressBar::new(0.0).animate(true)
                } else {
                    egui::ProgressBar::new(done as f32 / total as f32).show_percentage()
                };
                ui.add(bar.desired_width(ui.available_width()));
            });
        ui.add_space(8.0);
    }

    pub(crate) fn handle_dropped(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            if self.view == View::Board {
                let bank = self.bank;
                self.import_paths(bank, dropped);
            } else {
                self.status = "Open Soundboard or Soundpad to add sounds".into();
            }
        }
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if hovering && self.view == View::Board {
            let name = self
                .board
                .banks
                .get(self.bank)
                .map(|b| b.name.clone())
                .unwrap_or_default();
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("drop_hint"),
            ));
            let rect = ctx.screen_rect();
            painter.rect_filled(rect, 0.0, Color32::from_black_alpha(170));
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                format!("Drop sounds or folders into \u{201C}{name}\u{201D}"),
                FontId::proportional(24.0),
                TEXT,
            );
        }
    }

    pub(crate) fn handle_tray(&mut self, ctx: &egui::Context) {
        while let Ok(cmd) = self.tray_rx.try_recv() {
            match cmd {
                TrayCmd::Show => show_window(ctx),
                TrayCmd::StopAll => {
                    let _ = self.actions.send(Action::StopAll);
                }
                TrayCmd::DisableEffects => self.disable_effects(),
                TrayCmd::NextPreset => self.cycle_preset(),
                TrayCmd::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }
    }

    pub(crate) fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("OpenWire").size(19.0).strong().color(ACCENT));
            ui.add_space(18.0);
            meter(ui, "MIC", self.meters.mic());
            ui.add_space(6.0);
            meter(ui, "OUT", self.meters.out());
            ui.add_space(14.0);
            let voices = self.control.active_voices();
            let (fg, bg) = if voices > 0 { (ACCENT, ACCENT_SOFT) } else { (MUTED, CARD_HI) };
            pill(ui, &format!("{voices} playing"), fg, bg);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("Quit")).clicked() {
                    self.quitting = true;
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
                if ui.add(egui::Button::new("Hide to tray")).clicked() {
                    hide_to_tray(ui.ctx());
                }
                let stop = egui::Button::new(RichText::new("Stop all").strong().color(Color32::WHITE)).fill(DANGER);
                if ui.add(stop).clicked() {
                    let _ = self.actions.send(Action::StopAll);
                }
                ui.add_space(14.0);
                let mut monitor = self.sampler.master_monitor();
                ui.label(RichText::new(format!("{:.0}%", monitor * 100.0)).monospace().color(TEXT));
                let slider = egui::Slider::new(&mut monitor, 0.0..=1.0).show_value(false);
                let volume = ui.add_sized([150.0, 20.0], slider);
                if volume.changed() {
                    self.sampler.set_master_monitor(monitor);
                }
                if volume.drag_stopped() || (volume.changed() && !volume.dragged()) {
                    self.settings.master_monitor = monitor;
                    self.save_settings();
                }
                ui.label(RichText::new("VOLUME").small().strong().color(MUTED));
            });
        });
    }

    pub(crate) fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        section_label(ui, "Play");
        ui.add_space(4.0);
        nav_button(ui, &mut self.view, View::Board, self.app_mode.label());
        ui.add_space(16.0);
        section_label(ui, "Control");
        ui.add_space(4.0);
        let voice_label = if self.snap.voice_enabled { "Voice & DSP" } else { "Voice & DSP (off)" };
        nav_button(ui, &mut self.view, View::Voice, voice_label);
        nav_button(ui, &mut self.view, View::KeyBinds, "Keybinds");
        nav_button(ui, &mut self.view, View::Settings, "Settings");
        let spare = (ui.available_height() - 72.0).max(0.0);
        ui.add_space(spare);
        let (text, color) = if self.engine_running {
            ("PipeWire connected", ACCENT)
        } else {
            ("PipeWire unavailable", DANGER)
        };
        dot_label(ui, color, text);
        ui.add_space(4.0);
        ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).small().color(MUTED));
    }

    pub(crate) fn pad_cells(&self) -> Vec<PadCell> {
        let Some(bank) = self.board.banks.get(self.bank) else {
            return Vec::new();
        };
        let views = self.sampler.playback_views(self.bank);
        bank.pads
            .iter()
            .map(|p| {
                let view = views.get(&p.id).copied();
                let duration = match view {
                    Some(v) => format!("{} / {}", fmt_time(v.elapsed), fmt_time(v.length)),
                    None if p.is_loaded() => format_duration(p.duration_ms),
                    None => String::new(),
                };
                PadCell {
                    title: match (p.favorite, p.name.is_empty()) {
                        (true, true) => "\u{2605} empty".into(),
                        (true, false) => format!("\u{2605} {}", p.name),
                        (false, true) => "empty".into(),
                        (false, false) => p.name.clone(),
                    },
                    favorite: p.favorite,
                    plays: p.play_count,
                    key: p.hotkey.clone().unwrap_or_else(|| "-".into()),
                    duration,
                    loaded: p.is_loaded(),
                    looping: p.play_mode.is_looping(),
                    playing: view.is_some(),
                    progress: view.map_or(0.0, |v| v.progress),
                }
            })
            .collect()
    }
}
