# Table Rendering and Smart Formatting Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Implement Markdown table formatting (column alignment with Unicode width, smart Enter/Tab continuation and navigation) in `crates/core` and table rendering in `crates/ui` (hiding the delimiter row text, compact horizontal divider line, and subtle borders).

**Architecture:**
- `truff_core::markdown::table`: Module for scanning table boundaries, parsing cells with alignment, calculating column widths, and generating aligned Markdown tables in atomic `Transaction`s.
- `truff_core::markdown::smart_edit`: Extension of `plan_enter` and `plan_tab` to handle table row creation, exiting empty tables, and navigating cells.
- `truff_core::commands`: Register `markdown.format_table` (`:format-table`, `:table-format`, `:table`).
- `truff_ui::editor::display`: Support hiding table delimiter text `|---|---|---|` in `DisplayMap` when unfocused, reducing line height to a divider in `paint.rs`, and styling `|` column borders cleanly.

**Tech Stack:** Rust 2024, crop (Rope), unicode-segmentation 1.13.3, pulldown-cmark 0.13.4, egui 0.36.2.

---

### Task 1: Table Formatter in `crates/core`

**Files:**
- Create: `crates/core/src/markdown/table.rs`
- Modify: `crates/core/src/markdown/mod.rs`
- Test: `crates/core/tests/table_format.rs`

**Step 1: Write the failing test**
Create `crates/core/tests/table_format.rs` with test cases:
- Unaligned table with mismatched column lengths -> formatted table with aligned pipes.
- Column alignments preserved (`:---`, `:---:`, `---:`).
- Escaped pipes `\|` within cells handled properly without splitting columns.
- Table with Unicode Russian characters formatted with correct column widths.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-core --test table_format`  
Expected: FAIL (module/functions missing).

**Step 3: Implement minimal table formatter in `crates/core/src/markdown/table.rs`**
- Functions:
  - `find_table_range_at(buffer: &Buffer, pos: usize) -> Option<Range<usize>>`
  - `format_table_str(raw: &str) -> Option<String>`
  - `format_table_transaction(buffer: &Buffer, pos: usize) -> Option<Transaction>`
- Register in `crates/core/src/markdown/mod.rs`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-core --test table_format`  
Expected: PASS.

**Step 5: Commit**
```bash
git add crates/core/src/markdown/table.rs crates/core/src/markdown/mod.rs crates/core/tests/table_format.rs
git commit -m "feat(core): add markdown table formatting engine with unicode alignment"
```

---

### Task 2: Smart Edit for Tables in `crates/core`

**Files:**
- Modify: `crates/core/src/markdown/smart_edit.rs`
- Test: `crates/core/tests/smart_edit_tables.rs`

**Step 1: Write the failing test**
Create `crates/core/tests/smart_edit_tables.rs`:
- `plan_enter` inside a table row with content creates a new table row with identical column count `|   |   |`.
- `plan_enter` on an empty table row (only pipes and whitespace) exits the table (removes the empty row and inserts a newline).
- `plan_table_tab` moves cursor to next cell (or creates a new row if at the last cell of the last row).

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-core --test smart_edit_tables`  
Expected: FAIL.

**Step 3: Implement smart enter and tab for tables in `smart_edit.rs`**
- In `plan_enter`: check if cursor is inside a table row. If empty table row, remove row and end table. If non-empty, auto-format current table and insert new row below.
- Add `plan_table_tab(buffer: &Buffer, cursor: BytePos, backward: bool) -> Option<Transaction>`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-core --test smart_edit_tables`  
Expected: PASS.

**Step 5: Commit**
```bash
git add crates/core/src/markdown/smart_edit.rs crates/core/tests/smart_edit_tables.rs
git commit -m "feat(core): support smart enter and tab navigation in markdown tables"
```

---

### Task 3: Command Registration (`:format-table`)

**Files:**
- Modify: `crates/core/src/commands/registry.rs`
- Modify: `crates/ui/src/app/commands/mod.rs` (or dispatch handler)
- Test: `crates/core/tests/table_commands.rs`

**Step 1: Write the failing test**
Create test checking `markdown.format_table` command exists in registry and dispatches `format_table_transaction` on the active buffer.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-core --test table_commands`  
Expected: FAIL.

**Step 3: Implement command registration and dispatch**
- Register `markdown.format_table` in `CommandRegistry::new` with title "Format Table".
- Add command line aliases `:format-table`, `:table-format`, `:table`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-core --test table_commands`  
Expected: PASS.

**Step 5: Commit**
```bash
git add crates/core/src/commands/ crates/ui/src/app/commands/ crates/core/tests/table_commands.rs
git commit -m "feat(commands): register format-table command and aliases"
```

---

### Task 4: UI Table Delimiter Hiding & Clean Border Rendering

**Files:**
- Modify: `crates/ui/src/editor/display/markers.rs`
- Modify: `crates/ui/src/editor/display/projection.rs`
- Modify: `crates/ui/src/native/editor/paragraph/paint.rs`
- Modify: `crates/ui/src/native/editor/paragraph/mod.rs`
- Modify: `crates/ui/src/native/editor/viewport.rs`
- Test: `crates/ui/tests/table_display.rs`

**Step 1: Write the failing test**
Create `crates/ui/tests/table_display.rs`:
- In `LivePreview`, when cursor is NOT on table delimiter row, `DisplayMap` marks delimiter row text as hidden.
- In `LivePreview`, when cursor IS on delimiter row, delimiter row text is revealed for editing.
- In `Reading` mode, delimiter row text is unconditionally hidden.
- Table delimiter row height is compressed to a divider line height (e.g. 6-8 dp).

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test table_display`  
Expected: FAIL.

**Step 3: Implement delimiter hiding and styling**
- In `markers.rs`: recognize delimiter rows. When `presentation.table_delimiter` is true, add a marker covering the line.
- In `projection.rs`: hide delimiter text when unfocused (or in Reading mode).
- In `paragraph/paint.rs`: draw clean horizontal separator line `painter.hline` between header and body.
- Style pipes `|` using subtle border color `[120, 130, 145, 120]`.
- Adjust paragraph row height for unfocused delimiter rows.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test table_display`  
Expected: PASS.

**Step 5: Commit**
```bash
git add crates/ui/src/editor/display/ crates/ui/src/native/editor/ crates/ui/tests/table_display.rs
git commit -m "feat(ui): render clean table grid and hide delimiter rows when unfocused"
```

---

### Task 5: End-to-End Verification and Workspace Checks

**Files:**
- Run workspace verification checks

**Step 1: Run comprehensive workspace checks**
```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
Expected: All tests pass, 0 warnings, formatting is clean.

**Step 2: Commit**
```bash
git commit -am "chore: verify workspace tests and clippy for table rendering and formatting"
```
