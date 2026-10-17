mod paint;
use crate::app::global_config::TypographyConfig;
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
    pub(super) text_indent: f32,
    pub(super) revision: u64,
    pub(super) presentation: truff_core::markdown::presentation::LinePresentation,
    pub(super) presentation_revision: u64,
    color: Color32,
    pixels_per_point: f32,
    pub(super) width: f32,
    typography: TypographyKey,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct TypographyKey {
    proportional: bool,
    font_size: u32,
    line_height: u32,
}

impl From<&TypographyConfig> for TypographyKey {
    fn from(typography: &TypographyConfig) -> Self {
        Self {
            proportional: typography.font_family == "proportional",
            font_size: typography.base_font_size.to_bits(),
            line_height: typography.line_height_multiplier.to_bits(),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ParagraphStyle<'a> {
    pub(super) foreground: Color32,
    pub(super) pixels_per_point: f32,
    pub(super) width: f32,
    pub(super) projection: ProjectionMode,
    pub(super) typography: &'a TypographyConfig,
}
impl Paragraph {
    pub(super) fn new(
        range: Range<usize>,
        source: String,
        presentation: truff_core::markdown::presentation::LinePresentation,
        painter: &Painter,
        style: ParagraphStyle<'_>,
    ) -> Self {
        let display = DisplayMap::from_owned_with_presentation(
            source,
            style.projection,
            Some(presentation.clone()),
        );
        let (galley, text_indent) = super::super::paint::shape(
            painter,
            &display,
            style.foreground,
            style.width,
            style.typography,
        );
        Self {
            range,
            display,
            galley,
            text_indent,
            revision: 0,
            presentation,
            presentation_revision: 0,
            color: style.foreground,
            pixels_per_point: style.pixels_per_point,
            width: style.width,
            typography: style.typography.into(),
        }
    }
    /// Synchronizes view-dependent markers and reshapes only if the rendered paragraph changes.
    pub(super) fn refresh(
        &mut self,
        painter: &Painter,
        style: ParagraphStyle<'_>,
        cursor: BytePos,
        selection: Option<(BytePos, BytePos)>,
    ) -> bool {
        let mode_changed = self.display.mode() != style.projection;
        let view_changed = !self.display.matches_view(cursor, selection) || mode_changed;
        let old_visual_revision = self.display.visual_revision();
        self.display.set_mode(style.projection);
        if view_changed {
            self.display.update(None, cursor, selection);
        }
        let projection_changed = mode_changed || old_visual_revision != self.display.visual_revision();
        if !projection_changed
            && self.color == style.foreground
            && self.pixels_per_point == style.pixels_per_point
            && self.width == style.width
            && self.typography == TypographyKey::from(style.typography)
        {
            return false;
        }
        (self.galley, self.text_indent) = super::super::paint::shape(
            painter,
            &self.display,
            style.foreground,
            style.width,
            style.typography,
        );
        self.color = style.foreground;
        self.pixels_per_point = style.pixels_per_point;
        self.width = style.width;
        self.typography = style.typography.into();
        true
    }
}
