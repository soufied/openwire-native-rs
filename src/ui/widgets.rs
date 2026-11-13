use eframe::egui;
use eframe::egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke};
use crate::audio::dsp::DspSnapshot;
use crate::playback::PlaybackView;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::time::Duration;
use super::app::{GroupAction, PadCell, View};
use super::theme::{
    ACCENT, ACCENT_DIM, ACCENT_SOFT, BORDER, CARD, CARD_HI, DANGER, METER_BG, MUTED, PAD_EMPTY, PAD_HOVER,
    PAD_IDLE, TEXT, WARN,
};

pub(crate) fn card<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::default()
        .fill(CARD)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            if !title.is_empty() {
                ui.label(RichText::new(title.to_uppercase()).small().strong().color(MUTED));
                ui.add_space(8.0);
            }
            add(ui)
        })
        .inner
}

pub(crate) fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).small().strong().color(MUTED));
}

pub(crate) fn pill(ui: &mut egui::Ui, text: &str, fg: Color32, bg: Color32) {
    let font = FontId::proportional(12.0);
    let width = text.chars().count() as f32 * 7.0 + 20.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 20.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 10.0, bg);
    painter.text(rect.center(), Align2::CENTER_CENTER, text, font, fg);
}

pub(crate) fn nav_button(ui: &mut egui::Ui, view: &mut View, target: View, label: &str) {
    let active = *view == target;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), Sense::click());
    let painter = ui.painter_at(rect);
    let background = if active {
        CARD_HI
    } else if response.hovered() {
        CARD
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(rect, 8.0, background);
    if active {
        let bar = Rect::from_min_size(
            Pos2::new(rect.left(), rect.top() + 9.0),
            egui::vec2(3.0, rect.height() - 18.0),
        );
        painter.rect_filled(bar, 2.0, ACCENT);
    }
    let color = if active { TEXT } else { MUTED };
    painter.text(
        Pos2::new(rect.left() + 16.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.5),
        color,
    );
    if response.clicked() {
        *view = target;
    }
}

pub(crate) fn meter(ui: &mut egui::Ui, label: &str, value: f32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).small().strong().color(MUTED));
        let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 8.0), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, METER_BG);
        let level = value.clamp(0.0, 1.0);
        if level > 0.0 {
            let color = if level > 0.9 {
                DANGER
            } else if level > 0.7 {
                WARN
            } else {
                ACCENT
            };
            let fill = Rect::from_min_size(rect.min, egui::vec2(rect.width() * level, rect.height()));
            painter.rect_filled(fill, 4.0, color);
        }
    });
}

pub(crate) fn waveform_ui(
    ui: &mut egui::Ui,
    peaks: &[f32],
    view: Option<PlaybackView>,
    total_ms: u32,
) -> Option<f32> {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 112.0), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8.0, METER_BG);
    let font = FontId::monospace(12.0);

    let (elapsed, length) = match view {
        Some(v) => (v.elapsed, v.length),
        None => (Duration::ZERO, Duration::from_millis(total_ms as u64)),
    };
    painter.text(rect.left_bottom() + egui::vec2(10.0, -8.0), Align2::LEFT_BOTTOM, fmt_time(elapsed), font.clone(), TEXT);
    painter.text(rect.right_bottom() + egui::vec2(-10.0, -8.0), Align2::RIGHT_BOTTOM, fmt_time(length), font, MUTED);

    if peaks.is_empty() {
        return None;
    }
    let pointer = response
        .interact_pointer_pos()
        .map(|p| ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0));

    let progress = view.map(|v| v.progress);
    let mid = rect.center().y - 8.0;
    let half = (rect.height() - 40.0) * 0.5;
    let step = rect.width() / peaks.len() as f32;
    for (i, peak) in peaks.iter().enumerate() {
        let x = rect.left() + step * (i as f32 + 0.5);
        let fraction = i as f32 / peaks.len() as f32;
        let color = if progress.is_some_and(|p| fraction <= p) { ACCENT } else { ACCENT_DIM };
        let h = peak.max(0.04) * half;
        painter.line_segment([Pos2::new(x, mid - h), Pos2::new(x, mid + h)], Stroke::new(2.0_f32, color));
    }

    let playhead = if response.dragged() { pointer } else { progress };
    if let Some(position) = playhead {
        let x = rect.left() + rect.width() * position;
        painter.line_segment(
            [Pos2::new(x, rect.top() + 4.0), Pos2::new(x, rect.bottom() - 28.0)],
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

const INDEX_W: f32 = 44.0;
const KEY_W: f32 = 84.0;
const DURATION_W: f32 = 104.0;
const PLAYS_W: f32 = 64.0;
const GAP: f32 = 10.0;

struct Columns {
    index: Rect,
    title: Rect,
    plays: Rect,
    duration: Rect,
    key: Rect,
}

fn columns(rect: Rect) -> Columns {
    let key = Rect::from_min_max(Pos2::new(rect.right() - KEY_W, rect.top()), rect.right_bottom());
    let duration_right = key.left() - GAP;
    let duration = Rect::from_min_max(
        Pos2::new(duration_right - DURATION_W, rect.top()),
        Pos2::new(duration_right, rect.bottom()),
    );
    let index = Rect::from_min_max(rect.left_top(), Pos2::new(rect.left() + INDEX_W, rect.bottom()));
    let plays = Rect::from_min_max(
        Pos2::new(duration.left() - GAP - PLAYS_W, rect.top()),
        Pos2::new(duration.left() - GAP, rect.bottom()),
    );
    let title = Rect::from_min_max(
        Pos2::new(index.right(), rect.top()),
        Pos2::new(plays.left() - GAP, rect.bottom()),
    );
    Columns { index, title, plays, duration, key }
}

fn cell(painter: &egui::Painter, col: Rect, right_aligned: bool, text: &str, font: FontId, color: Color32) {
    let (x, align) = if right_aligned {
        (col.right(), Align2::RIGHT_CENTER)
    } else {
        (col.left() + 6.0, Align2::LEFT_CENTER)
    };
    painter
        .with_clip_rect(col)
        .text(Pos2::new(x, col.center().y), align, text, font, color);
}

fn header_rule(painter: &egui::Painter, rect: Rect) {
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0_f32, BORDER),
    );
}

pub(crate) fn table_header(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let c = columns(rect);
    let font = FontId::proportional(11.5);
    cell(&painter, c.index, false, "#", font.clone(), MUTED);
    cell(&painter, c.title, false, "NAME", font.clone(), MUTED);
    cell(&painter, c.duration, true, "DURATION", font.clone(), MUTED);
    cell(&painter, c.key, false, "KEYBIND", font, MUTED);
    header_rule(&painter, rect);
}

fn row_background(response: &egui::Response, playing: bool, selected: bool) -> Color32 {
    if playing {
        ACCENT_SOFT
    } else if selected {
        CARD_HI
    } else if response.hovered() {
        CARD
    } else {
        Color32::TRANSPARENT
    }
}

pub(crate) fn pad_row(ui: &mut egui::Ui, slot: usize, cell_data: &PadCell, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect.shrink2(egui::vec2(0.0, 2.0)), 7.0, row_background(&response, cell_data.playing, selected));
    if selected {
        let bar = Rect::from_min_size(
            Pos2::new(rect.left(), rect.top() + 8.0),
            egui::vec2(3.0, rect.height() - 16.0),
        );
        painter.rect_filled(bar, 2.0, ACCENT);
    }
    if cell_data.playing {
        let strip = Rect::from_min_size(
            Pos2::new(rect.left() + 6.0, rect.bottom() - 4.0),
            egui::vec2((rect.width() - 12.0) * cell_data.progress, 2.0),
        );
        painter.rect_filled(strip, 1.0, ACCENT);
    }
    let c = columns(rect);
    let font = FontId::proportional(14.0);
    let name_color = if !cell_data.loaded {
        MUTED
    } else if cell_data.playing {
        ACCENT
    } else {
        TEXT
    };
    cell(&painter, c.index, false, &(slot + 1).to_string(), font.clone(), MUTED);
    cell(&painter, c.title, false, cell_data.title.as_str(), font.clone(), name_color);
    cell(&painter, c.duration, true, cell_data.duration.as_str(), font.clone(), MUTED);
    cell(&painter, c.key, false, cell_data.key.as_str(), FontId::monospace(12.5), ACCENT);
    response.clicked()
}

pub(crate) fn group_menu(ui: &mut egui::Ui, bank: usize, menu: &mut Option<(usize, GroupAction)>) {
    if ui.button("Rename").clicked() {
        *menu = Some((bank, GroupAction::Rename));
        ui.close_menu();
    }
    if ui.button(RichText::new("Delete group").color(DANGER)).clicked() {
        *menu = Some((bank, GroupAction::Delete));
        ui.close_menu();
    }
}

pub(crate) fn keycap(ui: &mut egui::Ui, label: Option<&str>) {
    let Some(text) = label else {
        ui.label(RichText::new("not set").italics().color(MUTED));
        return;
    };
    let width = (text.chars().count() as f32 * 9.0 + 22.0).max(64.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 24.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, ACCENT_SOFT);
    painter.text(rect.center(), Align2::CENTER_CENTER, text, FontId::monospace(13.0), ACCENT);
}

pub(crate) fn pad_tile(ui: &mut egui::Ui, cell: &PadCell, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(164.0, 92.0), Sense::click());
    let painter = ui.painter_at(rect);
    let base = if cell.playing {
        ACCENT
    } else if !cell.loaded {
        PAD_EMPTY
    } else if response.hovered() {
        PAD_HOVER
    } else {
        PAD_IDLE
    };
    let (outline, inset) = if selected { (ACCENT, 2.0) } else { (BORDER, 1.0) };
    painter.rect_filled(rect, 12.0, outline);
    painter.rect_filled(rect.shrink(inset), 11.0, base);

    let ink = if cell.playing {
        Color32::BLACK
    } else if cell.loaded {
        TEXT
    } else {
        MUTED
    };

    if cell.key != "-" {
        let badge_w = cell.key.chars().count() as f32 * 7.5 + 14.0;
        let badge = Rect::from_min_size(rect.left_top() + egui::vec2(9.0, 9.0), egui::vec2(badge_w, 18.0));
        let (badge_bg, badge_fg) = if cell.playing {
            (Color32::BLACK, ACCENT)
        } else {
            (ACCENT_SOFT, ACCENT)
        };
        painter.rect_filled(badge, 5.0, badge_bg);
        painter.text(badge.center(), Align2::CENTER_CENTER, cell.key.as_str(), FontId::monospace(11.5), badge_fg);
    }

    let title_clip = Rect::from_min_max(rect.left_top() + egui::vec2(8.0, 30.0), rect.right_bottom() - egui::vec2(8.0, 22.0));
    painter.with_clip_rect(title_clip).text(
        title_clip.center(),
        Align2::CENTER_CENTER,
        cell.title.as_str(),
        FontId::proportional(14.5),
        ink,
    );

    let footer_color = if cell.playing { Color32::BLACK } else { MUTED };
    painter.text(
        Pos2::new(rect.left() + 10.0, rect.bottom() - 10.0),
        Align2::LEFT_BOTTOM,
        cell.duration.as_str(),
        FontId::monospace(11.0),
        footer_color,
    );
    if cell.looping {
        painter.text(
            Pos2::new(rect.right() - 10.0, rect.bottom() - 10.0),
            Align2::RIGHT_BOTTOM,
            "LOOP",
            FontId::proportional(10.5),
            if cell.playing { Color32::BLACK } else { ACCENT },
        );
    }
    if cell.playing {
        let bar = Rect::from_min_size(
            Pos2::new(rect.left() + 1.0, rect.bottom() - 5.0),
            egui::vec2((rect.width() - 2.0) * cell.progress, 4.0),
        );
        painter.rect_filled(bar, 2.0, Color32::BLACK);
    }
    response
}

pub(crate) fn chip(ui: &mut egui::Ui, name: &str, on: bool) {
    pill(
        ui,
        name,
        if on { ACCENT } else { MUTED },
        if on { ACCENT_SOFT } else { PAD_EMPTY },
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

pub(crate) fn sound_header(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let c = columns(rect);
    let font = FontId::proportional(11.5);
    cell(&painter, c.index, false, "#", font.clone(), MUTED);
    cell(&painter, c.title, false, "NAME", font.clone(), MUTED);
    cell(&painter, c.plays, true, "PLAYS", font.clone(), MUTED);
    cell(&painter, c.duration, true, "DURATION", font.clone(), MUTED);
    cell(&painter, c.key, false, "HOTKEY", font, MUTED);
    header_rule(&painter, rect);
}

pub(crate) fn sound_row(ui: &mut egui::Ui, index: usize, cell_data: &PadCell, selected: bool, scroll_to: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), Sense::click());
    if selected && scroll_to {
        response.scroll_to_me(Some(egui::Align::Center));
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect.shrink2(egui::vec2(0.0, 2.0)), 7.0, row_background(&response, cell_data.playing, selected));
    if selected {
        let bar = Rect::from_min_size(
            Pos2::new(rect.left(), rect.top() + 8.0),
            egui::vec2(3.0, rect.height() - 16.0),
        );
        painter.rect_filled(bar, 2.0, ACCENT);
    }
    if cell_data.playing {
        let strip = Rect::from_min_size(
            Pos2::new(rect.left() + 6.0, rect.bottom() - 4.0),
            egui::vec2((rect.width() - 12.0) * cell_data.progress, 2.0),
        );
        painter.rect_filled(strip, 1.0, ACCENT);
    }
    let c = columns(rect);
    let font = FontId::proportional(14.0);
    let ink = if cell_data.playing {
        ACCENT
    } else if cell_data.loaded {
        TEXT
    } else {
        MUTED
    };
    cell(&painter, c.index, false, &index.to_string(), font.clone(), MUTED);
    cell(&painter, c.title, false, cell_data.title.as_str(), font.clone(), ink);
    cell(&painter, c.plays, true, &cell_data.plays.to_string(), font.clone(), MUTED);
    cell(&painter, c.duration, true, cell_data.duration.as_str(), font.clone(), MUTED);
    cell(&painter, c.key, false, cell_data.key.as_str(), FontId::monospace(12.5), ACCENT);
    response.on_hover_text(cell_data.title.as_str())
}

pub(crate) fn switch(ui: &mut egui::Ui, on: &mut bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(44.0, 24.0), Sense::click());
    let clicked = response.clicked();
    if clicked {
        *on = !*on;
    }
    let t = ui.ctx().animate_bool(response.id, *on);
    let track = if *on { ACCENT_DIM } else { PAD_EMPTY };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, rect.height() / 2.0, track);
    let x = rect.left() + 12.0 + t * (rect.width() - 24.0);
    let knob = if *on { ACCENT } else { MUTED };
    painter.circle_filled(Pos2::new(x, rect.center().y), 9.0, knob);
    clicked
}
