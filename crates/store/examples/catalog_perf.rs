use std::hint::black_box;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use truff_store::Store;
use truff_store::catalog::{CatalogEvent, CatalogWorker};

fn open(paths: &truff_store::VaultPaths) -> Result<(), Box<dyn std::error::Error>> {
    let worker = CatalogWorker::start(paths.clone())?;
    let start = Instant::now();
    loop {
        for event in worker.poll_events() {
            match event {
                CatalogEvent::Ready => return Ok(()),
                CatalogEvent::Error(message) => return Err(message.into()),
                event => {
                    black_box(event);
                }
            }
        }
        if start.elapsed() > Duration::from_secs(30) {
            return Err("catalog timeout".into());
        }
        std::thread::sleep(Duration::from_micros(100));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "truff-catalog-perf-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    std::fs::create_dir(&root)?;
    let text = format!("# Heading\n{}", "[[note99#Heading]] ".repeat(20));
    for id in 0..100 {
        std::fs::write(root.join(format!("note{id}.md")), &text)?;
    }
    let store = Store::new(root.clone())?;
    open(store.paths())?;
    open(store.paths())?;
    let iterations = 5;
    let start = Instant::now();
    for _ in 0..iterations {
        open(store.paths())?;
    }
    let elapsed = start.elapsed().as_nanos();
    println!("scenario,iterations,total_ns,ns_per_op");
    println!(
        "catalog_reopen_100_unchanged,{iterations},{elapsed},{}",
        elapsed / iterations
    );
    drop(store);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
