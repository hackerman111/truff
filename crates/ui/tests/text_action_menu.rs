use truff_core::vim::{Key, Mode};
use truff_ui::{app::Application, native::NativeApp, overlays::text_action::TextActionMode};

fn frame(
    app: &mut NativeApp,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            app.show(ui);
        },
    );
    output.textures_delta.clear();
    output
}

fn setup_app(text: &str) -> (NativeApp, egui::Context) {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text(text);
    let mut app = NativeApp::new(application, &ctx);
    frame(&mut app, &ctx, vec![]);
    (app, ctx)
}

#[test]
fn test_space_c_opens_text_action_overlay_in_visual_mode() {
    let (mut app, _) = setup_app("hello world test\n");
    // Move to 'world' and select it
    app.key(Key::Char('w')); // cursor at 'world'
    app.key(Key::Char('v')); // Visual mode
    app.key(Key::Char('e')); // select 'world'
    assert_eq!(app.application.shell.mode(), Mode::VisualChar);

    app.key(Key::Char(' '));
    app.key(Key::Char('c'));

    assert!(
        app.application.shell.text_action().is_open(),
        "Space c must open TextActionOverlay"
    );
    assert_eq!(
        app.application.shell.text_action().mode(),
        &TextActionMode::Root {
            has_selection: true
        }
    );
    assert_eq!(
        app.application.shell.text_action().initial_text(),
        Some("world")
    );
}

#[test]
fn test_text_color_via_mnemonic_key_and_undo() {
    let (mut app, _) = setup_app("hello world test\n");
    app.key(Key::Char('w'));
    app.key(Key::Char('v'));
    app.key(Key::Char('e'));

    // Open menu: Space c
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));

    // Pick text color: c
    app.key(Key::Char('c'));
    assert!(matches!(
        app.application.shell.text_action().mode(),
        TextActionMode::ColorSelect {
            is_highlight: false
        }
    ));

    // Pick red: r (#BF616A)
    app.key(Key::Char('r'));

    assert!(!app.application.shell.text_action().is_open());
    assert_eq!(app.application.shell.mode(), Mode::Normal);

    let text = app.application.shell.editor_text();
    assert!(
        text.contains("<span style=\"color:#BF616A\">world</span>"),
        "Text must contain red span: {text}"
    );

    // Test Undo
    app.key(Key::Char('u'));
    assert_eq!(app.application.shell.editor_text(), "hello world test\n");
}

#[test]
fn test_text_highlight_via_arrow_keys() {
    let (mut app, _) = setup_app("hello world test\n");
    app.key(Key::Char('w'));
    app.key(Key::Char('v'));
    app.key(Key::Char('e'));

    app.key(Key::Char(' '));
    app.key(Key::Char('c'));

    // Pick highlight: h
    app.key(Key::Char('h'));
    assert!(matches!(
        app.application.shell.text_action().mode(),
        TextActionMode::ColorSelect { is_highlight: true }
    ));

    // Navigate right to second color (Orange #D08770) and press Enter
    app.key(Key::Right);
    app.key(Key::Enter);

    assert!(!app.application.shell.text_action().is_open());
    let text = app.application.shell.editor_text();
    assert!(
        text.contains("<mark style=\"background-color:#D08770\">world</mark>"),
        "Text must contain orange mark: {text}"
    );
}

#[test]
fn test_visual_selection_n_prompts_target_with_title() {
    let (mut app, _) = setup_app("hello world test\n");
    app.key(Key::Char('w'));
    app.key(Key::Char('v'));
    app.key(Key::Char('e')); // selects 'world'

    app.key(Key::Char(' '));
    app.key(Key::Char('c'));

    // Note link: n opens LinkInput with title = "world" and target = ""
    app.key(Key::Char('n'));

    assert!(app.application.shell.text_action().is_open());
    assert!(matches!(
        app.application.shell.text_action().mode(),
        TextActionMode::LinkInput {
            is_url: false,
            has_selection: true,
            title_input,
            active_field: 1,
            ..
        } if title_input == "world"
    ));

    // Type target note name "TargetDoc"
    for ch in "TargetDoc".chars() {
        app.key(Key::Char(ch));
    }
    app.key(Key::Enter);

    assert!(!app.application.shell.text_action().is_open());
    assert_eq!(app.application.shell.mode(), Mode::Normal);
    let text = app.application.shell.editor_text();
    assert!(
        text.contains("[[TargetDoc|world]]"),
        "Text must contain aliased wikilink with selected text as title: {text}"
    );
}

#[test]
fn test_normal_mode_word_n_prompts_target_with_title() {
    let (mut app, _) = setup_app("hello world test\n");
    app.key(Key::Char('w')); // on 'world' in Normal mode

    app.key(Key::Char(' '));
    app.key(Key::Char('c'));

    // Note link: n opens LinkInput with captured word as title
    app.key(Key::Char('n'));

    assert!(app.application.shell.text_action().is_open());
    assert!(matches!(
        app.application.shell.text_action().mode(),
        TextActionMode::LinkInput {
            is_url: false,
            has_selection: true,
            title_input,
            active_field: 1,
            ..
        } if title_input == "world"
    ));

    for ch in "TargetDoc".chars() {
        app.key(Key::Char(ch));
    }
    app.key(Key::Enter);

    assert!(!app.application.shell.text_action().is_open());
    assert_eq!(app.application.shell.mode(), Mode::Normal);
    let text = app.application.shell.editor_text();
    assert!(
        text.contains("[[TargetDoc|world]]"),
        "Word under cursor must be used as link title: {text}"
    );
}

#[test]
fn test_note_link_autocomplete_from_vault() {
    let (mut app, ctx) = setup_app("hello world test\n");
    app.application
        .shell
        .set_sidebar_files(vec!["Architecture.md".into(), "RustGuide.md".into()]);

    app.key(Key::Char('w'));
    app.key(Key::Char('v'));
    app.key(Key::Char('e')); // selects 'world'

    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    app.key(Key::Char('n'));

    // Run frame to let show() populate suggestions from vault files
    frame(&mut app, &ctx, vec![]);

    // Verify suggestions contains "Architecture"
    if let TextActionMode::LinkInput { suggestions, .. } =
        app.application.shell.text_action().mode()
    {
        assert!(
            suggestions.contains(&"Architecture".to_string()),
            "Suggestions must contain Architecture from vault: {:?}",
            suggestions
        );
    } else {
        panic!("Expected LinkInput mode");
    }

    // Down arrow selects first suggestion ("Architecture")
    app.key(Key::Down);
    // Enter applies the selected candidate
    app.key(Key::Enter);

    assert!(!app.application.shell.text_action().is_open());
    let text = app.application.shell.editor_text();
    assert!(
        text.contains("[[Architecture|world]]"),
        "Text must contain wikilink with autocompleted target and selected title: {text}"
    );
}

#[test]
fn test_text_action_egui_text_input_works() {
    let (mut app, ctx) = setup_app("hello world test\n");
    app.key(Key::Char('w'));
    app.key(Key::Char('v'));
    app.key(Key::Char('e')); // selects 'world'

    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    app.key(Key::Char('n')); // opens LinkInput

    // First frame ensures input field is rendered and focused
    frame(&mut app, &ctx, vec![]);

    // Send Event::Text into the application
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Text("TypedTarget".into())],
    );

    // Verify target_input got the typed text!
    if let TextActionMode::LinkInput { target_input, .. } =
        app.application.shell.text_action().mode()
    {
        assert_eq!(
            target_input, "TypedTarget",
            "Typed text must be received by TextEdit inside LinkInput"
        );
    } else {
        panic!("Expected LinkInput mode");
    }

    // Now press Enter
    app.key(Key::Enter);
    assert!(!app.application.shell.text_action().is_open());
    let text = app.application.shell.editor_text();
    assert!(
        text.contains("[[TypedTarget|world]]"),
        "Text must contain wikilink with typed target: {text}"
    );
}

#[test]
fn test_url_link_with_selection_prompts_url_only() {
    let (mut app, _) = setup_app("visit rust docs now\n");
    app.key(Key::Char('w'));
    app.key(Key::Char('v'));
    app.key(Key::Char('e')); // selects 'rust'

    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    app.key(Key::Char('u')); // URL link

    assert!(matches!(
        app.application.shell.text_action().mode(),
        TextActionMode::LinkInput {
            is_url: true,
            has_selection: true,
            title_input,
            active_field: 1,
            ..
        } if title_input == "rust"
    ));

    if let TextActionMode::LinkInput { target_input, .. } =
        app.application.shell.text_action_mut().mode_mut()
    {
        *target_input = "https://rust-lang.org".into();
    }
    app.key(Key::Enter);

    assert!(!app.application.shell.text_action().is_open());
    let text = app.application.shell.editor_text();
    assert!(
        text.contains("[rust](https://rust-lang.org)"),
        "Text must contain markdown link: {text}"
    );
}

#[test]
fn test_empty_cursor_note_link_dual_fields() {
    let (mut app, _) = setup_app("\n");
    // Cursor at empty position
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));

    assert!(app.application.shell.text_action().is_open());
    assert_eq!(
        app.application.shell.text_action().mode(),
        &TextActionMode::Root {
            has_selection: false
        }
    );

    // Press n to open dual field input
    app.key(Key::Char('n'));
    assert!(matches!(
        app.application.shell.text_action().mode(),
        TextActionMode::LinkInput {
            is_url: false,
            has_selection: false,
            active_field: 0,
            ..
        }
    ));

    // Fill target, tab, fill title, Enter
    if let TextActionMode::LinkInput {
        target_input,
        title_input,
        ..
    } = app.application.shell.text_action_mut().mode_mut()
    {
        *target_input = "Architecture".into();
        *title_input = "Earth".into();
    }
    app.key(Key::Enter);

    assert!(!app.application.shell.text_action().is_open());
    let text = app.application.shell.editor_text();
    assert!(
        text.contains("[[Architecture|Earth]]"),
        "Text must contain aliased wikilink: {text}"
    );
}

#[test]
fn test_empty_cursor_note_link_without_title() {
    let (mut app, _) = setup_app("\n");
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    app.key(Key::Char('n'));

    if let TextActionMode::LinkInput { target_input, .. } =
        app.application.shell.text_action_mut().mode_mut()
    {
        *target_input = "Architecture".into();
    }
    app.key(Key::Enter);

    assert!(!app.application.shell.text_action().is_open());
    let text = app.application.shell.editor_text();
    assert!(
        text.contains("[[Architecture]]"),
        "Text must contain standard wikilink without alias: {text}"
    );
}

#[test]
fn test_empty_cursor_url_link_dual_fields() {
    let (mut app, _) = setup_app("\n");
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    app.key(Key::Char('u'));

    assert!(matches!(
        app.application.shell.text_action().mode(),
        TextActionMode::LinkInput {
            is_url: true,
            has_selection: false,
            active_field: 0,
            ..
        }
    ));

    if let TextActionMode::LinkInput {
        target_input,
        title_input,
        ..
    } = app.application.shell.text_action_mut().mode_mut()
    {
        *target_input = "https://example.com".into();
        *title_input = "Example".into();
    }
    app.key(Key::Enter);

    assert!(!app.application.shell.text_action().is_open());
    let text = app.application.shell.editor_text();
    assert!(
        text.contains("[Example](https://example.com)"),
        "Text must contain markdown link: {text}"
    );
}

#[test]
fn test_empty_cursor_style_rejected_with_status() {
    let (mut app, _) = setup_app("\n");
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));

    // Try color on empty cursor
    app.key(Key::Char('c'));
    assert_eq!(
        app.application.status,
        "Выделите текст или поместите курсор на слово для применения стиля"
    );
    // Menu remains in Root { has_selection: false }
    assert_eq!(
        app.application.shell.text_action().mode(),
        &TextActionMode::Root {
            has_selection: false
        }
    );
}

#[test]
fn test_remove_style_via_x() {
    let (mut app, _) = setup_app("hello <span style=\"color:#BF616A\">world</span> test\n");
    // Move cursor onto "world" inside the span (byte index 36)
    for _ in 0..36 {
        app.key(Key::Char('l'));
    }

    // Space c in Normal mode captures "world" and x removes enclosing style
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    app.key(Key::Char('x')); // Remove style

    assert!(!app.application.shell.text_action().is_open());
    let text = app.application.shell.editor_text();
    assert!(
        !text.contains("<span"),
        "Style tags should be removed: {text}"
    );
    assert_eq!(text, "hello world test\n");
}

#[test]
fn test_render_does_not_panic() {
    let (mut app, ctx) = setup_app("testing floating overlay render\n");
    app.key(Key::Char('w'));
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    assert!(app.application.shell.text_action().is_open());

    // Run UI pass with open overlay
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();

    // Also test TaskStatusSelect rendering pass
    app.key(Key::Char('t'));
    assert!(matches!(
        app.application.shell.text_action().mode(),
        TextActionMode::TaskStatusSelect { .. }
    ));
    let mut output2 = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output2.textures_delta.clear();
}

#[test]
fn test_space_c_task_create_todo_and_cycle_statuses() {
    let (mut app, _) = setup_app("Buy milk\n");
    // In Normal mode on "Buy milk", press <space> c -> t -> 1 (Todo)
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    assert!(app.application.shell.text_action().is_open());

    app.key(Key::Char('t'));
    assert_eq!(
        app.application.shell.text_action().mode(),
        &TextActionMode::TaskStatusSelect { selected_index: 0 }
    );

    app.key(Key::Char('1')); // Select Todo
    assert!(!app.application.shell.text_action().is_open());
    assert_eq!(app.application.shell.editor_text(), "- [ ] Buy milk\n");

    // Open <space> c -> t -> 2 (In Progress)
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    app.key(Key::Char('t'));
    app.key(Key::Char('2'));
    assert_eq!(app.application.shell.editor_text(), "- [/] Buy milk\n");

    // Open <space> c -> t -> 3 (Done)
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    app.key(Key::Char('t'));
    app.key(Key::Char('3'));
    assert_eq!(app.application.shell.editor_text(), "- [x] Buy milk\n");

    // Open <space> c -> t -> 4 (Remove)
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    app.key(Key::Char('t'));
    app.key(Key::Char('4'));
    assert_eq!(app.application.shell.editor_text(), "Buy milk\n");
}

#[test]
fn test_space_c_task_on_empty_line() {
    let (mut app, _) = setup_app("\n");
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    assert!(app.application.shell.text_action().is_open());

    app.key(Key::Char('t'));
    app.key(Key::Char('1'));
    assert_eq!(app.application.shell.editor_text(), "- [ ] \n");
}

#[test]
fn test_space_c_task_multiline_selection() {
    let (mut app, _) = setup_app("milk\nbread\neggs\n");
    // Visual Line select 3 lines: V j j
    app.key(Key::Char('V'));
    app.key(Key::Char('j'));
    app.key(Key::Char('j'));
    assert_eq!(app.application.shell.mode(), Mode::VisualLine);

    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    app.key(Key::Char('t'));
    app.key(Key::Char('1')); // Todo

    assert_eq!(
        app.application.shell.editor_text(),
        "- [ ] milk\n- [ ] bread\n- [ ] eggs\n"
    );
}

#[test]
fn test_space_c_task_preserves_indent() {
    let (mut app, _) = setup_app("    nested item\n");
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    app.key(Key::Char('t'));
    app.key(Key::Char('1'));
    assert_eq!(
        app.application.shell.editor_text(),
        "    - [ ] nested item\n"
    );
}

#[test]
fn test_tab_in_url_link_input() {
    let (mut app, ctx) = setup_app("\n");
    app.key(Key::Char(' '));
    app.key(Key::Char('c'));
    app.key(Key::Char('u')); // opens LinkInput with is_url: true, active_field: 0

    // Frame 1: render popup and request focus on title
    frame(&mut app, &ctx, vec![]);
    // Frame 2: focus settled on title
    frame(&mut app, &ctx, vec![]);

    let target_edit_id = egui::Id::new("text_action_target_input");
    let title_edit_id = egui::Id::new("text_action_title_input");

    assert_eq!(ctx.memory(|m| m.focused()), Some(title_edit_id));
    if let TextActionMode::LinkInput { active_field, .. } =
        app.application.shell.text_action().mode()
    {
        assert_eq!(*active_field, 0);
    }

    // Press Tab to switch from Title -> URL/Target
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    // Settle frame
    frame(&mut app, &ctx, vec![]);

    assert_eq!(ctx.memory(|m| m.focused()), Some(target_edit_id));
    if let TextActionMode::LinkInput { active_field, .. } =
        app.application.shell.text_action().mode()
    {
        assert_eq!(
            *active_field, 1,
            "active_field should be 1 (target/URL) after Tab"
        );
    }

    // Press Tab again to switch from URL/Target -> Title
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    // Settle frame
    frame(&mut app, &ctx, vec![]);

    assert_eq!(ctx.memory(|m| m.focused()), Some(title_edit_id));
    if let TextActionMode::LinkInput { active_field, .. } =
        app.application.shell.text_action().mode()
    {
        assert_eq!(
            *active_field, 0,
            "active_field should be 0 (title) after second Tab"
        );
    }

    // Press Shift+Tab to switch backwards from Title -> URL/Target
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::SHIFT,
        }],
    );
    // Settle frame
    frame(&mut app, &ctx, vec![]);

    assert_eq!(ctx.memory(|m| m.focused()), Some(target_edit_id));
    if let TextActionMode::LinkInput { active_field, .. } =
        app.application.shell.text_action().mode()
    {
        assert_eq!(
            *active_field, 1,
            "active_field should be 1 (target/URL) after Shift+Tab"
        );
    }
}
