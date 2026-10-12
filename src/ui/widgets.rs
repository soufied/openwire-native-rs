use eframe::egui;
use eframe::egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke};
use crate::audio::dsp::DspSnapshot;
use crate::playback::PlaybackView;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::time::Duration;
use super::app::{GroupAction, PadCell, View};
use super::theme::{ACCENT, ACCENT_DIM, CARD, DANGER, METER_BG, MUTED, PAD_EMPTY, PAD_HOVER, PAD_IDLE, TEXT, WARN};

pub(crate) fn card<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::group(ui.style())
        .fill(CARD)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.label(RichText::new(title).strong().color(TEXT));
            ui.add_space(4.0);
            add(ui)
        })
        .inner
}

pub(crate) fn nav_button(ui: &mut egui::Ui, view: &mut View, target: View, label: &str) {
    let active = *view == target;
    let button = egui::Button::new(RichText::new(label).strong())
        .selected(active)
        .min_size(egui::vec2(166.0, 34.0));
    if ui.add(button).clicked() {
        *view = target;
    }
}

pub(crate) fn meter(ui: &mut egui::Ui, label: &str, value: f32) {
    ui.label(RichText::new(label).small().color(MUTED));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(150.0, 12.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, METER_BG);
    let level = value.clamp(0.0, 1.0);
    let fill = Rect::from_min_size(rect.min, egui::vec2(rect.width() * level, rect.height()));
    let color = if level > 0.9 {
        DANGER
    } else if level > 0.7 {
        WARN
    } else {
        ACCENT
    };
    painter.rect_filled(fill, 3.0, color);
}

pub(crate) fn waveform_ui(
    ui: &mut egui::Ui,
    peaks: &[f32],
    view: Option<PlaybackView>,
    total_ms: u32,
) -> Option<f32> {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 96.0), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, METER_BG);
    let font = FontId::monospace(12.0);

    let (elapsed, length) = match view {
        Some(v) => (v.elapsed, v.length),
        None => (Duration::ZERO, Duration::from_millis(total_ms as u64)),
    };
    painter.text(rect.left_bottom() + egui::vec2(8.0, -6.0), Align2::LEFT_BOTTOM, fmt_time(elapsed), font.clone(), TEXT);
    painter.text(rect.right_bottom() + egui::vec2(-8.0, -6.0), Align2::RIGHT_BOTTOM, fmt_time(length), font, MUTED);

    if peaks.is_empty() {
        return None;
    }
    let pointer = response
        .interact_pointer_pos()
        .map(|p| ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0));

    let mid = rect.center().y - 6.0;
    let step = rect.width() / peaks.len() as f32;
    let stroke = Stroke::new(1.0_f32, ACCENT_DIM);
    for (i, peak) in peaks.iter().enumerate() {
        let x = rect.left() + step * (i as f32 + 0.5);
        let h = peak * (rect.height() * 0.4);
        painter.line_segment([Pos2::new(x, mid - h), Pos2::new(x, mid + h)], stroke);
    }

    let playhead = if response.dragged() {
        pointer
    } else {
        view.map(|v| v.progress)
    };
    if let Some(progress) = playhead {
        let x = rect.left() + rect.width() * progress;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom() - 22.0)],
            Stroke::new(2.0_f32, TEXT),
        );
    }

    if response.clicked() || response.drag_stopped() {
        pointer
    } else {
        None
    }
}

pub(crate) fn fmt_time(d: Duration) -> String {
    let secs = d.as_secs_f32() as u64;
    format!("{}:{:02}", secs / 60, secs % 60)
}

pub(crate) fn dot_label(ui: &mut egui::Ui, color: Color32, text: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, color);
        ui.label(RichText::new(text).small().color(color));
    });
}

pub(crate) fn format_duration(ms: u32) -> String {
    format!("{}:{:02}", ms / 60_000, (ms / 1000) % 60)
}

pub(crate) fn table_header(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let y = rect.center().y;
    let font = FontId::proportional(12.5);
    painter.text(Pos2::new(rect.left() + 10.0, y), Align2::LEFT_CENTER, "#", font.clone(), MUTED);
    painter.text(Pos2::new(rect.left() + 48.0, y), Align2::LEFT_CENTER, "Name", font.clone(), MUTED);
    painter.text(Pos2::new(rect.right() - 170.0, y), Align2::LEFT_CENTER, "Duration", font.clone(), MUTED);
    painter.text(Pos2::new(rect.right() - 80.0, y), Align2::LEFT_CENTER, "Keybind", font, MUTED);
}

pub(crate) fn pad_row(ui: &mut egui::Ui, slot: usize, cell: &PadCell, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), Sense::click());
    let painter = ui.painter_at(rect);
    let background = if cell.playing {
        ACCENT_DIM
    } else if selected {
        Color32::from_rgb(44, 50, 60)
    } else if response.hovered() {
        Color32::from_rgb(34, 38, 46)
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(rect, 3.0, background);
    if cell.playing {
        let strip = Rect::from_min_size(
            Pos2::new(rect.left(), rect.bottom() - 3.0),
            egui::vec2(rect.width() * cell.progress, 3.0),
        );
        painter.rect_filled(strip, 0.0, ACCENT);
    }
    let y = rect.center().y;
    let font = FontId::proportional(14.0);
    let name_color = if !cell.loaded {
        MUTED
    } else if cell.playing {
        Color32::WHITE
    } else {
        TEXT
    };
    painter.text(Pos2::new(rect.left() + 10.0, y), Align2::LEFT_CENTER, format!("{}", slot + 1), font.clone(), MUTED);
    painter.text(Pos2::new(rect.left() + 48.0, y), Align2::LEFT_CENTER, cell.title.as_str(), font.clone(), name_color);
    painter.text(Pos2::new(rect.right() - 170.0, y), Align2::LEFT_CENTER, cell.duration.as_str(), font.clone(), MUTED);
    painter.text(Pos2::new(rect.right() - 80.0, y), Align2::LEFT_CENTER, cell.key.as_str(), font, ACCENT);
    response.clicked()
}

pub(crate) fn group_menu(ui: &mut egui::Ui, bank: usize, menu: &mut Option<(usize, GroupAction)>) {
    if ui.button("Rename").clicked() {
        *menu = Some((bank, GroupAction::Rename));
        ui.close_menu();
    }
    if ui.button("Delete group").clicked() {
        *menu = Some((bank, GroupAction::Delete));
        ui.close_menu();
    }
}

pub(crate) fn keycap(ui: &mut egui::Ui, label: Option<&str>) {
    let Some(text) = label else {
        ui.label(RichText::new("not set").italics().color(MUTED));
        return;
    };
    let width = (text.chars().count() as f32 * 9.0 + 24.0).max(72.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 26.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, Color32::from_rgb(38, 72, 78));
    painter.text(rect.center(), Align2::CENTER_CENTER, text, FontId::monospace(13.0), TEXT);
}

pub(crate) fn pad_tile(ui: &mut egui::Ui, cell: &PadCell, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(150.0, 72.0), Sense::click());
    let painter = ui.painter_at(rect);
    let base = if cell.playing {
        ACCENT
    } else if response.hovered() && cell.loaded {
        PAD_HOVER
    } else if cell.loaded {
        PAD_IDLE
    } else {
        PAD_EMPTY
    };
    if selected {
        painter.rect_filled(rect, 10.0, ACCENT);
        painter.rect_filled(rect.shrink(2.0), 8.0, base);
    } else {
        painter.rect_filled(rect, 10.0, base);
    }
    let ink = if cell.playing { Color32::BLACK } else if cell.loaded { TEXT } else { MUTED };
    painter.text(
        Pos2::new(rect.left() + 10.0, rect.top() + 8.0),
        Align2::LEFT_TOP,
        cell.key.as_str(),
        FontId::monospace(11.0),
        if cell.playing { Color32::BLACK } else { ACCENT },
    );
    painter.text(
        rect.center() - egui::vec2(0.0, 6.0),
        Align2::CENTER_CENTER,
        cell.title.as_str(),
        FontId::proportional(14.0),
        ink,
    );
    painter.text(
        Pos2::new(rect.right() - 10.0, rect.bottom() - 10.0),
        Align2::RIGHT_BOTTOM,
        cell.duration.as_str(),
        FontId::monospace(11.0),
        if cell.playing { Color32::BLACK } else { MUTED },
    );
    if cell.playing {
        let bar = Rect::from_min_size(
            Pos2::new(rect.left(), rect.bottom() - 4.0),
            egui::vec2(rect.width() * cell.progress, 4.0),
        );
        painter.rect_filled(bar, 0.0, Color32::BLACK);
    }
    response
}

pub(crate) fn chip(ui: &mut egui::Ui, name: &str, on: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(96.0, 26.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 13.0, if on { ACCENT_DIM } else { PAD_EMPTY });
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        name,
        FontId::proportional(13.0),
        if on { TEXT } else { MUTED },
    );
}

pub(crate) fn chain_strip(ui: &mut egui::Ui, s: &DspSnapshot) {
    let eq_on = s.eq_low_db.abs() > 0.01 || s.eq_mid_gain_db.abs() > 0.01 || s.eq_high_db.abs() > 0.01;
    let stages = [
        ("Gate", s.noise_gate_enabled),
        ("Pitch", s.pitch_enabled),
        ("Formant", (s.formant_shift - 1.0).abs() > 0.001),
        ("EQ", eq_on),
        ("Radio", s.radio_enabled),
        ("Ducking", s.ducking_enabled),
    ];
    ui.horizontal_wrapped(|ui| {
        for (name, on) in stages {
            chip(ui, name, on);
        }
    });
}

pub(crate) fn display_path(result: anyhow::Result<PathBuf>) -> String {
    result.map(|p| p.display().to_string()).unwrap_or_default()
}

pub(crate) fn slider(ui: &mut egui::Ui, value: &mut f32, range: RangeInclusive<f32>, label: &str) -> bool {
    ui.add(egui::Slider::new(value, range).text(label)).changed()
}
