use eframe::egui;
use eframe::egui::{Color32, Stroke, TextStyle};

pub(crate) const BG: Color32 = Color32::from_rgb(10, 13, 18);

pub(crate) const PANEL: Color32 = Color32::from_rgb(16, 20, 27);

pub(crate) const CARD: Color32 = Color32::from_rgb(22, 27, 36);

pub(crate) const CARD_HI: Color32 = Color32::from_rgb(31, 38, 50);

pub(crate) const BORDER: Color32 = Color32::from_rgb(38, 47, 61);

pub(crate) const ACCENT: Color32 = Color32::from_rgb(58, 218, 180);

pub(crate) const ACCENT_DIM: Color32 = Color32::from_rgb(26, 112, 94);

pub(crate) const ACCENT_SOFT: Color32 = Color32::from_rgb(20, 56, 52);

pub(crate) const WARN: Color32 = Color32::from_rgb(242, 186, 74);

pub(crate) const DANGER: Color32 = Color32::from_rgb(238, 96, 96);

pub(crate) const MUTED: Color32 = Color32::from_rgb(122, 134, 152);

pub(crate) const TEXT: Color32 = Color32::from_rgb(232, 237, 244);

pub(crate) const PAD_IDLE: Color32 = Color32::from_rgb(28, 42, 50);

pub(crate) const PAD_EMPTY: Color32 = Color32::from_rgb(19, 23, 30);

pub(crate) const METER_BG: Color32 = Color32::from_rgb(7, 9, 12);

pub(crate) const PAD_HOVER: Color32 = Color32::from_rgb(38, 56, 66);

pub(crate) fn apply_style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = CARD;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.extreme_bg_color = METER_BG;
    visuals.faint_bg_color = CARD;
    visuals.override_text_color = Some(TEXT);
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.inactive.bg_fill = CARD_HI;
    visuals.widgets.inactive.weak_bg_fill = CARD_HI;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(44, 55, 71);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(44, 55, 71);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT_DIM);
    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.widgets.active.weak_bg_fill = ACCENT_DIM;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 9.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = 26.0;
        style.text_styles.insert(TextStyle::Heading, egui::FontId::proportional(20.0));
        style.text_styles.insert(TextStyle::Body, egui::FontId::proportional(14.5));
        style.text_styles.insert(TextStyle::Button, egui::FontId::proportional(14.0));
        style.text_styles.insert(TextStyle::Small, egui::FontId::proportional(12.0));
        style.text_styles.insert(TextStyle::Monospace, egui::FontId::monospace(13.0));
    });
}
