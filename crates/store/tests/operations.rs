use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use truff_core::Buffer;
use truff_core::text::BufferId;
use truff_store::VaultPaths;
use truff_store::index::{FileId, IndexDatabase};
use truff_store::operations::{FileOpError, FileOpPlan, LinkSelection, recover_pending_operations};

fn temp_test_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("truff_test_{name}_{}_{id}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn link_selection_one_transaction() {
    let mut buffer = Buffer::from_text(BufferId::new(1), "Hello world in truff").unwrap();

    // Select "world" (byte range 6..11) and convert it to a wiki-link
    let tx = LinkSelection::wikilink(6..11, "world_note", Some("my_world"));
    buffer.apply(tx).unwrap();

    assert_eq!(
        buffer.rope().to_string(),
        "Hello [[world_note|my_world]] in truff"
    );

    // Single undo step restores original text completely
    buffer.undo().unwrap();
    assert_eq!(buffer.rope().to_string(), "Hello world in truff");
}

#[test]
fn rename_respects_occurrences() {
    let temp_path = temp_test_dir("ops_test");
    let vault = VaultPaths::new(temp_path.clone());
    let mut db = IndexDatabase::open_in_memory().unwrap();

    let note_a_path = temp_path.join("note_a.md");
    let note_b_path = temp_path.join("note_b.md");

    fs::write(&note_a_path, "# Note A\nOriginal A content").unwrap();

    let note_b_content = "Link to [[note_a]], alias [[note_a|Alias]], heading [[note_a#Intro]], code `[[note_a]]`.\n```\n[[note_a]]\n```\n";
    fs::write(&note_b_path, note_b_content).unwrap();

    // Index both files
    let facts_a =
        truff_core::markdown::facts::MarkdownFacts::extract("# Note A\nOriginal A content")
            .unwrap();
    let facts_b = truff_core::markdown::facts::MarkdownFacts::extract(note_b_content).unwrap();
    db.index_file(
        FileId(1),
        "note_a.md",
        "Note A",
        "fp_a",
        1,
        &facts_a,
        "# Note A\nOriginal A content",
    )
    .unwrap();
    db.index_file(
        FileId(2),
        "note_b.md",
        "Note B",
        "fp_b",
        1,
        &facts_b,
        note_b_content,
    )
    .unwrap();

    // Create and execute rename plan
    let mut plan = FileOpPlan::rename(&vault, "note_a.md", "note_a_new.md").unwrap();

    // No dirty buffers
    let dirty_buffers: Vec<PathBuf> = Vec::new();
    plan.validate(&vault, &dirty_buffers, &db).unwrap();

    // Staging and apply
    plan.stage(&vault, db.connection_mut()).unwrap();
    plan.apply(&vault, db.connection_mut()).unwrap();

    // note_a.md should no longer exist, note_a_new.md should exist
    assert!(!note_a_path.exists());
    let new_a_path = temp_path.join("note_a_new.md");
    assert!(new_a_path.exists());

    // note_b.md should have updated occurrences, but code spans must remain untouched
    let updated_b = fs::read_to_string(&note_b_path).unwrap();
    assert!(updated_b.contains("[[note_a_new]]"));
    assert!(updated_b.contains("[[note_a_new|Alias]]"));
    assert!(updated_b.contains("[[note_a_new#Intro]]"));
    assert!(updated_b.contains("`[[note_a]]`"));
    assert!(updated_b.contains("```\n[[note_a]]\n```"));
}

#[test]
fn dirty_affected_file_blocked() {
    let temp_path = temp_test_dir("ops_test");
    let vault = VaultPaths::new(temp_path.clone());
    let mut db = IndexDatabase::open_in_memory().unwrap();

    let note_a_path = temp_path.join("note_a.md");
    let note_b_path = temp_path.join("note_b.md");

    fs::write(&note_a_path, "# Note A").unwrap();
    fs::write(&note_b_path, "Link to [[note_a]]").unwrap();

    let facts_a = truff_core::markdown::facts::MarkdownFacts::extract("# Note A").unwrap();
    let facts_b =
        truff_core::markdown::facts::MarkdownFacts::extract("Link to [[note_a]]").unwrap();
    db.index_file(
        FileId(1),
        "note_a.md",
        "Note A",
        "fp_a",
        1,
        &facts_a,
        "# Note A",
    )
    .unwrap();
    db.index_file(
        FileId(2),
        "note_b.md",
        "Note B",
        "fp_b",
        1,
        &facts_b,
        "Link to [[note_a]]",
    )
    .unwrap();

    let mut plan = FileOpPlan::rename(&vault, "note_a.md", "note_a_new.md").unwrap();

    // note_b.md is dirty in editor!
    let dirty_buffers = vec![PathBuf::from("note_b.md")];
    let err = plan.validate(&vault, &dirty_buffers, &db).unwrap_err();

    match err {
        FileOpError::DirtyBufferBlocked(path) => {
            assert_eq!(path, PathBuf::from("note_b.md"));
        }
        other => panic!("expected DirtyBufferBlocked, got {other:?}"),
    }
}

#[test]
fn operation_recovery() {
    let temp_path = temp_test_dir("ops_test");
    let vault = VaultPaths::new(temp_path.clone());
    let mut db = IndexDatabase::open_in_memory().unwrap();

    let file_path = temp_path.join("doc.md");
    fs::write(&file_path, "Hello trash").unwrap();

    let facts = truff_core::markdown::facts::MarkdownFacts::extract("Hello trash").unwrap();
    db.index_file(FileId(1), "doc.md", "Doc", "fp1", 1, &facts, "Hello trash")
        .unwrap();

    // Stage a Trash operation
    let mut plan = FileOpPlan::trash(&vault, "doc.md").unwrap();
    let dirty: Vec<PathBuf> = Vec::new();
    plan.validate(&vault, &dirty, &db).unwrap();
    let _op_id = plan.stage(&vault, db.connection_mut()).unwrap();

    // Fault injection: simulate crash before plan.apply() was called!
    // The operation is in status 'staged' in operation_log.
    assert!(file_path.exists());

    // Run recovery on restart
    let recovered_count = recover_pending_operations(&vault, db.connection_mut()).unwrap();
    assert_eq!(recovered_count, 1);

    // After recovery, the staged trash operation was either completed or rolled back consistently
    // In trash recovery: if the file still existed at original path, trash was completed or restored
    let trash_target = vault.trash_dir().join("doc.md");
    assert!(trash_target.exists() || file_path.exists());
}
