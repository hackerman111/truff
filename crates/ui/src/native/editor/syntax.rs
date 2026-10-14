use truff_core::{
    Buffer,
    markdown::{IncrementalSyntaxTree, SyntaxSnapshot},
    text::BufferId,
    workspace::BufferChange,
};

#[derive(Default)]
pub(super) struct EditorSyntax {
    tree: Option<IncrementalSyntaxTree>,
    source: crop::Rope,
    key: Option<(BufferId, u64)>,
}

// Candidate prefixes enable the real block parser; plain text needs no document tree.
fn may_need_block_context(source: crop::RopeSlice<'_>) -> bool {
    source.lines().any(|line| {
        let mut chars = line.chars().skip_while(|ch| matches!(ch, ' ' | '\t'));
        match chars.next() {
            Some('-' | '+' | '*') => chars.next().is_some_and(char::is_whitespace),
            Some('`') => chars.next() == Some('`') && chars.next() == Some('`'),
            Some('~') => chars.next() == Some('~') && chars.next() == Some('~'),
            Some('0'..='9') => {
                let end = chars.find(|ch| !ch.is_ascii_digit());
                matches!(end, Some('.' | ')')) && chars.next().is_some_and(char::is_whitespace)
            }
            Some('>') => true,
            _ => false,
        }
    })
}

impl EditorSyntax {
    pub(super) fn snapshot(&mut self, buffer: &Buffer) -> Option<SyntaxSnapshot> {
        if self.key != Some((buffer.id(), buffer.revision())) {
            self.tree = may_need_block_context(buffer.rope().byte_slice(..)).then(|| {
                let mut tree = IncrementalSyntaxTree::new();
                tree.init_rope(buffer.rope());
                tree
            });
            self.source = buffer.rope().clone();
            self.key = Some((buffer.id(), buffer.revision()));
        }
        self.tree.as_ref().map(IncrementalSyntaxTree::snapshot)
    }

    pub(super) fn apply_change(&mut self, buffer: &Buffer, change: &BufferChange) {
        if change.invalidate_suffix
            || change.buffer_id != buffer.id()
            || change.revision != buffer.revision()
            || !self.key.is_some_and(|(id, revision)| {
                id == buffer.id()
                    && revision.checked_add(change.edit_batches) == Some(change.revision)
            })
        {
            return;
        }
        if let Some(tree) = &mut self.tree {
            tree.replace_rope_range(
                &self.source,
                buffer.rope(),
                change.old_range.clone(),
                change.new_range.clone(),
            );
        } else {
            let first = buffer.line_of_byte(change.new_range.start);
            let last = buffer.line_of_byte(change.new_range.end);
            let end = if last + 1 < buffer.line_len() {
                buffer.byte_of_line(last + 1)
            } else {
                buffer.byte_len()
            };
            if may_need_block_context(buffer.rope().byte_slice(buffer.byte_of_line(first)..end)) {
                let mut tree = IncrementalSyntaxTree::new();
                tree.init_rope(buffer.rope());
                self.tree = Some(tree);
            }
        }
        self.source = buffer.rope().clone();
        self.key = Some((buffer.id(), buffer.revision()));
    }
}
