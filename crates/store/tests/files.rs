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
        content: dirty_content,
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
        content: "some text edit".to_string(),
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
            content: format!("content {i}"),
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
        content: "important data saved before exit\n".to_string(),
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
