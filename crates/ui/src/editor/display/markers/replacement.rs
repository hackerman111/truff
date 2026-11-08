//! Marker replacement strings, list marker lookup, and table cell grapheme measurement.

use std::borrow::Cow;
use std::ops::Range;

use truff_core::markdown::format_latex_math;
use truff_core::markdown::presentation::{ListKind, ListMarker};
use unicode_segmentation::UnicodeSegmentation;

use super::super::DisplayMap;

impl DisplayMap {
    pub(in crate::editor::display) fn replacement(
        &self,
        range: &Range<usize>,
    ) -> Option<Cow<'_, str>> {
        if let Some((_, pad)) = self.table_paddings.iter().find(|(r, _)| r == range) {
            return Some(Cow::Borrowed(pad.as_str()));
        }
        if self.table_pipes.iter().any(|r| r == range) {
            return Some(Cow::Borrowed(" "));
        }
        if self.presentation.is_some()
            && range.start == 0
            && let Some(list) = self.lists.first()
            && (list.range.start == range.end
                || self
                    .source_text
                    .get(..range.end)
                    .is_some_and(|s| s.bytes().all(|b| b == b' ' || b == b'\t')))
            && range.end > 0
        {
            const INDENTS: [&str; 7] = [
                "",
                "    ",
                "        ",
                "            ",
                "                ",
                "                    ",
                "                        ",
            ];
            return Some(
                INDENTS
                    .get(list.depth)
                    .copied()
                    .map_or_else(|| Cow::Owned("    ".repeat(list.depth)), Cow::Borrowed),
            );
        }
        if let Some(slice) = self.source_text.get(range.clone()) {
            let trimmed = slice.trim();
            if self
                .presentation
                .as_ref()
                .is_some_and(|p| p.table_delimiter)
                || (trimmed.starts_with('|')
                    && trimmed.ends_with('|')
                    && trimmed.contains('-')
                    && trimmed
                        .chars()
                        .all(|c| c == '|' || c == '-' || c == ':' || c == ' '))
            {
                return Some(Cow::Borrowed(""));
            }
            if trimmed == "---" || trimmed == "***" || trimmed == "___" {
                return Some(Cow::Borrowed(" "));
            }
            if trimmed == "-->" || trimmed == "->" {
                return Some(Cow::Borrowed("→"));
            }
            if trimmed == "<--" || trimmed == "<-" {
                return Some(Cow::Borrowed("←"));
            }
            if trimmed == "<-->" || trimmed == "<->" {
                return Some(Cow::Borrowed("↔"));
            }
            if (trimmed.starts_with("$$") && trimmed.ends_with("$$") && trimmed.len() >= 4)
                || (trimmed.starts_with('$') && trimmed.ends_with('$') && trimmed.len() >= 2)
            {
                return Some(Cow::Owned(format_latex_math(trimmed)));
            }
        }
        let list = self.list_at_marker(range)?;
        match list.kind {
            ListKind::Bullet => Some(Cow::Borrowed(match list.depth % 2 {
                0 => "• ",
                _ => "◦ ",
            })),
            ListKind::Task {
                checked: false,
                checkbox,
            } => {
                if self
                    .source_text
                    .as_bytes()
                    .get(checkbox)
                    .is_some_and(|&b| b == b'/' || b == b'-')
                {
                    Some(Cow::Borrowed("◐ "))
                } else {
                    Some(Cow::Borrowed("☐ "))
                }
            }
            ListKind::Task { checked: true, .. } => Some(Cow::Borrowed("☑ ")),
            ListKind::Ordered => None,
        }
    }

    pub(in crate::editor::display) fn list_at_marker(
        &self,
        range: &Range<usize>,
    ) -> Option<&ListMarker> {
        let index = self
            .lists
            .binary_search_by_key(&range.start, |list| list.range.start)
            .ok()?;
        self.lists.get(index).filter(|list| list.range == *range)
    }

    pub(super) fn visual_cell_graphemes(&self, cell_range: Range<usize>) -> usize {
        let mut visual_graphemes = 0;
        let mut last = cell_range.start;

        for (marker_range, _) in &self.markers {
            if marker_range.start < last || marker_range.start >= cell_range.end {
                continue;
            }
            if marker_range.start > last {
                let non_marker = &self.source_text[last..marker_range.start];
                visual_graphemes += non_marker.graphemes(true).count();
            }
            if let Some(rep) = self.replacement(marker_range) {
                visual_graphemes += rep.graphemes(true).count();
            }
            last = marker_range.end.min(cell_range.end);
        }

        if last < cell_range.end {
            let non_marker = &self.source_text[last..cell_range.end];
            visual_graphemes += non_marker.graphemes(true).count();
        }

        visual_graphemes
    }
}
