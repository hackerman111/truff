use truff_ui::navigator::{NavigatorItem, NavigatorMode, NavigatorState};

#[test]
fn test_navigator_selection_model() {
    let mut state = NavigatorState::new();
    state.open_mode(
        NavigatorMode::Files,
        "Vault",
        vec![
            NavigatorItem::new_file(1, "a.md", "a.md".into(), false, 0),
            NavigatorItem::new_file(2, "b.md", "b.md".into(), false, 0),
            NavigatorItem::new_file(3, "c.md", "c.md".into(), false, 0),
        ],
    );

    assert!(!state.in_visual_mode());
    assert_eq!(state.selected_file_paths(), vec!["a.md"]);

    // Enter visual mode at index 0, move to index 2
    state.visual_anchor = Some(0);
    state.selected_index = 2;
    assert!(state.in_visual_mode());
    assert!(state.is_index_selected(0));
    assert!(state.is_index_selected(1));
    assert!(state.is_index_selected(2));
    assert_eq!(state.selected_file_paths(), vec!["a.md", "b.md", "c.md"]);

    // Toggle mark on index 1 and clear visual mode
    state.visual_anchor = None;
    state.marked_indices.insert(1);
    assert!(!state.in_visual_mode());
    assert!(state.is_index_selected(1));
    assert_eq!(state.selected_file_paths(), vec!["b.md"]);

    // Clear selection
    state.clear_selection();
    assert_eq!(state.marked_indices.len(), 0);
    assert!(state.visual_anchor.is_none());
    assert_eq!(state.selected_file_paths(), vec!["c.md"]);
}
