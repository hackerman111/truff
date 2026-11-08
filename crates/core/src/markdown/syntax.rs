use std::ops::Range;

use tree_sitter::{InputEdit, Node, Parser, Tree};

/// An immutable snapshot of the syntax tree produced by Tree-sitter.
#[derive(Debug, Clone)]
pub struct SyntaxSnapshot {
    tree: Tree,
}

impl SyntaxSnapshot {
    /// Creates a snapshot wrapping a `Tree`.
    pub fn new(tree: Tree) -> Self {
        Self { tree }
    }

    /// S-expression representation of the syntax tree.
    pub fn syntax_tree_sexp(&self) -> String {
        self.tree.root_node().to_sexp()
    }

    /// Accesses the root node of the syntax tree.
    pub fn root_node(&self) -> Node<'_> {
        self.tree.root_node()
    }

    /// Finds the smallest named syntax node covering `pos`.
    pub fn find_node_at(&self, pos: usize) -> Option<Node<'_>> {
        self.tree.root_node().descendant_for_byte_range(pos, pos)
    }

    /// Collects byte ranges of all nodes with the given syntax kind name.
    pub fn syntax_ranges_for_kind(&self, kind: &str) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        let mut cursor = self.tree.walk();

        fn walk(cursor: &mut tree_sitter::TreeCursor, kind: &str, acc: &mut Vec<Range<usize>>) {
            let node = cursor.node();
            if node.kind() == kind {
                acc.push(node.byte_range());
            }
            if cursor.goto_first_child() {
                loop {
                    walk(cursor, kind, acc);
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
                cursor.goto_parent();
            }
        }

        walk(&mut cursor, kind, &mut ranges);
        ranges
    }
}

/// Stateful incremental syntax tree engine.
pub struct IncrementalSyntaxTree {
    parser: Parser,
    tree: Option<Tree>,
}

impl Default for IncrementalSyntaxTree {
    fn default() -> Self {
        Self::new()
    }
}

impl IncrementalSyntaxTree {
    /// Creates a new parser configured with Markdown block grammar.
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_md::LANGUAGE.into())
            .expect("Failed to initialize tree-sitter Markdown language");

        Self { parser, tree: None }
    }

    /// Parses text from scratch.
    pub fn parse_fresh(text: &str) -> Self {
        let mut tree = Self::new();
        tree.init(text);
        tree
    }

    /// Initializes or resets the syntax tree with the provided text.
    pub fn init(&mut self, text: &str) {
        self.tree = self.parser.parse(text, None);
    }

    /// Initializes the tree directly from borrowed Rope chunks.
    pub fn init_rope(&mut self, rope: &crop::Rope) {
        self.tree = None;
        self.parse_rope(rope);
    }

    /// Applies an edit whose old points were captured before modifying the Rope.
    /// Point columns are byte offsets from the start of their logical line.
    pub fn edit_and_parse(&mut self, edit: InputEdit, rope: &crop::Rope) -> SyntaxSnapshot {
        if let Some(ref mut old_tree) = self.tree {
            old_tree.edit(&edit);
        }
        self.parse_rope(rope);
        self.snapshot()
    }

    /// Reparses a summarized replacement while retaining unchanged syntax subtrees.
    pub fn replace_rope_range(
        &mut self,
        old: &crop::Rope,
        new: &crop::Rope,
        old_range: Range<usize>,
        new_range: Range<usize>,
    ) {
        self.apply_input_edit(old, new, &old_range, &new_range);
        self.parse_rope(new);
    }

    /// Updates block syntax state for an edit, deferring the C block reparse when
    /// a single-line edit cannot alter Markdown block structure while still shifting
    /// tree byte ranges via `Tree::edit`.
    pub fn update_block_context_rope(
        &mut self,
        old: &crop::Rope,
        new: &crop::Rope,
        old_range: Range<usize>,
        new_range: Range<usize>,
    ) {
        self.apply_input_edit(old, new, &old_range, &new_range);
        if !is_block_invariant_line_edit(old, new, &old_range, &new_range) {
            self.parse_rope(new);
        }
    }

    fn apply_input_edit(
        &mut self,
        old: &crop::Rope,
        new: &crop::Rope,
        old_range: &Range<usize>,
        new_range: &Range<usize>,
    ) {
        if let Some(tree) = &mut self.tree {
            tree.edit(&InputEdit {
                start_byte: old_range.start,
                old_end_byte: old_range.end,
                new_end_byte: new_range.end,
                start_position: rope_byte_to_point(old, old_range.start),
                old_end_position: rope_byte_to_point(old, old_range.end),
                new_end_position: rope_byte_to_point(new, new_range.end),
            });
        }
    }

    fn parse_rope(&mut self, rope: &crop::Rope) {
        self.tree = self.parser.parse_with_options(
            &mut |offset, _| {
                if offset >= rope.byte_len() {
                    &[][..]
                } else {
                    rope.byte_slice(offset..)
                        .chunks()
                        .next()
                        .unwrap_or("")
                        .as_bytes()
                }
            },
            self.tree.as_ref(),
            None,
        );
    }

    /// Returns a snapshot of the current syntax tree.
    pub fn snapshot(&self) -> SyntaxSnapshot {
        let tree = self
            .tree
            .clone()
            .expect("IncrementalSyntaxTree has not been initialized with text");
        SyntaxSnapshot::new(tree)
    }
}

/// Converts a byte offset within a Rope to a Tree-sitter Point (row, column) in O(log N).
#[inline]
pub fn rope_byte_to_point(rope: &crop::Rope, byte_offset: usize) -> tree_sitter::Point {
    let clamped = byte_offset.min(rope.byte_len());
    let row = rope.line_of_byte(clamped);
    let line_start = rope.byte_of_line(row);
    tree_sitter::Point {
        row,
        column: clamped - line_start,
    }
}

fn copy_rope_line_bytes<'a>(
    rope: &crop::Rope,
    line_idx: usize,
    buf: &'a mut [u8; 1024],
) -> Option<(usize, &'a [u8])> {
    let start = rope.byte_of_line(line_idx);
    let mut end = if line_idx + 1 < rope.line_len() {
        rope.byte_of_line(line_idx + 1)
    } else {
        rope.byte_len()
    };
    if end - start > buf.len() {
        return None;
    }
    let mut written = 0;
    for chunk in rope.byte_slice(start..end).chunks() {
        let bytes = chunk.as_bytes();
        buf[written..written + bytes.len()].copy_from_slice(bytes);
        written += bytes.len();
    }
    while written > 0 && matches!(buf[written - 1], b'\n' | b'\r') {
        written -= 1;
        end -= 1;
    }
    let _ = end;
    Some((start, &buf[..written]))
}

fn atx_prefix_len(rest: &[u8]) -> Option<usize> {
    let hashes = rest.iter().take_while(|&&b| b == b'#').count();
    if (1..=6).contains(&hashes) && rest.get(hashes).is_some_and(|b| matches!(*b, b' ' | b'\t')) {
        Some(hashes + 1)
    } else {
        None
    }
}

fn is_plain_first_byte(b: u8) -> bool {
    !matches!(
        b,
        b'#' | b'-' | b'*' | b'+' | b'>' | b'`' | b'~' | b'<' | b'|' | b'=' | b'_' | b'[' | b'0'
            ..=b'9'
    )
}

fn is_thematic_candidate(rest: &[u8]) -> bool {
    rest.iter()
        .all(|&b| matches!(b, b'-' | b'*' | b'_' | b' ' | b'\t'))
}

fn is_block_invariant_line_edit(
    old: &crop::Rope,
    new: &crop::Rope,
    old_range: &Range<usize>,
    new_range: &Range<usize>,
) -> bool {
    if old.line_len() != new.line_len()
        || old_range.end > old.byte_len()
        || new_range.end > new.byte_len()
        || old.byte_len() == 0
        || new.byte_len() == 0
    {
        return false;
    }
    let line_idx = old.line_of_byte(old_range.start);
    if old.line_of_byte(old_range.end) != line_idx
        || new.line_of_byte(new_range.start) != line_idx
        || new.line_of_byte(new_range.end) != line_idx
    {
        return false;
    }
    let mut old_buf = [0u8; 1024];
    let mut new_buf = [0u8; 1024];
    let Some((line_start, old_bytes)) = copy_rope_line_bytes(old, line_idx, &mut old_buf) else {
        return false;
    };
    let Some((_, new_bytes)) = copy_rope_line_bytes(new, line_idx, &mut new_buf) else {
        return false;
    };
    if old_bytes.contains(&b'|') || new_bytes.contains(&b'|') {
        return false;
    }
    let ws_len = old_bytes
        .iter()
        .take_while(|&&b| matches!(b, b' ' | b'\t'))
        .count();
    let new_ws_len = new_bytes
        .iter()
        .take_while(|&&b| matches!(b, b' ' | b'\t'))
        .count();
    if ws_len != new_ws_len || old_bytes[..ws_len] != new_bytes[..ws_len] {
        return false;
    }
    let indent_cols: usize = old_bytes[..ws_len]
        .iter()
        .map(|&b| if b == b'\t' { 4 } else { 1 })
        .sum();
    if indent_cols >= 4 {
        return false;
    }
    let old_rest = old_bytes[ws_len..].trim_ascii_end();
    let new_rest = new_bytes[ws_len..].trim_ascii_end();
    if old_rest.is_empty() || new_rest.is_empty() {
        return false;
    }
    if is_plain_first_byte(old_rest[0]) && is_plain_first_byte(new_rest[0]) {
        return true;
    }
    let edit_col = old_range.start.saturating_sub(line_start);
    if let (Some(p_old), Some(p_new)) = (atx_prefix_len(old_rest), atx_prefix_len(new_rest)) {
        return p_old == p_new
            && edit_col >= ws_len + p_old
            && !old_rest.ends_with(b"#")
            && !new_rest.ends_with(b"#");
    }
    if !old_rest.starts_with(b"<")
        && !new_rest.starts_with(b"<")
        && !is_thematic_candidate(old_rest)
        && !is_thematic_candidate(new_rest)
        && let (Ok(old_str), Ok(new_str)) = (
            std::str::from_utf8(old_bytes),
            std::str::from_utf8(new_bytes),
        )
        && let (Some(m_old), Some(m_new)) = (
            super::presentation::parse_line_list_marker(old_str, 0, 0),
            super::presentation::parse_line_list_marker(new_str, 0, 0),
        )
    {
        return m_old == m_new && edit_col >= m_old.range.end;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deferred_block_reparse_shifts_downstream_blocks_and_catches_up_on_structural_edit() {
        let initial = "# Heading\n\nPlain paragraph\n\n```rust\nlet x = 1;\n```\n- [ ] task\n";
        let mut old = crop::Rope::from(initial);
        let mut tree = IncrementalSyntaxTree::new();
        tree.init_rope(&old);

        // Plain body edits defer C reparse while keeping downstream byte ranges exact.
        for ch in ["A", "Б", "🦀"] {
            let pos = old.to_string().find("paragraph").unwrap();
            let mut new = old.clone();
            new.insert(pos, ch);
            assert!(is_block_invariant_line_edit(
                &old,
                &new,
                &(pos..pos),
                &(pos..pos + ch.len())
            ));
            tree.update_block_context_rope(&old, &new, pos..pos, pos..pos + ch.len());
            old = new;
        }
        let snap = tree.snapshot();
        let text = old.to_string();
        let code_pos = text.find("let x").unwrap();
        let task_pos = text.find("- [ ] task").unwrap();
        assert!(
            snap.line_presentation_in_rope(&old, code_pos..code_pos + 9)
                .code
        );
        assert!(
            snap.line_presentation_in_rope(&old, task_pos..task_pos + 10)
                .list
                .is_some()
        );

        // Structural edit catches up and matches a fresh parse.
        let mut new = old.clone();
        new.insert(0, "- new list\n");
        tree.update_block_context_rope(&old, &new, 0..0, 0..11);
        assert_eq!(
            tree.snapshot().syntax_tree_sexp(),
            IncrementalSyntaxTree::parse_fresh(&new.to_string())
                .snapshot()
                .syntax_tree_sexp()
        );
    }
}
