use std::path::PathBuf;

use truff_core::commands::{Action, Effect};
use truff_core::text::Transaction;
use truff_core::workspace::{StoreJob, Workspace};

#[test]
fn queued_save_keeps_its_revision_when_the_buffer_is_edited_again() {
    let mut workspace = Workspace::new();
    let original = "αβ\n".repeat(20_000);
    let id = workspace
        .open_buffer_from_text(PathBuf::from("note.md"), &original)
        .unwrap();
    let view = workspace.create_view(id).unwrap();
    let effects = workspace.dispatch(Action::SaveBuffer(id)).unwrap();
    workspace
        .dispatch(Action::ApplyEdit {
            view_id: view,
            transaction: Transaction::replace(0..4, "new"),
        })
        .unwrap();
    let Effect::SubmitStoreJob(StoreJob::SaveFile { key, content, .. }) = &effects[0] else {
        panic!("expected save");
    };
    assert_eq!(content.to_string(), original);
    assert!(workspace.buffers().get_buffer(id).unwrap().revision() > key.revision);
    assert!(
        workspace
            .buffers()
            .get_buffer(id)
            .unwrap()
            .as_str()
            .starts_with("new")
    );
}
