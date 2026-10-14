use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use truff_store::Store;
use truff_store::catalog::{CatalogEvent, CatalogJob, CatalogWorker};
use truff_store::index::{FileId, IndexDatabase};

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

#[test]
fn restart_skips_unchanged_notes_preserves_ids_and_resolves_forward_anchors() {
    let root = std::env::temp_dir().join(format!(
        "truff-catalog-incremental-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("m.md"), "[[z#Heading]]").unwrap();
    std::fs::write(root.join("z.md"), "# Heading\n").unwrap();
    let store = Store::new(root.clone()).unwrap();
    let worker = CatalogWorker::start(store.paths().clone()).unwrap();
    wait(&worker, |e| matches!(e, CatalogEvent::Ready));
    drop(worker);
    let db_path = store.paths().truff_dir().join("index.db");
    let db = IndexDatabase::open(&db_path).unwrap();
    let id: i64 = db
        .connection()
        .query_row("SELECT id FROM files WHERE relative_path='z.md'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        db.get_backlinks(FileId(id), Some("heading"), 0, 100)
            .unwrap()
            .total_matches,
        1
    );
    db.connection()
        .execute(
            "UPDATE files SET index_generation=123 WHERE relative_path='z.md'",
            [],
        )
        .unwrap();
    drop(db);
    std::fs::write(root.join("a.md"), "new file").unwrap();
    let worker = CatalogWorker::start(store.paths().clone()).unwrap();
    wait(&worker, |e| matches!(e, CatalogEvent::Ready));
    let db = IndexDatabase::open(&db_path).unwrap();
    let state: (i64, i64) = db
        .connection()
        .query_row(
            "SELECT id,index_generation FROM files WHERE relative_path='z.md'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, (id, 123));
    // A create job must not discover unrelated files or rewrite the existing index.
    std::fs::write(root.join("unrelated.md"), "outside this operation").unwrap();
    worker
        .try_submit(CatalogJob::Create {
            path: "created.md".into(),
        })
        .unwrap();
    wait(&worker, |e| matches!(e, CatalogEvent::JobDone));
    let missing: i64 = db
        .connection()
        .query_row(
            "SELECT count(*) FROM files WHERE relative_path='unrelated.md'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(missing, 0);
    worker
        .try_submit(CatalogJob::Trash {
            path: "created.md".into(),
            dirty: vec![],
        })
        .unwrap();
    wait(&worker, |e| matches!(e, CatalogEvent::JobDone));
    assert_eq!(
        db.connection()
            .query_row(
                "SELECT count(*) FROM files WHERE relative_path='created.md'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(db.file_path(FileId(id)).unwrap().as_deref(), Some("z.md"));
    drop(db);
    drop(worker);
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}
