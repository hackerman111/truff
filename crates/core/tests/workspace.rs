use std::path::PathBuf;
use truff_core::commands::{Action, AppError, Effect};
use truff_core::text::{BytePos, Transaction};
use truff_core::workspace::buffers::{BufferState, VaultEpoch};
use truff_core::workspace::{StoreEvent, Workspace};

#[test]
fn two_views_one_buffer() {
    let mut ws = Workspace::new();
    let path = PathBuf::from("notes/hello.md");
    let buf_id = ws.open_buffer_from_text(path, "Hello world").unwrap();

    let view1 = ws.create_view(buf_id).unwrap();
    let view2 = ws.create_view(buf_id).unwrap();

    // Set view1 cursor at byte 5 (after "Hello"), view2 at byte 11 (after "world")
    ws.dispatch(Action::MoveCursor {
        view_id: view1,
        anchor: BytePos::new(5),
        head: BytePos::new(5),
    })
    .unwrap();

    ws.dispatch(Action::MoveCursor {
        view_id: view2,
        anchor: BytePos::new(11),
        head: BytePos::new(11),
    })
    .unwrap();

    // view1 applies an edit: inserts ", beautiful" at offset 5
    let tx = Transaction::insert(5, ", beautiful");
    ws.dispatch(Action::ApplyEdit {
        view_id: view1,
        transaction: tx,
    })
    .unwrap();

    // Both views must see the updated text
    let v1 = ws.get_view(view1).unwrap();
    let v2 = ws.get_view(view2).unwrap();
    let buf = ws.buffers().get_buffer(buf_id).unwrap();
    assert_eq!(buf.rope().to_string(), "Hello, beautiful world");

    // view1 caret was at 5, inserted 11 bytes -> rebased or updated
    // view2 caret was at 11 -> rebased to 11 + 11 = 22
    assert_eq!(v2.head(), BytePos::new(22));
    assert_eq!(v2.anchor(), BytePos::new(22));
    assert_eq!(v1.head(), BytePos::new(16));
}

#[test]
fn stale_result_after_reopen() {
    let mut ws = Workspace::new();
    let path = PathBuf::from("notes/note.md");
    let buf_id_old = ws.open_buffer_from_text(path.clone(), "old text").unwrap();
    let old_key = ws.buffers().get_snapshot_key(buf_id_old).unwrap();

    // Close buffer explicitly
    ws.close_buffer_force(buf_id_old).unwrap();

    // Reopen buffer -> gets a new BufferId
    let buf_id_new = ws.open_buffer_from_text(path, "new text").unwrap();
    assert_ne!(buf_id_old, buf_id_new);

    // Incoming store event with old SnapshotKey must be rejected as stale
    let stale_event = StoreEvent::SaveAck {
        key: old_key,
        fingerprint: "abc".to_string(),
    };
    let res = ws.accept(stale_event);
    assert!(res.is_err());
    match res.unwrap_err() {
        AppError::StaleSnapshotKey { expected, actual } => {
            assert_eq!(actual, old_key);
            assert_ne!(expected.buffer_id, old_key.buffer_id);
        }
        err => panic!("expected StaleSnapshotKey, got: {err:?}"),
    }
}

#[test]
fn old_vault_reply() {
    let mut ws = Workspace::new();
    let path = PathBuf::from("notes/vault1.md");
    let buf_id = ws.open_buffer_from_text(path, "vault text").unwrap();
    let old_key = ws.buffers().get_snapshot_key(buf_id).unwrap();
    assert_eq!(old_key.epoch, VaultEpoch::new(0));

    // Advance vault epoch (switch vault)
    ws.switch_vault(VaultEpoch::new(1));

    // Event from old epoch 0
    let stale_event = StoreEvent::SaveAck {
        key: old_key,
        fingerprint: "xyz".to_string(),
    };
    let res = ws.accept(stale_event);
    assert!(res.is_err());
    match res.unwrap_err() {
        AppError::StaleEpoch { current, received } => {
            assert_eq!(current, VaultEpoch::new(1));
            assert_eq!(received, VaultEpoch::new(0));
        }
        err => panic!("expected StaleEpoch, got: {err:?}"),
    }
}

#[test]
fn close_view_keeps_dirty() {
    let mut ws = Workspace::new();
    let path = PathBuf::from("notes/dirty.md");
    let buf_id = ws.open_buffer_from_text(path, "initial").unwrap();
    let view_id = ws.create_view(buf_id).unwrap();

    // Apply edit to make buffer dirty
    ws.dispatch(Action::ApplyEdit {
        view_id,
        transaction: Transaction::insert(7, " edit"),
    })
    .unwrap();

    assert_eq!(ws.buffers().get_state(buf_id), Some(BufferState::Dirty));

    // Close the view
    let effects = ws.dispatch(Action::CloseView(view_id)).unwrap();
    assert!(effects.contains(&Effect::ViewClosed(view_id)));

    // View is gone
    assert!(ws.get_view(view_id).is_none());

    // Buffer is STILL in the registry and STILL dirty!
    let entry = ws.buffers().get(buf_id).expect("buffer must be preserved");
    assert_eq!(entry.state(), BufferState::Dirty);
    let buf = ws.buffers().get_buffer(buf_id).unwrap();
    assert_eq!(buf.rope().to_string(), "initial edit");
}

#[test]
fn late_save_ack_keeps_dirty() {
    let mut ws = Workspace::new();
    let path = PathBuf::from("notes/late_ack.md");
    let buf_id = ws.open_buffer_from_text(path, "v0").unwrap();
    let view_id = ws.create_view(buf_id).unwrap();

    // Edit 1: rev becomes 1
    ws.dispatch(Action::ApplyEdit {
        view_id,
        transaction: Transaction::insert(2, " v1"),
    })
    .unwrap();

    let key_rev1 = ws.buffers().get_snapshot_key(buf_id).unwrap();
    assert_eq!(key_rev1.revision, 1);
    assert_eq!(ws.buffers().get_state(buf_id), Some(BufferState::Dirty));

    // Dispatch SaveBuffer at rev 1
    let effects = ws.dispatch(Action::SaveBuffer(buf_id)).unwrap();
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::SubmitStoreJob(_)))
    );

    // Before save finishes, user makes another edit: rev becomes 2
    ws.dispatch(Action::ApplyEdit {
        view_id,
        transaction: Transaction::insert(5, " v2"),
    })
    .unwrap();

    let current_key = ws.buffers().get_snapshot_key(buf_id).unwrap();
    assert_eq!(current_key.revision, 2);

    // Late SaveAck for revision 1 arrives!
    let ack_event = StoreEvent::SaveAck {
        key: key_rev1,
        fingerprint: "hash_rev1".to_string(),
    };
    ws.accept(ack_event).unwrap();

    // Current revision (2) > acknowledged revision (1), so buffer must REMAIN DIRTY!
    assert_eq!(ws.buffers().get_state(buf_id), Some(BufferState::Dirty));
    let buf = ws.buffers().get_buffer(buf_id).unwrap();
    assert!(buf.is_dirty());
}
