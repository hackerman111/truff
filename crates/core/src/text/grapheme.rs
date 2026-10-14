//! UAX #29 traversal over borrowed Rope chunks, including clusters spanning leaves.
use crop::Rope;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

fn context(cursor: &mut GraphemeCursor, rope: &Rope, end: usize) {
    let chunk = rope.byte_slice(..end).chunks().next_back().unwrap_or("");
    cursor.provide_context(chunk, end - chunk.len());
}

pub(super) fn next(rope: &Rope, pos: usize) -> Option<usize> {
    if pos >= rope.byte_len() || !rope.is_char_boundary(pos) {
        return None;
    }
    boundary(rope, pos, true)
}

pub(super) fn prev(rope: &Rope, pos: usize) -> Option<usize> {
    let pos = pos.min(rope.byte_len());
    if pos == 0 || !rope.is_char_boundary(pos) {
        return None;
    }
    boundary(rope, pos, false)
}

fn boundary(rope: &Rope, pos: usize, forward: bool) -> Option<usize> {
    let mut cursor = GraphemeCursor::new(pos, rope.byte_len(), true);
    let mut start = pos;
    let mut chunk = "";
    if forward {
        chunk = rope.byte_slice(pos..).chunks().next().unwrap_or("");
    } else if let Some(before) = rope.byte_slice(..pos).chunks().next_back() {
        chunk = before;
        start -= before.len();
    }
    loop {
        let result = if forward {
            cursor.next_boundary(chunk, start)
        } else {
            cursor.prev_boundary(chunk, start)
        };
        match result {
            Ok(boundary) => return boundary,
            Err(GraphemeIncomplete::PreContext(end)) => context(&mut cursor, rope, end),
            Err(GraphemeIncomplete::NextChunk) => {
                start += chunk.len();
                chunk = rope.byte_slice(start..).chunks().next().unwrap_or("");
            }
            Err(GraphemeIncomplete::PrevChunk) => {
                chunk = rope.byte_slice(..start).chunks().next_back().unwrap_or("");
                start -= chunk.len();
            }
            Err(GraphemeIncomplete::InvalidOffset) => {
                unreachable!("borrowed Rope chunk contains the grapheme cursor")
            }
        }
    }
}

pub(super) fn boundaries(rope: &Rope, pos: usize) -> impl Iterator<Item = usize> + '_ {
    let valid = pos <= rope.byte_len() && rope.is_char_boundary(pos);
    let mut start = if valid { pos } else { rope.byte_len() };
    let mut chunks = rope.byte_slice(start..).chunks();
    let mut chunk = chunks.next().unwrap_or("");
    let mut cursor = GraphemeCursor::new(start, rope.byte_len(), true);
    let mut check_join = false;
    std::iter::from_fn(move || {
        loop {
            if check_join {
                match cursor.is_boundary(chunk, start) {
                    Ok(boundary) => {
                        check_join = false;
                        if boundary {
                            return Some(start);
                        }
                    }
                    Err(GraphemeIncomplete::PreContext(end)) => {
                        context(&mut cursor, rope, end);
                        continue;
                    }
                    _ => unreachable!("join lies at the beginning of a nonempty Rope chunk"),
                }
            }
            match cursor.next_boundary(chunk, start) {
                Ok(boundary) => return boundary,
                Err(GraphemeIncomplete::NextChunk) => {
                    start += chunk.len();
                    chunk = chunks
                        .next()
                        .expect("cursor requested an existing next Rope chunk");
                    // Recompute the join context: carrying regional-indicator state
                    // into provide_context would count the preceding indicator twice.
                    cursor = GraphemeCursor::new(start, rope.byte_len(), true);
                    check_join = true;
                }
                Err(GraphemeIncomplete::PreContext(end)) => context(&mut cursor, rope, end),
                Err(GraphemeIncomplete::PrevChunk | GraphemeIncomplete::InvalidOffset) => {
                    unreachable!("forward grapheme iterator stays inside its Rope chunk")
                }
            }
        }
    })
}

pub(super) fn count(rope: &Rope) -> usize {
    boundaries(rope, 0).count()
}
