#!/usr/bin/env bash
set -euo pipefail

# Headless component runner; it does not measure native GUI or presentation.
# Accepts: --vault <PATH> --scenario <NAME> --runs <N> --out <DIR>

VAULT=""
SCENARIO="all"
RUNS=5
OUT="docs/perf/p7/headless"

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
        --help|-h)
            echo "Usage: tools/perf/measure.sh [--vault <DIR>] [--scenario <NAME>] [--runs <N>] [--out <DIR>]"
            echo "Scenarios: startup, edit-latency, scroll-layout, memory-retention, workers-shutdown, all"
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

mkdir -p "$OUT"

echo "=== Truff v2 Headless Component Measurement Harness ==="
echo "Scenario: $SCENARIO"
echo "Vault: ${VAULT:-<in-memory/synthetic>}"
echo "Runs: $RUNS"
echo "Output Directory: $OUT"
echo

BIN_PATH="./target/release/truff"

if [[ ! -f "$BIN_PATH" ]]; then
    echo "Building release binary with perf feature..."
    cargo build --locked --release -p truff-ui --features perf
fi

ARGS=(--scenario "$SCENARIO" --runs "$RUNS" --out "$OUT")
if [[ -n "$VAULT" ]]; then
    ARGS+=(--vault "$VAULT")
fi

echo "Executing: $BIN_PATH ${ARGS[*]}"
"$BIN_PATH" "${ARGS[@]}"

echo
echo "Harness run complete. Artifacts produced in '$OUT':"
ls -la "$OUT"
