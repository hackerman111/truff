# Truff headless component measurements

`--scenario` runs Core/Store/Shell and Parley without a native window. These samples exclude window rendering and presentation; they do not certify GUI budgets, native keyboard workflows, idle CPU, or v1 release readiness.

`startup` measures in-process initialization and a layout probe; `note_open_1mib_ms` measures Rope construction. `scroll_frame_*` measures paragraph layout work, not a presented frame. RSS/PSS belong to this headless process.

| Scenario | Metric | Samples | Mean | Min | Max | Unit |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| startup | note_open_1mib_ms | 3 | 4.9224 | 4.7517 | 5.0943 | ms |
| startup | pss_mib | 3 | 24.7975 | 23.9512 | 25.9785 | mib |
| startup | rss_peak_mib | 3 | 27.7279 | 26.3164 | 28.5312 | mib |
| startup | rss_steady_mib | 3 | 27.2878 | 26.3164 | 28.5312 | mib |
| startup | wall_time_ms | 3 | 62.9936 | 62.2226 | 63.8955 | ms |

Means of metrics named p50/p95/p99 are means of per-run percentiles, not pooled event percentiles. Raw records are in `samples.csv`. Native integration evidence is recorded separately in `docs/perf/p7/summary.md`.
