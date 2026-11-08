mod replacement;

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use truff_core::markdown::presentation::{LinePresentation, ListKind, ListMarker};
use truff_core::markdown::style::ColorSpan;

use super::DisplayMap;
use super::types::{InlineStyle, StyledSpan};

#[inline]
fn has_markdown_syntax(text: &str) -> bool {
    text.bytes().any(|b| {
        matches!(
            b,
            b'*' | b'_'
                | b'~'
                | b'`'
                | b'['
                | b'#'
                | b'-'
                | b'+'
                | b'='
                | b'$'
                | b'<'
                | b'>'
                | b'|'
                | b'.'
                | b')'
        )
    })
}

fn collect_wrapped_list_markers(text: &str, spans: &[ColorSpan], lists: &mut Vec<ListMarker>) {
    for span in spans {
        let content_str = &text[span.content.clone()];
        let ws_inner = content_str
            .bytes()
            .take_while(|b| matches!(*b, b' ' | b'\t'))
            .count();
        let after_ws = &content_str[ws_inner..];
        if after_ws.starts_with("- ")
            || after_ws.starts_with("* ")
            || after_ws.starts_with("+ ")
            || (after_ws.len() >= 2
                && after_ws.as_bytes()[0].is_ascii_digit()
                && (after_ws.as_bytes()[1] == b'.' || after_ws.as_bytes()[1] == b')'))
        {
            let line_start = text[..span.opening.start].rfind('\n').map_or(0, |i| i + 1);
            let ws_indent = (span.opening.start - line_start) + ws_inner;
            let depth = ws_indent / 2;
            if let Some(marker) = truff_core::markdown::presentation::parse_line_list_marker(
                &text[line_start..],
                line_start,
                depth,
            ) && !lists.iter().any(|m| m.range == marker.range)
            {
                lists.push(marker);
            }
        }
    }
}

fn apply_html_style_spans(
    spans: &[ColorSpan],
    markers: &mut Vec<(Range<usize>, Option<Range<usize>>)>,
    styles: &mut Vec<StyledSpan>,
    mk_style: fn([u8; 4]) -> InlineStyle,
) {
    for span in spans {
        markers.retain(|(range, _)| {
            !ranges_intersect(range, &span.opening) && !ranges_intersect(range, &span.closing)
        });
        let enclosing = Some(span.opening.start..span.closing.end);
        markers.push((span.opening.clone(), enclosing.clone()));
        markers.push((span.closing.clone(), enclosing));
        styles.push(StyledSpan {
            content: span.content.clone(),
            style: mk_style(span.rgba),
        });
    }
}

impl DisplayMap {
    pub(super) fn scan_markers(&mut self) {
        self.markers.clear();
        self.lists.clear();
        self.styles.clear();
        self.table_paddings.clear();
        self.table_pipes.clear();
        if self.presentation.as_ref().is_some_and(|line| line.code) {
            self.colors.clear();
            self.highlights.clear();
            return;
        }
        if self
            .presentation
            .as_ref()
            .is_some_and(|line| line.task_fence)
        {
            self.colors.clear();
            self.highlights.clear();
            if !self.source_text.is_empty() {
                let len = self.source_text.len();
                self.markers.push((0..len, Some(0..len)));
            }
            return;
        }
        let text = &self.source_text;
        let has_list_presentation = self
            .presentation
            .as_ref()
            .is_some_and(|line| line.list.is_some());
        if !has_list_presentation
            && self.colors.is_empty()
            && self.highlights.is_empty()
            && !has_markdown_syntax(text)
        {
            return;
        }
        let indent = if has_list_presentation {
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
                    if source.starts_with('[')
                        && let Some(close_bracket) = source.find("](").or_else(|| source.find("]["))
                    {
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
                    if let Some(previous) = text_ranges.last_mut().filter(|previous| {
                        previous.end == range.start
                            || (previous.end + 1 == range.start
                                && text.as_bytes().get(previous.end) == Some(&b'\\'))
                    }) {
                        previous.end = range.end;
                    } else {
                        text_ranges.push(range);
                    }
                }
                Event::Rule => {
                    self.markers.push((range.clone(), Some(range)));
                }
                _ => {}
            }
        }
        let trimmed_start = text.trim_start();
        if trimmed_start.starts_with('>') {
            let ws_before = text.len() - trimmed_start.len();
            let gt_count = trimmed_start.bytes().take_while(|b| *b == b'>').count();
            let after_gt = &trimmed_start[gt_count..];
            let ws_after = usize::from(after_gt.starts_with(' '));
            let marker_len = ws_before + gt_count + ws_after;
            self.markers.push((0..marker_len, None));
        }
        for range in &text_ranges {
            let mut start = range.start;
            while let Some(open) = text[start..range.end].find("[[") {
                let open = start + open;
                let Some(close) = text[open + 2..range.end].find("]]") else {
                    break;
                };
                let close = open + 2 + close;
                let span = open..close + 2;
                let inner = &text[open + 2..close];
                if let Some(pipe_idx) = inner.find('|') {
                    let alias_start = open + 2 + pipe_idx + 1;
                    if alias_start < close && !text[alias_start..close].trim().is_empty() {
                        self.markers.push((open..alias_start, Some(span.clone())));
                        self.markers.push((close..close + 2, Some(span)));
                        self.styles.push(StyledSpan {
                            content: alias_start..close,
                            style: InlineStyle::Link,
                        });
                        start = close + 2;
                        continue;
                    }
                }
                self.markers.push((open..open + 2, Some(span.clone())));
                self.markers.push((close..close + 2, Some(span)));
                self.styles.push(StyledSpan {
                    content: (open + 2)..close,
                    style: InlineStyle::Link,
                });
                start = close + 2;
            }
        }
        let arrow_candidates = ["<-->", "-->", "<--", "<->", "->", "<-"];
        for range in text_ranges {
            let mut start = range.start;
            while start < range.end {
                let slice = &text[start..range.end];
                let mut earliest: Option<(usize, &'static str)> = None;
                for &cand in &arrow_candidates {
                    if let Some(pos) = slice.find(cand) {
                        match earliest {
                            Some((best_pos, best_cand)) => {
                                if pos < best_pos
                                    || (pos == best_pos && cand.len() > best_cand.len())
                                {
                                    earliest = Some((pos, cand));
                                }
                            }
                            None => {
                                earliest = Some((pos, cand));
                            }
                        }
                    }
                }
                if let Some((pos, cand)) = earliest {
                    let idx = start + pos;
                    self.markers
                        .push((idx..idx + cand.len(), Some(idx..idx + cand.len())));
                    start = idx + cand.len();
                } else {
                    break;
                }
            }
        }
        // Obsidian ==highlight== syntax
        let mut h_start = 0;
        while let Some(open) = text[h_start..].find("==") {
            let open = h_start + open;
            if let Some(close) = text[open + 2..].find("==") {
                let close = open + 2 + close;
                let span = open..close + 2;
                self.markers.push((open..open + 2, Some(span.clone())));
                self.markers.push((close..close + 2, Some(span)));
                self.styles.push(StyledSpan {
                    content: (open + 2)..close,
                    style: InlineStyle::Highlight([235, 203, 139, 255]),
                });
                h_start = close + 2;
            } else {
                break;
            }
        }
        // LaTeX block math $$ ... $$
        let mut m_start = 0;
        while let Some(open) = text[m_start..].find("$$") {
            let open = m_start + open;
            if let Some(close) = text[open + 2..].find("$$") {
                let close = open + 2 + close;
                let span = open..close + 2;
                self.markers.push((span.clone(), Some(span.clone())));
                self.styles.push(StyledSpan {
                    content: span.clone(),
                    style: InlineStyle::Math,
                });
                m_start = close + 2;
            } else {
                break;
            }
        }
        // LaTeX inline math $ ... $
        let mut im_start = 0;
        while let Some(open) = text[im_start..].find('$') {
            let open = im_start + open;
            if text[open..].starts_with("$$") {
                im_start = open + 2;
                continue;
            }
            if let Some(close) = text[open + 1..].find('$') {
                let close = open + 1 + close;
                if !text[close..].starts_with("$$") && close > open + 1 {
                    let span = open..close + 1;
                    self.markers.push((span.clone(), Some(span.clone())));
                    self.styles.push(StyledSpan {
                        content: span.clone(),
                        style: InlineStyle::Math,
                    });
                    im_start = close + 1;
                    continue;
                }
            }
            im_start = open + 1;
        }
        // Table row styling
        let trimmed_text = text.trim();
        let is_del_from_pres = self
            .presentation
            .as_ref()
            .is_some_and(|p| p.table_delimiter);
        if is_del_from_pres
            || (trimmed_text.starts_with('|')
                && trimmed_text.ends_with('|')
                && trimmed_text.len() > 1)
        {
            let is_delimiter = is_del_from_pres
                || (trimmed_text
                    .chars()
                    .all(|c| c == '|' || c == '-' || c == ':' || c == ' ')
                    && trimmed_text.contains('-'));
            if is_delimiter {
                self.markers.push((0..text.len(), Some(0..text.len())));
                self.styles.push(StyledSpan {
                    content: 0..text.len(),
                    style: InlineStyle::TextColor([76, 86, 106, 200]),
                });
            } else {
                let is_header = self.presentation.as_ref().is_some_and(|p| p.table_header);
                let mut cell_start = None;
                let pipe_indices = truff_core::markdown::table::find_table_pipe_indices(text);
                for &idx in &pipe_indices {
                    let pipe_range = idx..idx + 1;
                    self.styles.push(StyledSpan {
                        content: pipe_range.clone(),
                        style: InlineStyle::TextColor([94, 129, 172, 220]),
                    });
                    self.markers
                        .push((pipe_range.clone(), Some(pipe_range.clone())));
                    self.table_pipes.push(pipe_range);
                    if is_header {
                        if let Some(start) = cell_start
                            && start < idx
                        {
                            self.styles.push(StyledSpan {
                                content: start..idx,
                                style: InlineStyle::Bold,
                            });
                        }
                        cell_start = Some(idx + 1);
                    }
                }
                if let Some(col_widths) = self
                    .presentation
                    .as_ref()
                    .and_then(|p| p.table_col_widths.as_deref())
                    && pipe_indices.len() >= 2
                {
                    for col in 0..pipe_indices.len() - 1 {
                        let open_pipe = pipe_indices[col];
                        let close_pipe = pipe_indices[col + 1];
                        let cell_range = open_pipe + 1..close_pipe;
                        let visual_graphemes = self.visual_cell_graphemes(cell_range);
                        if let Some(&target_w) = col_widths.get(col)
                            && target_w > visual_graphemes
                        {
                            let padding = target_w - visual_graphemes;
                            let pad_range = close_pipe..close_pipe;
                            self.markers
                                .push((pad_range.clone(), Some(pad_range.clone())));
                            self.table_paddings.push((pad_range, " ".repeat(padding)));
                        }
                    }
                }
            }
        }
        collect_wrapped_list_markers(text, &self.colors, &mut self.lists);
        collect_wrapped_list_markers(text, &self.highlights, &mut self.lists);
        self.lists.sort_by_key(|m| m.range.start);
        if let Some(LinePresentation { list, .. }) = &self.presentation {
            self.lists.clear();
            self.lists.extend(list.iter().cloned());
        }
        let has_presentation = self.presentation.is_some();
        let ws_len = text
            .bytes()
            .take_while(|b| *b == b' ' || *b == b'\t')
            .count();
        for list in &mut self.lists {
            if list.range.start >= text.len() {
                continue;
            }
            list.range.end = list.range.end.min(text.len());
            // Normalize marker spacing while keeping the source indentation intact.
            while list.range.start < list.range.end
                && matches!(text.as_bytes().get(list.range.start), Some(b' ' | b'\t'))
            {
                list.range.start += 1;
            }
            while list.range.end < text.len()
                && matches!(text.as_bytes().get(list.range.end), Some(b' ' | b'\t'))
            {
                list.range.end += 1;
            }
            let end = text[list.range.end..]
                .find('\n')
                .map_or(text.len(), |len| list.range.end + len);
            if has_presentation && ws_len > 0 && ws_len <= list.range.start {
                self.markers.push((0..ws_len, None));
            }
            self.markers
                .push((list.range.clone(), Some(list.range.start..end)));
            if let ListKind::Task { checked, checkbox } = list.kind {
                let status_byte = text.as_bytes().get(checkbox).copied().unwrap_or(b' ');
                let is_in_progress = !checked && matches!(status_byte, b'/' | b'-');
                if checked {
                    self.styles.push(StyledSpan {
                        content: list.range.start..end,
                        style: InlineStyle::TextColor([163, 190, 140, 235]),
                    });
                    if list.range.end < end {
                        self.styles.push(StyledSpan {
                            content: list.range.end..end,
                            style: InlineStyle::Strikethrough,
                        });
                    }
                } else if is_in_progress {
                    self.styles.push(StyledSpan {
                        content: list.range.start..end,
                        style: InlineStyle::TextColor([235, 203, 139, 255]),
                    });
                } else {
                    self.styles.push(StyledSpan {
                        content: list.range.start..end,
                        style: InlineStyle::TextColor([191, 97, 106, 255]),
                    });
                }
            }
        }
        apply_html_style_spans(
            &self.colors,
            &mut self.markers,
            &mut self.styles,
            InlineStyle::TextColor,
        );
        apply_html_style_spans(
            &self.highlights,
            &mut self.markers,
            &mut self.styles,
            InlineStyle::Highlight,
        );
        self.markers
            .sort_by(|(a, _), (b, _)| a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end)));
    }
}

pub(in crate::editor::display) fn ranges_intersect(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && a.end > b.start
}

pub(in crate::editor::display) fn ranges_touch(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start <= b.end && a.end >= b.start
}
