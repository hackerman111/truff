use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use truff_store::Store;
use truff_store::catalog::{CatalogEvent, CatalogJob, CatalogWorker};
use truff_store::index::IndexDatabase;

fn wait(worker: &CatalogWorker, done: impl Fn(&CatalogEvent) -> bool) {
    let start = Instant::now();
    loop {
        for event in worker.poll_events() {
            if let CatalogEvent::Error(error) = &event {
                panic!("{error}");
            }
            if done(&event) {
                return;
            }
        }
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn indexed_paths(db: &IndexDatabase, prefix: &str) -> Vec<String> {
    let mut statement = db
        .connection()
        .prepare(
            "SELECT relative_path FROM files WHERE relative_path LIKE ?1 ORDER BY relative_path",
        )
        .unwrap();
    statement
        .query_map([format!("{prefix}/%")], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn file_id(db: &IndexDatabase, path: &str) -> i64 {
    db.connection()
        .query_row(
            "SELECT id FROM files WHERE relative_path = ?1",
            [path],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn rename_and_trash_subtree_update_files_and_index_without_affecting_stable_id() {
    let root = std::env::temp_dir().join(format!(
        "truff-catalog-subtree-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("old/sub")).unwrap();
    std::fs::write(root.join("old/a.md"), "# A\n").unwrap();
    std::fs::write(root.join("old/sub/b.md"), "# B\n").unwrap();
    std::fs::write(root.join("stable.md"), "# Stable\n").unwrap();

    let store = Store::new(root.clone()).unwrap();
    let worker = CatalogWorker::start(store.paths().clone()).unwrap();
    wait(&worker, |event| matches!(event, CatalogEvent::Ready));
    let db = IndexDatabase::open(store.paths().truff_dir().join("index.db")).unwrap();
    let stable_id = file_id(&db, "stable.md");

    worker
        .try_submit(CatalogJob::Rename {
            from: "old".into(),
            to: "new".into(),
            dirty: vec![],
        })
        .unwrap();
    wait(&worker, |event| matches!(event, CatalogEvent::JobDone));
    assert_eq!(indexed_paths(&db, "new"), ["new/a.md", "new/sub/b.md"]);
    assert!(indexed_paths(&db, "old").is_empty());
    assert_eq!(file_id(&db, "stable.md"), stable_id);
    assert!(root.join("new/a.md").is_file());
    assert!(root.join("new/sub/b.md").is_file());
    assert!(!root.join("old").exists());

    worker
        .try_submit(CatalogJob::Trash {
            path: "new".into(),
            dirty: vec![],
        })
        .unwrap();
    wait(&worker, |event| matches!(event, CatalogEvent::JobDone));
    assert!(indexed_paths(&db, "new").is_empty());
    assert_eq!(file_id(&db, "stable.md"), stable_id);
    assert!(!root.join("new").exists());

    drop(db);
    drop(worker);
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}
