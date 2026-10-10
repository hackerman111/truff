//! Color, highlight, and callout styling operations via atomic transactions.
//!
//! Enforces:
//! - Protected regions: never break fenced code blocks (` ``` `) or link delimiters (`[[...]]`).
//! - Wrapper replacement: re-styling replaces existing `<span...>` or `<mark...>` wrappers
//!   without infinite tag nesting.
//! - Clean removal: `remove_style` strips recognized styling tags.
//! - Exact undo: all edits produce standard `Transaction`s that undo byte-for-byte.

use std::ops::Range;

use crate::text::Transaction;

/// Supported Callout kinds following Obsidian conventions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalloutKind {
    Note,
    Warning,
    Tip,
    Important,
    Custom(String),
}

impl CalloutKind {
    /// Returns the tag identifier string (e.g. "note", "warning").
    pub fn tag(&self) -> &str {
        match self {
            Self::Note => "note",
            Self::Warning => "warning",
            Self::Tip => "tip",
            Self::Important => "important",
            Self::Custom(s) => s.as_str(),
        }
    }

    /// Parses a tag string into a `CalloutKind`.
    pub fn from_tag(tag: &str) -> Self {
        match tag.to_ascii_lowercase().as_str() {
            "note" => Self::Note,
            "warning" => Self::Warning,
            "tip" => Self::Tip,
            "important" => Self::Important,
            other => Self::Custom(other.to_string()),
        }
    }
}

/// Errors occurring during styling operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleError {
    /// The specified byte range is outside the buffer bounds or inverted.
    InvalidByteRange { range: Range<usize>, len: usize },
    /// The byte range boundaries do not fall on UTF-8 character boundaries.
    NotCharBoundary { pos: usize },
    /// The styling range attempts to break or split a fenced code block boundary.
    IntersectsCodeFence { fence_range: Range<usize> },
    /// The styling range attempts to break or split a link delimiter (`[[` or `]]`).
    IntersectsLinkDelimiter { link_range: Range<usize> },
    /// The provided range is empty.
    EmptyRange,
}

impl std::fmt::Display for StyleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidByteRange { range, len } => {
                write!(
                    f,
                    "invalid styling range {:?} for text length {}",
                    range, len
                )
            }
            Self::NotCharBoundary { pos } => {
                write!(
                    f,
                    "styling offset {} is not a UTF-8 character boundary",
                    pos
                )
            }
            Self::IntersectsCodeFence { fence_range } => {
                write!(
                    f,
                    "styling range intersects code fence at {:?}",
                    fence_range
                )
            }
            Self::IntersectsLinkDelimiter { link_range } => {
                write!(
                    f,
                    "styling range intersects link delimiter at {:?}",
                    link_range
                )
            }
            Self::EmptyRange => write!(f, "styling range cannot be empty"),
        }
    }
}

impl std::error::Error for StyleError {}

/// Applies an inline text color wrapper `<span style="color:#RRGGBB">...</span>`.
///
/// If the target range is already wrapped by an existing color span (or is the inner content of one),
/// the existing wrapper's color is updated without creating nested tags.
pub fn apply_text_color(
    text: &str,
    range: Range<usize>,
    color_hex: &str,
) -> Result<Transaction, StyleError> {
    validate_range(text, &range)?;
    validate_protection(text, &range)?;

    let normalized_color = normalize_hex_color(color_hex);

    // Check if range is inside or encompasses an existing <span style="color:...> wrapper.
    if let Some((outer_range, inner_text)) = find_enclosing_color_span(text, &range) {
        let new_wrapper = format!(
            "<span style=\"color:{}\">{}</span>",
            normalized_color, inner_text
        );
        return Ok(Transaction::replace(outer_range, new_wrapper));
    }

    // Strip any internal color spans within the selected text to prevent nesting.
    let selected_text = &text[range.start..range.end];
    let stripped = strip_internal_color_spans(selected_text);
    let new_wrapper = format!(
        "<span style=\"color:{}\">{}</span>",
        normalized_color, stripped
    );

    Ok(Transaction::replace(range, new_wrapper))
}

/// Applies an inline highlight marker wrapper `<mark style="background-color:#RRGGBB">...</mark>`.
///
/// If the target range is already inside an existing highlight mark, the existing wrapper
/// is updated without creating nested tags.
pub fn apply_highlight(
    text: &str,
    range: Range<usize>,
    color_hex: &str,
) -> Result<Transaction, StyleError> {
    validate_range(text, &range)?;
    validate_protection(text, &range)?;

    let normalized_color = normalize_hex_color(color_hex);

    // Check if range is inside or encompasses an existing <mark style="background-color:...> wrapper.
    if let Some((outer_range, inner_text)) = find_enclosing_highlight_mark(text, &range) {
        let new_wrapper = format!(
            "<mark style=\"background-color:{}\">{}</mark>",
            normalized_color, inner_text
        );
        return Ok(Transaction::replace(outer_range, new_wrapper));
    }

    let selected_text = &text[range.start..range.end];
    let stripped = strip_internal_highlight_marks(selected_text);
    let new_wrapper = format!(
        "<mark style=\"background-color:{}\">{}</mark>",
        normalized_color, stripped
    );

    Ok(Transaction::replace(range, new_wrapper))
}

/// Wraps the specified line or block range into an Obsidian-style callout block.
///
/// Format:
/// ```markdown
/// > [!note]
/// > content...
/// ```
pub fn wrap_callout(
    text: &str,
    range: Range<usize>,
    kind: CalloutKind,
) -> Result<Transaction, StyleError> {
    validate_range(text, &range)?;
    validate_protection(text, &range)?;

    let selected = &text[range.start..range.end];
    let mut out = String::with_capacity(selected.len() + 32);
    out.push_str("> [!");
    out.push_str(kind.tag());
    out.push_str("]\n");

    for line in selected.lines() {
        out.push_str("> ");
        out.push_str(line);
        out.push('\n');
    }

    // If selected did not end with a newline, trim the trailing newline to preserve shape.
    if !selected.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }

    Ok(Transaction::replace(range, out))
}

/// Removes recognized styling tags (`<span>` text colors and `<mark>` highlights)
/// from the specified range or enclosing wrapper.
pub fn remove_style(text: &str, range: Range<usize>) -> Result<Transaction, StyleError> {
    validate_range(text, &range)?;

    // 1. Check if enclosed in color span.
    if let Some((outer_range, inner_text)) = find_enclosing_color_span(text, &range) {
        return Ok(Transaction::replace(outer_range, inner_text));
    }

    // 2. Check if enclosed in highlight mark.
    if let Some((outer_range, inner_text)) = find_enclosing_highlight_mark(text, &range) {
        return Ok(Transaction::replace(outer_range, inner_text));
    }

    // 3. Strip any internal wrappers in the range.
    let selected = &text[range.start..range.end];
    let mut cleaned = strip_internal_color_spans(selected);
    cleaned = strip_internal_highlight_marks(&cleaned);

    if cleaned != selected {
        Ok(Transaction::replace(range, cleaned))
    } else {
        // No styling to remove, empty edit transaction
        Ok(Transaction::new())
    }
}

// ---------------------------------------------------------------------------
// Helpers and scanners
// ---------------------------------------------------------------------------

fn validate_range(text: &str, range: &Range<usize>) -> Result<(), StyleError> {
    if range.start > range.end || range.end > text.len() {
        return Err(StyleError::InvalidByteRange {
            range: range.clone(),
            len: text.len(),
        });
    }
    if range.start == range.end {
        return Err(StyleError::EmptyRange);
    }
    if !text.is_char_boundary(range.start) {
        return Err(StyleError::NotCharBoundary { pos: range.start });
    }
    if !text.is_char_boundary(range.end) {
        return Err(StyleError::NotCharBoundary { pos: range.end });
    }
    Ok(())
}

fn normalize_hex_color(color: &str) -> String {
    let trimmed = color.trim();
    if trimmed.starts_with('#') {
        trimmed.to_string()
    } else {
        format!("#{}", trimmed)
    }
}

/// Ensures the range does not slice a fenced code block fence or a link delimiter.
fn validate_protection(text: &str, range: &Range<usize>) -> Result<(), StyleError> {
    let bytes = text.as_bytes();

    // 1. Scan for fenced code blocks (``` or ~~~)
    let mut in_fence = false;
    let mut fence_char = b'`';
    let mut fence_len = 0;
    let mut fence_start = 0;

    let mut line_start = 0;
    while line_start < bytes.len() {
        // Find line end
        let mut line_end = line_start;
        while line_end < bytes.len() && bytes[line_end] != b'\n' {
            line_end += 1;
        }

        let line = &text[line_start..line_end];
        let trimmed = line.trim_start();
        let leading_spaces = line.len() - trimmed.len();

        if leading_spaces <= 3 {
            if !in_fence {
                if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                    in_fence = true;
                    fence_char = trimmed.as_bytes()[0];
                    fence_len = trimmed
                        .chars()
                        .take_while(|&c| c as u8 == fence_char)
                        .count();
                    fence_start = line_start;
                }
            } else {
                let fence_prefix_bytes = vec![fence_char; fence_len];
                let fence_prefix = std::str::from_utf8(&fence_prefix_bytes).unwrap_or("");
                if !fence_prefix.is_empty() && trimmed.starts_with(fence_prefix) {
                    // Fence closes on this line
                    let fence_end = if line_end < bytes.len() {
                        line_end + 1
                    } else {
                        line_end
                    };
                    let code_block_range = fence_start..fence_end;

                    if range_crosses_boundary(range, fence_start, line_start + trimmed.len()) {
                        return Err(StyleError::IntersectsCodeFence {
                            fence_range: code_block_range,
                        });
                    }

                    if range_crosses_boundary(range, line_start, fence_end) {
                        return Err(StyleError::IntersectsCodeFence {
                            fence_range: code_block_range,
                        });
                    }

                    in_fence = false;
                }
            }
        }

        line_start = if line_end < bytes.len() {
            line_end + 1
        } else {
            line_end
        };
    }

    if in_fence {
        // Unclosed fence to end of file
        let code_block_range = fence_start..bytes.len();
        if range_crosses_boundary(range, fence_start, bytes.len()) {
            return Err(StyleError::IntersectsCodeFence {
                fence_range: code_block_range,
            });
        }
    }

    // 2. Scan for wiki links: [[...]]
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            let open_start = i;
            let open_end = i + 2;
            if let Some(close_rel) = text[open_end..].find("]]") {
                let close_start = open_end + close_rel;
                let close_end = close_start + 2;
                let full_link = open_start..close_end;

                let open_delim = open_start..open_end;
                let close_delim = close_start..close_end;

                let touches_open = ranges_overlap(range, &open_delim);
                let touches_close = ranges_overlap(range, &close_delim);

                if touches_open || touches_close {
                    let encloses_full =
                        range.start <= full_link.start && range.end >= full_link.end;
                    if !encloses_full {
                        let broken_delim = if touches_open {
                            open_delim
                        } else {
                            close_delim
                        };
                        return Err(StyleError::IntersectsLinkDelimiter {
                            link_range: broken_delim,
                        });
                    }
                }

                i = close_end;
                continue;
            } else {
                let open_delim = open_start..open_end;
                if ranges_overlap(range, &open_delim) {
                    return Err(StyleError::IntersectsLinkDelimiter {
                        link_range: open_delim,
                    });
                }
            }
        }
        i += 1;
    }

    Ok(())
}

fn ranges_overlap(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && a.end > b.start
}

fn range_crosses_boundary(
    range: &Range<usize>,
    boundary_start: usize,
    boundary_end: usize,
) -> bool {
    // If range starts before boundary and ends inside boundary, or starts inside and ends after
    (range.start < boundary_start && range.end > boundary_start && range.end <= boundary_end)
        || (range.start >= boundary_start && range.start < boundary_end && range.end > boundary_end)
}

/// Finds if `range` is wrapped by or equals a `<span style="color:...>...</span>`.
fn find_enclosing_color_span(text: &str, range: &Range<usize>) -> Option<(Range<usize>, String)> {
    find_enclosing_tag(text, range, "<span style=\"color:", "</span>")
}

/// Finds if `range` is wrapped by or equals a `<mark style="background-color:...>...</mark>`.
fn find_enclosing_highlight_mark(
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
            let inner = &candidate[tag_open_end + 1..candidate.len() - closing_tag.len()];
            return Some((range.clone(), inner.to_string()));
        }
    }

    // Case 2: range is inside an enclosing tag
    let prefix_pos = text[..range.start].rfind(tag_prefix)?;
    let tag_open_end = text[prefix_pos..].find('>')? + prefix_pos + 1;

    // Check if the closing tag comes after or at range.end
    let closing_pos = text[range.end..].find(closing_tag)? + range.end;
    let outer_end = closing_pos + closing_tag.len();

    // Verify there are no other closing tags between tag_open_end and range.start
    if text[tag_open_end..range.start].contains(closing_tag) {
        return None;
    }

    let inner_text = text[tag_open_end..closing_pos].to_string();
    Some((prefix_pos..outer_end, inner_text))
}

fn strip_internal_color_spans(text: &str) -> String {
    strip_tag_pattern(text, "<span style=\"color:", "</span>")
}

fn strip_internal_highlight_marks(text: &str) -> String {
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
