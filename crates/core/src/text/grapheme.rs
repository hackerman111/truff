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

pub(super) fn count(rope: &Rope) -> usize {
    let mut chunks = rope.chunks();
    let mut chunk = chunks.next().unwrap_or("");
    let mut start = 0;
    let mut count = 0;
    let mut cursor = GraphemeCursor::new(0, rope.byte_len(), true);
    loop {
        match cursor.next_boundary(chunk, start) {
            Ok(Some(_)) => count += 1,
            Ok(None) => return count,
            Err(GraphemeIncomplete::NextChunk) => {
                start += chunk.len();
                chunk = chunks
                    .next()
                    .expect("cursor requested an existing next Rope chunk");
            }
            Err(GraphemeIncomplete::PreContext(end)) => context(&mut cursor, rope, end),
            Err(GraphemeIncomplete::PrevChunk | GraphemeIncomplete::InvalidOffset) => {
                unreachable!("forward grapheme iterator stays inside its Rope chunk")
            }
        }
    }
}
