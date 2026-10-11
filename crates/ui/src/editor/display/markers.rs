use std::ops::Range;

use super::DisplayMap;

impl DisplayMap {
    pub(super) fn scan_markers(&mut self) {
        self.markers = scan_markdown_markers(&self.source_text);
        // Marker parsing belongs to source changes, never to caret movement or scrolling.
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
}

pub(super) fn ranges_touch(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start <= b.end && a.end >= b.start
}

/// Scans source text for standard Markdown syntax markers:
/// - Bold: `**`
/// - Italic: `*`
/// - Strikethrough: `~~`
/// - Inline code: `` ` ``
/// - Headings: `# `, `## `, `### `
/// - Wiki links: `[[` and `]]`
fn scan_markdown_markers(text: &str) -> Vec<(Range<usize>, Option<Range<usize>>)> {
    let mut markers = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();

    // 1. Headings at start of lines: e.g. `# `, `## `, etc.
    let mut line_start = 0;
    while line_start < len {
        let mut line_end = line_start;
        while line_end < len && bytes[line_end] != b'\n' {
            line_end += 1;
        }

        let line = &text[line_start..line_end];
        let trimmed = line.trim_start();
        let leading_spaces = line.len() - trimmed.len();

        if trimmed.starts_with('#') {
            let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
            if hash_count <= 6 {
                let rest = &trimmed[hash_count..];
                if rest.starts_with(' ') {
                    let marker_start = line_start + leading_spaces;
                    let marker_end = marker_start + hash_count + 1; // hashes + space
                    markers.push((marker_start..marker_end, Some(line_start..line_end)));
                }
            }
        }

        line_start = if line_end < len {
            line_end + 1
        } else {
            line_end
        };
    }

    // 2. Bold `**...**` and Strikethrough `~~...~~`
    scan_delimiter_pairs(text, "**", &mut markers);
    scan_delimiter_pairs(text, "~~", &mut markers);

    // 3. Wiki links `[[...]]`
    let mut i = 0;
    while i + 1 < len {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            let open_start = i;
            let open_end = i + 2;
            if let Some(close_rel) = text[open_end..].find("]]") {
                let close_start = open_end + close_rel;
                let close_end = close_start + 2;
                let span = open_start..close_end;

                markers.push((open_start..open_end, Some(span.clone())));
                markers.push((close_start..close_end, Some(span)));
                i = close_end;
                continue;
            }
        }
        i += 1;
    }

    // 4. Single `*` italic (outside of `**`)
    scan_single_italic(text, &mut markers);

    markers
}

fn scan_delimiter_pairs(
    text: &str,
    delim: &str,
    markers: &mut Vec<(Range<usize>, Option<Range<usize>>)>,
) {
    let mut search_from = 0;
    while let Some(open_rel) = text[search_from..].find(delim) {
        let open_start = search_from + open_rel;
        let open_end = open_start + delim.len();

        if let Some(close_rel) = text[open_end..].find(delim) {
            let close_start = open_end + close_rel;
            let close_end = close_start + delim.len();
            let span = open_start..close_end;

            markers.push((open_start..open_end, Some(span.clone())));
            markers.push((close_start..close_end, Some(span)));
            search_from = close_end;
        } else {
            break;
        }
    }
}

fn scan_single_italic(text: &str, markers: &mut Vec<(Range<usize>, Option<Range<usize>>)>) {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut open_idx = None;

    while i < bytes.len() {
        if bytes[i] == b'*' {
            let prev_is_star = i > 0 && bytes[i - 1] == b'*';
            let next_is_star = i + 1 < bytes.len() && bytes[i + 1] == b'*';

            if !prev_is_star && !next_is_star {
                if let Some(open_start) = open_idx.take() {
                    let close_start = i;
                    let span = open_start..close_start + 1;
                    markers.push((open_start..open_start + 1, Some(span.clone())));
                    markers.push((close_start..close_start + 1, Some(span)));
                } else {
                    open_idx = Some(i);
                }
            }
        }
        i += 1;
    }
}
