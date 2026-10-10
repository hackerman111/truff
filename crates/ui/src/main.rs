use std::path::{Path, PathBuf};

use masonry::core::{ErasedAction, NewWidget, WidgetId};
use masonry::dpi::LogicalSize;
use masonry::theme::default_property_set;
use masonry_winit::app::{AppDriver, DriverCtx, NewWindow, WindowId};
use masonry_winit::winit::window::Window;
use truff_ui::editor::{EditorProbe, EditorWidget};
use truff_ui::perf_scenarios::{
    PerfSample, run_edit_latency, run_memory_retention, run_scroll_layout, run_startup,
    run_workers_shutdown, write_metadata_toml, write_samples_csv, write_summary_md,
};

struct TruffDriver;

impl AppDriver for TruffDriver {
    fn on_action(
        &mut self,
        _window_id: WindowId,
        _ctx: &mut DriverCtx<'_, '_>,
        _widget_id: WidgetId,
        _action: ErasedAction,
    ) {
    }
}

fn print_help() {
    println!("Truff v2 — Native Markdown Editor & Knowledge Workspace");
    println!();
    println!("Usage: truff [OPTIONS]");
    println!();
    println!("Options:");
    println!(
        "  --scenario <NAME>    Run benchmark scenario (startup, edit-latency, scroll-layout, memory-retention, workers-shutdown, all)"
    );
    println!("  --vault <PATH>       Vault directory for benchmark data");
    println!("  --runs <N>           Number of benchmark iterations (default: 5)");
    println!(
        "  --out <DIR>          Output directory for benchmark reports (samples.csv, metadata.toml, summary.md)"
    );
    println!("  --probe              Run headless EditorProbe Parley/Rope layout check");
    println!("  --headless           Run in headless mode without opening GUI window");
    println!("  --help, -h           Show this help message");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return;
    }

    // Check for scenario benchmark mode
    if let Some(pos) = args.iter().position(|arg| arg == "--scenario") {
        let scenario_name = args
            .get(pos + 1)
            .cloned()
            .unwrap_or_else(|| "all".to_string());

        let mut vault_path = None;
        if let Some(v_pos) = args.iter().position(|arg| arg == "--vault")
            && let Some(v) = args.get(v_pos + 1)
        {
            vault_path = Some(PathBuf::from(v));
        }

        let mut runs = 5;
        if let Some(r_pos) = args.iter().position(|arg| arg == "--runs")
            && let Some(r_str) = args.get(r_pos + 1)
            && let Ok(parsed_runs) = r_str.parse::<usize>()
        {
            runs = parsed_runs;
        }

        let mut out_dir = None;
        if let Some(o_pos) = args.iter().position(|arg| arg == "--out")
            && let Some(o) = args.get(o_pos + 1)
        {
            out_dir = Some(PathBuf::from(o));
        }

        println!("=== Truff v2 Performance Scenario Runner ===");
        println!("Scenario: {scenario_name}");
        println!("Runs: {runs}");
        println!(
            "Vault: {:?}",
            vault_path.as_deref().unwrap_or(Path::new("<synthetic>"))
        );
        println!(
            "Output: {:?}",
            out_dir.as_deref().unwrap_or(Path::new("<stdout only>"))
        );
        println!();

        let mut all_samples: Vec<PerfSample> = Vec::new();

        match scenario_name.as_str() {
            "startup" => {
                all_samples.extend(run_startup(vault_path.as_deref(), runs));
            }
            "edit-latency" => {
                all_samples.extend(run_edit_latency(runs));
            }
            "scroll-layout" => {
                all_samples.extend(run_scroll_layout(runs));
            }
            "memory-retention" => {
                all_samples.extend(run_memory_retention(vault_path.as_deref(), runs));
            }
            "workers-shutdown" => {
                all_samples.extend(run_workers_shutdown(runs));
            }
            "all" => {
                all_samples.extend(run_startup(vault_path.as_deref(), runs));
                all_samples.extend(run_edit_latency(runs));
                all_samples.extend(run_scroll_layout(runs));
                all_samples.extend(run_memory_retention(vault_path.as_deref(), runs));
                all_samples.extend(run_workers_shutdown(runs));
            }
            unknown => {
                eprintln!(
                    "Unknown scenario: {unknown}. Expected one of: startup, edit-latency, scroll-layout, memory-retention, workers-shutdown, all"
                );
                std::process::exit(1);
            }
        }

        if let Some(ref out) = out_dir {
            let csv_path = out.join("samples.csv");
            if let Err(e) = write_samples_csv(&csv_path, &all_samples) {
                eprintln!("Failed to write samples CSV to {}: {e}", csv_path.display());
            } else {
                println!("Samples written to {}", csv_path.display());
            }

            let meta_path = out.join("metadata.toml");
            let bin_path = std::env::current_exe().ok();
            if let Err(e) = write_metadata_toml(&meta_path, bin_path.as_deref()) {
                eprintln!(
                    "Failed to write metadata TOML to {}: {e}",
                    meta_path.display()
                );
            } else {
                println!("Metadata written to {}", meta_path.display());
            }

            let summary_path = out.join("summary.md");
            if let Err(e) = write_summary_md(&summary_path, &all_samples) {
                eprintln!(
                    "Failed to write summary MD to {}: {e}",
                    summary_path.display()
                );
            } else {
                println!("Summary written to {}", summary_path.display());
            }
        }

        println!("\nScenario run complete.");
        return;
    }

    if args
        .iter()
        .any(|arg| arg == "--probe" || arg == "--headless")
    {
        println!("Truff v2 Editor Probe (headless benchmark)");
        let mut probe = EditorProbe::new(
            "Truff v2 editor probe initial text\nTesting Parley & Crop\nПривет мир!",
        );
        probe.relayout(Some(800.0));
        let hit = probe.hit_test(100.0, 10.0);
        println!(
            "Probe layout height: {}, hit test offset: {hit}",
            probe.hit_test(10.0, 10.0)
        );
        return;
    }

    let sample_text = "Welcome to Truff v2!\n\nMinimal retained editor probe using crop::Rope and Parley.\nПоддержка Unicode, кириллицы, emoji 🦀 и IME.\n";
    let editor = EditorWidget::new(sample_text);

    let window_size = LogicalSize::new(900.0, 600.0);
    let window_attributes = Window::default_attributes()
        .with_title("Truff v2 — Editor Probe")
        .with_resizable(true)
        .with_min_inner_size(window_size);

    let driver = TruffDriver;
    let event_loop = masonry_winit::app::EventLoop::with_user_event()
        .build()
        .expect("Failed to create event loop");

    masonry_winit::app::run_with(
        event_loop,
        vec![NewWindow::new(
            window_attributes,
            NewWidget::new(editor).erased(),
        )],
        driver,
        default_property_set(),
    )
    .expect("Failed to run Masonry application");
}
