//! Zero-allocation rope heading extraction, incremental edit rebasing, and heading fold state.

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use truff_core::Buffer;
use truff_core::markdown::HeadingItem;
use truff_core::text::BufferId;

use super::EditorWidget;

pub(super) struct HeadingCache {
    pub(super) buffer_id: BufferId,
    pub(super) revision: u64,
    pub(super) headings: Arc<[HeadingItem]>,
    code_ranges: Vec<Range<usize>>,
}

#[inline]
fn line_fence_prefix(line: crop::RopeSlice<'_>) -> Option<(u8, usize, bool)> {
    let mut bytes = line.bytes();
    let mut spaces = 0usize;
    let first = loop {
        match bytes.next() {
            Some(b' ') if spaces < 4 => spaces += 1,
            Some(b) => break b,
            None => return None,
        }
    };
    if spaces > 3 || !matches!(first, b'`' | b'~') {
        return None;
    }
    let mut run = 1usize;
    let mut trailing_only_ws = true;
    let mut has_backtick_in_rest = false;
    let mut in_run = true;
    for b in bytes {
        if matches!(b, b'\r' | b'\n') {
            break;
        }
        if in_run && b == first {
            run += 1;
            continue;
        }
        in_run = false;
        if !matches!(b, b' ' | b'\t') {
            trailing_only_ws = false;
        }
        if b == b'`' {
            has_backtick_in_rest = true;
        }
    }
    if run < 3 || (first == b'`' && has_backtick_in_rest) {
        return None;
    }
    Some((first, run, trailing_only_ws))
}

#[inline]
fn atx_heading_level(line: crop::RopeSlice<'_>) -> Option<u8> {
    let mut bytes = line.bytes();
    let mut spaces = 0usize;
    let first = loop {
        match bytes.next() {
            Some(b' ') if spaces < 4 => spaces += 1,
            Some(b) => break b,
            None => return None,
        }
    };
    if spaces > 3 || first != b'#' {
        return None;
    }
    let mut hashes = 1u8;
    loop {
        match bytes.next() {
            Some(b'#') => {
                hashes += 1;
                if hashes > 6 {
                    return None;
                }
            }
            None | Some(b' ' | b'\t' | b'\r' | b'\n') => return Some(hashes),
            Some(_) => return None,
        }
    }
}

impl HeadingCache {
    pub(super) fn scan(buffer: &Buffer) -> Self {
        let rope = buffer.rope();
        let total_lines = buffer.line_len();
        let mut headings = Vec::new();
        let mut code_ranges = Vec::new();
        let mut open_fence: Option<(u8, usize, usize)> = None;

        for (line_idx, line) in rope.lines().enumerate() {
            if let Some((f_char, f_len, start_line)) = open_fence {
                if let Some((c, run, trailing_only_ws)) = line_fence_prefix(line)
                    && c == f_char
                    && run >= f_len
                    && trailing_only_ws
                {
                    code_ranges.push(start_line..(line_idx + 1));
                    open_fence = None;
                }
                continue;
            }
            if let Some((c, run, _)) = line_fence_prefix(line) {
                open_fence = Some((c, run, line_idx));
                continue;
            }
            if let Some(level) = atx_heading_level(line) {
                headings.push(HeadingItem::new(line_idx, level));
            }
        }
        if let Some((_, _, start_line)) = open_fence {
            code_ranges.push(start_line..total_lines);
        }

        Self {
            buffer_id: buffer.id(),
            revision: buffer.revision(),
            headings: headings.into(),
            code_ranges,
        }
    }

    /// Incrementally updates cached headings for an edit replacing `first..=old_last` with
    /// `first..=new_last`. Returns `(kept_valid, headings_changed)`.
    pub(super) fn apply_change(
        &mut self,
        buffer: &Buffer,
        first: usize,
        old_last: usize,
        new_last: usize,
        line_delta: isize,
    ) -> (bool, bool) {
        if self.buffer_id != buffer.id() {
            return (false, false);
        }

        // If the edit touched any existing fence opener/closer line, fall back to a full rope scan.
        for r in &self.code_ranges {
            let closer = r.end.saturating_sub(1);
            if (r.start >= first && r.start <= old_last)
                || (r.end > r.start && closer >= first && closer <= old_last)
            {
                return (false, true);
            }
        }

        let rope = buffer.rope();
        let total_lines = buffer.line_len();
        for l in first..=new_last.min(total_lines.saturating_sub(1)) {
            let s = buffer.byte_of_line(l);
            let e = if l + 1 < total_lines {
                buffer.byte_of_line(l + 1)
            } else {
                buffer.byte_len()
            };
            if line_fence_prefix(rope.byte_slice(s..e)).is_some() {
                return (false, true);
            }
        }

        let mut inside_code = false;
        for r in &mut self.code_ranges {
            if old_last < r.start {
                r.start = r.start.saturating_add_signed(line_delta);
                r.end = r.end.saturating_add_signed(line_delta);
            } else if first > r.start && old_last + 1 < r.end {
                r.end = r.end.saturating_add_signed(line_delta);
                inside_code = true;
            }
        }

        let mut local_headings: [HeadingItem; 4] = [HeadingItem::new(0, 1); 4];
        let mut local_len = 0usize;
        let mut overflow_headings: Option<Vec<HeadingItem>> = None;

        if !inside_code {
            for l in first..=new_last.min(total_lines.saturating_sub(1)) {
                let s = buffer.byte_of_line(l);
                let e = if l + 1 < total_lines {
                    buffer.byte_of_line(l + 1)
                } else {
                    buffer.byte_len()
                };
                if let Some(level) = atx_heading_level(rope.byte_slice(s..e)) {
                    let item = HeadingItem::new(l, level);
                    if let Some(vec) = &mut overflow_headings {
                        vec.push(item);
                    } else if local_len < local_headings.len() {
                        local_headings[local_len] = item;
                        local_len += 1;
                    } else {
                        let mut vec = Vec::with_capacity(8);
                        vec.extend_from_slice(&local_headings);
                        vec.push(item);
                        overflow_headings = Some(vec);
                    }
                }
            }
        }

        let new_slice: &[HeadingItem] = match &overflow_headings {
            Some(vec) => vec.as_slice(),
            None => &local_headings[..local_len],
        };

        let lo = self.headings.partition_point(|h| h.line < first);
        let hi = lo + self.headings[lo..].partition_point(|h| h.line <= old_last);

        // Zero-allocation fast path: single-line / same-line-count edit with unchanged headings!
        if line_delta == 0 && &self.headings[lo..hi] == new_slice {
            self.revision = buffer.revision();
            return (true, false);
        }

        let mut updated = Vec::with_capacity(self.headings.len() - (hi - lo) + new_slice.len());
        updated.extend_from_slice(&self.headings[..lo]);
        updated.extend_from_slice(new_slice);
        for h in &self.headings[hi..] {
            updated.push(HeadingItem::new(
                h.line.saturating_add_signed(line_delta),
                h.level,
            ));
        }
        self.headings = updated.into();
        self.revision = buffer.revision();
        (true, true)
    }
}

impl EditorWidget {
    pub fn headings_for_buffer(&mut self, buffer: &Buffer) -> Arc<[HeadingItem]> {
        if let Some(cache) = &self.cached_headings
            && cache.buffer_id == buffer.id()
            && cache.revision == buffer.revision()
        {
            return Arc::clone(&cache.headings);
        }
        let cache = HeadingCache::scan(buffer);
        let headings = Arc::clone(&cache.headings);
        self.cached_headings = Some(cache);
        headings
    }

    pub(super) fn rebase_headings_and_folds(
        &mut self,
        buffer: &Buffer,
        first: usize,
        old_last: usize,
        new_last: usize,
        line_delta: isize,
    ) {
        let mut headings_changed = false;
        if let Some(cache) = &mut self.cached_headings
            && cache.buffer_id == buffer.id()
        {
            let (valid, changed) =
                cache.apply_change(buffer, first, old_last, new_last, line_delta);
            headings_changed = changed;
            if !valid {
                self.cached_headings = None;
            }
        }

        if line_delta != 0
            && let Some(folds) = self.buffer_folds.get_mut(&buffer.id())
            && !folds.is_empty()
        {
            let old_folds = std::mem::take(folds);
            for line in old_folds {
                if line < first {
                    folds.insert(line);
                } else if line > old_last {
                    folds.insert(line.saturating_add_signed(line_delta));
                } else {
                    folds.insert(first);
                }
            }
        }

        if (line_delta != 0 || headings_changed)
            && (!self.active_hidden_ranges.is_empty()
                || self
                    .buffer_folds
                    .get(&buffer.id())
                    .is_some_and(|s| !s.is_empty()))
        {
            let headings = self.headings_for_buffer(buffer);
            let empty_set = BTreeSet::new();
            let folds = self.buffer_folds.get(&buffer.id()).unwrap_or(&empty_set);
            self.active_hidden_ranges = truff_core::markdown::compute_hidden_line_ranges(
                &headings,
                folds,
                buffer.line_len(),
            );
        }
    }

    pub fn toggle_fold(&mut self, heading_line: usize, buffer: &Buffer) {
        let folds = self.buffer_folds.entry(buffer.id()).or_default();
        if !folds.remove(&heading_line) {
            folds.insert(heading_line);
        }
        self.update_hidden_ranges(buffer);
    }

    pub fn fold_close(&mut self, heading_line: usize, buffer: &Buffer) {
        let folds = self.buffer_folds.entry(buffer.id()).or_default();
        folds.insert(heading_line);
        self.update_hidden_ranges(buffer);
    }

    pub fn fold_open(&mut self, heading_line: usize, buffer: &Buffer) {
        let folds = self.buffer_folds.entry(buffer.id()).or_default();
        folds.remove(&heading_line);
        self.update_hidden_ranges(buffer);
    }

    pub fn fold_close_all(&mut self, buffer: &Buffer) {
        let headings = self.headings_for_buffer(buffer);
        let folds = self.buffer_folds.entry(buffer.id()).or_default();
        for h in headings.iter() {
            folds.insert(h.line);
        }
        self.update_hidden_ranges(buffer);
    }

    pub fn fold_open_all(&mut self, buffer: &Buffer) {
        self.buffer_folds.remove(&buffer.id());
        self.update_hidden_ranges(buffer);
    }

    pub fn is_line_folded(&self, line: usize) -> bool {
        truff_core::markdown::is_line_in_hidden_ranges(line, &self.active_hidden_ranges)
    }

    pub fn is_heading_folded(&self, buffer_id: BufferId, line: usize) -> bool {
        self.buffer_folds
            .get(&buffer_id)
            .is_some_and(|set| set.contains(&line))
    }

    pub fn update_hidden_ranges(&mut self, buffer: &Buffer) {
        let headings = self.headings_for_buffer(buffer);
        let empty_set = BTreeSet::new();
        let folds = self.buffer_folds.get(&buffer.id()).unwrap_or(&empty_set);
        let lines = buffer.line_len();
        let hidden = truff_core::markdown::compute_hidden_line_ranges(&headings, folds, lines);

        for r in &hidden {
            for l in r.clone() {
                self.heights.update_block_height(l, 0.0, None);
            }
        }
        for r in &self.active_hidden_ranges {
            for l in r.clone() {
                if !truff_core::markdown::is_line_in_hidden_ranges(l, &hidden) {
                    self.heights
                        .update_block_height(l, self.heights.default_line_height(), None);
                }
            }
        }

        self.active_hidden_ranges = hidden;
        self.cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use truff_core::Transaction;

    #[test]
    fn incremental_heading_cache_matches_full_scan_and_skips_fences() {
        let initial = "# Title\nbody\n```rust\n# Not a heading\n```\n## Section\n";
        let mut buf = Buffer::from_str(initial);
        let mut cache = HeadingCache::scan(&buf);
        assert_eq!(
            &*cache.headings,
            &[HeadingItem::new(0, 1), HeadingItem::new(5, 2)]
        );

        // Single-character body edit takes the zero-allocation fast path.
        let ptr_before = Arc::as_ptr(&cache.headings);
        let pos = initial.find("body").unwrap();
        buf.apply(Transaction::insert(pos, "x")).unwrap();
        let (valid, changed) = cache.apply_change(&buf, 1, 1, 1, 0);
        assert!(valid && !changed);
        assert!(std::ptr::eq(Arc::as_ptr(&cache.headings), ptr_before));

        // Inserting two lines shifts downstream headings and preserves fenced skip.
        buf.apply(Transaction::insert(pos, "### New Sub\nplain\n"))
            .unwrap();
        let (valid, changed) = cache.apply_change(&buf, 1, 1, 3, 2);
        assert!(valid && changed);
        let full = HeadingCache::scan(&buf);
        assert_eq!(&*cache.headings, &*full.headings);
    }
}
