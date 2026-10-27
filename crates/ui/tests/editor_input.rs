use truff_core::Buffer;
use truff_core::markdown::syntax::IncrementalSyntaxTree;
use truff_ui::editor::input::{EditorInput, InputAdapter};
use truff_ui::shell::Shell;
use truff_ui::theme::Theme;

#[test]
fn ime_preedit_cancel() {
    let mut buffer = Buffer::from_str("Hello world");
    let initial_rev = buffer.revision();
    let mut cursor = 5;
    let mut selection = None;
    let mut adapter = InputAdapter::new();

    // 1. IME composition starts: preedit text received
    let change = adapter
        .handle_input(
            &mut buffer,
            &mut cursor,
            &mut selection,
            EditorInput::ImePreedit {
                text: "преэдит".to_string(),
                cursor: Some((7, 7)),
            },
        )
        .expect("handle preedit");

    assert!(change.is_none(), "Preedit must not generate buffer changes");
    assert_eq!(adapter.ime_preedit(), Some("преэдит"));
    assert_eq!(buffer.as_str(), "Hello world");
    assert_eq!(
        buffer.revision(),
        initial_rev,
        "Preedit must NOT alter buffer revision!"
    );

    // 2. IME composition cancelled by user (e.g. Esc or backspacing all composition characters)
    let change_cancel = adapter
        .handle_input(
            &mut buffer,
            &mut cursor,
            &mut selection,
            EditorInput::ImeCancel,
        )
        .expect("handle cancel");

    assert!(
        change_cancel.is_none(),
        "Cancel must not generate buffer changes"
    );
    assert_eq!(adapter.ime_preedit(), None);
    assert_eq!(
        buffer.as_str(),
        "Hello world",
        "Cancelled IME preedit must leave buffer completely unmodified!"
    );
    assert_eq!(
        buffer.revision(),
        initial_rev,
        "Buffer revision must remain identical after IME cancellation"
    );
}

#[test]
fn ime_commit_single() {
    let mut buffer = Buffer::from_str("Prefix: ");
    let initial_rev = buffer.revision();
    let mut cursor = buffer.byte_len();
    let mut selection = None;
    let mut adapter = InputAdapter::new();

    // Preedit arrives first
    adapter
        .handle_input(
            &mut buffer,
            &mut cursor,
            &mut selection,
            EditorInput::ImePreedit {
                text: "коммит".to_string(),
                cursor: Some((6, 6)),
            },
        )
        .expect("preedit");

    // Commit arrives
    let change = adapter
        .handle_input(
            &mut buffer,
            &mut cursor,
            &mut selection,
            EditorInput::ImeCommit("коммит".to_string()),
        )
        .expect("commit");

    assert!(change.is_some(), "Commit must generate a ChangeSet");
    assert_eq!(adapter.ime_preedit(), None);
    assert_eq!(buffer.as_str(), "Prefix: коммит");
    assert_eq!(
        buffer.revision(),
        initial_rev + 1,
        "IME commit must execute exactly ONE Buffer::apply transaction!"
    );

    // Calling undo once must restore the buffer to its exact pre-commit state
    let undo_change = buffer.undo().expect("undo");
    assert!(undo_change.is_some());
    assert_eq!(buffer.as_str(), "Prefix: ");
    assert_eq!(buffer.revision(), initial_rev + 2); // undo adds undo transaction
    assert!(!buffer.is_dirty()); // at savepoint
}

#[test]
fn theme_no_parse() {
    let markdown = "# Heading\n\nSome text with **bold** and `code`.\n\n- item 1\n- item 2";
    let syntax_tree = IncrementalSyntaxTree::parse_fresh(markdown);
    let original_sexp = syntax_tree.snapshot().syntax_tree_sexp();

    let mut shell = Shell::new();
    assert_eq!(shell.theme(), &Theme::nord());

    // Switch theme to light theme
    shell.set_theme(Theme::light());
    assert_eq!(shell.theme(), &Theme::light());

    // Switch theme to obsidian_dark
    shell.set_theme(Theme::obsidian_dark());
    assert_eq!(shell.theme(), &Theme::obsidian_dark());

    // Theme switch must NOT reparse or invalidate the syntax tree!
    let snapshot_after = syntax_tree.snapshot();
    assert_eq!(
        snapshot_after.syntax_tree_sexp(),
        original_sexp,
        "Theme switch must not trigger Markdown syntax parsing or tree rebuild!"
    );
}

#[test]
fn app_theme_command_and_persistence() {
    let tmp = std::env::temp_dir().join(format!("truff-test-theme-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let config_dir = tmp.join("config");
    std::fs::create_dir_all(&config_dir).unwrap();

    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &config_dir);
    }

    let mut app = truff_ui::app::Application::new();
    assert_eq!(app.shell.theme(), &Theme::nord());

    // Switch to dark via command
    app.execute_command("theme dark")
        .expect("switch theme to dark");
    assert_eq!(app.shell.theme(), &Theme::obsidian_dark());

    // Verify config was persisted
    let config = truff_ui::app::global_config::GlobalConfig::load();
    assert_eq!(config.theme, "dark");

    // Switch to nord via command
    app.execute_command("theme nord")
        .expect("switch theme to nord");
    assert_eq!(app.shell.theme(), &Theme::nord());

    let _ = app.shutdown();
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_four_tier_palette_tokens() {
    let nord = Theme::nord();
    assert_eq!(
        nord.bg_primary,
        truff_ui::theme::Color::from_hex("#2e3440").unwrap()
    );
    assert_eq!(
        nord.bg_secondary,
        truff_ui::theme::Color::from_hex("#313642").unwrap()
    );
    assert_eq!(
        nord.bg_tertiary,
        truff_ui::theme::Color::from_hex("#252a35").unwrap()
    );
    assert_eq!(
        nord.selection,
        truff_ui::theme::Color::from_hex("#3d4758").unwrap()
    );
    assert_eq!(
        nord.accent,
        truff_ui::theme::Color::from_hex("#88c0d0").unwrap()
    );
    assert_eq!(
        nord.border,
        truff_ui::theme::Color::from_hex("#3b4252").unwrap()
    );
    assert_eq!(
        nord.text_primary,
        truff_ui::theme::Color::from_hex("#eceff4").unwrap()
    );
    assert_eq!(
        nord.text_muted,
        truff_ui::theme::Color::from_hex("#8892b0").unwrap()
    );
}

#[test]
fn test_semantic_design_tokens() {
    let nord = Theme::nord();
    assert_eq!(nord.bg_canvas, nord.bg_primary);
    assert_eq!(nord.bg_panel, nord.bg_secondary);
    assert_eq!(nord.bg_elevated, nord.bg_surface);
    assert_eq!(nord.border_subtle, nord.border);
    assert_eq!(nord.accent_primary, nord.accent);
    assert_eq!(nord.density, truff_ui::theme::Density::Compact);
    assert_eq!(nord.geometry.tab_height, 34.0);
    assert_eq!(nord.geometry.tree_row_height, 30.0);
    assert_eq!(nord.geometry.status_height, 26.0);

    let dark = Theme::obsidian_dark();
    assert_eq!(dark.bg_canvas, dark.bg_primary);
    assert_eq!(dark.bg_panel, dark.bg_secondary);
    assert_eq!(dark.bg_elevated, dark.bg_surface);
    assert_eq!(dark.accent_primary, dark.accent);

    let light = Theme::light();
    assert_eq!(light.bg_canvas, light.bg_primary);
    assert_eq!(light.bg_panel, light.bg_secondary);
    assert_eq!(light.bg_elevated, light.bg_surface);
    assert_eq!(light.accent_primary, light.accent);
}
#[test]
fn task_toggle_preserves_selection_and_can_be_undone() {
    let mut shell = Shell::new();
    shell.set_editor_text("- [ ] задача\n- [X] готово\n");
    shell
        .apply_input(EditorInput::SetSelection {
            anchor: 6,
            head: 10,
        })
        .unwrap();
    let before = shell.editor_view().unwrap().clone();
    shell
        .apply_input(EditorInput::ToggleTask { checkbox: 3 })
        .unwrap();
    assert!(shell.editor_text().starts_with("- [x]"));
    let after = shell.editor_view().unwrap();
    assert_eq!(after.anchor(), before.anchor());
    assert_eq!(after.head(), before.head());
    let id = after.id();
    shell
        .workspace_mut()
        .dispatch(truff_core::commands::Action::Undo(id))
        .unwrap();
    assert!(shell.editor_text().starts_with("- [ ]"));
    let revision = shell.editor_buffer().unwrap().revision();
    shell
        .apply_input(EditorInput::ToggleTask {
            checkbox: usize::MAX,
        })
        .unwrap();
    shell
        .apply_input(EditorInput::ToggleTask { checkbox: 0 })
        .unwrap();
    assert_eq!(shell.editor_buffer().unwrap().revision(), revision);
}

#[test]
fn test_smart_edit_enter_tab_shift_tab_and_undo() {
    use truff_core::vim::{Key, Mode};

    let mut shell = Shell::new();

    // 1. Bullet list continuation
    shell.set_editor_text("- first item");
    // Enter insert mode at the end: 'A'
    shell.feed_key(Key::Char('A'));
    assert_eq!(shell.mode(), Mode::Insert);
    shell.feed_key(Key::Enter);
    assert_eq!(shell.editor_text(), "- first item\n- ");

    // 2. Empty bullet item exit on Enter
    shell.feed_key(Key::Enter);
    assert_eq!(shell.editor_text(), "- first item\n");

    // 3. Task list continuation produces unchecked task
    shell.set_editor_text("- [x] completed task");
    shell.feed_key(Key::Char('A'));
    assert_eq!(shell.mode(), Mode::Insert);
    shell.feed_key(Key::Enter);
    assert_eq!(shell.editor_text(), "- [x] completed task\n- [ ] ");

    // 4. Ordered list auto-numbering and sibling renumbering
    shell.set_editor_text("1. first\n2. second\n");
    // Place cursor at the end of line 1 (after "first")
    if let Some(id) = shell.workspace().active_view() {
        let pos = truff_core::text::BytePos::new("1. first".len());
        shell
            .workspace_mut()
            .get_view_mut(id)
            .unwrap()
            .set_cursor(pos, pos);
    }
    shell.feed_key(Key::Char('i'));
    shell.feed_key(Key::Enter);
    assert_eq!(shell.editor_text(), "1. first\n2. \n2. second\n");

    // 5. Smart Tab indents list marker
    shell.set_editor_text("- item");
    shell.feed_key(Key::Char('i'));
    shell.feed_key(Key::Tab);
    assert!(
        shell.editor_text().starts_with("  - item")
            || shell.editor_text().starts_with("    - item")
            || shell.editor_text().starts_with("\t- item"),
        "Tab should indent list item, got {:?}",
        shell.editor_text()
    );

    // 6. Smart ShiftTab dedents list marker
    shell.feed_key(Key::ShiftTab);
    assert_eq!(shell.editor_text(), "- item");

    // 7. Single undo restores state before ShiftTab, second undo restores state before Tab
    shell.feed_key(Key::Esc);
    assert_eq!(shell.mode(), Mode::Normal);
    shell.feed_key(Key::Char('u'));
    assert_eq!(shell.editor_text(), "    - item");
    shell.feed_key(Key::Char('u'));
    assert_eq!(shell.editor_text(), "- item");
}

#[test]
fn test_tab_subitem_enter_reproduction() {
    use truff_core::vim::Key;
    let mut shell = Shell::new();
    shell.set_editor_text("- item");
    shell.feed_key(Key::Char('A'));
    println!("1. After A: {:?}", shell.editor_text());
    // Press Enter
    shell.feed_key(Key::Enter);
    println!("2. After Enter: {:?}", shell.editor_text());
    // Press Tab
    shell.feed_key(Key::Tab);
    println!("3. After Tab: {:?}", shell.editor_text());
    // Type "sub"
    for c in "sub".chars() {
        shell.feed_key(Key::Char(c));
    }
    println!("4. After typing sub: {:?}", shell.editor_text());
    // Press Enter
    shell.feed_key(Key::Enter);
    println!("5. After Enter: {:?}", shell.editor_text());
}
