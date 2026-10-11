use crate::text::Buffer;

use super::cursor::snap_grapheme;

/// Literal UTF-8 search with O(query length) working memory; Rope chunks stay borrowed.
pub fn literal_search(
    buffer: &Buffer,
    pos: usize,
    query: &str,
    forward: bool,
    count: usize,
) -> Option<usize> {
    if query.is_empty() {
        return None;
    }
    let needle = query.as_bytes();
    let mut failure = vec![0; needle.len()];
    let mut matched = 0;
    for i in 1..needle.len() {
        while matched > 0 && needle[i] != needle[matched] {
            matched = failure[matched - 1];
        }
        if needle[i] == needle[matched] {
            matched += 1;
        }
        failure[i] = matched;
    }
    let find = |position: usize| {
        let slice = buffer.slice(0..buffer.byte_len());
        let mut matched = 0;
        let mut offset = 0;
        let mut first = None;
        let mut last = None;
        let mut before = None;
        let mut after = None;
        for chunk in slice.chunks() {
            for &byte in chunk.as_bytes() {
                while matched > 0 && byte != needle[matched] {
                    matched = failure[matched - 1];
                }
                if byte == needle[matched] {
                    matched += 1;
                }
                offset += 1;
                if matched == needle.len() {
                    let start = offset - needle.len();
                    first.get_or_insert(start);
                    last = Some(start);
                    if start < position {
                        before = Some(start);
                    }
                    if start > position && after.is_none() {
                        after = Some(start);
                    }
                    matched = failure[matched - 1];
                }
            }
        }
        if forward {
            after.or(first)
        } else {
            before.or(last)
        }
    };
    let mut current = pos.min(buffer.byte_len());
    for _ in 0..count {
        current = find(current)?;
    }
    Some(snap_grapheme(buffer, current))
}
