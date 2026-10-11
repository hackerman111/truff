use truff_core::navigation::{Move, RowId, RowItem, RowModel};
use truff_core::vim::Key;
use truff_ui::rows::VirtualRows;
use truff_ui::shell::{Focus, Shell, ShellAction};

#[test]
fn requesting_write_does_not_mark_unsaved_text_clean() {
    use truff_core::vim::{InputContext, InputResult, VimState};
    use truff_core::workspace::{View, ViewId};
    let mut buffer = truff_core::Buffer::from_str("");
    let mut view = View::new(ViewId::next(), buffer.id());
    let mut vim = VimState::new();
    for key in [
        Key::Char('i'),
        Key::Char('x'),
        Key::Esc,
        Key::Char(':'),
        Key::Char('w'),
        Key::Enter,
    ] {
        vim.feed_key(
            key,
            InputContext::Editor {
                view_id: view.id(),
                buffer: &mut buffer,
                view: &mut view,
            },
        );
    }
    // A failed or still pending write must continue to block :q.
    for key in [Key::Char(':'), Key::Char('q'), Key::Enter] {
        let action = vim.feed_key(
            key,
            InputContext::Editor {
                view_id: view.id(),
                buffer: &mut buffer,
                view: &mut view,
            },
        );
        if key == Key::Enter {
            assert!(matches!(action, InputResult::Error(_)), "{action:?}");
        }
    }
}

#[test]
fn row_motion_shared() {
    let mut model = RowModel::new();
    let mut items = Vec::new();
    for i in 0..10 {
        items.push(RowItem {
            id: RowId::new(i as u64),
            title: format!("Item {i}"),
            depth: 0,
            is_folder: false,
            is_collapsed: false,
            data: None,
        });
    }
    model.set_items(items);

    assert_eq!(model.selected_index(), 0);

    // Test 1: j moves down
    model.move_selection(Move::Down(1));
    assert_eq!(model.selected_index(), 1);

    // Test 2: count j (e.g. 5j) moves down by 5
    model.move_selection(Move::Down(5));
    assert_eq!(model.selected_index(), 6);

    // Test 3: k moves up
    model.move_selection(Move::Up(2));
    assert_eq!(model.selected_index(), 4);

    // Test 4: G moves to last item
    model.move_selection(Move::Bottom);
    assert_eq!(model.selected_index(), 9);

    // Test 5: gg moves to first item
    model.move_selection(Move::Top);
    assert_eq!(model.selected_index(), 0);

    // Test 6: Folder collapse preserves valid selection
    let tree_items = vec![
        RowItem {
            id: RowId::new(100),
            title: "Folder A".to_string(),
            depth: 0,
            is_folder: true,
            is_collapsed: false,
            data: None,
        },
        RowItem {
            id: RowId::new(101),
            title: "File A1".to_string(),
            depth: 1,
            is_folder: false,
            is_collapsed: false,
            data: None,
        },
        RowItem {
            id: RowId::new(102),
            title: "Folder B".to_string(),
            depth: 0,
            is_folder: true,
            is_collapsed: false,
            data: None,
        },
    ];
    model.set_items(tree_items);

    // Select child File A1 (index 1)
    model.move_selection(Move::Down(1));
    assert_eq!(model.selected_row().unwrap().id, RowId::new(101));

    // Collapse Folder A (id 100) -> selection must adjust to Folder A (index 0)
    model.collapse(RowId::new(100));
    assert_eq!(model.visible_count(), 2); // Folder A and Folder B
    assert_eq!(model.selected_index(), 0);
    assert_eq!(model.selected_row().unwrap().id, RowId::new(100));
}

#[test]
fn viewport_bounded() {
    let total_items = 100_000;
    let row_height = 24.0;
    let viewport_height = 600.0; // 25 rows visible on screen

    let mut virtual_rows = VirtualRows::new(row_height);
    virtual_rows.set_total_count(total_items);
    virtual_rows.set_viewport_size(800.0, viewport_height);
    virtual_rows.set_scroll_offset(0.0);

    // Visible rows: 25. With 1 viewport buffer on each side: ~75 active widgets
    let initial_widget_count = virtual_rows.active_widget_count();
    assert!(
        initial_widget_count < 100,
        "Active widget count ({initial_widget_count}) must be bounded by viewport, not {total_items}"
    );

    // Scroll deep into the list (e.g. row 50,000)
    virtual_rows.set_scroll_offset(50_000.0 * row_height);
    let scrolled_widget_count = virtual_rows.active_widget_count();
    assert!(
        scrolled_widget_count < 100,
        "Scrolled active widget count ({scrolled_widget_count}) must remain bounded"
    );

    // Verify rendered range covers target scroll position
    let range = virtual_rows.rendered_range();
    assert!(range.contains(&50_000));
    assert!(range.end <= total_items);
}

#[test]
fn focus_routes_once() {
    let mut shell = Shell::new();
    shell.set_sidebar_files(vec!["note1.md".into(), "note2.md".into()]);
    shell.set_editor_text("first line\nsecond line\n");

    // 1. Focus on Editor
    shell.set_focus(Focus::Editor);
    assert_eq!(shell.focus(), Focus::Editor);

    // Press 'd' 'd' -> should delete text line in editor, sidebar unaffected
    let action1 = shell.feed_key(Key::Char('d'));
    assert_eq!(action1, ShellAction::Incomplete);
    let action2 = shell.feed_key(Key::Char('d'));
    assert_eq!(action2, ShellAction::EditorEdit);
    assert_eq!(shell.editor_text(), "second line\n");
    assert_eq!(shell.sidebar_files(), vec!["note1.md", "note2.md"]);

    // 2. Switch focus to Sidebar
    shell.set_focus(Focus::Sidebar);
    assert_eq!(shell.focus(), Focus::Sidebar);

    // In Sidebar, pressing 'd' deletes the selected file in sidebar, editor unaffected
    let action3 = shell.feed_key(Key::Char('d'));
    assert_eq!(action3, ShellAction::SidebarDeleteFile("note1.md".into()));
    assert_eq!(shell.editor_text(), "second line\n"); // editor unchanged!
}

#[test]
fn cannot_focus_nonexistent_right_panel_and_can_write() {
    let mut shell = Shell::new();
    shell.set_editor_text("initial text\n");
    assert_eq!(shell.focus(), Focus::Editor);

    // Try navigating right with <C-w>l when no right panel exists
    shell.feed_key(Key::Ctrl('w'));
    let action = shell.feed_key(Key::Char('l'));
    assert_eq!(action, ShellAction::FocusChanged(Focus::Editor));
    assert_eq!(shell.focus(), Focus::Editor);

    // Typing 'i' must work and allow editing because we remained in Editor
    let i_action = shell.feed_key(Key::Char('i'));
    assert_ne!(
        i_action,
        ShellAction::Error("Editor target unavailable".into())
    );
    assert_eq!(shell.mode(), truff_core::vim::Mode::Insert);
}

#[test]
fn cannot_focus_nonexistent_sidebar_and_can_write() {
    let mut shell = Shell::new();
    shell.set_editor_text("hello\n");
    assert_eq!(shell.focus(), Focus::Editor);

    // Try navigating left with <C-w>h when sidebar has no files and is not available
    shell.feed_key(Key::Ctrl('w'));
    let action = shell.feed_key(Key::Char('h'));
    assert_eq!(action, ShellAction::FocusChanged(Focus::Editor));
    assert_eq!(shell.focus(), Focus::Editor);

    // Directly trying to set focus to nonexistent Sidebar or RightPanel coerces to Editor
    shell.set_focus(Focus::RightPanel);
    assert_eq!(shell.focus(), Focus::Editor);
    shell.set_focus(Focus::Sidebar);
    assert_eq!(shell.focus(), Focus::Editor);
}

#[test]
fn pressing_insert_or_esc_in_panel_restores_editor_focus() {
    let mut shell = Shell::new();
    shell.set_sidebar_files(vec!["note1.md".into()]);
    shell.set_editor_text("existing text\n");

    shell.set_focus(Focus::Sidebar);
    assert_eq!(shell.focus(), Focus::Sidebar);

    // Pressing 'i' in sidebar switches focus to Editor and enters insert mode
    let action = shell.feed_key(Key::Char('i'));
    assert_eq!(shell.focus(), Focus::Editor);
    assert_eq!(shell.mode(), truff_core::vim::Mode::Insert);
    assert_ne!(
        action,
        ShellAction::Error("Editor target unavailable".into())
    );

    // Pressing Esc exits insert mode
    shell.feed_key(Key::Esc);
    assert_eq!(shell.mode(), truff_core::vim::Mode::Normal);

    // Switch back to Sidebar, pressing Esc returns focus to Editor
    shell.set_focus(Focus::Sidebar);
    assert_eq!(shell.focus(), Focus::Sidebar);
    let esc_action = shell.feed_key(Key::Esc);
    assert_eq!(esc_action, ShellAction::FocusChanged(Focus::Editor));
    assert_eq!(shell.focus(), Focus::Editor);
}

#[test]
fn ctrl_w_w_cycles_between_available_panes() {
    let mut shell = Shell::new();
    assert_eq!(shell.focus(), Focus::Editor);

    // 1. With only Editor available:
    shell.feed_key(Key::Ctrl('w'));
    let action = shell.feed_key(Key::Char('w'));
    assert_eq!(action, ShellAction::FocusChanged(Focus::Editor));
    assert_eq!(shell.focus(), Focus::Editor);

    // 2. With Sidebar available:
    shell.set_sidebar_files(vec!["note.md".into()]);
    shell.feed_key(Key::Ctrl('w'));
    let action = shell.feed_key(Key::Char('w'));
    assert_eq!(action, ShellAction::FocusChanged(Focus::Sidebar));
    assert_eq!(shell.focus(), Focus::Sidebar);

    shell.feed_key(Key::Ctrl('w'));
    let action = shell.feed_key(Key::Char('w'));
    assert_eq!(action, ShellAction::FocusChanged(Focus::Editor));
    assert_eq!(shell.focus(), Focus::Editor);

    // 3. With both Sidebar and RightPanel available:
    shell.set_right_panel_available(true);

    // From Editor -> RightPanel
    shell.feed_key(Key::Ctrl('w'));
    let action = shell.feed_key(Key::Char('w'));
    assert_eq!(action, ShellAction::FocusChanged(Focus::RightPanel));
    assert_eq!(shell.focus(), Focus::RightPanel);

    // From RightPanel -> Sidebar
    shell.feed_key(Key::Ctrl('w'));
    let action = shell.feed_key(Key::Char('w'));
    assert_eq!(action, ShellAction::FocusChanged(Focus::Sidebar));
    assert_eq!(shell.focus(), Focus::Sidebar);

    // From Sidebar -> Editor
    shell.feed_key(Key::Ctrl('w'));
    let action = shell.feed_key(Key::Char('w'));
    assert_eq!(action, ShellAction::FocusChanged(Focus::Editor));
    assert_eq!(shell.focus(), Focus::Editor);
}

#[test]
fn empty_backlinks_reports_message_and_restores_editor() {
    use truff_ui::app::Application;

    let tmp = std::env::temp_dir().join(format!("truff-test-backlinks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let mut app = Application::new();
    app.create_and_open_vault(&tmp, Some("note.md")).unwrap();

    // Open the note
    app.open_note("note.md").unwrap();
    assert_eq!(app.shell.focus(), Focus::Editor);

    // Run :backlinks command for note without backlinks
    app.execute_command("backlinks").unwrap();
    for _ in 0..50 {
        app.poll();
        if app.status == "No backlinks found" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(app.status, "No backlinks found");
    assert_eq!(app.shell.focus(), Focus::Editor);
    assert!(!app.shell.has_right_panel());

    let _ = app.shutdown();
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_sidebar_hierarchical_folders_and_auto_expansion() {
    let mut shell = Shell::new();
    shell.set_sidebar_available(true);
    shell.set_sidebar_files(vec![
        "docs/guide.md".into(),
        "docs/api/endpoints.md".into(),
        "Welcome.md".into(),
    ]);

    let model = shell.sidebar_model();
    // Initially: root level has "docs" (collapsed) and "Welcome.md"
    let visible_titles: Vec<&str> = model
        .visible_rows()
        .iter()
        .map(|r| r.title.as_str())
        .collect();
    assert_eq!(visible_titles, vec!["docs", "Welcome.md"]);
    assert!(model.visible_rows()[0].is_folder);
    assert!(!model.visible_rows()[1].is_folder);

    // Auto-reveal: selecting a nested file expands parent folders
    shell.select_sidebar_file("docs/api/endpoints.md");
    let model = shell.sidebar_model();
    let visible_titles: Vec<&str> = model
        .visible_rows()
        .iter()
        .map(|r| r.title.as_str())
        .collect();
    assert_eq!(
        visible_titles,
        vec!["docs", "api", "endpoints.md", "guide.md", "Welcome.md"]
    );
}

#[test]
fn test_active_vs_selected_file_state() {
    let mut app = truff_ui::app::Application::new();
    let target = std::env::temp_dir().join(format!("truff_tree_state_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&target);
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("file1.md"), "# File 1").unwrap();
    std::fs::write(target.join("file2.md"), "# File 2").unwrap();

    app.open_vault(&target).unwrap();
    for _ in 0..50 {
        app.poll();
        if app.shell.sidebar_model().visible_count() >= 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    app.open_note("file1.md").unwrap();

    // file1.md is active in editor
    assert_eq!(app.active_relative_path().as_deref(), Some("file1.md"));

    // Navigate sidebar selection to file2.md without opening it
    app.shell.select_sidebar_file("file2.md");
    let model = app.shell.sidebar_model();
    let selected_row = model.selected_row().unwrap();
    assert_eq!(selected_row.title, "file2.md");

    // Active file is distinct from selected row
    assert_ne!(
        app.active_relative_path().as_deref(),
        selected_row.data.as_deref()
    );

    // Verify is_path_dirty
    assert!(!app.is_path_dirty("file1.md"));

    let _ = app.shutdown();
    let _ = std::fs::remove_dir_all(target);
}
