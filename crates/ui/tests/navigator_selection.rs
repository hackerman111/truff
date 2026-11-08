use truff_ui::navigator::{
    ClipboardOp, NavKey, NavigatorAction, NavigatorItem, NavigatorMode, NavigatorState,
};

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

#[test]
fn test_navigator_keyboard_motions_and_clipboard() {
    let mut state = NavigatorState::new();
    state.open_mode(
        NavigatorMode::Files,
        "Vault",
        vec![
            NavigatorItem::new_file(1, "a.md", "a.md".into(), false, 0),
            NavigatorItem::new_file(2, "b.md", "b.md".into(), false, 0),
            NavigatorItem::new_file(3, "c.md", "c.md".into(), false, 0),
            NavigatorItem::new_file(4, "folder", "folder".into(), true, 0),
        ],
    );

    // Press 'v' to enter visual mode
    state.handle_key_event(NavKey::Char('v'), false);
    assert!(state.in_visual_mode());

    // Press '2' then 'j' to move down by 2 -> selects a.md, b.md, c.md
    state.handle_key_event(NavKey::Char('2'), false);
    state.handle_key_event(NavKey::Char('j'), false);
    assert_eq!(state.selected_index, 2);
    assert_eq!(state.selected_file_paths(), vec!["a.md", "b.md", "c.md"]);

    // Press 'y' to yank -> copies into clipboard and exits visual mode
    state.handle_key_event(NavKey::Char('y'), false);
    assert!(!state.in_visual_mode());
    assert_eq!(state.clipboard.as_ref().unwrap().op, ClipboardOp::Copy);
    assert_eq!(
        state.clipboard.as_ref().unwrap().paths,
        vec!["a.md", "b.md", "c.md"]
    );

    // Move to folder (index 3) and press 'p' -> triggers paste into folder
    state.selected_index = 3;
    let action = state.handle_key_event(NavKey::Char('p'), false);
    match action {
        NavigatorAction::PasteFiles {
            target_dir,
            op,
            paths,
        } => {
            assert_eq!(target_dir, "folder");
            assert_eq!(op, ClipboardOp::Copy);
            assert_eq!(paths, vec!["a.md", "b.md", "c.md"]);
        }
        other => panic!("expected PasteFiles action, got {:?}", other),
    }

    // Press 'x' on index 3 -> marks index 3 and moves to next line
    state.selected_index = 0;
    state.handle_key_event(NavKey::Char('x'), false);
    assert!(state.marked_indices.contains(&0));
    assert_eq!(state.selected_index, 1);

    // Press 'Escape' -> clears selection without closing navigator
    let action = state.handle_key_event(NavKey::Escape, false);
    assert_eq!(action, NavigatorAction::None);
    assert!(!state.is_index_selected(0));

    // Next 'Escape' -> closes navigator
    let action = state.handle_key_event(NavKey::Escape, false);
    assert_eq!(action, NavigatorAction::Close);
}
