# Kanban Panel Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Build an aesthetic, Markdown-backed Kanban panel supporting both a `Space b` / `:kanban` modal dashboard and a native central Board/Source view for `*.kanban.md` / `<!-- truff:kanban -->` notes, with fast file-linked task chips (`f`, `[[...]]` autocomplete, `Enter`/`gf` jump) and full Vim motions.

**Architecture:** `truff-core::markdown::kanban` owns pure Markdown parsing of `##` columns, task cards (`- [ ]`, `- [/]`, `- [x]`), inline `[[links]]` / `#tags`, and range-based `Transaction` builders so surrounding Markdown and CRLF line endings stay untouched and `Buffer` undo/redo works out of the box. `truff-ui::native::kanban` owns the Vim state machine, fuzzy file-link picker, shared Nord/Obsidian/Light renderer, and dual integration into `NativeApp` (`Space b` overlay + `CentralPanel` board view).

**Tech Stack:** Rust, `truff-core` (`Buffer`, `Transaction`, `SingleEdit`, `UndoGroup`, `LinkOccurrence`), `truff-store` (`compute_fingerprint`), `truff-ui` (`egui`, `Theme`, `Application`, `Shell`, `NativeApp`).

---

### Task 1: Core Kanban Parser & Board Detection (`truff_core::markdown::kanban`)

**Files:**
- Create: `crates/core/src/markdown/kanban/mod.rs`
- Create: `crates/core/src/markdown/kanban/parser.rs`
- Modify: `crates/core/src/markdown/mod.rs`
- Create: `crates/core/tests/kanban.rs`

**Step 1: Write the failing unit tests for board detection and Markdown parsing**

```rust
// crates/core/tests/kanban.rs
use truff_core::markdown::kanban::{
    KanbanBoard, KanbanTaskStatus, is_kanban_document,
};

#[test]
fn detects_kanban_by_path_or_marker() {
    assert!(is_kanban_document(Some("roadmap.kanban.md"), "# Roadmap\n"));
    assert!(is_kanban_document(Some("kanban.md"), ""));
    assert!(is_kanban_document(Some("projects/kanban.md"), "# Sprint\n"));
    assert!(is_kanban_document(
        Some("notes/plan.md"),
        "<!-- truff:kanban -->\n# Plan\n"
    ));
    assert!(!is_kanban_document(Some("notes/daily.md"), "## Todo\n- [ ] Milk\n"));
}

#[test]
fn parses_columns_cards_links_tags_and_continuations() {
    let md = "\
<!-- truff:kanban -->
# Проект

Вводный абзац, который нельзя потерять.

## План
- [ ] Сделать парсер [[core/parser|Парсер]] #core #urgent
  Подробности реализации
- [/] Проверить граничные случаи [[docs/spec]]

## Готово
- [x] Согласовать дизайн
";
    let board = KanbanBoard::parse(md);
    assert_eq!(board.title.as_deref(), Some("Проект"));
    assert_eq!(board.columns.len(), 2);
    assert_eq!(board.columns[0].title, "План");
    assert_eq!(board.columns[0].cards.len(), 2);

    let c0 = &board.columns[0].cards[0];
    assert_eq!(c0.status, KanbanTaskStatus::Todo);
    assert_eq!(c0.clean_title, "Сделать парсер");
    assert_eq!(c0.links.len(), 1);
    assert_eq!(c0.links[0].target, "core/parser");
    assert_eq!(c0.links[0].display(), "Парсер");
    assert_eq!(c0.tags, vec!["core", "urgent"]);
    assert_eq!(c0.body_lines, vec!["Подробности реализации"]);

    let c1 = &board.columns[0].cards[1];
    assert_eq!(c1.status, KanbanTaskStatus::InProgress);
    assert_eq!(c1.links[0].target, "docs/spec");
    assert_eq!(c1.links[0].display(), "spec");

    assert_eq!(board.columns[1].title, "Готово");
    assert_eq!(board.columns[1].cards[0].status, KanbanTaskStatus::Done);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-core --test kanban`
Expected: FAIL with `unresolved import truff_core::markdown::kanban`

**Step 3: Implement `kanban/mod.rs` and `kanban/parser.rs`**

Create `crates/core/src/markdown/kanban/mod.rs` and `crates/core/src/markdown/kanban/parser.rs`:
- Define `KanbanTaskStatus` (`Todo`, `InProgress`, `Done`) with `cycle()`, `toggle_done()`, `marker_char()`.
- Define `KanbanCardLink { pub target: String, pub heading: Option<String>, pub alias: Option<String>, pub raw_range_in_line: Range<usize> }` with `display(&self) -> &str` (returns `alias` if present, else file stem of `target`) and `navigation_target(&self) -> String`.
- Define `KanbanCard`:
  - `pub status: KanbanTaskStatus`
  - `pub raw_title: String` (the first line text after `- [ ] `)
  - `pub clean_title: String` (title with `[[...]]` chips and `#tag` tokens neatly stripped/collapsed for crisp card display; if stripping leaves empty string, falls back to link display names)
  - `pub body_lines: Vec<String>` (indented continuation lines)
  - `pub links: Vec<KanbanCardLink>`
  - `pub tags: Vec<String>`
  - `pub full_range: Range<usize>` (exact byte range from start of card line through trailing newline of its last continuation line)
  - `pub status_char_byte: usize` (exact byte offset of the character inside `[ ]`)
  - `pub title_range: Range<usize>` (exact byte range of `raw_title` on the card's first line)
  - `pub line_index: usize` (0-based line number)
- Define `KanbanColumn`:
  - `pub title: String`
  - `pub heading_range: Range<usize>` (full heading line including trailing newline)
  - `pub title_range: Range<usize>` (byte range of the heading title text)
  - `pub section_range: Range<usize>` (from start of `##` line up to the start of the next `##` / `#` heading or EOF)
  - `pub insert_card_offset: usize` (byte offset at end of column's cards or right after `##` heading line for appending a new card)
  - `pub cards: Vec<KanbanCard>`
- Implement `is_kanban_document(path: Option<&str>, text: &str) -> bool` and `KanbanBoard::parse(text: &str) -> KanbanBoard` (ignoring fenced code blocks so `##` or `- [ ]` inside ```` ``` ```` are never misparsed).
- Re-export `pub mod kanban;` in `crates/core/src/markdown/mod.rs`.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-core --test kanban`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/core/src/markdown/kanban/mod.rs crates/core/src/markdown/kanban/parser.rs crates/core/src/markdown/mod.rs crates/core/tests/kanban.rs
git commit -m "feat(core): add kanban markdown parser and link/tag extraction"
```

---

### Task 2: Core Range-Based Kanban Transactions (`truff_core::markdown::kanban::ops`)

**Files:**
- Create: `crates/core/src/markdown/kanban/ops.rs`
- Modify: `crates/core/src/markdown/kanban/mod.rs`
- Modify: `crates/core/tests/kanban.rs`

**Step 1: Write failing tests for card/column transactions, CRLF safety, surrounding Markdown preservation, and Undo/Redo**

```rust
// Append to crates/core/tests/kanban.rs
use truff_core::markdown::kanban::{
    CardInsertPos, KanbanTaskStatus, default_kanban_template, plan_add_card, plan_add_column,
    plan_attach_file_link, plan_cycle_card_status, plan_delete_card, plan_delete_empty_column,
    plan_edit_card_title, plan_move_card_column, plan_move_card_vertical, plan_move_column,
    plan_paste_card, plan_rename_column,
};
use truff_core::text::Buffer;

#[test]
fn range_transactions_preserve_non_card_markdown_and_support_undo_redo() {
    let initial = "\
<!-- truff:kanban -->
# Доска

Заметка сверху.

## План
- [ ] Первая
  Детали первой
- [ ] Вторая

Комментарий внизу колонки.

## В работе
- [/] Активная
";
    let mut buf = Buffer::from_text(initial);

    // 1. Move "Первая" (with its continuation line) right to "В работе"
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_move_card_column(&s, &board, 0, 0, 1).expect("move right");
    buf.apply(tx).unwrap();

    let after_move = buf.as_str();
    assert!(after_move.contains("Заметка сверху."));
    assert!(after_move.contains("Комментарий внизу колонки."));
    let reparsed = KanbanBoard::parse(&after_move);
    assert_eq!(reparsed.columns[0].cards.len(), 1);
    assert_eq!(reparsed.columns[0].cards[0].clean_title, "Вторая");
    assert_eq!(reparsed.columns[1].cards.len(), 2);
    assert_eq!(reparsed.columns[1].cards[1].clean_title, "Первая");
    assert_eq!(reparsed.columns[1].cards[1].body_lines, vec!["Детали первой"]);

    // 2. Attach file link to "Вторая"
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_attach_file_link(&s, &board, 0, 0, "notes/arch.md").expect("attach link");
    buf.apply(tx).unwrap();
    let linked = KanbanBoard::parse(&buf.as_str());
    assert_eq!(linked.columns[0].cards[0].links[0].target, "notes/arch");

    // 3. Reorder inside "В работе" (move index 1 up to 0)
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_move_card_vertical(&s, &board, 1, 1, -1).expect("move up");
    buf.apply(tx).unwrap();
    let reordered = KanbanBoard::parse(&buf.as_str());
    assert_eq!(reordered.columns[1].cards[0].clean_title, "Первая");
    assert_eq!(reordered.columns[1].cards[1].clean_title, "Активная");

    // 4. Undo all 3 steps cleanly back to exact initial text
    assert!(buf.undo().unwrap().is_some());
    assert!(buf.undo().unwrap().is_some());
    assert!(buf.undo().unwrap().is_some());
    assert_eq!(buf.as_str(), initial);

    // Redo restores edit
    assert!(buf.redo().unwrap().is_some());
}

#[test]
fn crlf_and_empty_board_initialization_work() {
    let mut buf = Buffer::from_text("");
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_add_card(&s, &board, 0, CardInsertPos::End, "Задача [[readme]]").unwrap();
    buf.apply(tx).unwrap();
    let parsed = KanbanBoard::parse(&buf.as_str());
    assert_eq!(parsed.columns.len(), 3);
    assert_eq!(parsed.columns[0].cards[0].links[0].target, "readme");

    let crlf = "<!-- truff:kanban -->\r\n## Todo\r\n- [ ] One\r\n- [ ] Two\r\n";
    let mut crlf_buf = Buffer::from_text(crlf);
    let s = crlf_buf.as_str();
    let b = KanbanBoard::parse(&s);
    let tx = plan_cycle_card_status(&s, &b, 0, 0).unwrap();
    crlf_buf.apply(tx).unwrap();
    assert_eq!(
        crlf_buf.as_str(),
        "<!-- truff:kanban -->\r\n## Todo\r\n- [/] One\r\n- [ ] Two\r\n"
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-core --test kanban`
Expected: FAIL with unresolved imports from `truff_core::markdown::kanban`

**Step 3: Implement `crates/core/src/markdown/kanban/ops.rs`**

Implement pure transaction planners using `Transaction`, `SingleEdit`, and `UndoGroup::next()`:
- Detect document line ending (`"\r\n"` if text contains `"\r\n"`, else `"\n"`).
- `default_kanban_template(title: &str) -> String`: generates `<!-- truff:kanban -->\n# {title}\n\n## План\n\n## В работе\n\n## Готово\n`.
- `CardInsertPos` (`Before(usize)`, `After(usize)`, `End`).
- `plan_add_card`: if `board.columns.is_empty()`, inserts default template with the card placed in `## План`; otherwise inserts `- [ ] {card_text}{nl}` at the target card boundary or column `insert_card_offset`.
- `plan_edit_card_title`: replaces `card.title_range` with `new_title`.
- `plan_set_card_status` / `plan_cycle_card_status`: replaces the 1-byte `status_char_byte..status_char_byte + 1` with `new_status.marker_char()`.
- `plan_delete_card`: deletes `card.full_range` and returns `(Transaction, String)` where `String` is the yanked raw card block.
- `plan_paste_card`: inserts a yanked raw card block (normalizing trailing newline) at `CardInsertPos`.
- `plan_move_card_vertical`: swaps adjacent cards in the same column via a single replacement over `min_start..max_end` or two sorted disjoint `SingleEdit`s.
- `plan_move_card_column`: builds two sorted disjoint `SingleEdit`s (one `SingleEdit::delete(card.full_range)`, one `SingleEdit::insert(dest_col.insert_card_offset, raw_card)`), ordered ascending by byte offset so `Buffer::apply` validates cleanly.
- `plan_attach_file_link`: strips `.md` suffix for clean wiki-link `[[target]]`; if card doesn't already link to `target`, appends ` [[target]]` at the end of `card.title_range`.
- `plan_add_column`, `plan_rename_column`, `plan_move_column`, `plan_delete_empty_column`.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-core --test kanban`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/core/src/markdown/kanban/ops.rs crates/core/src/markdown/kanban/mod.rs crates/core/tests/kanban.rs
git commit -m "feat(core): add range-based kanban transactions with undo support"
```

---

### Task 3: UI Kanban State Machine, Vim Motions & Fuzzy File Link Picker (`crates/ui/src/native/kanban`)

**Files:**
- Create: `crates/ui/src/native/kanban/mod.rs`
- Create: `crates/ui/src/native/kanban/state.rs`
- Create: `crates/ui/src/native/kanban/picker.rs`
- Modify: `crates/ui/src/native/mod.rs` (declare `pub mod kanban;`)

**Step 1: Write failing unit tests for Vim motions, counts, yank/paste, and fuzzy file picker**

Add unit tests in `crates/ui/src/native/kanban/state.rs` testing:
- `3j`, `2l`, `gg`, `G`, `0`, `$` navigation across columns and cards (including Cyrillic layout keys `р/о/л/д` matching Truff's existing task hub convenience).
- `H`/`L` emitting `KanbanIntent::MoveCardColumn`, `J`/`K` emitting `KanbanIntent::MoveCardVertical`.
- `o`/`O`/`i`/`e` entering `CardInput` with `[[` inline file completion popup and `Enter` / `Esc` committing.
- `f` opening `FilePicker`, typing a query filtering vault files by fuzzy/substring rank, `Enter` emitting `KanbanIntent::AttachFileLink`.
- `Enter` / `gf` on a card with 1 link emitting `KanbanIntent::OpenLink(target)`, with >1 links opening `LinkJumpPicker`, and with 0 links entering inline edit (for `Enter`) or status message (for `gf`).
- `dd` + `p` cutting and pasting cards across columns, `u` / `Ctrl-r` emitting `Undo` / `Redo`, `gs` emitting `ToggleSourceView`.

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-ui kanban::state::tests`
Expected: FAIL (module does not exist yet)

**Step 3: Implement `kanban/mod.rs`, `kanban/state.rs`, and `kanban/picker.rs`**

- `KanbanIntent` enum returned by `KanbanBoardState::feed_key(key: Key, board: &KanbanBoard, vault_files: &[String])`:
  - `None`
  - `ApplyTx(Transaction)`
  - `CutCard { tx: Transaction, raw_card: String }`
  - `OpenLink(String)`
  - `Undo`
  - `Redo`
  - `SwitchBoard { next: bool }`
  - `SelectBoard(String)`
  - `CreateNewBoard`
  - `ToggleSourceView`
  - `CloseOverlay`
  - `Status(String)`
- `KanbanInputMode`:
  - `Normal`
  - `CardInput { col_idx: usize, edit_card_idx: Option<usize>, insert_pos: CardInsertPos, buffer: String, wiki_ac: Option<InlineFileAutocomplete> }`
  - `ColumnInput { rename_col_idx: Option<usize>, buffer: String }`
  - `FilePicker(FuzzyFilePicker)`
  - `LinkJumpPicker { links: Vec<KanbanCardLink>, selected: usize }`
  - `FilterSearch`
- Support count accumulation (`count_prefix: Option<usize>`), two-key sequences (`gg`, `gf`, `gs`, `dd`, `yy`), and `yank_register: Option<String>`.
- In `picker.rs`, implement `FuzzyFilePicker` and `InlineFileAutocomplete` (ranking exact stem match > prefix > substring > subsequence over `vault_files`).

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui kanban::state::tests`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/ui/src/native/kanban/mod.rs crates/ui/src/native/kanban/state.rs crates/ui/src/native/kanban/picker.rs crates/ui/src/native/mod.rs
git commit -m "feat(ui): implement kanban vim state machine and file link picker"
```

---

### Task 4: Aesthetic Kanban Renderer & Vector Icons (`crates/ui/src/native/kanban/render.rs`)

**Files:**
- Modify: `crates/ui/src/native/icons.rs`
- Create: `crates/ui/src/native/kanban/render.rs`
- Modify: `crates/ui/src/native/kanban/mod.rs`

**Step 1: Write failing render smoke test**

Add a headless `egui::Context` test in `crates/ui/src/native/kanban/render.rs` verifying that rendering both modal (`render_kanban_modal`) and embedded central view (`render_kanban_central`) with columns, cards, file chips, `#tags`, and active `FilePicker` completes cleanly without panics across `Theme::nord()`, `Theme::obsidian_dark()`, and `Theme::light()`.

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-ui kanban::render::tests`
Expected: FAIL (missing `render` module and icons)

**Step 3: Implement `Icon::Kanban`, `Icon::InProgress`, and `kanban/render.rs`**

- In `crates/ui/src/native/icons.rs`:
  - Add `Icon::Kanban` (3 crisp vertical column bars of staggered heights inside a rounded frame) and `Icon::InProgress` (square with diagonal/half accent indicator).
- In `crates/ui/src/native/kanban/render.rs`:
  - Shared `render_board_surface(ui, state, board, board_list, active_board_path, vault_files, theme, is_modal) -> KanbanIntent`:
    - **Header bar:** `Icon::Kanban`, board switcher pills (active board highlighted with `theme.accent`), completion badge (`done / total` + mini progress bar), search filter input (`/`), `+ Колонка (A)`, `Исходник (gs)` (when `!is_modal`) or `Закрыть (Esc)` (when `is_modal`).
    - **Columns area:** horizontal `egui::ScrollArea::horizontal()` with `296px` columns (`bg_secondary`, `1px border_subtle`, `8px` radius). Column header paints semantic accent dot (cycling `callout_note`, `state_warning`, `state_success`, `accent`), bold column title, count pill, and `3px` completion bar. Active column auto-scrolls into view.
    - **Cards:** elevated card frames (`bg_elevated` / `bg_canvas`, `6px` radius). Selected card draws `1.5px theme.accent` border + `3px` left accent stripe and calls `scroll_to_me(Some(egui::Align::Center))` when selection changes.
    - **Card contents:**
      - Clickable vector status checkbox (`Todo` -> `Icon::Square`, `InProgress` -> `Icon::InProgress` in `theme.state_warning`, `Done` -> `Icon::CheckSquare` in `theme.state_success` with dimmed title).
      - Clean title + muted indented continuation lines (up to 2 lines preview).
      - `#tag` pills (`bg_selected`, `text_secondary`, `4px` radius).
      - **File-link chips:** horizontal wrap of clickable pills (`bg_tertiary`, subtle `accent` border on hover, leading `Icon::File` + `link.display()`). Clicking a chip immediately emits `KanbanIntent::OpenLink(link.navigation_target())`.
    - **Inline editors & popups:** inline `TextEdit` for `CardInput` (with floating `[[` autocomplete dropdown) and `ColumnInput`, modal sub-card for `FilePicker` (`f`) and `LinkJumpPicker`, plus a clean footer bar showing contextual Vim key pills (`h/j/k/l`, `H/J/K/L`, `Space: статус`, `o: карточка`, `f: файл`, `Enter/gf: открыть`, `u: отмена`).

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui kanban::render::tests`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/ui/src/native/icons.rs crates/ui/src/native/kanban/render.rs crates/ui/src/native/kanban/mod.rs
git commit -m "feat(ui): add aesthetic kanban board renderer and vector icons"
```

---

### Task 5: Dual App Integration (`Space b` Overlay + Central `Board ↔ Source`) & Workflow Tests

**Files:**
- Modify: `crates/ui/src/app/mod.rs`
- Modify: `crates/ui/src/app/commands/mod.rs`
- Modify: `crates/ui/src/app/completion.rs`
- Modify: `crates/ui/src/shell/mod.rs`
- Modify: `crates/ui/src/shell/routing/commands.rs`
- Create: `crates/ui/src/native/kanban/controller.rs`
- Modify: `crates/ui/src/native/mod.rs`
- Modify: `crates/ui/src/native/content.rs`
- Modify: `crates/ui/src/native/events/mod.rs`
- Modify: `crates/ui/src/native/events/actions.rs`
- Create: `crates/ui/tests/kanban_workflows.rs`

**Step 1: Write failing end-to-end integration tests (`crates/ui/tests/kanban_workflows.rs`)**

Write tests covering both Mode A and Mode B:
1. **`space_b_opens_kanban_overlay_creates_tasks_links_file_and_jumps`:**
   - Open a temporary vault with `notes/target.md` (`# Target Note`) and `notes/current.md`.
   - Press `Space` then `b` -> `app.kanban.is_open()` becomes `true` (targeting `kanban.md`).
   - Press `o`, type `Проверить архитектуру`, press `Enter` -> default columns + card created and persisted.
   - Press `f`, type `target`, press `Enter` -> card now has link `[[notes/target]]`.
   - Press `L` -> card moves from `План` to `В работе`; press `Space` -> status cycles to `InProgress`.
   - Press `u` -> status reverts to `Todo` (proving `Buffer` undo works inside overlay!).
   - Press `Enter` on the linked card -> overlay closes and active editor note becomes `notes/target.md`!
2. **`central_panel_renders_kanban_file_and_toggles_source_with_gs`:**
   - Create `sprint.kanban.md` with two columns and open it in editor.
   - Verify `app.is_central_kanban_active()` is `true`.
   - Send `j`, `L`, `x` -> buffer updates via `Transaction`, card moves to column 2 and marks `[x]`.
   - Send `g` then `s` -> toggles to raw Markdown `Source` view (`is_central_kanban_active() == false`), allowing raw Vim text editing.
   - Execute `:kanban` (or toggle back) -> returns to central Kanban board view!

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-ui --test kanban_workflows`
Expected: FAIL (missing `NativeApp::kanban` and routing)

**Step 3: Wire `Application`, `Shell`, `NativeApp`, and `content.rs`**

1. **`Shell` & `CommandRegistry` routing:**
   - Add `ShellAction::OpenKanban` in `crates/ui/src/shell/mod.rs`.
   - Route `"panel.boards"` (`Space b`) in `crates/ui/src/shell/routing/commands.rs` to `ShellAction::OpenKanban` (keeping `NavigatorMode::Boards` intact for programmatic callers if any).
2. **`Application` (`crates/ui/src/app/`):**
   - Add `pub kanban_requested: Option<String>` and `kanban_source_views: HashSet<ViewId>` (tracks views temporarily toggled from Board -> Source via `gs`).
   - Add `:kanban [path]` and `:board [path]` in `crates/ui/src/app/commands/mod.rs` (if active note is a kanban note in Source override, clears override; otherwise sets `kanban_requested = Some(argument.to_owned())`) and register `"kanban"` in `crates/ui/src/app/completion.rs`.
3. **`KanbanController` (`crates/ui/src/native/kanban/controller.rs`):**
   - Owns `modal_open: bool`, `modal_board_path: String`, `modal_state: KanbanBoardState`, and `central_state: KanbanBoardState`.
   - Resolves available boards in vault (`kanban.md` + all `*.kanban.md` + open buffers with `<!-- truff:kanban -->`).
   - Applies `Transaction`s directly through `Workspace` buffers (opening `modal_board_path` into `Workspace::buffers` on demand so `Buffer::apply`, `Buffer::undo`, `Buffer::redo`, dirty/savepoint, and disk sync share one canonical implementation without duplicating editor state).
   - Handles `KanbanIntent::OpenLink(target)` by closing modal (if modal) and calling `application.navigate_to_link(&target)`.
4. **`NativeApp`, `events`, and `content.rs`:**
   - In `NativeApp::show`: handle `application.kanban_requested`, include `self.kanban.is_modal_open()` in `editor.input_blocked`, render central board in `content::show` when active document is a Kanban board and not in `kanban_source_views`, and render modal board overlay when `self.kanban.is_modal_open()`.
   - In `NativeApp::key`: route keys to `self.kanban` when modal is open, OR when focus is `Focus::Editor`, no command/dialog/navigator overlay is open, and central Kanban view is active.

**Step 4: Run integration tests and full workspace verification**

Run:
```bash
cargo test -p truff-ui --test kanban_workflows
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
Expected: ALL PASS with zero warnings.

**Step 5: Commit**

```bash
git add crates/ui/src/app/mod.rs crates/ui/src/app/commands/mod.rs crates/ui/src/app/completion.rs crates/ui/src/shell/mod.rs crates/ui/src/shell/routing/commands.rs crates/ui/src/native/kanban/ crates/ui/src/native/mod.rs crates/ui/src/native/content.rs crates/ui/src/native/events/mod.rs crates/ui/src/native/events/actions.rs crates/ui/tests/kanban_workflows.rs
git commit -m "feat(ui): integrate kanban modal overlay and central board view"
```
