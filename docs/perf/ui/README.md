# UI: headless microbenchmarks

These measurements use a headless egui context and `NativeApp::show`, with an
1100×700 screen and the editor text `plain **bold** and 字🦀\n` repeated 30,000
times. Each scenario gets a fresh app and context, two warmup frames, then 100
timed frames. Five paired baseline/current runs alternated order. Timings cover
`context.run_ui`, `app.show`, and clearing `textures_delta`; setup is outside the
timer. The CSV files contain per-frame medians and ranges across those runs.

| Scenario | Before, µs/frame median [min, max] | After, µs/frame median [min, max] | Layout builds before → after | Retained after |
|---|---:|---:|---:|---:|
| `idle_frame` | 121.966 [112.364, 153.206] | 42.793 [41.381, 53.273] | 0 → 0 | 52 |
| `insert_character_frame` | 165.754 [162.649, 176.611] | 83.850 [76.946, 90.156] | 100 → 100 | 52 |
| `insert_newline_frame` | 325.471 [314.557, 349.148] | 139.891 [136.606, 203.762] | 6877 → 200 | 78 |
| `cursor_down_frame` | 148.520 [144.893, 151.822] | 69.869 [62.030, 77.864] | 77 → 77 | 78 |
| `navigator_filter_10000` | 4508.589 [4438.697, 4590.681] | 4168.975 [4129.075, 4263.246] | 0 → 0 | 1 |
| `flash_word_windows` | 47.408 [46.139, 49.668] | 42.876 [42.321, 45.433] | 0 → 0 | 100 |

`insert_character_frame` and `insert_newline_frame` send `i` before warmup,
then send one edit per frame; `cursor_down_frame` sends `j`. Navigator filtering
uses four queries over 10,000 items per iteration. Flash uses 800 Unicode text
ranges from the example corpus. Flash changed from a 47,408 ns to 42,876 ns
median (4.532 µs); this small sample and CPU-warm headless workload do not
establish a broad FPS or interactive-performance gain. The runs were not pinned
to a CPU or governor. A revision-gap regression test also verifies that a direct
`Buffer` edit followed by a published edit cannot leave the old layout cache
active.

These are headless CPU timings, not GPU frame rate, interactive latency, or RSS.
The retained column is the example's retained-cache count, not memory usage.

For a comparable run, build and execute the same example in both checkouts;
the baseline is `35f4cdef92155f56aa7a0dcf4bb8fc9dfb03729d`. The example was
copied to that checkout so both versions use the same harness.

```sh
rtk proxy cargo build --release -p truff-ui --example ui_perf --locked --offline
rtk proxy env XDG_CONFIG_HOME="$(mktemp -d)" target/release/examples/ui_perf
rtk proxy env XDG_CONFIG_HOME="$(mktemp -d)" target/release/examples/ui_perf
```

Use a separate empty `XDG_CONFIG_HOME` for every invocation; repeat five times
in alternating baseline/current order. Results are in
`before.csv` and `after.csv`; scenario setup and timer boundaries are in
`crates/ui/examples/ui_perf.rs`.
