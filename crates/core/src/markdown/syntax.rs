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
        self.parse_rope(new);
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
