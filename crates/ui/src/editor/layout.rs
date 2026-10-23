//! Text layout, HeightIndex, egui paragraph virtualization, and hit-testing (§8.1).
//!
//! Guarantees:
//! - Virtualization: egui layout is built ONLY for visible blocks (viewport ± 1 screen), caching them.
//! - Stable scroll anchor: Height adjustments above viewport rebase scroll offset without content jumping.
//! - Cursor movement does NOT invalidate or rebuild layouts of unchanged paragraphs.
//! - Grapheme cluster preservation on all hit-testing.

use std::collections::HashMap;

use unicode_segmentation::UnicodeSegmentation;

pub use super::heights::{BlockInfo, HeightIndex, ScrollAnchor};

/// Viewport-only paragraph shaping shared by headless measurement scenarios.
pub struct VirtualizedLayout {
    context: egui::Context,
    font_size: f32,
    cache: HashMap<usize, (u64, u32, std::sync::Arc<egui::Galley>)>,
    rebuild_count: usize,
}

impl Default for VirtualizedLayout {
    fn default() -> Self {
        Self::new(16.0)
    }
}

impl VirtualizedLayout {
    pub fn new(font_size: f32) -> Self {
        Self {
            context: font_context(),
            font_size,
            cache: HashMap::new(),
            rebuild_count: 0,
        }
    }
    pub fn rebuild_count(&self) -> usize {
        self.rebuild_count
    }
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
    pub fn layout_visible_window(
        &mut self,
        text: &str,
        heights: &mut HeightIndex,
        top: f32,
        height: f32,
        width: Option<f32>,
        mut anchor: Option<&mut ScrollAnchor>,
    ) -> Vec<usize> {
        if heights.block_count() == 0 {
            return Vec::new();
        }
        let (first, _) = heights.block_at_y((top - height).max(0.0));
        let (last, _) = heights.block_at_y(top + height * 2.0);
        let key = width.map_or(0, f32::to_bits);
        self.cache
            .retain(|index, _| *index >= first && *index <= last);
        for index in first..=last {
            let block = &heights.blocks()[index];
            if self
                .cache
                .get(&index)
                .is_some_and(|(revision, old_width, _)| {
                    *revision == block.revision && *old_width == key
                })
            {
                continue;
            }
            let source = text.get(block.source_range.clone()).unwrap_or("");
            let galley = shape(&self.context, source, self.font_size, width);
            let revision = block.revision;
            heights.update_block_height(
                index,
                galley.size().y.max(self.font_size),
                anchor.as_deref_mut(),
            );
            self.cache.insert(index, (revision, key, galley));
            self.rebuild_count += 1;
        }
        (first..=last).collect()
    }
    pub fn get_layout(&self, index: usize) -> Option<&std::sync::Arc<egui::Galley>> {
        self.cache.get(&index).map(|(_, _, galley)| galley)
    }
}

/// Small component-layout adapter; the production editor shapes bounded Rope windows.
pub struct EditorLayout {
    context: egui::Context,
    galley: std::sync::Arc<egui::Galley>,
    font_size: f32,
}
impl Default for EditorLayout {
    fn default() -> Self {
        Self::new(16.0)
    }
}
impl EditorLayout {
    pub fn new(font_size: f32) -> Self {
        let context = font_context();
        let galley = shape(&context, "", font_size, None);
        Self {
            context,
            galley,
            font_size,
        }
    }
    pub fn compute(&mut self, text: &str, width: Option<f32>) {
        self.galley = shape(&self.context, text, self.font_size, width);
    }
    pub fn height(&self) -> f32 {
        self.galley.size().y
    }
    pub fn width(&self) -> f32 {
        self.galley.size().x
    }
    pub fn galley(&self) -> &std::sync::Arc<egui::Galley> {
        &self.galley
    }
    pub fn hit_test(&self, text: &str, x: f32, y: f32) -> usize {
        let index = self.galley.cursor_from_pos(egui::vec2(x, y)).index.0;
        let byte = text
            .char_indices()
            .nth(index)
            .map_or(text.len(), |(byte, _)| byte);
        snap_to_grapheme_boundary(text, byte)
    }
}
fn font_context() -> egui::Context {
    let context = egui::Context::default();
    context
        .run_ui(Default::default(), |_| {})
        .drop_without_applying_deltas();
    context
}
fn shape(
    context: &egui::Context,
    text: &str,
    size: f32,
    width: Option<f32>,
) -> std::sync::Arc<egui::Galley> {
    let trimmed = text.trim_start();
    let (font, line_height) = if trimmed.starts_with("# ") {
        (egui::FontId::proportional(size * 1.375), 32.0)
    } else if trimmed.starts_with("## ") {
        (egui::FontId::proportional(size * 1.156), 28.0)
    } else if trimmed.starts_with("### ") {
        (egui::FontId::proportional(size * 1.031), 25.0)
    } else if trimmed.starts_with("```") || trimmed.starts_with("    ") {
        (egui::FontId::monospace(size * 0.875), 22.0)
    } else {
        (egui::FontId::proportional(size), 24.0)
    };
    let mut job = egui::text::LayoutJob::simple(
        text.to_owned(),
        font,
        egui::Color32::WHITE,
        width.filter(|width| *width > 0.0).unwrap_or(f32::INFINITY),
    );
    job.sections[0].format.line_height = Some(line_height);
    job.break_on_newline = true;
    context.fonts_mut(|fonts| fonts.layout_job(job))
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
        assert_eq!(snap_to_grapheme_boundary(text, 0), 0);
        assert_eq!(snap_to_grapheme_boundary(text, 1), 0);
        assert_eq!(snap_to_grapheme_boundary(text, 2), 2);
        assert_eq!(snap_to_grapheme_boundary(text, 3), 2);
    }

    #[test]
    fn test_snap_to_grapheme_boundary_emoji() {
        let text = "👨‍👩‍👧‍👦🦀";
        let family_len = "👨‍👩‍👧‍👦".len();
        assert_eq!(snap_to_grapheme_boundary(text, 5), 0);
        assert_eq!(snap_to_grapheme_boundary(text, family_len), family_len);
        assert_eq!(snap_to_grapheme_boundary(text, family_len + 1), family_len);
        assert_eq!(snap_to_grapheme_boundary(text, text.len()), text.len());
    }
}
