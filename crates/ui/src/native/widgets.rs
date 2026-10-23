//! Standardized Zed-inspired interactive widgets (Ghost IconButtons, Primary/Secondary Buttons).
use super::icons::{Icon, render_icon};
use eframe::egui::{self, CornerRadius, Response, RichText, Sense, Ui, vec2};

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
    let font = egui::TextStyle::Button.resolve(ui.style());
    let visuals = ui.visuals();
    let fill = visuals.hyperlink_color;
    let text_color = visuals.panel_fill;
    let rich = RichText::new(text).font(font).strong().color(text_color);

    let btn = egui::Button::new(rich)
        .min_size(vec2(0.0, ui.spacing().interact_size.y.max(32.0)))
        .corner_radius(visuals.widgets.inactive.corner_radius)
        .fill(fill);

    ui.add(btn)
}

/// Primary button with a leading procedural vector icon (no missing emoji glyphs).
pub fn primary_button_with_icon(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let visuals = ui.visuals();
    let fill = visuals.hyperlink_color;
    let fg = visuals.panel_fill;
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let height = ui.spacing().interact_size.y.max(32.0);
    let pad_x = 12.0;
    let icon_w = 14.0;
    let gap = 6.0;
    let width = pad_x * 2.0 + icon_w + gap + galley.size().x;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let bg = if resp.is_pointer_button_down_on() {
        fill.gamma_multiply(0.85)
    } else if resp.hovered() {
        fill.gamma_multiply(0.93)
    } else {
        fill
    };
    ui.painter()
        .rect_filled(rect, ui.visuals().widgets.inactive.corner_radius, bg);
    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + pad_x, rect.center().y - icon_w * 0.5),
        vec2(icon_w, icon_w),
    );
    render_icon(ui.painter(), icon, icon_rect, fg, 1.5);
    let text_pos = egui::pos2(
        icon_rect.right() + gap,
        rect.center().y - galley.size().y * 0.5,
    );
    ui.painter().galley(text_pos, galley, fg);
    resp
}

/// Secondary button: height 30 px, radius 5 px, subtle border in default state.
pub fn secondary_button(ui: &mut Ui, text: &str) -> Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let visuals = ui.visuals();
    let text_color = visuals.widgets.inactive.fg_stroke.color;
    let rich = RichText::new(text).font(font).color(text_color);

    let btn = egui::Button::new(rich)
        .min_size(vec2(0.0, ui.spacing().interact_size.y.max(32.0)))
        .corner_radius(visuals.widgets.inactive.corner_radius)
        .fill(egui::Color32::TRANSPARENT);

    ui.add(btn)
}

/// Secondary button with a leading procedural vector icon (no missing emoji glyphs).
pub fn secondary_button_with_icon(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let visuals = ui.visuals();
    let fg = if visuals.dark_mode {
        visuals.widgets.inactive.fg_stroke.color
    } else {
        visuals.widgets.hovered.fg_stroke.color
    };
    let stroke = visuals.widgets.inactive.bg_stroke;
    let radius = visuals.widgets.inactive.corner_radius;
    let hovered_bg = visuals.widgets.hovered.bg_fill;
    let active_bg = visuals.widgets.active.bg_fill;
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let height = ui.spacing().interact_size.y.max(32.0);
    let pad_x = 12.0;
    let icon_w = 14.0;
    let gap = 6.0;
    let width = pad_x * 2.0 + icon_w + gap + galley.size().x;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, radius, active_bg);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, radius, hovered_bg);
    }
    ui.painter()
        .rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + pad_x, rect.center().y - icon_w * 0.5),
        vec2(icon_w, icon_w),
    );
    render_icon(ui.painter(), icon, icon_rect, fg, 1.4);
    let text_pos = egui::pos2(
        icon_rect.right() + gap,
        rect.center().y - galley.size().y * 0.5,
    );
    ui.painter().galley(text_pos, galley, fg);
    resp
}
