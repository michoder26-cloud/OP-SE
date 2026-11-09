//! Colours read from the original's default dark theme (see MECHANICS.md).

use eframe::egui::{self, Color32, CornerRadius, Stroke};

pub const EDITOR: Color32 = Color32::from_rgb(0x18, 0x18, 0x18);
pub const SIDE: Color32 = Color32::from_rgb(0x14, 0x14, 0x14);
pub const WIDGET: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x1f);
pub const RAISED: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);
pub const TEXT: Color32 = Color32::from_rgb(0xd6, 0xd6, 0xdd);
pub const BRIGHT: Color32 = Color32::from_rgb(0xf0, 0xf0, 0xf0);
pub const MUTED: Color32 = Color32::from_rgb(0x8a, 0x8a, 0x8a);
pub const FAINT: Color32 = Color32::from_rgb(0x5f, 0x5f, 0x5f);
pub const BORDER: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2a);
pub const ACCENT: Color32 = Color32::from_rgb(0x81, 0xa1, 0xc1);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0x19, 0x1c, 0x22);
pub const SELECTION: Color32 = Color32::from_rgb(0x40, 0x40, 0x40);
pub const ADDED_BG: Color32 = Color32::from_rgba_premultiplied(0x0d, 0x21, 0x15, 0x33);
pub const REMOVED_BG: Color32 = Color32::from_rgba_premultiplied(0x25, 0x00, 0x0f, 0x33);
pub const ADDED: Color32 = Color32::from_rgb(0x3f, 0xa2, 0x66);
pub const REMOVED: Color32 = Color32::from_rgb(0xe0, 0x4a, 0x6f);
pub const KEYWORD: Color32 = Color32::from_rgb(0x82, 0xd2, 0xce);
pub const TYPE: Color32 = Color32::from_rgb(0xef, 0xb0, 0x80);
pub const NUMBER: Color32 = Color32::from_rgb(0xeb, 0xc8, 0x8d);
pub const STRING: Color32 = Color32::from_rgb(0xe3, 0x94, 0xdc);
pub const FUNCTION: Color32 = Color32::from_rgb(0xaa, 0xa0, 0xfa);
pub const COMMENT: Color32 = Color32::from_rgb(0x6d, 0x6d, 0x6d);
pub const LINE_NO: Color32 = Color32::from_rgb(0x5c, 0x5c, 0x5c);

pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = SIDE;
    visuals.window_fill = WIDGET;
    visuals.extreme_bg_color = EDITOR;
    visuals.faint_bg_color = WIDGET;
    visuals.code_bg_color = WIDGET;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.menu_corner_radius = CornerRadius::same(6);
    visuals.selection.bg_fill = SELECTION;
    visuals.selection.stroke = Stroke::new(1.0_f32, BRIGHT);
    visuals.hyperlink_color = ACCENT;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    let radius = CornerRadius::same(4);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.inactive.weak_bg_fill = RAISED;
    visuals.widgets.inactive.bg_fill = RAISED;
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x33, 0x33, 0x33);
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(0x3a, 0x3a, 0x3a);
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.open.corner_radius = radius;
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
        style.interaction.selectable_labels = false;
    });
}
