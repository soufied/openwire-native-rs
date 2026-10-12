use crate::input::keybinds as hotkeys;
use crate::config::paths;
use std::sync::mpsc;
use eframe::egui;
use eframe::egui::{Color32, RichText, ViewportCommand};
use crate::audio::dsp::{DspSnapshot, Params};
use crate::audio::engine::{CommandSender, InputDevice, Meters};
use crate::audio::mixer::MixerControl;
use crate::board::{Pad, Soundboard};
use crate::config::settings::{BoardLayout, Settings};
use crate::input::keybinds::{Bindings, CaptureSlot, Target};
use crate::playback::{Action, Sampler};
use crate::presets::{ORDINARY_VOICE, PresetStore};
use crate::system::tray::{TrayCmd, Wake};
use crate::system::window::{hide_to_tray, show_window};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use super::theme::{ACCENT, DANGER, MUTED, PANEL, apply_style};
use super::widgets::{display_path, dot_label, fmt_time, format_duration, meter, nav_button};

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
    pub engine_commands: Option<CommandSender>,
    pub inputs: Arc<Mutex<Vec<InputDevice>>>,
    pub engine_running: bool,
    pub capture: CaptureSlot,
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
    pub(crate) confirm_delete: Option<usize>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(33));
        self.handle_tray(ctx);
        self.poll_background();
        self.poll_capture();

        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.settings.close_to_tray {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            hide_to_tray(ctx);
        }

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
                .default_width(360.0)
                .frame(egui::Frame::default().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 10)))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.properties(ui));
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(egui::Margin::symmetric(16, 12)))
            .show(ctx, |ui| {
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
            confirm_delete: None,
        };
        app.select(0);
        app
    }

    pub(crate) fn refresh(&mut self) {
        self.board = self.sampler.snapshot();
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
        let outcome = self.waveform_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = outcome {
            self.waveform_rx = None;
            self.waveform = result.unwrap_or_default();
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
        if let Err(err) = self.settings.save() {
            self.status = format!("Settings save error: {err:#}");
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
            ui.label(RichText::new("OpenWire").size(20.0).strong().color(ACCENT));
            ui.add_space(16.0);
            meter(ui, "MIC", self.meters.mic());
            meter(ui, "OUT", self.meters.out());
            ui.add_space(16.0);
            ui.label(RichText::new(format!("{} active", self.control.active_voices())).color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let quit = egui::Button::new("Quit");
                if ui.add(quit).clicked() {
                    self.quitting = true;
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
                let hide = egui::Button::new("Hide to tray");
                if ui.add(hide).clicked() {
                    hide_to_tray(ui.ctx());
                }
                let stop = egui::Button::new(RichText::new("Stop all").strong().color(Color32::WHITE)).fill(DANGER);
                if ui.add(stop).clicked() {
                    let _ = self.actions.send(Action::StopAll);
                }
                let mut monitor = self.sampler.master_monitor();
                let slider = egui::Slider::new(&mut monitor, 0.0..=1.0)
                    .text("Monitor")
                    .show_value(false);
                if ui.add_sized([150.0, 20.0], slider).changed() {
                    self.sampler.set_master_monitor(monitor);
                }
                ui.label(RichText::new("ESC").monospace().color(MUTED));
            });
        });
    }

    pub(crate) fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.label(RichText::new("SECTIONS").small().color(MUTED));
        ui.add_space(4.0);
        nav_button(ui, &mut self.view, View::Board, "Soundboard");
        nav_button(ui, &mut self.view, View::Voice, "Voice & DSP");
        nav_button(ui, &mut self.view, View::KeyBinds, "Keybinds");
        nav_button(ui, &mut self.view, View::Settings, "Settings");
        let spare = (ui.available_height() - 28.0).max(0.0);
        ui.add_space(spare);
        let (text, color) = if self.engine_running {
            ("PipeWire connected", ACCENT)
        } else {
            ("PipeWire unavailable", DANGER)
        };
        dot_label(ui, color, text);
    }

    pub(crate) fn pad_cells(&self) -> Vec<PadCell> {
        self.board
            .banks
            .get(self.bank)
            .map(|bank| {
                bank.pads
                    .iter()
                    .map(|p| {
                        let view = self.sampler.playback(self.bank, p.id);
                        let duration = match view {
                            Some(v) => format!("{} / {}", fmt_time(v.elapsed), fmt_time(v.length)),
                            None if p.is_loaded() => format_duration(p.duration_ms),
                            None => String::new(),
                        };
                        PadCell {
                            title: if p.name.is_empty() { "empty".into() } else { p.name.clone() },
                            key: p.hotkey.clone().unwrap_or_else(|| "-".into()),
                            duration,
                            loaded: p.is_loaded(),
                            looping: p.play_mode.is_looping(),
                            playing: view.is_some(),
                            progress: view.map(|v| v.progress).unwrap_or(0.0),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
