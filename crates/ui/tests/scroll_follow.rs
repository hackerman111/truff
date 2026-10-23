use egui::Event;
use truff_ui::{
    app::Application,
    native::NativeApp,
    navigator::{self, NavigatorItem, NavigatorMode, NavigatorState},
    theme::Theme,
};

fn raw(size: egui::Vec2, events: Vec<Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        events,
        ..Default::default()
    }
}

fn nav_frame(
    state: &mut NavigatorState,
    ctx: &egui::Context,
    events: Vec<Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(raw(egui::vec2(1100.0, 700.0), events), |_| {
        navigator::show(state, ctx, &Theme::nord());
    });
    output.textures_delta.clear();
    output
}

fn selected_is_painted(output: &egui::FullOutput, title: &str) -> bool {
    selected_in(output, title, egui::vec2(1100.0, 700.0))
}
fn selected_in(output: &egui::FullOutput, title: &str, size: egui::Vec2) -> bool {
    output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == title && text.galley.job.sections[0].format.font_id.size == Theme::nord().geometry.ui_font_size && shape.clip_rect.contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size())) && text.pos.x + text.galley.size().x / 2.0 < size.x / 2.0 && text.pos.y >= 0.0 && text.pos.y + text.galley.size().y <= size.y))
}

#[test]
fn navigator_offscreen_selection_is_actually_painted() {
    let ctx = egui::Context::default();
    let mut state = NavigatorState::new();
    state.open_mode(
        NavigatorMode::Files,
        "test",
        (0..1000)
            .map(|i| {
                NavigatorItem::new_file(i, &format!("note-{i}"), format!("note-{i}"), false, 0)
            })
            .collect(),
    );
    nav_frame(&mut state, &ctx, vec![]);
    for _ in 0..100 {
        nav_frame(&mut state, &ctx, vec![Event::Text("j".into())]);
    }
    let output = nav_frame(&mut state, &ctx, vec![]);
    assert!(selected_is_painted(&output, "note-100"));
    let output = nav_frame(&mut state, &ctx, vec![Event::Text("G".into())]);
    assert!(selected_is_painted(&output, "note-999"));
    let output = nav_frame(&mut state, &ctx, vec![Event::Text("gg".into())]);
    assert!(selected_is_painted(&output, "note-0"));
}

fn key(key: egui::Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn navigator_count_page_filter_resize_and_preview_keep_the_selected_row_visible() {
    let ctx = egui::Context::default();
    let mut state = NavigatorState::new();
    state.open_mode(
        NavigatorMode::Sessions,
        "test",
        (0..1000)
            .map(|i| {
                NavigatorItem::new_file(i, &format!("note-{i}"), format!("note-{i}"), false, 0)
            })
            .collect(),
    );
    nav_frame(&mut state, &ctx, vec![]);
    let output = nav_frame(&mut state, &ctx, vec![Event::Text("50j".into())]);
    assert_eq!(state.selected_index, 50);
    assert!(selected_is_painted(&output, "note-50"));
    let output = nav_frame(&mut state, &ctx, vec![key(egui::Key::PageDown)]);
    let page = state.selected_index - 50;
    assert!(page > 0);
    assert!(selected_is_painted(
        &output,
        &format!("note-{}", state.selected_index)
    ));
    let output = nav_frame(&mut state, &ctx, vec![Event::Text(" ".into())]);
    assert!(selected_is_painted(
        &output,
        &format!("note-{}", state.selected_index)
    ));
    state.set_query("note-999");
    let output = nav_frame(&mut state, &ctx, vec![]);
    assert_eq!(state.selected_index, 0);
    assert!(selected_is_painted(&output, "note-999"));
    state.set_query("");
    for (size, scale) in [
        (egui::vec2(800.0, 600.0), 1.0),
        (egui::vec2(1280.0, 720.0), 2.0),
        (egui::vec2(1920.0, 1080.0), 1.5),
    ] {
        ctx.set_pixels_per_point(scale);
        let mut output = ctx.run_ui(raw(size, vec![Event::Text("G".into())]), |_| {
            navigator::show(&mut state, &ctx, &Theme::nord());
        });
        output.textures_delta.clear();
        // A DPI change invalidates egui's fonts/Area sizing; inspect the requested repaint.
        for _ in 0..2 {
            output = ctx.run_ui(raw(size, vec![]), |_| {
                navigator::show(&mut state, &ctx, &Theme::nord());
            });
            output.textures_delta.clear();
        }
        assert!(
            selected_in(&output, "note-999", size),
            "size {size:?}, scale {scale}"
        );
    }
}

#[test]
fn help_eighty_items_keyboard_filter_and_escape_restore_the_previous_pane() {
    use truff_core::{config::KeyHint, vim::Key};
    use truff_ui::overlays::{WhichKeyCategory, WhichKeyMode};
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("word text\n");
    let mut app = NativeApp::new(application, &ctx);
    native_frame(&mut app, &ctx, raw(egui::vec2(1100.0, 700.0), vec![]));
    let focus = app.application.shell.focus();
    app.application
        .shell
        .which_key_mut()
        .show_help(vec![WhichKeyCategory {
            name: "test".into(),
            hints: (0..100)
                .map(|i| KeyHint {
                    key: Key::Char('x'),
                    description: format!("help-item-{i:03}"),
                    is_prefix: false,
                })
                .collect(),
        }]);
    for event in [None, Some(Event::Text("G".into())), None] {
        native_frame(
            &mut app,
            &ctx,
            raw(egui::vec2(1100.0, 700.0), event.into_iter().collect()),
        );
    }
    let mut output = ctx.run_ui(raw(egui::vec2(1100.0, 700.0), vec![]), |ui| app.show(ui));
    output.textures_delta.clear();
    assert!(selected_is_painted(&output, "help-item-099"));
    native_frame(
        &mut app,
        &ctx,
        raw(egui::vec2(1100.0, 700.0), vec![Event::Text("gg".into())]),
    );
    let mut output = ctx.run_ui(raw(egui::vec2(1100.0, 700.0), vec![]), |ui| app.show(ui));
    output.textures_delta.clear();
    assert!(selected_is_painted(&output, "help-item-000"));
    native_frame(
        &mut app,
        &ctx,
        raw(egui::vec2(1100.0, 700.0), vec![Event::Text("/".into())]),
    );
    native_frame(
        &mut app,
        &ctx,
        raw(egui::vec2(1100.0, 700.0), vec![Event::Text("099".into())]),
    );
    for (size, scale) in [
        (egui::vec2(800.0, 600.0), 1.0),
        (egui::vec2(1280.0, 720.0), 2.0),
        (egui::vec2(1920.0, 1080.0), 1.5),
    ] {
        ctx.set_pixels_per_point(scale);
        let mut output = ctx.run_ui(raw(size, vec![]), |ui| app.show(ui));
        for _ in 0..2 {
            output.textures_delta.clear();
            output = ctx.run_ui(raw(size, vec![]), |ui| app.show(ui));
        }
        output.textures_delta.clear();
        assert!(selected_in(&output, "help-item-099", size));
        assert!(!selected_in(&output, "help-item-000", size));
    }
    native_frame(
        &mut app,
        &ctx,
        raw(egui::vec2(1100.0, 700.0), vec![key(egui::Key::Escape)]),
    );
    assert!(app.application.shell.which_key().is_visible());
    assert_eq!(app.application.shell.which_key().mode(), WhichKeyMode::Help);
    native_frame(
        &mut app,
        &ctx,
        raw(egui::vec2(1100.0, 700.0), vec![key(egui::Key::Escape)]),
    );
    assert!(!app.application.shell.which_key().is_visible());
    assert_eq!(app.application.shell.focus(), focus);
}

#[test]
fn modal_folder_field_keeps_literal_text_and_normal_editor_still_starts_flash() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("word text\n");
    let mut app = NativeApp::new(application, &ctx);
    for word in ["session", "source", "glass", "диссертация", "jkd: "] {
        app.dialogs.folder.open = true;
        app.dialogs.folder.path.clear();
        native_frame(&mut app, &ctx, raw(egui::vec2(1100.0, 700.0), vec![]));
        native_frame(
            &mut app,
            &ctx,
            raw(egui::vec2(1100.0, 700.0), vec![Event::Text(word.into())]),
        );
        assert_eq!(app.dialogs.folder.path, word);
        assert!(!app.application.shell.flash().is_active());
        native_frame(
            &mut app,
            &ctx,
            raw(egui::vec2(1100.0, 700.0), vec![key(egui::Key::Escape)]),
        );
    }
    native_frame(&mut app, &ctx, raw(egui::vec2(1100.0, 700.0), vec![]));
    native_frame(
        &mut app,
        &ctx,
        raw(egui::vec2(1100.0, 700.0), vec![Event::Text("s".into())]),
    );
    assert!(app.application.shell.flash().is_active());
}

#[test]
fn backlinks_use_row_motions_and_paint_the_offscreen_target() {
    use truff_store::index::{FileId, backlinks::BacklinkOccurrence};
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text("word text\n");
    application.set_backlinks(
        (0..1000)
            .map(|i| BacklinkOccurrence {
                source_file_id: FileId(i),
                source_path: format!("backlink-{i}"),
                occurrence_ordinal: 0,
                range: 0..1,
                raw_target: "test".into(),
                heading: None,
                label: None,
                is_wiki: true,
            })
            .collect(),
    );
    application
        .shell
        .set_focus(truff_ui::shell::Focus::RightPanel);
    let mut app = NativeApp::new(application, &ctx);
    native_frame(&mut app, &ctx, raw(egui::vec2(1100.0, 700.0), vec![]));
    native_frame(
        &mut app,
        &ctx,
        raw(egui::vec2(1100.0, 700.0), vec![Event::Text("100j".into())]),
    );
    assert_eq!(app.application.shell.right_model().selected_index(), 100);
    native_frame(
        &mut app,
        &ctx,
        raw(egui::vec2(1100.0, 700.0), vec![Event::Text("G".into())]),
    );
    assert_eq!(app.application.shell.right_model().selected_index(), 999);
    let mut output = ctx.run_ui(raw(egui::vec2(1100.0, 700.0), vec![]), |ui| app.show(ui));
    output.textures_delta.clear();
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(t) if t.galley.job.text == "backlink-999" && shape.clip_rect.contains_rect(egui::Rect::from_min_size(t.pos, t.galley.size())))));
}

#[test]
fn editor_caret_at_end_of_tall_wrapped_paragraph_is_visible() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application
        .shell
        .set_editor_text(&format!("# heading\n{}", "word 界 👩‍💻 ".repeat(150)));
    let mut app = NativeApp::new(application, &ctx);
    native_frame(&mut app, &ctx, raw(egui::vec2(800.0, 600.0), vec![]));
    native_frame(
        &mut app,
        &ctx,
        raw(egui::vec2(800.0, 600.0), vec![Event::Text("G$".into())]),
    );
    for _ in 0..3 {
        native_frame(&mut app, &ctx, raw(egui::vec2(800.0, 600.0), vec![]));
    }
    let caret = app.editor.cursor_rect().expect("caret must be painted");
    assert!(caret.top() >= 0.0 && caret.bottom() < 580.0, "{caret:?}");
}

#[test]
fn editor_pages_visual_motion_resize_and_two_views_keep_source_and_scroll() {
    use truff_core::{commands::Action, text::BytePos};
    let ctx = egui::Context::default();
    let mut application = Application::new();
    let text: String = (0..2000)
        .map(|i| {
            if i % 10 == 0 {
                format!("# Heading {i} 界 👩‍💻\n")
            } else {
                format!("line {i} {}\n", "word Ж 界 ".repeat(8))
            }
        })
        .collect();
    application.shell.set_editor_text(&text);
    let mut app = NativeApp::new(application, &ctx);
    let size = egui::vec2(1100.0, 700.0);
    native_frame(&mut app, &ctx, raw(size, vec![]));
    let first_view = app.application.shell.editor_view().unwrap().id();
    let buffer_id = app.application.shell.editor_buffer().unwrap().id();
    native_frame(&mut app, &ctx, raw(size, vec![key(egui::Key::PageDown)]));
    let head = app.application.shell.editor_view().unwrap().head();
    assert!(
        head.get() > 0,
        "PageDown moves the caret as well as the viewport"
    );
    native_frame(&mut app, &ctx, raw(size, vec![Event::Text("v30j".into())]));
    assert_eq!(
        app.application.shell.mode(),
        truff_core::vim::Mode::VisualChar
    );
    assert_eq!(app.application.shell.editor_view().unwrap().anchor(), head);
    native_frame(&mut app, &ctx, raw(egui::vec2(800.0, 600.0), vec![]));
    for _ in 0..3 {
        native_frame(&mut app, &ctx, raw(egui::vec2(800.0, 600.0), vec![]));
    }
    let caret = app.editor.cursor_rect().expect("caret after resize");
    assert!(caret.top() >= 0.0 && caret.bottom() < 580.0, "{caret:?}");
    native_frame(&mut app, &ctx, raw(size, vec![key(egui::Key::Escape)]));
    app.editor.scroll_by(200.0);
    native_frame(&mut app, &ctx, raw(size, vec![]));
    let first_offset = app.editor.scroll_offset();
    let second_view = app
        .application
        .shell
        .workspace_mut()
        .create_view(buffer_id)
        .unwrap();
    app.application
        .shell
        .workspace_mut()
        .set_active_view(Some(second_view));
    native_frame(&mut app, &ctx, raw(size, vec![]));
    let target = app
        .application
        .shell
        .editor_buffer()
        .unwrap()
        .byte_of_line(1500);
    app.application
        .shell
        .workspace_mut()
        .dispatch(Action::MoveCursor {
            view_id: second_view,
            anchor: BytePos::new(target),
            head: BytePos::new(target),
        })
        .unwrap();
    native_frame(&mut app, &ctx, raw(size, vec![]));
    let second_offset = app.editor.scroll_offset();
    assert!(second_offset > first_offset + 1000.0);
    app.application
        .shell
        .workspace_mut()
        .set_active_view(Some(first_view));
    native_frame(&mut app, &ctx, raw(size, vec![]));
    assert!((app.editor.scroll_offset() - first_offset).abs() < 1.0);
    app.application
        .shell
        .workspace_mut()
        .set_active_view(Some(second_view));
    native_frame(&mut app, &ctx, raw(size, vec![]));
    assert!((app.editor.scroll_offset() - second_offset).abs() < 1.0);
}

#[test]
fn help_owns_flash_key_and_editor_scroll() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application
        .shell
        .set_editor_text(&"word text\n".repeat(2000));
    let mut app = NativeApp::new(application, &ctx);
    native_frame(&mut app, &ctx, raw(egui::vec2(1100.0, 700.0), vec![]));
    native_frame(
        &mut app,
        &ctx,
        raw(egui::vec2(1100.0, 700.0), vec![Event::Text("?".into())]),
    );
    assert!(app.application.shell.which_key().is_visible());
    native_frame(
        &mut app,
        &ctx,
        raw(egui::vec2(1100.0, 700.0), vec![Event::Text("s".into())]),
    );
    assert!(!app.application.shell.flash().is_active());
    let before = app.editor.scroll_offset();
    native_frame(
        &mut app,
        &ctx,
        raw(
            egui::vec2(1100.0, 700.0),
            vec![
                Event::PointerMoved(egui::pos2(600.0, 300.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Move,
                    delta: egui::vec2(0.0, -120.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        ),
    );
    assert_eq!(app.editor.scroll_offset(), before);
}

#[test]
fn opening_help_mid_batch_owns_paste_and_cut() {
    for event in [Event::Paste("LEAK".into()), Event::Cut] {
        let ctx = egui::Context::default();
        let mut application = Application::new();
        application.shell.set_editor_text("word text\n");
        let mut app = NativeApp::new(application, &ctx);
        native_frame(&mut app, &ctx, raw(egui::vec2(1100.0, 700.0), vec![]));
        let view = app.application.shell.editor_view().unwrap().id();
        app.application
            .shell
            .workspace_mut()
            .get_view_mut(view)
            .unwrap()
            .set_cursor(
                truff_core::text::BytePos::new(0),
                truff_core::text::BytePos::new(4),
            );
        native_frame(
            &mut app,
            &ctx,
            raw(
                egui::vec2(1100.0, 700.0),
                vec![Event::Text("?".into()), event],
            ),
        );
        assert!(app.application.shell.which_key().is_visible());
        assert_eq!(app.application.shell.editor_text(), "word text\n");
    }
}

#[test]
fn editing_above_an_inactive_view_rebases_its_scroll_anchor() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text(
        &(0..2000)
            .map(|i| format!("original {i}\n"))
            .collect::<String>(),
    );
    let mut app = NativeApp::new(application, &ctx);
    let size = egui::vec2(1100.0, 700.0);
    native_frame(&mut app, &ctx, raw(size, vec![]));
    let first = app.application.shell.editor_view().unwrap().id();
    let buffer = app.application.shell.editor_buffer().unwrap().id();
    native_frame(&mut app, &ctx, raw(size, vec![Event::Text("500j".into())]));
    let top = app.editor.top_line();
    assert!(top > 400);
    let second = app
        .application
        .shell
        .workspace_mut()
        .create_view(buffer)
        .unwrap();
    app.application
        .shell
        .workspace_mut()
        .set_active_view(Some(second));
    native_frame(&mut app, &ctx, raw(size, vec![]));
    app.application
        .shell
        .workspace_mut()
        .dispatch(truff_core::commands::Action::ApplyEdit {
            view_id: second,
            transaction: truff_core::Transaction::insert(0, "new\n".repeat(100)),
        })
        .unwrap();
    native_frame(&mut app, &ctx, raw(size, vec![]));
    app.application
        .shell
        .workspace_mut()
        .set_active_view(Some(first));
    native_frame(&mut app, &ctx, raw(size, vec![]));
    assert_eq!(app.editor.top_line(), top + 100);
}

#[test]
fn first_edit_preserves_the_top_gutter() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application
        .shell
        .set_editor_text(&"line text\n".repeat(1000));
    let mut app = NativeApp::new(application, &ctx);
    let size = egui::vec2(1100.0, 700.0);
    native_frame(&mut app, &ctx, raw(size, vec![]));
    assert_eq!(app.editor.scroll_offset(), 0.0);
    native_frame(&mut app, &ctx, raw(size, vec![Event::Text("ix".into())]));
    assert_eq!(app.editor.scroll_offset(), 0.0);
}

#[test]
fn navigator_wheel_stays_manual_until_the_next_keyboard_motion() {
    let ctx = egui::Context::default();
    let mut state = NavigatorState::new();
    state.open_mode(
        NavigatorMode::Files,
        "test",
        (0..1000)
            .map(|i| {
                NavigatorItem::new_file(i, &format!("note-{i}"), format!("note-{i}"), false, 0)
            })
            .collect(),
    );
    nav_frame(&mut state, &ctx, vec![]);
    let output = nav_frame(&mut state, &ctx, vec![Event::Text("100j".into())]);
    let position = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if text.galley.job.text == "note-100"
                    && text.galley.job.sections[0].format.font_id.size
                        == Theme::nord().geometry.ui_font_size =>
            {
                Some(text.pos + egui::vec2(10.0, 5.0))
            }
            _ => None,
        })
        .expect("selected list row");
    nav_frame(&mut state, &ctx, vec![Event::PointerMoved(position)]);
    for _ in 0..10 {
        nav_frame(
            &mut state,
            &ctx,
            vec![Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(0.0, -120.0),
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    let mut output = nav_frame(&mut state, &ctx, vec![]);
    for _ in 0..60 {
        output = nav_frame(&mut state, &ctx, vec![]);
    }
    assert_eq!(state.selected_index, 100);
    assert!(
        !selected_is_painted(&output, "note-100"),
        "wheel must not be snapped back to the unchanged selection"
    );
    let output = nav_frame(&mut state, &ctx, vec![Event::Text("j".into())]);
    assert!(selected_is_painted(&output, "note-101"));
}

#[test]
fn file_and_vault_fields_own_literal_text() {
    for vault in [false, true] {
        for word in ["session", "source", "glass", "диссертация", "jkd: "] {
            let ctx = egui::Context::default();
            let mut application = Application::new();
            application.shell.set_editor_text("word text\n");
            let mut app = NativeApp::new(application, &ctx);
            if vault {
                app.dialogs.vault.open = true;
                app.dialogs.vault.path.clear();
            } else {
                app.dialogs.file.open = true;
            }
            let size = egui::vec2(1100.0, 700.0);
            native_frame(&mut app, &ctx, raw(size, vec![]));
            native_frame(&mut app, &ctx, raw(size, vec![Event::Text(word.into())]));
            let path = if vault {
                &app.dialogs.vault.path
            } else {
                &app.dialogs.file.path
            };
            assert_eq!(path, word);
            assert!(!app.application.shell.flash().is_active());
            assert_eq!(app.application.shell.editor_text(), "word text\n");
        }
    }
}

#[test]
fn dragging_editor_selection_at_bottom_scrolls_and_preserves_source_anchor() {
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application
        .shell
        .set_editor_text(&"line text\n".repeat(2000));
    let mut app = NativeApp::new(application, &ctx);
    let size = egui::vec2(1100.0, 700.0);
    native_frame(&mut app, &ctx, raw(size, vec![]));
    let start = app.editor.cursor_rect().expect("initial caret").center();
    native_frame(
        &mut app,
        &ctx,
        raw(
            size,
            vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        ),
    );
    let anchor = app.application.shell.editor_view().unwrap().anchor();
    let before = app.editor.scroll_offset();
    native_frame(
        &mut app,
        &ctx,
        raw(size, vec![Event::PointerMoved(egui::pos2(start.x, 675.0))]),
    );
    for _ in 0..30 {
        native_frame(&mut app, &ctx, raw(size, vec![]));
    }
    assert!(
        app.editor.scroll_offset() > before + 30.0,
        "edge drag must scroll"
    );
    let view = app.application.shell.editor_view().unwrap();
    assert_eq!(view.anchor(), anchor);
    assert!(view.head().get() > anchor.get());
}

fn native_frame(app: &mut NativeApp, ctx: &egui::Context, input: egui::RawInput) {
    let mut output = ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
}
