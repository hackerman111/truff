use eframe::egui::{self, Event};
use truff_ui::{
    app::Application,
    native::NativeApp,
    navigator::{self, NavigatorItem, NavigatorMode, NavigatorState},
    theme::Theme,
};

fn input(size: egui::Vec2, events: Vec<Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        events,
        ..Default::default()
    }
}

fn color(value: truff_ui::theme::Color) -> egui::Color32 {
    let [r, g, b, a] = value.to_rgba_u8();
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn elevated_rect(output: &egui::FullOutput, theme: &Theme) -> egui::Rect {
    fn find(shape: &egui::Shape, fill: egui::Color32) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Rect(rect) if rect.fill == fill => Some(rect.rect),
            egui::Shape::Vec(shapes) => shapes
                .iter()
                .filter_map(|shape| find(shape, fill))
                .max_by(|a, b| a.area().total_cmp(&b.area())),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .filter_map(|shape| find(&shape.shape, color(theme.bg_elevated)))
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .expect("elevated panel painted")
}

#[test]
fn navigator_uses_available_space_and_keeps_last_row_visible_at_each_density_and_dpi() {
    for comfortable in [false, true] {
        let theme = if comfortable {
            Theme::nord().apply_custom(&truff_ui::theme::CustomThemeConfig {
                density: Some("comfortable".into()),
                ..Default::default()
            })
        } else {
            Theme::nord()
        };
        for (size, scale) in [
            (egui::vec2(800.0, 600.0), 1.0),
            (egui::vec2(1280.0, 720.0), 2.0),
            (egui::vec2(1920.0, 1080.0), 1.5),
        ] {
            let ctx = egui::Context::default();
            ctx.all_styles_mut(|style| style.animation_time = 0.0);
            ctx.set_pixels_per_point(scale);
            let mut state = NavigatorState::new();
            state.open_mode(
                NavigatorMode::Files,
                "Vault",
                (0..1000)
                    .map(|i| {
                        NavigatorItem::new_file(
                            i,
                            &format!("note-{i}"),
                            format!("folder/note-{i}.md"),
                            false,
                            0,
                        )
                    })
                    .collect(),
            );
            let mut output = ctx.run_ui(input(size, vec![Event::Text("G".into())]), |_| {
                navigator::show(&mut state, &ctx, &theme);
            });
            for _ in 0..3 {
                output.textures_delta.clear();
                output = ctx.run_ui(input(size, vec![]), |_| {
                    navigator::show(&mut state, &ctx, &theme);
                });
            }
            let rect = elevated_rect(&output, &theme);
            assert!(
                rect.width() >= (size.x * 0.80).min(1360.0),
                "{size:?}: {rect:?}"
            );
            assert!(rect.height() >= size.y * 0.72, "{size:?}: {rect:?}");
            assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(rect));
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::Text(text) if text.galley.job.text == "note-999" && text.galley.job.sections[0].format.font_id.size == theme.geometry.ui_font_size && shape.clip_rect.contains_rect(egui::Rect::from_min_size(text.pos,text.galley.size())))));
            output.textures_delta.clear();
        }
    }
}

#[test]
fn light_theme_reaches_native_widgets_and_creation_dialogs_fit_the_viewport() {
    for size in [
        egui::vec2(800.0, 600.0),
        egui::vec2(1280.0, 720.0),
        egui::vec2(1920.0, 1080.0),
    ] {
        for dialog in 0..4 {
            let ctx = egui::Context::default();
            ctx.all_styles_mut(|style| style.animation_time = 0.0);
            let theme = Theme::light();
            let mut application = Application::new();
            application.shell.set_editor_text("word text\n");
            application.shell.set_theme(theme.clone());
            let mut app = NativeApp::new(application, &ctx);
            match dialog {
                0 => app.dialogs.file.open = true,
                1 => app.dialogs.folder.open = true,
                2 => app.dialogs.vault.open = true,
                _ => app.dialogs.settings.open = true,
            }
            let mut output = ctx.run_ui(input(size, vec![]), |ui| app.show(ui));
            for _ in 0..2 {
                output.textures_delta.clear();
                output = ctx.run_ui(input(size, vec![]), |ui| app.show(ui));
            }
            let rect = elevated_rect(&output, &theme);
            assert!(
                rect.width() >= 480.0 && rect.width() <= size.x - 24.0,
                "dialog {dialog}, {rect:?}"
            );
            assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(rect));
            let visuals = ctx.global_style().visuals.clone();
            assert!(!visuals.dark_mode);
            assert_eq!(visuals.extreme_bg_color, color(theme.bg_canvas));
            assert_eq!(visuals.widgets.inactive.weak_bg_fill, color(theme.bg_panel));
            output.textures_delta.clear();
        }
    }
}

#[test]
fn opening_vault_keeps_the_active_theme_and_typography() {
    let path = std::env::temp_dir().join(format!("truff-p1-theme-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    let mut application = Application::new();
    let theme = Theme::light();
    let typography = truff_ui::app::global_config::TypographyConfig {
        font_family: "proportional".into(),
        base_font_size: 18.0,
        ..Default::default()
    };
    application.shell.set_theme(theme.clone());
    application.shell.set_typography(typography.clone());
    application.open_vault(&path).unwrap();
    let actual = application.shell.theme().clone();
    let actual_typography = application.shell.typography().clone();
    application.shutdown().unwrap();
    std::fs::remove_dir_all(&path).unwrap();
    assert_eq!(actual, theme);
    assert_eq!(actual_typography, typography);
}

#[test]
fn native_persists_egui_panel_sizes() {
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|style| style.animation_time = 0.0);
    let mut app = NativeApp::new(Application::new(), &ctx);
    let size = egui::vec2(1100.0, 700.0);
    let frame = |app: &mut NativeApp, events| {
        let mut output = ctx.run_ui(input(size, events), |ui| app.show(ui));
        output.textures_delta.clear();
    };
    frame(&mut app, vec![]);
    let before = egui::containers::panel::PanelState::load(&ctx, egui::Id::new("vault"))
        .unwrap()
        .size()
        .x;
    let start = egui::pos2(before, 200.0);
    frame(&mut app, vec![Event::PointerMoved(start)]);
    frame(
        &mut app,
        vec![Event::PointerButton {
            pos: start,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let end = start + egui::vec2(50.0, 0.0);
    frame(&mut app, vec![Event::PointerMoved(end)]);
    frame(
        &mut app,
        vec![Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let after = egui::containers::panel::PanelState::load(&ctx, egui::Id::new("vault"))
        .unwrap()
        .size()
        .x;
    assert!(after > before + 20.0);
    let mut storage = TestStorage::default();
    if eframe::App::persist_egui_memory(&app) {
        ctx.memory(|memory| eframe::set_value(&mut storage, "egui", memory));
    }
    let restored = eframe::get_value(&storage, "egui").expect("egui memory saved");
    let next = egui::Context::default();
    next.memory_mut(|memory| *memory = restored);
    let mut app = NativeApp::new(Application::new(), &next);
    let mut output = next.run_ui(input(size, vec![]), |ui| app.show(ui));
    output.textures_delta.clear();
    assert!(
        (egui::containers::panel::PanelState::load(&next, egui::Id::new("vault"))
            .unwrap()
            .size()
            .x
            - after)
            .abs()
            < 1.0
    );
}

#[derive(Default)]
struct TestStorage(std::collections::BTreeMap<String, String>);
impl eframe::Storage for TestStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.into(), value);
    }
    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }
    fn flush(&mut self) {}
}

#[test]
fn each_view_keeps_its_own_projection() {
    use truff_ui::editor::ProjectionMode;
    let mut app = Application::new();
    app.shell.set_editor_text("**word**\n");
    let first = app.shell.editor_view().unwrap().id();
    let buffer = app.shell.editor_buffer().unwrap().id();
    app.execute_command("source").unwrap();
    let second = app.shell.workspace_mut().create_view(buffer).unwrap();
    app.shell.workspace_mut().set_active_view(Some(second));
    assert_eq!(app.projection(), ProjectionMode::LivePreview);
    app.shell.workspace_mut().set_active_view(Some(first));
    assert_eq!(app.projection(), ProjectionMode::Source);
}

#[test]
fn focus_command_hides_docks_and_focus_cannot_move_into_a_hidden_pane() {
    let ctx = egui::Context::default();
    let mut app = Application::new();
    app.shell.set_editor_text("word text\n");
    app.execute_command("focus").unwrap();
    let mut native = NativeApp::new(app, &ctx);
    let size = egui::vec2(1100.0, 700.0);
    let mut output = ctx.run_ui(input(size, vec![]), |ui| native.show(ui));
    output.textures_delta.clear();
    assert!(!output.shapes.iter().any(
        |shape| matches!(&shape.shape,egui::Shape::Text(text) if text.galley.job.text == "VAULT")
    ));
    native
        .application
        .shell
        .set_sidebar_files(vec!["note.md".into()]);
    output = ctx.run_ui(
        input(
            size,
            vec![
                Event::Key {
                    key: egui::Key::W,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::CTRL,
                },
                Event::Text("h".into()),
            ],
        ),
        |ui| native.show(ui),
    );
    output.textures_delta.clear();
    assert_eq!(
        native.application.shell.focus(),
        truff_ui::shell::Focus::Editor
    );
    native.application.execute_command("focus").unwrap();
    output = ctx.run_ui(input(size, vec![]), |ui| native.show(ui));
    output.textures_delta.clear();
    assert!(output.shapes.iter().any(
        |shape| matches!(&shape.shape,egui::Shape::Text(text) if text.galley.job.text == "VAULT")
    ));
}

#[test]
fn comfortable_settings_actions_are_reachable_by_keyboard_in_a_small_window() {
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|style| style.animation_time = 0.0);
    let mut application = Application::new();
    application.shell.set_theme(
        Theme::light().apply_custom(&truff_ui::theme::CustomThemeConfig {
            density: Some("comfortable".into()),
            ..Default::default()
        }),
    );
    let mut app = NativeApp::new(application, &ctx);
    app.dialogs.settings.open = true;
    let size = egui::vec2(800.0, 600.0);
    let mut reached = false;
    let mut theme_reached = false;
    for step in 0..32 {
        let events = if step < 3 {
            vec![]
        } else {
            vec![Event::Key {
                key: egui::Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]
        };
        let mut output = ctx.run_ui(input(size, events), |ui| app.show(ui));
        reached |= output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == "Применить и сохранить"
                && shape.clip_rect.contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size()))));
        theme_reached |= output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == "light"
                && shape.clip_rect.contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size()))));
        output.textures_delta.clear();
    }
    assert!(
        theme_reached,
        "Tab must reveal theme choices in the scrolling body"
    );
    assert!(
        reached,
        "Tab must reveal the settings actions inside the scroll area"
    );
}

#[test]
fn both_docks_leave_a_usable_editor_in_a_small_comfortable_window() {
    use truff_store::index::{FileId, backlinks::BacklinkOccurrence};
    let ctx = egui::Context::default();
    let mut application = Application::new();
    application.shell.set_theme(
        Theme::nord().apply_custom(&truff_ui::theme::CustomThemeConfig {
            density: Some("comfortable".into()),
            ..Default::default()
        }),
    );
    application.shell.set_editor_text("word text\n");
    application.set_backlinks(vec![BacklinkOccurrence {
        source_file_id: FileId(1),
        source_path: "backlink-1.md".into(),
        occurrence_ordinal: 0,
        range: 0..1,
        raw_target: "test".into(),
        heading: None,
        label: None,
        is_wiki: true,
    }]);
    let mut app = NativeApp::new(application, &ctx);
    let size = egui::vec2(800.0, 600.0);
    let mut output = ctx.run_ui(input(size, vec![]), |ui| app.show(ui));
    for _ in 0..3 {
        output.textures_delta.clear();
        output = ctx.run_ui(input(size, vec![]), |ui| app.show(ui));
    }
    let left = egui::containers::panel::PanelState::load(&ctx, egui::Id::new("vault"))
        .unwrap()
        .outer_rect;
    let right = egui::containers::panel::PanelState::load(&ctx, egui::Id::new("references"))
        .unwrap()
        .outer_rect;
    assert!(right.left() - left.right() >= 240.0);
    assert!(
        app.editor
            .cursor_rect()
            .is_some_and(|caret| caret.left() > left.right() && caret.right() < right.left())
    );
    for needle in ["word text", "backlink-1.md"] {
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text)
            if text.galley.job.text.contains(needle)
                && shape.clip_rect.contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size())))), "{needle} must fit its pane");
    }
    output.textures_delta.clear();
}
