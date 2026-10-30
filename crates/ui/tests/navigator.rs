use truff_core::commands::CommandId;
use truff_core::text::BufferId;
use truff_ui::navigator::{NavKey, NavigatorAction, NavigatorItem, NavigatorMode, NavigatorState};

#[test]
fn test_navigator_initial_state_and_mode_setup() {
    let mut state = NavigatorState::new();
    assert!(!state.is_open());
    assert_eq!(state.visible_count(), 0);

    let items = vec![
        NavigatorItem::new_file(1, "Moch.md", "Moch.md".into(), false, 0),
        NavigatorItem::new_file(2, "docs", "docs".into(), true, 0),
        NavigatorItem::new_file(3, "Truff.md", "docs/Truff.md".into(), false, 1),
    ];

    state.open_mode(NavigatorMode::Files, "Vault / Test", items);
    assert!(state.is_open());
    assert_eq!(state.mode, NavigatorMode::Files);
    assert_eq!(state.context_title, "Vault / Test");
    assert_eq!(state.visible_count(), 3);
    assert_eq!(state.selected_index, 0);
    assert_eq!(state.selected_item().unwrap().display_title(), "Moch.md");
}

#[test]
fn test_navigator_fuzzy_search_and_score() {
    let mut state = NavigatorState::new();
    let items = vec![
        NavigatorItem::new_file(1, "Moch.md", "Moch.md".into(), false, 0),
        NavigatorItem::new_file(2, "Truff.md", "Truff.md".into(), false, 0),
        NavigatorItem::new_file(3, "Починить.md", "Починить.md".into(), false, 0),
        NavigatorItem::new_file(4, "readme.txt", "readme.txt".into(), false, 0),
    ];
    state.open_mode(NavigatorMode::Files, "Vault", items);

    // Filter by "moch"
    state.set_query("moch");
    assert_eq!(state.visible_count(), 1);
    assert_eq!(state.selected_item().unwrap().display_title(), "Moch.md");

    // Filter by "md" (should match all .md files)
    state.set_query("md");
    assert_eq!(state.visible_count(), 3);

    // Filter with no matches
    state.set_query("xyz_not_found");
    assert_eq!(state.visible_count(), 0);
    assert!(state.selected_item().is_none());

    // Clear query
    state.set_query("");
    assert_eq!(state.visible_count(), 4);
}

#[test]
fn test_navigator_vim_motion_navigation() {
    let mut state = NavigatorState::new();
    let items = vec![
        NavigatorItem::new_file(1, "1.md", "1.md".into(), false, 0),
        NavigatorItem::new_file(2, "2.md", "2.md".into(), false, 0),
        NavigatorItem::new_file(3, "3.md", "3.md".into(), false, 0),
    ];
    state.open_mode(NavigatorMode::Files, "Vault", items);

    // Initial at index 0
    assert_eq!(state.selected_index, 0);

    // j moves down
    state.handle_key_event(NavKey::Char('j'), false);
    assert_eq!(state.selected_index, 1);

    // j moves down again
    state.handle_key_event(NavKey::Char('j'), false);
    assert_eq!(state.selected_index, 2);

    // j at bottom stays at bottom
    state.handle_key_event(NavKey::Char('j'), false);
    assert_eq!(state.selected_index, 2);

    // k moves up
    state.handle_key_event(NavKey::Char('k'), false);
    assert_eq!(state.selected_index, 1);

    // G moves to bottom
    state.handle_key_event(NavKey::Char('G'), false);
    assert_eq!(state.selected_index, 2);
}

#[test]
fn test_navigator_mode_transitions_and_search_mode() {
    let mut state = NavigatorState::new();
    let items = vec![
        NavigatorItem::new_file(1, "Alpha.md", "Alpha.md".into(), false, 0),
        NavigatorItem::new_file(2, "Beta.md", "Beta.md".into(), false, 0),
    ];
    state.open_mode(NavigatorMode::Files, "Vault", items);
    assert!(!state.search_mode);

    // '/' enters search mode
    let action = state.handle_key_event(NavKey::Char('/'), false);
    assert_eq!(action, NavigatorAction::None);
    assert!(state.search_mode);

    // In search mode, letters type query characters
    state.handle_key_event(NavKey::Char('b'), false);
    state.handle_key_event(NavKey::Char('e'), false);
    state.handle_key_event(NavKey::Char('t'), false);
    assert_eq!(state.query, "bet");
    assert_eq!(state.visible_count(), 1);
    assert_eq!(state.selected_item().unwrap().display_title(), "Beta.md");

    // Esc in search mode exits search mode back to normal navigation, preserving query
    let action = state.handle_key_event(NavKey::Escape, false);
    assert_eq!(action, NavigatorAction::None);
    assert!(!state.search_mode);
    assert_eq!(state.query, "bet");

    // Esc in normal mode closes navigator
    let action = state.handle_key_event(NavKey::Escape, false);
    assert_eq!(action, NavigatorAction::Close);
    assert!(!state.is_open());
}

#[test]
fn test_navigator_quick_actions_and_context_menu() {
    let mut state = NavigatorState::new();
    let items = vec![NavigatorItem::new_file(
        1,
        "Notes.md",
        "work/Notes.md".into(),
        false,
        0,
    )];
    state.open_mode(NavigatorMode::Files, "Vault", items);

    // Primary action on Enter
    let act = state.primary_action();
    assert_eq!(
        act,
        NavigatorAction::OpenFile {
            path: "work/Notes.md".to_string()
        }
    );

    // '?' opens context actions menu
    state.handle_key_event(NavKey::Char('?'), false);
    assert!(state.actions_menu_open);

    let actions = state.context_actions();
    assert!(!actions.is_empty());
    assert!(actions.iter().any(|a| a.key == "a"));
    assert!(actions.iter().any(|a| a.key == "r"));
    assert!(actions.iter().any(|a| a.key == "d"));

    // Selecting 'r' for rename inside actions menu
    let act = state.handle_key_event(NavKey::Char('r'), false);
    assert_eq!(
        act,
        NavigatorAction::RenameFile {
            path: "work/Notes.md".to_string()
        }
    );
    assert!(!state.actions_menu_open);
}

#[test]
fn test_navigator_sessions_and_commands_mode() {
    let mut state = NavigatorState::new();
    let buf1 = BufferId::new(1);
    let buf2 = BufferId::new(2);

    let items = vec![
        NavigatorItem::new_buffer(
            1,
            buf1,
            "First.md".into(),
            Some("First.md".into()),
            false,
            25,
        ),
        NavigatorItem::new_buffer(
            2,
            buf2,
            "Second.md".into(),
            Some("Second.md".into()),
            true,
            50,
        ),
    ];

    state.open_mode(NavigatorMode::Sessions, "Вкладки", items);
    assert_eq!(state.visible_count(), 2);

    // Primary action switches buffer
    assert_eq!(
        state.primary_action(),
        NavigatorAction::SwitchBuffer { buffer_id: buf1 }
    );

    // 'd' in sessions mode closes the selected buffer
    let act = state.handle_key_event(NavKey::Char('d'), false);
    assert_eq!(act, NavigatorAction::CloseBuffer { buffer_id: buf1 });

    // Switch to commands mode
    let cmd_items = vec![NavigatorItem::new_command(
        10,
        CommandId::new("file.open"),
        "Открыть файл".into(),
        "Найти и открыть заметку".into(),
        "Файлы".into(),
        Some("Space f".into()),
    )];
    state.open_mode(NavigatorMode::Commands, "Команды", cmd_items);
    assert_eq!(state.visible_count(), 1);

    assert_eq!(
        state.primary_action(),
        NavigatorAction::ExecuteCommand {
            command_id: CommandId::new("file.open"),
        }
    );
}

#[test]
fn test_navigator_normal_mode_shortcuts_and_native_app_integration() {
    use truff_core::vim::Key;
    use truff_ui::app::Application;
    use truff_ui::native::NativeApp;
    use truff_ui::shell::Focus;

    let ctx = egui::Context::default();
    let mut app = NativeApp::new(Application::new(), &ctx);
    app.application
        .shell
        .set_editor_text("# Hello World\nSome content\n## Section 2\n");
    assert_eq!(app.application.shell.focus(), Focus::Editor);

    // 1. Space -> populates prefix hints with 300ms delay
    let action = app.application.shell.feed_key(Key::Char(' '));
    assert_eq!(action, truff_ui::shell::ShellAction::Incomplete);
    assert!(!app.application.shell.which_key().hints().is_empty());
    assert!(!app.application.shell.which_key().is_visible());

    // After 350ms delay, Which-Key becomes visible
    app.application.shell.which_key_mut().tick(350);
    assert!(app.application.shell.which_key().is_visible());

    let action = app.application.shell.feed_key(Key::Char('f'));
    assert_eq!(
        action,
        truff_ui::shell::ShellAction::OpenNavigator(NavigatorMode::Files)
    );
    app.open_navigator(NavigatorMode::Files);
    assert!(app.navigator.is_open());
    assert_eq!(app.navigator.mode, NavigatorMode::Files);
    assert_eq!(app.application.shell.focus(), Focus::Navigator);

    // Close navigator -> restores focus
    app.handle_navigator_action(NavigatorAction::Close);
    assert!(!app.navigator.is_open());
    assert_eq!(app.application.shell.focus(), Focus::Editor);

    // 2. Ctrl-P -> opens Commands navigator
    let action = app.application.shell.feed_key(Key::Ctrl('p'));
    assert_eq!(
        action,
        truff_ui::shell::ShellAction::OpenNavigator(NavigatorMode::Commands)
    );
    app.open_navigator(NavigatorMode::Commands);
    assert!(app.navigator.is_open());
    assert_eq!(app.navigator.mode, NavigatorMode::Commands);
    assert!(app.navigator.visible_count() > 0);
    // Ensure all commands have human-readable titles
    assert!(
        app.navigator
            .items
            .iter()
            .any(|item| item.display_title().contains("файл")
                || item.display_title().contains("Команд")
                || item.display_title().contains("Открыть"))
    );

    // Close
    app.handle_navigator_action(NavigatorAction::Close);
    assert_eq!(app.application.shell.focus(), Focus::Editor);

    // 3. Space Enter -> opens Outline/Backlinks/Tasks hub
    let _ = app.application.shell.feed_key(Key::Char(' '));
    let action = app.application.shell.feed_key(Key::Enter);
    assert_eq!(
        action,
        truff_ui::shell::ShellAction::OpenNavigator(NavigatorMode::Outline)
    );
    app.open_navigator(NavigatorMode::Outline);
    assert!(app.navigator.is_open());
    assert_eq!(app.navigator.mode, NavigatorMode::Outline);
    // Check heading extraction and section preview
    assert_eq!(app.navigator.visible_count(), 2);
    assert_eq!(app.navigator.items[0].display_title(), "# Hello World");
    assert!(
        app.navigator.items[0]
            .preview
            .as_deref()
            .unwrap()
            .contains("Some content")
    );
    assert_eq!(app.navigator.items[1].display_title(), "## Section 2");

    // Tab toggles directly between Outline and Links (Backlinks)
    let tab_action = app.navigator.handle_key_event(NavKey::Tab, false);
    assert_eq!(
        tab_action,
        NavigatorAction::SwitchProvider {
            mode: NavigatorMode::Links
        }
    );
    app.handle_navigator_action(tab_action);
    assert_eq!(app.navigator.mode, NavigatorMode::Links);

    let back_action = app.navigator.handle_key_event(NavKey::Tab, false);
    assert_eq!(
        back_action,
        NavigatorAction::SwitchProvider {
            mode: NavigatorMode::Outline
        }
    );
}
#[test]
fn file_labels_borrow_path_and_preserve_explicit_names() {
    use truff_ui::navigator::NavigatorItemPayload;
    for separator in ['/', '\\'] {
        let item =
            NavigatorItem::new_file(1, "字🦀.md", format!("Папка{separator}字🦀.md"), false, 0);
        let NavigatorItemPayload::File { path, .. } = &item.payload else {
            unreachable!()
        };
        assert_eq!(item.display_title(), "字🦀.md");
        assert_eq!(item.display_subtitle(), Some("Папка"));
        assert_eq!(
            item.display_title().as_ptr(),
            path["Папка".len() + 1..].as_ptr()
        );
        assert_eq!(item.display_subtitle().unwrap().as_ptr(), path.as_ptr());
    }
    let item = NavigatorItem::new_file(2, "folder/note.md", "folder/note.md".into(), false, 0);
    assert_eq!(item.display_title(), "folder/note.md");
    assert_eq!(item.display_subtitle(), Some("folder"));
    let folder = NavigatorItem::new_file(3, "folder", "folder".into(), true, 0);
    assert_eq!(folder.display_subtitle(), Some("Папка"));
    let root = NavigatorItem::new_file(4, "note.md", "note.md".into(), false, 0);
    assert_eq!(root.display_subtitle(), None);
}

#[test]
fn closing_navigator_releases_items_and_search_buffers() {
    let mut state = NavigatorState::new();
    state.open_mode(
        NavigatorMode::Files,
        "Vault",
        (0..10_000)
            .map(|id| NavigatorItem::new_file(id, "note.md", format!("{id}/note.md"), false, 0))
            .collect(),
    );
    state.set_query("note");
    state.move_bottom();
    state.close();
    assert!(!state.is_open());
    assert!(state.selected_item().is_none());
    assert_eq!(state.items.capacity(), 0);
    assert_eq!(state.filtered_indices.capacity(), 0);
    assert_eq!(state.query.capacity(), 0);
    state.open_mode(
        NavigatorMode::Files,
        "Another vault",
        vec![NavigatorItem::new_file(
            1,
            "new.md",
            "new.md".into(),
            false,
            0,
        )],
    );
    assert_eq!(state.selected_index, 0);
    assert_eq!(
        state.primary_action(),
        NavigatorAction::OpenFile {
            path: "new.md".into()
        }
    );
}

#[test]
fn test_space_enter_hub_three_task_categories_and_persistence() {
    use truff_ui::app::Application;
    use truff_ui::native::NativeApp;
    use truff_ui::navigator::{HubTaskCategory, HubTaskStatus};

    let vault_root_buf = std::env::temp_dir().join(format!(
        "truff_space_enter_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&vault_root_buf);
    let vault_root = vault_root_buf.as_path();
    let project_dir = vault_root.join("project_alpha");
    std::fs::create_dir_all(&project_dir).unwrap();

    // 1. Vault index.md (Общие задачи)
    std::fs::write(
        vault_root.join("index.md"),
        "# Убежище\n\n- [ ] Глобальная задача 1\n- [x] Глобальная задача 2\n",
    )
    .unwrap();

    // 2. Project index.md (Задачи проекта)
    std::fs::write(
        project_dir.join("index.md"),
        "# Проект Alpha\n\n- [ ] Задача проекта А\n",
    )
    .unwrap();

    // 3. Active note inside project_alpha with ```tasks fenced block (Задачи файла)
    let note_path = project_dir.join("spec.md");
    std::fs::write(
        &note_path,
        "# Раздел 1\nТекст раздела 1.\n\n## Раздел 2\nТекст раздела 2.\n\n```tasks\n- [ ] Задача файла 1\n```\n",
    )
    .unwrap();

    let ctx = egui::Context::default();
    let mut app = NativeApp::new(Application::new(), &ctx);
    app.application.open_vault(vault_root).unwrap();
    for _ in 0..50 {
        app.application.poll();
        if !app.application.needs_poll() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app.application.open_note("project_alpha/spec.md").unwrap();
    for _ in 0..50 {
        app.application.poll();
        if !app.application.needs_poll() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    // Open Space+Enter hub (Outline mode)
    app.open_navigator(NavigatorMode::Outline);
    assert_eq!(app.navigator.hub_tasks.lists[0].len(), 2); // Vault
    assert_eq!(app.navigator.hub_tasks.lists[1].len(), 1); // File (```tasks)
    assert_eq!(app.navigator.hub_tasks.lists[2].len(), 1); // Project

    // Press 'l' to focus tasks column
    app.navigator.handle_key_event(NavKey::Char('l'), false);
    assert!(app.navigator.hub_tasks.focus_tasks);
    assert_eq!(app.navigator.hub_tasks.category(), HubTaskCategory::Vault);

    // Cycle status of first Vault task (' ' -> InProgress '[/]')
    let act = app.navigator.handle_key_event(NavKey::Char(' '), false);
    app.handle_navigator_action(act);
    assert_eq!(
        app.navigator.hub_tasks.lists[0][0].status,
        HubTaskStatus::InProgress
    );
    let vault_disk = std::fs::read_to_string(vault_root.join("index.md")).unwrap();
    assert!(vault_disk.contains("- [/] Глобальная задача 1"));

    // Switch to category 2 (Задачи файла) via '2', add a new task via 'a'
    app.navigator.handle_key_event(NavKey::Char('2'), false);
    assert_eq!(app.navigator.hub_tasks.category(), HubTaskCategory::File);
    app.navigator.handle_key_event(NavKey::Char('a'), false);
    for ch in "Новая задача файла".chars() {
        app.navigator.handle_key_event(NavKey::Char(ch), false);
    }
    let add_act = app.navigator.handle_key_event(NavKey::Enter, false);
    app.handle_navigator_action(add_act);
    assert_eq!(app.navigator.hub_tasks.lists[1].len(), 2);
    assert_eq!(
        app.navigator.hub_tasks.lists[1][1].text,
        "Новая задача файла"
    );
    let buf_text = app.application.shell.editor_buffer().unwrap().as_str();
    assert!(buf_text.contains("```tasks\n- [ ] Задача файла 1\n- [ ] Новая задача файла\n```"));

    // Switch to category 3 (Задачи проекта) via '3', edit existing task via 'e', then delete via 'd'
    app.navigator.handle_key_event(NavKey::Char('3'), false);
    assert_eq!(app.navigator.hub_tasks.category(), HubTaskCategory::Project);
    app.navigator.handle_key_event(NavKey::Char('e'), false);
    app.navigator.handle_key_event(NavKey::Backspace, false);
    app.navigator.handle_key_event(NavKey::Char('Б'), false);
    let edit_act = app.navigator.handle_key_event(NavKey::Enter, false);
    app.handle_navigator_action(edit_act);
    let proj_disk = std::fs::read_to_string(project_dir.join("index.md")).unwrap();
    assert!(proj_disk.contains("- [ ] Задача проекта Б"));

    let del_act = app.navigator.handle_key_event(NavKey::Char('d'), false);
    app.handle_navigator_action(del_act);
    assert!(app.navigator.hub_tasks.lists[2].is_empty());
    let _ = std::fs::remove_dir_all(&vault_root_buf);
}
