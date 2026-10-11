use std::time::{Duration, Instant};

use truff_core::text::BufferId;
use truff_core::workspace::{SnapshotKey, VaultEpoch};
use truff_store::{DEFAULT_MAX_RECOVERY_STORAGE, RecoveryManager};
use truff_ui::app::Application;

fn idle(app: &mut Application) {
    let start = Instant::now();
    while app.needs_poll() {
        app.poll();
        assert!(start.elapsed() < Duration::from_secs(15), "{}", app.status);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn clean_buffer_command_edits_schedule_and_write_idle_recovery() {
    for command in ["link target", "color #3388ff"] {
        let vault = std::env::temp_dir().join(format!(
            "truff-command-recovery-{}-{}",
            std::process::id(),
            BufferId::next()
        ));
        std::fs::create_dir_all(&vault).unwrap();
        std::fs::write(vault.join("note.md"), "word").unwrap();
        let mut app = Application::new();
        app.open_vault(&vault).unwrap();
        idle(&mut app);
        let id = app.shell.workspace().active_view().unwrap();
        app.shell
            .workspace_mut()
            .get_view_mut(id)
            .unwrap()
            .set_cursor(
                truff_core::text::BytePos::ZERO,
                truff_core::text::BytePos::new(4),
            );
        assert!(!app.shell.editor_buffer().unwrap().is_dirty());
        assert!(app.recovery_deadline().is_none());
        app.execute_command(command).unwrap();
        assert!(app.recovery_deadline().is_some());
        let expected = app.shell.editor_text();
        idle(&mut app);
        let recovery = RecoveryManager::new(vault.clone(), DEFAULT_MAX_RECOVERY_STORAGE);
        let snapshots = recovery.recover_all_latest_snapshots().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].content, expected);
        assert_eq!(
            std::fs::read_to_string(vault.join("note.md")).unwrap(),
            "word"
        );
        app.shutdown().unwrap();
        drop(app);
        std::fs::remove_dir_all(vault).unwrap();
    }
}

#[test]
fn restored_snapshots_use_ids_outside_the_historical_namespace() {
    let old_id = BufferId::next().get() + 10_000;
    let vault =
        std::env::temp_dir().join(format!("truff-id-recovery-{}-{old_id}", std::process::id()));
    std::fs::create_dir_all(&vault).unwrap();
    let recovery = RecoveryManager::new(vault.clone(), DEFAULT_MAX_RECOVERY_STORAGE);
    for (id, path, text) in [(old_id, "b.md", "dirty b"), (old_id + 1, "a.md", "dirty a")] {
        std::fs::write(vault.join(path), "disk").unwrap();
        recovery
            .write_snapshot(
                SnapshotKey::new(VaultEpoch::new(0), BufferId::new(id), 1),
                path,
                text,
                None,
            )
            .unwrap();
    }
    let mut app = Application::new();
    app.open_vault(&vault).unwrap();
    for path in ["a.md", "b.md"] {
        let id = app
            .shell
            .workspace()
            .buffers()
            .get_by_path(&vault.join(path))
            .unwrap();
        assert!(id.get() > old_id + 1);
    }
    // These must survive an immediate second crash, before shutdown can rewrite them.
    let snapshots = recovery.recover_all_latest_snapshots().unwrap();
    assert_eq!(snapshots.len(), 2);
    assert!(
        snapshots
            .iter()
            .any(|s| s.rel_path == "a.md" && s.content == "dirty a")
    );
    assert!(
        snapshots
            .iter()
            .any(|s| s.rel_path == "b.md" && s.content == "dirty b")
    );
    app.shutdown().unwrap();
    std::fs::remove_dir_all(vault).unwrap();
}

#[test]
fn rename_reloads_more_resident_backlinks_than_the_storage_queue_holds() {
    let vault = std::env::temp_dir().join(format!(
        "truff-many-reloads-{}-{}",
        std::process::id(),
        BufferId::next()
    ));
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(vault.join("target.md"), "target\n").unwrap();
    let text = format!("[[target]]\n{}\n", "a".repeat(200_000));
    for index in 0..60 {
        std::fs::write(vault.join(format!("note{index:02}.md")), &text).unwrap();
    }
    let mut app = Application::new();
    app.open_vault(&vault).unwrap();
    idle(&mut app);
    let mut ids = Vec::new();
    for index in 0..60 {
        ids.push(
            app.shell
                .workspace_mut()
                .open_buffer_from_text(vault.join(format!("note{index:02}.md")), &text)
                .unwrap(),
        );
    }
    app.open_note("target.md").unwrap();
    idle(&mut app);
    app.execute_command("rename changed.md").unwrap();
    idle(&mut app);
    // Don't hang Drop if the assertion exposes the regression.
    let frozen = app.file_operation_pending();
    if frozen {
        std::mem::forget(app);
        panic!("rename leaves native input and shutdown permanently frozen");
    }
    for id in ids {
        assert!(
            app.shell
                .workspace()
                .buffers()
                .get_buffer(id)
                .unwrap()
                .as_str()
                .starts_with("[[changed]]")
        );
    }
    app.shutdown().unwrap();
    std::fs::remove_dir_all(vault).unwrap();
}

fn wait_for(app: &mut Application, condition: impl Fn(&Application) -> bool) {
    let start = Instant::now();
    while !condition(app) {
        app.poll();
        assert!(start.elapsed() < Duration::from_secs(5), "{}", app.status);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn actual_watcher_reloads_clean_conflicts_dirty_and_suppresses_own_saves() {
    use truff_core::commands::Action;
    use truff_core::text::Transaction;
    use truff_core::workspace::BufferState;
    use truff_ui::shell::ShellAction;
    let vault = std::env::temp_dir().join(format!(
        "truff-watcher-{}-{}",
        std::process::id(),
        BufferId::next()
    ));
    std::fs::create_dir_all(&vault).unwrap();
    let path = vault.join("note.md");
    std::fs::write(&path, "base").unwrap();
    let mut app = Application::new();
    let wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = wakes.clone();
    app.set_storage_waker(std::sync::Arc::new(move || {
        count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }));
    app.open_vault(&vault).unwrap();
    idle(&mut app);
    let id = app.shell.editor_view().unwrap().buffer_id();
    let view = app.shell.editor_view().unwrap().id();
    std::fs::write(&path, "clean external").unwrap();
    wait_for(&mut app, |app| app.shell.editor_text() == "clean external");
    let base = app
        .shell
        .workspace()
        .buffers()
        .get(id)
        .unwrap()
        .fingerprint()
        .unwrap()
        .to_owned();
    app.handle_shell_action(ShellAction::Core(Action::ApplyEdit {
        view_id: view,
        transaction: Transaction::insert(14, " draft"),
    }));
    assert!(
        app.recovery_deadline()
            .unwrap()
            .duration_since(Instant::now())
            <= Duration::from_millis(750)
    );
    std::fs::write(&path, "dirty external").unwrap();
    wait_for(&mut app, |app| {
        app.shell.workspace().buffers().get_state(id) == Some(BufferState::Conflict)
    });
    assert_eq!(app.shell.editor_text(), "clean external draft");
    assert_eq!(
        app.shell
            .workspace()
            .buffers()
            .get(id)
            .unwrap()
            .fingerprint(),
        Some(base.as_str())
    );
    assert!(app.execute_command("w").is_err());
    app.execute_command("reload!").unwrap();
    wait_for(&mut app, |app| app.shell.editor_text() == "dirty external");
    app.handle_shell_action(ShellAction::Core(Action::ApplyEdit {
        view_id: view,
        transaction: Transaction::insert(14, " saved"),
    }));
    app.execute_command("w").unwrap();
    wait_for(&mut app, |app| {
        app.shell.workspace().buffers().get_state(id) == Some(BufferState::Clean)
    });
    let revision = app.shell.editor_buffer().unwrap().revision();
    let deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < deadline {
        app.poll();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        app.shell.editor_buffer().unwrap().revision(),
        revision,
        "own save must not reload"
    );
    assert_eq!(
        app.shell.workspace().buffers().get_state(id),
        Some(BufferState::Clean)
    );
    assert!(wakes.load(std::sync::atomic::Ordering::Relaxed) > 0);
    app.shutdown().unwrap();
    std::fs::remove_dir_all(vault).unwrap();
}
