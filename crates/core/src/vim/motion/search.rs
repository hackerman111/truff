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
    let pos = pos.min(buffer.byte_len());
    if count == 0 {
        return Some(snap_grapheme(buffer, pos));
    }
    let reversed;
    let needle = if forward {
        query.as_bytes()
    } else {
        reversed = query.bytes().rev().collect::<Vec<_>>();
        &reversed
    };
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
    let start = buffer.floor_char_boundary(pos);
    // Include matches that straddle the cursor without matching across EOF.
    let end = buffer.floor_char_boundary(pos.saturating_add(needle.len()));
    let suffix = buffer.slice(start..buffer.byte_len());
    let prefix = buffer.slice(0..end);
    let cycle = |remaining: &mut usize| {
        if forward {
            scan(
                suffix.chunks().flat_map(str::bytes),
                start,
                needle,
                &failure,
                true,
                |m| m > pos,
                remaining,
            )
            .or_else(|| {
                scan(
                    prefix.chunks().flat_map(str::bytes),
                    0,
                    needle,
                    &failure,
                    true,
                    |m| m <= pos,
                    remaining,
                )
            })
        } else {
            scan(
                prefix.chunks().rev().flat_map(|s| s.bytes().rev()),
                end,
                needle,
                &failure,
                false,
                |m| m < pos,
                remaining,
            )
            .or_else(|| {
                scan(
                    suffix.chunks().rev().flat_map(|s| s.bytes().rev()),
                    buffer.byte_len(),
                    needle,
                    &failure,
                    false,
                    |m| m >= pos,
                    remaining,
                )
            })
        }
    };
    let mut remaining = count;
    if let Some(found) = cycle(&mut remaining) {
        return Some(snap_grapheme(buffer, found));
    }
    let total = count - remaining;
    if total == 0 {
        return None;
    }
    // Once a complete cycle is known, arbitrarily large counts need one more
    // bounded traversal, rather than count full scans or a vector of all matches.
    remaining = (remaining - 1) % total + 1;
    cycle(&mut remaining).map(|found| snap_grapheme(buffer, found))
}

#[allow(clippy::too_many_arguments)]
fn scan(
    bytes: impl Iterator<Item = u8>,
    mut offset: usize,
    needle: &[u8],
    failure: &[usize],
    forward: bool,
    accept: impl Fn(usize) -> bool,
    remaining: &mut usize,
) -> Option<usize> {
    let mut matched = 0;
    for byte in bytes {
        while matched > 0 && byte != needle[matched] {
            matched = failure[matched - 1];
        }
        if byte == needle[matched] {
            matched += 1;
        }
        if forward {
            offset += 1;
        } else {
            offset -= 1;
        }
        if matched == needle.len() {
            let start = if forward {
                offset - needle.len()
            } else {
                offset
            };
            if accept(start) {
                *remaining -= 1;
                if *remaining == 0 {
                    return Some(start);
                }
            }
            matched = failure[matched - 1];
        }
    }
    None
}
