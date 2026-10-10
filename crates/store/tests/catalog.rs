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
