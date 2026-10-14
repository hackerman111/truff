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
    let mut opening_end = 0;

    let mut line_start = 0;
    while line_start < bytes.len() {
        if line_start >= range.end
            && (!in_fence || !ranges_overlap(range, &(fence_start..opening_end)))
        {
            in_fence = false;
            break;
        }
        // Find line end
        let mut line_end = line_start;
        while line_end < bytes.len() && bytes[line_end] != b'\n' {
            line_end += 1;
        }

        let line = text[line_start..line_end]
            .strip_suffix('\r')
            .unwrap_or(&text[line_start..line_end]);
        let trimmed = line.trim_start_matches(' ');
        let leading_spaces = line.len() - trimmed.len();

        if leading_spaces <= 3 {
            if !in_fence {
                if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                    in_fence = true;
                    fence_char = trimmed.as_bytes()[0];
                    fence_len = trimmed.bytes().take_while(|&c| c == fence_char).count();
                    if fence_char == b'`' && trimmed[fence_len..].contains('`') {
                        in_fence = false;
                    }
                    fence_start = line_start;
                    opening_end = (line_end + 1).min(bytes.len());
                }
            } else {
                let run = trimmed.bytes().take_while(|&c| c == fence_char).count();
                if run >= fence_len && trimmed[run..].bytes().all(|c| matches!(c, b' ' | b'\t')) {
                    // Fence closes on this line
                    let fence_end = if line_end < bytes.len() {
                        line_end + 1
                    } else {
                        line_end
                    };
                    let code_block_range = fence_start..fence_end;

                    let encloses_block = range.start <= fence_start && range.end >= fence_end;
                    if !encloses_block
                        && (ranges_overlap(range, &(fence_start..opening_end))
                            || ranges_overlap(range, &(line_start..fence_end)))
                    {
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
        if ranges_overlap(range, &(fence_start..opening_end))
            && !(range.start <= fence_start && range.end == bytes.len())
        {
            return Err(StyleError::IntersectsCodeFence {
                fence_range: code_block_range,
            });
        }
    }

    // 2. Scan for wiki links: [[...]]
    let mut i = 0;
    let mut no_closing = false;
    while i + 1 < bytes.len() && i < range.end {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            let open_start = i;
            let open_end = i + 2;
            let closing = (!no_closing).then(|| text[open_end..].find("]]")).flatten();
            if let Some(close_rel) = closing {
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
                no_closing = true;
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
