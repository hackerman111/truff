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

    /// Renders Parley layout glyphs onto the Vello scene.
    pub fn paint_text(
        scene: &mut Scene,
        origin: Point,
        layout: &parley::Layout<[u8; 4]>,
        default_color: VelloColor,
    ) {
        let transform = Affine::translate(origin.to_vec2());
        for line in layout.lines() {
            for item in line.items() {
                let parley::layout::PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let style = glyph_run.style();
                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();
                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let synthesis = run.synthesis();
                let glyph_xform = synthesis
                    .skew()
                    .map(|angle| Affine::skew(angle.to_radians().tan() as f64, 0.0));
                let coords = run.normalized_coords();
                let [r, g, b, a] = style.brush;
                let brush = if [r, g, b, a] == [255, 255, 255, 255] {
                    default_color
                } else {
                    VelloColor::from_rgba8(r, g, b, a)
                };

                scene
                    .draw_glyphs(font)
                    .brush(brush)
                    .hint(false)
                    .transform(transform)
                    .glyph_transform(glyph_xform)
                    .font_size(font_size)
                    .normalized_coords(coords)
                    .draw(
                        Fill::NonZero,
                        glyph_run.glyphs().map(|glyph| {
                            let gx = x + glyph.x;
                            let gy = y - glyph.y;
                            x += glyph.advance;
                            masonry::vello::Glyph {
                                id: glyph.id,
                                x: gx,
                                y: gy,
                            }
                        }),
                    );
            }
        }
    }
}
