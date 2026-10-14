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

pub use types::{Affinity, DisplayChunk, ProjectionMode, SourceHit, VisualPos};

use std::ops::Range;

use truff_core::text::BytePos;

/// The visual projection map translating between source buffer and layout coordinates.
pub struct DisplayMap {
    source_text: String,
    mode: ProjectionMode,
    cursor: BytePos,
    selection: Option<(BytePos, BytePos)>,
    chunks: Vec<DisplayChunk>,
    visual_text: String,
    colors: Vec<truff_core::markdown::style::ColorSpan>,
    markers: Vec<(Range<usize>, Option<Range<usize>>)>,
}

impl DisplayMap {
    /// Creates a new `DisplayMap` for the given text and projection mode.
    pub fn new(text: &str, mode: ProjectionMode) -> Self {
        Self::from_owned(text.to_owned(), mode)
    }

    pub(crate) fn from_owned(text: String, mode: ProjectionMode) -> Self {
        let colors = truff_core::markdown::style::color_spans(&text);
        let mut map = Self {
            source_text: text,
            mode,
            cursor: BytePos::ZERO,
            selection: None,
            chunks: Vec::new(),
            visual_text: String::new(),
            colors,
            markers: Vec::new(),
        };
        map.scan_markers();
        map.rebuild();
        map
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
        self.source_text = text.to_string();
        self.colors = truff_core::markdown::style::color_spans(text);
        self.scan_markers();
        self.rebuild();
    }

    pub fn color_spans(&self) -> &[truff_core::markdown::style::ColorSpan] {
        &self.colors
    }

    /// Synchronizes a view projection with at most one marker rebuild.
    pub fn update(
        &mut self,
        text: Option<&str>,
        cursor: BytePos,
        selection: Option<(BytePos, BytePos)>,
    ) {
        if let Some(text) = text {
            self.source_text.clear();
            self.source_text.push_str(text);
            self.colors = truff_core::markdown::style::color_spans(text);
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
        &self.visual_text
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
