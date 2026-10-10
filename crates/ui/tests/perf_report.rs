//! A headless report must not invent measurements or certify native release gates.
use truff_ui::perf_scenarios::{PerfSample, write_summary_md};

#[test]
fn report_contains_only_supplied_measurements_without_release_claims() {
    let path = std::env::temp_dir().join(format!("truff-report-{}.md", std::process::id()));
    write_summary_md(&path, &[]).unwrap();
    let empty = std::fs::read_to_string(&path).unwrap();
    assert!(!empty.contains("PASS"));
    assert!(!empty.contains("[x]"));
    assert!(!empty.contains("0.0%"));
    assert!(empty.contains("No samples"));
    write_summary_md(
        &path,
        &[PerfSample {
            scenario: "startup".into(),
            metric: "wall_time_ms".into(),
            run_id: 1,
            sample_idx: 0,
            value: 1234.0,
            unit: "ms".into(),
        }],
    )
    .unwrap();
    let populated = std::fs::read_to_string(&path).unwrap();
    assert!(populated.contains("1234"));
    assert!(!populated.contains("rss_steady_mib"));
    assert!(!populated.contains("PASS"));
    std::fs::remove_file(path).unwrap();
}
