use std::fs;

/// Process memory statistics sampled from Linux `/proc/self/status` and `/proc/self/smaps_rollup`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryStats {
    pub rss_kib: u64,
    pub peak_rss_kib: u64,
    pub pss_kib: u64,
}

impl MemoryStats {
    pub fn current() -> Self {
        let mut rss_kib = 0;
        let mut peak_rss_kib = 0;
        let mut pss_kib = 0;

        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    rss_kib = parse_proc_kb_line(line);
                } else if line.starts_with("VmHWM:") {
                    peak_rss_kib = parse_proc_kb_line(line);
                }
            }
        }

        if let Ok(smaps) = fs::read_to_string("/proc/self/smaps_rollup") {
            for line in smaps.lines() {
                if line.starts_with("Pss:") {
                    pss_kib = parse_proc_kb_line(line);
                }
            }
        }

        Self {
            rss_kib,
            peak_rss_kib,
            pss_kib,
        }
    }

    pub fn rss_mib(&self) -> f64 {
        self.rss_kib as f64 / 1024.0
    }

    pub fn peak_rss_mib(&self) -> f64 {
        self.peak_rss_kib as f64 / 1024.0
    }

    pub fn pss_mib(&self) -> f64 {
        self.pss_kib as f64 / 1024.0
    }
}

fn parse_proc_kb_line(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|val| val.parse::<u64>().ok())
        .unwrap_or(0)
}

/// An individual metric measurement record.
#[derive(Debug, Clone)]
pub struct PerfSample {
    pub scenario: String,
    pub metric: String,
    pub run_id: usize,
    pub sample_idx: usize,
    pub value: f64,
    pub unit: String,
}

/// Computes percentile from a sorted slice of values.
pub fn percentile(sorted_samples: &[f64], pct: f64) -> f64 {
    if sorted_samples.is_empty() {
        return 0.0;
    }
    let idx = ((sorted_samples.len() as f64 - 1.0) * pct).round() as usize;
    sorted_samples[idx.min(sorted_samples.len() - 1)]
}
