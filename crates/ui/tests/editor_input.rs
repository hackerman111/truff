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
        truff_ui::theme::Color::from_hex("#2d333f").unwrap()
    );
    assert_eq!(
        nord.bg_secondary,
        truff_ui::theme::Color::from_hex("#252a34").unwrap()
    );
    assert_eq!(
        nord.bg_tertiary,
        truff_ui::theme::Color::from_hex("#1b1e24").unwrap()
    );
    assert_eq!(
        nord.selection,
        truff_ui::theme::Color::from_hex("#394351").unwrap()
    );
    assert_eq!(
        nord.accent,
        truff_ui::theme::Color::from_hex("#88a9c1").unwrap()
    );
    assert_eq!(
        nord.border,
        truff_ui::theme::Color::from_hex("#333945").unwrap()
    );
    assert_eq!(
        nord.text_primary,
        truff_ui::theme::Color::from_hex("#e2e6ed").unwrap()
    );
    assert_eq!(
        nord.text_muted,
        truff_ui::theme::Color::from_hex("#8b95a5").unwrap()
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
    assert_eq!(nord.geometry.tab_height, 32.0);
    assert_eq!(nord.geometry.tree_row_height, 28.0);
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
