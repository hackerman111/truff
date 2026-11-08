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
fn test_catalog_copy_file() {
    let vault = std::env::temp_dir().join(format!("truff-catalog-copy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&vault);
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(vault.join("source.md"), "# Source note content\n").unwrap();

    let store = Store::new(vault.clone()).unwrap();
    let worker = CatalogWorker::start(store.paths().clone()).unwrap();
    wait(&worker, |event| matches!(event, CatalogEvent::Ready));

    worker
        .try_submit(CatalogJob::Copy {
            from: "source.md".into(),
            to: "subfolder/copied.md".into(),
        })
        .unwrap();

    wait(
        &worker,
        |event| matches!(event, CatalogEvent::Copied { to, .. } if to == "subfolder/copied.md"),
    );

    assert!(vault.join("source.md").exists());
    assert!(vault.join("subfolder/copied.md").exists());
    let copied_content = std::fs::read_to_string(vault.join("subfolder/copied.md")).unwrap();
    assert_eq!(copied_content, "# Source note content\n");

    let _ = std::fs::remove_dir_all(&vault);
}
