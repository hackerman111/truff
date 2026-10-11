use super::jobs::process_job;
use crate::files::{WriteStage, inject_write_failure};
use crate::recovery::RecoveryManager;
use std::path::PathBuf;
use truff_core::workspace::{StoreEvent, StoreJob};

#[test]
fn failed_write_stages_never_ack_or_remove_recovery() {
    let vault = std::env::temp_dir().join(format!("truff-fault-stages-{}", std::process::id()));
    std::fs::create_dir_all(&vault).unwrap();
    let paths = crate::VaultPaths::new(vault.clone());
    let recovery = RecoveryManager::new(vault.clone(), 4096);
    let key = truff_core::workspace::SnapshotKey::new(
        truff_core::workspace::VaultEpoch::new(0),
        truff_core::text::BufferId::new(919),
        1,
    );
    for stage in [
        WriteStage::Write,
        WriteStage::Sync,
        WriteStage::Rename,
        WriteStage::DirectorySync,
    ] {
        let path = vault.join("note.md");
        std::fs::write(&path, "base").unwrap();
        recovery
            .write_snapshot(key, "note.md", "draft", None)
            .unwrap();
        inject_write_failure(Some(stage));
        let (event, _) = process_job(
            &paths,
            &recovery,
            StoreJob::OverwriteFile {
                key,
                path: path.clone(),
                content: "draft".into(),
            },
        );
        inject_write_failure(None);
        assert!(matches!(event, StoreEvent::SaveFailed { .. }));
        assert_eq!(
            recovery
                .recover_latest_valid_snapshot(key.buffer_id)
                .unwrap()
                .content,
            "draft"
        );
        if stage != WriteStage::DirectorySync {
            assert_eq!(std::fs::read_to_string(path).unwrap(), "base");
        }
    }
    std::fs::remove_dir_all(vault).unwrap();
}

#[test]
fn recovery_faults_never_ack_and_preserve_the_previous_copy() {
    let vault = std::env::temp_dir().join(format!("truff-recovery-faults-{}", std::process::id()));
    std::fs::create_dir_all(&vault).unwrap();
    let paths = crate::VaultPaths::new(vault.clone());
    let recovery = RecoveryManager::new(vault.clone(), 4096);
    let old = truff_core::workspace::SnapshotKey::new(
        truff_core::workspace::VaultEpoch::new(0),
        truff_core::text::BufferId::new(920),
        1,
    );
    let previous = recovery
        .write_snapshot(old, "note.md", "previous", None)
        .unwrap();
    for (index, stage) in [
        WriteStage::Write,
        WriteStage::Sync,
        WriteStage::Rename,
        WriteStage::DirectorySync,
    ]
    .into_iter()
    .enumerate()
    {
        let key = truff_core::workspace::SnapshotKey {
            revision: index as u64 + 2,
            ..old
        };
        inject_write_failure(Some(stage));
        let (event, _) = process_job(
            &paths,
            &recovery,
            StoreJob::WriteRecoverySnapshot {
                key,
                path: PathBuf::from("note.md"),
                content: "new draft".into(),
                base_fingerprint: None,
            },
        );
        inject_write_failure(None);
        assert!(matches!(event, StoreEvent::SaveFailed { .. }));
        assert!(previous.exists());
    }
    std::fs::remove_dir_all(vault).unwrap();
}
