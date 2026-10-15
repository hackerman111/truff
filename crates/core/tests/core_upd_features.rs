use std::path::PathBuf;

use truff_core::commands::{Action, CommandContext, CommandId};
use truff_core::config::KeymapConfig;
use truff_core::markdown::flash::{FlashMode, FlashTargetKind, collect_flash_targets};
use truff_core::markdown::link_selection::{LinkSelection, LinkTarget};
use truff_core::markdown::smart_edit::{plan_enter, plan_shift_tab, plan_tab};
use truff_core::markdown::snapshot::{MarkdownSnapshot, SemanticKind};
use truff_core::navigation::{Move, RowId, RowItem, RowModel};
use truff_core::text::{Buffer, BytePos};
use truff_core::vim::execute::KeyDisposition;
use truff_core::vim::motion::TextObjectTarget;
use truff_core::vim::{Key, Mode, VimState};
use truff_core::workspace::{RefactorPlan, Workspace};

#[test]
fn test_input_semantics_and_key_disposition() {
    let keymap = KeymapConfig::default().compile().unwrap();
    let mut state = VimState::default();

    // 1. Normal mode in Editor: 's' resolves to flash.jump_word
    let disp =
        state.resolve_key_disposition(Key::Char('s'), &[], CommandContext::Editor, false, &keymap);
    assert_eq!(
        disp,
        KeyDisposition::KeymapCommand(CommandId::from_static("flash.jump_word"))
    );

    // 2. Insert mode in Editor: 's' is LiteralText, NEVER flash or command
    state.mode = Mode::Insert;
    let disp =
        state.resolve_key_disposition(Key::Char('s'), &[], CommandContext::Editor, false, &keymap);
    assert_eq!(disp, KeyDisposition::LiteralText('s'));

    // 3. Insert mode in TextField: printable character is LiteralText
    let disp = state.resolve_key_disposition(
        Key::Char('a'),
        &[],
        CommandContext::TextField,
        false,
        &keymap,
    );
    assert_eq!(disp, KeyDisposition::LiteralText('a'));

    // 4. IME composing owns all key events
    let disp =
        state.resolve_key_disposition(Key::Char('s'), &[], CommandContext::Editor, true, &keymap);
    assert_eq!(disp, KeyDisposition::ImeComposing);

    // 5. Esc in Insert mode inside field exits to Normal in field without closing outer modal
    let disp =
        state.resolve_key_disposition(Key::Esc, &[], CommandContext::TextField, false, &keymap);
    assert_eq!(disp, KeyDisposition::ExitInsertInField);

    // 6. Esc in Normal mode without pending input closes modal
    state.mode = Mode::Normal;
    let disp = state.resolve_key_disposition(Key::Esc, &[], CommandContext::Modal, false, &keymap);
    assert_eq!(disp, KeyDisposition::CloseModalOrCancel);

    // 7. Space leader prefix in Normal mode
    let disp =
        state.resolve_key_disposition(Key::Char(' '), &[], CommandContext::Editor, false, &keymap);
    assert_eq!(disp, KeyDisposition::KeymapPrefix);

    // 8. Space f completes panel.files
    let disp = state.resolve_key_disposition(
        Key::Char('f'),
        &[Key::Char(' ')],
        CommandContext::Editor,
        false,
        &keymap,
    );
    assert_eq!(
        disp,
        KeyDisposition::KeymapCommand(CommandId::from_static("panel.files"))
    );
}

#[test]
fn test_row_model_visual_selection_and_actions() {
    let items = vec![
        RowItem {
            id: RowId::new(1),
            title: "root1".into(),
            depth: 0,
            is_folder: false,
            is_collapsed: false,
            data: None,
        },
        RowItem {
            id: RowId::new(2),
            title: "root2".into(),
            depth: 0,
            is_folder: false,
            is_collapsed: false,
            data: None,
        },
        RowItem {
            id: RowId::new(3),
            title: "root3".into(),
            depth: 0,
            is_folder: false,
            is_collapsed: false,
            data: None,
        },
    ];

    let mut model = RowModel::new();
    model.set_items(items);
    assert_eq!(model.selected_range(), 0..=0);
    assert_eq!(model.selected_rows().len(), 1);

    // Start visual selection and move down
    model.start_visual_selection();
    model.move_selection(Move::Down(1));
    model.move_selection(Move::Down(1));
    assert_eq!(model.selected_range(), 0..=2);
    assert_eq!(model.selected_rows().len(), 3);

    // Clear visual selection
    model.clear_visual_selection();
    assert_eq!(model.selected_range(), 2..=2);
    assert_eq!(model.selected_rows().len(), 1);
}

#[test]
fn test_unicode_visual_char_and_line_ranges() {
    // String with complex Unicode: Cyrillic, composite e + acute, family emoji, flag
    let text = "Привет\ne\u{0301}\n👩‍💻\n🇺🇸\nEOF";
    let buffer = Buffer::from_str(text);

    // VisualChar covers full grapheme cluster regardless of direction
    let e_offset = text.find("e\u{0301}").unwrap();
    let e_pos = BytePos::new(e_offset);
    let e_len = "e\u{0301}".len();
    let range_forward = buffer.visual_char_inclusive_range(e_pos, e_pos);
    assert_eq!(&text[range_forward.clone()], "e\u{0301}");
    assert_eq!(range_forward, e_offset..(e_offset + e_len));

    // Direction test: anchor > head
    let next_line_offset = text.find("👩‍💻").unwrap();
    let range_backward = buffer.visual_char_inclusive_range(BytePos::new(next_line_offset), e_pos);
    assert_eq!(range_backward.start, e_offset);

    // VisualLine covering lines 0 to 1
    let line_range = buffer.visual_line_range(BytePos::ZERO, e_pos);
    assert_eq!(&text[line_range.clone()], "Привет\ne\u{0301}\n");

    // Line ending normalization test
    let crlf_buf = Buffer::from_str("Привет\r\nМир\r\n");
    let crlf_range = crlf_buf.visual_line_range(BytePos::ZERO, BytePos::new(2));
    assert_eq!(crlf_buf.slice(crlf_range).to_string(), "Привет\r\n");
}

#[test]
fn test_markdown_snapshot_facts_and_queries() {
    let md = r#"---
title: Test Document
status: active
---

# Heading One with `code`

Some text with a [[target-note#section-2|Target Alias]] and an anchor ^block-123.

## Heading Two

> [!tip] Practical Tip
> Be careful with CRLF.

| Col A | Col B |
| --- | --- |
| Cell 1 | [[Note\|Cell Alias]] |

- [ ] Task incomplete
- [x] Task complete
"#;

    let snapshot = MarkdownSnapshot::parse(truff_core::text::BufferId::new(1), 1, md);

    // Frontmatter check
    let fm = snapshot.frontmatter.as_ref().expect("Frontmatter parsed");
    assert_eq!(fm.properties.len(), 2);
    assert_eq!(
        fm.properties[0],
        ("title".to_string(), "Test Document".to_string())
    );

    // Heading with inline code check
    assert_eq!(snapshot.facts.headings.len(), 2);
    assert_eq!(snapshot.facts.headings[0].text, "Heading One with code");

    // Sections check
    assert_eq!(snapshot.sections.len(), 2);
    assert_eq!(snapshot.sections[0].level, 1);
    assert_eq!(snapshot.sections[1].level, 2);
    assert_eq!(snapshot.sections[1].parent_section, Some(0));

    // Links check (including wiki with alias and escaped pipe in table)
    assert_eq!(snapshot.facts.links.len(), 2);
    assert_eq!(snapshot.facts.links[0].target, "target-note");
    assert_eq!(
        snapshot.facts.links[0].heading.as_deref(),
        Some("section-2")
    );
    assert_eq!(
        snapshot.facts.links[0].label.as_deref(),
        Some("Target Alias")
    );

    assert_eq!(snapshot.facts.links[1].target, "Note");
    assert_eq!(snapshot.facts.links[1].label.as_deref(), Some("Cell Alias"));

    // Tasks check
    assert_eq!(snapshot.tasks.len(), 2);
    assert!(!snapshot.tasks[0].checked);
    assert!(snapshot.tasks[1].checked);

    // Callout check
    assert_eq!(snapshot.callouts.len(), 1);
    assert_eq!(snapshot.callouts[0].title.as_deref(), Some("Practical Tip"));

    // Syntax object ranges for Vim text objects
    let syntax_ranges = snapshot.syntax_object_ranges();
    assert!(
        syntax_ranges
            .iter()
            .any(|r| r.target == TextObjectTarget::MarkdownHeading)
    );
    assert!(
        syntax_ranges
            .iter()
            .any(|r| r.target == TextObjectTarget::MarkdownSection)
    );
    assert!(
        syntax_ranges
            .iter()
            .any(|r| r.target == TextObjectTarget::MarkdownQuote)
    );
    assert!(
        syntax_ranges
            .iter()
            .any(|r| r.target == TextObjectTarget::MarkdownListItem)
    );

    // Queries check
    let offset_in_h2 = snapshot.facts.headings[1].range.start + 2;
    let heading = snapshot.heading_under_cursor(offset_in_h2).unwrap();
    assert_eq!(heading.text, "Heading Two");
}

#[test]
fn test_markdown_smart_edit_and_undo() {
    // 1. Task continuation: Enter after a checked task creates an unchecked task
    let mut buffer = Buffer::from_str("- [x] Done task");
    let cursor = BytePos::new(buffer.byte_len());
    let tx = plan_enter(&buffer, cursor).expect("Plan enter for task");
    buffer.apply(tx).unwrap();
    assert_eq!(buffer.as_str(), "- [x] Done task\n- [ ] ");

    // Undo restores exact original text
    buffer.undo().unwrap();
    assert_eq!(buffer.as_str(), "- [x] Done task");

    // 2. Empty item closure: Enter on empty bullet clears the bullet
    let mut buffer = Buffer::from_str("- ");
    let cursor = BytePos::new(buffer.byte_len());
    let tx = plan_enter(&buffer, cursor).expect("Plan enter for empty bullet");
    buffer.apply(tx).unwrap();
    assert_eq!(buffer.as_str(), "");

    // 3. Ordered list continuation (only new item incremented per MEBC-1 §3.1)
    let initial = "1. First\n2. Second\n";
    let mut buffer = Buffer::from_str(initial);
    // Cursor at end of line 1 ("1. First")
    let cursor = BytePos::new(8);
    let tx = plan_enter(&buffer, cursor).expect("Plan enter for ordered list");
    buffer.apply(tx).unwrap();
    assert_eq!(buffer.as_str(), "1. First\n2. \n2. Second\n");

    buffer.undo().unwrap();
    assert_eq!(buffer.as_str(), initial);

    // 4. Tab indent and ShiftTab outdent (default 2 spaces per MEBC-1 §3.2)
    let mut buffer = Buffer::from_str("- Item");
    let tx_tab = plan_tab(&buffer, BytePos::new(2)).expect("Plan tab");
    buffer.apply(tx_tab).unwrap();
    assert_eq!(buffer.as_str(), "  - Item");

    let tx_shift_tab = plan_shift_tab(&buffer, BytePos::new(4)).expect("Plan shift tab");
    buffer.apply(tx_shift_tab).unwrap();
    assert_eq!(buffer.as_str(), "- Item");
}

#[test]
fn test_link_selection_and_refactor_plan() {
    let mut buffer = Buffer::from_str("Extract this important text from document.");
    let selected_range = 8..27; // "this important text"

    let target = LinkTarget {
        note: "ImportantConcept".into(),
        heading: Some("Overview".into()),
        block_id: None,
        alias: None, // Should auto-use selected text as alias!
        use_wikilink: true,
        inside_table: false,
    };

    let wrap_tx = LinkSelection::plan_wrap_selection(&buffer, selected_range.clone(), &target)
        .expect("Plan wrap selection");
    buffer.apply(wrap_tx).unwrap();
    assert_eq!(
        buffer.as_str(),
        "Extract [[ImportantConcept#Overview|this important text]] from document."
    );

    // Undo restore
    buffer.undo().unwrap();
    assert_eq!(
        buffer.as_str(),
        "Extract this important text from document."
    );

    // Plan note extraction
    let extract_plan = LinkSelection::plan_extract_note(
        &buffer,
        selected_range,
        PathBuf::from("vault/ImportantConcept.md"),
        "ImportantConcept",
        None,
    )
    .expect("Plan extract note");

    assert_eq!(extract_plan.extracted_content, "this important text");
    assert_eq!(
        extract_plan.target_path,
        PathBuf::from("vault/ImportantConcept.md")
    );

    // Conversion to RefactorPlan
    let refactor_plan: RefactorPlan = extract_plan.into();
    assert_eq!(refactor_plan.extracted_content, "this important text");
}

#[test]
fn test_collect_flash_targets() {
    let md = "# Title\n\nWord one two three.\n";
    let buffer = Buffer::from_str(md);
    let snapshot = MarkdownSnapshot::from_buffer(&buffer);

    let word_targets = collect_flash_targets(
        &buffer,
        Some(&snapshot),
        0..buffer.byte_len(),
        FlashMode::Word,
    );
    assert!(!word_targets.is_empty());
    assert_eq!(word_targets[0].kind, FlashTargetKind::Word);

    let syntax_targets = collect_flash_targets(
        &buffer,
        Some(&snapshot),
        0..buffer.byte_len(),
        FlashMode::SyntaxObject,
    );
    assert!(!syntax_targets.is_empty());
    assert!(
        syntax_targets
            .iter()
            .any(|t| matches!(t.kind, FlashTargetKind::Syntax(SemanticKind::Heading(_))))
    );
}

#[test]
fn test_workspace_activate_row_effect() {
    let mut ws = Workspace::new();
    let effects = ws
        .dispatch(Action::ActivateRow {
            panel: "files".into(),
            row_id: RowId::new(42),
            data: Some("test.md".into()),
        })
        .expect("Dispatch ActivateRow");

    assert_eq!(effects.len(), 1);
    assert!(matches!(
        &effects[0],
        truff_core::commands::Effect::StatusMessage(msg) if msg.contains("files") && msg.contains("test.md")
    ));
}
