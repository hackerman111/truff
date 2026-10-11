//! Performance benchmarks and latency measurements for the EditorProbe (P0 baseline).

use std::time::Instant;
use truff_ui::editor::{EditorInput, EditorProbe};

#[test]
fn test_probe_1mib_performance_baseline() {
    // Generate ~1 MiB of diverse text (English, Cyrillic, emoji, code, headings)
    let paragraph = "Архитектура Truff v2: минимальный retained-редактор, высокая производительность и надёжность. 🦀\n\
                     The quick brown fox jumps over the lazy dog. UTF-8 boundaries and grapheme clusters are preserved.\n\
                     ```rust\n\
                     fn verify() -> bool { true }\n\
                     ```\n";
    let repeat_count = (1024 * 1024) / paragraph.len() + 1;
    let text_1mib = paragraph.repeat(repeat_count);
    let total_bytes = text_1mib.len();
    assert!(total_bytes >= 1024 * 1024, "Text should be at least 1 MiB");

    // 1. Rope construction latency
    let t0 = Instant::now();
    let mut probe = EditorProbe::new(&text_1mib).unwrap();
    let rope_init_duration = t0.elapsed();
    println!("1 MiB Rope Init: {rope_init_duration:?} for {total_bytes} bytes");
    let init_threshold_ms = if cfg!(debug_assertions) { 150 } else { 50 };
    assert!(
        rope_init_duration.as_millis() < init_threshold_ms,
        "1 MiB Rope initialization must be under {init_threshold_ms}ms (got {rope_init_duration:?})"
    );

    // 2. Bounded viewport layout, independent of the document's total byte count.
    let t1 = Instant::now();
    probe.relayout(Some(800.0));
    let layout_duration = t1.elapsed();
    println!("1 MiB document, bounded viewport layout: {layout_duration:?}");

    // 3. Hit testing latency (1000 queries)
    let queries = 1000;
    let t2 = Instant::now();
    for i in 0..queries {
        let x = (i % 800) as f32;
        let y = ((i * 17) % 5000) as f32;
        let hit = probe.hit_test(x, y);
        assert!(hit <= total_bytes);
    }
    let hit_test_duration = t2.elapsed();
    let avg_hit_us = hit_test_duration.as_micros() as f64 / queries as f64;
    println!("1000 Hit Tests: {hit_test_duration:?} (avg {avg_hit_us:.2} µs/query)");
    let hit_threshold_us = if cfg!(debug_assertions) {
        2000.0
    } else {
        200.0
    };
    assert!(
        avg_hit_us < hit_threshold_us,
        "Hit test average latency must be under {hit_threshold_us}µs (got {avg_hit_us:.2}µs)"
    );

    // 4. 500 sequential edits
    let t3 = Instant::now();
    for i in 0..500 {
        probe
            .handle_input(EditorInput::InsertText(format!("{i} ")))
            .unwrap();
    }
    let edits_duration = t3.elapsed();
    let avg_edit_us = edits_duration.as_micros() as f64 / 500.0;
    println!("500 Edits: {edits_duration:?} (avg {avg_edit_us:.2} µs/edit)");
    let edit_threshold_us = if cfg!(debug_assertions) {
        20000.0
    } else {
        5000.0
    };
    assert!(
        avg_edit_us < edit_threshold_us,
        "Average edit latency must be under {edit_threshold_us}µs (got {avg_edit_us:.2}µs)"
    );
}
