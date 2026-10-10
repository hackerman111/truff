use std::ops::Range;

use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

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

    /// Applies an incremental edit and reparses the updated text.
    pub fn edit_and_parse(
        &mut self,
        start_byte: usize,
        old_len: usize,
        new_len: usize,
        new_text: &str,
    ) -> SyntaxSnapshot {
        let old_end_byte = start_byte + old_len;
        let new_end_byte = start_byte + new_len;

        if let Some(ref mut old_tree) = self.tree {
            let start_position = byte_to_point(new_text, start_byte);
            let old_end_position = byte_to_point(
                new_text,
                start_byte + old_len.min(new_text.len() - start_byte),
            );
            let new_end_position = byte_to_point(new_text, new_end_byte.min(new_text.len()));

            let input_edit = InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            };

            old_tree.edit(&input_edit);
            let new_tree = self
                .parser
                .parse(new_text, Some(old_tree))
                .expect("Incremental reparse failed");
            self.tree = Some(new_tree.clone());
            SyntaxSnapshot::new(new_tree)
        } else {
            self.init(new_text);
            self.snapshot()
        }
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

fn byte_to_point(text: &str, pos: usize) -> Point {
    let mut row = 0;
    let mut col = 0;
    for (i, c) in text.char_indices() {
        if i >= pos {
            break;
        }
        if c == '\n' {
            row += 1;
            col = 0;
        } else {
            col += c.len_utf8();
        }
    }
    Point { row, column: col }
}
