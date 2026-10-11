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

mod protection;
mod tags;

use self::protection::{validate_protection, validate_range};
use self::tags::{
    find_enclosing_color_span, find_enclosing_highlight_mark, strip_internal_color_spans,
    strip_internal_highlight_marks,
};

pub use self::tags::{ColorSpan, color_spans};

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

fn normalize_hex_color(color: &str) -> String {
    let trimmed = color.trim();
    if trimmed.starts_with('#') {
        trimmed.to_string()
    } else {
        format!("#{}", trimmed)
    }
}
