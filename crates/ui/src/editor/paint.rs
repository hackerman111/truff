//! Text rendering, syntax styling, selection, caret, and flash badge painting (§8).

use masonry::kurbo::{Affine, Point, Rect, Size};
use masonry::vello::Scene;
use masonry::vello::peniko::{Color as VelloColor, Fill};

use crate::theme::Theme;

/// Painter responsible for rendering visible editor blocks, syntax runs, selection, caret, and badges.
pub struct EditorPainter;

impl EditorPainter {
    /// Paints the editor background.
    pub fn paint_background(scene: &mut Scene, size: Size, theme: &Theme) {
        let rect = Rect::from_origin_size(Point::ZERO, size);
        let [r, g, b, a] = theme.bg_primary.to_rgba_u8();
        let color = VelloColor::from_rgba8(r, g, b, a);
        scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &rect);
    }

    /// Paints the selection highlight rectangle.
    pub fn paint_selection_rect(scene: &mut Scene, rect: Rect, theme: &Theme) {
        let [r, g, b, a] = theme.selection.to_rgba_u8();
        let color = VelloColor::from_rgba8(r, g, b, a);
        scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &rect);
    }

    /// Paints the editor cursor/caret (vertical bar in insert mode or block in normal mode).
    pub fn paint_cursor(scene: &mut Scene, pos: Point, height: f64, is_block: bool, theme: &Theme) {
        let width = if is_block { 8.0 } else { 2.0 };
        let rect = Rect::from_origin_size(pos, Size::new(width, height));
        let [r, g, b, a] = theme.cursor.to_rgba_u8();
        let color = VelloColor::from_rgba8(r, g, b, a);
        scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &rect);
    }

    /// Paints a Flash navigation badge with label over a target word.
    pub fn paint_flash_badge(scene: &mut Scene, origin: Point, label: &str, theme: &Theme) {
        let badge_width = (label.len() as f64 * 8.0 + 6.0).max(14.0);
        let badge_height = 14.0;
        let rect = Rect::from_origin_size(origin, Size::new(badge_width, badge_height));

        // Badge background
        let [r, g, b, a] = theme.flash_bg.to_rgba_u8();
        let bg_color = VelloColor::from_rgba8(r, g, b, a);
        scene.fill(Fill::NonZero, Affine::IDENTITY, bg_color, None, &rect);
    }
}
