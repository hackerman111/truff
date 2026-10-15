use std::time::{Duration, Instant};

use truff_store::Store;
use truff_store::catalog::{CatalogEvent, CatalogJob, CatalogWorker};

fn wait(worker: &CatalogWorker, pred: impl Fn(&CatalogEvent) -> bool) {
    let start = Instant::now();
    loop {
        for event in worker.poll_events() {
            if let CatalogEvent::Error(error) = &event {
                panic!("{error}");
            }
            if pred(&event) {
                return;
            }
        }
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn dirty_backlinks_query_does_not_hide_disk_links_from_later_rename() {
    let vault = std::env::temp_dir().join(format!("truff-catalog-overlay-{}", std::process::id()));
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(vault.join("a.md"), "[[b]]\n").unwrap();
    std::fs::write(vault.join("b.md"), "# B\n").unwrap();
    let store = Store::new(vault.clone()).unwrap();
    let worker = CatalogWorker::start(store.paths().clone()).unwrap();
    wait(&worker, |event| matches!(event, CatalogEvent::Ready));
    worker
        .try_submit(CatalogJob::Backlinks {
            path: "b.md".into(),
            dirty: vec![("a.md".into(), "deleted link\n".into())],
        })
        .unwrap();
    wait(
        &worker,
        |event| matches!(event, CatalogEvent::Backlinks { occurrences, .. } if occurrences.is_empty()),
    );
    worker
        .try_submit(CatalogJob::Rename {
            from: "b.md".into(),
            to: "c.md".into(),
            dirty: vec![],
        })
        .unwrap();
    wait(&worker, |event| {
        matches!(event, CatalogEvent::Renamed { .. })
    });
    assert_eq!(
        std::fs::read_to_string(vault.join("a.md")).unwrap(),
        "[[c]]\n"
    );
    drop(worker);
    drop(store);
    std::fs::remove_dir_all(vault).unwrap();
}

#[test]
fn legacy_database_without_file_stem_and_user_version_3_is_migrated_and_worker_stays_alive() {
    let vault = std::env::temp_dir().join(format!("truff-legacy-schema-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&vault);
    std::fs::create_dir_all(vault.join(".truff")).unwrap();
    std::fs::write(vault.join("existing.md"), "# Existing Note\n").unwrap();

    // Create legacy table schema with user_version = 3 (as seen in older branches)
    let db_path = vault.join(".truff").join("index.db");
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "PRAGMA user_version = 3;
             CREATE TABLE files (
                 id INTEGER PRIMARY KEY,
                 relative_path TEXT NOT NULL UNIQUE,
                 title TEXT NOT NULL,
                 fingerprint TEXT NOT NULL,
                 index_generation INTEGER NOT NULL
             );
             CREATE TABLE headings (
                 file_id INTEGER NOT NULL,
                 ordinal INTEGER NOT NULL,
                 level INTEGER NOT NULL,
                 text TEXT NOT NULL,
                 anchor TEXT NOT NULL,
                 byte_start INTEGER NOT NULL,
                 byte_end INTEGER NOT NULL,
                 PRIMARY KEY (file_id, ordinal)
             );
             CREATE TABLE blocks (
                 file_id INTEGER NOT NULL,
                 block_anchor TEXT NOT NULL,
                 byte_start INTEGER NOT NULL,
                 byte_end INTEGER NOT NULL,
                 PRIMARY KEY (file_id, block_anchor)
             );
             CREATE TABLE links (
                 source_file_id INTEGER NOT NULL,
                 occurrence_ordinal INTEGER NOT NULL,
                 byte_start INTEGER NOT NULL,
                 byte_end INTEGER NOT NULL,
                 raw_target TEXT NOT NULL,
                 raw_heading TEXT,
                 raw_label TEXT,
                 is_wiki INTEGER NOT NULL,
                 resolved_file_id INTEGER,
                 resolved_anchor TEXT,
                 PRIMARY KEY (source_file_id, occurrence_ordinal)
             );
             CREATE VIRTUAL TABLE notes_fts USING fts5(
                 file_id UNINDEXED, title, body, tokenize = 'unicode61'
             );
             CREATE TABLE operation_log (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 op_type TEXT NOT NULL,
                 status TEXT NOT NULL,
                 payload TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             INSERT INTO files (id, relative_path, title, fingerprint, index_generation)
             VALUES (1, 'existing.md', 'existing.md', 'fp', 1);
             INSERT INTO notes_fts (rowid, file_id, title, body)
             VALUES (99, 1, 'existing.md', 'Existing Note');",
        )
        .unwrap();
    }

    let paths = truff_store::VaultPaths::new(vault.clone());
    let worker = CatalogWorker::start(paths).unwrap();
    wait(&worker, |e| matches!(e, CatalogEvent::Ready));
    assert!(
        worker.is_running(),
        "Worker must stay running after schema migration"
    );

    // Creating a note in a new subfolder (folder creation with initial note)
    let new_note = "new_folder/note.md";
    worker
        .try_submit(CatalogJob::Create {
            path: new_note.into(),
        })
        .unwrap();

    wait(&worker, |e| matches!(e, CatalogEvent::JobDone));
    assert!(vault.join(new_note).exists());
    assert!(
        worker.is_running(),
        "Worker must stay running after note creation"
    );

    drop(worker);
    let _ = std::fs::remove_dir_all(vault);
}
