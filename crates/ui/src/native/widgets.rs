//! Standardized Zed-inspired interactive widgets (Ghost IconButtons, Primary/Secondary Buttons,
//! modal frames, search filter bars, and key pills).
use super::icons::{Icon, render_icon};
use crate::components::color;
use crate::theme::Theme;
use egui::{CornerRadius, Response, RichText, Sense, Stroke, Ui, vec2};

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

/// Compact ghost icon button (20×20 px, 3 px corner radius) for card actions and dense headers.
pub fn compact_icon_button(ui: &mut Ui, icon: Icon, tooltip: &str) -> Response {
    let size = vec2(20.0, 20.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());

    let is_hovered = resp.hovered();
    let is_pressed = resp.is_pointer_button_down_on();
    let visuals = ui.visuals();

    if is_pressed {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(3), visuals.widgets.active.bg_fill);
    } else if is_hovered {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(3), visuals.widgets.hovered.bg_fill);
    }

    let icon_color = if is_hovered || is_pressed {
        visuals.widgets.hovered.fg_stroke.color
    } else {
        visuals.widgets.inactive.fg_stroke.color
    };

    render_icon(ui.painter(), icon, rect, icon_color, 1.3);

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

/// Monospace keyboard-shortcut badge (`enabled = false` renders a dimmed muted border/text).
pub fn key_pill(ui: &mut Ui, key_str: &str, theme: &Theme, enabled: bool) {
    let fg = color(if enabled {
        theme.accent
    } else {
        theme.text_muted
    });
    egui::Frame::new()
        .fill(color(theme.bg_canvas))
        .stroke(Stroke::new(1.0, fg))
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(4, 1))
        .show(ui, |ui| {
            ui.label(
                RichText::new(key_str)
                    .monospace()
                    .strong()
                    .small()
                    .color(fg),
            );
        });
}

/// Search filter bar with leading search icon, optional keyboard focus sync, and clear button.
/// Returns `true` whenever `query` changed (either via typing or clicking clear).
pub fn search_filter_bar(
    ui: &mut Ui,
    id: &'static str,
    query: &mut String,
    search_active: Option<&mut bool>,
    hint: &str,
    theme: &Theme,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let (icon_rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        render_icon(
            ui.painter(),
            Icon::Search,
            icon_rect,
            color(theme.text_muted),
            1.2,
        );

        let resp = ui.add(
            egui::TextEdit::singleline(query)
                .id(egui::Id::new(id))
                .hint_text(hint)
                .desired_width((ui.available_width() - 24.0).max(40.0)),
        );
        if let Some(active) = search_active {
            if resp.gained_focus() || resp.clicked() {
                *active = true;
            } else if *active && !resp.has_focus() {
                resp.request_focus();
            } else if !*active && resp.has_focus() {
                resp.surrender_focus();
            }
        }
        changed |= resp.changed();
        if !query.is_empty() && icon_button(ui, Icon::Close, "Очистить").clicked() {
            query.clear();
            changed = true;
        }
    });
    changed
}

/// Centered modal window frame shared by full-dashboard overlays (Folder Wiki, Note Stream).
pub fn centered_modal_window(
    ctx: &egui::Context,
    id: &'static str,
    width: f32,
    height: f32,
    theme: &Theme,
    body: impl FnOnce(&mut Ui),
) {
    let frame = egui::Frame::new()
        .fill(color(theme.bg_panel))
        .stroke(Stroke::new(1.0, color(theme.border)))
        .corner_radius(CornerRadius::same(theme.geometry.corner_radius as u8))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: egui::Color32::from_black_alpha(100),
        })
        .inner_margin(egui::Margin::symmetric(20, 16));

    egui::Window::new(id)
        .title_bar(false)
        .frame(frame)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .fixed_size(vec2(width, height))
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_min_size(vec2(width - 40.0, height - 32.0));
            body(ui);
        });
}
