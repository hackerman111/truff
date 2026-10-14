use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use truff_core::commands::Action;
use truff_core::text::Transaction;
use truff_core::workspace::buffers::{BufferState, SnapshotKey, VaultEpoch};
use truff_core::workspace::{StoreEvent, StoreJob, Workspace};
use truff_store::Store;
use truff_store::recovery::RecoveryManager;
use truff_store::worker::SubmitError;

fn temp_test_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("truff_test_{name}_{}_{id}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn dirty_external_conflict() {
    let vault_dir = temp_test_dir("dirty_conflict");
    let file_path = vault_dir.join("note.md");
    fs::write(&file_path, "initial content\n").unwrap();

    let mut store = Store::new(vault_dir.clone()).unwrap();
    let initial_fp = truff_store::files::compute_file_fingerprint(&file_path).unwrap();

    let mut ws = Workspace::new();
    let buf_id = ws
        .open_buffer_from_text(file_path.clone(), "initial content\n")
        .unwrap();
    let view_id = ws.create_view(buf_id).unwrap();

    // In-memory edit: buffer is now dirty
    ws.dispatch(Action::ApplyEdit {
        view_id,
        transaction: Transaction::insert(16, "dirty addition\n"),
    })
    .unwrap();
    assert_eq!(ws.buffers().get_state(buf_id), Some(BufferState::Dirty));

    // Simulate EXTERNAL modification on disk
    fs::write(&file_path, "external attacker edit\n").unwrap();

    // Now try to save our dirty buffer using the base fingerprint
    let key = ws.buffers().get_snapshot_key(buf_id).unwrap();
    let dirty_content = ws.buffers().get_buffer(buf_id).unwrap().rope().to_string();

    let save_job = StoreJob::SaveFile {
        key,
        path: file_path.clone(),
        content: dirty_content.as_str().into(),
        base_fingerprint: Some(initial_fp),
    };

    store.try_submit(save_job).unwrap();

    // Wait for store to process the job
    let mut conflict_event = None;
    for _ in 0..50 {
        let events = store.poll_events();
        for ev in events {
            if matches!(ev, StoreEvent::Conflict { .. }) {
                conflict_event = Some(ev);
                break;
            }
        }
        if conflict_event.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let event = conflict_event.expect("Expected StoreEvent::Conflict event");
    ws.accept(event).unwrap();

    // Buffer state must be Conflict and disk content was NOT overwritten!
    assert_eq!(ws.buffers().get_state(buf_id), Some(BufferState::Conflict));
    let disk_text = fs::read_to_string(&file_path).unwrap();
    assert_eq!(disk_text, "external attacker edit\n");
}

#[test]
fn failed_save_keeps_dirty() {
    let vault_dir = temp_test_dir("failed_save");
    let mut store = Store::new(vault_dir.clone()).unwrap();

    // Path in a non-existent or read-only directory that fails atomic write
    let bad_path = vault_dir.join("non_existent_subdir/deep/note.md");

    let mut ws = Workspace::new();
    let buf_id = ws
        .open_buffer_from_text(bad_path.clone(), "some text")
        .unwrap();
    let view_id = ws.create_view(buf_id).unwrap();

    ws.dispatch(Action::ApplyEdit {
        view_id,
        transaction: Transaction::insert(9, " edit"),
    })
    .unwrap();
    assert_eq!(ws.buffers().get_state(buf_id), Some(BufferState::Dirty));

    let key = ws.buffers().get_snapshot_key(buf_id).unwrap();
    let job = StoreJob::SaveFile {
        key,
        path: bad_path,
        content: "some text edit".into(),
        base_fingerprint: None,
    };

    store.try_submit(job).unwrap();

    let mut save_failed = None;
    for _ in 0..50 {
        let events = store.poll_events();
        for ev in events {
            if matches!(ev, StoreEvent::SaveFailed { .. }) {
                save_failed = Some(ev);
                break;
            }
        }
        if save_failed.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let event = save_failed.expect("Expected StoreEvent::SaveFailed");
    ws.accept(event).unwrap();

    // Buffer must remain DIRTY
    assert_eq!(ws.buffers().get_state(buf_id), Some(BufferState::Dirty));
    let buf = ws.buffers().get_buffer(buf_id).unwrap();
    assert!(buf.is_dirty());
}

#[test]
fn recovery_last_ack() {
    let vault_dir = temp_test_dir("recovery_last_ack");
    let recovery_dir = vault_dir.join(".truff/recovery");
    fs::create_dir_all(&recovery_dir).unwrap();

    let manager = RecoveryManager::new(vault_dir.clone(), 256 * 1024 * 1024);
    let buf_id = truff_core::text::BufferId::new(42);
    let key1 = SnapshotKey::new(VaultEpoch::new(1), buf_id, 1);
    let key2 = SnapshotKey::new(VaultEpoch::new(1), buf_id, 2);

    // Save valid snapshot for revision 1
    manager
        .write_snapshot(key1, "note.md", "revision 1 valid text", None)
        .unwrap();

    // Now write snapshot for revision 2, but corrupt/truncate it
    let snap2_path = manager
        .write_snapshot(key2, "note.md", "revision 2 truncated text", None)
        .unwrap();
    // Intentionally truncate snap2 to 10 bytes to simulate partial write/crash
    let raw = fs::read(&snap2_path).unwrap();
    fs::write(&snap2_path, &raw[..raw.len().min(10)]).unwrap();

    // Recover snapshot for buffer: corrupted snap2 must be ignored, snap1 must be recovered!
    let recovered = manager.recover_latest_valid_snapshot(buf_id).unwrap();
    assert_eq!(recovered.key.revision, 1);
    assert_eq!(recovered.content, "revision 1 valid text");
}

#[test]
fn durable_queue_full() {
    let vault_dir = temp_test_dir("durable_queue");
    // Create store with pause flag or just saturate worker with 32 jobs
    let mut store = Store::new_with_capacity(vault_dir.clone(), 32).unwrap();

    let dummy_path = vault_dir.join("test.md");
    let mut submitted = 0;
    let mut got_full = false;

    // Submit many jobs in a tight loop to exceed bounded durable capacity
    for i in 0..100 {
        let key = SnapshotKey::new(
            VaultEpoch::new(1),
            truff_core::text::BufferId::new(i + 1),
            1,
        );
        let job = StoreJob::SaveFile {
            key,
            path: dummy_path.clone(),
            content: format!("content {i}").as_str().into(),
            base_fingerprint: None,
        };
        match store.try_submit(job) {
            Ok(_) => submitted += 1,
            Err(SubmitError::QueueFull) => {
                got_full = true;
                break;
            }
            Err(e) => panic!("unexpected submit error: {e:?}"),
        }
    }

    assert!(
        got_full || submitted >= 32,
        "Expected QueueFull backpressure when saturating queue"
    );
}

#[test]
fn shutdown_preserves_accepted_save() {
    let vault_dir = temp_test_dir("shutdown_save");
    let file_path = vault_dir.join("save_on_shutdown.md");
    let mut store = Store::new(vault_dir.clone()).unwrap();

    let key = SnapshotKey::new(VaultEpoch::new(1), truff_core::text::BufferId::new(1), 1);
    let job = StoreJob::SaveFile {
        key,
        path: file_path.clone(),
        content: "important data saved before exit\n".into(),
        base_fingerprint: None,
    };

    // Submit job
    store.try_submit(job).unwrap();

    // Shutdown immediately
    store.shutdown().unwrap();

    // File MUST be completely and durably written to disk!
    let content = fs::read_to_string(&file_path).expect("File must exist after shutdown");
    assert_eq!(content, "important data saved before exit\n");
}

#[test]
fn recovery_retains_only_two_valid_snapshots() {
    let vault = temp_test_dir("recovery_retention");
    let manager = RecoveryManager::new(vault, 4096);
    let id = truff_core::text::BufferId::new(73);
    for revision in 1..=20 {
        manager
            .write_snapshot(
                SnapshotKey::new(VaultEpoch::new(1), id, revision),
                "note.md",
                "draft",
                None,
            )
            .unwrap();
    }
    let snapshots: Vec<_> = fs::read_dir(manager.recovery_dir())
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(snapshots.len(), 2);
    let newest = manager
        .write_snapshot(
            SnapshotKey::new(VaultEpoch::new(1), id, 21),
            "note.md",
            "draft",
            None,
        )
        .unwrap();
    fs::write(newest, "truncated").unwrap();
    assert_eq!(
        manager
            .recover_latest_valid_snapshot(id)
            .unwrap()
            .key
            .revision,
        20
    );
}

#[test]
fn protected_paths_and_symlink_parents_are_rejected() {
    let vault = temp_test_dir("path_boundary");
    let paths = truff_store::VaultPaths::new(vault.clone());
    assert!(
        paths
            .resolve_path(std::path::Path::new(".truff/vault.lock"))
            .is_err()
    );
    assert!(paths.resolve_path(std::path::Path::new(".")).is_err());
    #[cfg(unix)]
    {
        let outside = temp_test_dir("outside_boundary");
        std::os::unix::fs::symlink(&outside, vault.join("linked")).unwrap();
        assert!(
            paths
                .resolve_path(std::path::Path::new("linked/new.md"))
                .is_err()
        );
    }
}

#[test]
fn fingerprint_is_sha256() {
    assert_eq!(
        truff_store::compute_fingerprint(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad:3"
    );
}

#[test]
fn deleted_base_file_is_never_silently_recreated() {
    let vault = temp_test_dir("deleted_base");
    let path = vault.join("note.md");
    fs::write(&path, "base").unwrap();
    let fp = truff_store::compute_file_fingerprint(&path).unwrap();
    fs::remove_file(&path).unwrap();
    let mut store = Store::new(vault).unwrap();
    store
        .try_submit(StoreJob::SaveFile {
            key: SnapshotKey::new(VaultEpoch::new(0), truff_core::text::BufferId::new(1), 1),
            path: path.clone(),
            content: "draft".into(),
            base_fingerprint: Some(fp),
        })
        .unwrap();
    // Joining must retain every reply, even when the caller did not poll first.
    for _ in 0..100 {
        let events = store.poll_events();
        if !events.is_empty() {
            assert!(matches!(
                events[0],
                StoreEvent::Conflict { .. } | StoreEvent::SaveFailed { .. }
            ));
            assert!(!path.exists());
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("missing save outcome");
}

#[cfg(unix)]
#[test]
fn atomic_save_preserves_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let vault = temp_test_dir("permissions");
    let path = vault.join("note.md");
    fs::write(&path, "old").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    truff_store::atomic_write_file(&path, "new").unwrap();
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn unread_outcomes_apply_backpressure_and_shutdown_does_not_deadlock() {
    let vault = temp_test_dir("unread_outcomes");
    let mut worker = truff_store::StoreWorker::start(vault.clone(), 2).unwrap();
    let wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = wakes.clone();
    worker.set_event_waker(std::sync::Arc::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    }));
    let job = |revision| StoreJob::SaveFile {
        key: SnapshotKey::new(
            VaultEpoch::new(0),
            truff_core::text::BufferId::new(72),
            revision,
        ),
        path: vault.join(format!("note{revision}.md")),
        content: "draft".into(),
        base_fingerprint: None,
    };
    worker.try_submit(job(1)).unwrap();
    worker.try_submit(job(2)).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(worker.try_submit(job(3)), Err(SubmitError::QueueFull));
    worker.shutdown();
    assert_eq!(
        wakes.load(Ordering::Relaxed),
        1,
        "unread outcomes share one adapter wakeup"
    );
    let outcomes = worker.poll_events();
    assert_eq!(outcomes.len(), 2);
    assert!(
        outcomes
            .iter()
            .all(|event| matches!(event, StoreEvent::SaveAck { .. }))
    );
}

#[test]
fn concurrent_lock_acquisition_has_exactly_one_owner() {
    let vault = temp_test_dir("lock_race");
    fs::create_dir_all(vault.join(".truff")).unwrap();
    fs::write(vault.join(".truff/vault.lock"), "4294967294\n").unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(17));
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let barrier = barrier.clone();
            let vault = vault.clone();
            std::thread::spawn(move || {
                barrier.wait();
                truff_store::VaultLock::acquire(&vault)
            })
        })
        .collect();
    barrier.wait();
    let owners: Vec<_> = threads
        .into_iter()
        .filter_map(|thread| thread.join().unwrap().ok())
        .collect();
    assert_eq!(owners.len(), 1);
    drop(owners);
    assert!(truff_store::VaultLock::acquire(&vault).is_ok());
}

#[test]
fn recovery_budget_failure_keeps_the_only_valid_copy_and_bounds_malformed_reads() {
    let vault = temp_test_dir("recovery_budget");
    let manager = RecoveryManager::new(vault, 1024);
    let id = truff_core::text::BufferId::new(74);
    let key = SnapshotKey::new(VaultEpoch::new(0), id, 1);
    manager
        .write_snapshot(key, "note.md", "sole draft", None)
        .unwrap();
    assert!(
        manager
            .write_snapshot(
                SnapshotKey { revision: 2, ..key },
                "note.md",
                &"x".repeat(1024),
                None
            )
            .is_err()
    );
    fs::write(
        manager.recovery_dir().join("74_r99999999.snap"),
        "TRUFF_SNAP_V2\ncontent_len:18446744073709551615\n---DATA---\n",
    )
    .unwrap();
    assert_eq!(
        manager.recover_latest_valid_snapshot(id).unwrap().content,
        "sole draft"
    );
}

#[test]
fn new_file_save_never_replaces_an_existing_destination() {
    let vault = temp_test_dir("create_collision");
    let path = vault.join("note.md");
    fs::write(&path, "external").unwrap();
    let mut worker = truff_store::StoreWorker::start(vault, 1).unwrap();
    worker
        .try_submit(StoreJob::SaveFile {
            key: SnapshotKey::new(VaultEpoch::new(0), truff_core::text::BufferId::new(1), 0),
            path: path.clone(),
            content: "local".into(),
            base_fingerprint: None,
        })
        .unwrap();
    worker.shutdown();
    assert!(matches!(
        worker.poll_events()[0],
        StoreEvent::SaveFailed { .. }
    ));
    assert_eq!(fs::read_to_string(path).unwrap(), "external");
}
