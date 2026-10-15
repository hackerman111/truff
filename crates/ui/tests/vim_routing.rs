use truff_core::vim::Key;
use truff_core::{Action, BytePos};
use truff_ui::editor::input::EditorInput;
use truff_ui::shell::{Focus, Shell, ShellAction};
fn keys(shell: &mut Shell, s: &str) {
    for c in s.chars() {
        shell.feed_key(Key::Char(c));
    }
}
#[test]
fn mixed_insert_commits_and_preedit_share_one_undo_group() {
    let mut shell = Shell::new();
    shell.set_editor_text("one two");
    keys(&mut shell, "ciw");
    shell
        .apply_input(EditorInput::InsertText("a".into()))
        .unwrap();
    let revision = shell.editor_buffer().unwrap().revision();
    shell
        .apply_input(EditorInput::ImePreedit {
            text: "compose".into(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(shell.editor_buffer().unwrap().revision(), revision);
    shell
        .apply_input(EditorInput::ImeCommit("日本".into()))
        .unwrap();
    shell.apply_input(EditorInput::Paste("👨‍👩‍👧‍👦".into())).unwrap();
    shell.feed_key(Key::Backspace);
    shell.feed_key(Key::Esc);
    assert_eq!(shell.editor_text(), "a日本 two");
    keys(&mut shell, "u");
    assert_eq!(shell.editor_text(), "one two");
    let action = shell.feed_key(Key::Char('.'));
    assert_eq!(shell.editor_text(), "a日本 two", "{action:?}");
}
#[test]
fn inactive_views_and_jump_history_rebase_for_edits_and_undo() {
    let mut shell = Shell::new();
    shell.set_editor_text("abcd");
    let first = shell.editor_view().unwrap().id();
    let id = shell.editor_buffer().unwrap().id();
    let second = shell.workspace_mut().create_view(id).unwrap();
    let v = shell.workspace_mut().get_view_mut(second).unwrap();
    v.set_cursor(BytePos::new(3), BytePos::new(3));
    v.set_cursor_with_jump(BytePos::new(4), BytePos::new(4));
    shell
        .workspace_mut()
        .dispatch(Action::FocusView(first))
        .unwrap();
    keys(&mut shell, "i");
    shell.commit_text("日本").unwrap();
    shell.feed_key(Key::Esc);
    let v = shell.workspace().get_view(second).unwrap();
    assert_eq!(v.head().get(), 10);
    let jump = shell
        .workspace_mut()
        .get_view_mut(second)
        .unwrap()
        .jump_history_mut()
        .jump_back()
        .unwrap();
    assert_eq!(jump.1.get(), 9);
    keys(&mut shell, "u");
    assert_eq!(shell.workspace().get_view(second).unwrap().head().get(), 4);
    shell.feed_key(Key::Ctrl('r'));
    assert_eq!(shell.workspace().get_view(second).unwrap().head().get(), 10);
    let changes = shell.take_buffer_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].buffer_id, id);
    assert!(!changes[0].invalidate_suffix);
}
#[test]
fn flash_pending_operator_uses_source_target_once() {
    let mut shell = Shell::new();
    shell.set_editor_text("one two three");
    keys(&mut shell, "d");
    shell.start_flash(0..13);
    let label = shell.flash().targets()[1].label.clone();
    keys(&mut shell, &label);
    assert_eq!(shell.editor_text(), "one  three");
    keys(&mut shell, "u");
    assert_eq!(shell.editor_text(), "one two three");
}
#[test]
fn shared_panel_counts_and_literal_filter() {
    let mut shell = Shell::new();
    shell.set_sidebar_files((0..20).map(|i| format!("note {i}")).collect());
    shell.set_focus(Focus::Sidebar);
    keys(&mut shell, "5j");
    assert_eq!(shell.sidebar_model().selected_index(), 5);
    keys(&mut shell, "gg");
    assert_eq!(shell.sidebar_model().selected_index(), 0);
    keys(&mut shell, "/note 1");
    shell.feed_key(Key::Enter);
    assert_eq!(shell.sidebar_model().visible_count(), 11);
    keys(&mut shell, "G");
    assert_eq!(shell.sidebar_model().selected_index(), 10);
    shell.feed_key(Key::Ctrl('w'));
    assert_eq!(
        shell.feed_key(Key::Char('l')),
        ShellAction::FocusChanged(Focus::Editor)
    );
}

#[test]
fn ime_surrounding_deletion_counts_scalars_and_undoes_with_insert() {
    let mut shell = Shell::new();
    shell.set_editor_text("aé\u{301}🙂z");
    keys(&mut shell, "i");
    shell
        .apply_input(EditorInput::SetSelection { anchor: 5, head: 5 })
        .unwrap();
    shell
        .apply_input(EditorInput::ImeDeleteSurrounding {
            before_chars: 1,
            after_chars: 1,
        })
        .unwrap();
    assert_eq!(shell.editor_text(), "aéz");
    shell.feed_key(Key::Esc);
    keys(&mut shell, "u");
    assert_eq!(shell.editor_text(), "aé\u{301}🙂z");
}
#[test]
fn flash_modes_select_canonical_objects_and_cancel_operator() {
    use truff_ui::overlays::FlashKind;
    for (kind, text, expected) in [
        (FlashKind::Sentence, "One. Two! Three.", "One. Three."),
        (FlashKind::Paragraph, "one\n\ntwo\n\nthree", "one\n\nthree"),
    ] {
        let mut shell = Shell::new();
        shell.set_editor_text(text);
        keys(&mut shell, "d");
        shell.start_flash_kind(0..text.len(), kind).unwrap();
        let label = shell.flash().targets()[1].label.clone();
        keys(&mut shell, &label);
        assert_eq!(shell.editor_text(), expected);
        keys(&mut shell, "u");
        assert_eq!(shell.editor_text(), text);
    }
    let mut shell = Shell::new();
    shell.set_editor_text("one two");
    keys(&mut shell, "d");
    shell.start_flash(0..7);
    shell.feed_key(Key::Esc);
    keys(&mut shell, "w");
    assert_eq!(shell.editor_text(), "one two");
}
#[test]
fn flash_uses_disjoint_windows_without_copying_source_envelope() {
    use truff_ui::overlays::FlashKind;
    let mut shell = Shell::new();
    let text = format!("alpha {} omega", "x".repeat(2 * 1024 * 1024));
    shell.set_editor_text(&text);
    shell
        .start_flash_ranges(&[0..5, text.len() - 5..text.len()], FlashKind::Word)
        .unwrap();
    assert_eq!(shell.flash().targets().len(), 2);
    assert_eq!(
        shell.flash().targets()[1].source_range,
        text.len() - 5..text.len()
    );
    assert!(
        shell
            .start_flash_kind(0..text.len(), FlashKind::Word)
            .is_err()
    );
}
#[test]
fn remapped_binding_retains_count_and_registry_context() {
    use truff_core::commands::CommandId;
    use truff_core::config::{KeyBinding, KeymapConfig};
    use truff_core::vim::Mode;
    let mut shell = Shell::new();
    shell.set_editor_text("a\nb\nc\nd");
    let config = KeymapConfig {
        bindings: vec![KeyBinding {
            mode: Mode::Normal,
            keys: vec![Key::Char('z'), Key::Char('j')],
            command: CommandId::new("motion.down"),
            description: "down".into(),
        }],
        ..Default::default()
    };
    shell.set_keymap(config).unwrap();
    keys(&mut shell, "2zj");
    assert_eq!(shell.editor_view().unwrap().head().get(), 4);
    shell.set_sidebar_files(vec!["sidebar_note.md".into()]);
    shell.set_focus(Focus::Sidebar);
    assert!(matches!(
        shell.execute_registered(CommandId::new("edit.insert"), String::new()),
        ShellAction::Error(_)
    ));
}

#[test]
fn flash_syntax_object_targets_ast_nodes_and_shortcuts() {
    use truff_core::commands::CommandId;
    use truff_ui::overlays::FlashKind;

    let mut shell = Shell::new();
    let text = "# Title\n\n- task 1\n- task 2\n\n```rust\nlet x = 42;\n```\n";
    shell.set_editor_text(text);

    // 1. Direct key 's' produces flash.jump_word in Normal Editor
    let action_s = shell.feed_key(Key::Char('s'));
    assert_eq!(
        action_s,
        ShellAction::RegisteredCommand {
            id: CommandId::from_static("flash.jump_word"),
            argument: String::new(),
        }
    );

    // 2. Direct key 'S' produces flash.select_syntax_object in Normal Editor
    let action_cap_s = shell.feed_key(Key::Char('S'));
    assert_eq!(
        action_cap_s,
        ShellAction::RegisteredCommand {
            id: CommandId::from_static("flash.select_syntax_object"),
            argument: String::new(),
        }
    );

    // 3. Activating SyntaxObject flash collects markdown AST targets
    #[allow(clippy::single_range_in_vec_init)]
    let target_ranges = [0..text.len()];
    shell
        .start_flash_ranges(&target_ranges, FlashKind::SyntaxObject)
        .unwrap();
    assert!(shell.flash().is_active());
    let targets = shell.flash().targets();
    assert!(
        !targets.is_empty(),
        "SyntaxObject flash must extract AST targets from MarkdownSnapshot"
    );
    let labels: Vec<_> = targets.iter().map(|t| t.label.clone()).collect();
    assert!(!labels[0].is_empty());

    // Selecting first target jumps/selects
    let label = labels[0].clone();
    keys(&mut shell, &label);
    assert!(!shell.flash().is_active());
}
