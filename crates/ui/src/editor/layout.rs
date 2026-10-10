//! Text layout and hit-testing implementation using Parley.

use parley::layout::{Cluster, ClusterSide};
use parley::style::StyleProperty;
use parley::{FontContext, Layout, LayoutContext};
use unicode_segmentation::UnicodeSegmentation;

/// Text layout wrapper ensuring UTF-8 and grapheme cluster boundary safety.
pub struct EditorLayout {
    font_cx: FontContext,
    layout_cx: LayoutContext<[u8; 4]>,
    layout: Layout<[u8; 4]>,
    font_size: f32,
    last_width: f32,
}

impl Default for EditorLayout {
    fn default() -> Self {
        Self::new(16.0)
    }
}

impl EditorLayout {
    /// Creates a new editor layout manager with the given font size in logical pixels.
    pub fn new(font_size: f32) -> Self {
        Self {
            font_cx: FontContext::new(),
            layout_cx: LayoutContext::new(),
            layout: Layout::new(),
            font_size,
            last_width: 0.0,
        }
    }

    /// Recomputes text layout for the given string with an optional wrap width.
    pub fn compute(&mut self, text: &str, wrap_width: Option<f32>) {
        let mut builder = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(self.font_size));
        builder.push_default(StyleProperty::Brush([255, 255, 255, 255]));

        self.layout = builder.build(text);
        if let Some(width) = wrap_width
            && width > 0.0
        {
            self.layout.break_all_lines(Some(width));
            self.last_width = width;
            return;
        }
        self.layout.break_all_lines(None);
        self.last_width = self.layout.width();
    }

    /// Returns the total height of the laid out text.
    pub fn height(&self) -> f32 {
        self.layout.height()
    }

    /// Returns the width of the laid out text.
    pub fn width(&self) -> f32 {
        self.layout.width()
    }

    /// Hit-tests a coordinate (x, y) in layout space against the given text.
    ///
    /// GUARANTEE: The returned offset is ALWAYS a valid UTF-8 byte index
    /// and snaps cleanly to a grapheme cluster boundary. It never slices
    /// a multi-byte code point or an extended grapheme cluster.
    pub fn hit_test(&self, text: &str, x: f32, y: f32) -> usize {
        if text.is_empty() {
            return 0;
        }

        let raw_offset = self.raw_hit_test(text, x, y);
        snap_to_grapheme_boundary(text, raw_offset)
    }

    fn raw_hit_test(&self, text: &str, x: f32, y: f32) -> usize {
        if y < 0.0 {
            return 0;
        }
        if y > self.layout.height() {
            return text.len();
        }

        // Try Parley's cluster-based point hit test.
        if let Some((cluster, side)) = Cluster::from_point(&self.layout, x, y) {
            let range = cluster.text_range();
            return match side {
                ClusterSide::Left => range.start,
                ClusterSide::Right => range.end,
            };
        }

        // Fallback: search lines for closest vertical match.
        let mut best_line = None;
        let mut prev_bottom = 0.0f32;

        for line in self.layout.lines() {
            let metrics = line.metrics();
            let top = prev_bottom;
            let bottom = top + metrics.line_height;
            if y >= top && y <= bottom {
                best_line = Some(line);
                break;
            }
            prev_bottom = bottom;
        }

        if let Some(line) = best_line {
            let range = line.text_range();
            if x <= 0.0 { range.start } else { range.end }
        } else if x <= 0.0 {
            0
        } else {
            text.len()
        }
    }
}

/// Snaps an arbitrary byte offset to the nearest valid UTF-8 grapheme cluster boundary.
///
/// Ensures:
/// 1. Returned index <= text.len()
/// 2. `text.is_char_boundary(index)` is true
/// 3. Never splits a combining character, surrogate, or emoji sequence.
pub fn snap_to_grapheme_boundary(text: &str, byte_offset: usize) -> usize {
    if byte_offset == 0 || text.is_empty() {
        return 0;
    }
    if byte_offset >= text.len() {
        return text.len();
    }

    // Fast-path: check if it's already on a grapheme boundary.
    let mut closest = 0;
    let mut min_distance = usize::MAX;

    for (start, cluster) in text.grapheme_indices(true) {
        let start_dist = start.abs_diff(byte_offset);
        if start_dist < min_distance {
            min_distance = start_dist;
            closest = start;
        }

        let end = start + cluster.len();
        let end_dist = end.abs_diff(byte_offset);
        if end_dist < min_distance {
            min_distance = end_dist;
            closest = end;
        }

        if start > byte_offset && start_dist > min_distance {
            break;
        }
    }

    debug_assert!(
        text.is_char_boundary(closest),
        "Result must be on a valid UTF-8 character boundary"
    );
    closest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snap_to_grapheme_boundary_cyrillic() {
        let text = "Привет, мир!";
        // Cyrillic letters are 2 bytes each in UTF-8
        // 'П' is bytes 0..2, 'р' is bytes 2..4
        assert_eq!(snap_to_grapheme_boundary(text, 0), 0);
        assert_eq!(snap_to_grapheme_boundary(text, 1), 0); // inside 'П' -> snaps to 0 or 2
        assert_eq!(snap_to_grapheme_boundary(text, 2), 2);
        assert_eq!(snap_to_grapheme_boundary(text, 3), 2); // inside 'р' -> snaps to 2 or 4
    }

    #[test]
    fn test_snap_to_grapheme_boundary_emoji() {
        // Family emoji: 25 bytes, single grapheme cluster with ZWJ
        let text = "👨‍👩‍👧‍👦🦀";
        let family_len = "👨‍👩‍👧‍👦".len();
        assert_eq!(snap_to_grapheme_boundary(text, 5), 0);
        assert_eq!(snap_to_grapheme_boundary(text, family_len), family_len);
        assert_eq!(snap_to_grapheme_boundary(text, family_len + 1), family_len);
        assert_eq!(snap_to_grapheme_boundary(text, text.len()), text.len());
    }
}
