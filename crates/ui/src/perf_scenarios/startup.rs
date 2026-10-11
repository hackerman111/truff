use std::fs;
use std::path::Path;
use std::time::Instant;

use truff_core::text::Buffer;
use truff_store::VaultPaths;
use truff_store::index::IndexDatabase;

use crate::driver::UiDriver;
use crate::editor::EditorLayout;
use crate::perf_scenarios::fixtures::{generate_bench_note, temp_vault_dir};
use crate::perf_scenarios::metrics::{MemoryStats, PerfSample};

pub fn run_startup(vault: Option<&Path>, runs: usize) -> Vec<PerfSample> {
    let mut samples = Vec::new();
    println!("--- Scenario: Startup (runs={runs}) ---");

    let fallback_vault = temp_vault_dir("startup_fallback");
    let vault_path = vault.unwrap_or(&fallback_vault);
    let _ = fs::create_dir_all(vault_path);

    for run_id in 1..=runs {
        let t0 = Instant::now();

        // 1. Initialize vault paths and index database
        let _vault_paths = VaultPaths::new(vault_path.to_path_buf());
        let _db = IndexDatabase::open_in_memory().ok();

        // 2. Initialize UiDriver and Shell
        let mut driver = UiDriver::new();

        // 3. Populate default active note
        let note_text = generate_bench_note(64 * 1024);
        driver.shell_mut().set_editor_text(&note_text);

        // 4. Parley layout probe
        let mut layout = EditorLayout::new(16.0);
        layout.compute(&note_text, Some(800.0));
        let _hit = layout.hit_test(&note_text, 100.0, 50.0);

        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let mem = MemoryStats::current();

        samples.push(PerfSample {
            scenario: "startup".to_string(),
            metric: "wall_time_ms".to_string(),

            run_id,
            sample_idx: 0,
            value: elapsed_ms,
            unit: "ms".to_string(),
        });
        samples.push(PerfSample {
            scenario: "startup".to_string(),
            metric: "rss_peak_mib".to_string(),
            run_id,
            sample_idx: 0,
            value: mem.peak_rss_mib(),
            unit: "mib".to_string(),
        });
        samples.push(PerfSample {
            scenario: "startup".to_string(),
            metric: "rss_steady_mib".to_string(),
            run_id,
            sample_idx: 0,
            value: mem.rss_mib(),
            unit: "mib".to_string(),
        });
        samples.push(PerfSample {
            scenario: "startup".to_string(),
            metric: "pss_mib".to_string(),
            run_id,
            sample_idx: 0,
            value: mem.pss_mib(),
            unit: "mib".to_string(),
        });

        // 5. Note open <= 1 MiB latency (§10.2: p95 <= 100 ms)
        let note_1mib = generate_bench_note(1024 * 1024);
        let t_open = Instant::now();
        let buf_1mib = Buffer::from_str(&note_1mib);
        let _rope_len = buf_1mib.rope().byte_len();
        let open_1mib_ms = t_open.elapsed().as_secs_f64() * 1000.0;
        samples.push(PerfSample {
            scenario: "startup".to_string(),
            metric: "note_open_1mib_ms".to_string(),
            run_id,
            sample_idx: 0,
            value: open_1mib_ms,
            unit: "ms".to_string(),
        });

        println!(
            "  Run {run_id}/{runs}: startup={elapsed_ms:.2} ms | 1 MiB note open={open_1mib_ms:.2} ms | Peak RSS={:.2} MiB | Steady RSS={:.2} MiB",
            mem.peak_rss_mib(),
            mem.rss_mib()
        );
    }

    samples
}
