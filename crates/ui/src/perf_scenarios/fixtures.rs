use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) fn temp_vault_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("truff_perf_{name}_{}_{id}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Generates a representative synthetic 64 KiB or 1 MiB Markdown note for benchmarks.
pub fn generate_bench_note(target_bytes: usize) -> String {
    let mut out = String::with_capacity(target_bytes + 512);
    out.push_str("# Performance Benchmark Note\n\n");
    out.push_str("Created for automated Truff v2 benchmarks. Invariants: UTF-8, Markdown, Parley layout.\n\n");

    let paragraphs = [
        "Текст с кириллицей, английскими терминами и emoji 🦀 для проверки графем и hit-testing.\n\n",
        "## Раздел с кодом\n\n```rust\nfn calculate_layout_metrics(w: f32) -> f32 {\n    w * 1.618\n}\n```\n\n",
        "- Элемент списка с wikilink [[TargetNote#Heading|Custom Label]] и подсветкой ==highlight==.\n",
        "> [!note] Ключевой инвариант\n> Markdown — единственный источник истины, SQLite — удаляемый индекс.\n\n",
    ];

    let mut idx = 0;
    while out.len() < target_bytes {
        out.push_str(paragraphs[idx % paragraphs.len()]);
        idx += 1;
    }

    out
}
