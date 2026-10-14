use std::path::PathBuf;
use truff_core::commands::{Action, AppError, Effect};
use truff_core::text::Transaction;
use truff_core::workspace::{BufferState, StoreEvent, StoreJob, Workspace};

#[test]
fn unload_preserves_identity_and_rejects_old_replies_after_reopen() {
    let mut ws = Workspace::new();
    let path = PathBuf::from("note.md");
    let id = ws.open_buffer_from_text(path.clone(), "draft").unwrap();
    let view = ws.create_view(id).unwrap();
    assert!(!ws.unload_buffer(id).unwrap());
    ws.dispatch(Action::ApplyEdit {
        view_id: view,
        transaction: Transaction::insert(5, "!"),
    })
    .unwrap();
    ws.close_view(view).unwrap();
    assert!(!ws.unload_buffer(id).unwrap());
    let old = ws.buffers().get_snapshot_key(id).unwrap();
    ws.accept(StoreEvent::SaveAck {
        key: old,
        fingerprint: "disk-fp".into(),
    })
    .unwrap();
    let generation = ws.buffers().watch_generation();
    assert!(ws.unload_buffer(id).unwrap());
    assert!(ws.buffers().watch_generation() > generation);
    assert!(ws.buffers().get_buffer(id).is_none());
    assert_eq!(ws.buffers().get_by_path(&path), Some(id));
    assert_eq!(ws.buffers().get(id).unwrap().fingerprint(), Some("disk-fp"));
    assert_eq!(ws.buffers().get_state(id), Some(BufferState::Unloaded));
    let reserved = ws.buffers().get_snapshot_key(id).unwrap();
    assert!(reserved.revision > old.revision);
    for event in [
        StoreEvent::FileLoaded {
            key: old,
            content: "stale".into(),
            fingerprint: "old".into(),
        },
        StoreEvent::SaveAck {
            key: old,
            fingerprint: "old".into(),
        },
        StoreEvent::Conflict {
            key: old,
            disk_content: None,
            disk_fingerprint: "old".into(),
        },
    ] {
        assert!(matches!(
            ws.accept(event),
            Err(AppError::StaleSnapshotKey { .. })
        ));
    }
    let effects = ws.dispatch(Action::OpenPath(path)).unwrap();
    assert!(effects.iter().any(|effect| matches!(effect,
        Effect::SubmitStoreJob(StoreJob::LoadFile { key, .. }) if *key == reserved)));
    ws.accept(StoreEvent::FileLoaded {
        key: reserved,
        content: "new disk text".into(),
        fingerprint: "new-fp".into(),
    })
    .unwrap();
    let buffer = ws.buffers().get_buffer(id).unwrap();
    assert_eq!(buffer.as_str(), "new disk text");
    assert_eq!(buffer.revision(), reserved.revision);
    assert!(!buffer.is_dirty());
    assert!(matches!(
        ws.accept(StoreEvent::SaveAck {
            key: old,
            fingerprint: "old".into()
        }),
        Err(AppError::StaleSnapshotKey { .. })
    ));
    assert_eq!(ws.buffers().get_state(id), Some(BufferState::Clean));
}

#[test]
fn queued_watcher_does_not_reload_an_unloaded_buffer() {
    let mut ws = Workspace::new();
    let path = PathBuf::from("note.md");
    let id = ws.open_buffer_from_text(path.clone(), "text").unwrap();
    assert!(ws.unload_buffer(id).unwrap());
    let key = ws.buffers().get_snapshot_key(id).unwrap();
    assert!(
        ws.accept(StoreEvent::ExternalChanged {
            key,
            path,
            new_fingerprint: "changed".into()
        })
        .unwrap()
        .is_empty()
    );
    assert!(ws.buffers().get_buffer(id).is_none());
}
