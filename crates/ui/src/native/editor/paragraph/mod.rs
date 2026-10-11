mod paint;
use crate::editor::{DisplayMap, ProjectionMode};
use eframe::egui::{Color32, Galley, Painter};
pub(super) use paint::ParagraphView;
use std::ops::Range;
use std::sync::Arc;
use truff_core::text::BytePos;

pub(super) struct Paragraph {
    pub(super) range: Range<usize>,
    pub(super) display: DisplayMap,
    pub(super) galley: Arc<Galley>,
    pub(super) revision: u64,
    color: Color32,
    pixels_per_point: f32,
    width: f32,
}

#[derive(Clone, Copy)]
pub(super) struct ParagraphStyle {
    pub(super) foreground: Color32,
    pub(super) pixels_per_point: f32,
    pub(super) width: f32,
    pub(super) projection: ProjectionMode,
}
impl Paragraph {
    pub(super) fn new(
        range: Range<usize>,
        source: &str,
        painter: &Painter,
        style: ParagraphStyle,
    ) -> Self {
        let display = DisplayMap::new(source, style.projection);
        let galley = super::super::paint::shape(painter, &display, style.foreground, style.width);
        Self {
            range,
            display,
            galley,
            revision: 0,
            color: style.foreground,
            pixels_per_point: style.pixels_per_point,
            width: style.width,
        }
    }
    /// Synchronizes view-dependent markers and reshapes only if the rendered paragraph changes.
    pub(super) fn refresh(
        &mut self,
        painter: &Painter,
        style: ParagraphStyle,
        cursor: BytePos,
        selection: Option<(BytePos, BytePos)>,
    ) -> bool {
        let view_changed = !self.display.matches_view(cursor, selection)
            || self.display.mode() != style.projection;
        let old_projection = view_changed.then(|| self.display.visual_text().to_owned());
        self.display.set_mode(style.projection);
        self.display.update(None, cursor, selection);
        let projection_changed = old_projection
            .as_deref()
            .is_some_and(|old| old != self.display.visual_text());
        if !projection_changed
            && self.color == style.foreground
            && self.pixels_per_point == style.pixels_per_point
            && self.width == style.width
        {
            return false;
        }
        self.galley =
            super::super::paint::shape(painter, &self.display, style.foreground, style.width);
        self.color = style.foreground;
        self.pixels_per_point = style.pixels_per_point;
        self.width = style.width;
        true
    }
}
