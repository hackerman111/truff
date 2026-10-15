# UI implementation — truff-crates-2026-10-14-v1

Scope: `03_UI_AGENT_PROMPT.md`. All changes and artifacts stay in `crates/ui`.
Branch: `argon-codex`. Existing untracked root prompts belong to the user.

## UI-0 audit

Current input owners: Shell Editor/Rows (Core Vim), native Backlinks interception,
Navigator Normal/Search/Actions, egui creation/settings fields, command bar,
Flash, Which-Key Prefix/Help. Navigator rendering duplicated model key handling.
Creation dialogs already stop native event dispatch; Help did not stop `s` or
editor wheel handling. Shell focus history exists; Help currently leaves its
previous pane focus in place.

| Surface | Independent scroll | Baseline geometry |
| --- | --- | --- |
| Navigator Files/Sessions/Boards/Outline/Links/Commands | virtual list + preview | width 760, max 800, height 480, list 380 × 360 |
| FileTree | virtual rows 28 + egui spacing | resizable dock |
| Backlinks | virtual rows 24 + egui spacing | width 220, min 140, max 600 |
| Editor | HeightIndex + local offset | 24 estimated height, real Galley measurements |
| Help | shared Window, inner ScrollArea | bottom anchored, nonresizable, max height 320, 3 columns |
| Prefix Which-Key | no internal scroll | bottom anchored, 2–4 columns |
| Kanban/Stream/Search | native content modules | inspected during P0 list coverage |

Theme already provides semantic colors. Navigator uses theme colors but local
radii 3/4/6 and fixed font sizes 11/12/14; Help uses the same sizes and local
card styling. Broader token migration belongs to P1.

Reproductions are real egui frames in `tests/scroll_follow.rs`:

1. 1000 Navigator entries, 100 selection steps: the selected **list row** is
   absent from the clipped viewport (preview title must not count).
2. 800×600 editor, heading and long Unicode paragraph, `G$`: caret ends below
   the viewport. Estimated row following and measured correction disagree;
   final clamp excludes the editor top margin and follow is only attempted
   while the cursor identity changes.
3. Open Help with `?`, then `s`: Flash becomes active underneath Help.
   Help baseline has no keyboard scrolling handler and fixed 320 px cap.

RED evidence: `cargo test -p truff-ui --test scroll_follow` — 0/3 pass on the
unmodified production code (2026-10-14). Initial harness errors corrected
before recording these behavior failures.

Hot paths: virtualized row rendering, paragraph layout/height lookup; preserve
bounded visible rows/cache and avoid filesystem I/O in paint.

## Progress

- UI-0: complete, including saved baseline frames/screenshots.
- UI-P0: complete for the current functional gate; limitations below.
- UI-P1: complete for the responsive/theme/keyboard gate; persistence boundary below.
- UI-P2: complete for navigator search, palette, and workspace actions.
- UI-P3: complete for editor & core synergy, smart edit, projection, and typography.
- UI-P4: complete for floating TOC, folder wiki, and note stream.
- UI-P5: pending, separate phase as specified.

Ruling: use this crate-local record instead of a root skill scratch ledger,
because the prompt explicitly forbids writes outside `crates/ui`.
Ruling: execute P0 in the existing feature branch and preserve unrelated files.
The first deliverable is P0; the prompt assigns redesign to the following PR.

## Cross-crate requests

None required for P0. Later stages must use public Core/Store APIs; do not copy
Markdown/Vim/search engines into UI.

## UI-P0 acceptance report — 2026-10-14

### Files changed and canonical owners

- `src/rows/scroll.rs`: typed `EnsureVisible` with nearest/start/end alignment,
  observed offset/visible range/viewport height. Shared by FileTree and Backlinks;
  Navigator and Help own the same small controller.
- `src/navigator/{model,render}.rs`: Core VimParser owns count/motion parsing;
  renderer converts native events and draws the requested virtual range.
- `src/native/editor/{mod,invalidation,navigation,viewport}.rs` and paragraph
  geometry visibility: estimate, measure, then paint/follow using the same caret
  rectangle as painting. Per-view source anchors are revision checked and rebased.
- `src/native/{events,content,mod,sidebar,backlinks,hints}.rs`: input ownership,
  native adaptation, virtual rows and large Help geometry.
- `src/overlays/which_key{.rs,/help_state.rs}`, `src/shell/{help,mod,routing}.rs`,
  `src/overlays.rs`: Help keyboard/filter state and active keymap/registry hints.
- `src/app/{mod,navigation,vault,commands/mod,io/catalog_events}.rs`: Backlinks
  use the existing right RowModel instead of a second selection counter.
- `src/native/creation/{file,folder,vault}.rs`: focus the primary text field.
- `tests/scroll_follow.rs`, `examples/p0_scroll_probe.rs`, `tools/p0_*`, this
  report and `evidence/`: regression tests and reproducible acceptance artifacts.

### Confirmed independent causes and fixes

Navigator changed selection but never computed a target offset outside the
rendered range. The controller now computes the offset before `show_rows`, using
the real row pitch and viewport height. Selection/filter/size changes request
following; idle frames observe egui's actual offset and preserve wheel scrolling.
`G`/`gg` request end/start alignment and pages use the measured viewport.

Editor followed an estimated logical row before measuring its Galley, then
painted before applying the final correction. Its bottom clamp omitted the top
margin. Now a distant target row is measured directly, height changes preserve
the source anchor, and the measured caret rectangle determines the final offset
before painting. Pending measurement can request another frame. Page commands
move the caret through the existing Shell API, preserving Visual anchors. Drag
at viewport edges scrolls. Saved view positions use source byte anchors, not
stale line numbers; edits to their buffer rebase inactive positions as well.

Help had a short bottom Window without keyboard navigation, and native input
could start Flash or scroll the underlying editor. Full Help is now a centered
Modal with fixed header/footer and a virtual inner ScrollArea. It supports
counts, j/k, gg/G, page/half-page, filtering and staged Esc. Help entries use the
active compiled keymap and CommandRegistry; Vim-only hints are fallback.

### Geometry and focus

| Window | Full Help target dp | Navigator in this phase |
| --- | --- | --- |
| 800×600 | 688×492 | Existing 760×480 content, 380×360 list |
| 1280×720 | 1100.8×590.4 | Same existing content/list sizes |
| 1920×1080 | 1360×885.6 | Same existing content/list sizes |

Help target includes frame padding; screenshots show the actual rendering.
The spacious responsive Navigator and shared typography/density tokens are
UI-P1, deliberately separate from the first functional correction.

Printable Text events are authoritative. Modal fields and Help Insert belong
to egui TextEdit; Help Normal belongs to its motion handler. Flash's raw `s`
shortcut is scoped to Editor Normal with no active command/search/overlay/prefix.
Event ownership is refreshed between events in a batch, preventing Paste/Cut
from leaking after Help opens. Closing Help consumes the rest of that batch.
Help retains the previous Shell pane focus. Dialogs and Navigator stop native
editor dispatch; Editor wheel/click/drag are blocked while their overlay is open.

### Evidence and verification

Baseline commit: `d49e80433f20c88da74dd132674a912b0f8ee63c`.
Core is being changed independently in the live checkout. To separate UI evidence
from that work, snapshots in `/tmp/truff-ui-p0-baseline` and
`/tmp/truff-ui-p0-check` use baseline Core/Store and respectively original/current
UI. Only `crates/ui` was edited by this task. Live workspace checks also passed.

- `tests/scroll_follow.rs`: 14 passing real egui-frame tests. They verify actual
  clipped list text/caret geometry rather than only selection indices: 1000 rows,
  100 j, counts, G/gg, page, filter shrinking, preview, resize/DPI, 100 Help items,
  Help filter/Esc, modal literal input, Backlinks, Unicode/wrap/heading caret,
  Visual/page/two views, Help event leakage, inactive-view edit rebasing, initial
  gutter, real wheel preservation and held pointer edge drag.
- Isolated original-Core/current-UI `cargo test -p truff-ui`: 142 passed in
  21 suites. Initial three regressions were RED on original UI; review regressions
  also failed before their fixes and now pass.
- Live `cargo fmt --all -- --check`: passed.
- Live `cargo check --workspace --all-targets`: passed.
- Live `cargo test --workspace`: 299 passed in 56 suites.
- Live `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed. `git diff --check -- crates/ui`: passed.
- Read-only final review used **gpt-6-luna**, as requested: no blocking findings.
  Earlier review's event leakage, inactive anchor and gutter findings were fixed
  with regressions before that review.
- Native Xvfb screenshots: 12 originals in `evidence/p0-before`, 12 current Nord
  frames in `evidence/p0-after`, 12 Light frames in `evidence/p0-after-light`.
  Each set captures Help, Help last, Navigator and Navigator after 100 j at all
  three window sizes. `tools/p0_screenshots.py` uses isolated XDG directories.
  Headless DPI tests cover scales 1/1.5/2; screenshots use native scale 1.

### Real performance samples and limits

Release CPU egui probe, 5 warmup + 500 measured frames per workload, 1100×700:

| Workload | Before p50/p95 ms | After p50/p95 ms | Retained before/after |
| --- | --- | --- | --- |
| Navigator 100 | 0.078 / 0.087 | 0.091 / 0.107 | 12 / 11 list text shapes |
| Navigator 1000 | 0.078 / 0.088 | 0.092 / 0.117 | 12 / 11 list text shapes |
| Navigator 10000 | 0.078 / 0.087 | 0.091 / 0.112 | 12 / 11 list text shapes |
| Editor 2000 mixed rows | 0.045 / 0.088 | 0.047 / 0.092 | 30 / 28 paragraphs |

Raw values are in `evidence/p0-{before,after}/perf.txt`; the saved plot is
`evidence/p0-cpu-frame-p95.png`, regenerated by `tools/p0_plot.py`.
Both versions receive physical Key plus Text for Navigator; resets use the same
public `move_top`. Selection moves in both; original UI stays on its first rows,
while fixed UI lays out newly visible titles. The added work costs about
0.02–0.03 ms p95 here. These are functional workload comparisons, not a speedup
claim. An earlier Text-only baseline was discarded because original Navigator
ignored those events. Shared Cargo target reuse required refreshing the snapshot
source timestamps to ensure the current UI was rebuilt before the after run.

Single probe process peak RSS was 15120 KiB in both runs; this includes the
headless 10k-item corpus and process-spawn overhead, and excludes a native GPU
window. Host/compiler/boundaries are saved in `evidence/p0-environment.txt`.
No native memory p95,
hardware trackpad acceleration or input-to-presented-frame latency was measured.
The CPU values alone do not certify the 16.7 ms native frame budget.

Existing component scenarios were also executed (debug, three runs each):
`startup` ~62.2–63.9 ms in-process initialization, `scroll-layout` warm layout
p95 ~0.019–0.020 ms, `memory-retention` steady RSS ~23.9–26.2 MiB. Their raw CSV,
metadata and boundary explanations live under `evidence/p0-after/components-*`.
The memory scenario uses synthetic buffers, not a loaded native 10k-file vault;
its search sample indexes one note. Existing `ui_perf` diagnostics are saved in
`ui-p0-baseline.txt` (before) and `evidence/p0-after/ui-perf-debug.txt` (after).

Reproduce the CPU probe with `cargo run --release -p truff-ui --example
p0_scroll_probe`; capture windows with `python3 crates/ui/tools/p0_screenshots.py
--binary target/debug/truff --out /tmp/truff-ui-p0-shots --theme nord`.

### Remaining work and limits

- UI-P1–P5 remain separate phases; this report does not claim their acceptance
  matrix scenarios are implemented. P1 owns responsive spacious Navigator,
  shared design tokens and density; P3 owns the full glyph/selection audit.
- Rename currently reports a status string; there is no rename modal to test.
  Literal input was verified in existing file/folder/vault creation fields.
- Opening a vault recreates Shell and resets the configured theme (pre-existing
  P1 issue). Light captures explicitly run `:theme light` after vault opening.
  Those captures also show dark TextEdit/button backgrounds and weak contrast
  in Light: the general egui widget visual style still needs P1 token integration.
- Outline, Sessions, Commands, Links and Boards use the corrected Navigator
  list. FileTree/Backlinks use the shared helper. Native standalone Graph,
  Stream, Search and Kanban surfaces are still later-phase work.
- Hardware trackpad inertia/acceleration, native HiDPI screenshots and native
  frame/input/memory budgets require separate measurement.
- No commit, push or PR publication was made; unrelated concurrent Core changes
  remain outside this phase's ownership.


## UI-P1 acceptance report — 2026-10-14

### Design and ownership

`src/theme/geometry.rs` owns density and responsive size calculations;
`src/theme/style.rs` owns egui visuals and typography, including TextEdit/button
backgrounds in Light. `src/components.rs` provides small shared frame, search,
list-row and modal functions. Existing native docks own resizing; egui owns
splitters and panel memory. No dependency or widget framework was added.

Navigator render now composes `render/{chrome,list,preview}.rs`: fixed header,
search and footer, one virtual list and a separately scrolling preview/action
area. List width is 48% of available body space. A list row advances the parent
layout once; painting uses a child Ui to avoid doubling the virtual row pitch.
P0 keyboard following and manual wheel behavior remain covered by regressions.

| Token | Compact | Comfortable |
| --- | --- | --- |
| Navigator | 86vw, max 1360dp × 80vh | same |
| Full Help | 86vw, max 1360dp × 82vh | same |
| Sidebar default | 280dp | 320dp |
| Backlinks default | 320dp | 360dp |
| Editor maximum column | 800dp | 850dp |
| Creation/settings width | 560dp | 600dp |
| Navigator/Help/Backlinks row | 36dp | 42dp |
| Tree row | 28dp | 38dp |
| UI body/button font | 13dp proportional | 14dp proportional |
| Overlay radius | 8dp | 10dp |

Docks clamp their widths on narrow windows. Editor columns become fluid below
the maximum. Dialog widths clamp to the viewport and long bodies scroll. Settings
Apply/Cancel stay outside the scrolling body. Tab/Shift-Tab follow the actual body
control response; footer controls cannot create a body scroll request. Mouse wheel
does not trigger keyboard following. Other creation dialogs retain literal text,
Enter/Escape and their existing storage/error behavior.

Theme colors now reach general egui widget states; hover, selection and focus
share the same palette as overlays and docks. Proportional typography is the new
default for UI and note text; code/commands/keycaps remain monospace, and explicit
user typography settings are preserved. Opening a vault preserves the active
theme and typography instead of recreating Shell with defaults.

`:focus` hides both docks and keeps pane navigation in Editor while hidden;
`:focus` again restores them. `:source`/`:preview` now belong to the active View,
so two Views sharing a buffer can use independent projections. Closed View entries
are pruned when setting projection; changing vault clears the map.
`NativeApp::persist_egui_memory()` enables eframe's existing panel persistence.

### Verification and evidence

- 8 new real egui-frame integration tests in `tests/responsive.rs`: all three
  viewport sizes, DPI 1/1.5/2, both densities, clipped last-row text, Light widget
  visuals, four modal kinds, vault/theme retention, actual splitter drag + saved
  memory roundtrip, independent View projections, focus mode, keyboard access to
  settings theme controls/actions, and both docks at 800×600 with a real backlink.
  The last scenario leaves at least 240dp for Editor and checks painted text/caret.
- A ninth regression in `components.rs` verifies that Tab on a fixed footer
  does not move the body. It fails with the discarded global same-layer focus
  guard and passes with explicitly scoped body responses. RED evidence is saved
  in `evidence/p1-after/footer-scope-red.log`.
- Settings initially failed keyboard access to bottom actions, then to offscreen
  theme choices. Both assertions pass with fixed footer + scoped focus following.
- The native cache test now waits for the second measured panel frame before
  asserting idle reuse. Instrumentation showed the viewport bottom changing
  from 645 to 663dp, admitting one new paragraph (27 → 28), rather than rebuilding
  existing paragraphs. Temporary instrumentation was removed.
- All 14 P0 scroll-follow regressions remain passing.
- Final workspace fmt/check/test/strict clippy passed (308 tests in 57 suites); complete logs are in
  `evidence/p1-after/{fmt,check,test,clippy}.log`. Diff whitespace check passed.
- **gpt-6-luna** reviewed P1 against the saved P0 UI snapshot. Its confirmed
  footer focus-scope finding was fixed and regression-tested. Final scoped review
  found no remaining concrete issue. The initially unverified narrow two-dock
  risk was checked by the new integration test. Reviewer did not run tests.
- 90 native screenshots: 30 each under `evidence/p1-nord`, `p1-light`,
  `p1-comfortable`, at 800×600, 1280×720 and 1920×1080. Captures include Help,
  last Help row, Navigator, 100j, Sidebar, file/folder creation, Settings,
  Settings after Tab navigation, and focus mode. Native scale is 1; DPI coverage
  comes from the egui tests. Screenshot automation waits for new field focus
  before typing `session`.

The same original Core/Store snapshot is used for before/after probes and native
captures (`/tmp/truff-ui-p1-before` and `/tmp/truff-ui-p1-check`). Workspace checks
also exercise the independently updated live Core. Only `crates/ui` was changed
by this phase. No commit, push or PR publication was made in the shared branch.

### Performance sample

Same release CPU egui probe as P0: 1100×700, five warmup and 500 measured frames,
100/1000/10000 Navigator items and 2000 mixed Editor paragraphs. Three alternating
before/after trials run with GUI captures stopped; `perf.txt` reports median trial
p50/p95 and maximum retained count. Individual trials are retained next to it.
The protocol and list-text shape count are identical in both snapshots.

| Workload | Before p50/p95 ms | After p50/p95 ms | Retained before/after |
| --- | --- | --- | --- |
| navigator 100 | 0.086 / 0.096 | 0.075 / 0.080 | 11 / 11 |
| navigator 1000 | 0.092 / 0.098 | 0.073 / 0.081 | 11 / 11 |
| navigator 10000 | 0.087 / 0.096 | 0.076 / 0.080 | 11 / 11 |
| editor 2000 | 0.044 / 0.126 | 0.045 / 0.129 | 28 / 27 |

Retained counts stay bounded by the viewport, not corpus size. These samples
measure CPU-side egui work, excluding GPU presentation, native input latency and
native process memory. They do not establish a general speedup or a native frame
budget. P0 environment/boundary evidence remains applicable to the same host.

### Cross-crate requests and remaining boundaries

1. **Store session presentation metadata:** `SessionViewEntry` currently contains
   only `buffer_path`, `cursor_anchor`, `cursor_head` and `top_line`. Persisting
   Source/LivePreview across restart requires a backward-compatible optional
   presentation field in Store's session API (with a default for old sessions),
   without making Store depend on UI. UI must then write/restore it per serialized
   View, including two Views of the same buffer. Current P1 projection retention
   is within the open Application session; restart persistence is not claimed.
2. **P2 registry migration:** `:focus` uses the existing UI command adapter.
   Palette/which-key discovery and configurable binding should use the canonical
   Core registry when P2 integrates UI actions; do not duplicate command engines.

The old P0 Light-background and vault-theme-reset limitations are resolved here.
P2 provider/search/command unification, P3 glyph/selection/fallback-font audit,
and later board/graph/stream surfaces remain separate work. Default egui fonts
can show missing CJK glyphs on this host; current screenshots are not evidence
of complete Unicode font coverage. Hardware trackpad/native HiDPI and native
input/frame/memory budgets remain unmeasured.


## UI-P2 execution — 2026-10-14

P1 source snapshot: `/tmp/truff-ui-p2-before/crates/ui`. Preserve concurrent
Core changes and root prompts. All implementation and evidence remain in UI.
Use existing public Core/Store APIs; review the final P2 delta with Luna.

Tasks, in order:
1. Registry/keymap execution: leader remapping, Palette commands, contextual hints.
2. One native input owner per event, including a panel opening mid-batch.
3. Navigator providers, provider switching, hierarchy, search/preview/help.
4. Async Store FTS adapter with bounded queues and stale-query rejection.
5. Native rename/trash confirmation and repeatable file workflows.
6. View-specific sessions and buffer actions.
7. Command line completion/history/Normal and Insert routing.
8. Focused regressions, performance/native captures, workspace checks, Luna review.

Ruling: keep phase records in this file and use the existing feature branch —
user scope forbids root skill ledgers and changing other crates.


### P2 review scope and API boundaries

Implemented: compiled leader/palette routing; active binding labels; one native input
owner; egui Text/Paste/IME commit before deferred field controls; 13 Navigator
providers and Tab/Shift-Tab/source selector; cached Core MarkdownFacts Outline and
outgoing links; Store backlinks destinations; hierarchical catalog folders;
Store FTS on a bounded transport thread with exact generation/query/vault/catalog
matching, page size 100 and paging; repeatable create/rename/trash for notes;
confirmation for trash; exact ViewId sessions, pins, close others and 32-entry
reopen history; per-Application 128-note MRU; command Insert/Normal, 128-entry /
256 KiB history, registry/path/current-document-heading/cached-vault suggestions.

The public Registry has no extension API (`from_specs` is private) and
`KeymapConfig::compile` always uses the default registry. Additional stable IDs
for Mixed/Vaults/Recent providers and UI view actions cannot be registered from
this crate. Therefore `Space /`, `Space v`, `Space r` are pending Core IDs; these
providers are currently reachable with Tab/Shift-Tab or the selector. `Space c`
uses the existing `style.text_color` ID; Commands remain on `Ctrl-p`. Visual
`Space l` uses `note.link_selection`. The previous `Ctrl-w w` native cycle alias
is retained, while directional bindings use existing `focus.*` IDs.

Cross-crate requests:
1. Core: validated public registry extensions and keymap compilation against that
   registry, with provider IDs, view actions and typed command argument metadata;
   a public canonical colon resolver for all aliases. This also enables tag
   completion, command argument diagnostics and registry-discovered UI commands.
2. Store: search through CatalogWorker or an explicit read-only search connection,
   query scope, cancellation of a running query, dirty-text overlays and bounded
   snippet/title response bytes. Current UI only calls public IndexDatabase APIs
   on its transport thread; it contains no SQL or search engine. Cancellation
   coalesces queued jobs and rejects old results; it does not interrupt SQLite.
3. Store/Core: directory operation acknowledgements listing affected path mappings
   and buffers, descendant dirty checks, atomic batch plans, empty-directory rows,
   and worker CreateFolder / Create-with-content. Directory rename/trash is disabled
   in this interface: current plans check only the exact dirty path and Core only
   remaps the exact registered buffer path. No independent directory catalog is
   synthesized. Folder creation retains the existing UI API; new notes use the
   Store Create plan and begin empty rather than writing a template in UI.
4. Store: bounded preview-read job, title-bearing catalog rows, modification/open
   timestamps and per-View session presentation metadata. Files preview metadata;
   resident session and FTS previews do not imply arbitrary file preview. Recent
   lists visits in this open Application; persistent Note Stream remains P4.
5. Core: board semantics/facts for actual board providers; current Boards provider
   lists `.kanban.md` files and Tasks lists Core snapshot tasks in the active note.
   P3 rendering/styles and P5 board/graph surfaces are not claimed by P2.

These boundaries are explicit implementation limits, not permission requests.
No other crate, root config or root documentation is edited by this phase.

### P2 completion and verification — 2026-10-15

UI-P0 and UI-P1 remain complete. UI-P2 implementation and verification are
complete within the public APIs listed above; the cross-crate requests remain
open. UI-P3, UI-P4 and UI-P5 are not started by this phase.

The final code uses focused owners: Shell registry/help/command state,
Navigator model/actions/rendering, native providers/search transport/dialogs,
and Application selection/view/file-operation adapters. The full P2 file delta
against the P1 snapshot is in `evidence/p2-after/source-delta.json`; previous
P0/P1 and concurrent Core edits are excluded from that comparison.

Regressions cover remapped leader execution and matching hints, palette edits
and undo, literal field input, modal Copy/Cut ownership, opening/closing overlays
mid-event, Text→Escape→motion and Paste→Enter ordering, provider switching,
collapsed trees and folder ancestry, directory-operation rejection before a
dialog, exact ViewId activation for two views of one buffer, pinned views and
reopening, captured Visual selection transfer and revision rejection, Setext
Outline and fenced-code exclusion, typed task state/line navigation, and real
Store body search with rapidly replaced queries. The real-vault GUI regression
also verifies rename on disk, incoming-link rewriting, buffer path updates,
Navigator query retention, trash cancellation and explicit confirmation.

Final workspace checks all exited 0; logs and exact commands are under
`evidence/p2-after/`:

- `rtk cargo fmt --all -- --check` — passed.
- `rtk cargo check --workspace --all-targets` — passed.
- `rtk cargo test --workspace` — **334 passed, 59 suites**.
- `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings` — passed.

Independent review used **gpt-6-luna**, read-only, against the P1 source snapshot.
It found lost folder depth and sidebar dialogs offered for unsupported directory
operations. Both were reproduced by failing regressions and fixed; the reviewer
ran the 19-test Navigator suite successfully. Subsequent review covered the
dialog sizing-pass focus correction and typed task presentation. Final review
reported no confirmed Critical or Important findings. The main agent ran the
full checks after those changes; the reviewer did not rerun the last two fixes.

Native evidence: **180 PNGs**, Nord and Light, at **800×600, 1280×720 and
1920×1080**. They cover all 13 providers, Help, keyboard scroll, actual FTS
page 2, session pins, command completion/Normal, Visual color picker, file
operation dialogs above a still-open Navigator, and repeated cancellation.
Nord additionally covers Sidebar, Settings, Tab focus and focus mode. The six
Tasks screenshots were refreshed after replacing heading presentation with
typed checkbox rows. `tools/p0_screenshots.py --p2` uses an isolated Xvfb and
private config/state/vault; it does not manipulate the desktop. Screenshot
inspection is supplementary to event/layout tests, not a complete pixel diff.

CPU frame comparison reused `examples/p0_scroll_probe.rs`, identical Core/Store
sources, release builds, five alternating before/after trials, five warm-up
frames and 500 measured frames per scenario. Values below are medians of trial
percentiles. Raw CSVs and `perf-summary.csv` are saved in the evidence directory.
An earlier run overlapped native capture; its noisy results are retained under
`perf-with-gui/` and are excluded from this comparison.

| Scenario | Before p50/p95 ms | P2 p50/p95 ms | Max retained before/P2 |
| --- | --- | --- | --- |
| Navigator, 100 rows | 0.075 / 0.097 | 0.078 / 0.094 | 11 / 11 |
| Navigator, 1,000 rows | 0.076 / 0.105 | 0.077 / 0.097 | 11 / 11 |
| Navigator, 10,000 rows | 0.076 / 0.098 | 0.077 / 0.099 | 11 / 11 |
| Editor, 2,000 lines | 0.047 / 0.138 | 0.046 / 0.136 | 27 / 27 |

There is no meaningful measured change in this scrolling workload. Retained
counts are painted row labels / cached paragraphs, not bytes or RSS.
The saved binary hashes identify the measured revisions; the final Task-row
presentation correction is outside these file-list/editor probe scenarios.

`examples/p2_search_probe.rs` measures the production asynchronous Navigator
adapter on real temporary vaults. It warms up five queries, measures 30 more,
and verifies every returned snippet belongs to the latest body-only query and
each page has at most 100 items. Results from this single run:

| Notes | Request-to-page p50/p95 ms | Pending/accepting frame p95 ms | Max page rows |
| --- | --- | --- | --- |
| 100 | 1.953 / 2.173 | 0.746 | 10 |
| 1,000 | 1.726 / 1.887 | 0.536 | 100 |
| 10,000 | 1.984 / 3.220 | 0.667 | 100 |

The search measurement includes the Store thread, UI adoption and a 1 ms polling
interval after catalog readiness. It excludes indexing/startup, cold-cache
queries, physical input, GPU presentation, dirty overlays and scoped search;
it is not a before/after comparison or hardware input-to-paint latency proof.

Owned native process memory from `/proc/<pid>/status` and `smaps_rollup` is saved
beside each theme's captures. Across the primary sequences, Nord RSS was
**203.6–213.3 MiB**, PSS **114.3–120.7 MiB**; Light RSS was **200.1–208.9 MiB**,
PSS **110.8–116.4 MiB**. These are whole debug eframe-process observations on Xvfb,
including graphics/libraries, rather than UI-only memory or an incremental P2
cost. No native total-memory budget or hardware/HiDPI/trackpad claim is made.

Further limits: SQLite queries already running cannot be interrupted through
the current public API; Store can allocate title/snippet strings before the
worker truncates them to 256/4096 UTF-8 bytes for GUI adoption. Resident-buffer
session preview is bounded at 4096 bytes. Reopened cursor offsets are clamped
to current grapheme boundaries, but closed views do not receive live offset
rebasing while another view edits the shared buffer. Restart persistence of
pins/recent/closed views, arbitrary file previews, typed/tag completion, new
provider key IDs, directory operations and full board semantics require the
cross-crate work above. Unicode fallback/font geometry remains UI-P3 work.

## Phase UI-P3: Editor & Core Synergy, Smart Edit, Projection & Styling

### 1. Scope and Implementation Summary
Phase UI-P3 establishes complete bidirectional synergy between `truff-ui` and `truff-core` Markdown and Vim engine capabilities:

1. **Unicode Grapheme Cluster Geometry & Caret Sizing**:
   - Replaced legacy hardcoded 8.5px empty-line dimension and fixed 0.75 row-height cropping with dynamically resolved font metric bounds.
   - Accurately measures multi-codepoint Unicode grapheme clusters (`W`, `i`, `Ж`, `界`, `👩‍💻` ZWJ emoji sequence, `e\u{0301}` combining accent).
   - Uniform row vertical bounds across mixed scripts, verified by `test_unicode_grapheme_selection_and_heading_geometry`.

2. **Smart Edit Core Integration**:
   - Routed `Enter`, `Tab`, and `ShiftTab` during Insert mode directly to `truff_core::markdown::{plan_enter, plan_tab, plan_shift_tab}`.
   - Supports bullet lists, task lists, ordered lists, and block quotes with atomic Core transaction dispatch.
   - Preserves single-step Undo semantics across all list mutations (`test_smart_edit_enter_tab_shift_tab_and_undo`).

3. **Flash AST Navigation & Key Routing**:
   - `s` in Editor Normal: triggers word jump over the visible viewport via `FlashKind::JumpWord`.
   - `S` in Editor Normal: targets structured AST syntax objects via Core `collect_flash_targets(..., FlashMode::SyntaxObject)` using `MarkdownSnapshot::from_buffer`.
   - Verified by `test_flash_syntax_object_targets_ast_nodes_and_shortcuts` and `flash_modes_select_canonical_objects_and_cancel_operator`.

4. **Expanded Color & Style Palette (`Space c` / `NavigatorMode::Colors`)**:
   - Interactive palette supporting:
     - Foreground text colors (e.g. `<span style="color:#BF616A">`).
     - Background text highlights (`<mark style="background-color:#EBCB8B">`).
     - Obsidian callouts (`wrap_callout` for `[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, `[!CAUTION]`).
     - Style removal (`remove_style`) stripping markdown and HTML styling wrappers.
   - Applied via single atomic Core transaction with instant single-step Undo (`test_color_palette_expanded_style_highlight_callout_remove_and_undo`).

5. **Markdown Projection Modes & Inline Typography**:
   - Added `ProjectionMode::Reading` alongside `Source` and `LivePreview`. In `Reading` mode, all markdown markers (bold, italic, strikethrough, links, lists, color spans, highlights) are unconditionally hidden regardless of cursor position.
   - Added `:reading`, `:preview`, and `:source` command-line actions in `Application`.
   - Heading hierarchy H1–H6 resolved across 4 shared size tiers: H1 (1.6x), H2 (1.35x), H3/H4 (1.15x), H5/H6/body (1.0x).
   - Integrated `LayoutJob` inline styling: bold, italic, strikethrough, monospace code spans with padding background, underlined clickable links, text color spans, and semi-transparent highlight backgrounds.
   - Verified by `reading_mode_unconditionally_hides_markers`, `styled_spans_in_display_map`, and `test_native_heading_hierarchy_and_reading_mode_and_inline_paint`.

### 2. Verification Evidence
All workspace and crate verification gates executed cleanly:
- `cargo fmt --all -- --check` -> PASS (clean formatting)
- `cargo check -p truff-ui --all-targets` -> PASS (clean compile)
- `cargo clippy -p truff-ui --all-targets --all-features -- -D warnings` -> PASS (0 warnings)
- `cargo test -p truff-ui` -> PASS (all unit, integration, and UI tests pass)

## Phase UI-P4: Advanced Navigation, Structure & Feed (Floating TOC, Folder Wiki, Note Stream)

### 1. Scope and Implementation Summary
Phase UI-P4 implements Section 6 of `03_UI_AGENT_PROMPT.md` inside `crates/ui` without modifying Core or Store crates:

1. **Floating TOC (`crates/ui/src/native/toc/`)**:
   - **Active Heading Auto-Sync**: Bidirectional synchronization between editor caret / visible viewport line and the TOC active heading indicator.
   - **Hierarchical Collapse & Expand**: Full tree hierarchy (`H1 > H2 > H3...`), with fold state toggling via `h` (collapse / jump to parent), `l` (expand / jump to child), or click.
   - **Sticky Parent Breadcrumb Chain**: Displays the active ancestral path (`Parent H1 > Subsection H2 > Item H3`) at the top of the TOC for rapid orientation.
   - **Dual Placement Mode**: Operates both docked in the right side panel and as a floating overlay window with pin toggle (`p`). Pinned floating windows stay on top across workspace navigation.
   - **Scope Switching**: Supports `ActiveFile` (current note headings), `Folder` (all markdown files and headings in the current directory), and `Vault` (entire vault outline hierarchy).
   - **UI-P0 Scroll Controller Integration**: Integrated with `EnsureVisible` scroll controller for keyboard navigation (`j/k`, `Enter` to jump heading, `o` to open adjacent note).
   - **Search Filter**: Real-time fuzzy query filtering (`/`), with direct `Esc` handling to restore or close.

2. **Folder Wiki (`crates/ui/src/native/wiki/`)**:
   - **Master Note Detection & Creation**: Automatically discovers folder entry note (`index.md` or folder-named `.md`); offers `Create index.md` action if absent.
   - **Notes Tree & Headings**: Structured breakdown of all folder documents with nested headings and line counts.
   - **Aggregated Tasks & Ideas**: Aggregates `- [ ]` and `- [x]` markdown check items across all folder notes into a unified action list.
   - **Obsidian Bases Metadata Table**: Displays file metrics (Note name, Line count, Byte size, Task progress ratio, Status).
   - **Backlinks & Recent Changes**: Contextual backlinks referencing notes within the directory and recent file modification timestamps.
   - **Direct Tab Switching**: Tab navigation via `1..=5` (`1: Overview`, `2: Notes Tree`, `3: Tasks`, `4: Backlinks`, `5: Recent`) and `Tab`/`Shift+Tab`.
   - **Action Shortcuts**: `a` to create a new note directly within the folder, `Enter` to open selected note/task, `Esc` to close.

3. **Note Stream (`crates/ui/src/native/stream/`)**:
   - **Continuous Note Feed**: Virtualized stream cards representing vault markdown files with lazy snippet extraction.
   - **Bounded Memory Without Monolithic Strings**: Extracts snippets dynamically without concatenating all vault notes into a massive string.
   - **Filter Modes**:
     - `1: Today` (`file_path` contains date or modified today).
     - `2: Unfinished` (notes with pending tasks `- [ ]`).
     - `3: Related` (notes with bi-directional wikilinks to current active buffer).
     - `4: Revisit` (notes inactive for >7 days).
     - `5: All` (full vault feed).
   - **Why-Shown Explanation Badges**: Explanatory tags on each card ("Сегодня", "N незаверш. задач", "Связана с текущей", "Не открывалась более недели").
   - **Keyboard Navigation & Actions**: `j/k` motion, `1..=5` / `Tab/ShiftTab` filter toggling, `a` to create note, `Enter` to open selected card in editor.

4. **GUI Overlay Dispatch & Command Bar Routing (`crates/ui/src/native/p4_actions.rs`, `crates/ui/src/native/events.rs`)**:
   - Integrated command bar commands `:toc`, `:wiki [folder]`, and `:stream`.
   - Leader key shortcuts: `Space t` (Floating TOC), `Space w` (Folder Wiki), `Space s` (Note Stream).
   - Registered in WhichKey overlays under Navigation and Tools categories.
   - Safe input isolation: underlying editor input is blocked while an overlay is active (`is_input_blocked()`), and `Esc` cleanly restores editor focus.

### 2. Verification Evidence
- Full test suite execution: `cargo test -p truff-ui` -> PASS (all tests pass, 0 failures, 10 dedicated P4 integration tests).
- Linter clean: `cargo clippy -p truff-ui --all-targets --all-features -- -D warnings` -> PASS (0 warnings).
- Formatting verified: `cargo fmt -p truff-ui -- --check` -> PASS (clean code style).


