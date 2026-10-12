use eframe::egui;
use eframe::egui::{Color32, TextStyle};

pub(crate) const ACCENT: Color32 = Color32::from_rgb(46, 196, 160);

pub(crate) const ACCENT_DIM: Color32 = Color32::from_rgb(28, 120, 100);

pub(crate) const WARN: Color32 = Color32::from_rgb(230, 180, 60);

pub(crate) const DANGER: Color32 = Color32::from_rgb(220, 80, 80);

pub(crate) const MUTED: Color32 = Color32::from_rgb(140, 150, 162);

pub(crate) const TEXT: Color32 = Color32::from_rgb(222, 228, 235);

pub(crate) const CARD: Color32 = Color32::from_rgb(26, 29, 35);

pub(crate) const PANEL: Color32 = Color32::from_rgb(18, 20, 24);

pub(crate) const PAD_IDLE: Color32 = Color32::from_rgb(33, 46, 52);

pub(crate) const PAD_EMPTY: Color32 = Color32::from_rgb(28, 30, 36);

pub(crate) const METER_BG: Color32 = Color32::from_rgb(12, 14, 17);

pub(crate) const PAD_HOVER: Color32 = Color32::from_rgb(44, 62, 70);

pub(crate) fn apply_style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = CARD;
    visuals.extreme_bg_color = METER_BG;
    visuals.faint_bg_color = CARD;
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.hyperlink_color = ACCENT;
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(38, 42, 50);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(38, 42, 50);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(50, 56, 66);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(50, 56, 66);
    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.widgets.active.weak_bg_fill = ACCENT_DIM;
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.text_styles.insert(TextStyle::Heading, egui::FontId::proportional(22.0));
        style.text_styles.insert(TextStyle::Body, egui::FontId::proportional(15.0));
        style.text_styles.insert(TextStyle::Button, egui::FontId::proportional(15.0));
        style.text_styles.insert(TextStyle::Small, egui::FontId::proportional(12.5));
    });
}
