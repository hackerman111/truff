# Truff headless component measurements

`--scenario` runs Core/Store/Shell and Parley without a native window. These samples exclude window rendering and presentation; they do not certify GUI budgets, native keyboard workflows, idle CPU, or v1 release readiness.

`startup` measures in-process initialization and a layout probe; `note_open_1mib_ms` measures Rope construction. `scroll_frame_*` measures paragraph layout work, not a presented frame. RSS/PSS belong to this headless process.

| Scenario | Metric | Samples | Mean | Min | Max | Unit |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| memory-retention | rss_delta_mib | 3 | 4.8477 | 0.0000 | 14.5430 | mib |
| memory-retention | rss_initial_mib | 3 | 20.6237 | 9.3086 | 26.3633 | mib |
| memory-retention | rss_steady_mib | 3 | 25.4167 | 23.8516 | 26.1992 | mib |
| memory-retention | search_first_page_ms | 3 | 0.1886 | 0.1881 | 0.1890 | ms |

Means of metrics named p50/p95/p99 are means of per-run percentiles, not pooled event percentiles. Raw records are in `samples.csv`. Native integration evidence is recorded separately in `docs/perf/p7/summary.md`.
