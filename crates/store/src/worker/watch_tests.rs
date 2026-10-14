use super::*;
use std::collections::HashMap;
use truff_core::text::BufferId;
use truff_core::workspace::{SnapshotKey, VaultEpoch};

#[test]
fn identical_refresh_skips_full_scan_but_preserves_pending_external_changes() {
    let root = std::env::temp_dir().join(format!("truff-watch-refresh-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("note.md");
    std::fs::write(&path, "base").unwrap();
    let paths = VaultPaths::new(root.clone());
    let key = SnapshotKey::new(VaultEpoch::new(1), BufferId::new(1), 1);
    let base = files::compute_file_fingerprint(&path).unwrap();
    let mut state = WatchState {
        watched: HashMap::new(),
        dirty: Arc::default(),
    };
    state.track(WatchedFile {
        key,
        path: path.clone(),
        fingerprint: base.clone(),
    });
    let targets = Mutex::new(Some(vec![(key, path.clone(), Some(base.clone()))]));
    state.refresh_targets(&paths, &targets);
    assert!(!state.dirty.lock().unwrap().all);
    assert!(state.dirty.lock().unwrap().paths.is_empty());

    std::fs::write(&path, "external").unwrap();
    state.dirty.lock().unwrap().paths.insert(path.clone());
    let next_key = SnapshotKey { revision: 2, ..key };
    *targets.lock().unwrap() = Some(vec![(next_key, path.clone(), Some(base))]);
    state.refresh_targets(&paths, &targets);
    assert!(!state.dirty.lock().unwrap().all);
    let (tx, rx) = std::sync::mpsc::sync_channel(4);
    state.reconcile(&paths, &tx, &Mutex::default(), &AtomicBool::new(false));
    assert!(
        matches!(rx.try_recv().unwrap(), StoreEvent::ExternalChanged { key, .. } if key == next_key)
    );
    // A different epoch is a new target and must be checked against its supplied base.
    let epoch_key = SnapshotKey {
        epoch: VaultEpoch::new(2),
        ..next_key
    };
    *targets.lock().unwrap() = Some(vec![(epoch_key, path.clone(), Some("stale".into()))]);
    state.refresh_targets(&paths, &targets);
    assert!(state.dirty.lock().unwrap().paths.contains(&path));
    state.reconcile(&paths, &tx, &Mutex::default(), &AtomicBool::new(false));
    assert!(
        matches!(rx.try_recv().unwrap(), StoreEvent::ExternalChanged { key, .. } if key == epoch_key)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn reconciliation_still_rejects_a_watched_path_replaced_with_a_symlink() {
    let root = std::env::temp_dir().join(format!("truff-watch-symlink-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("note.md");
    std::fs::write(&path, "base").unwrap();
    let paths = VaultPaths::new(root.clone());
    let key = SnapshotKey::new(VaultEpoch::new(1), BufferId::new(2), 1);
    let fingerprint = files::compute_file_fingerprint(&path).unwrap();
    let mut state = WatchState {
        watched: HashMap::new(),
        dirty: Arc::default(),
    };
    state.track(WatchedFile {
        key,
        path: path.clone(),
        fingerprint,
    });
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", &path).unwrap();
    state.dirty.lock().unwrap().paths.insert(path);
    let (tx, rx) = std::sync::mpsc::sync_channel(4);
    state.reconcile(&paths, &tx, &Mutex::default(), &AtomicBool::new(false));
    assert!(
        matches!(rx.try_recv().unwrap(), StoreEvent::ExternalChanged { new_fingerprint, .. } if new_fingerprint.is_empty())
    );
    std::fs::remove_dir_all(root).unwrap();
}
