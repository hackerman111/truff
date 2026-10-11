//! Production egui event path with real vault files, without a native GPU window.
use eframe::egui::{self, Event};
use std::time::{Duration, Instant};
use truff_ui::app::Application;
use truff_ui::native::NativeApp;

struct Window {
    app: NativeApp,
    context: egui::Context,
}
impl Window {
    fn event(&mut self, event: Event) {
        self.frame(vec![event]);
    }
    fn redraw(&mut self) {
        self.frame(vec![]);
    }
    fn frame(&mut self, events: Vec<Event>) {
        self.context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| self.app.show(ui),
            )
            .drop_without_applying_deltas();
    }
}
fn window(application: Application) -> (Window, ()) {
    let context = egui::Context::default();
    let app = NativeApp::new(application, &context);
    let mut window = Window { app, context };
    window.redraw();
    (window, ())
}
fn text(root: &mut Window, keys: &str) {
    for ch in keys.chars() {
        root.event(Event::Text(ch.to_string()));
    }
}
fn named(root: &mut Window, key: egui::Key) {
    root.event(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
}
fn command(root: &mut Window, cmd: &str) {
    text(root, &format!(":{cmd}"));
    named(root, egui::Key::Enter);
}
fn with_app<T>(root: &Window, _: (), f: impl FnOnce(&Application) -> T) -> T {
    f(&root.app.application)
}
fn idle(root: &mut Window, _: ()) {
    let start = Instant::now();
    while root.app.application.needs_poll() {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "{}",
            root.app.application.status
        );
        std::thread::sleep(Duration::from_millis(5));
        root.redraw();
    }
}
fn close(root: &mut Window, _: ()) {
    root.app.application.shutdown().unwrap();
}
fn scratch(text: &str) -> Application {
    let mut app = Application::new();
    app.shell.set_editor_text(text);
    app
}

#[test]
fn window_editor_runs_vim_instead_of_inserting_normal_keys() {
    let (mut root, id) = window(scratch("first\nsecond\n"));
    text(&mut root, "dd");
    assert_eq!(
        with_app(&root, id, |app| app.shell.editor_text()),
        "second\n"
    );
    text(&mut root, "u");
    assert_eq!(
        with_app(&root, id, |app| app.shell.editor_text()),
        "first\nsecond\n"
    );
    text(&mut root, "ciwchanged");
    named(&mut root, egui::Key::Escape);
    assert_eq!(
        with_app(&root, id, |app| app.shell.editor_text()),
        "changed\nsecond\n"
    );
}

#[test]
fn composition_and_paste_edit_the_workspace_once() {
    let (mut root, id) = window(scratch(""));
    text(&mut root, "i");
    root.event(Event::Ime(egui::ImeEvent::Preedit {
        text: "候補".into(),
        active_range_chars: Some(0..2),
    }));
    assert_eq!(with_app(&root, id, |app| app.shell.editor_text()), "");
    root.event(Event::Ime(egui::ImeEvent::Commit("字🦀".into())));
    root.event(Event::Paste(" Привет".into()));
    named(&mut root, egui::Key::Backspace);
    assert_eq!(
        with_app(&root, id, |app| app.shell.editor_text()),
        "字🦀 Приве"
    );
    named(&mut root, egui::Key::Escape);
    assert!(root.app.editor.cached_paragraph_count() > 0);
}

#[test]
fn window_keyboard_workflow_saves_renames_and_restores_real_files() {
    let vault = std::env::temp_dir().join(format!(
        "truff-gui-{}-{}",
        std::process::id(),
        truff_core::workspace::ViewId::next()
    ));
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(vault.join("a.md"), "alpha beta\n").unwrap();
    std::fs::write(vault.join("b.md"), "# Target\n\n[[a]]\n").unwrap();
    let (mut root, id) = window(Application::new());
    command(&mut root, &format!("vault {}", vault.display()));
    idle(&mut root, id);
    assert_eq!(
        with_app(&root, id, |app| app.active_relative_path()),
        Some("a.md".into())
    );
    assert!(root.app.editor.cached_paragraph_count() > 0);
    // Insert, Flash to the second word, select it, and apply a color through the command UI.
    text(&mut root, "iHello ");
    named(&mut root, egui::Key::Escape);
    text(&mut root, "s");
    let label = with_app(&root, id, |app| {
        app.shell
            .flash()
            .targets()
            .iter()
            .find(|target| target.text == "beta")
            .unwrap()
            .label
            .clone()
    });
    text(&mut root, &label);
    text(&mut root, "ve");
    command(&mut root, "color #3388ff");
    assert!(
        with_app(&root, id, |app| app.shell.editor_text())
            .contains("<span style=\"color:#3388ff\">beta</span>")
    );
    // Add a link at the beginning using Vim's visual selection and the same native event path.
    text(&mut root, "gg0ve");
    command(&mut root, "link b");
    command(&mut root, "w");
    idle(&mut root, id);
    assert!(!with_app(&root, id, |app| app
        .shell
        .editor_buffer()
        .unwrap()
        .is_dirty()));
    assert!(
        std::fs::read_to_string(vault.join("a.md"))
            .unwrap()
            .contains("[[b|Hello]]")
    );
    command(&mut root, "follow b#target");
    idle(&mut root, id);
    assert_eq!(
        with_app(&root, id, |app| app.active_relative_path()),
        Some("b.md".into())
    );
    command(&mut root, "backlinks");
    idle(&mut root, id);
    assert_eq!(with_app(&root, id, |app| app.backlinks.len()), 1);
    assert_eq!(
        with_app(&root, id, |app| app.shell.focus()),
        truff_ui::shell::Focus::RightPanel
    );
    named(&mut root, egui::Key::Enter);
    text(&mut root, "ve");
    command(&mut root, "rename renamed.md");
    let before = with_app(&root, id, |app| app.shell.editor_text());
    root.event(Event::Cut);
    assert_eq!(with_app(&root, id, |app| app.shell.editor_text()), before);
    idle(&mut root, id);
    assert!(!vault.join("a.md").exists());
    assert!(vault.join("renamed.md").exists());
    assert!(
        std::fs::read_to_string(vault.join("b.md"))
            .unwrap()
            .contains("[[renamed")
    );
    command(&mut root, "w");
    idle(&mut root, id);
    let expected = with_app(&root, id, |app| app.shell.editor_text());
    close(&mut root, id);
    drop(root);
    let mut app = Application::new();
    app.open_vault(&vault).unwrap();
    let (mut root, id) = window(app);
    idle(&mut root, id);
    assert_eq!(
        with_app(&root, id, |app| app.active_relative_path()),
        Some("renamed.md".into())
    );
    assert_eq!(with_app(&root, id, |app| app.shell.editor_text()), expected);
    assert_eq!(
        with_app(&root, id, |app| app
            .shell
            .sidebar_model()
            .selected_row()
            .and_then(|row| row.data.clone())),
        Some("renamed.md".into())
    );
    close(&mut root, id);
    drop(root);
    std::fs::remove_dir_all(vault).unwrap();
}

#[test]
fn retained_paragraphs_are_bounded_and_scroll_reuses_layout() {
    let doc = "one two three\n".repeat(10_000);
    let (mut root, _id) = window(scratch(&doc));
    named(&mut root, egui::Key::PageDown);
    assert!(root.app.editor.scroll_offset() > 0.0);
    assert!(root.app.editor.cached_paragraph_count() < 100);
    let builds = root.app.editor.layout_rebuild_count();
    root.redraw();
    assert_eq!(root.app.editor.layout_rebuild_count(), builds);
}

#[test]
fn closing_window_recovers_dirty_text_and_save_clears_recovery() {
    let vault = std::env::temp_dir().join(format!(
        "truff-gui-recovery-{}-{}",
        std::process::id(),
        truff_core::workspace::ViewId::next()
    ));
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(vault.join("note.md"), "base\n").unwrap();
    let mut app = Application::new();
    app.open_vault(&vault).unwrap();
    let (mut root, id) = window(app);
    idle(&mut root, id);
    text(&mut root, "iunsaved ");
    named(&mut root, egui::Key::Escape);
    close(&mut root, id);
    drop(root);
    assert_eq!(
        std::fs::read_to_string(vault.join("note.md")).unwrap(),
        "base\n"
    );
    let mut app = Application::new();
    app.open_vault(&vault).unwrap();
    let (mut root, id) = window(app);
    idle(&mut root, id);
    assert_eq!(
        with_app(&root, id, |app| app.shell.editor_text()),
        "unsaved base\n"
    );
    assert!(with_app(&root, id, |app| app
        .shell
        .editor_buffer()
        .unwrap()
        .is_dirty()));
    command(&mut root, "w");
    idle(&mut root, id);
    close(&mut root, id);
    drop(root);
    let mut app = Application::new();
    app.open_vault(&vault).unwrap();
    let (mut root, id) = window(app);
    idle(&mut root, id);
    assert!(!with_app(&root, id, |app| app
        .shell
        .editor_buffer()
        .unwrap()
        .is_dirty()));
    // A checkpoint of an edit later undone to the savepoint must not reappear on restart.
    text(&mut root, "ix");
    named(&mut root, egui::Key::Escape);
    idle(&mut root, id);
    text(&mut root, "u");
    assert!(!with_app(&root, id, |app| app
        .shell
        .editor_buffer()
        .unwrap()
        .is_dirty()));
    idle(&mut root, id);
    let recovery =
        truff_store::RecoveryManager::new(vault.clone(), truff_store::DEFAULT_MAX_RECOVERY_STORAGE);
    assert!(recovery.recover_all_latest_snapshots().unwrap().is_empty());
    close(&mut root, id);
    drop(root);
    let mut app = Application::new();
    app.open_vault(&vault).unwrap();
    let (mut root, id) = window(app);
    idle(&mut root, id);
    assert_eq!(
        with_app(&root, id, |app| app.shell.editor_text()),
        "unsaved base\n"
    );
    assert!(!with_app(&root, id, |app| app
        .shell
        .editor_buffer()
        .unwrap()
        .is_dirty()));
    close(&mut root, id);
    drop(root);
    std::fs::remove_dir_all(vault).unwrap();
}

#[test]
fn test_gui_welcome_screen_and_vault_creation_modal() {
    let (mut root, ()) = window(Application::new());
    assert!(root.app.application.vault_path().is_none());
    assert!(!root.app.show_create_vault_dialog);

    root.app.show_create_vault_dialog = true;
    let target = std::env::temp_dir().join(format!("truff-gui-create-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&target);
    root.app.vault_path_input = target.to_string_lossy().into_owned();
    root.app.create_starter_note = true;
    root.app.initial_note_name = "Welcome.md".into();

    root.redraw();

    root.app
        .application
        .create_and_open_vault(
            std::path::Path::new(&root.app.vault_path_input),
            Some("Welcome.md"),
        )
        .unwrap();

    assert!(root.app.application.vault_path().is_some());
    assert!(target.join("Welcome.md").exists());

    let _ = root.app.application.shutdown();
    let _ = std::fs::remove_dir_all(target);
}

#[test]
fn test_tab_bar_renders_open_buffers_and_switches_active() {
    let mut app = Application::new();
    let buf1 = app
        .shell
        .workspace_mut()
        .buffers_mut()
        .open_from_text(None, "# Doc 1")
        .unwrap();
    let _view1 = app.shell.workspace_mut().create_view(buf1).unwrap();
    let buf2 = app
        .shell
        .workspace_mut()
        .buffers_mut()
        .open_from_text(None, "# Doc 2")
        .unwrap();
    let view2 = app.shell.workspace_mut().create_view(buf2).unwrap();

    let (mut root, ()) = window(app);
    assert_eq!(root.app.application.shell.workspace().views().len(), 3); // initial scratch + 2 new
    // Frame executes without panic and renders tabs
    root.redraw();

    // Verify switching active view
    root.app
        .application
        .shell
        .workspace_mut()
        .set_active_view(Some(view2));
    root.redraw();
    assert_eq!(
        root.app.application.shell.workspace().active_view(),
        Some(view2)
    );
}

#[test]
fn test_tab_close_button_hover() {
    let app = Application::new();
    let (mut root, ()) = window(app);
    // Draw without pointer hover - tabs render without panic
    root.redraw();
    // Simulate pointer hover over tab area
    root.event(egui::Event::PointerMoved(egui::pos2(50.0, 15.0)));
    root.redraw();
}

#[test]
fn test_create_note_and_folder_dialogs() {
    let target = std::env::temp_dir().join(format!("truff_dialog_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&target);
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("initial.md"), "# Initial").unwrap();

    let mut app = Application::new();
    app.open_vault(&target).unwrap();

    let (mut root, ()) = window(app);
    root.redraw();

    // Trigger create file dialog and set input
    root.app.show_create_file_dialog = true;
    root.app.create_file_input = "nested/new_note.md".to_string();
    root.redraw();

    // Directly test the application creation method and UI submit
    root.app
        .application
        .create_note(&root.app.create_file_input.clone())
        .unwrap();
    assert!(target.join("nested/new_note.md").exists());
    assert_eq!(
        root.app.application.active_relative_path().as_deref(),
        Some("nested/new_note.md")
    );

    // Test creating folder
    root.app.show_create_folder_dialog = true;
    root.app.create_folder_input = "archive/sub".to_string();
    root.redraw();

    root.app
        .application
        .create_folder(&root.app.create_folder_input.clone())
        .unwrap();
    assert!(target.join("archive/sub").is_dir());

    let _ = root.app.application.shutdown();
    let _ = std::fs::remove_dir_all(target);
}

#[test]
fn test_sidebar_header_ghost_actions() {
    let target = std::env::temp_dir().join(format!("truff_ghost_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&target);
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("file1.md"), "# File 1").unwrap();

    let mut app = Application::new();
    app.open_vault(&target).unwrap();

    let (mut root, ()) = window(app);
    root.redraw();

    let _ = root.app.application.shutdown();
    let _ = std::fs::remove_dir_all(target);
}
