#!/usr/bin/env bash
set -euo pipefail

# Truff v2 Benchmark and Metric Runner
# Accepts: --vault <PATH> --scenario <NAME> --runs <N> --out <DIR>

VAULT=""
SCENARIO=""
RUNS=5
OUT="docs/perf/p0"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --vault)
            VAULT="$2"
            shift 2
            ;;
        --scenario)
            SCENARIO="$2"
            shift 2
            ;;
        --runs)
            RUNS="$2"
            shift 2
            ;;
        --out)
            OUT="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

mkdir -p "$OUT"

echo "=== Truff v2 Performance Measurement ==="
echo "Scenario: ${SCENARIO:-all}"
echo "Vault: ${VAULT:-<in-memory/synthetic>}"
echo "Runs: $RUNS"
echo "Output Directory: $OUT"

python3 - <<PYEOF
import os
import sys
import time
import subprocess
import statistics

out_dir = "$OUT"
scenario = "$SCENARIO" or "baseline"
runs = int("$RUNS")

print(f"Executing {runs} measurement runs for scenario '{scenario}'...")

# Measure binary size
bin_path = "./target/release/truff"
bin_size_bytes = os.path.getsize(bin_path) if os.path.exists(bin_path) else 0

latencies_ms = []
rss_samples_kib = []

for r in range(runs):
    t0 = time.perf_counter()
    p = subprocess.Popen([bin_path, "--probe"], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    stdout, stderr = p.communicate()
    t1 = time.perf_counter()
    
    elapsed_ms = (t1 - t0) * 1000.0
    latencies_ms.append(elapsed_ms)
    
    # Process memory via /proc if available or estimate
    latencies_ms.append(elapsed_ms)

avg_lat = statistics.mean(latencies_ms)
p95_lat = sorted(latencies_ms)[int(len(latencies_ms) * 0.95)]

summary_file = os.path.join(out_dir, "summary.md")
with open(summary_file, "w") as f:
    f.write(f"# Performance Summary: {scenario}\n\n")
    f.write(f"- **Binary Size**: {bin_size_bytes / (1024 * 1024):.2f} MiB\n")
    f.write(f"- **Probe Latency Average**: {avg_lat:.2f} ms\n")
    f.write(f"- **Probe Latency p95**: {p95_lat:.2f} ms\n")
    f.write(f"- **Runs**: {runs}\n")

print(f"Summary written to {summary_file}")
PYEOF
