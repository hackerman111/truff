use std::time::Instant;

use truff_core::vim::Key;

use crate::overlays::FlashOverlay;
use crate::perf_scenarios::fixtures::generate_bench_note;
use crate::perf_scenarios::metrics::{PerfSample, percentile};
use crate::shell::Shell;

pub fn run_edit_latency(runs: usize) -> Vec<PerfSample> {
    let mut samples = Vec::new();
    println!("--- Scenario: Edit Latency (runs={runs}, 500 events/run) ---");

    let note_64k = generate_bench_note(64 * 1024);

    for run_id in 1..=runs {
        let mut shell = Shell::new();
        shell.set_editor_text(&note_64k);

        let mut latencies_us: Vec<f64> = Vec::with_capacity(500);

        // Sequence of 500 representative editing and navigation actions:
        // - 200 text inserts via Vim
        // - 100 normal mode motions
        // - 100 deletions
        // - 100 undo operations
        for i in 0..500 {
            let t0 = Instant::now();

            if i < 200 {
                // Insert mode typing
                shell.feed_key(Key::Char('i'));
                shell.feed_key(Key::Char('a'));
                shell.feed_key(Key::Esc);
            } else if i < 300 {
                // Motions
                let key = match i % 4 {
                    0 => Key::Char('h'),
                    1 => Key::Char('j'),
                    2 => Key::Char('k'),
                    _ => Key::Char('l'),
                };
                shell.feed_key(key);
            } else if i < 400 {
                // Deletions
                shell.feed_key(Key::Char('x'));
            } else {
                // Undo
                shell.feed_key(Key::Char('u'));
            }

            let lat_us = t0.elapsed().as_nanos() as f64 / 1000.0;
            latencies_us.push(lat_us);

            samples.push(PerfSample {
                scenario: "edit-latency".to_string(),
                metric: "edit_latency_us".to_string(),
                run_id,
                sample_idx: i,
                value: lat_us,
                unit: "us".to_string(),
            });
        }

        latencies_us.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p50_ms = percentile(&latencies_us, 0.50) / 1000.0;
        let p95_ms = percentile(&latencies_us, 0.95) / 1000.0;
        let p99_ms = percentile(&latencies_us, 0.99) / 1000.0;

        samples.push(PerfSample {
            scenario: "edit-latency".to_string(),
            metric: "edit_p50_ms".to_string(),
            run_id,
            sample_idx: 0,
            value: p50_ms,
            unit: "ms".to_string(),
        });
        samples.push(PerfSample {
            scenario: "edit-latency".to_string(),
            metric: "edit_p95_ms".to_string(),
            run_id,
            sample_idx: 0,
            value: p95_ms,
            unit: "ms".to_string(),
        });
        samples.push(PerfSample {
            scenario: "edit-latency".to_string(),
            metric: "edit_p99_ms".to_string(),
            run_id,
            sample_idx: 0,
            value: p99_ms,
            unit: "ms".to_string(),
        });

        // Flash in visible area (§10.2: p95 <= 30 ms)
        let mut flash = FlashOverlay::new();
        let t_flash = Instant::now();
        flash.activate(&note_64k, 0);
        let flash_ms = t_flash.elapsed().as_secs_f64() * 1000.0;
        samples.push(PerfSample {
            scenario: "edit-latency".to_string(),
            metric: "flash_labels_ms".to_string(),
            run_id,
            sample_idx: 0,
            value: flash_ms,
            unit: "ms".to_string(),
        });

        println!(
            "  Run {run_id}/{runs}: Edit latency p50={p50_ms:.3} ms | p95={p95_ms:.3} ms | p99={p99_ms:.3} ms | Flash labels={flash_ms:.3} ms",
        );
    }

    samples
}
