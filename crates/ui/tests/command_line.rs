use truff_core::vim::Key;
use truff_ui::shell::{Shell, ShellAction};

fn type_text(shell: &mut Shell, text: &str) {
    for ch in text.chars() {
        shell.feed_key(Key::Char(ch));
    }
}

#[test]
fn escape_first_leaves_command_insert_then_closes_command_line() {
    let mut shell = Shell::new();
    type_text(&mut shell, ":new session.md");
    shell.feed_key(Key::Esc);
    assert_eq!(shell.command(), Some("new session.md"));
    shell.feed_key(Key::Esc);
    assert_eq!(shell.command(), None);
}

#[test]
fn command_history_is_selected_in_normal_and_insert_text_is_literal() {
    let mut shell = Shell::new();
    type_text(&mut shell, ":new glass.md");
    assert_eq!(
        shell.feed_key(Key::Enter),
        ShellAction::Command("new glass.md".into())
    );
    type_text(&mut shell, ":");
    shell.feed_key(Key::Esc);
    shell.feed_key(Key::Char('k'));
    assert_eq!(shell.command(), Some("new glass.md"));
    shell.feed_key(Key::Char('i'));
    type_text(&mut shell, " jkd :session");
    assert_eq!(shell.command(), Some("new glass.md jkd :session"));
    assert!(!shell.flash().is_active());
}

#[test]
fn registry_id_command_uses_the_registered_keyboard_route() {
    let mut shell = Shell::new();
    type_text(&mut shell, ":panel.files");
    assert_eq!(
        shell.feed_key(Key::Enter),
        ShellAction::OpenNavigator(truff_ui::navigator::NavigatorMode::Files)
    );
}
