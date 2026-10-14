use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use truff_core::markdown::presentation::{LinePresentation, ListKind, ListMarker};

use super::DisplayMap;

impl DisplayMap {
    pub(super) fn scan_markers(&mut self) {
        self.markers.clear();
        self.lists.clear();
        if self.presentation.as_ref().is_some_and(|line| line.code) {
            self.colors.clear();
            return;
        }
        let text = &self.source_text;
        let indent = if self
            .presentation
            .as_ref()
            .is_some_and(|line| line.list.is_some())
        {
            text.len() - text.trim_start_matches([' ', '\t']).len()
        } else {
            0
        };
        let mut text_ranges: Vec<Range<usize>> = Vec::new();
        let mut depth = 0usize;
        let mut current_item = None;
        for (event, range) in Parser::new_ext(
            &text[indent..],
            Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
        )
        .into_offset_iter()
        {
            let range = range.start + indent..range.end + indent;
            match event {
                Event::Start(Tag::List(_)) => depth += 1,
                Event::End(TagEnd::List(_)) => depth = depth.saturating_sub(1),
                Event::Start(Tag::Item) => {
                    let tail = &text[range.start..range.end];
                    let prefix_len = tail
                        .bytes()
                        .take_while(|byte| !byte.is_ascii_whitespace())
                        .count();
                    let end = range.start + prefix_len;
                    let kind = if tail.starts_with(['-', '*', '+']) {
                        ListKind::Bullet
                    } else {
                        ListKind::Ordered
                    };
                    self.lists.push(ListMarker {
                        range: range.start..end,
                        depth: depth.saturating_sub(1),
                        kind,
                    });
                    current_item = Some(self.lists.len() - 1);
                }
                Event::TaskListMarker(checked) => {
                    if let Some(index) = current_item {
                        self.lists[index].range.end = range.end;
                        self.lists[index].kind = ListKind::Task {
                            checked,
                            checkbox: range.start + 1,
                        };
                    }
                }
                Event::Start(Tag::Strong | Tag::Emphasis | Tag::Strikethrough) => {
                    let source = &text[range.clone()];
                    let marker_len = if source.starts_with("**")
                        || source.starts_with("__")
                        || source.starts_with("~~")
                    {
                        2
                    } else {
                        1
                    };
                    self.markers
                        .push((range.start..range.start + marker_len, Some(range.clone())));
                    self.markers
                        .push((range.end - marker_len..range.end, Some(range)));
                }
                Event::Code(_) => {
                    let count = text[range.clone()]
                        .bytes()
                        .take_while(|byte| *byte == b'`')
                        .count();
                    self.markers
                        .push((range.start..range.start + count, Some(range.clone())));
                    self.markers
                        .push((range.end - count..range.end, Some(range)));
                }
                Event::Start(Tag::Heading { .. }) => {
                    let prefix = text[range.clone()]
                        .bytes()
                        .take_while(|byte| *byte == b'#')
                        .count();
                    if prefix > 0 {
                        let end = range.start + prefix;
                        let end = end + usize::from(text.as_bytes().get(end) == Some(&b' '));
                        self.markers.push((range.start..end, Some(range)));
                    }
                }
                Event::Text(_) => {
                    if let Some(previous) = text_ranges
                        .last_mut()
                        .filter(|previous| previous.end == range.start)
                    {
                        previous.end = range.end;
                    } else {
                        text_ranges.push(range);
                    }
                }
                _ => {}
            }
        }
        for range in text_ranges {
            let mut start = range.start;
            while let Some(open) = text[start..range.end].find("[[") {
                let open = start + open;
                let Some(close) = text[open + 2..range.end].find("]]") else {
                    break;
                };
                let close = open + 2 + close;
                let span = open..close + 2;
                self.markers.push((open..open + 2, Some(span.clone())));
                self.markers.push((close..close + 2, Some(span)));
                start = close + 2;
            }
        }
        if let Some(LinePresentation { list, .. }) = &self.presentation {
            self.lists.clear();
            self.lists.extend(list.iter().cloned());
        }
        for list in &mut self.lists {
            // Normalize marker spacing while keeping the source indentation intact.
            while matches!(text.as_bytes().get(list.range.end), Some(b' ' | b'\t')) {
                list.range.end += 1;
            }
            let end = text[list.range.end..]
                .find('\n')
                .map_or(text.len(), |len| list.range.end + len);
            self.markers
                .push((list.range.clone(), Some(list.range.start..end)));
        }
        for span in &self.colors {
            self.markers.retain(|(range, _)| {
                !ranges_touch(range, &span.opening) && !ranges_touch(range, &span.closing)
            });
            let enclosing = Some(span.opening.start..span.closing.end);
            self.markers.push((span.opening.clone(), enclosing.clone()));
            self.markers.push((span.closing.clone(), enclosing));
        }
        self.markers.sort_by_key(|(range, _)| range.start);
    }

    pub(super) fn replacement(&self, range: &Range<usize>) -> Option<&'static str> {
        let list = self.list_at_marker(range)?;
        match list.kind {
            ListKind::Bullet => Some(match list.depth % 3 {
                0 => "• ",
                1 => "◦ ",
                _ => "▪ ",
            }),
            ListKind::Task { checked: false, .. } => Some("☐ "),
            ListKind::Task { checked: true, .. } => Some("☑ "),
            ListKind::Ordered => None,
        }
    }

    pub(super) fn list_at_marker(&self, range: &Range<usize>) -> Option<&ListMarker> {
        let index = self
            .lists
            .binary_search_by_key(&range.start, |list| list.range.start)
            .ok()?;
        self.lists.get(index).filter(|list| list.range == *range)
    }
}

pub(super) fn ranges_touch(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start <= b.end && a.end >= b.start
}
