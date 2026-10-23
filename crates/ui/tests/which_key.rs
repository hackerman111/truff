use truff_core::vim::{Key, Mode};
use truff_ui::app::Application;
use truff_ui::overlays::{WhichKeyMode, WhichKeyOverlay};
use truff_ui::shell::{Shell, ShellAction};

#[test]
fn test_which_key_modes_and_help_categories() {
    let mut overlay = WhichKeyOverlay::new();
    assert!(!overlay.is_visible());
    assert_eq!(overlay.mode(), WhichKeyMode::Prefix);

    let default_cats = WhichKeyOverlay::default_help_categories();
    assert!(!default_cats.is_empty());
    assert!(
        default_cats
            .iter()
            .any(|c| c.name.contains("Навигация") || c.name.contains("Motions"))
    );

    overlay.show_help(default_cats);
    assert!(overlay.is_visible());
    assert_eq!(overlay.mode(), WhichKeyMode::Help);
    assert!(!overlay.categories().is_empty());

    overlay.hide();
    assert!(!overlay.is_visible());
    assert!(overlay.hints().is_empty());
    assert!(overlay.categories().is_empty());
}

#[test]
fn test_question_mark_in_normal_mode_opens_help() {
    let mut shell = Shell::new();
    shell.set_editor_text("sample markdown\n");
    assert_eq!(shell.mode(), Mode::Normal);

    let action = shell.feed_key(Key::Char('?'));
    assert_eq!(action, ShellAction::Incomplete);
    assert!(shell.which_key().is_visible());
    assert_eq!(shell.which_key().mode(), WhichKeyMode::Help);
    assert_eq!(shell.editor_text(), "sample markdown\n");

    // Esc closes it
    let esc_action = shell.feed_key(Key::Esc);
    assert!(!shell.which_key().is_visible());
    assert_ne!(
        esc_action,
        ShellAction::Error("Editor target unavailable".into())
    );
}

#[test]
fn test_question_mark_in_insert_mode_inserts_char() {
    let mut shell = Shell::new();
    shell.set_editor_text("sample\n");
    shell.feed_key(Key::Char('i'));
    assert_eq!(shell.mode(), Mode::Insert);

    // In insert mode, ? is an edit, not which-key help
    shell.feed_key(Key::Char('?'));
    assert!(!shell.which_key().is_visible());
}

#[test]
fn test_help_command_opens_which_key_help() {
    let mut app = Application::new();
    let res = app.execute_command("help");
    assert!(res.is_ok());
    assert!(app.shell.which_key().is_visible());
    assert_eq!(app.shell.which_key().mode(), WhichKeyMode::Help);
}

#[test]
fn test_gui_renders_which_key_overlay() {
    let ctx = egui::Context::default();
    let mut app = truff_ui::native::NativeApp::new(Application::new(), &ctx);
    app.application.shell.feed_key(Key::Char('?'));
    assert!(app.application.shell.which_key().is_visible());

    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1100.0, 700.0),
            )),
            ..Default::default()
        },
        |ui| {
            app.show(ui);
        },
    )
    .drop_without_applying_deltas();
}
