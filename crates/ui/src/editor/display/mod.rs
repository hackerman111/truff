//! Visual projection map (`DisplayMap`), marker hiding/unhiding, and coordinate mapping.
//!
//! Enforces:
//! - Source projection: 1:1 mapping of raw Markdown source to visual layout.
//! - LivePreview projection: Markdown markers (e.g. `**`, `*`, `~~`, `# `, `[[`, `]]`)
//!   are hidden, BUT if the cursor or selection touches the marker or its span,
//!   the markers are unhidden into raw Markdown for live editing.
//! - Stable roundtrip mapping between source `BytePos` and `VisualPos` with `Affinity`.

mod mapping;
mod markers;
mod projection;
mod types;

pub use types::{
    Affinity, DisplayChunk, InlineStyle, ProjectionMode, SourceHit, StyledSpan, VisualPos,
};

use std::ops::Range;

use truff_core::text::BytePos;

/// The visual projection map translating between source buffer and layout coordinates.
pub struct DisplayMap {
    source_text: String,
    mode: ProjectionMode,
    cursor: BytePos,
    selection: Option<(BytePos, BytePos)>,
    chunks: Vec<DisplayChunk>,
    visual_text: Option<Box<str>>,
    visual_revision: u64,
    colors: Vec<truff_core::markdown::style::ColorSpan>,
    styles: Vec<StyledSpan>,
    markers: Vec<(Range<usize>, Option<Range<usize>>)>,
    lists: Vec<truff_core::markdown::presentation::ListMarker>,
    presentation: Option<truff_core::markdown::presentation::LinePresentation>,
}

impl DisplayMap {
    /// Creates a new `DisplayMap` for the given text and projection mode.
    pub fn new(text: &str, mode: ProjectionMode) -> Self {
        Self::from_owned(text.to_owned(), mode)
    }

    pub(crate) fn from_owned(text: String, mode: ProjectionMode) -> Self {
        Self::from_owned_with_presentation(text, mode, None)
    }

    pub(crate) fn from_owned_with_presentation(
        text: String,
        mode: ProjectionMode,
        presentation: Option<truff_core::markdown::presentation::LinePresentation>,
    ) -> Self {
        let colors = if text.contains("<span") {
            truff_core::markdown::style::color_spans(&text)
        } else {
            Vec::new()
        };
        let mut map = Self {
            source_text: text,
            mode,
            cursor: BytePos::ZERO,
            selection: None,
            chunks: Vec::new(),
            visual_text: None,
            visual_revision: 0,
            colors,
            styles: Vec::new(),
            markers: Vec::new(),
            lists: Vec::new(),
            presentation,
        };
        map.scan_markers();
        map.rebuild();
        map
    }

    /// Monotonic revision of the projected visual text representation.
    pub fn visual_revision(&self) -> u64 {
        self.visual_revision
    }

    /// Returns the current projection mode.
    pub fn mode(&self) -> ProjectionMode {
        self.mode
    }

    /// Sets the projection mode and rebuilds mapping.
    pub fn set_mode(&mut self, mode: ProjectionMode) {
        if self.mode != mode {
            self.mode = mode;
            self.rebuild();
        }
    }

    /// Updates the source text.
    pub fn set_text(&mut self, text: &str) {
        self.presentation = None;
        self.source_text.clear();
        self.source_text.push_str(text);
        if text.contains("<span") {
            self.colors = truff_core::markdown::style::color_spans(text);
        } else {
            self.colors.clear();
        }
        self.scan_markers();
        self.visual_revision = self.visual_revision.wrapping_add(1);
        self.rebuild();
    }

    pub fn color_spans(&self) -> &[truff_core::markdown::style::ColorSpan] {
        &self.colors
    }

    pub fn styled_spans(&self) -> &[StyledSpan] {
        &self.styles
    }

    pub(crate) fn task_at_visual(&self, position: VisualPos) -> Option<usize> {
        let hit = self.visual_to_source(position);
        self.lists.iter().find_map(|list| {
            if !hit.in_marker
                || hit.pos.get() != list.range.start
                || !self
                    .chunks
                    .iter()
                    .any(|chunk| chunk.hidden && chunk.source_range() == list.range)
            {
                return None;
            }
            match list.kind {
                truff_core::markdown::presentation::ListKind::Task { checkbox, .. } => {
                    Some(checkbox)
                }
                _ => None,
            }
        })
    }

    pub(crate) fn list_content_start(&self) -> Option<VisualPos> {
        (self.mode == ProjectionMode::LivePreview).then_some(())?;
        let list = self.lists.first()?;
        Some(self.source_to_visual(BytePos::new(list.range.end), Affinity::Right))
    }

    /// Synchronizes a view projection with at most one marker rebuild.
    pub fn update(
        &mut self,
        text: Option<&str>,
        cursor: BytePos,
        selection: Option<(BytePos, BytePos)>,
    ) {
        if let Some(text) = text {
            self.presentation = None;
            self.source_text.clear();
            self.source_text.push_str(text);
            if text.contains("<span") {
                self.colors = truff_core::markdown::style::color_spans(text);
            } else {
                self.colors.clear();
            }
            self.scan_markers();
        }
        if text.is_some() || self.cursor != cursor || self.selection != selection {
            self.cursor = cursor;
            self.selection = selection;
            self.rebuild();
        }
    }

    /// Current source text.
    pub fn source_text(&self) -> &str {
        &self.source_text
    }

    /// Current projected visual text.
    pub fn visual_text(&self) -> &str {
        self.visual_text.as_deref().unwrap_or(&self.source_text)
    }

    /// Returns true if this line represents a thematic break (---, ***, ___).
    pub fn is_thematic_break(&self) -> bool {
        let trimmed = self.source_text.trim();
        trimmed == "---" || trimmed == "***" || trimmed == "___"
    }

    /// Returns true if this paragraph is a thematic break whose marker is currently hidden.
    pub fn is_rule_hidden(&self) -> bool {
        self.is_thematic_break() && self.chunks.iter().any(|c| c.is_marker && c.hidden)
    }

    /// Updates the active cursor position.
    ///
    /// If in `LivePreview`, placing the cursor on or near a hidden marker reveals it!
    pub fn set_cursor(&mut self, cursor: BytePos) {
        if self.cursor != cursor {
            self.cursor = cursor;
            self.rebuild();
        }
    }

    /// Updates active selection range `(anchor, head)`.
    pub fn set_selection(&mut self, selection: Option<(BytePos, BytePos)>) {
        if self.selection != selection {
            self.selection = selection;
            self.rebuild();
        }
    }

    pub(crate) fn matches_view(
        &self,
        cursor: BytePos,
        selection: Option<(BytePos, BytePos)>,
    ) -> bool {
        self.cursor == cursor && self.selection == selection
    }
}
