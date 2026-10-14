use std::ops::Range;

/// Finds if `range` is wrapped by or equals a `<span style="color:...>...</span>`.
pub(super) fn find_enclosing_color_span(
    text: &str,
    range: &Range<usize>,
) -> Option<(Range<usize>, String)> {
    find_enclosing_tag(text, range, "<span style=\"color:", "</span>")
}

/// Finds if `range` is wrapped by or equals a `<mark style="background-color:...>...</mark>`.
pub(super) fn find_enclosing_highlight_mark(
    text: &str,
    range: &Range<usize>,
) -> Option<(Range<usize>, String)> {
    find_enclosing_tag(text, range, "<mark style=\"background-color:", "</mark>")
}

fn find_enclosing_tag(
    text: &str,
    range: &Range<usize>,
    tag_prefix: &str,
    closing_tag: &str,
) -> Option<(Range<usize>, String)> {
    // Case 1: range itself starts with tag_prefix and ends with closing_tag
    if range.end <= text.len() && range.start < range.end {
        let candidate = &text[range.start..range.end];
        if candidate.starts_with(tag_prefix)
            && candidate.ends_with(closing_tag)
            && let Some(tag_open_end) = candidate.find('>')
        {
            let inner_end = candidate.len() - closing_tag.len();
            if let Some(inner) = candidate.get(tag_open_end + 1..inner_end)
                && !inner.contains(closing_tag)
            {
                return Some((range.clone(), inner.to_string()));
            }
        }
    }

    // Case 2: range is inside an enclosing tag
    let prefix_pos = text[..range.start].rfind(tag_prefix)?;
    let tag_open_end = text[prefix_pos..].find('>')? + prefix_pos + 1;

    // Check if the closing tag comes after or at range.end
    if tag_open_end > range.start {
        return None;
    }
    let closing_pos = text[range.end..].find(closing_tag)? + range.end;
    let outer_end = closing_pos + closing_tag.len();

    // Reject spans whose content crosses another closing tag before the match.
    if text[tag_open_end..closing_pos].contains(closing_tag) {
        return None;
    }

    let inner_text = text[tag_open_end..closing_pos].to_string();
    Some((prefix_pos..outer_end, inner_text))
}

pub(super) fn strip_internal_color_spans(text: &str) -> String {
    strip_tag_pattern(text, "<span style=\"color:", "</span>")
}

pub(super) fn strip_internal_highlight_marks(text: &str) -> String {
    strip_tag_pattern(text, "<mark style=\"background-color:", "</mark>")
}

fn strip_tag_pattern(text: &str, tag_prefix: &str, closing_tag: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut cur = text;

    while let Some(open_idx) = cur.find(tag_prefix) {
        result.push_str(&cur[..open_idx]);
        let rem = &cur[open_idx..];
        if let Some(tag_close_idx) = rem.find('>') {
            let inner_start = tag_close_idx + 1;
            let after_open = &rem[inner_start..];
            if let Some(close_idx) = after_open.find(closing_tag) {
                result.push_str(&after_open[..close_idx]);
                cur = &after_open[close_idx + closing_tag.len()..];
            } else {
                result.push_str(rem);
                return result;
            }
        } else {
            result.push_str(rem);
            return result;
        }
    }
    result.push_str(cur);
    result
}

/// A recognized color wrapper emitted by `apply_text_color`, with source byte ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorSpan {
    pub opening: Range<usize>,
    pub content: Range<usize>,
    pub closing: Range<usize>,
    pub rgba: [u8; 4],
}

/// Extract supported color wrappers outside fenced and inline code.
pub fn color_spans(text: &str) -> Vec<ColorSpan> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let prefix = "<span style=\"color:#";
    let Some(first) = text.find(prefix) else {
        return Vec::new();
    };
    let mut code = Vec::new();
    let mut fence_start = None;
    for (event, range) in Parser::new(text).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_)) => fence_start = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = fence_start.take() {
                    code.push(start..range.end);
                }
            }
            Event::Code(_) => code.push(range),
            _ => {}
        }
    }
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
        let Some(close_relative) = text[content_start..].find("</span>") else {
            break;
        };
        let close_start = content_start + close_relative;
        let end = close_start + 7;
        offset = end;
        let index = code.partition_point(|range| range.end <= start);
        if code.get(index).is_some_and(|range| range.start < end) {
            continue;
        }
        let channel =
            |index| u8::from_str_radix(&hex[index..index + 2], 16).expect("validated hex pair");
        spans.push(ColorSpan {
            opening: start..content_start,
            content: content_start..close_start,
            closing: close_start..end,
            rgba: [channel(0), channel(2), channel(4), 255],
        });
    }
    spans
}
