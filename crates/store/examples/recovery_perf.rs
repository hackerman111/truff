use std::hint::black_box;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use truff_core::text::BufferId;
use truff_core::workspace::buffers::{SnapshotKey, VaultEpoch};
use truff_store::recovery::RecoveryManager;

const ITERATIONS: u64 = 100;

struct TempVault(std::path::PathBuf);

impl TempVault {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "truff-recovery-perf-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for TempVault {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let vault = TempVault::new()?;
    let manager = RecoveryManager::new(vault.0.clone(), 4 * 1024 * 1024);
    let content = "x".repeat(16 * 1024);
    let key = |revision| SnapshotKey::new(VaultEpoch(1), BufferId(1), revision);

    for revision in 1..=2 {
        black_box(manager.write_snapshot(key(revision), "note.md", &content, None)?);
    }

    let start = Instant::now();
    for revision in 3..=ITERATIONS + 2 {
        let path = manager.write_snapshot(key(revision), "note.md", &content, None)?;
        black_box(path);
    }
    let total_ns = start.elapsed().as_nanos();
    println!("scenario,iterations,total_ns,ns_per_op");
    println!(
        "recovery_write_16k,{ITERATIONS},{total_ns},{}",
        total_ns / u128::from(ITERATIONS)
    );
    Ok(())
}
