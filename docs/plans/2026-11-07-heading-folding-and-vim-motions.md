# Heading Folding and Vim Motions Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Implement collapsible Markdown headings in Obsidian style with interactive visual indicators (chevron, badge) and Vim fold commands (`za`, `zc`, `zo`, `zM`, `zR`).

**Architecture:** Heading ranges are extracted from existing `DocFacts` / `HeadingFact`. The `EditorWidget` maintains folded heading line numbers (`folded_headings`), maps folded lines to height `0.0` in `HeightIndex`, and hides them in the viewport. Clicking chevron / badge emits `EditorInput::ToggleFold`, while Vim motions `za`, `zc`, `zo`, `zM`, `zR` execute fold actions and re-position the caret to the heading if it was within the folded section.

**Tech Stack:** Rust (2024 edition), `egui`, `truff-core` (crop Rope, Vim parser & executor), `truff-ui` (HeightIndex, EditorWidget, Viewport).

---

### Task 1: Vim Parser & Command Representation for `z` Fold Motions

**Files:**
- Modify: `crates/core/src/vim/parser/command.rs`
- Modify: `crates/core/src/vim/parser/mod.rs`
- Modify: `crates/core/src/vim/execute/context.rs`
- Modify: `crates/core/src/vim/execute/command.rs`
- Test: `crates/core/tests/vim.rs`

**Step 1: Write failing tests for `za`, `zc`, `zo`, `zM`, `zR`**
Add tests asserting that `Key::Char('z')` followed by `'a'`, `'c'`, `'o'`, `'M'`, `'R'` produces `Command::Fold(FoldCommand::...)`.

**Step 2: Run tests to verify failure**
Run: `cargo test -p truff-core --test vim`

**Step 3: Implement minimal command and parser changes**
- Add `pub enum FoldCommand { Toggle, Close, Open, CloseAll, OpenAll }` to `command.rs`.
- Add `Command::Fold(FoldCommand)` to `enum Command`.
- In `crates/core/src/vim/parser/mod.rs`, expand `Key::Char('z')` match to handle `'a'`, `'c'`, `'o'`, `'M'`, `'R'`.
- In `crates/core/src/vim/execute/context.rs` and `command.rs`, map `Command::Fold(cmd)` to `InputResult::Fold(cmd)`.

**Step 4: Run tests to verify they pass**
Run: `cargo test -p truff-core --test vim`

---

### Task 2: Heading Fold Range Calculation

**Files:**
- Modify: `crates/core/src/markdown/facts.rs` or `crates/ui/src/editor/`
- Test: `crates/core/tests/markdown.rs` or `crates/ui/tests/display.rs`

**Step 1: Write unit tests for fold range computation**
Test single heading, nested headings (H1 with H2, H3, and subsequent H1/H2), and EOF boundaries.

**Step 2: Implement fold range helper**
Compute:
- Given headings and a heading index (or line), calculate `(start_line..end_line)` of content to fold.
- Given folded heading set, check whether a line index is hidden.
- Given cursor line, find target heading index to fold/unfold.

**Step 3: Run tests to verify correctness**
Run: `cargo test -p truff-core` and `cargo test -p truff-ui`

---

### Task 3: HeightIndex and Viewport Fold Integration

**Files:**
- Modify: `crates/ui/src/editor/heights.rs`
- Modify: `crates/ui/src/editor/input.rs`
- Modify: `crates/ui/src/native/editor/mod.rs`
- Modify: `crates/ui/src/native/editor/viewport.rs`
- Modify: `crates/ui/src/native/editor/paragraph/paint.rs`

**Step 1: Allow 0.0 height for folded lines in HeightIndex**
Update `update_block_height` to allow `new_height == 0.0` (skipping Fenwick error on negative or non-finite values).

**Step 2: Add `EditorInput::ToggleFold { heading_line: usize }`**
In `crates/ui/src/editor/input.rs`.

**Step 3: Render chevron `▼` / `▶` and fold badge `...`**
- In `viewport.rs`, skip measuring and painting folded lines.
- Paint interactive chevron next to heading line.
- Paint clickable ellipsis badge `...` when folded.
- On click, return `EditorInput::ToggleFold`.

---

### Task 4: ShellAction and Vim Event Handling

**Files:**
- Modify: `crates/ui/src/shell/mod.rs`
- Modify: `crates/ui/src/shell/routing.rs`
- Modify: `crates/ui/src/native/events.rs`
- Modify: `crates/ui/src/native/editor/mod.rs`

**Step 1: Add `ShellAction::Fold(FoldCommand)`**
Route from `InputResult::Fold(cmd)` in `routing.rs`.

**Step 2: Implement fold execution**
In `events.rs` and `EditorWidget`:
- `Toggle`: toggle fold of heading at cursor (or enclosing heading).
- `Close`: close fold. If cursor is on hidden line, move cursor to heading line.
- `Open`: open fold.
- `CloseAll`: close all headings.
- `OpenAll`: open all headings.
- Handle `EditorInput::ToggleFold { heading_line }`.

---

### Task 5: Integration Tests & Full Workspace Verification

**Files:**
- Test: `crates/ui/tests/native.rs` and `crates/ui/tests/editor_input.rs`

**Step 1: Write integration tests for fold commands**
Verify `za`, `zc`, `zo`, `zM`, `zR`, clicking fold chevron, and cursor jumping.

**Step 2: Workspace check**
Run:
- `cargo check --workspace --all-targets`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --all -- --check`
