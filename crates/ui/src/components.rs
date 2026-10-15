//! Shared geometry and styling for the existing native surfaces.
use crate::theme::Theme;
use eframe::egui;

pub(crate) use crate::theme::style::color;

pub(crate) fn overlay_frame(theme: &Theme) -> egui::Frame {
    egui::Frame::new()
        .fill(color(theme.bg_elevated))
        .stroke(egui::Stroke::new(
            theme.geometry.border_width,
            color(theme.border_subtle),
        ))
        .corner_radius(egui::CornerRadius::same(theme.geometry.overlay_radius))
        .inner_margin(egui::Margin::same(theme.spacing_lg as i8))
        .shadow(egui::epaint::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: egui::Color32::from_black_alpha(if theme.is_dark() { 64 } else { 24 }),
        })
}

pub(crate) fn dock_frame(theme: &Theme) -> egui::Frame {
    egui::Frame::new()
        .fill(color(theme.bg_panel))
        .inner_margin(egui::Margin::same(theme.spacing_sm as i8))
}

pub(crate) fn list_row(
    ui: &mut egui::Ui,
    height: f32,
    selected: bool,
    theme: &Theme,
    body: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            theme.geometry.corner_radius,
            color(if selected {
                theme.bg_selected
            } else {
                theme.bg_tertiary
            }),
        );
    }
    if selected {
        ui.painter().line_segment(
            [
                rect.left_top() + egui::vec2(0.0, 4.0),
                rect.left_bottom() - egui::vec2(0.0, 4.0),
            ],
            egui::Stroke::new(2.0, color(theme.accent_primary)),
        );
    }
    // A child paints within the row; scope_builder would advance the virtual-list cursor twice.
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(theme.spacing_sm, theme.spacing_xs))),
    );
    child.spacing_mut().interact_size.y = height - theme.spacing_xs * 2.0;
    child.spacing_mut().item_spacing.y = 0.0;
    body(&mut child);
    response
}

pub(crate) fn search_input(
    ui: &mut egui::Ui,
    text: &mut String,
    hint: &str,
    theme: &Theme,
) -> egui::Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .id(egui::Id::new("overlay_search_input"))
            .char_limit(4096)
            .hint_text(hint)
            .desired_width(ui.available_width())
            .font(egui::FontId::proportional(theme.geometry.ui_font_size))
            .margin(egui::Margin::symmetric(
                theme.spacing_md as i8,
                theme.spacing_sm as i8,
            )),
    )
}

pub(crate) fn dialog<R>(
    context: &egui::Context,
    theme: &Theme,
    id: &'static str,
    body: impl FnOnce(&mut egui::Ui) -> R,
) {
    dialog_with_footer(context, theme, id, body, |_| {});
}

pub(crate) fn dialog_with_footer<R>(
    context: &egui::Context,
    theme: &Theme,
    id: &'static str,
    body: impl FnOnce(&mut egui::Ui) -> R,
    footer: impl FnOnce(&mut egui::Ui),
) {
    let size = context.content_rect().size();
    let width = theme
        .geometry
        .dialog_width
        .min((size.x - theme.spacing_xl * 2.0).max(0.0));
    egui::Modal::new(egui::Id::new(id))
        .frame(overlay_frame(theme))
        .show(context, |ui| {
            ui.set_width((width - theme.spacing_lg * 2.0).max(0.0));
            egui::ScrollArea::vertical()
                .max_height(
                    (size.y * 0.86 - theme.spacing_lg * 3.0 - theme.geometry.list_row_height)
                        .max(1.0),
                )
                .auto_shrink([false, true])
                .show(ui, body);
            footer(ui);
        });
}

/// Call while building the scrolling body; footer controls keep their own focus.
pub(crate) fn follow_tab_focus(response: egui::Response, ui: &egui::Ui) -> egui::Response {
    if response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Tab)) {
        response.scroll_to_me(None);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_focus_in_fixed_footer_does_not_scroll_the_body() {
        let context = egui::Context::default();
        context.all_styles_mut(|style| style.animation_time = 0.0);
        let theme = Theme::nord();
        let frame = |tab| {
            let mut top = 0.0;
            let events = if tab {
                vec![egui::Event::Key {
                    key: egui::Key::Tab,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }]
            } else {
                vec![]
            };
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |_| {
                    dialog_with_footer(
                        &context,
                        &theme,
                        "footer_scope_test",
                        |ui| {
                            top = ui.label("Body top").rect.top();
                            ui.add_space(800.0);
                            ui.label("Body bottom");
                        },
                        |ui| {
                            ui.button("Fixed action").request_focus();
                        },
                    );
                },
            );
            output.textures_delta.clear();
            top
        };
        for _ in 0..12 {
            frame(false);
        }
        let before = frame(false);
        frame(true);
        for _ in 0..4 {
            frame(false);
        }
        let after = frame(false);
        assert!((after - before).abs() < 0.1);
    }
}
