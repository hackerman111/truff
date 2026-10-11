use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::perf_scenarios::metrics::PerfSample;

pub fn write_samples_csv(path: &Path, samples: &[PerfSample]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    writeln!(file, "scenario,metric,run_id,sample_idx,value,unit")?;
    for s in samples {
        writeln!(
            file,
            "{},{},{},{},{:.4},{}",
            s.scenario, s.metric, s.run_id, s.sample_idx, s.value, s.unit
        )?;
    }
    Ok(())
}

pub fn write_metadata_toml(path: &Path, bin_path: Option<&Path>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bin_path_buf = bin_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("./target/release/truff"));
    let bin_size_bytes = fs::metadata(&bin_path_buf).map(|m| m.len()).unwrap_or(0);
    let bin_size_mib = bin_size_bytes as f64 / (1024.0 * 1024.0);

    let mut file = File::create(path)?;
    writeln!(
        file,
        "# Headless component run metadata; no native window is created."
    )?;
    writeln!(file, "# Generated automatically on benchmark completion.\n")?;

    writeln!(file, "[host]")?;
    writeln!(file, "os = \"{}\"", std::env::consts::OS)?;
    writeln!(file, "arch = \"{}\"", std::env::consts::ARCH)?;
    writeln!(file, "debug_assertions = {}", cfg!(debug_assertions))?;
    writeln!(file, "measurement_boundary = \"headless components\"\n")?;

    writeln!(file, "[binary]")?;
    writeln!(file, "path = \"{}\"", bin_path_buf.display())?;
    writeln!(file, "size_bytes = {}", bin_size_bytes)?;
    writeln!(file, "size_mib = {:.2}\n", bin_size_mib)?;

    writeln!(file, "[thresholds]")?;
    writeln!(file, "empty_window_rss_limit_mib = 100.0")?;
    writeln!(file, "empty_window_rss_target_mib = 60.0")?;
    writeln!(file, "vault_10k_rss_limit_mib = 160.0")?;
    writeln!(file, "vault_10k_overhead_limit_mib = 60.0")?;
    writeln!(file, "core_edit_p95_ms = 2.0")?;
    writeln!(file, "warm_scroll_frame_p95_ms = 16.7")?;
    writeln!(file, "visible_layout_ready_p95_ms = 50.0")?;
    writeln!(file, "warm_startup_p95_ms = 700.0")?;
    writeln!(file, "idle_cpu_30s_max_percent = 1.0")?;

    Ok(())
}

pub fn write_summary_md(path: &Path, samples: &[PerfSample]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    writeln!(file, "# Truff headless component measurements\n")?;
    writeln!(
        file,
        "`--scenario` runs Core/Store/Shell and Parley without a native window. \
        These samples exclude window rendering and presentation; they do not certify GUI budgets, \
        native keyboard workflows, idle CPU, or v1 release readiness.\n"
    )?;
    writeln!(
        file,
        "`startup` measures in-process initialization and a layout probe; \
        `note_open_1mib_ms` measures Rope construction. `scroll_frame_*` measures paragraph \
        layout work, not a presented frame. RSS/PSS belong to this headless process.\n"
    )?;
    if samples.is_empty() {
        writeln!(file, "No samples supplied; no measurements reported.")?;
        return Ok(());
    }
    // Report only supplied metrics. Means aggregate run-level samples, including named percentiles.
    let mut groups = std::collections::BTreeMap::new();
    for sample in samples {
        groups
            .entry((&sample.scenario, &sample.metric, &sample.unit))
            .or_insert_with(Vec::new)
            .push(sample.value);
    }
    writeln!(
        file,
        "| Scenario | Metric | Samples | Mean | Min | Max | Unit |"
    )?;
    writeln!(file, "| --- | --- | ---: | ---: | ---: | ---: | --- |")?;
    for ((scenario, metric, unit), values) in groups {
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        writeln!(
            file,
            "| {scenario} | {metric} | {} | {mean:.4} | {min:.4} | {max:.4} | {unit} |",
            values.len()
        )?;
    }
    writeln!(
        file,
        "\nMeans of metrics named p50/p95/p99 are means of per-run percentiles, \
        not pooled event percentiles. Raw records are in `samples.csv`. \
        Native integration evidence is recorded separately in `docs/perf/p7/summary.md`."
    )?;
    Ok(())
}
