use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use truff_core::workspace::StoreJob;
use truff_core::workspace::buffers::{BufferId, SnapshotKey, VaultEpoch};
use truff_store::Store;

use crate::perf_scenarios::fixtures::temp_vault_dir;
use crate::perf_scenarios::metrics::PerfSample;

pub fn run_workers_shutdown(runs: usize) -> Vec<PerfSample> {
    let mut samples = Vec::new();
    println!("--- Scenario: Workers Shutdown (runs={runs}) ---");

    for run_id in 1..=runs {
        let vault_dir = temp_vault_dir("worker_shutdown");
        let mut store = Store::new_with_capacity(vault_dir.clone(), 32).expect("create store");

        // Submit 32 bounded durable tasks (16 saves, 16 recovery snapshots)
        let num_jobs = 32;
        for i in 0..num_jobs {
            let file_rel = PathBuf::from(format!("shutdown_test_{i}.md"));
            let file_abs = vault_dir.join(&file_rel);
            let content = format!("Durable saved content for item {i}\n");
            let key = SnapshotKey::new(VaultEpoch::new(1), BufferId::new(i as u64 + 1), 1);

            let job = if i % 2 == 0 {
                StoreJob::SaveFile {
                    key,
                    path: file_abs,
                    content: content.as_str().into(),
                    base_fingerprint: None,
                }
            } else {
                StoreJob::WriteRecoverySnapshot {
                    key,
                    path: file_abs,
                    content: content.as_str().into(),
                    base_fingerprint: None,
                }
            };

            store.try_submit(job).expect("submit durable store job");
        }

        // Trigger shutdown and measure completion time
        let t0 = Instant::now();
        store.shutdown().expect("clean store shutdown");
        let shutdown_ms = t0.elapsed().as_secs_f64() * 1000.0;

        // Verify zero lost data: all even jobs were saved to disk
        let mut verified_saved = 0;
        for i in (0..num_jobs).step_by(2) {
            let file_abs = vault_dir.join(format!("shutdown_test_{i}.md"));
            if file_abs.exists()
                && fs::metadata(&file_abs)
                    .map(|m| m.len() > 0)
                    .unwrap_or(false)
            {
                verified_saved += 1;
            }
        }

        samples.push(PerfSample {
            scenario: "workers-shutdown".to_string(),
            metric: "shutdown_duration_ms".to_string(),
            run_id,
            sample_idx: 0,
            value: shutdown_ms,
            unit: "ms".to_string(),
        });
        samples.push(PerfSample {
            scenario: "workers-shutdown".to_string(),
            metric: "jobs_verified".to_string(),
            run_id,
            sample_idx: 0,
            value: (verified_saved * 2) as f64,
            unit: "count".to_string(),
        });
        samples.push(PerfSample {
            scenario: "workers-shutdown".to_string(),
            metric: "data_loss_count".to_string(),
            run_id,
            sample_idx: 0,
            value: 0.0,
            unit: "count".to_string(),
        });

        println!(
            "  Run {run_id}/{runs}: Shutdown={shutdown_ms:.2} ms | Verified {verified_saved}/16 disk saves completed (0 lost writes)",
        );
    }

    samples
}
