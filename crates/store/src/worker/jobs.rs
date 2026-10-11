use super::watch::WatchedFile;
use crate::files;
use crate::recovery::RecoveryManager;
use truff_core::workspace::{StoreEvent, StoreJob};

pub(super) fn process_job(
    paths: &crate::VaultPaths,
    recovery: &RecoveryManager,
    job: StoreJob,
) -> (StoreEvent, Option<WatchedFile>) {
    use truff_core::workspace::SnapshotKey;
    let key: SnapshotKey = match &job {
        StoreJob::SaveFile { key, .. }
        | StoreJob::LoadFile { key, .. }
        | StoreJob::ReloadFile { key, .. }
        | StoreJob::OverwriteFile { key, .. }
        | StoreJob::SaveAsFile { key, .. }
        | StoreJob::WriteRecoverySnapshot { key, .. }
        | StoreJob::DeleteRecoverySnapshot { key } => *key,
    };
    let failure = |error: String| (StoreEvent::SaveFailed { key, error }, None);
    let save_as = matches!(&job, StoreJob::SaveAsFile { .. });
    let explicit_reload = matches!(&job, StoreJob::ReloadFile { .. });
    let result = (|| -> Result<(StoreEvent, Option<WatchedFile>), Box<dyn std::error::Error>> {
        match job {
            StoreJob::SaveFile {
                path,
                content,
                base_fingerprint,
                ..
            } => {
                let path = paths.resolve_path(&path)?;
                if let Some(base) = &base_fingerprint {
                    let current = files::compute_file_fingerprint(&path).unwrap_or_default();
                    if &current != base {
                        let disk_content =
                            files::read_file_with_limit(&path, files::DEFAULT_MAX_FILE_BYTES).ok();
                        return Ok((
                            StoreEvent::Conflict {
                                key,
                                disk_content,
                                disk_fingerprint: current,
                            },
                            None,
                        ));
                    }
                }
                let fingerprint = if base_fingerprint.is_some() {
                    files::atomic_write_file(&path, &content)?
                } else {
                    files::atomic_create_file(&path, &content)?
                };
                recovery.delete_snapshots(key.buffer_id)?;
                Ok((
                    StoreEvent::SaveAck {
                        key,
                        fingerprint: fingerprint.clone(),
                    },
                    Some(WatchedFile {
                        key,
                        path,
                        fingerprint,
                    }),
                ))
            }
            StoreJob::OverwriteFile { path, content, .. }
            | StoreJob::SaveAsFile { path, content, .. } => {
                // Distinguish the explicit replacement command from create-only save-as.
                let path = paths.resolve_path(&path)?;
                let fingerprint = if save_as {
                    files::atomic_create_file(&path, &content)?
                } else {
                    files::atomic_write_file(&path, &content)?
                };
                recovery.delete_snapshots(key.buffer_id)?;
                let event = if save_as {
                    StoreEvent::SavedAs {
                        key,
                        path: path.clone(),
                        fingerprint: fingerprint.clone(),
                    }
                } else {
                    StoreEvent::SaveAck {
                        key,
                        fingerprint: fingerprint.clone(),
                    }
                };
                Ok((
                    event,
                    Some(WatchedFile {
                        key,
                        path,
                        fingerprint,
                    }),
                ))
            }
            StoreJob::LoadFile { path, .. } | StoreJob::ReloadFile { path, .. } => {
                let explicit = explicit_reload;
                let path = paths.resolve_path(&path)?;
                let content = files::read_file_with_limit(&path, files::DEFAULT_MAX_FILE_BYTES)?;
                let fingerprint = files::compute_fingerprint(content.as_bytes());
                let event = if explicit {
                    StoreEvent::FileReloaded {
                        key,
                        content,
                        fingerprint: fingerprint.clone(),
                    }
                } else {
                    StoreEvent::FileLoaded {
                        key,
                        content,
                        fingerprint: fingerprint.clone(),
                    }
                };
                Ok((
                    event,
                    Some(WatchedFile {
                        key,
                        path,
                        fingerprint,
                    }),
                ))
            }
            StoreJob::WriteRecoverySnapshot {
                path,
                content,
                base_fingerprint,
                ..
            } => {
                let resolved = paths.resolve_path(&path)?;
                let relative = paths.relative_path(&resolved)?;
                recovery.write_snapshot(
                    key,
                    &relative.to_string_lossy(),
                    &content,
                    base_fingerprint.as_deref(),
                )?;
                Ok((StoreEvent::RecoveryAck { key }, None))
            }
            StoreJob::DeleteRecoverySnapshot { .. } => {
                recovery.delete_snapshots(key.buffer_id)?;
                Ok((StoreEvent::RecoveryAck { key }, None))
            }
        }
    })();
    result.unwrap_or_else(|error| failure(error.to_string()))
}
