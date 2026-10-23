use egui::Event;
use truff_core::{
    commands::CommandId,
    config::{KeyBinding, KeymapConfig},
    vim::{Key, Mode},
};
use truff_ui::{
    app::Application,
    native::NativeApp,
    navigator::{NavKey, NavigatorAction, NavigatorMode},
    shell::{Shell, ShellAction},
};

fn frame(app: &mut NativeApp, ctx: &egui::Context, events: Vec<Event>) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    output
}

fn pressed(key: egui::Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn folders_preserve_parent_navigation() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application
        .shell
        .set_sidebar_files(vec!["docs/sub/n.md".into()]);
    let mut app = NativeApp::new(application, &ctx);
    app.open_navigator(NavigatorMode::Folders);
    assert_eq!(app.navigator.items[1].depth, 1);
    app.navigator.expand_or_child();
    app.navigator.selected_index = 1;
    app.navigator.collapse_or_parent();
    app.navigator.collapse_or_parent();
    assert_eq!(app.navigator.selected_index, 0);
}

#[test]
fn sidebar_folder_operations_report_unavailable_before_opening_a_dialog() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application
        .shell
        .set_sidebar_files(vec!["docs/n.md".into()]);
    application.shell.set_focus(truff_ui::shell::Focus::Sidebar);
    let mut app = NativeApp::new(application, &ctx);
    for key in ["r", "d"] {
        frame(&mut app, &ctx, vec![Event::Text(key.into())]);
        assert!(!app.dialogs.is_any_open());
        assert!(!app.application.file_operation_pending());
        assert!(
            app.application.status.contains("папками"),
            "{}",
            app.application.status
        );
    }
}

#[test]
fn leaving_search_preserves_text_before_escape_and_routes_the_tail_as_motion() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("unchanged\n");
    application
        .shell
        .set_sidebar_files(vec!["note-a.md".into(), "note-b.md".into()]);
    let mut app = NativeApp::new(application, &ctx);
    app.open_navigator(NavigatorMode::Files);
    frame(
        &mut app,
        &ctx,
        vec![
            Event::Text("/note".into()),
            pressed(egui::Key::Escape),
            Event::Text("j".into()),
        ],
    );
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.navigator.query, "note");
    assert!(!app.navigator.search_mode);
    assert_eq!(app.navigator.selected_index, 1);
    assert_eq!(app.application.shell.editor_text(), "unchanged\n");
}

#[test]
fn command_paste_is_committed_before_enter_in_the_same_batch() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("unchanged\n");
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![Event::Text(":".into())]);
    frame(
        &mut app,
        &ctx,
        vec![
            Event::Paste("panel.files".into()),
            pressed(egui::Key::Enter),
        ],
    );
    assert!(app.navigator.is_open());
    assert_eq!(app.navigator.mode, NavigatorMode::Files);
    assert_eq!(app.application.shell.editor_text(), "unchanged\n");
}

#[test]
fn palette_color_transfer_keeps_the_original_visual_selection() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("alpha beta\n");
    for ch in "vllll".chars() {
        application.shell.feed_key(Key::Char(ch));
    }
    let mut app = NativeApp::new(application, &ctx);
    app.open_navigator(NavigatorMode::Commands);
    app.handle_navigator_action(NavigatorAction::ExecuteCommand {
        command_id: CommandId::new("style.text_color"),
    });
    assert_eq!(app.navigator.mode, NavigatorMode::Colors);
    app.handle_navigator_action(app.navigator.primary_action());
    let text = app.application.shell.editor_text();
    assert!(text.contains("alpha</span> beta"), "{text}");
    app.application.shell.feed_key(Key::Char('u'));
    assert_eq!(app.application.shell.editor_text(), "alpha beta\n");
}

#[test]
fn color_picker_rejects_a_selection_after_its_buffer_changes() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("alpha\n");
    application.shell.feed_key(Key::Char('v'));
    let mut app = NativeApp::new(application, &ctx);
    app.open_navigator(NavigatorMode::Colors);
    app.application
        .shell
        .apply_transaction(truff_core::Transaction::insert(0, "new "))
        .unwrap();
    app.handle_navigator_action(app.navigator.primary_action());
    assert_eq!(app.application.shell.editor_text(), "new alpha\n");
    assert!(app.application.status.contains("Selection changed"));
}

#[test]
fn remapped_leader_is_executed_and_help_matches_the_binding() {
    let mut shell = Shell::new();
    shell
        .set_keymap(KeymapConfig {
            bindings: vec![KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char(' '), Key::Char('f')],
                command: CommandId::new("panel.sessions"),
                description: "Sessions".into(),
            }],
            ..Default::default()
        })
        .unwrap();
    shell.feed_key(Key::Char(' '));
    assert!(
        shell
            .which_key()
            .hints()
            .iter()
            .any(|hint| hint.description == "Sessions")
    );
    assert_eq!(
        shell.feed_key(Key::Char('f')),
        ShellAction::OpenNavigator(NavigatorMode::Sessions)
    );
}

#[test]
fn palette_executes_the_same_registered_edit_as_the_keyboard() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("first\nsecond\n");
    let mut app = NativeApp::new(application, &ctx);
    app.open_navigator(NavigatorMode::Commands);
    app.handle_navigator_action(NavigatorAction::ExecuteCommand {
        command_id: CommandId::new("edit.delete_line"),
    });
    assert_eq!(app.application.shell.editor_text(), "second\n");
    assert!(!app.navigator.is_open());
    app.application.shell.feed_key(Key::Char('u'));
    assert_eq!(app.application.shell.editor_text(), "first\nsecond\n");
}

#[test]
fn opening_navigator_mid_batch_does_not_replay_the_leader_or_touch_the_editor() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("unchanged\n");
    application
        .shell
        .set_sidebar_files((0..120).map(|i| format!("note-{i:03}.md")).collect());
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![Event::Text(" f100j".into())]);
    assert_eq!(app.navigator.selected_index, 100);
    assert!(
        app.navigator.preview_visible,
        "opening Space must not toggle preview"
    );
    assert_eq!(app.application.shell.editor_text(), "unchanged\n");
}

#[test]
fn navigator_insert_and_preview_shortcuts_follow_the_spec() {
    let mut state = truff_ui::navigator::NavigatorState::new();
    state.open_mode(NavigatorMode::Files, "vault", vec![]);
    state.handle_key_event(NavKey::Char('p'), false);
    assert!(!state.preview_visible);
    state.handle_key_event(NavKey::Char('i'), false);
    assert!(state.search_mode);
    state.handle_key_event(NavKey::Char('s'), false);
    assert_eq!(state.query, "s");
}

#[test]
fn actions_menu_distinguishes_file_and_folder_creation_keys() {
    let mut state = truff_ui::navigator::NavigatorState::new();
    state.open_mode(
        NavigatorMode::Files,
        "vault",
        vec![truff_ui::navigator::NavigatorItem::new_file(
            1,
            "n.md",
            "n.md".into(),
            false,
            0,
        )],
    );
    state.handle_key_event(NavKey::Char('?'), false);
    assert!(matches!(
        state.handle_key_event(NavKey::Char('A'), false),
        NavigatorAction::CreateFolder { .. }
    ));
}

#[test]
fn collapsed_tree_hides_descendants_and_h_moves_to_parent() {
    use truff_ui::navigator::{NavigatorItem, NavigatorState};
    let mut state = NavigatorState::new();
    state.open_mode(
        NavigatorMode::Files,
        "vault",
        vec![
            NavigatorItem::new_file(1, "docs", "docs".into(), true, 0),
            NavigatorItem::new_file(2, "sub", "docs/sub".into(), true, 1),
            NavigatorItem::new_file(3, "n.md", "docs/sub/n.md".into(), false, 2),
            NavigatorItem::new_file(4, "other.md", "other.md".into(), false, 0),
        ],
    );
    state.selected_index = 2;
    state.collapse_or_parent();
    assert_eq!(state.selected_item().unwrap().id, 2);
    state.collapse_or_parent();
    assert_eq!(state.visible_count(), 3);
    state.expand_or_child();
    assert_eq!(state.visible_count(), 4);
    state.set_query("n.md");
    assert_eq!(state.selected_item().unwrap().id, 3);
}

#[test]
fn navigator_cut_cannot_delete_underlying_text() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("unchanged\n");
    let mut app = NativeApp::new(application, &ctx);
    app.open_navigator(NavigatorMode::Files);
    let id = app.application.shell.workspace().active_view().unwrap();
    app.application
        .shell
        .workspace_mut()
        .get_view_mut(id)
        .unwrap()
        .set_cursor(
            truff_core::text::BytePos::new(0),
            truff_core::text::BytePos::new(4),
        );
    frame(&mut app, &ctx, vec![Event::Cut]);
    assert_eq!(app.application.shell.editor_text(), "unchanged\n");
}

#[test]
fn closing_navigator_mid_text_event_cannot_execute_editor_commands() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("unchanged\n");
    let mut app = NativeApp::new(application, &ctx);
    frame(
        &mut app,
        &ctx,
        vec![
            Event::Text(" f".into()),
            Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
            Event::Text("dd".into()),
        ],
    );
    assert_eq!(app.application.shell.editor_text(), "unchanged\n");
}

#[test]
fn session_activation_preserves_the_selected_view_of_a_shared_buffer() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("shared\n");
    let first = application.shell.workspace().active_view().unwrap();
    let buffer = application
        .shell
        .workspace()
        .get_view(first)
        .unwrap()
        .buffer_id();
    let second = application
        .shell
        .workspace_mut()
        .create_view(buffer)
        .unwrap();
    application
        .shell
        .workspace_mut()
        .get_view_mut(second)
        .unwrap()
        .set_scroll_top_line(7);
    application
        .shell
        .workspace_mut()
        .set_active_view(Some(first));
    let mut app = NativeApp::new(application, &ctx);
    app.open_navigator(NavigatorMode::Sessions);
    app.navigator.selected_index = 1;
    app.handle_navigator_action(app.navigator.primary_action());
    assert_eq!(
        app.application.shell.workspace().active_view(),
        Some(second)
    );
    assert_eq!(
        app.application
            .shell
            .workspace()
            .get_view(second)
            .unwrap()
            .scroll_top_line(),
        7
    );
}

#[test]
fn outline_uses_core_markdown_facts_including_setext_and_excluding_code() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application
        .shell
        .set_editor_text("Title\n=====\n\n```md\n# fake\n```\n\n## real\n");
    let mut app = NativeApp::new(application, &ctx);
    app.open_navigator(NavigatorMode::Outline);
    assert_eq!(app.navigator.items.len(), 2);
    assert!(app.navigator.items[0].display_title().contains("Title"));
    assert!(app.navigator.items[1].display_title().contains("real"));
}

#[test]
fn task_provider_preserves_core_check_state_and_jumps_to_the_task_line() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application
        .shell
        .set_editor_text("# Title\n\n- [ ] first\n- [x] done\n");
    let mut app = NativeApp::new(application, &ctx);
    app.open_navigator(NavigatorMode::Tasks);
    assert_eq!(app.navigator.items[0].display_title(), "first");
    assert!(matches!(
        app.navigator.items[1].payload,
        truff_ui::navigator::NavigatorItemPayload::Task {
            line: 4,
            checked: true
        }
    ));
    app.navigator.selected_index = 1;
    app.handle_navigator_action(app.navigator.primary_action());
    assert!(!app.navigator.is_open());
    assert_eq!(
        app.application.shell.editor_view().unwrap().head().get(),
        app.application
            .shell
            .editor_buffer()
            .unwrap()
            .byte_of_line(3)
    );
}

#[test]
fn navigator_tab_switches_provider_without_closing_the_overlay() {
    let ctx = egui::Context::default();
    let mut app = NativeApp::new(Application::new(), &ctx);
    app.open_navigator(NavigatorMode::Files);
    frame(
        &mut app,
        &ctx,
        vec![Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(app.navigator.is_open());
    assert_ne!(app.navigator.mode, NavigatorMode::Files);
}

#[test]
fn file_actions_keep_navigator_open_and_trash_waits_for_confirmation() {
    use truff_ui::navigator::{NavigatorItem, NavigatorState};
    let mut state = NavigatorState::new();
    state.open_mode(
        NavigatorMode::Files,
        "vault",
        vec![NavigatorItem::new_file(1, "a.md", "a.md".into(), false, 0)],
    );
    let action = state.handle_key_event(NavKey::Char('r'), false);
    assert!(state.is_open());
    assert_eq!(
        action,
        NavigatorAction::RenameFile {
            path: "a.md".into()
        }
    );
    let ctx = egui::Context::default();
    let mut app = NativeApp::new(Application::new(), &ctx);
    app.open_navigator(NavigatorMode::Files);
    app.handle_navigator_action(NavigatorAction::DeleteFile {
        path: "a.md".into(),
    });
    assert!(app.navigator.is_open());
    assert!(app.dialogs.is_any_open());
    assert!(!app.application.file_operation_pending());
}

#[test]
fn entering_search_mid_event_keeps_literal_text_without_replaying_the_prefix() {
    let ctx = egui::Context::default();
    let mut app = NativeApp::new(Application::new(), &ctx);
    app.open_navigator(NavigatorMode::Files);
    frame(&mut app, &ctx, vec![Event::Text("/session".into())]);
    assert_eq!(app.navigator.query, "session");
    assert!(app.navigator.search_mode);
}

#[test]
fn pinned_views_survive_close_others_and_closed_views_reopen() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("one\ntwo\nthree\n");
    let first = application.shell.workspace().active_view().unwrap();
    let buffer = application
        .shell
        .workspace()
        .get_view(first)
        .unwrap()
        .buffer_id();
    let second = application
        .shell
        .workspace_mut()
        .create_view(buffer)
        .unwrap();
    let third = application
        .shell
        .workspace_mut()
        .create_view(buffer)
        .unwrap();
    application
        .shell
        .workspace_mut()
        .get_view_mut(third)
        .unwrap()
        .set_cursor(truff_core::BytePos::new(4), truff_core::BytePos::new(4));
    let mut app = NativeApp::new(application, &ctx);
    app.open_navigator(NavigatorMode::Sessions);
    app.handle_navigator_action(NavigatorAction::PinView { view_id: second });
    app.handle_navigator_action(NavigatorAction::CloseOtherViews { view_id: first });
    assert!(app.application.shell.workspace().get_view(first).is_some());
    assert!(app.application.shell.workspace().get_view(second).is_some());
    assert!(app.application.shell.workspace().get_view(third).is_none());
    app.handle_navigator_action(NavigatorAction::ReopenView);
    let reopened = app.application.shell.workspace().active_view().unwrap();
    assert_eq!(
        app.application
            .shell
            .workspace()
            .get_view(reopened)
            .unwrap()
            .head()
            .get(),
        4
    );
    assert_eq!(
        app.application
            .shell
            .workspace()
            .get_view(reopened)
            .unwrap()
            .buffer_id(),
        buffer
    );
    assert!(app.navigator.is_open());
}

#[test]
fn content_search_matches_body_and_latest_query_wins() {
    use std::time::{Duration, Instant};
    let root = std::env::temp_dir().join(format!(
        "truff-p2-search-{}-{}",
        std::process::id(),
        truff_core::BufferId::next()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("alpha.md"), "# Alpha\nuniqueunicorn body\n").unwrap();
    std::fs::write(root.join("beta.md"), "# Beta\nuniquedragon body\n").unwrap();
    let mut application = Application::new();
    application.open_vault(&root).unwrap();
    let start = Instant::now();
    while application.needs_poll() {
        application.poll();
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "{}",
            application.status
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let ctx = egui::Context::default();
    let mut app = NativeApp::new(application, &ctx);
    app.open_navigator(NavigatorMode::Content);
    for _ in 0..10 {
        app.navigator.set_query("uniqueunicorn");
        frame(&mut app, &ctx, vec![]);
        app.navigator.set_query("uniquedragon");
        frame(&mut app, &ctx, vec![]);
    }
    let start = Instant::now();
    while app.navigator.items.is_empty() {
        frame(&mut app, &ctx, vec![]);
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "{}",
            app.navigator.context_title
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(app.navigator.items.len(), 1);
    assert!(
        matches!(&app.navigator.items[0].payload, truff_ui::navigator::NavigatorItemPayload::File { path, .. } if path == "beta.md")
    );
    assert!(
        app.navigator.items[0]
            .preview
            .as_deref()
            .unwrap()
            .contains("uniquedragon")
    );
    assert_eq!(
        app.navigator.visible_count(),
        1,
        "body match must survive presentation filtering"
    );
    app.navigator.close();
    app.application.shutdown().unwrap();
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn test_color_palette_expanded_style_highlight_callout_remove_and_undo() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    let text = "Styled note text line";
    application.shell.set_editor_text(text);
    let mut app = NativeApp::new(application, &ctx);

    // 1. Select "note" (bytes 7..=10) using VisualChar
    if let Some(view_id) = app.application.shell.workspace().active_view() {
        app.application
            .shell
            .workspace_mut()
            .get_view_mut(view_id)
            .unwrap()
            .set_cursor(truff_core::BytePos::new(7), truff_core::BytePos::new(10));
    }
    // Set shell mode to VisualChar so selection can be captured
    app.application.shell.feed_key(Key::Char('v'));

    // 2. Open Navigator in Colors mode
    app.open_navigator(NavigatorMode::Colors);
    assert!(app.navigator.is_open());
    assert_eq!(app.navigator.mode, NavigatorMode::Colors);

    // Verify all 4 categories exist in items
    let has_color = app.navigator.items.iter().any(|it| {
        matches!(
            it.payload,
            truff_ui::navigator::NavigatorItemPayload::Color { .. }
        )
    });
    let has_highlight = app.navigator.items.iter().any(|it| {
        matches!(
            it.payload,
            truff_ui::navigator::NavigatorItemPayload::Highlight { .. }
        )
    });
    let has_callout = app.navigator.items.iter().any(|it| {
        matches!(
            it.payload,
            truff_ui::navigator::NavigatorItemPayload::Callout { .. }
        )
    });
    let has_remove = app.navigator.items.iter().any(|it| {
        matches!(
            it.payload,
            truff_ui::navigator::NavigatorItemPayload::RemoveStyle
        )
    });
    assert!(has_color, "Colors provider must contain foreground colors");
    assert!(has_highlight, "Colors provider must contain highlights");
    assert!(has_callout, "Colors provider must contain callouts");
    assert!(has_remove, "Colors provider must contain remove_style");

    // 3. Apply Text Color (#BF616A)
    app.handle_navigator_action(NavigatorAction::ApplyColor { hex: "#BF616A" });
    assert!(!app.navigator.is_open());
    assert_eq!(
        app.application.shell.editor_text(),
        "Styled <span style=\"color:#BF616A\">note</span> text line"
    );
    // Single undo reverts
    app.application.shell.feed_key(Key::Esc);
    app.application.shell.feed_key(Key::Char('u'));
    assert_eq!(app.application.shell.editor_text(), text);

    // 4. Select "note" again and apply Highlight
    if let Some(view_id) = app.application.shell.workspace().active_view() {
        app.application
            .shell
            .workspace_mut()
            .get_view_mut(view_id)
            .unwrap()
            .set_cursor(truff_core::BytePos::new(7), truff_core::BytePos::new(10));
    }
    app.application.shell.feed_key(Key::Char('v'));
    app.open_navigator(NavigatorMode::Colors);
    app.handle_navigator_action(NavigatorAction::ApplyHighlight { hex: "#EBCB8B" });
    assert_eq!(
        app.application.shell.editor_text(),
        "Styled <mark style=\"background-color:#EBCB8B\">note</mark> text line"
    );
    // Single undo reverts
    app.application.shell.feed_key(Key::Esc);
    app.application.shell.feed_key(Key::Char('u'));
    assert_eq!(app.application.shell.editor_text(), text);

    // 5. Select line and apply Callout (Tip)
    if let Some(view_id) = app.application.shell.workspace().active_view() {
        app.application
            .shell
            .workspace_mut()
            .get_view_mut(view_id)
            .unwrap()
            .set_cursor(
                truff_core::BytePos::new(0),
                truff_core::BytePos::new(text.len()),
            );
    }
    app.application.shell.feed_key(Key::Char('V'));
    app.open_navigator(NavigatorMode::Colors);
    app.handle_navigator_action(NavigatorAction::ApplyCallout {
        kind: truff_core::markdown::CalloutKind::Tip,
    });
    assert_eq!(
        app.application.shell.editor_text(),
        "> [!tip]\n> Styled note text line"
    );
    // Single undo reverts
    app.application.shell.feed_key(Key::Esc);
    app.application.shell.feed_key(Key::Char('u'));
    assert_eq!(app.application.shell.editor_text(), text);

    // 6. Test RemoveStyle
    // First apply color
    if let Some(view_id) = app.application.shell.workspace().active_view() {
        app.application
            .shell
            .workspace_mut()
            .get_view_mut(view_id)
            .unwrap()
            .set_cursor(truff_core::BytePos::new(7), truff_core::BytePos::new(10));
    }
    app.application.shell.feed_key(Key::Char('v'));
    app.open_navigator(NavigatorMode::Colors);
    app.handle_navigator_action(NavigatorAction::ApplyColor { hex: "#BF616A" });
    let styled_len = app.application.shell.editor_text().len();

    // Now select the entire text and apply RemoveStyle
    if let Some(view_id) = app.application.shell.workspace().active_view() {
        app.application
            .shell
            .workspace_mut()
            .get_view_mut(view_id)
            .unwrap()
            .set_cursor(
                truff_core::BytePos::new(0),
                truff_core::BytePos::new(styled_len),
            );
    }
    app.application.shell.feed_key(Key::Char('V'));
    app.open_navigator(NavigatorMode::Colors);
    app.handle_navigator_action(NavigatorAction::RemoveStyle);
    assert_eq!(app.application.shell.editor_text(), text);
    // Single undo restores the span
    app.application.shell.feed_key(Key::Esc);
    app.application.shell.feed_key(Key::Char('u'));
    assert_eq!(
        app.application.shell.editor_text(),
        "Styled <span style=\"color:#BF616A\">note</span> text line"
    );
}
