use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};
use truff_core::{
    commands::Action,
    text::{BufferId, Transaction},
    workspace::BufferState,
};
use truff_store::{DEFAULT_MAX_RECOVERY_STORAGE, RecoveryManager};
use truff_ui::{app::Application, shell::ShellAction};

struct Vault(PathBuf);
impl Vault {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "truff-buffer-retention-{label}-{}-{}",
            std::process::id(),
            BufferId::next()
        ));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("a.md"), "large document\n".repeat(80_000)).unwrap();
        fs::write(path.join("b.md"), "second document").unwrap();
        Self(path)
    }
    fn recovery(&self) -> RecoveryManager {
        RecoveryManager::new(self.0.clone(), DEFAULT_MAX_RECOVERY_STORAGE)
    }
}
impl Drop for Vault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn idle(app: &mut Application) {
    let start = Instant::now();
    while app.needs_poll() {
        app.poll();
        assert!(start.elapsed() < Duration::from_secs(15), "{}", app.status);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn closed_clean_text_is_released_and_reopened_from_disk_with_same_identity() {
    let vault = Vault::new("reopen");
    let mut app = Application::new();
    app.open_vault(&vault.0).unwrap();
    idle(&mut app);
    app.open_note("a.md").unwrap();
    idle(&mut app);
    let view = app.shell.editor_view().unwrap().id();
    let id = app.shell.editor_buffer().unwrap().id();
    let revision = app.shell.editor_buffer().unwrap().revision();
    app.open_note("b.md").unwrap();
    idle(&mut app);
    app.handle_shell_action(ShellAction::Core(Action::CloseView(view)));
    assert!(app.shell.workspace().buffers().get_buffer(id).is_some());
    idle(&mut app);
    let entry = app.shell.workspace().buffers().get(id).unwrap();
    assert_eq!(entry.state(), BufferState::Unloaded);
    assert!(entry.buffer().is_none());
    assert_eq!(entry.path(), Some(vault.0.join("a.md").as_path()));
    assert!(entry.fingerprint().is_some());
    assert!(
        vault
            .recovery()
            .recover_all_latest_snapshots()
            .unwrap()
            .is_empty()
    );
    fs::write(vault.0.join("a.md"), "changed while closed 字🦀").unwrap();
    app.poll();
    assert!(app.shell.workspace().buffers().get_buffer(id).is_none());
    app.open_note("a.md").unwrap();
    idle(&mut app);
    assert_eq!(app.shell.editor_buffer().unwrap().id(), id);
    assert!(app.shell.editor_buffer().unwrap().revision() > revision);
    assert_eq!(app.shell.editor_text(), "changed while closed 字🦀");
    assert!(!app.shell.editor_buffer().unwrap().is_dirty());
    app.shutdown().unwrap();
}

#[test]
fn pending_save_and_recovery_finish_before_closed_buffer_is_released() {
    let vault = Vault::new("save");
    let mut app = Application::new();
    app.open_vault(&vault.0).unwrap();
    idle(&mut app);
    app.open_note("a.md").unwrap();
    idle(&mut app);
    let view = app.shell.editor_view().unwrap().id();
    let id = app.shell.editor_buffer().unwrap().id();
    app.handle_shell_action(ShellAction::Core(Action::ApplyEdit {
        view_id: view,
        transaction: Transaction::replace(
            0..app.shell.editor_buffer().unwrap().byte_len(),
            "saved draft",
        ),
    }));
    idle(&mut app);
    assert_eq!(
        vault
            .recovery()
            .recover_latest_valid_snapshot(id)
            .unwrap()
            .content,
        "saved draft"
    );
    app.execute_command("w").unwrap();
    app.handle_shell_action(ShellAction::Core(Action::CloseView(view)));
    assert!(app.shell.workspace().buffers().get_buffer(id).is_some());
    idle(&mut app);
    assert_eq!(
        fs::read_to_string(vault.0.join("a.md")).unwrap(),
        "saved draft"
    );
    assert!(app.shell.workspace().buffers().get_buffer(id).is_none());
    assert!(
        vault
            .recovery()
            .recover_all_latest_snapshots()
            .unwrap()
            .is_empty()
    );
    app.shutdown().unwrap();
}

#[test]
fn dirty_closed_buffer_stays_resident_and_recoverable() {
    let vault = Vault::new("dirty");
    let mut app = Application::new();
    app.open_vault(&vault.0).unwrap();
    idle(&mut app);
    let view = app.shell.editor_view().unwrap().id();
    let id = app.shell.editor_buffer().unwrap().id();
    app.handle_shell_action(ShellAction::Core(Action::ApplyEdit {
        view_id: view,
        transaction: Transaction::insert(0, "unsaved "),
    }));
    app.handle_shell_action(ShellAction::Core(Action::CloseView(view)));
    idle(&mut app);
    assert!(
        app.shell
            .workspace()
            .buffers()
            .get_buffer(id)
            .unwrap()
            .is_dirty()
    );
    assert!(
        vault
            .recovery()
            .recover_latest_valid_snapshot(id)
            .unwrap()
            .content
            .starts_with("unsaved ")
    );
    app.shutdown().unwrap();
}

#[cfg(unix)]
#[test]
fn failed_recovery_cleanup_keeps_the_resident_copy() {
    let vault = Vault::new("failed-cleanup");
    let mut app = Application::new();
    app.open_vault(&vault.0).unwrap();
    idle(&mut app);
    let view = app.shell.editor_view().unwrap().id();
    let id = app.shell.editor_buffer().unwrap().id();
    let recovery_dir = vault.recovery().recovery_dir().to_path_buf();
    fs::create_dir_all(&recovery_dir).unwrap();
    fs::rename(&recovery_dir, vault.0.join("old-recovery")).unwrap();
    std::os::unix::fs::symlink(vault.0.join("old-recovery"), &recovery_dir).unwrap();
    app.handle_shell_action(ShellAction::Core(Action::CloseView(view)));
    idle(&mut app);
    assert!(app.shell.workspace().buffers().get_buffer(id).is_some());
    assert!(app.status.contains("Save failed"), "{}", app.status);
    fs::remove_file(&recovery_dir).unwrap();
    fs::rename(vault.0.join("old-recovery"), recovery_dir).unwrap();
    app.shutdown().unwrap();
}
