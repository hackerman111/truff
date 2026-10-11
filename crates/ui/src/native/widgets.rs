//! Standardized Zed-inspired interactive widgets (Ghost IconButtons, Primary/Secondary Buttons).
use super::icons::{Icon, render_icon};
use eframe::egui::{self, CornerRadius, FontId, Response, RichText, Sense, Ui, vec2};

/// Ghost icon button with 28×28 px clickable area and 5 px corner radius.
pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str) -> Response {
    let size = vec2(28.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());

    let is_hovered = resp.hovered();
    let is_pressed = resp.is_pointer_button_down_on();

    if is_pressed {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(5),
            egui::Color32::from_rgb(0x2D, 0x33, 0x3F),
        );
    } else if is_hovered {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(5),
            egui::Color32::from_rgb(0x39, 0x43, 0x51),
        );
    }

    let icon_color = if is_hovered || is_pressed {
        egui::Color32::from_rgb(0xE2, 0xE6, 0xED)
    } else {
        egui::Color32::from_rgb(0x8B, 0x95, 0xA5)
    };

    render_icon(ui.painter(), icon, rect, icon_color, 1.5);

    if !tooltip.is_empty() {
        resp.on_hover_text(tooltip)
    } else {
        resp
    }
}

/// Primary button: height 32 px, radius 5 px, prominent accent fill `#88A9C1`.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let font = FontId::proportional(13.0);
    let rich = RichText::new(text)
        .font(font)
        .strong()
        .color(egui::Color32::from_rgb(0x20, 0x24, 0x2C));

    let btn = egui::Button::new(rich)
        .min_size(vec2(0.0, 32.0))
        .corner_radius(CornerRadius::same(5))
        .fill(egui::Color32::from_rgb(0x88, 0xA9, 0xC1));

    ui.add(btn)
}

/// Secondary button: height 32 px, radius 5 px, ghost transparent in default state.
pub fn secondary_button(ui: &mut Ui, text: &str) -> Response {
    let font = FontId::proportional(13.0);
    let rich = RichText::new(text)
        .font(font)
        .color(egui::Color32::from_rgb(0x8B, 0x95, 0xA5));

    let btn = egui::Button::new(rich)
        .min_size(vec2(0.0, 32.0))
        .corner_radius(CornerRadius::same(5))
        .fill(egui::Color32::TRANSPARENT);

    ui.add(btn)
}
