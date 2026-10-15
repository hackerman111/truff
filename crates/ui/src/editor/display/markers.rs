use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use truff_core::markdown::presentation::{LinePresentation, ListKind, ListMarker};
use truff_core::markdown::style::ColorSpan;

use super::DisplayMap;
use super::types::{InlineStyle, StyledSpan};

pub fn highlight_spans(text: &str) -> Vec<ColorSpan> {
    let prefix = "<mark style=\"background-color:#";
    let Some(first) = text.find(prefix) else {
        return Vec::new();
    };
    let mut spans = Vec::new();
    let mut offset = first;
    while let Some(relative) = text[offset..].find(prefix) {
        let start = offset + relative;
        let hex_start = start + prefix.len();
        let hex_end = hex_start + 6;
        offset = hex_start;
        let Some(hex) = text.get(hex_start..hex_end) else {
            continue;
        };
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) || !text[hex_end..].starts_with("\">") {
            continue;
        }
        let content_start = hex_end + 2;
        let Some(close_relative) = text[content_start..].find("</mark>") else {
            break;
        };
        let close_start = content_start + close_relative;
        let end = close_start + "</mark>".len();
        offset = end;
        let channel = |index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap_or(0);
        spans.push(ColorSpan {
            opening: start..content_start,
            content: content_start..close_start,
            closing: close_start..end,
            rgba: [channel(0), channel(2), channel(4), 255],
        });
    }
    spans
}

impl DisplayMap {
    pub(super) fn scan_markers(&mut self) {
        self.markers.clear();
        self.lists.clear();
        self.styles.clear();
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
                Event::Start(Tag::Strong) => {
                    let source = &text[range.clone()];
                    let marker_len = if source.starts_with("**") || source.starts_with("__") {
                        2
                    } else {
                        1
                    };
                    self.markers
                        .push((range.start..range.start + marker_len, Some(range.clone())));
                    self.markers
                        .push((range.end - marker_len..range.end, Some(range.clone())));
                    if range.len() >= marker_len * 2 {
                        self.styles.push(StyledSpan {
                            content: (range.start + marker_len)..(range.end - marker_len),
                            style: InlineStyle::Bold,
                        });
                    }
                }
                Event::Start(Tag::Emphasis) => {
                    let source = &text[range.clone()];
                    let marker_len = if source.starts_with("**") || source.starts_with("__") {
                        2
                    } else {
                        1
                    };
                    self.markers
                        .push((range.start..range.start + marker_len, Some(range.clone())));
                    self.markers
                        .push((range.end - marker_len..range.end, Some(range.clone())));
                    if range.len() >= marker_len * 2 {
                        self.styles.push(StyledSpan {
                            content: (range.start + marker_len)..(range.end - marker_len),
                            style: InlineStyle::Italic,
                        });
                    }
                }
                Event::Start(Tag::Strikethrough) => {
                    let marker_len = 2;
                    self.markers
                        .push((range.start..range.start + marker_len, Some(range.clone())));
                    self.markers
                        .push((range.end - marker_len..range.end, Some(range.clone())));
                    if range.len() >= marker_len * 2 {
                        self.styles.push(StyledSpan {
                            content: (range.start + marker_len)..(range.end - marker_len),
                            style: InlineStyle::Strikethrough,
                        });
                    }
                }
                Event::Code(_) => {
                    let count = text[range.clone()]
                        .bytes()
                        .take_while(|byte| *byte == b'`')
                        .count();
                    self.markers
                        .push((range.start..range.start + count, Some(range.clone())));
                    self.markers
                        .push((range.end - count..range.end, Some(range.clone())));
                    if range.len() >= count * 2 {
                        self.styles.push(StyledSpan {
                            content: (range.start + count)..(range.end - count),
                            style: InlineStyle::Code,
                        });
                    }
                }
                Event::Start(Tag::Link { .. }) => {
                    let source = &text[range.clone()];
                    if source.starts_with('[') {
                        if let Some(close_bracket) = source.find("](") {
                            let open_marker = range.start..range.start + 1;
                            let close_marker = (range.start + close_bracket)..range.end;
                            let content_start = range.start + 1;
                            let content_end = range.start + close_bracket;
                            self.markers.push((open_marker, Some(range.clone())));
                            self.markers.push((close_marker, Some(range.clone())));
                            if content_start < content_end {
                                self.styles.push(StyledSpan {
                                    content: content_start..content_end,
                                    style: InlineStyle::Link,
                                });
                            }
                        } else if let Some(close_bracket) = source.find("][") {
                            let open_marker = range.start..range.start + 1;
                            let close_marker = (range.start + close_bracket)..range.end;
                            let content_start = range.start + 1;
                            let content_end = range.start + close_bracket;
                            self.markers.push((open_marker, Some(range.clone())));
                            self.markers.push((close_marker, Some(range.clone())));
                            if content_start < content_end {
                                self.styles.push(StyledSpan {
                                    content: content_start..content_end,
                                    style: InlineStyle::Link,
                                });
                            }
                        }
                    }
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
                self.styles.push(StyledSpan {
                    content: (open + 2)..close,
                    style: InlineStyle::Link,
                });
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
            self.styles.push(StyledSpan {
                content: span.content.clone(),
                style: InlineStyle::TextColor(span.rgba),
            });
        }
        for span in highlight_spans(text) {
            self.markers.retain(|(range, _)| {
                !ranges_touch(range, &span.opening) && !ranges_touch(range, &span.closing)
            });
            let enclosing = Some(span.opening.start..span.closing.end);
            self.markers.push((span.opening.clone(), enclosing.clone()));
            self.markers.push((span.closing.clone(), enclosing));
            self.styles.push(StyledSpan {
                content: span.content.clone(),
                style: InlineStyle::Highlight(span.rgba),
            });
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
