use truff_core::navigation::{Move, RowId, RowItem, RowModel};
use truff_core::vim::Key;
use truff_ui::rows::VirtualRows;
use truff_ui::shell::{Focus, Shell, ShellAction};

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
