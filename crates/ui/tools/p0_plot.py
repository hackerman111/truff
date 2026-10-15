#!/usr/bin/env python3
"""Plot saved CPU frame samples; excludes native presentation and input latency."""
import csv
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt


def main():
    root = Path(__file__).resolve().parents[1] / "evidence"
    fig, ax = plt.subplots(figsize=(7, 4))
    for folder, label in [
        ("p0-before", "Before: selection moves, viewport stays"),
        ("p0-after", "After: selection and viewport move"),
    ]:
        with (root / folder / "perf.txt").open() as source:
            rows = [r for r in csv.DictReader(source) if r["scenario"] == "navigator"]
        counts = [int(r["items"]) for r in rows]
        ax.plot(counts, [float(r["p95_ms"]) for r in rows], "o-", label=label)
    ax.set_xscale("log")
    ax.set_xticks(counts, labels=[str(n) for n in counts])
    ax.set_xlabel("Navigator items")
    ax.set_ylabel("CPU egui frame p95 (ms)")
    ax.set_ylim(0, max(float(r["p95_ms"]) for r in rows) * 1.4)
    ax.grid(alpha=.25)
    ax.legend(fontsize=9)
    ax.set_title("UI-P0: 500 release frames; GPU/native latency excluded")
    fig.tight_layout()
    fig.savefig(root / "p0-cpu-frame-p95.png", dpi=160)


if __name__ == "__main__":
    main()
