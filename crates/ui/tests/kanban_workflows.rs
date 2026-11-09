use std::time::{Duration, Instant};

use egui::Event;
use truff_core::markdown::kanban::{KanbanBoard, KanbanTaskStatus};
use truff_core::text::BufferId;
use truff_core::vim::Key;
use truff_ui::app::Application;
use truff_ui::native::NativeApp;

fn idle(app: &mut Application) {
    let start = Instant::now();
    while app.needs_poll() {
        app.poll();
        assert!(start.elapsed() < Duration::from_secs(15), "{}", app.status);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn frame(app: &mut NativeApp, ctx: &egui::Context, events: Vec<Event>) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 700.0),
            )),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    output
}

#[test]
fn space_b_opens_kanban_overlay_creates_tasks_links_file_and_jumps() {
    let vault = std::env::temp_dir().join(format!(
        "truff-test-kanban-overlay-{}-{}",
        std::process::id(),
        BufferId::next()
    ));
    std::fs::create_dir_all(vault.join("notes")).unwrap();
    std::fs::write(
        vault.join("notes/target.md"),
        "# Target Note\nContent of target note\n",
    )
    .unwrap();
    std::fs::write(
        vault.join("notes/current.md"),
        "# Current Note\nWorking here\n",
    )
    .unwrap();

    let mut application = Application::new();
    application.open_vault(&vault).unwrap();
    idle(&mut application);

    application.open_note("notes/current.md").unwrap();
    assert_eq!(
        application.active_relative_path().as_deref(),
        Some("notes/current.md")
    );

    let ctx = egui::Context::default();
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![]);

    // 1. Press Space then b -> opens overlay targeting kanban.md
    app.key(Key::Char(' '));
    app.key(Key::Char('b'));
    frame(&mut app, &ctx, vec![]);
    assert!(
        app.kanban.is_open(),
        "Kanban overlay must be open after Space b"
    );

    // 2. Press 'o', type "Проверить архитектуру", press Enter
    app.key(Key::Char('o'));
    for ch in "Проверить архитектуру".chars() {
        app.key(Key::Char(ch));
    }
    app.key(Key::Enter);
    frame(&mut app, &ctx, vec![]);

    // Verify kanban.md was created with columns and new card
    let kanban_content = std::fs::read_to_string(vault.join("kanban.md")).unwrap();
    assert!(
        kanban_content.contains("Проверить архитектуру"),
        "kanban.md must contain created card title"
    );
    let board = KanbanBoard::parse(&kanban_content);
    assert_eq!(board.columns.len(), 3);
    assert_eq!(board.columns[0].cards.len(), 1);
    assert_eq!(
        board.columns[0].cards[0].clean_title,
        "Проверить архитектуру"
    );
    assert_eq!(board.columns[0].cards[0].status, KanbanTaskStatus::Todo);

    // 3. Press 'f', type "target", press Enter -> card now has link to notes/target
    app.key(Key::Char('f'));
    for ch in "target".chars() {
        app.key(Key::Char(ch));
    }
    app.key(Key::Enter);
    frame(&mut app, &ctx, vec![]);

    let kanban_content_after_link = std::fs::read_to_string(vault.join("kanban.md")).unwrap();
    let board_after_link = KanbanBoard::parse(&kanban_content_after_link);
    assert_eq!(board_after_link.columns[0].cards[0].links.len(), 1);
    assert!(
        board_after_link.columns[0].cards[0].links[0]
            .target
            .contains("target")
    );

    // 4. Press 'L' -> move to column 1 ("В работе"), press 't' -> status cycles to InProgress
    app.key(Key::Char('L'));
    frame(&mut app, &ctx, vec![]);
    let kanban_moved = std::fs::read_to_string(vault.join("kanban.md")).unwrap();
    let board_moved = KanbanBoard::parse(&kanban_moved);
    assert_eq!(board_moved.columns[0].cards.len(), 0);
    assert_eq!(board_moved.columns[1].cards.len(), 1);

    app.key(Key::Char('t'));
    frame(&mut app, &ctx, vec![]);
    let kanban_cycled = std::fs::read_to_string(vault.join("kanban.md")).unwrap();
    let board_cycled = KanbanBoard::parse(&kanban_cycled);
    assert_eq!(
        board_cycled.columns[1].cards[0].status,
        KanbanTaskStatus::InProgress
    );

    // 5. Press 'u' -> status reverts to Todo (undo works inside overlay)
    app.key(Key::Char('u'));
    frame(&mut app, &ctx, vec![]);
    let kanban_undone = std::fs::read_to_string(vault.join("kanban.md")).unwrap();
    let board_undone = KanbanBoard::parse(&kanban_undone);
    assert_eq!(
        board_undone.columns[1].cards[0].status,
        KanbanTaskStatus::Todo
    );

    // 6. Press Enter on the linked card -> overlay closes and active editor note becomes notes/target.md!
    app.key(Key::Enter);
    frame(&mut app, &ctx, vec![]);
    assert!(
        !app.kanban.is_open(),
        "Overlay should close after link navigation"
    );
    assert_eq!(
        app.application.active_relative_path().as_deref(),
        Some("notes/target.md"),
        "Active editor note should switch to target note"
    );
}

#[test]
fn central_panel_renders_kanban_file_and_toggles_source_with_gs() {
    let vault = std::env::temp_dir().join(format!(
        "truff-test-kanban-central-{}-{}",
        std::process::id(),
        BufferId::next()
    ));
    std::fs::create_dir_all(&vault).unwrap();

    let sprint_content = "\
<!-- truff:kanban -->
# Спринт

## В работе
- [ ] Задача 1
- [ ] Задача 2

## Готово
";
    std::fs::write(vault.join("sprint.kanban.md"), sprint_content).unwrap();

    let mut application = Application::new();
    application.open_vault(&vault).unwrap();
    idle(&mut application);

    // Open sprint.kanban.md in editor
    application.open_note("sprint.kanban.md").unwrap();

    let ctx = egui::Context::default();
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![]);

    // 1. Central kanban view should be active
    assert!(
        app.is_central_kanban_active(),
        "sprint.kanban.md must activate central kanban view"
    );

    // 2. Select card 1 (Задача 2), move to column 1 with 'L', mark done with 'x'
    app.key(Key::Char('j'));
    app.key(Key::Char('L'));
    app.key(Key::Char('x'));
    frame(&mut app, &ctx, vec![]);

    let updated_text = std::fs::read_to_string(vault.join("sprint.kanban.md")).unwrap();
    let updated_board = KanbanBoard::parse(&updated_text);
    assert_eq!(updated_board.columns[0].cards.len(), 1);
    assert_eq!(updated_board.columns[0].cards[0].clean_title, "Задача 1");
    assert_eq!(updated_board.columns[1].cards.len(), 1);
    assert_eq!(updated_board.columns[1].cards[0].clean_title, "Задача 2");
    assert_eq!(
        updated_board.columns[1].cards[0].status,
        KanbanTaskStatus::Done
    );

    // 3. Send 'g' then 's' -> toggles to Source view
    app.key(Key::Char('g'));
    app.key(Key::Char('s'));
    frame(&mut app, &ctx, vec![]);

    assert!(
        !app.is_central_kanban_active(),
        "Pressing gs should toggle central view to Source mode"
    );

    // 4. Send 'g' then 's' again while in Source view -> toggles back to Board view!
    app.key(Key::Char('g'));
    app.key(Key::Char('s'));
    frame(&mut app, &ctx, vec![]);

    assert!(
        app.is_central_kanban_active(),
        "Pressing gs in Source mode should toggle back to Board view"
    );

    // 5. Toggle to Source with gs, then execute :kanban command to restore Board view
    app.key(Key::Char('g'));
    app.key(Key::Char('s'));
    frame(&mut app, &ctx, vec![]);
    assert!(!app.is_central_kanban_active());

    app.application.execute_command("kanban").unwrap();
    frame(&mut app, &ctx, vec![]);
    assert!(
        app.is_central_kanban_active(),
        ":kanban command must clear source override and restore Board view"
    );

    // 6. Test Cyrillic пы (gs equivalent) to toggle to Source view
    app.key(Key::Char('п'));
    app.key(Key::Char('ы'));
    frame(&mut app, &ctx, vec![]);
    assert!(
        !app.is_central_kanban_active(),
        "Cyrillic пы must toggle to Source view"
    );

    // 7. Test Cyrillic пы to toggle back to Board view
    app.key(Key::Char('п'));
    app.key(Key::Char('ы'));
    frame(&mut app, &ctx, vec![]);
    assert!(
        app.is_central_kanban_active(),
        "Cyrillic пы must toggle back to Board view"
    );
}

#[test]
fn modal_overlay_gs_and_cyrillic_toggles_source_view() {
    let vault = std::env::temp_dir().join(format!(
        "truff-test-kanban-overlay-gs-{}-{}",
        std::process::id(),
        BufferId::next()
    ));
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(
        vault.join("kanban.md"),
        "# Доска\n\n## План\n- [ ] Задача 1\n",
    )
    .unwrap();
    std::fs::write(vault.join("note.md"), "# Note\nText\n").unwrap();

    let mut application = Application::new();
    application.open_vault(&vault).unwrap();
    idle(&mut application);
    application.open_note("note.md").unwrap();

    let ctx = egui::Context::default();
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![]);

    // Open overlay with Space b
    app.key(Key::Char(' '));
    app.key(Key::Char('b'));
    frame(&mut app, &ctx, vec![]);
    assert!(app.kanban.is_open());
    assert!(!app.kanban.overlay_state.source_override);

    // gs toggles to source view inside modal
    app.key(Key::Char('g'));
    app.key(Key::Char('s'));
    frame(&mut app, &ctx, vec![]);
    assert!(app.kanban.overlay_state.source_override);

    // gs toggles back to board view inside modal
    app.key(Key::Char('g'));
    app.key(Key::Char('s'));
    frame(&mut app, &ctx, vec![]);
    assert!(!app.kanban.overlay_state.source_override);

    // Cyrillic пы toggles to source view
    app.key(Key::Char('п'));
    app.key(Key::Char('ы'));
    frame(&mut app, &ctx, vec![]);
    assert!(app.kanban.overlay_state.source_override);

    // Cyrillic пы toggles back
    app.key(Key::Char('п'));
    app.key(Key::Char('ы'));
    frame(&mut app, &ctx, vec![]);
    assert!(!app.kanban.overlay_state.source_override);
}

#[test]
fn kanban_undo_redo_persistence_across_mutations() {
    let vault = std::env::temp_dir().join(format!(
        "truff-test-kanban-undo-{}-{}",
        std::process::id(),
        BufferId::next()
    ));
    std::fs::create_dir_all(&vault).unwrap();

    let initial = "\
<!-- truff:kanban -->
# Доска

Важный пролог, который не должен пострадать.

## План
- [ ] Первая задача
- [ ] Вторая задача

## В работе

## Готово

Важный эпилог.
";
    std::fs::write(vault.join("board.kanban.md"), initial).unwrap();

    let mut application = Application::new();
    application.open_vault(&vault).unwrap();
    idle(&mut application);

    application.open_note("board.kanban.md").unwrap();

    let ctx = egui::Context::default();
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![]);

    assert!(app.is_central_kanban_active());

    // 1. Move card 0 to "В работе"
    app.key(Key::Char('L'));
    frame(&mut app, &ctx, vec![]);

    let step1_text = std::fs::read_to_string(vault.join("board.kanban.md")).unwrap();
    assert!(step1_text.contains("Важный пролог"));
    assert!(step1_text.contains("Важный эпилог"));
    let b1 = KanbanBoard::parse(&step1_text);
    assert_eq!(b1.columns[0].cards.len(), 1);
    assert_eq!(b1.columns[1].cards.len(), 1);

    // 2. Undo with 'u'
    app.key(Key::Char('u'));
    frame(&mut app, &ctx, vec![]);

    let undone_text = std::fs::read_to_string(vault.join("board.kanban.md")).unwrap();
    let b_undone = KanbanBoard::parse(&undone_text);
    assert_eq!(b_undone.columns[0].cards.len(), 2);
    assert_eq!(b_undone.columns[1].cards.len(), 0);
    assert!(undone_text.contains("Важный пролог"));
    assert!(undone_text.contains("Важный эпилог"));

    // 3. Redo with Ctrl-r
    app.key(Key::Ctrl('r'));
    frame(&mut app, &ctx, vec![]);

    let redone_text = std::fs::read_to_string(vault.join("board.kanban.md")).unwrap();
    let b_redone = KanbanBoard::parse(&redone_text);
    assert_eq!(b_redone.columns[0].cards.len(), 1);
    assert_eq!(b_redone.columns[1].cards.len(), 1);
    assert!(redone_text.contains("Важный пролог"));
    assert!(redone_text.contains("Важный эпилог"));
}

#[test]
fn colon_kanban_command_opens_overlay_with_path_and_escapes() {
    let vault = std::env::temp_dir().join(format!(
        "truff-test-kanban-colon-{}-{}",
        std::process::id(),
        BufferId::next()
    ));
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(
        vault.join("custom.kanban.md"),
        "<!-- truff:kanban -->\n# Custom Board\n## Колон 1\n- [ ] Card 1\n",
    )
    .unwrap();

    let mut application = Application::new();
    application.open_vault(&vault).unwrap();
    idle(&mut application);

    let ctx = egui::Context::default();
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![]);

    // Execute :kanban custom.kanban.md
    app.application
        .execute_command("kanban custom.kanban.md")
        .unwrap();
    frame(&mut app, &ctx, vec![]);

    assert!(app.kanban.is_open());
    assert_eq!(
        app.kanban.active_board_path.as_deref(),
        Some("custom.kanban.md")
    );

    // Press Esc -> closes overlay
    app.key(Key::Esc);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.kanban.is_open());

    // Execute :board alias
    app.application.execute_command("board").unwrap();
    frame(&mut app, &ctx, vec![]);
    assert!(app.kanban.is_open());

    // Press 'q' -> closes overlay
    app.key(Key::Char('q'));
    frame(&mut app, &ctx, vec![]);
    assert!(!app.kanban.is_open());
}

#[test]
fn multi_link_jump_picker_workflow() {
    let vault = std::env::temp_dir().join(format!(
        "truff-test-kanban-multi-link-{}-{}",
        std::process::id(),
        BufferId::next()
    ));
    std::fs::create_dir_all(vault.join("docs")).unwrap();
    std::fs::write(vault.join("docs/one.md"), "# One\n").unwrap();
    std::fs::write(vault.join("docs/two.md"), "# Two\n").unwrap();

    let board_text = "\
<!-- truff:kanban -->
# Доска

## Задачи
- [ ] Мультиссылка [[docs/one]] и [[docs/two]]
";
    std::fs::write(vault.join("tasks.kanban.md"), board_text).unwrap();

    let mut application = Application::new();
    application.open_vault(&vault).unwrap();
    idle(&mut application);

    application.open_note("tasks.kanban.md").unwrap();

    let ctx = egui::Context::default();
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![]);

    assert!(app.is_central_kanban_active());

    // Press Enter on card with 2 links -> opens LinkJumpPicker
    app.key(Key::Enter);
    frame(&mut app, &ctx, vec![]);

    // Press Down to select second link (docs/two), then Enter
    app.key(Key::Down);
    app.key(Key::Enter);
    frame(&mut app, &ctx, vec![]);

    assert_eq!(
        app.application.active_relative_path().as_deref(),
        Some("docs/two.md"),
        "Should jump to docs/two.md after selecting in LinkJumpPicker"
    );
}

#[test]
fn space_in_central_kanban_invokes_which_key_menu() {
    let vault = std::env::temp_dir().join(format!(
        "space_which_key_kanban_{}_{}",
        std::process::id(),
        BufferId::next()
    ));
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(
        vault.join("todo.kanban.md"),
        "<!-- truff:kanban -->\n# Задачи\n\n## План\n- [ ] Задача 1\n",
    )
    .unwrap();

    let mut application = Application::new();
    application.open_vault(&vault).unwrap();
    idle(&mut application);
    application.open_note("todo.kanban.md").unwrap();

    let ctx = egui::Context::default();
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![]);

    assert!(app.is_central_kanban_active());
    assert!(app.application.shell.which_key().hints().is_empty());

    // Press Space -> must NOT cycle card status, but invoke Which-Key menu!
    app.key(Key::Char(' '));
    frame(&mut app, &ctx, vec![]);

    assert!(
        !app.application.shell.which_key().hints().is_empty(),
        "Space in Kanban must invoke Which-Key menu prefix"
    );
    assert_eq!(
        app.application.shell.which_key().title(),
        "<Space> Навигатор"
    );

    // Card status was untouched
    let text = std::fs::read_to_string(vault.join("todo.kanban.md")).unwrap();
    assert!(text.contains("- [ ] Задача 1"));
}

#[test]
fn rename_standard_column_in_kanban() {
    let vault = std::env::temp_dir().join(format!(
        "rename_col_kanban_{}_{}",
        std::process::id(),
        BufferId::next()
    ));
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(
        vault.join("board.kanban.md"),
        "<!-- truff:kanban -->\n# Доска\n\n## План\n- [ ] Задача 1\n\n## В работе\n\n## Готово\n",
    )
    .unwrap();

    let mut application = Application::new();
    application.open_vault(&vault).unwrap();
    idle(&mut application);
    application.open_note("board.kanban.md").unwrap();

    let ctx = egui::Context::default();
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![]);

    assert!(app.is_central_kanban_active());

    // Press 'R' to rename standard column 0 ("План")
    app.key(Key::Char('R'));
    frame(&mut app, &ctx, vec![]);

    // Clear and type "Бэклог", then press Enter
    for _ in 0..10 {
        app.key(Key::Backspace);
    }
    for ch in "Бэклог".chars() {
        app.key(Key::Char(ch));
    }
    app.key(Key::Enter);
    frame(&mut app, &ctx, vec![]);

    let text = std::fs::read_to_string(vault.join("board.kanban.md")).unwrap();
    let parsed = KanbanBoard::parse(&text);
    assert_eq!(parsed.columns[0].title, "Бэклог");
    assert_eq!(parsed.columns[0].cards.len(), 1);
    assert_eq!(parsed.columns[0].cards[0].clean_title, "Задача 1");
    assert_eq!(parsed.columns[1].title, "В работе");
    assert_eq!(parsed.columns[2].title, "Готово");
}

#[test]
fn determine_board_location_in_kanban_panel() {
    let vault = std::env::temp_dir().join(format!(
        "custom_location_kanban_{}_{}",
        std::process::id(),
        BufferId::next()
    ));
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(
        vault.join("kanban.md"),
        "<!-- truff:kanban -->\n# Главный Канбан\n\n## План\n- [ ] Первая\n",
    )
    .unwrap();

    let mut application = Application::new();
    application.open_vault(&vault).unwrap();
    idle(&mut application);

    let ctx = egui::Context::default();
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![]);

    // 1. Open Kanban modal overlay targeting root kanban.md
    app.kanban.open_overlay("kanban.md", &mut app.application);
    frame(&mut app, &ctx, vec![]);
    assert!(app.kanban.is_open());
    assert_eq!(app.kanban.active_board_path.as_deref(), Some("kanban.md"));

    // 2. Relocate kanban.md to archive/old_kanban.kanban.md
    let relocate_intent = truff_ui::native::kanban::state::KanbanIntent::SetBoardPath {
        old_path: Some("kanban.md".to_string()),
        new_path: "archive/old_kanban.kanban.md".to_string(),
        is_new: false,
    };
    let buf_id = app
        .kanban
        .ensure_overlay_buffer("kanban.md", &mut app.application)
        .unwrap();
    app.kanban
        .handle_overlay_intent(relocate_intent, buf_id, &mut app.application);
    frame(&mut app, &ctx, vec![]);

    assert_eq!(
        app.kanban.active_board_path.as_deref(),
        Some("archive/old_kanban.kanban.md")
    );
    assert!(vault.join("archive/old_kanban.kanban.md").exists());
    assert!(!vault.join("kanban.md").exists());

    // 3. Create a brand new board at specific location tasks/sprint.kanban.md
    let create_intent = truff_ui::native::kanban::state::KanbanIntent::SetBoardPath {
        old_path: None,
        new_path: "tasks/sprint.kanban.md".to_string(),
        is_new: true,
    };
    let buf_id = app
        .kanban
        .ensure_overlay_buffer("archive/old_kanban.kanban.md", &mut app.application)
        .unwrap();
    app.kanban
        .handle_overlay_intent(create_intent, buf_id, &mut app.application);
    frame(&mut app, &ctx, vec![]);

    assert_eq!(
        app.kanban.active_board_path.as_deref(),
        Some("tasks/sprint.kanban.md")
    );
    assert!(vault.join("tasks/sprint.kanban.md").exists());
    let sprint_content = std::fs::read_to_string(vault.join("tasks/sprint.kanban.md")).unwrap();
    assert!(sprint_content.contains("<!-- truff:kanban -->"));
    assert!(sprint_content.contains("## План"));
}

#[test]
fn board_path_autocompletion_and_vim_motions_in_kanban_overlay() {
    let vault = std::env::temp_dir().join(format!(
        "kanban_path_ac_{}_{}",
        std::process::id(),
        BufferId::next()
    ));
    std::fs::create_dir_all(vault.join("projects")).unwrap();
    std::fs::create_dir_all(vault.join("notes")).unwrap();
    std::fs::write(
        vault.join("projects").join("tasks.kanban.md"),
        "<!-- truff:kanban -->\n# Tasks\n\n## План\n- [ ] Task 1\n",
    )
    .unwrap();
    std::fs::write(vault.join("notes").join("memo.md"), "# Memo\n").unwrap();
    std::fs::write(
        vault.join("kanban.md"),
        "<!-- truff:kanban -->\n# Main\n\n## План\n- [ ] Init\n",
    )
    .unwrap();

    let mut application = Application::new();
    application.open_vault(&vault).unwrap();
    idle(&mut application);

    let ctx = egui::Context::default();
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![]);

    // 1. Open Kanban modal overlay
    app.kanban.open_overlay("kanban.md", &mut app.application);
    frame(&mut app, &ctx, vec![]);
    assert!(app.kanban.is_open());

    // 2. Switch to BoardPathInput with empty query
    let vault_files = vec![
        "projects/tasks.kanban.md".to_string(),
        "notes/memo.md".to_string(),
        "kanban.md".to_string(),
    ];
    app.kanban.overlay_state.mode =
        truff_ui::native::kanban::state::KanbanInputMode::BoardPathInput {
            buffer: "proj".to_string(),
            is_new: true,
            path_ac: Some(truff_ui::native::kanban::picker::PathAutocomplete::new(
                "proj",
                &vault_files,
            )),
        };
    frame(&mut app, &ctx, vec![]);

    // Verify candidates are present and top candidate is projects/
    if let truff_ui::native::kanban::state::KanbanInputMode::BoardPathInput { path_ac, .. } =
        &app.kanban.overlay_state.mode
    {
        let ac = path_ac.as_ref().expect("path_ac must be active");
        assert!(!ac.candidates.is_empty());
        assert_eq!(ac.selected, 0);
        assert_eq!(ac.selected_candidate().unwrap().path, "projects/");
    } else {
        panic!("expected BoardPathInput mode");
    }

    // 3. Vim motion: Ctrl+j moves to next candidate
    app.key(Key::Ctrl('j'));
    frame(&mut app, &ctx, vec![]);
    if let truff_ui::native::kanban::state::KanbanInputMode::BoardPathInput { path_ac, .. } =
        &app.kanban.overlay_state.mode
    {
        assert_eq!(path_ac.as_ref().unwrap().selected, 1);
    }

    // 4. Vim motion: Ctrl+k moves back to prev candidate
    app.key(Key::Ctrl('k'));
    frame(&mut app, &ctx, vec![]);
    if let truff_ui::native::kanban::state::KanbanInputMode::BoardPathInput { path_ac, .. } =
        &app.kanban.overlay_state.mode
    {
        assert_eq!(path_ac.as_ref().unwrap().selected, 0);
    }

    // 5. Tab: completes current selection ("projects/")
    app.key(Key::Tab);
    frame(&mut app, &ctx, vec![]);
    if let truff_ui::native::kanban::state::KanbanInputMode::BoardPathInput { buffer, .. } =
        &app.kanban.overlay_state.mode
    {
        assert_eq!(buffer, "projects/");
    }

    // 6. Ctrl+w: deletes backward segment -> empty
    app.key(Key::Ctrl('w'));
    frame(&mut app, &ctx, vec![]);
    if let truff_ui::native::kanban::state::KanbanInputMode::BoardPathInput { buffer, .. } =
        &app.kanban.overlay_state.mode
    {
        assert_eq!(buffer, "");
    }

    // 7. Esc cancels autocomplete, then second Esc closes overlay or cancels input
    app.key(Key::Esc);
    frame(&mut app, &ctx, vec![]);
    app.key(Key::Esc);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(
        app.kanban.overlay_state.mode,
        truff_ui::native::kanban::state::KanbanInputMode::Normal
    );
}
