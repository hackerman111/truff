# Keyboard Navigation, Note Stream and Kanban Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Default execution is inline in this workspace; delegation needs the user's explicit choice.

**Goal:** Управлять Truff без мыши, быстро искать заметки, читать отдельные Markdown-файлы общей лентой и вести Markdown-kanban.

**Architecture:** Расширить существующие VimState/CommandRegistry/Buffer/RowModel. Core владеет правилами и диапазонами исходника, store — индексом и восстановимыми операциями, ui — фокусом и представлениями. Search выполняется в catalog worker с latest-query mailbox; SQLite остаётся производным индексом.

**Tech Stack:** Rust 2024, crop Rope, pulldown-cmark, Tree-sitter, SQLite/FTS5, serde/TOML, egui/eframe. Новые зависимости по умолчанию не нужны; сохранить Cargo.lock.

**Spec:** [Согласованная концепция и дополнение скорости](../../plans/2026-10-12-keyboard-navigation-note-stream-and-kanban-design.md).

## Global Constraints

- Без GPUI; текущий egui-frontend сохраняется.
- Новая заметка — отдельный Markdown-файл; лента является представлением.
- Все текстовые правки — Buffer::apply / Transaction с общим undo/recovery.
- Counts/operators/registers/macros остаются у VimParser; UI не заводит второй parser.
- Search page ≤100, table limit 1..1000, reading cache ≤8 MiB, debounce 35 ms.
- Catalog indexing slice ≤16 файлов; jobs queue 8 и replies queue 16 сохраняют bounded backpressure.
- Search p95 на 10k×4 KiB: metadata ≤50 ms, combined ≤150 ms, during indexing ≤250 ms; граница включает debounce/queue, rendering измеряется отдельно.
- Память/latency оцениваются release-замерами на том же corpus/компьютере; headless не доказывает GUI presentation.
- Сохранять существующие незакоммиченные правки. Commit только своих файлов/частей diff; при смешанном файле staging проверяется по hunks.
- Корни модулей компонуют API; domain rules не переходят в ui/native или app/mod.rs.

## Review Focus

1. Старый search/Follow/Backlinks после закрытия picker, смены vault или revision — не меняет новый фокус/текст (T04, T07).
2. Одинаковые заголовки/имена, кириллица, collapsed ancestors — поиск не теряет или подменяет цель (T05, T06).
3. Dirty-документ изменяет совпадение/порядок страниц — дисковая версия исключена до сортировки и пагинации (T05, T10, T11).
4. Rename/trash папки с dirty потомком и crash — чужой текст сохранён, ссылки/пути восстанавливаются (T08).
5. Kanban с вложенными tasks, CRLF и обычными блоками — move/delete не теряют исходный Markdown (T12, T13).

## Порядок и проверяемые этапы

T01–T02: общее управление. T03–T07: быстрый поиск/picker/ссылки.
T08–T11: папки/таблицы/лента. T12–T14: kanban и итоговая проверка.
Каждый этап даёт рабочую функцию; вся реализация заканчивается только после T14.
Изолированные части не выполняются одновременно с изменением их интерфейсов.

Во всех задачах тест сначала запускается и должен упасть по новой проверке
или отсутствующему API, затем пишется реализация и повторяется точная команда.
Непредвиденная старая ошибка фиксируется отдельно, без её маскировки.

### T01. Единая маршрутизация команд и недостающие motions

**Files:** modify `crates/core/src/commands/{types,defaults,registry}.rs`, `crates/core/src/config/{defaults,compiled}.rs`, `crates/core/src/navigation/{mod,state}.rs`, `crates/core/src/vim/parser/mod.rs`, `crates/core/src/vim/motion/{types,word,cursor}.rs`, `crates/ui/src/shell/{mod,routing,panels}.rs`, `crates/ui/src/native/{events,backlinks}.rs`, `crates/ui/src/app/{mod,navigation}.rs`; create `crates/core/src/navigation/target.rs`, `crates/ui/src/shell/focus.rs`; tests `crates/core/tests/vim_routing.rs`, `crates/ui/tests/{vim_routing,navigation,which_key}.rs`.

**Interfaces:** add core `RowTarget::{Path(PathBuf), Heading { path: PathBuf, ordinal: usize }, Backlink { path: PathBuf, byte: usize }, Vault(PathBuf), View(ViewId), Command(CommandId)}` in `navigation/target.rs`; add `RowItem::target: Option<RowTarget>`, migrate all initializers. Focus routing resolves to one RowModel; `Shell::activate_selected() -> ShellAction` emits typed target. `Application::activate_target(&RowTarget) -> AppResult<()>` is canonical activation. Add registry IDs and default bindings for every spec table; put field/board-only command contexts in core commands.

- [ ] Add `reverse_search_is_not_help`, asserting `?two<Enter>` finds previous occurrence and `<Space>?` opens help.
- [ ] Add `default_and_remapped_counts_match`, asserting `3j` and remapped `3<key>` reach the same text/row destination; test pending `dfx`, registers/macros and literal Space in Insert.
- [ ] Run `rtk cargo test --offline --locked -p truff-ui --test vim_routing` and corresponding core test; confirm new assertions fail.
- [ ] Implement W/B/E, D/C/Y and viewport half-page motion using existing operator execution. Add a typed viewport-move result for native resolution; counts are preserved.
- [ ] Move pane navigation/which-key to registry dispatch; remove native backlinks j/k/Enter interception and `backlink_selection`. Keep source-byte mapping for visual motions.
- [ ] Run focused tests above and `rtk cargo check --offline --locked --workspace --all-targets`; update existing row fixtures. Done: one selection owner, registry reflects executable bindings, no text prefix is swallowed.

### T02. Vim editing for all fields

**Files:** create `crates/ui/src/editor/field.rs`, `crates/ui/src/native/fields.rs`; modify `editor/mod.rs`, `native/events.rs`, `native/creation/{file,folder,vault,settings}.rs`, `shell/routing.rs`; tests create `crates/ui/tests/vim_fields.rs`, modify `settings_dialog.rs`, `path_completion.rs`.

**Interfaces:** `VimField::new(text: &str, max_bytes: usize) -> Result<Self, EditError>` owns existing Buffer/View/VimState; `feed_key(Key) -> FieldResult`, `apply_input(EditorInput) -> Result<(), InputError>`, `text() -> String`; `FieldResult::{Changed, Submitted, Cancelled, ModeChanged, Ignored, Error(String)}`. Add `enter_insert()` and `mode() -> Mode`. Native adapter `show_field(ui: &mut egui::Ui, field: &mut VimField) -> Option<FieldResult>` only handles presentation and input.

- [ ] Add `field_change_undo_and_ime`: `ciw`, mixed IME/paste, Esc, u restore original text in one undo; main editor revision unchanged.
- [ ] Add `field_escape_submit_and_limit`: first Esc → Normal, second → Cancelled, Enter → Submitted; over-limit paste leaves text unchanged; test CRLF/newline rejection in single-line fields.
- [ ] Run `rtk cargo test --offline --locked -p truff-ui --test vim_fields`; confirm fail, implement field through InputAdapter/VimState, then repeat to PASS.
- [ ] Replace editable dialog fields and command prompt input with the adapter. Preserve validation, completion and external links; focus ownership prevents duplicate egui Text/Key processing.
- [ ] Run existing dialog/input tests. Done: paths/titles/search/command fields share actual motions and undo, with visible mode and Esc hints.

### T03. Persistent incremental catalog and search baseline

**Files:** modify `crates/store/src/catalog/{mod,scan,jobs,protocol}.rs`, `index/{mod,update,schema.sql}.rs`; create `catalog/runtime.rs`, `index/catalog.rs`, `crates/store/examples/search_bench.rs`, `tools/perf/search_corpus.py`; tests `crates/store/tests/catalog.rs`, create `crates/store/tests/catalog_incremental.rs`.

**Interfaces:** `CatalogSnapshot { generation: u64, entries: Vec<CatalogEntry> }`; `CatalogEntry { file_id: Option<FileId>, path: PathBuf, kind: EntryKind, title: String, modified: Option<u64> }`; `EntryKind::{Note, Folder}` defined in store/catalog/protocol.rs. Replace Files event with snapshot and migrate UI/tests. `ReconcileState::step(&mut self, db: &mut IndexDatabase, paths: &VaultPaths, max_files: usize) -> Result<bool, IndexError>` performs an indexing slice; filesystem errors carry context in IndexError. Stable IDs allocated from DB, never enumeration ordinals.

- [ ] Write corpus generator with seed 12, 10k files of 4 KiB, Russian/English H1 plus 8 headings, duplicate titles, empty/deep folders and links; checksum manifest freezes inputs.
- [ ] Add release baseline example using current IndexDatabase::search before changing the index; run `rtk cargo run --offline --locked --release -p truff-store --example search_bench -- --baseline /tmp/truff-search-baseline.json`. Record indexing/query times; label this headless and exclude corpus construction from query timers.
- [ ] Add `restart_reuses_ids_and_unchanged_facts`, `single_create_does_not_reindex_vault`, `empty_folders_are_catalog_entries`; assert IDs stable, only changed-file counter grows, folder survives with no md child.
- [ ] Run `rtk cargo test --offline --locked -p truff-store --test catalog_incremental`; confirm new failures.
- [ ] Replace delete-all rebuild with metadata reconcile: register new destinations, update changed files, remove missing entries, refresh affected resolutions. Persist stat/fingerprint fields; full explicit reindex remains available for missing/corrupt derived index. Migrate FTS row identity to `rowid = files.id`; delete/update by rowid rather than scanning UNINDEXED file_id for each document. Rebuild derived FTS once for schema migration, preserving persistent file IDs.
- [ ] Slice startup scan/index and service jobs between slices; watcher updates affected paths, not full vault. Preserve recovery-before-index and shutdown draining.
- [ ] Repeat tests plus `rtk cargo test --offline --locked -p truff-store --test catalog`. Done: startup and every create/rename no longer delete/rebuild all facts; empty folders included.

### T04. Latest-query scheduling and result freshness

**Files:** create `crates/core/src/navigation/search.rs`, `crates/store/src/catalog/search.rs`; modify navigation/mod.rs, catalog/{mod,runtime,protocol,jobs}.rs, ui/app/io/catalog_events.rs; tests `catalog_incremental.rs`, create `crates/store/tests/search_worker.rs`.

**Interfaces:** core `QueryGeneration(u64)` and `SearchKey { epoch: VaultEpoch, picker_id: u64, generation: QueryGeneration }`; `SearchRequest { key: SearchKey, text: String, folder: PathBuf, page: usize, kind: SearchKind }`, `SearchKind::{Metadata, Combined}`. Add `CatalogWorker::request_search(SearchRequest) -> Result<(), SubmitError>` using one latest-request mailbox and wakeup. Durable CatalogJob queue stays FIFO; search cancellation cannot discard a file operation.

- [ ] Add `latest_query_wins_during_indexing`, asserting query generations 1..20 coalesce to latest, response key matches, query is serviced before Ready for whole vault.
- [ ] Add `shutdown_with_full_results_queue_finishes` and `search_does_not_cancel_accepted_rename`, with bounded deterministic timeouts and no busy loop.
- [ ] Run `rtk cargo test --offline --locked -p truff-store --test search_worker`; confirm fail, then implement mailbox, cancellation token and 16-file slicing.
- [ ] Make replies freshness checked by exact SearchKey plus catalog/overlay generation. A retired picker rejects every result. SQLite cancellation uses its supported progress/interrupt API; enable only required rusqlite feature if currently unavailable, preserving version/lockfile.
- [ ] Repeat tests; Done: queue holds at most one pending search, background jobs have owned cancellation/shutdown, UI submission never blocks.

### T05. Bounded fuzzy/FTS search and dirty overlay

**Files:** create `crates/core/src/navigation/matching.rs`, `crates/store/src/index/search/{metadata,overlay}.rs`; modify `navigation/mod.rs`, `store/index/search.rs`, `index/{mod,update}.rs`, catalog/search.rs; tests create `crates/core/tests/matching.rs`, `crates/store/tests/search.rs`; migrate `store/tests/index.rs`.

**Interfaces:** `NormalizedText::new(&str) -> Self`, `score_match(query: &NormalizedText, candidate: &NormalizedText) -> Option<MatchScore>`; ordering exact → prefix → subsequence, ties path/anchor. `SearchHit { path: String, title: String, anchor: Option<String>, snippet: String, score: MatchScore }`; `SearchReply { key: SearchKey, index_generation: u64, hits: Vec<SearchHit>, has_more: bool }` in catalog/protocol.rs. Core MatchScore has total deterministic order. `SearchPage` migrates from total_matches to has_more; one canonical FTS executor. `IndexDatabase::update_search_overlay(path: &Path, key: SnapshotKey, rope: crop::Rope) -> Result<(), IndexError>` uses revision-qualified TEMP rows and excludes corresponding persistent row.

- [ ] Add `unicode_fuzzy_order_and_duplicate_heading_targets`, asserting Cyrillic lowercase, deterministic ranking, distinct heading anchors and one merged note hit.
- [ ] Add `dirty_removed_match_is_absent_before_pagination`, asserting stale disk match absent, dirty new match present, exactly page_size rows, no duplicate path target across pages.
- [ ] Add `punctuation_fts_input_is_literal`, testing quotes, FTS operators, empty query, prefix and excessive count/page arithmetic without panic.
- [ ] Run core matching and store search tests; confirm fail. Pre-normalize catalog fields on update, normalize only query on typing; select top-k with bounded heap instead of sorting every match.
- [ ] Query FTS LIMIT 101 for first 100, avoid COUNT; dirty TEMP index merges before ordering. Update overlay once per changed revision, never transfer all dirty bodies as Strings on every query. Borrow candidate strings and clone only returned page.
- [ ] Stable pagination binds catalog/overlay generations: if changed, invalidate and restart at page zero. Add counters ensuring repeated selection/query performs no file reads or facts extraction.
- [ ] Repeat tests and release example against baseline. Done: bounded deterministic correct results; note performance numbers without interpreting unmeasured GUI cost.

### T06. Unified picker and mixed search UI

**Files:** create `crates/ui/src/app/picker/{mod,state,search,activation}.rs`, `crates/ui/src/native/picker.rs`; modify app/{mod,registered}.rs, native/{mod,events,hints}.rs, shell/focus.rs, core/config/defaults.rs; tests create `crates/ui/tests/picker.rs`, modify native.rs/which_key.rs.

**Interfaces:** `PickerKind::{Notes, Vaults, Buffers, Headings, Commands, Links}`; `PickerMode::{Tree, Search, Mixed}`; `PickerInput::{Query, Navigation}` with VimField supplying Query mode. `PickerState::new(kind: PickerKind, return_focus: Focus) -> Self`, `feed_key(Key) -> PickerResult`, `accept_search(SearchReply) -> bool`; `PickerResult::{Changed, Activate(RowTarget), Close}`. Picker-specific IDs map to typed RowTarget, not command text. Application `open_picker(PickerKind, PickerMode) -> AppResult<()>`.

- [ ] Add `mixed_search_reveals_collapsed_ancestors`, `switch_mode_preserves_query_and_target`, `picker_escape_restores_original_view`, `vault_picker_handles_missing_path`.
- [ ] Add `stale_reply_after_vault_switch_is_ignored` and `picker_text_never_edits_note`; assert all three Esc transitions from Query/Insert and Ctrl-j/k in Query.
- [ ] Run `rtk cargo test --offline --locked -p truff-ui --test picker`; confirm fail, then implement debounce 35 ms with an actual next repaint deadline, exact response key checking and bounded page model.
- [ ] Render one common frame/theme and virtual rows. Mixed mode builds ancestors from current page, preserving ordinary tree collapse state; folder boundary uses Path components (`a` excludes `ab`).
- [ ] Wire ff/fs/fm/vv/bb/pp bindings, creation and recent vaults. Run picker/native/which-key tests. Done: tree/search/mixed/vault reachable and useful without mouse.

### T07. Link hints and note jump history

**Files:** modify `crates/ui/src/{shell/flash.rs,overlays/flash.rs,app/navigation.rs,app/commands/links.rs,native/events.rs}`, core/workspace/views.rs, store/catalog/{protocol,jobs}.rs, store/index/resolve.rs; tests core/markdown.rs and ui/{navigation,native}.rs; create ui/tests/link_navigation.rs.

**Interfaces:** add `FlashKind::Link`; `Application::follow_occurrence(key: SnapshotKey, ordinal: usize) -> AppResult<()>` uses current facts and resolver. Follow/Backlinks jobs/replies receive source SnapshotKey and navigation request generation. `NoteLocation { path: PathBuf, anchor: Option<String>, byte: Option<usize> }` in core/navigation/target.rs; jump history stores locations as well as view-local cursors.

- [ ] Add `visible_link_label_follows_once`, asserting code exclusion, repeated occurrences, dirty revision rejection and viewport-bounded labels.
- [ ] Add `ambiguous_target_opens_candidates`, `jump_back_restores_note_and_cursor`, `late_follow_does_not_steal_focus`; test external http/mailto via injected platform-open recording adapter.
- [ ] Run link tests; confirm fail. Reuse Flash labels and MarkdownFacts; route selected occurrence to canonical resolver, expose ambiguous alternatives instead of an error-only branch.
- [ ] Wire gf, jl, ll, Ctrl-o/i and restore jump history in session. Repeat tests; Done: no separate link parser or unsafely stale source range.

### T08. Recoverable folder operations

**Files:** modify `crates/store/src/operations/{plan,rewrites,commit,recovery}.rs`, catalog/jobs.rs, core/workspace/buffers.rs, ui/app/io/catalog_events.rs, ui/app/commands/files.rs; tests store/operations.rs, create store/tests/folder_operations.rs, ui/navigation.rs.

**Interfaces:** `FileOpPlan::rename/move_path/trash` retain public API, expand manifest to subtree identities/rewrites. `BufferRegistry::rename_subtree(from: &Path, to: &Path) -> Result<(), AppError>` validates all destinations before rebinding; `CatalogEvent::Renamed` carries path mappings for every affected note, not just root.

- [ ] Add `rename_folder_rewrites_links_and_preserves_ids`, `dirty_descendant_blocks_rename_and_trash`, `crash_mid_folder_move_recovers`, `symlink_descendant_is_rejected`.
- [ ] Run `rtk cargo test --offline --locked -p truff-store --test folder_operations`; confirm failure, extend one canonical FileOpPlan with directory staging/undo manifest and prefix-safe path matching.
- [ ] Apply UI buffer rebinding after durable acknowledgement; keep session/recovery paths coherent and preserve shared views. Reindex only affected files using T03.
- [ ] Repeat tests and existing operations test. Done: empty/deep folders work; no partial live identity update before durable success.

### T09. Outline, expanded sections and automatic folder TOC

**Files:** create core/markdown/toc.rs, store/index/outline.rs, ui/app/outline.rs, ui/native/outline.rs, ui/app/commands/toc.rs; modify core/markdown/mod.rs, catalog/protocol.rs/jobs.rs, ui/app/registered.rs; tests create core/tests/toc.rs, ui/tests/outline.rs, store/tests/toc.rs.

**Interfaces:** `TocDocument::parse(text: &str) -> Result<Self, TocError>` validates exactly zero/one managed pair; `replace_block(&self, body: &str) -> Transaction`; `FolderOutlineQuery { folder: PathBuf, include_descendants: bool }` and paged reply in catalog/protocol.rs. Heading ordinal+snapshot identity locates duplicates; excerpt ends at next heading of equal/lower level.

- [ ] Add `toc_update_preserves_user_text_and_is_one_undo`, `duplicate_markers_are_error`, `outline_duplicate_headings_keep_occurrences`, `auto_toc_does_not_overwrite_dirty_or_external_file`.
- [ ] Run toc/outline tests; confirm fail. Build outline from indexed headings plus dirty overlay and lazy section read; never recursively follow arbitrary links.
- [ ] Wire oo/of/oi commands. oi persists enabled folder auto-TOC setting in vault config; refresh on relevant catalog changes, excluding `_index.md` and managed block. Use normal save fingerprint validation.
- [ ] Repeat tests. Done: folder index automatically stays current, optional dirty-buffer replacement is one undoable transaction, no uncontrolled watcher-write cycle.

### T10. Metadata and declarative tables

**Files:** create core/markdown/{metadata,query}.rs, store/index/{metadata,query}.rs, ui/app/query_views.rs, ui/native/query_views.rs; modify core/markdown/facts.rs/mod.rs, store/index/schema.sql/update.rs, ui/editor/display/projection.rs, catalog/protocol.rs/jobs.rs; tests create core/tests/queries.rs, store/tests/queries.rs, ui/tests/query_views.rs.

**Interfaces:** `NoteMetadata { created: Option<u64>, tags: Vec<String>, fields: BTreeMap<String, String> }`; `QuerySpec::parse(&str) -> Result<Self, QueryError>` enforces TOML keys, table/list, allowlisted columns, title substring/exact tag, sort and limit 1..1000. `QueryPage { key: SearchKey, rows: Vec<QueryRow>, has_more: bool }`, `QueryCell::{Text(String), Missing}`; `IndexDatabase::query_notes(&QuerySpec, page: usize) -> Result<QueryPageData, IndexError>` shares metadata dirty overlay.

- [ ] Add `query_rejects_unknown_columns_and_sql_fragments`, `old_note_created_is_missing`, `dirty_query_filter_sort_precedes_pagination`, `query_preview_does_not_modify_source`.
- [ ] Run named suites; confirm fail. Extend canonical facts only for initial truff-meta and truff-query fenced blocks; index fields with versioned derived schema migration/rebuild and bound SQL parameters.
- [ ] Render table/list through shared virtual rows; selection opens typed path target, source mode shows original fence. Snapshot mismatch cancels stale table result.
- [ ] Repeat tests. Done: doc's query example works, errors local to block, no arbitrary SQL/JS and no generated Markdown insertion.

### T11. Note stream, drafts and rediscovery

**Files:** create ui/app/stream/{mod,state,drafts,reading}.rs, ui/native/stream.rs, store/index/stream.rs; modify ui/app/{mod,registered,session}.rs, catalog/{protocol,jobs}.rs, store/index/schema.sql, ui/native/content.rs; tests create ui/tests/stream.rs, store/tests/stream.rs.

**Interfaces:** `StreamOrder::{Newest, LeastRecentlyOpened}`; `StreamState::new(folder: PathBuf, order: StreamOrder) -> Self`; reading cache owns ≤8 MiB and discards clean reading snapshots only. `Application::new_stream_note() -> AppResult<BufferId>` creates pathless draft; `save_stream_note(id: BufferId) -> AppResult<()>` reserves destination by create-new/FileOpPlan then saves and binds existing buffer after durable acknowledgement. Catalog stream page includes typed note targets, title and optional timestamps.

- [ ] Add `new_entry_saves_separate_file`, `two_same_timestamp_creates_never_overwrite`, `unsaved_draft_recovers_after_crash`, `blank_draft_does_not_create_file`.
- [ ] Add `rediscovery_prioritizes_never_opened`, `read_cache_is_bounded_and_keeps_dirty_editor`, `return_from_edit_restores_stream_selection`.
- [ ] Run stream tests; confirm fail. Add last_opened derived table updated on actual opening; metadata created assigned on first save, not regenerated on edits.
- [ ] Wire ns/nn, j/k/gg/G/half-page/Enter/e and folder/search picker. Expanded cards lazily load reading snapshots; editing uses Workspace buffer and existing editor, not concatenation into a second editable document.
- [ ] Repeat tests and recovery/session suites. Done: creation/save/restart and old-note discovery work with bounded reading memory.

### T12. Kanban facts and source-preserving operations

**Files:** create core/markdown/kanban/{mod,facts,editing}.rs; modify core/markdown/mod.rs/facts.rs; test create core/tests/kanban.rs.

**Interfaces:** `KanbanFacts::extract(text: &str) -> Result<Self, KanbanError>` is called by canonical extractor; `ColumnId(usize)`, `CardId(usize)` refer to ordinal within revision; `BoardAction::{Move { card: CardId, column: ColumnId, before: Option<CardId> }, Toggle(CardId), Delete(CardId), Add { column: ColumnId, text: String }, RenameColumn { column: ColumnId, title: String }, MoveColumn { column: ColumnId, before: Option<ColumnId> }, DeleteColumn { column: ColumnId, destination: Option<ColumnId> }}`; `edit_board(buffer: &Buffer, expected_revision: u64, facts: &KanbanFacts, action: BoardAction) -> Result<Transaction, KanbanError>`.

- [ ] Add `move_card_preserves_nested_tasks_links_and_crlf`: apply move, compare whole source to explicit expected Markdown; undo restores byte-for-byte original.
- [ ] Add `plain_blocks_are_not_cards_and_survive_delete`, `stale_revision_rejects_edit`, `column_delete_requires_destination`, `empty_and_last_column_moves_are_safe`.
- [ ] Run `rtk cargo test --offline --locked -p truff-core --test kanban`; confirm fail. Extract H2/immediate task-list source ranges with pulldown-cmark; retain continuation/nested blocks and line endings.
- [ ] Build replacement transactions without reserializing untouched content; validate source/destination before mutation, no-op move produces no spurious history.
- [ ] Repeat tests. Done: one revision-qualified edit path supports add/move/order/toggle/delete and column operations safely.

### T13. Kanban UI and keyboard actions

**Files:** create ui/app/kanban.rs, ui/native/kanban.rs; modify ui/app/{mod,registered}.rs, native/{content,events}.rs, shell/focus.rs, core/commands/defaults.rs/config/defaults.rs; tests create ui/tests/kanban.rs, modify native.rs.

**Interfaces:** `KanbanState { buffer_id: BufferId, revision: u64, column: ColumnId, card: Option<CardId> }`; `Application::open_kanban() -> AppResult<()>`, `board_action(BoardAction) -> AppResult<()>` runs T12 and dispatches existing ApplyEdit. Field editing commits into a single Transaction; no independent card database.

- [ ] Add `keyboard_create_move_toggle_undo_and_restart` with spec kk/kn/h/l/j/k/H/L/J/K/o/e/x/dd/u/Ctrl-r; assert canonical buffer and saved file agree.
- [ ] Add `switch_board_source_preview_keeps_text`, `board_external_conflict_blocks_save`, `virtual_board_does_not_shape_all_cards` for 2k cards.
- [ ] Run `rtk cargo test --offline --locked -p truff-ui --test kanban`; confirm fail. Implement virtual columns/cards, typed board context bindings/which-key, column management picker and links.
- [ ] Repeat tests; Done: Markdown board editable/readable without mouse, source and board share dirty/undo/recovery.

### T14. Full integration and measured search acceptance

**Files:** modify README.md, tools/gui/native_smoke.py; create tools/gui/keyboard_workflow.py, tools/perf/search_latency.py, docs/perf/search/summary.md; extend store/examples/search_bench.rs and ui/tests/e2e_workflow.rs.

- [ ] Add keyboard-only end-to-end scenario: vault/picker → folder/new note → Vim save → fuzzy mixed search → link hints/history → TOC/table → stream create/revisit → kanban save/restart. GUI test owns temporary vault/process, never sends input to unrelated windows.
- [ ] Run `rtk cargo fmt --all -- --check`, `rtk cargo check --offline --locked --workspace --all-targets`, `rtk cargo test --offline --locked --workspace`, `rtk cargo clippy --offline --locked --workspace --all-targets --all-features -- -D warnings`; all must exit 0, preserve lockfile.
- [ ] Run `rtk cargo build --offline --locked --release -p truff-ui` and `rtk proxy python3 tools/gui/keyboard_workflow.py --binary target/release/truff`; require saved-file assertions plus inspected screenshots/focus outcomes. Record any unavailable native check as unverified.
- [ ] Run release search benchmark on frozen 10k corpus: ≥1000 queries per class, warmups excluded, empty/one-char/common/rare/Russian/heading/folder/no-match cases, warm and startup indexing separately. Compare result fingerprints before timings; no weakening of validation/query semantics for speed.
- [ ] Run `rtk proxy python3 tools/perf/search_latency.py --binary target/release/truff --files 10000 --output /tmp/truff-search-latency.json`. Native harness timestamps input/change/result application/frame separately, reports p50/p95/max and RSS/PSS. Debounce 35 ms and queue time are included in input-to-result budget.
- [ ] If ≥50/150/250 ms p95 budgets fail, profile the failing stage, change one bottleneck, rerun identical paired workload. Investigate ≥1% regression relative to observed variance; preserve top-k quality and dirty overlay correctness.
- [ ] Repeat 100k stress case and 10 MiB single-file concurrent indexing; document worst-case/memory, not a universal latency promise. Check that selections read no files and re-extract no facts, mailbox/reading/row caches remain bounded.
- [ ] Inspect complete diff for user-change preservation, duplicate paths, unbounded state, avoidable clones on hot paths, and temporary diagnostics. Update README with actual stack/keys/examples and measured report; no claims based on obsolete Masonry/headless reports.

## Plan review checklist

- [x] Spec §1 → T01/T02; §2/2.1 → T03–T06/T14; §3 → T07.
- [x] Spec §4 → T08/T09; §5 → T10; §6 → T11; §7 → T12/T13.
- [x] Review Focus failure classes have explicit test names in owning tasks.
- [x] Producers/consumers use the same RowTarget/SearchKey/QuerySpec/BoardAction names and canonical owners.
- [x] Plan explicitly preserves user modifications and removes full-index rebuild from common create/rename/search paths.
- [x] T14 requires actual workload/results/verification boundary; feature tasks include GUI entry points for all requested capabilities.

План подготовлен для review. Код и performance-результаты будут добавлены при исполнении;
текущие бюджеты являются условиями приёмки, а не выполненными замерами.
