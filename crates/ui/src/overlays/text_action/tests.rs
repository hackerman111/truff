use super::*;
use truff_core::{BufferId, text::Buffer};

#[test]
fn test_menu_initial_state_and_navigation_to_color() {
    let mut menu = TextActionOverlay::new();
    assert!(!menu.is_open());

    menu.open(Some("Sample text".into()), Some([100.0, 150.0]));
    assert!(menu.is_open());
    assert_eq!(
        menu.mode(),
        &TextActionMode::Root {
            has_selection: true
        }
    );

    let action = menu.handle_key(Key::Char('c'));
    assert_eq!(action, TextActionResponse::None);
    assert!(matches!(
        menu.mode(),
        TextActionMode::ColorSelect {
            is_highlight: false
        }
    ));

    assert_eq!(menu.selected_color_index(), 0);
    menu.handle_key(Key::Right);
    assert_eq!(menu.selected_color_index(), 1);

    let action = menu.handle_key(Key::Enter);
    assert!(matches!(action, TextActionResponse::ApplyColor { hex } if hex == PALETTE[1].hex));
    assert!(!menu.is_open());
}

#[test]
fn test_menu_without_selection_rejects_styles() {
    let mut menu = TextActionOverlay::new();
    menu.open(None, None);
    assert_eq!(
        menu.mode(),
        &TextActionMode::Root {
            has_selection: false
        }
    );

    // c, h, x should not change mode when there is no selection
    assert_eq!(menu.handle_key(Key::Char('c')), TextActionResponse::None);
    assert_eq!(
        menu.mode(),
        &TextActionMode::Root {
            has_selection: false
        }
    );

    assert_eq!(menu.handle_key(Key::Char('h')), TextActionResponse::None);
    assert_eq!(
        menu.mode(),
        &TextActionMode::Root {
            has_selection: false
        }
    );

    assert_eq!(menu.handle_key(Key::Char('x')), TextActionResponse::None);
    assert_eq!(
        menu.mode(),
        &TextActionMode::Root {
            has_selection: false
        }
    );
}

#[test]
fn test_menu_with_selection_n_opens_link_input_with_title() {
    let mut menu = TextActionOverlay::new();
    menu.open(Some("My Note".into()), None);

    let action = menu.handle_key(Key::Char('n'));
    assert_eq!(action, TextActionResponse::None);
    assert!(matches!(
        menu.mode(),
        TextActionMode::LinkInput {
            is_url: false,
            has_selection: true,
            title_input,
            active_field: 1,
            ..
        } if title_input == "My Note"
    ));

    // Type target note name
    menu.handle_key(Key::Char('T'));
    menu.handle_key(Key::Char('a'));
    menu.handle_key(Key::Char('r'));
    menu.handle_key(Key::Char('g'));
    menu.handle_key(Key::Char('e'));
    menu.handle_key(Key::Char('t'));

    let action = menu.handle_key(Key::Enter);
    assert_eq!(
        action,
        TextActionResponse::ApplyLink {
            is_url: false,
            target: "Target".into(),
            title: Some("My Note".into()),
        }
    );
    assert!(!menu.is_open());
}

#[test]
fn test_menu_link_input_autocompletion_navigation() {
    let mut menu = TextActionOverlay::new();
    menu.open(Some("Text".into()), None);
    menu.handle_key(Key::Char('n'));

    if let TextActionMode::LinkInput { suggestions, .. } = menu.mode_mut() {
        *suggestions = vec!["Alpha".into(), "Beta".into(), "Gamma".into()];
    }

    // Down arrow selects first suggestion "Alpha"
    menu.handle_key(Key::Down);
    if let TextActionMode::LinkInput {
        selected_suggestion,
        ..
    } = menu.mode()
    {
        assert_eq!(*selected_suggestion, Some(0));
    }

    // Down arrow selects second suggestion "Beta"
    menu.handle_key(Key::Down);
    if let TextActionMode::LinkInput {
        selected_suggestion,
        ..
    } = menu.mode()
    {
        assert_eq!(*selected_suggestion, Some(1));
    }

    // Tab completes "Beta" into target_input
    menu.handle_key(Key::Tab);
    if let TextActionMode::LinkInput {
        target_input,
        selected_suggestion,
        ..
    } = menu.mode()
    {
        assert_eq!(target_input, "Beta");
        assert_eq!(*selected_suggestion, None);
    }

    // Enter submits link with target "Beta" and title "Text"
    let action = menu.handle_key(Key::Enter);
    assert_eq!(
        action,
        TextActionResponse::ApplyLink {
            is_url: false,
            target: "Beta".into(),
            title: Some("Text".into()),
        }
    );
    assert!(!menu.is_open());
}

#[test]
fn test_menu_with_selection_u_opens_single_url_input() {
    let mut menu = TextActionOverlay::new();
    menu.open(Some("My Site".into()), None);

    let action = menu.handle_key(Key::Char('u'));
    assert_eq!(action, TextActionResponse::None);
    assert!(matches!(
        menu.mode(),
        TextActionMode::LinkInput {
            is_url: true,
            has_selection: true,
            title_input,
            active_field: 1,
            ..
        } if title_input == "My Site"
    ));

    // In LinkInput, simulate entering url
    if let TextActionMode::LinkInput { target_input, .. } = menu.mode_mut() {
        *target_input = "https://example.com".into();
    }

    let action = menu.handle_key(Key::Enter);
    assert_eq!(
        action,
        TextActionResponse::ApplyLink {
            is_url: true,
            target: "https://example.com".into(),
            title: Some("My Site".into()),
        }
    );
    assert!(!menu.is_open());
}

#[test]
fn test_menu_without_selection_n_and_u_open_dual_inputs() {
    let mut menu = TextActionOverlay::new();
    menu.open(None, None);

    menu.handle_key(Key::Char('n'));
    assert!(matches!(
        menu.mode(),
        TextActionMode::LinkInput {
            is_url: false,
            has_selection: false,
            active_field: 0,
            ..
        }
    ));

    // Tab toggles field
    menu.handle_key(Key::Tab);
    assert!(matches!(
        menu.mode(),
        TextActionMode::LinkInput {
            active_field: 1,
            ..
        }
    ));
    menu.handle_key(Key::Tab);
    assert!(matches!(
        menu.mode(),
        TextActionMode::LinkInput {
            active_field: 0,
            ..
        }
    ));

    // Set inputs
    if let TextActionMode::LinkInput {
        target_input,
        title_input,
        ..
    } = menu.mode_mut()
    {
        *target_input = "TargetNote".into();
        *title_input = "CustomTitle".into();
    }

    let action = menu.handle_key(Key::Enter);
    assert_eq!(
        action,
        TextActionResponse::ApplyLink {
            is_url: false,
            target: "TargetNote".into(),
            title: Some("CustomTitle".into()),
        }
    );
    assert!(!menu.is_open());
}

#[test]
fn test_menu_color_direct_mnemonic_key() {
    let mut menu = TextActionOverlay::new();
    menu.open(Some("Sample text".into()), Some([50.0, 50.0]));
    menu.handle_key(Key::Char('h'));
    assert!(matches!(
        menu.mode(),
        TextActionMode::ColorSelect { is_highlight: true }
    ));

    let action = menu.handle_key(Key::Char('g'));
    assert!(matches!(action, TextActionResponse::ApplyHighlight { hex } if hex == "#A3BE8C"));
    assert!(!menu.is_open());
}

#[test]
fn test_word_boundary_at_cursor() {
    let buffer =
        Buffer::from_text(BufferId::new(1), "Hello world, rust programming!").expect("buffer");
    let range = word_boundary_at(&buffer, 7);
    assert_eq!(range, Some(6..11));

    let space_range = word_boundary_at(&buffer, 5);
    assert_eq!(space_range, None);
}

#[test]
fn test_menu_task_status_navigation_and_selection() {
    let mut menu = TextActionOverlay::new();
    menu.open(Some("Task line".into()), None);

    // Press 't' to open task status selection
    let action = menu.handle_key(Key::Char('t'));
    assert_eq!(action, TextActionResponse::None);
    assert_eq!(
        menu.mode(),
        &TextActionMode::TaskStatusSelect { selected_index: 0 }
    );

    // Down arrow navigates to InProgress (index 1)
    menu.handle_key(Key::Down);
    assert_eq!(
        menu.mode(),
        &TextActionMode::TaskStatusSelect { selected_index: 1 }
    );

    // Down arrow navigates to Done (index 2)
    menu.handle_key(Key::Down);
    assert_eq!(
        menu.mode(),
        &TextActionMode::TaskStatusSelect { selected_index: 2 }
    );

    // Enter applies Done
    let action = menu.handle_key(Key::Enter);
    assert_eq!(
        action,
        TextActionResponse::ApplyTask {
            status: TaskStatus::Done
        }
    );
    assert!(!menu.is_open());
}

#[test]
fn test_menu_task_status_hotkeys() {
    // Press '1' applies Todo immediately
    let mut menu = TextActionOverlay::new();
    menu.open(None, None);
    menu.handle_key(Key::Char('t'));
    let action = menu.handle_key(Key::Char('1'));
    assert_eq!(
        action,
        TextActionResponse::ApplyTask {
            status: TaskStatus::Todo
        }
    );
    assert!(!menu.is_open());

    // Press '2' applies InProgress immediately
    let mut menu = TextActionOverlay::new();
    menu.open(None, None);
    menu.handle_key(Key::Char('t'));
    let action = menu.handle_key(Key::Char('2'));
    assert_eq!(
        action,
        TextActionResponse::ApplyTask {
            status: TaskStatus::InProgress
        }
    );
    assert!(!menu.is_open());

    // Press '4' applies Remove immediately
    let mut menu = TextActionOverlay::new();
    menu.open(None, None);
    menu.handle_key(Key::Char('t'));
    let action = menu.handle_key(Key::Char('4'));
    assert_eq!(
        action,
        TextActionResponse::ApplyTask {
            status: TaskStatus::Remove
        }
    );
    assert!(!menu.is_open());
}
