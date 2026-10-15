# Truff headless component measurements

`--scenario` runs Core/Store/Shell and Parley without a native window. These samples exclude window rendering and presentation; they do not certify GUI budgets, native keyboard workflows, idle CPU, or v1 release readiness.

`startup` measures in-process initialization and a layout probe; `note_open_1mib_ms` measures Rope construction. `scroll_frame_*` measures paragraph layout work, not a presented frame. RSS/PSS belong to this headless process.

| Scenario | Metric | Samples | Mean | Min | Max | Unit |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| scroll-layout | cache_hit_rate_percent | 3 | 97.3654 | 97.3654 | 97.3654 | % |
| scroll-layout | cold_layout_ms | 3 | 51.9352 | 51.6854 | 52.2861 | ms |
| scroll-layout | frame_latency_ms | 600 | 0.0172 | 0.0093 | 0.0285 | ms |
| scroll-layout | paragraph_layout_ready_ms | 3 | 0.0121 | 0.0120 | 0.0122 | ms |
| scroll-layout | scroll_frame_p50_ms | 3 | 0.0173 | 0.0173 | 0.0174 | ms |
| scroll-layout | scroll_frame_p95_ms | 3 | 0.0193 | 0.0192 | 0.0195 | ms |

Means of metrics named p50/p95/p99 are means of per-run percentiles, not pooled event percentiles. Raw records are in `samples.csv`. Native integration evidence is recorded separately in `docs/perf/p7/summary.md`.
