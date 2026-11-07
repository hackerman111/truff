use truff_core::markdown::facts::MarkdownFacts;
use truff_core::markdown::syntax::IncrementalSyntaxTree;

#[test]
fn no_links_in_code() {
    let md = r#"
Here is a normal [[RealLink]].

```rust
// In a code fence:
let link = "[[CodeFenceLink]]";
let md_link = "[NotALink](https://example.com)";
```

And inline code: `[[InlineLink]]` or `[InlineMd](https://test.com)`.

Final paragraph with [[AnotherRealLink]].
"#;

    let facts = MarkdownFacts::extract(md).expect("Markdown parsing failed");
    let targets: Vec<&str> = facts.links.iter().map(|l| l.target.as_str()).collect();

    assert_eq!(
        targets,
        vec!["RealLink", "AnotherRealLink"],
        "Links inside code fence or inline code must be ignored"
    );
}

#[test]
fn escaped_wiki() {
    let md = r#"
Escaped wiki: \[[EscapedNote]] should not be a link.
Real wiki: [[ValidNote]].
Another escaped: \[[Dir/AnotherEscaped#Heading|Alias]].
"#;

    let facts = MarkdownFacts::extract(md).expect("Markdown parsing failed");
    let targets: Vec<&str> = facts.links.iter().map(|l| l.target.as_str()).collect();

    assert_eq!(
        targets,
        vec!["ValidNote"],
        "Escaped wiki links must not be recognized as links"
    );
}

#[test]
fn duplicate_occurrences() {
    let md = "First [[Note]] and second [[Note]] and third [[Note#Subheading|Alias]].";
    let facts = MarkdownFacts::extract(md).expect("Markdown parsing failed");

    assert_eq!(facts.links.len(), 3, "All occurrences must be retained");

    // Ordinals must be sequential (0, 1, 2)
    assert_eq!(facts.links[0].ordinal, 0);
    assert_eq!(facts.links[1].ordinal, 1);
    assert_eq!(facts.links[2].ordinal, 2);

    // Ranges must be distinct and non-overlapping
    assert_ne!(facts.links[0].range, facts.links[1].range);
    assert_ne!(facts.links[1].range, facts.links[2].range);

    // Verify exact content in ranges
    assert_eq!(&md[facts.links[0].range.clone()], "[[Note]]");
    assert_eq!(&md[facts.links[1].range.clone()], "[[Note]]");
    assert_eq!(
        &md[facts.links[2].range.clone()],
        "[[Note#Subheading|Alias]]"
    );

    // Details of complex wiki link
    assert_eq!(facts.links[2].target, "Note");
    assert_eq!(facts.links[2].heading.as_deref(), Some("Subheading"));
    assert_eq!(facts.links[2].label.as_deref(), Some("Alias"));
}

#[test]
fn block_id_and_headings_extraction() {
    let md = r#"# Top Heading

Paragraph one.

## Second Heading

Paragraph with block ID. ^my-block-id-1

### Third Heading
"#;

    let facts = MarkdownFacts::extract(md).expect("Markdown parsing failed");

    assert_eq!(facts.headings.len(), 3);
    assert_eq!(facts.headings[0].level, 1);
    assert_eq!(facts.headings[0].text, "Top Heading");
    assert_eq!(facts.headings[1].level, 2);
    assert_eq!(facts.headings[1].text, "Second Heading");

    assert_eq!(facts.blocks.len(), 1);
    assert_eq!(facts.blocks[0].id, "my-block-id-1");
}

#[test]
fn incremental_equals_fresh() {
    let initial_text = "# Title\n\nParagraph with [[Link]] and code.\n\n```rust\nlet x = 1;\n```\n";
    let mut tree = IncrementalSyntaxTree::new();
    tree.init(initial_text);

    // Edit: replace "[[Link]]" with "[[UpdatedLink]] and some extra text"
    let edit_start = initial_text.find("[[Link]]").unwrap();
    let old_len = "[[Link]]".len();
    let replacement = "[[UpdatedLink]] and some extra text";
    let new_len = replacement.len();

    let mut edited_text = initial_text.to_string();
    edited_text.replace_range(edit_start..edit_start + old_len, replacement);

    // Incremental update
    let buffer = truff_core::Buffer::from_str(initial_text);
    let point = |pos| {
        let row = buffer.line_of_byte(pos);
        tree_sitter::Point::new(row, pos - buffer.byte_of_line(row))
    };
    let new_buffer = truff_core::Buffer::from_str(&edited_text);
    let new_row = new_buffer.line_of_byte(edit_start + new_len);
    let edit = tree_sitter::InputEdit {
        start_byte: edit_start,
        old_end_byte: edit_start + old_len,
        new_end_byte: edit_start + new_len,
        start_position: point(edit_start),
        old_end_position: point(edit_start + old_len),
        new_end_position: tree_sitter::Point::new(
            new_row,
            edit_start + new_len - new_buffer.byte_of_line(new_row),
        ),
    };
    let incremental_snapshot = tree.edit_and_parse(edit, &crop::Rope::from(edited_text.as_str()));

    // Fresh parse from scratch
    let fresh_tree = IncrementalSyntaxTree::parse_fresh(&edited_text);
    let fresh_snapshot = fresh_tree.snapshot();

    // Verify structure equals fresh
    assert_eq!(
        incremental_snapshot.syntax_tree_sexp(),
        fresh_snapshot.syntax_tree_sexp(),
        "Incremental parse tree must match fresh parse tree exactly"
    );
}

#[test]
fn test_heading_folding_ranges_and_nested_subsections() {
    use std::collections::BTreeSet;
    use truff_core::markdown::{
        HeadingItem, compute_hidden_line_ranges, find_heading_for_line, heading_fold_range,
        is_line_in_hidden_ranges,
    };

    // Document structure:
    // Line 0: # H1 (level 1)
    // Line 1: text 1
    // Line 2: ## H2-A (level 2)
    // Line 3: text 2
    // Line 4: ### H3 (level 3)
    // Line 5: text 3
    // Line 6: ## H2-B (level 2)
    // Line 7: text 4
    // Line 8: # H1-Next (level 1)
    // Line 9: text 5
    let total_lines = 10;
    let headings = vec![
        HeadingItem::new(0, 1),
        HeadingItem::new(2, 2),
        HeadingItem::new(4, 3),
        HeadingItem::new(6, 2),
        HeadingItem::new(8, 1),
    ];

    // 1. Fold range of H1 at line 0 (level 1) spans to line 8 (next level 1)
    assert_eq!(heading_fold_range(&headings, 0, total_lines), 1..8);

    // 2. Fold range of H2-A at line 2 (level 2) spans to line 6 (next level 2)
    assert_eq!(heading_fold_range(&headings, 1, total_lines), 3..6);

    // 3. Fold range of H3 at line 4 (level 3) spans to line 6 (level 2 <= 3)
    assert_eq!(heading_fold_range(&headings, 2, total_lines), 5..6);

    // 4. Fold range of H2-B at line 6 (level 2) spans to line 8 (level 1 <= 2)
    assert_eq!(heading_fold_range(&headings, 3, total_lines), 7..8);

    // 5. Fold range of H1-Next at line 8 (level 1) spans to EOF line 10
    assert_eq!(heading_fold_range(&headings, 4, total_lines), 9..10);

    // 6. Test hidden ranges with multiple folds
    let mut folded = BTreeSet::new();
    folded.insert(2); // fold H2-A
    let hidden = compute_hidden_line_ranges(&headings, &folded, total_lines);
    assert_eq!(hidden, vec![3..6]);
    assert!(is_line_in_hidden_ranges(3, &hidden));
    assert!(is_line_in_hidden_ranges(4, &hidden));
    assert!(is_line_in_hidden_ranges(5, &hidden));
    assert!(!is_line_in_hidden_ranges(2, &hidden)); // heading itself is visible
    assert!(!is_line_in_hidden_ranges(6, &hidden));

    // When parent H1 is also folded, range merges to 1..8
    folded.insert(0);
    let hidden = compute_hidden_line_ranges(&headings, &folded, total_lines);
    assert_eq!(hidden, vec![1..8]);

    // 7. Find heading for line
    assert_eq!(find_heading_for_line(&headings, 2, total_lines), Some(1)); // line 2 is H2-A
    assert_eq!(find_heading_for_line(&headings, 3, total_lines), Some(1)); // line 3 inside H2-A
    assert_eq!(find_heading_for_line(&headings, 5, total_lines), Some(2)); // line 5 inside H3
    assert_eq!(find_heading_for_line(&headings, 7, total_lines), Some(3)); // line 7 inside H2-B
}
