# optim_ui.md execution and measurements

Plan: `optim_ui.md`. Baseline: `38e7068`, branch `argon-codex`.

Baseline workspace tests: 257 passed across 53 suites.

The plan is an optimization proposal without a separate specification. Existing behavior and tests remain the correctness contract.

## Decisions

- Ruling: work in the existing feature branch and leave changes in the user's checkout — the requested outcome is a local optimization, with no integration step — cost if wrong: the user must move the diff to another branch.
- Ruling: keep `right_model` and `row_vim` — routing uses both, including row search and counted motions — removing them would break panel navigation.
- Ruling: preserve caller-supplied file labels when they differ from the basename — backlinks and boards use full paths — otherwise ambiguous filenames lose context.
- Ruling: preserve identifier and depth widths until narrowing has a validated input bound — lossy casts can collide or truncate — costs several bytes per item versus the proposal.
- Ruling: use `Cow<'static, str>` for categories — built-in labels are borrowed, but custom command groups can be owned — costs one enum word versus a static reference.
- Ruling: keep custom-font fallbacks — a custom font can lack Cyrillic, emoji or other glyphs — retains fallback font data.
- Ruling: preserve full-document Markdown parsing for links and anchors, and contextual color validation — multiline links, reference definitions and fenced code need surrounding text — these paths still allocate a document copy. `link_selection` already reads only its selected Rope slice.
- Ruling: keep the configured base font size exact while quantizing heading sizes — configuration is an existing user contract — fractional base settings can still add glyph atlas sizes.
- Ruling: parse path component ranges once before sorting — scanning separators on every comparison doubled measured build time — adds temporary contiguous descriptors, but no copied paths or intermediate tree.
- Ruling: leave the sparse HeightIndex and `malloc_trim` suggestions for separate work — the proposal provides neither a sparse update design nor an allocator contract — remaining index/allocator memory is not reduced here.
- Ruling: borrow lowercase forms of the three built-in category names during filtering — derived path slices initially increased search latency by 15–17%, while repeated category lowercasing allocated per row — custom groups still use the Unicode conversion, and reference tests preserve matching order.

## Implementation and focused verification

- Navigator: boxed labels, borrowed built-in categories, derived file names/parents, one command description, and release of all item/query buffers on close. The 10,000-item retention test failed before the change and passes after it; filtering tests preserve Unicode and subtitle matching.
- Sidebar: remove the duplicate file list, move path ownership into rows, build flat rows with folders before files at every depth. Ownership/order/trailing-separator tests cover duplicate paths, mixed separators and empty components. Mouse-event frames verify folder toggling and nested-file opening.
- Editor: one screen plus four lines of overscan on each side, a Copy typography key, and borrowed source storage when DisplayMap hides nothing. A 100-step cursor test exposed the missing top margin in scroll following; adding it keeps the caret visible with the reduced cache. Custom font changes explicitly clear cached paragraphs.
- Fonts/UI: four explicit UI sizes, four editor size levels, reuse the sidebar frame response, disable eframe memory persistence, release WhichKey hint/category buffers on hide. Existing heading/visual selection checks were adjusted for the intended font size change.
- Rope: restore cursor selections and navigate to byte destinations using local chunk traversal. The nearest-boundary test compares every byte against independent Unicode segmentation across a multi-chunk document, including combining marks, CRLF, ZWJ, flags, invalid UTF-8 offsets and beyond EOF.

## Measurement

`cargo run --release -p truff-ui --example ui_memory_bench` counts requested live/peak heap bytes and allocations with a forwarding System allocator, over five identical synthetic runs. The scenarios cover 10,000 paths in 100 folders with subfolders, navigator open/close, 1,000 plain DisplayMaps, and native egui frames over 30,000 Markdown lines. This excludes GPU memory and pages retained by the system allocator; it is not a whole-process RSS measurement.

`cargo run --release -p truff-ui --example ui_perf` checks frame and navigator-filter CPU latency.

Environment: Intel Core i5-12400F, Linux `7.2.9-arch1-1`, Rust `1.95.0` (`59807616e`), release profile. Measurements are local synthetic samples, not portable thresholds.

Baseline and optimized binaries ran five times each with 100 iterations per CPU scenario, alternating before/after order; no concurrent compilation or tests ran during the final measurements. Heap scenarios also ran five times. Values below are medians; live heap counts were identical across the five runs. Sidebar input order is deliberately unsorted, and the file list is included in the measured allocations. The navigator fixture allocates a temporary name in both versions for comparability; the optimized real UI caller borrows that name.

Raw results: [heap before](optim-ui-heap-before.csv), [heap after](optim-ui-heap-after.csv), [CPU before](optim-ui-cpu-before.csv), [CPU after](optim-ui-cpu-after.csv). The heap CSV has separate two-column `cached_paragraphs` metadata rows; the CPU CSV repeats its header at each run boundary.

To reproduce the optimized scenarios:

```sh
cargo run --offline --release -p truff-ui --example ui_memory_bench
cargo run --offline --release -p truff-ui --example ui_perf
```

The baseline heap example used the same fixtures on `38e7068`, passing the temporary owned name to the old `new_file` constructor; the Rope comparison was added after the change and runs both algorithms in the same optimized binary. CPU baseline results came from the original `ui_perf` release executable built at `38e7068` before implementation.

| Scenario | Live heap before | Live heap after | Change |
| --- | ---: | ---: | ---: |
| Sidebar, 10,000 files | 2,272,172 B | 1,286,924 B | −43.4% |
| Open navigator, 10,000 files | 2,810,004 B | 2,100,004 B | −25.3% |
| Close navigator | 0 B released | 2,100,009 B released | Item/query storage freed |
| 1,000 plain DisplayMaps | 5,184,000 B | 2,776,000 B | −46.5% |
| First native frame | 1,900,675 B | 1,873,151 B | −1.4% |

Sidebar peak requested heap drops from 3,017,516 to 2,375,852 B (−21.3%); allocation count drops from 41,027 to 20,226. First-frame paragraph count drops from 52 to 30 (−42.3%), and the moving/editing CPU fixtures finish with 35 cached paragraphs instead of 78. This does **not** imply a proportional reduction in whole-process memory: HeightIndex and font state remain resident.

| CPU scenario | Before | After |
| --- | ---: | ---: |
| Sidebar build, 10,000 files | 3.407 ms | 3.427 ms |
| Navigator open, 10,000 files | 1.814 ms | 1.503 ms |
| Navigator close, 10,000 files | 0.000026 ms | 0.141 ms |
| Idle frame | 48.109 µs | 41.497 µs |
| Insert character frame | 85.341 µs | 86.568 µs |
| Insert newline frame | 148.636 µs | 133.329 µs |
| Cursor down frame | 71.354 µs | 60.155 µs |
| Navigator filter, four queries on 10,000 items | 4.243 ms | 3.098 ms |
| Flash word windows | 42.785 µs | 41.487 µs |

Tree building and character insertion vary by about 1% between the medians; these runs do not establish a meaningful latency change in those scenarios. Freeing the closed navigator intentionally costs a traversal of its owned items. Headless frame timings cover egui CPU work, not platform events, rendering/presentation or GPU time.

The separate 8 MiB cursor-snap fixture queries an offset inside a combining sequence near EOF. Copying and scanning the document takes 122.057 ms, requests a peak of 16,777,312 B, and makes 17 allocations. Local Rope traversal returns the same position in 6.760 µs, with 96 B peak and 2 allocations. This establishes removal of the whole-document copy for restoration/byte navigation, not a universal speedup factor for arbitrary Unicode context. Grapheme traversal can still need extended context for unusually long clusters.

No whole-process RSS, GPU atlas or allocator-to-OS release improvement was measured. The proposal's MiB estimates are not treated as achieved results.

## Final verification

- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace --all-targets`: passed.
- `cargo test --workspace`: 267 passed across 53 suites.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- `git diff --check`: passed.
- Fresh read-only code review against `38e7068`: no confirmed correctness findings. Small final label/filter changes were checked against the independent Unicode filter reference and by the full workspace suite.

Final review rulings: accept the reviewer’s unmeasured RSS/GPU/custom-font appearance boundaries; claim only CPU/requested-heap results above. Verify workspace checks directly, as the reviewer did not rerun them. No deferred minor review findings. The sparse HeightIndex, contextual Markdown-copy removal and OS page release remain separate work.
