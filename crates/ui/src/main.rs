use std::path::{Path, PathBuf};

use eframe::egui;
use truff_ui::app::Application;
use truff_ui::editor::EditorProbe;
use truff_ui::native::NativeApp;
use truff_ui::perf_scenarios::{
    PerfSample, run_edit_latency, run_memory_retention, run_scroll_layout, run_startup,
    run_workers_shutdown, write_metadata_toml, write_samples_csv, write_summary_md,
};

fn print_help() {
    println!("Truff v2 — Native Markdown Editor & Knowledge Workspace");
    println!();
    println!("Usage: truff [OPTIONS]");
    println!();
    println!("Options:");
    println!(
        "  --scenario <NAME>    Run headless component benchmark (startup, edit-latency, scroll-layout, memory-retention, workers-shutdown, all)"
    );
    println!("  --vault <PATH>       Vault directory to open (or benchmark data)");
    println!("  --runs <N>           Number of benchmark iterations (default: 5)");
    println!(
        "  --out <DIR>          Output directory for benchmark reports (samples.csv, metadata.toml, summary.md)"
    );
    println!("  --probe              Run headless EditorProbe Parley/Rope layout check");
    println!("  --headless           Run in headless mode without opening GUI window");
    println!("  --help, -h           Show this help message");
}

#[cfg(target_os = "linux")]
fn configure_linux_egl_vendor_if_needed() {
    if std::env::var_os("__EGL_VENDOR_LIBRARY_FILENAMES").is_none() {
        let nvidia_loaded = Path::new("/proc/driver/nvidia/version").exists()
            || Path::new("/sys/module/nvidia").exists();
        let nvidia_icd = Path::new("/usr/share/glvnd/egl_vendor.d/10_nvidia.json");
        if nvidia_loaded && nvidia_icd.is_file() {
            let has_intel_or_amd = (0..8).any(|i| {
                let card = format!("/sys/class/drm/card{i}/device/vendor");
                if let Ok(v) = std::fs::read_to_string(&card) {
                    let v = v.trim();
                    v == "0x8086" || v == "0x1002"
                } else {
                    false
                }
            });
            if !has_intel_or_amd {
                // SAFETY: Called early in main before graphics initialization while single-threaded.
                unsafe {
                    std::env::set_var(
                        "__EGL_VENDOR_LIBRARY_FILENAMES",
                        "/usr/share/glvnd/egl_vendor.d/10_nvidia.json",
                    );
                }
            }
        }
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    configure_linux_egl_vendor_if_needed();

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
        let mut probe =
            EditorProbe::new("Truff v2 editor viewport probe\nTesting egui & Crop\nПривет мир!")
                .expect("small probe text is valid");
        probe.relayout(Some(800.0));
        let hit = probe.hit_test(100.0, 10.0);
        println!(
            "Probe layout height: {}, hit test offset: {hit}",
            probe.hit_test(10.0, 10.0)
        );
        return;
    }

    let mut application = Application::new();
    if let Some(pos) = args.iter().position(|arg| arg == "--vault") {
        let Some(path) = args.get(pos + 1) else {
            eprintln!("--vault requires a directory");
            std::process::exit(1);
        };
        if let Err(error) = application.open_vault(Path::new(path)) {
            eprintln!("Cannot open vault: {error}");
            std::process::exit(1);
        }
    } else {
        let _ = application.try_auto_open_saved_vault();
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_app_id("truff")
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([640.0, 400.0]),
        depth_buffer: 0,
        stencil_buffer: 0,
        multisampling: 0,
        // On Wayland, SwapInterval::Wait (vsync: true) causes eglSwapBuffers to block
        // the main thread when the window is occluded/on an inactive workspace, preventing
        // winit from answering xdg_wm_base.ping and triggering OS "Not Responding" dialogs.
        glow_options: eframe::egui_glow::GlowConfiguration {
            vsync: false,
            ..Default::default()
        },
        ..Default::default()
    };
    if let Err(error) = eframe::run_native(
        "Truff v2",
        options,
        Box::new(move |context| Ok(Box::new(NativeApp::new(application, &context.egui_ctx)))),
    ) {
        eprintln!("Cannot run Truff: {error}");
        std::process::exit(1);
    }
}
