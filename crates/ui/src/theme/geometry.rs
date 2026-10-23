use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Density {
    #[default]
    Compact,
    Comfortable,
}

/// Geometry and typography scale parameters (§2.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometryTokens {
    pub tab_height: f32,
    pub tree_row_height: f32,
    pub status_height: f32,
    pub sidebar_default_width: f32,
    pub markdown_max_width: f32,
    pub corner_radius: f32,
    pub border_width: f32,
    pub ui_font_size: f32,
    pub editor_font_size: f32,
    pub list_row_height: f32,
    pub overlay_radius: u8,
    pub navigator_max_width: f32,
    pub navigator_width_ratio: f32,
    pub navigator_height_ratio: f32,
    pub right_default_width: f32,
    pub dialog_width: f32,
}

impl GeometryTokens {
    pub fn column_metrics(&self, available_width: f32) -> (f32, f32) {
        let width = (available_width - 48.0)
            .max(0.0)
            .min(self.markdown_max_width);
        (width, ((available_width - width) / 2.0).max(0.0))
    }
    pub const fn compact() -> Self {
        Self {
            tab_height: 32.0,
            tree_row_height: 28.0,
            status_height: 26.0,
            sidebar_default_width: 280.0,
            markdown_max_width: 800.0,
            corner_radius: 5.0,
            border_width: 1.0,
            ui_font_size: 13.0,
            editor_font_size: 14.0,
            list_row_height: 36.0,
            overlay_radius: 8,
            navigator_max_width: 1360.0,
            navigator_width_ratio: 0.86,
            navigator_height_ratio: 0.80,
            right_default_width: 320.0,
            dialog_width: 560.0,
        }
    }

    pub const fn comfortable() -> Self {
        Self {
            tab_height: 36.0,
            tree_row_height: 38.0,
            status_height: 28.0,
            sidebar_default_width: 320.0,
            markdown_max_width: 850.0,
            corner_radius: 6.0,
            border_width: 1.0,
            ui_font_size: 14.0,
            editor_font_size: 15.0,
            list_row_height: 42.0,
            overlay_radius: 10,
            navigator_max_width: 1360.0,
            navigator_width_ratio: 0.86,
            navigator_height_ratio: 0.80,
            right_default_width: 360.0,
            dialog_width: 600.0,
        }
    }
}

impl GeometryTokens {
    pub fn navigator_size(&self, available: egui::Vec2) -> egui::Vec2 {
        egui::vec2(
            (available.x * self.navigator_width_ratio).min(self.navigator_max_width),
            available.y * self.navigator_height_ratio,
        )
    }
}
