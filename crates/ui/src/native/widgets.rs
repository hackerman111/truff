//! Standardized Zed-inspired interactive widgets (Ghost IconButtons, Primary/Secondary Buttons).
use super::icons::{Icon, render_icon};
use eframe::egui::{self, CornerRadius, FontId, Response, RichText, Sense, Ui, vec2};

/// Ghost icon button with 28×28 px clickable area and 5 px corner radius.
pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str) -> Response {
    let size = vec2(28.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());

    let is_hovered = resp.hovered();
    let is_pressed = resp.is_pointer_button_down_on();
    let visuals = ui.visuals();

    if is_pressed {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), visuals.widgets.active.bg_fill);
    } else if is_hovered {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), visuals.widgets.hovered.bg_fill);
    }

    let icon_color = if is_hovered || is_pressed {
        visuals.widgets.hovered.fg_stroke.color
    } else {
        visuals.widgets.inactive.fg_stroke.color
    };

    render_icon(ui.painter(), icon, rect, icon_color, 1.5);

    if !tooltip.is_empty() {
        resp.on_hover_text(tooltip)
    } else {
        resp
    }
}

/// Primary button: height 32 px, radius 5 px, prominent accent fill.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let font = FontId::proportional(13.0);
    let visuals = ui.visuals();
    let fill = visuals.selection.stroke.color;
    let text_color = visuals.panel_fill;
    let rich = RichText::new(text).font(font).strong().color(text_color);

    let btn = egui::Button::new(rich)
        .min_size(vec2(0.0, 32.0))
        .corner_radius(CornerRadius::same(5))
        .fill(fill);

    ui.add(btn)
}

/// Secondary button: height 32 px, radius 5 px, ghost transparent in default state.
pub fn secondary_button(ui: &mut Ui, text: &str) -> Response {
    let font = FontId::proportional(13.0);
    let visuals = ui.visuals();
    let text_color = visuals.widgets.inactive.fg_stroke.color;
    let rich = RichText::new(text).font(font).color(text_color);

    let btn = egui::Button::new(rich)
        .min_size(vec2(0.0, 32.0))
        .corner_radius(CornerRadius::same(5))
        .fill(egui::Color32::TRANSPARENT);

    ui.add(btn)
}
