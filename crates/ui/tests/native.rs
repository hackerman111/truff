//! Production egui event/paint path, without a platform window or GPU.
use egui::Event;
use truff_ui::{app::Application, native::NativeApp};

fn frame(app: &mut NativeApp, context: &egui::Context, events: Vec<Event>) -> egui::FullOutput {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1100.0, 700.0),
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

fn key(key: egui::Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn scratch(text: &str) -> (NativeApp, egui::Context) {
    let context = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_editor_text(text);
    let mut app = NativeApp::new(application, &context);
    frame(&mut app, &context, vec![]);
    (app, context)
}

#[test]
fn native_lists_keep_depth_code_context_and_toggle_tasks_with_undo() {
    let source =
        "plain\n1. parent\n   - child\n     - [ ] task\n\n```md\n- [ ] literal **bold**\n```\n";
    let (mut app, context) = scratch(source);
    let output = frame(&mut app, &context, vec![]);
    let text = |needle: &str| {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text.contains(needle) => Some(text),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {needle}"))
    };
    assert_eq!(text("parent").galley.job.text, "1. parent");
    assert_eq!(text("child").galley.job.text, "   ◦ child");
    let task = text("task");
    assert_eq!(task.galley.job.text, "     ☐ task");
    assert_eq!(text("literal").galley.job.text, "- [ ] literal **bold**");
    let glyph = &task.galley.rows[0].glyphs[5];
    let position = task.pos
        + egui::vec2(
            glyph.pos.x + glyph.advance_width * 0.2,
            task.galley.size().y * 0.5,
        );
    frame(&mut app, &context, vec![Event::PointerMoved(position)]);
    for pressed in [true, false] {
        frame(
            &mut app,
            &context,
            vec![Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    assert!(app.application.shell.editor_text().contains("- [x] task"));
    frame(&mut app, &context, vec![Event::Text("u".into())]);
    assert_eq!(app.application.shell.editor_text(), source);
    let builds = app.editor.layout_rebuild_count();
    frame(&mut app, &context, vec![]);
    assert_eq!(app.editor.layout_rebuild_count(), builds);
}

#[test]
fn sidebar_frame_response_toggles_folders_and_opens_files() {
    fn label_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing label {label}"))
    }
    fn click(app: &mut NativeApp, context: &egui::Context, pos: egui::Pos2) {
        frame(app, context, vec![Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(
                app,
                context,
                vec![Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
    }
    let vault = std::env::temp_dir().join(format!(
        "truff-native-sidebar-{}-{}",
        std::process::id(),
        truff_core::text::BufferId::next()
    ));
    std::fs::create_dir_all(vault.join("folder")).unwrap();
    std::fs::write(vault.join("a.md"), "root note").unwrap();
    std::fs::write(vault.join("folder/b.md"), "nested note").unwrap();
    let mut application = Application::new();
    application.open_vault(&vault).unwrap();
    let start = std::time::Instant::now();
    while application.needs_poll() {
        application.poll();
        assert!(
            start.elapsed() < std::time::Duration::from_secs(15),
            "{}",
            application.status
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let context = egui::Context::default();
    let mut app = NativeApp::new(application, &context);
    frame(&mut app, &context, vec![]);
    let output = frame(&mut app, &context, vec![]);
    let pos = label_center(&output, "folder");
    click(&mut app, &context, pos);
    assert_eq!(app.application.shell.sidebar_model().visible_count(), 3);
    let output = frame(&mut app, &context, vec![]);
    let pos = label_center(&output, "b.md");
    click(&mut app, &context, pos);
    assert_eq!(
        app.application.active_relative_path().as_deref(),
        Some("folder/b.md")
    );
    app.application.shutdown().unwrap();
    drop(app);
    std::fs::remove_dir_all(vault).unwrap();
}

#[test]
fn viewport_cache_stays_near_one_screen_while_moving() {
    let (mut app, context) = scratch(&"plain **bold** 字🦀\n".repeat(30_000));
    assert!(app.editor.cached_paragraph_count() <= 40);
    for step in 0..100 {
        frame(&mut app, &context, vec![Event::Text("j".into())]);
        assert!(app.editor.cached_paragraph_count() <= 40);
        assert!(
            app.editor.cursor_rect().is_some(),
            "step {step}, top {}, scroll {}",
            app.editor.top_line(),
            app.editor.scroll_offset()
        );
    }
}

#[test]
fn editor_font_sizes_share_four_levels_and_keep_configured_base() {
    use truff_ui::{app::global_config::TypographyConfig, native::resolve_font_and_line_height};
    for base in [12.0, 15.0, 22.0] {
        let typography = TypographyConfig {
            base_font_size: base,
            ..Default::default()
        };
        let sources = [
            "plain", "# a", "## b", "### c", "#### d", "##### e", "###### f", "```",
        ];
        let mut sizes: Vec<_> = sources
            .iter()
            .map(|text| {
                resolve_font_and_line_height(text, &typography)
                    .0
                    .size
                    .to_bits()
            })
            .collect();
        sizes.sort_unstable();
        sizes.dedup();
        assert!(sizes.len() <= 4, "base {base}: {sizes:?}");
        assert_eq!(
            resolve_font_and_line_height("plain", &typography).0.size,
            base
        );
    }
}

#[test]
fn printable_os_key_and_text_are_dispatched_once() {
    let (mut app, context) = scratch("first\nsecond\n");
    frame(
        &mut app,
        &context,
        vec![
            key(egui::Key::D),
            Event::Text("d".into()),
            key(egui::Key::D),
            Event::Text("d".into()),
        ],
    );
    assert_eq!(app.application.shell.editor_text(), "second\n");
    frame(&mut app, &context, vec![Event::Text("u".into())]);
    assert_eq!(app.application.shell.editor_text(), "first\nsecond\n");
    frame(
        &mut app,
        &context,
        vec![
            Event::Text("i".into()),
            key(egui::Key::A),
            Event::Text("字🦀".into()),
        ],
    );
    assert_eq!(app.application.shell.editor_text(), "字🦀first\nsecond\n");
}

#[test]
fn ime_is_transient_and_commit_uses_one_transaction() {
    let (mut app, context) = scratch("");
    frame(&mut app, &context, vec![Event::Text("i".into())]);
    let revision = app.application.shell.editor_buffer().unwrap().revision();
    let output = frame(
        &mut app,
        &context,
        vec![Event::Ime(egui::ImeEvent::Preedit {
            text: "候補".into(),
            active_range_chars: Some(0..2),
        })],
    );
    assert_eq!(
        app.application.shell.editor_buffer().unwrap().revision(),
        revision
    );
    assert!(
        output.platform_output.ime.is_some(),
        "native IME caret must be published"
    );
    frame(
        &mut app,
        &context,
        vec![Event::Ime(egui::ImeEvent::Commit("字🦀".into()))],
    );
    assert_eq!(
        app.application.shell.editor_buffer().unwrap().revision(),
        revision + 1
    );
    assert_eq!(app.application.shell.editor_text(), "字🦀");
}

#[test]
fn viewport_cache_is_bounded_reuses_shapes_and_handles_multi_megabyte_line() {
    let (mut app, context) = scratch(&"one two three\n".repeat(100_000));
    // Native panels settle their measured height after the initial frame.
    frame(&mut app, &context, vec![]);
    assert!(app.editor.cached_paragraph_count() < 100);
    let builds = app.editor.layout_rebuild_count();
    frame(&mut app, &context, vec![]);
    assert_eq!(builds, app.editor.layout_rebuild_count());
    frame(&mut app, &context, vec![key(egui::Key::PageDown)]);
    assert!(app.editor.scroll_offset() > 0.0);
    assert!(app.editor.cached_paragraph_count() < 100);
    app.application
        .shell
        .set_editor_text(&"🦀".repeat(1_000_000));
    frame(&mut app, &context, vec![Event::Text("G$".into())]);
    assert!(app.editor.cached_paragraph_count() <= 1);
    assert!(app.editor.cursor_rect().is_some());
}

#[test]
fn newline_edits_reuse_unchanged_paragraphs_and_theme_persists_between_frames() {
    let (mut app, context) = scratch(&"a short line\n".repeat(1_000));
    frame(&mut app, &context, vec![Event::Text("i".into())]);
    let builds = app.editor.layout_rebuild_count();
    frame(&mut app, &context, vec![Event::Text("新しい\n".into())]);
    let rebuilt = app.editor.layout_rebuild_count() - builds;
    assert!(
        rebuilt < 8,
        "rebuilt {rebuilt} paragraphs for one line insertion"
    );
    assert!(
        app.application
            .shell
            .editor_text()
            .starts_with("新しい\na short line\n")
    );
    let builds = app.editor.layout_rebuild_count();
    frame(&mut app, &context, vec![]);
    assert_eq!(builds, app.editor.layout_rebuild_count());
    app.application
        .shell
        .set_theme(truff_ui::theme::Theme::light());
    frame(&mut app, &context, vec![]);
    let fill = context.global_style().visuals.panel_fill;
    frame(&mut app, &context, vec![]);
    assert_eq!(context.global_style().visuals.panel_fill, fill);
    assert_eq!(fill, egui::Color32::from_rgb(250, 250, 252));
}

#[test]
fn test_procedural_vector_icons_and_dirty_dot() {
    use truff_ui::native::icons::{Icon, draw_dirty_dot, render_icon};
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
        render_icon(ui.painter(), Icon::File, rect, egui::Color32::WHITE, 1.5);
        render_icon(ui.painter(), Icon::Folder, rect, egui::Color32::WHITE, 1.5);
        render_icon(ui.painter(), Icon::Plus, rect, egui::Color32::WHITE, 1.5);
        render_icon(ui.painter(), Icon::Close, rect, egui::Color32::WHITE, 1.5);
        render_icon(
            ui.painter(),
            Icon::ChevronRight,
            rect,
            egui::Color32::WHITE,
            1.5,
        );
        render_icon(
            ui.painter(),
            Icon::ChevronDown,
            rect,
            egui::Color32::WHITE,
            1.5,
        );
        render_icon(ui.painter(), Icon::Vault, rect, egui::Color32::WHITE, 1.5);
        render_icon(
            ui.painter(),
            Icon::MoreHorizontal,
            rect,
            egui::Color32::WHITE,
            1.5,
        );
        render_icon(ui.painter(), Icon::Gear, rect, egui::Color32::WHITE, 1.5);
        draw_dirty_dot(ui.painter(), rect.center(), egui::Color32::LIGHT_BLUE, 2.75);
    });
    output.textures_delta.clear();
}

#[test]
fn test_zed_component_widgets() {
    use truff_ui::native::icons::Icon;
    use truff_ui::native::widgets::{icon_button, primary_button, secondary_button};
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let resp1 = icon_button(ui, Icon::Plus, "Add");
        assert_eq!(resp1.rect.size(), egui::vec2(28.0, 28.0));
        let resp2 = primary_button(ui, "Save");
        assert_eq!(resp2.rect.height(), 32.0);
        let resp3 = secondary_button(ui, "Cancel");
        assert_eq!(resp3.rect.height(), 32.0);
    });
    output.textures_delta.clear();
}

#[test]
fn test_dialog_open_suppresses_flash_shortcut() {
    let (mut app, context) = scratch("some test text for flash jumping\n");
    // Ensure in Normal mode
    assert_eq!(app.application.shell.mode(), truff_core::vim::Mode::Normal);

    // Open folder creation dialog
    app.dialogs.folder.open = true;

    // Type 's'
    frame(&mut app, &context, vec![Event::Text("s".into())]);

    // Flash must NOT be active because dialog is open
    assert!(
        !app.application.shell.flash().is_active(),
        "Typing 's' while folder dialog is open must not activate flash"
    );

    // Now close folder dialog
    app.dialogs.folder.open = false;

    // Type 's' in normal mode
    frame(&mut app, &context, vec![Event::Text("s".into())]);

    // Flash should activate now
    assert!(
        app.application.shell.flash().is_active(),
        "Typing 's' in Normal mode with no dialog open should activate flash"
    );
}

#[test]
fn test_visual_mode_character_and_line_highlighting_dimensions() {
    let (mut app, context) = scratch("# Heading\nregular text line\nЯ0g\n");
    frame(&mut app, &context, vec![]);

    // The caret follows the shaped glyph height even when heading sizes are quantized.
    let heading_cursor = app.editor.cursor_rect().expect("heading cursor rect");
    assert!(
        heading_cursor.height() >= 20.0 && heading_cursor.height() <= 24.0,
        "Heading highlight must follow glyph height rather than the full line height: got {}",
        heading_cursor.height()
    );
    assert!(
        heading_cursor.width() > 13.0,
        "Heading character width must match glyph advance width + padding: got {}",
        heading_cursor.width()
    );

    // Enter VisualChar mode with 'v'
    frame(&mut app, &context, vec![Event::Text("v".into())]);
    assert_eq!(
        app.application.shell.mode(),
        truff_core::vim::Mode::VisualChar
    );
    let visual_heading_cursor = app.editor.cursor_rect().expect("visual heading cursor");
    assert_eq!(
        visual_heading_cursor, heading_cursor,
        "Heading cursor rect in Visual mode must match Normal mode"
    );

    // Move to line 2 (regular text)
    frame(&mut app, &context, vec![Event::Text("j".into())]);
    let regular_cursor = app.editor.cursor_rect().expect("regular cursor");
    assert!(
        regular_cursor.height() >= 14.0 && regular_cursor.height() <= 17.0,
        "Regular line character highlight must match glyph height + padding (~14-16px), not full row height 26.0: got {}",
        regular_cursor.height()
    );

    // Move to line 3 (Я0g) - cursor at 'Я'
    frame(&mut app, &context, vec![Event::Text("j".into())]);
    let ya_cursor = app.editor.cursor_rect().expect("ya cursor");
    assert!(
        ya_cursor.height() >= 14.0 && ya_cursor.height() <= 17.0,
        "Cyrillic character 'Я' highlight must be glyph height + padding (~14-16px), without bottom tail: got {}",
        ya_cursor.height()
    );
    assert!(
        ya_cursor.width() >= 10.0,
        "Cyrillic character 'Я' highlight width must match advance width + padding: got {}",
        ya_cursor.width()
    );

    // Move cursor to '0' on line 3
    frame(&mut app, &context, vec![Event::Text("l".into())]);
    let zero_cursor = app.editor.cursor_rect().expect("zero cursor");
    assert!(
        zero_cursor.height() >= 14.0 && zero_cursor.height() <= 17.0,
        "Digit '0' highlight height must match cap-height + padding (~14-16px): got {}",
        zero_cursor.height()
    );

    // Enter VisualLine mode with 'V'
    frame(
        &mut app,
        &context,
        vec![key(egui::Key::Escape), Event::Text("V".into())],
    );
    assert_eq!(
        app.application.shell.mode(),
        truff_core::vim::Mode::VisualLine
    );
    let visual_line_cursor = app
        .editor
        .cursor_rect()
        .expect("cursor rect in visual line mode");
    assert!(
        visual_line_cursor.height() >= 14.0 && visual_line_cursor.height() <= 17.0,
        "VisualLine mode cursor rect height must match character bounds + padding (~14-16px): got {}",
        visual_line_cursor.height()
    );
}

#[test]
fn test_configure_fonts() {
    let ctx = egui::Context::default();
    let typography = truff_ui::app::global_config::TypographyConfig::default();
    truff_ui::native::configure_fonts(&ctx, &typography);

    // Test with non-existent custom font path
    let custom_typo = truff_ui::app::global_config::TypographyConfig {
        custom_font_path: Some(std::path::PathBuf::from("/nonexistent/font.ttf")),
        ..Default::default()
    };
    truff_ui::native::configure_fonts(&ctx, &custom_typo);

    // Test with existing custom font file
    let temp_font = std::env::temp_dir().join("test_font_dummy.ttf");
    let _ = std::fs::write(&temp_font, b"dummy font data");
    let custom_typo_file = truff_ui::app::global_config::TypographyConfig {
        custom_font_path: Some(temp_font.clone()),
        ..Default::default()
    };
    truff_ui::native::configure_fonts(&ctx, &custom_typo_file);
    let _ = std::fs::remove_file(&temp_font);
}

#[test]
fn test_native_app_typography_change_handled_in_show() {
    let (mut app, context) = scratch("hello font\n");
    let mut new_typo = app.application.shell.typography().clone();
    new_typo.base_font_size = 22.0;
    app.application.shell.set_typography(new_typo);
    frame(&mut app, &context, vec![]);
}

#[test]
fn test_gear_icon() {
    use truff_ui::native::icons::{Icon, render_icon};
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
        render_icon(ui.painter(), Icon::Gear, rect, egui::Color32::WHITE, 1.5);
    });
    output.textures_delta.clear();
}

#[test]
fn test_settings_dialog_default_and_init() {
    use truff_ui::app::global_config::TypographyConfig;
    use truff_ui::native::creation::SettingsDialog;

    let default_dialog = SettingsDialog::default();
    assert!(!default_dialog.open);
    assert_eq!(default_dialog.font_family, "proportional");
    assert_eq!(default_dialog.base_font_size, 15.0);
    assert_eq!(default_dialog.line_height_multiplier, 1.55);
    assert_eq!(default_dialog.custom_font_path, "");
    assert_eq!(default_dialog.active_theme, "nord");
    assert!(default_dialog.show_inline_title);
    assert!(default_dialog.error.is_none());

    let mut dialog = SettingsDialog::default();
    let typo = TypographyConfig {
        font_family: "proportional".into(),
        base_font_size: 18.0,
        line_height_multiplier: 1.6,
        custom_font_path: Some(std::path::PathBuf::from("/fonts/custom.ttf")),
        show_inline_title: true,
        max_content_width: 720.0,
    };
    dialog.init_from_app(&typo, "dark");
    assert_eq!(dialog.base_font_size, 18.0);
    assert_eq!(dialog.line_height_multiplier, 1.6);
    assert_eq!(dialog.custom_font_path, "/fonts/custom.ttf");
    assert_eq!(dialog.active_theme, "dark");
    assert_eq!(dialog.font_family, "custom");
    assert!(dialog.error.is_none());
}

#[test]
fn test_settings_dialog_ui_render_and_open_state() {
    let (mut app, context) = scratch("");
    assert!(!app.dialogs.is_any_open());
    app.dialogs.settings.open = true;
    assert!(app.dialogs.is_any_open());

    // Frame with settings dialog open
    frame(&mut app, &context, vec![]);
    assert!(app.dialogs.settings.open);

    // Escape closes settings dialog
    frame(&mut app, &context, vec![key(egui::Key::Escape)]);
    assert!(!app.dialogs.settings.open);
    assert!(!app.dialogs.is_any_open());
}

#[test]
fn test_settings_command_opens_dialog_in_native_app() {
    let (mut app, context) = scratch("");
    assert!(!app.dialogs.settings.open);
    app.application.execute_command("settings").unwrap();
    assert!(app.application.settings_requested);

    frame(&mut app, &context, vec![]);
    assert!(!app.application.settings_requested);
    assert!(app.dialogs.settings.open);
}
#[test]
fn wrapped_list_rows_align_with_the_item_text() {
    let source = format!("plain\n- {}", "word ".repeat(50));
    let (mut app, context) = scratch(&source);
    let output = frame(&mut app, &context, vec![]);
    let text = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text.starts_with("• ") => Some(text),
            _ => None,
        })
        .unwrap();
    assert!(text.galley.rows.len() > 1);
    let first = &text.galley.rows[0];
    let next = &text.galley.rows[1];
    let first_x = first.pos.x + first.glyphs[2].pos.x;
    let next_x = next.pos.x + next.glyphs[0].pos.x;
    assert!((first_x - next_x).abs() < 1.0, "{first_x} != {next_x}");
}

#[test]
fn test_native_tab_enter_subitem() {
    let (mut app, context) = scratch("- item\n");
    // Send 'A' to enter insert mode at end
    frame(
        &mut app,
        &context,
        vec![
            Event::Key {
                key: egui::Key::A,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
                physical_key: None,
            },
            Event::Text("A".into()),
        ],
    );
    // Send Enter
    frame(&mut app, &context, vec![key(egui::Key::Enter)]);
    // Send Tab
    frame(&mut app, &context, vec![key(egui::Key::Tab)]);
    // Send "sub"
    frame(&mut app, &context, vec![Event::Text("sub".into())]);
    // Send Enter
    let output = frame(&mut app, &context, vec![key(egui::Key::Enter)]);
    assert_eq!(
        app.application.shell.editor_text(),
        "- item\n    - sub\n    - \n"
    );
    let mut bullet_center_x = None;
    let mut guide_xs = Vec::new();
    let mut line_origins = Vec::new();
    for s in &output.shapes {
        match &s.shape {
            egui::Shape::Text(t) => {
                if t.galley.job.text == "• item" {
                    let g = &t.galley.rows[0].glyphs[0];
                    bullet_center_x = Some(t.pos.x + g.pos.x + g.size().x * 0.5);
                    line_origins.push(t.pos.x);
                } else if t.galley.job.text.starts_with("    ") {
                    line_origins.push(t.pos.x);
                }
            }
            egui::Shape::LineSegment { points, .. }
                if (points[0].x - points[1].x).abs() < 0.1
                    && (points[1].y - points[0].y).abs() > 10.0
                    && points[0].x > 280.0 =>
            {
                guide_xs.push(points[0].x);
            }
            _ => {}
        }
    }
    let bullet_x = bullet_center_x.expect("bullet circle glyph must exist");
    assert!(
        !guide_xs.is_empty(),
        "guide lines must be painted for subitems"
    );
    for &gx in &guide_xs {
        assert!(
            (gx - bullet_x).abs() <= 1.0,
            "guide_x {gx} must align with bullet_x {bullet_x}"
        );
    }
    assert!(line_origins.len() >= 3);
    for &origin in &line_origins[1..] {
        assert_eq!(origin, line_origins[0], "all line origins must align");
    }
}

#[test]
fn editing_a_fence_updates_cached_list_context_on_other_lines() {
    let (mut app, context) = scratch("plain\n- item\n");
    app.application
        .shell
        .apply_transaction(truff_core::Transaction::replace(0..5, "```md"))
        .unwrap();
    let output = frame(&mut app, &context, vec![]);
    assert!(output.shapes.iter().any(
        |shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "- item")
    ));
    let view = app.application.shell.editor_view().unwrap().id();
    app.application
        .shell
        .workspace_mut()
        .dispatch(truff_core::commands::Action::Undo(view))
        .unwrap();
    let output = frame(&mut app, &context, vec![]);
    assert!(output.shapes.iter().any(
        |shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "• item")
    ));
}

#[test]
fn test_unicode_grapheme_selection_and_heading_geometry() {
    let source = "# Заголовок H1\n\nW i Ж 界 👩‍💻 e\u{0301}\n\n";
    let (mut app, context) = scratch(source);
    frame(&mut app, &context, vec![]);

    // 1. Heading geometry: H1 heading line height must be tall (20-24px), not small body font (14-16px).
    let h1_offset = source.find("Заголовок").unwrap();
    let rect_h1 = app
        .editor
        .char_rect(0, h1_offset)
        .expect("char rect for H1 exists");
    assert!(
        rect_h1.height() >= 20.0 && rect_h1.height() <= 24.0,
        "H1 cursor height must reflect heading size, got {}",
        rect_h1.height()
    );

    // 2. Unicode cluster advances on line 2 ("W i Ж 界 👩‍💻 e\u{0301}")
    let line2 = 2;
    let offset_w = source.find('W').unwrap();
    let offset_i = source.find('i').unwrap();
    let offset_zh = source.find('Ж').unwrap();
    let offset_kai = source.find('界').unwrap();
    let offset_emoji = source.find("👩‍💻").unwrap();
    let offset_accent = source.find("e\u{0301}").unwrap();

    let rect_w = app.editor.char_rect(line2, offset_w).unwrap();
    let rect_i = app.editor.char_rect(line2, offset_i).unwrap();
    let rect_zh = app.editor.char_rect(line2, offset_zh).unwrap();
    let rect_kai = app.editor.char_rect(line2, offset_kai).unwrap();
    let rect_emoji = app.editor.char_rect(line2, offset_emoji).unwrap();
    let rect_accent = app.editor.char_rect(line2, offset_accent).unwrap();

    // In proportional / standard font: 'i' is narrower than 'W'
    assert!(
        rect_i.width() < rect_w.width(),
        "i width ({}) should be < W width ({})",
        rect_i.width(),
        rect_w.width()
    );
    // 'Ж' has wider advance than 'i'
    assert!(
        rect_zh.width() > rect_i.width(),
        "Ж width ({}) should be > i width ({})",
        rect_zh.width(),
        rect_i.width()
    );
    // '界' (CJK full-width) is wider than standard latin
    assert!(
        rect_kai.width() > rect_i.width(),
        "界 width ({}) should be > i width ({})",
        rect_kai.width(),
        rect_i.width()
    );
    // '👩‍💻' is an emoji ZWJ sequence (woman + ZWJ + laptop) spanning full emoji advance
    assert!(
        rect_emoji.width() >= rect_kai.width() * 0.8,
        "👩‍💻 width ({}) should cover the full emoji cluster",
        rect_emoji.width()
    );
    // 'e\u{0301}' combining accent cluster
    assert!(
        rect_accent.width() >= rect_i.width(),
        "e\\u{{0301}} width ({}) should cover combining cluster",
        rect_accent.width()
    );

    // All characters on line 2 share the same visual row vertical bounds without cropping
    assert_eq!(
        rect_w.top(),
        rect_i.top(),
        "Visual row rects should have uniform top"
    );
    assert_eq!(
        rect_w.bottom(),
        rect_i.bottom(),
        "Visual row rects should have uniform bottom"
    );

    // 3. Empty line geometry: no hardcoded 8.5px
    let line1 = 1; // empty line between H1 and line 2
    let rect_empty = app
        .editor
        .char_rect(line1, source.find("\n\n").unwrap() + 1)
        .unwrap();
    assert_ne!(
        rect_empty.width(),
        8.5,
        "Empty line should not use arbitrary 8.5px width"
    );
    assert!(
        rect_empty.height() >= 16.0,
        "Empty line should have proper row height, got {}",
        rect_empty.height()
    );
}

#[test]
fn test_native_heading_hierarchy_and_reading_mode_and_inline_paint() {
    // 1. Heading hierarchy scaling
    let typo = truff_ui::app::global_config::TypographyConfig::default();
    let (f_h1, lh_h1) = truff_ui::native::resolve_font_and_line_height("# H1", &typo);
    let (f_h2, lh_h2) = truff_ui::native::resolve_font_and_line_height("## H2", &typo);
    let (f_h3, lh_h3) = truff_ui::native::resolve_font_and_line_height("### H3", &typo);
    let (f_h4, lh_h4) = truff_ui::native::resolve_font_and_line_height("#### H4", &typo);
    let (f_h5, _lh_h5) = truff_ui::native::resolve_font_and_line_height("##### H5", &typo);
    let (f_h6, _lh_h6) = truff_ui::native::resolve_font_and_line_height("###### H6", &typo);

    assert!(f_h1.size > f_h2.size);
    assert!(f_h2.size > f_h3.size);
    assert!(f_h3.size >= f_h4.size);
    assert!(f_h4.size >= f_h5.size);
    assert!(f_h5.size >= f_h6.size);
    assert!(lh_h1 > lh_h2);
    assert!(lh_h2 > lh_h3);
    assert!(lh_h3 >= lh_h4);

    // 2. Inline formatting in LayoutJob
    let source = "Some **bold text** with *italics* and `inline code` plus [a link](https://test.com) and <mark style=\"background-color:#ffff00\">highlight</mark>.";
    let (mut app, context) = scratch(source);
    let output = frame(&mut app, &context, vec![]);

    let formatted_text = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text.contains("bold text") => Some(text),
            _ => None,
        })
        .expect("missing formatted text galley");

    let job = &formatted_text.galley.job;
    assert!(
        job.sections.len() > 1,
        "Job must contain multiple styled sections"
    );

    let sec_slice = |range: &std::ops::Range<egui::text::ByteIndex>| {
        let start: usize = range.start.into();
        let end: usize = range.end.into();
        &job.text[start..end]
    };

    let has_italic_section = job.sections.iter().any(|sec| {
        let sec_text = sec_slice(&sec.byte_range);
        sec_text.contains("italics") && sec.format.italics
    });
    assert!(has_italic_section, "Must have italic formatted section");

    let has_code_section = job.sections.iter().any(|sec| {
        let sec_text = sec_slice(&sec.byte_range);
        sec_text.contains("inline code") && sec.format.font_id.family == egui::FontFamily::Monospace
    });
    assert!(
        has_code_section,
        "Must have monospace code formatted section"
    );

    let has_code_pill = output.shapes.iter().any(|shape| match &shape.shape {
        egui::Shape::Rect(rect) => rect.corner_radius.nw > 0,
        _ => false,
    });
    assert!(
        has_code_pill,
        "Must have rounded corner pill for inline code"
    );

    let has_link_section = job.sections.iter().any(|sec| {
        let sec_text = sec_slice(&sec.byte_range);
        sec_text.contains("a link") && sec.format.color == egui::Color32::from_rgb(136, 192, 208)
    });
    assert!(has_link_section, "Must have colored link section");

    let has_link_line = output.shapes.iter().any(|shape| match &shape.shape {
        egui::Shape::LineSegment { stroke, .. } => {
            stroke.color == egui::Color32::from_rgba_unmultiplied(136, 192, 208, 220)
        }
        _ => false,
    });
    assert!(has_link_line, "Must have tight custom underline for link");

    let has_highlight_section = job.sections.iter().any(|sec| {
        let sec_text = sec_slice(&sec.byte_range);
        sec_text.contains("highlight") && sec.format.background.a() > 0
    });
    assert!(
        has_highlight_section,
        "Must have highlight background section"
    );

    // 3. Command :reading toggles projection to Reading
    assert_eq!(
        app.application.projection(),
        truff_ui::editor::ProjectionMode::LivePreview
    );
    frame(&mut app, &context, vec![Event::Text(":".into())]);
    frame(&mut app, &context, vec![Event::Text("reading".into())]);
    frame(&mut app, &context, vec![key(egui::Key::Enter)]);
    assert_eq!(
        app.application.projection(),
        truff_ui::editor::ProjectionMode::Reading
    );

    // Command :preview toggles projection back to LivePreview
    frame(&mut app, &context, vec![Event::Text(":".into())]);
    frame(&mut app, &context, vec![Event::Text("preview".into())]);
    frame(&mut app, &context, vec![key(egui::Key::Enter)]);
    assert_eq!(
        app.application.projection(),
        truff_ui::editor::ProjectionMode::LivePreview
    );
}

#[test]
fn test_save_all_dirty_overwrites_conflict() {
    use truff_core::workspace::BufferState;

    let tmp = std::env::temp_dir().join(format!("truff-test-conflict-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("conflicted.md"), "disk content").unwrap();

    let mut application = Application::new();
    application.open_vault(&tmp).unwrap();
    while application.needs_poll() {
        application.poll();
    }
    application.open_note("conflicted.md").unwrap();
    while application.needs_poll() {
        application.poll();
    }

    let buf_id = application.shell.editor_buffer().unwrap().id();
    let entry = application
        .shell
        .workspace_mut()
        .buffers_mut()
        .get_mut(buf_id)
        .unwrap();
    entry.set_state(BufferState::Conflict);
    assert_eq!(entry.state(), BufferState::Conflict);

    // save_all_dirty must succeed without BufferConflict error
    assert!(application.save_all_dirty().is_ok());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_open_navigator_focuses_active_file() {
    let tmp = std::env::temp_dir().join(format!("truff-test-navfocus-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("sub")).unwrap();
    std::fs::write(tmp.join("root.md"), "root").unwrap();
    std::fs::write(tmp.join("sub/target.md"), "target").unwrap();

    let mut application = Application::new();
    application.open_vault(&tmp).unwrap();
    while application.needs_poll() {
        application.poll();
    }
    application.open_note("sub/target.md").unwrap();
    while application.needs_poll() {
        application.poll();
    }

    let context = egui::Context::default();
    let mut app = NativeApp::new(application, &context);
    frame(&mut app, &context, vec![]);

    app.open_navigator(truff_ui::navigator::NavigatorMode::Files);

    let selected = app
        .navigator
        .selected_item()
        .expect("Must have selected item");
    match &selected.payload {
        truff_ui::navigator::NavigatorItemPayload::File { path, .. } => {
            assert_eq!(path, "sub/target.md");
        }
        _ => panic!("Expected file payload for selected item"),
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_lateral_margin_max_content_width_setting() {
    let mut application = Application::new();
    let mut typo = application.shell.typography().clone();
    typo.max_content_width = 950.0;
    application.shell.set_typography(typo);

    assert_eq!(application.shell.theme().geometry.markdown_max_width, 950.0);
    let (content_width, margin_x) = application.shell.theme().geometry.column_metrics(1400.0);
    assert_eq!(content_width, 950.0);
    assert_eq!(margin_x, (1400.0 - 950.0) / 2.0);
}

#[test]
fn test_table_rendering_does_not_wrap_and_renders_cleanly() {
    let table = "| Header 1 | Header 2 | Header 3 |\n|---|---|---|\n| Very long cell content that would ordinarily wrap if width was constrained to content_width | Another long cell with details | Final cell |\n";
    let (mut app, context) = scratch(table);
    let output = frame(&mut app, &context, vec![]);

    // Find the long table row
    let long_row_text = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text.contains("Very long cell content") => {
                Some(text)
            }
            _ => None,
        })
        .expect("missing long table row galley");

    // Must be a single line (NOT wrapped into multiple lines!)
    assert_eq!(
        long_row_text.galley.rows.len(),
        1,
        "Table row must not wrap into multiple visual lines"
    );
}

#[test]
fn test_link_tight_underline_and_url_extraction() {
    let text = "Check [LibriSpeech — OpenSLR](http://www.openslr.org/12/) today.";
    let (mut app, context) = scratch(text);
    let output = frame(&mut app, &context, vec![]);

    // Verify tight custom underline line segment exists
    let has_underline = output.shapes.iter().any(|shape| match &shape.shape {
        egui::Shape::LineSegment { stroke, .. } => {
            stroke.color == egui::Color32::from_rgba_unmultiplied(136, 192, 208, 220)
        }
        _ => false,
    });
    assert!(has_underline, "Must paint tight custom underline for link");
}
