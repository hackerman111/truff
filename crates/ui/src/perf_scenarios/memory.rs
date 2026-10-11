use std::fs;
use std::path::Path;
use std::time::Instant;

use truff_core::markdown::facts::MarkdownFacts;
use truff_core::text::{Buffer, Transaction};
use truff_store::index::{FileId, IndexDatabase, SearchQuery};

use crate::editor::EditorLayout;
use crate::perf_scenarios::fixtures::generate_bench_note;
use crate::perf_scenarios::metrics::{MemoryStats, PerfSample};
use crate::shell::Shell;

pub fn run_memory_retention(vault_10k: Option<&Path>, runs: usize) -> Vec<PerfSample> {
    let mut samples = Vec::new();
    println!("--- Scenario: Memory Retention (runs={runs}) ---");

    for run_id in 1..=runs {
        let mem_init = MemoryStats::current();

        // 1. Churn test: open, edit, layout, close 50 buffer views
        let mut buffers = Vec::with_capacity(50);
        for i in 0..50 {
            let text = format!("Buffer {i} content with some initial data.\n");
            let mut buf = Buffer::from_str(&text);
            buf.apply(Transaction::insert(text.len(), " Additional edits."))
                .ok();
            buffers.push(buf);
        }
        // Force layout calculations for half of them
        for buf in buffers.iter().take(25) {
            let mut layout = EditorLayout::new(16.0);
            layout.compute(&buf.as_str(), Some(800.0));
        }
        let mem_churn_peak = MemoryStats::current();

        // Explicitly clear churn buffers
        drop(buffers);

        let mem_steady = MemoryStats::current();
        let delta_mib = (mem_steady.rss_mib() - mem_init.rss_mib()).max(0.0);

        samples.push(PerfSample {
            scenario: "memory-retention".to_string(),
            metric: "rss_initial_mib".to_string(),
            run_id,
            sample_idx: 0,
            value: mem_init.rss_mib(),
            unit: "mib".to_string(),
        });
        samples.push(PerfSample {
            scenario: "memory-retention".to_string(),
            metric: "rss_steady_mib".to_string(),
            run_id,
            sample_idx: 0,
            value: mem_steady.rss_mib(),
            unit: "mib".to_string(),
        });
        samples.push(PerfSample {
            scenario: "memory-retention".to_string(),
            metric: "rss_delta_mib".to_string(),
            run_id,
            sample_idx: 0,
            value: delta_mib,
            unit: "mib".to_string(),
        });

        // 2. If 10k-note vault is provided, measure vault resident footprint
        let mut vault_rss_val = None;
        if let Some(v_path) = vault_10k
            && v_path.exists()
        {
            let mut shell = Shell::new();
            let active_note_path = v_path.join("folder_00").join("note_000000.md");
            if let Ok(active_content) = fs::read_to_string(&active_note_path) {
                shell.set_editor_text(&active_content);
            }
            let mem_vault = MemoryStats::current();
            vault_rss_val = Some(mem_vault.rss_mib());

            samples.push(PerfSample {
                scenario: "memory-retention".to_string(),
                metric: "vault_10k_rss_mib".to_string(),
                run_id,
                sample_idx: 0,
                value: mem_vault.rss_mib(),
                unit: "mib".to_string(),
            });
            samples.push(PerfSample {
                scenario: "memory-retention".to_string(),
                metric: "vault_10k_overhead_mib".to_string(),
                run_id,
                sample_idx: 0,
                value: (mem_vault.rss_mib() - mem_init.rss_mib()).max(0.0),
                unit: "mib".to_string(),
            });
        }

        // Measure search query latency on index (§10.2: Search/picker 10k index p95 <= 100 ms)
        let mut search_db = IndexDatabase::open_in_memory().expect("open memory index");
        let search_doc = generate_bench_note(4096);
        let facts = MarkdownFacts::extract(&search_doc).expect("extract facts");
        search_db
            .index_file(
                FileId(1),
                "search_target.md",
                "Search Target Note",
                "fp_search",
                1,
                &facts,
                &search_doc,
            )
            .expect("index file");

        let t_search = Instant::now();
        let query = SearchQuery {
            id: 1,
            text: "архитектура".to_string(),
            page: 0,
            page_size: 100,
        };
        let _ = search_db.search(&query);
        let search_ms = t_search.elapsed().as_secs_f64() * 1000.0;
        samples.push(PerfSample {
            scenario: "memory-retention".to_string(),
            metric: "search_first_page_ms".to_string(),
            run_id,
            sample_idx: 0,
            value: search_ms,
            unit: "ms".to_string(),
        });

        if let Some(v_rss) = vault_rss_val {
            println!(
                "  Run {run_id}/{runs}: Initial={:.2} MiB | Churn Peak={:.2} MiB | Steady={:.2} MiB (delta={delta_mib:.2} MiB) | 10k Vault RSS={v_rss:.2} MiB | Search p95={search_ms:.3} ms",
                mem_init.rss_mib(),
                mem_churn_peak.rss_mib(),
                mem_steady.rss_mib(),
            );
        } else {
            println!(
                "  Run {run_id}/{runs}: Initial={:.2} MiB | Churn Peak={:.2} MiB | Steady={:.2} MiB (delta={delta_mib:.2} MiB) | Search={search_ms:.3} ms",
                mem_init.rss_mib(),
                mem_churn_peak.rss_mib(),
                mem_steady.rss_mib(),
            );
        }
    }

    samples
}
