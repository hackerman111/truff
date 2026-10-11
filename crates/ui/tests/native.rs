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
