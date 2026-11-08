mod paint;
mod spans;
pub(super) mod table;

use std::ops::Range;
use std::sync::Arc;

use egui::{Color32, Galley, Painter, Vec2};
use truff_core::text::BytePos;

use crate::app::global_config::TypographyConfig;
use crate::editor::{DisplayMap, ProjectionMode};
pub(super) use paint::ParagraphView;

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
    pub(super) table_row: Option<table::TableRowLayout>,
    pub(super) guide_xs: Box<[f32]>,
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
    pub(super) in_insert_mode: bool,
}

impl Paragraph {
    pub(super) fn new(
        range: Range<usize>,
        source: String,
        presentation: truff_core::markdown::presentation::LinePresentation,
        painter: &Painter,
        style: ParagraphStyle<'_>,
    ) -> Self {
        let mut display = DisplayMap::from_owned_with_presentation(
            source,
            style.projection,
            Some(presentation.clone()),
        );
        display.set_insert_mode(style.in_insert_mode);
        let table_row = table::TableRowLayout::build(
            display.source_text(),
            &presentation,
            painter,
            style,
            BytePos::new(usize::MAX),
            None,
        );
        let (galley, text_indent, guide_xs) =
            shape_paragraph_or_table(painter, &display, &presentation, &table_row, style);
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
            table_row,
            guide_xs,
        }
    }

    /// Logical layout dimensions of this paragraph (delegating to `TableRowLayout` on table rows).
    #[inline]
    pub(super) fn size(&self) -> Vec2 {
        if let Some(table) = &self.table_row {
            Vec2::new(table.total_width, table.height)
        } else {
            self.galley.size()
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
        let insert_mode_changed = self.display.in_insert_mode() != style.in_insert_mode;
        let view_changed =
            !self.display.matches_view(cursor, selection) || mode_changed || insert_mode_changed;
        let old_visual_revision = self.display.visual_revision();
        self.display.set_mode(style.projection);
        self.display.set_insert_mode(style.in_insert_mode);
        if view_changed {
            self.display.update(None, cursor, selection);
        }
        let projection_changed = mode_changed
            || insert_mode_changed
            || (self.table_row.is_some() && view_changed)
            || old_visual_revision != self.display.visual_revision();
        if !projection_changed
            && self.color == style.foreground
            && self.pixels_per_point == style.pixels_per_point
            && self.width == style.width
            && self.typography == TypographyKey::from(style.typography)
        {
            return false;
        }
        self.table_row = table::TableRowLayout::build(
            self.display.source_text(),
            &self.presentation,
            painter,
            style,
            cursor,
            selection,
        );
        let (galley, text_indent, guide_xs) = shape_paragraph_or_table(
            painter,
            &self.display,
            &self.presentation,
            &self.table_row,
            style,
        );
        self.galley = galley;
        self.text_indent = text_indent;
        self.guide_xs = guide_xs;
        self.color = style.foreground;
        self.pixels_per_point = style.pixels_per_point;
        self.width = style.width;
        self.typography = style.typography.into();
        true
    }

    pub(super) fn cursor_from_pos(&self, local_pos: egui::Vec2) -> egui::text::CCursor {
        if let Some(table) = &self.table_row {
            return table.cursor_from_pos(local_pos, &self.display);
        }
        self.galley.cursor_from_pos(local_pos)
    }

    pub(super) fn is_task(&self) -> bool {
        if self.display.mode() == ProjectionMode::Source {
            return false;
        }
        if let Some(list) = &self.presentation.list {
            matches!(
                list.kind,
                truff_core::markdown::presentation::ListKind::Task { .. }
            )
        } else {
            self.display.is_task()
        }
    }
}

fn shape_paragraph_or_table(
    painter: &Painter,
    display: &DisplayMap,
    presentation: &truff_core::markdown::presentation::LinePresentation,
    table_row: &Option<table::TableRowLayout>,
    style: ParagraphStyle<'_>,
) -> (Arc<Galley>, f32, Box<[f32]>) {
    // Table rows shape and paint their own cells; skip shaping the raw pipe-delimited line.
    if table_row.is_some() {
        let empty =
            painter.layout_no_wrap(String::new(), egui::FontId::default(), style.foreground);
        return (empty, 0.0, Box::default());
    }
    let (galley, text_indent) = super::super::paint::shape(
        painter,
        display,
        style.foreground,
        style.width,
        style.typography,
    );
    let list_depth = presentation.list.as_ref().map_or(0, |l| l.depth);
    let guide_xs = if list_depth > 0 {
        let font = galley
            .job
            .sections
            .first()
            .map(|s| s.format.font_id.clone())
            .unwrap_or_else(|| egui::FontId::new(15.0, egui::FontFamily::Proportional));
        spans::compute_list_guide_offsets(painter, &font, style.foreground, list_depth)
    } else {
        Box::default()
    };
    (galley, text_indent, guide_xs)
}

pub const TASK_PADDING_X: f32 = 10.0;
