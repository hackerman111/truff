//! Performance Benchmark Scenarios, Harness, and Metrics Collection (P6-P7 / T13).
//!
//! Headless component measurements; these do not certify native GUI budgets in §10.2 / §10.3:
//! - `startup`: In-process core/store/shell initialization and a Parley layout probe.
//! - `edit-latency`: High-resolution per-edit latency for Vim operations (p50, p95, p99).
//! - `scroll-layout`: Virtualized paragraph layout and HeightIndex caching during scroll.
//! - `memory-retention`: RSS/PSS memory retention under view churn and 10k-note vaults.
//! - `workers-shutdown`: Background durable queue completion and graceful shutdown timing.

mod editing;
mod fixtures;
mod memory;
mod metrics;
mod report;
mod scroll;
mod shutdown;
mod startup;

pub use editing::run_edit_latency;
pub use fixtures::generate_bench_note;
pub use memory::run_memory_retention;
pub use metrics::{MemoryStats, PerfSample, percentile};
pub use report::{write_metadata_toml, write_samples_csv, write_summary_md};
pub use scroll::run_scroll_layout;
pub use shutdown::run_workers_shutdown;
pub use startup::run_startup;
