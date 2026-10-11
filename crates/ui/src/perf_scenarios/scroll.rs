use std::time::Instant;

use crate::editor::layout::{HeightIndex, VirtualizedLayout};
use crate::perf_scenarios::fixtures::generate_bench_note;
use crate::perf_scenarios::metrics::{PerfSample, percentile};

pub fn run_scroll_layout(runs: usize) -> Vec<PerfSample> {
    let mut samples = Vec::new();
    println!("--- Scenario: Scroll Layout (runs={runs}, 200 scroll frames/run) ---");

    // Large document with ~2,000 paragraphs
    let large_note = generate_bench_note(256 * 1024);

    for run_id in 1..=runs {
        let mut height_index = HeightIndex::new(24.0);
        height_index.init_from_text(&large_note);
        let mut virtual_layout = VirtualizedLayout::new(16.0);

        // Cold initial layout
        let t_cold = Instant::now();
        let _ = virtual_layout.layout_visible_window(
            &large_note,
            &mut height_index,
            0.0,
            600.0,
            Some(800.0),
            None,
        );
        let cold_ms = t_cold.elapsed().as_secs_f64() * 1000.0;

        samples.push(PerfSample {
            scenario: "scroll-layout".to_string(),
            metric: "cold_layout_ms".to_string(),
            run_id,
            sample_idx: 0,
            value: cold_ms,
            unit: "ms".to_string(),
        });

        let mut frame_latencies_ms: Vec<f64> = Vec::with_capacity(200);
        let mut cache_hits = 0;
        let mut total_requests = 0;

        // Warm scroll downwards and upwards
        for step in 0..200 {
            let viewport_top = if step < 100 {
                step as f32 * 50.0
            } else {
                (200 - step) as f32 * 50.0
            };

            let before_rebuilds = virtual_layout.rebuild_count();
            let t_frame = Instant::now();
            let visible = virtual_layout.layout_visible_window(
                &large_note,
                &mut height_index,
                viewport_top,
                600.0,
                Some(800.0),
                None,
            );
            let frame_dur_ms = t_frame.elapsed().as_secs_f64() * 1000.0;
            let after_rebuilds = virtual_layout.rebuild_count();

            let newly_built = after_rebuilds - before_rebuilds;
            let reused = visible.len().saturating_sub(newly_built);
            cache_hits += reused;
            total_requests += visible.len();

            frame_latencies_ms.push(frame_dur_ms);
            samples.push(PerfSample {
                scenario: "scroll-layout".to_string(),
                metric: "frame_latency_ms".to_string(),
                run_id,
                sample_idx: step,
                value: frame_dur_ms,
                unit: "ms".to_string(),
            });
        }

        frame_latencies_ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p50_frame_ms = percentile(&frame_latencies_ms, 0.50);
        let p95_frame_ms = percentile(&frame_latencies_ms, 0.95);
        let hit_rate = if total_requests > 0 {
            (cache_hits as f64 / total_requests as f64) * 100.0
        } else {
            100.0
        };

        samples.push(PerfSample {
            scenario: "scroll-layout".to_string(),
            metric: "scroll_frame_p50_ms".to_string(),
            run_id,
            sample_idx: 0,
            value: p50_frame_ms,
            unit: "ms".to_string(),
        });
        samples.push(PerfSample {
            scenario: "scroll-layout".to_string(),
            metric: "scroll_frame_p95_ms".to_string(),
            run_id,
            sample_idx: 0,
            value: p95_frame_ms,
            unit: "ms".to_string(),
        });
        samples.push(PerfSample {
            scenario: "scroll-layout".to_string(),
            metric: "cache_hit_rate_percent".to_string(),
            run_id,
            sample_idx: 0,
            value: hit_rate,
            unit: "%".to_string(),
        });

        // Measure single paragraph layout ready time (§10.2: Visible layout ready p95 <= 50 ms)
        let t_para = Instant::now();
        if height_index.block_count() > 10 {
            height_index.set_block_revision(10, height_index.blocks()[10].revision + 1);
        }
        let _ = virtual_layout.layout_visible_window(
            &large_note,
            &mut height_index,
            0.0,
            600.0,
            Some(800.0),
            None,
        );
        let para_ready_ms = t_para.elapsed().as_secs_f64() * 1000.0;
        samples.push(PerfSample {
            scenario: "scroll-layout".to_string(),
            metric: "paragraph_layout_ready_ms".to_string(),
            run_id,
            sample_idx: 0,
            value: para_ready_ms,
            unit: "ms".to_string(),
        });

        println!(
            "  Run {run_id}/{runs}: Cold layout={cold_ms:.2} ms | Paragraph layout ready={para_ready_ms:.3} ms | Warm frame p50={p50_frame_ms:.3} ms | p95={p95_frame_ms:.3} ms | Cache Hit={hit_rate:.1}%",
        );
    }

    samples
}
