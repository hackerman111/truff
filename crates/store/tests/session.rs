use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use truff_core::text::BufferId;
use truff_core::workspace::buffers::{SnapshotKey, VaultEpoch};
use truff_store::VaultPaths;
use truff_store::recovery::RecoveryManager;
use truff_store::session::{
    BufferRestoreState, SessionBufferEntry, SessionData, SessionViewEntry, restore_session,
    save_session,
};

fn temp_test_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("truff_test_{name}_{}_{id}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn restore_active_first() {
    let temp_path = temp_test_dir("session_active_first");
    let vault = VaultPaths::new(temp_path.clone());
    let recovery = RecoveryManager::new(temp_path.clone(), 10 * 1024 * 1024);

    let active_path = temp_path.join("active.md");
    let inactive_path = temp_path.join("inactive.md");
    fs::write(&active_path, "# Active Note\nResident content").unwrap();
    fs::write(&inactive_path, "# Inactive Note\nLazy content").unwrap();

    let session = SessionData {
        version: 1,
        vault_path: temp_path.clone(),
        active_buffer: Some(PathBuf::from("active.md")),
        buffers: vec![
            SessionBufferEntry {
                path: PathBuf::from("active.md"),
                is_dirty: false,
                last_saved_fingerprint: None,
            },
            SessionBufferEntry {
                path: PathBuf::from("inactive.md"),
                is_dirty: false,
                last_saved_fingerprint: None,
            },
        ],
        views: vec![
            SessionViewEntry {
                buffer_path: PathBuf::from("active.md"),
                cursor_anchor: 5,
                cursor_head: 5,
                top_line: 0,
            },
            SessionViewEntry {
                buffer_path: PathBuf::from("inactive.md"),
                cursor_anchor: 0,
                cursor_head: 0,
                top_line: 0,
            },
        ],
        active_view_index: Some(0),
        jump_history: vec![(0, 5)],
    };

    save_session(&vault, &session).unwrap();

    let restored = restore_session(&vault, &recovery).unwrap();

    // 1. Metadata restored immediately
    assert_eq!(restored.buffers.len(), 2);
    assert_eq!(restored.views.len(), 2);
    assert_eq!(
        restored.active_buffer_path,
        Some(PathBuf::from("active.md"))
    );

    // 2. Active buffer is loaded first into memory
    let active_entry = restored
        .buffers
        .iter()
        .find(|b| b.path == Path::new("active.md"))
        .unwrap();
    assert!(matches!(active_entry.state, BufferRestoreState::Loaded(_)));
    if let BufferRestoreState::Loaded(ref text) = active_entry.state {
        assert_eq!(text, "# Active Note\nResident content");
    }

    // 3. Inactive buffer is left lazy (not resident in memory)
    let inactive_entry = restored
        .buffers
        .iter()
        .find(|b| b.path == Path::new("inactive.md"))
        .unwrap();
    assert!(matches!(inactive_entry.state, BufferRestoreState::Lazy));
}

#[test]
fn missing_file_session() {
    let temp_path = temp_test_dir("session_missing_file");
    let vault = VaultPaths::new(temp_path.clone());
    let recovery = RecoveryManager::new(temp_path.clone(), 10 * 1024 * 1024);

    let session = SessionData {
        version: 1,
        vault_path: temp_path.clone(),
        active_buffer: Some(PathBuf::from("missing.md")),
        buffers: vec![SessionBufferEntry {
            path: PathBuf::from("missing.md"),
            is_dirty: false,
            last_saved_fingerprint: None,
        }],
        views: vec![SessionViewEntry {
            buffer_path: PathBuf::from("missing.md"),
            cursor_anchor: 0,
            cursor_head: 0,
            top_line: 0,
        }],
        active_view_index: Some(0),
        jump_history: Vec::new(),
    };

    save_session(&vault, &session).unwrap();

    // Missing file must NOT panic and should be marked as MissingOnDisk
    let restored = restore_session(&vault, &recovery).unwrap();
    assert_eq!(restored.buffers.len(), 1);
    let entry = &restored.buffers[0];
    assert_eq!(entry.path, PathBuf::from("missing.md"));
    assert!(matches!(entry.state, BufferRestoreState::MissingOnDisk));
}

#[test]
fn closed_view_dirty_buffer() {
    let temp_path = temp_test_dir("session_closed_view_dirty");
    let vault = VaultPaths::new(temp_path.clone());
    let recovery = RecoveryManager::new(temp_path.clone(), 10 * 1024 * 1024);

    // Disk file exists
    let note_path = temp_path.join("draft.md");
    fs::write(&note_path, "Clean original text").unwrap();

    // Emergency snapshot was written for buffer 42 before the view/tab was closed
    let snap_key = SnapshotKey::new(VaultEpoch::new(1), BufferId::new(42), 5);
    recovery
        .write_snapshot(
            snap_key,
            "draft.md",
            "Unsaved dirty draft text",
            Some("clean_fp"),
        )
        .unwrap();

    // Session has no views open for draft.md (view was closed), or only other tabs
    let session = SessionData {
        version: 1,
        vault_path: temp_path.clone(),
        active_buffer: None,
        buffers: Vec::new(),
        views: Vec::new(),
        active_view_index: None,
        jump_history: Vec::new(),
    };

    save_session(&vault, &session).unwrap();

    let restored = restore_session(&vault, &recovery).unwrap();

    // Unsaved dirty buffer is detected and available from recovery snapshots
    assert_eq!(restored.recovered_dirty_buffers.len(), 1);
    let dirty_entry = &restored.recovered_dirty_buffers[0];
    assert_eq!(dirty_entry.rel_path, "draft.md");
    assert_eq!(dirty_entry.content, "Unsaved dirty draft text");
}
