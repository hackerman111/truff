use super::{Color, Theme};
use egui::{FontId, TextStyle};

pub(crate) fn color(value: Color) -> egui::Color32 {
    let [r, g, b, a] = value.to_rgba_u8();
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}

impl Theme {
    pub(crate) fn apply(&self, ui: &mut egui::Ui) {
        let mut style = (*ui.ctx().global_style()).clone();
        style.visuals = if self.is_dark() {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        let visuals = &mut style.visuals;
        visuals.override_text_color = Some(color(self.text_primary));
        visuals.panel_fill = color(self.bg_canvas);
        visuals.window_fill = color(self.bg_elevated);
        visuals.extreme_bg_color = color(self.bg_canvas);
        visuals.faint_bg_color = color(self.bg_panel);
        visuals.hyperlink_color = color(self.accent_primary);
        visuals.error_fg_color = color(self.state_error);
        visuals.warn_fg_color = color(self.state_warning);
        visuals.window_stroke =
            egui::Stroke::new(self.geometry.border_width, color(self.border_subtle));
        visuals.window_corner_radius = egui::CornerRadius::same(self.geometry.overlay_radius);
        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.bg_fill = color(self.bg_panel);
            widget.weak_bg_fill = color(self.bg_panel);
            widget.bg_stroke =
                egui::Stroke::new(self.geometry.border_width, color(self.border_subtle));
            widget.fg_stroke = egui::Stroke::new(1.0, color(self.text_primary));
            widget.corner_radius = egui::CornerRadius::same(self.geometry.corner_radius as u8);
        }
        visuals.widgets.hovered.bg_fill = color(self.bg_tertiary);
        visuals.widgets.hovered.weak_bg_fill = color(self.bg_tertiary);
        visuals.widgets.hovered.bg_stroke.color = color(self.accent_primary);
        visuals.widgets.active.bg_fill = color(self.bg_selected);
        visuals.widgets.active.weak_bg_fill = color(self.bg_selected);
        visuals.widgets.active.bg_stroke.color = color(self.accent_primary);
        visuals.selection.bg_fill = color(self.bg_selected);
        visuals.selection.stroke = egui::Stroke::new(1.0, color(self.text_primary));
        style.spacing.item_spacing = egui::vec2(self.spacing_sm, self.spacing_xs);
        style.spacing.button_padding = egui::vec2(self.spacing_md, self.spacing_sm);
        style.spacing.interact_size.y = self.geometry.list_row_height;
        style.text_styles.insert(
            TextStyle::Body,
            FontId::proportional(self.geometry.ui_font_size),
        );
        style.text_styles.insert(
            TextStyle::Button,
            FontId::proportional(self.geometry.ui_font_size),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::proportional(self.geometry.ui_font_size - 2.0),
        );
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::proportional(self.geometry.ui_font_size + 3.0),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::monospace(self.geometry.ui_font_size),
        );
        ui.ctx().set_global_style(style.clone());
        *ui.style_mut() = style;
    }
}
