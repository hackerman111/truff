use std::ops::Range;

use super::StyleError;

pub(super) fn validate_range(text: &str, range: &Range<usize>) -> Result<(), StyleError> {
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

/// Ensures the range does not slice a fenced code block fence or a link delimiter.
pub(super) fn validate_protection(text: &str, range: &Range<usize>) -> Result<(), StyleError> {
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
