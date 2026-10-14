#![cfg(unix)]

use std::{fs, os::unix::fs::symlink, path::PathBuf};
use truff_core::{
    text::BufferId,
    workspace::{SnapshotKey, VaultEpoch},
};
use truff_store::recovery::RecoveryManager;

struct Vault(PathBuf);
impl Vault {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "truff-recovery-directory-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Vault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn key(revision: u64) -> SnapshotKey {
    SnapshotKey::new(VaultEpoch::new(1), BufferId::new(1), revision)
}

#[test]
fn canonical_vault_boundary_accepts_alias_above_root() {
    let base = Vault::new("alias");
    let root = base.0.join("actual/vault");
    fs::create_dir_all(&root).unwrap();
    symlink(base.0.join("actual"), base.0.join("alias")).unwrap();
    let manager = RecoveryManager::new(base.0.join("alias/vault"), 4096);
    manager
        .write_snapshot(key(1), "note.md", "draft", None)
        .unwrap();
    assert_eq!(manager.recovery_dir(), root.join(".truff/recovery"));
    assert_eq!(
        manager
            .recover_latest_valid_snapshot(BufferId::new(1))
            .unwrap()
            .content,
        "draft"
    );
}

#[test]
fn replaced_internal_directory_is_checked_after_parent_cache_is_warm() {
    let vault = Vault::new("internal");
    let outside = Vault::new("outside");
    let manager = RecoveryManager::new(vault.0.clone(), 4096);
    manager
        .write_snapshot(key(1), "note.md", "draft", None)
        .unwrap();
    fs::rename(vault.0.join(".truff"), vault.0.join("old-internal")).unwrap();
    symlink(&outside.0, vault.0.join(".truff")).unwrap();
    assert!(
        manager
            .write_snapshot(key(2), "note.md", "new draft", None)
            .is_err()
    );
    assert!(manager.current_storage_bytes().is_err());
    assert!(manager.delete_snapshots(BufferId::new(1)).is_err());
    assert!(fs::read_dir(&outside.0).unwrap().next().is_none());
}

#[test]
fn vault_root_replaced_by_symlink_is_rejected() {
    let base = Vault::new("root");
    let root = base.0.join("vault");
    let outside = base.0.join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    let manager = RecoveryManager::new(root.clone(), 4096);
    manager
        .write_snapshot(key(1), "note.md", "draft", None)
        .unwrap();
    fs::rename(&root, base.0.join("old-vault")).unwrap();
    symlink(&outside, &root).unwrap();
    assert!(
        manager
            .write_snapshot(key(2), "note.md", "new draft", None)
            .is_err()
    );
    assert!(fs::read_dir(outside).unwrap().next().is_none());
}
