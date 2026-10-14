//! Production egui event/paint path, without a platform window or GPU.
use eframe::egui::{self, Event};
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

    // 1. Heading line: '#' character highlight matches character height + padding, not full 40px line height
    let heading_cursor = app.editor.cursor_rect().expect("heading cursor rect");
    assert!(
        heading_cursor.height() >= 23.0 && heading_cursor.height() <= 27.0,
        "Heading highlight height must match glyph height + padding (~24-26px), not full line height 40.0: got {}",
        heading_cursor.height()
    );
    assert!(
        heading_cursor.width() > 15.0,
        "Heading character width must match glyph advance width + padding (> 15.0): got {}",
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
    assert_eq!(default_dialog.font_family, "monospace");
    assert_eq!(default_dialog.base_font_size, 15.0);
    assert_eq!(default_dialog.line_height_multiplier, 1.5);
    assert_eq!(default_dialog.custom_font_path, "");
    assert_eq!(default_dialog.active_theme, "nord");
    assert!(default_dialog.error.is_none());

    let mut dialog = SettingsDialog::default();
    let typo = TypographyConfig {
        font_family: "proportional".into(),
        base_font_size: 18.0,
        line_height_multiplier: 1.6,
        custom_font_path: Some(std::path::PathBuf::from("/fonts/custom.ttf")),
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
