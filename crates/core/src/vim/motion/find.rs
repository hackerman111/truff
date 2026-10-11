use crate::text::{Buffer, BytePos};

use super::cursor::{line_content_end, snap_grapheme};

pub(super) fn find_char_on_line(
    buffer: &Buffer,
    pos: usize,
    target: char,
    forward: bool,
    till: bool,
    count: usize,
) -> Option<usize> {
    let line = buffer.line_of_byte(pos);
    let start = buffer.byte_of_line(line);
    let end = line_content_end(buffer, line);
    let slice = buffer.slice(start..end);
    if forward {
        find_in_chars(buffer, slice.chars(), start, pos, target, true, till, count)
    } else {
        find_in_chars(
            buffer,
            slice.chars().rev(),
            end,
            pos,
            target,
            false,
            till,
            count,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn find_in_chars(
    buffer: &Buffer,
    chars: impl Iterator<Item = char>,
    mut offset: usize,
    pos: usize,
    target: char,
    forward: bool,
    till: bool,
    count: usize,
) -> Option<usize> {
    let mut matches = 0;
    for c in chars {
        if !forward {
            offset -= c.len_utf8();
        }
        if ((forward && offset > pos) || (!forward && offset < pos)) && c == target {
            matches += 1;
            if matches == count {
                return Some(if till {
                    if forward {
                        buffer
                            .grapheme_prev(BytePos::new(offset))
                            .map_or(offset, BytePos::get)
                    } else {
                        buffer
                            .grapheme_next(BytePos::new(offset))
                            .map_or(offset, BytePos::get)
                    }
                } else {
                    snap_grapheme(buffer, offset)
                });
            }
        }
        if forward {
            offset += c.len_utf8();
        }
    }
    None
}
