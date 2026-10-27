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
