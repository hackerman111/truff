# Unified Markdown Rendering, Nested List Fix, and `<Space> Enter` Hub Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Fix nested list indentation/bullet alignment on tabs and deep nesting, unify preview markdown rendering with the main note editor pipeline (removing the secondary renderer), and replace `Space o` / `Space l` with a 3-column `<Space> Enter` hub (Outline/Backlinks + Preview + Vault/File/Project Tasks).

**Architecture:**
1. Normalize list item leading whitespace in `DisplayMap` (when `LinePresentation` is provided) to `"    ".repeat(list.depth)` and teach `SyntaxSnapshot::line_presentation` to recognize over-indented list items inside `indented_code_block` as well as task items inside ```` ```tasks ```` blocks.
2. Expose a shared read-only paragraph rendering helper (`paint_markdown_preview`) backed by `IncrementalSyntaxTree`, `DisplayMap`, and `Paragraph::paint`, replacing `crates/ui/src/navigator/render/markdown.rs`.
3. Rebind `panel.outline` to `Space Enter` (removing `Space o` and `Space l`), make `Tab` toggle `Outline` ↔ `Links`, and add a 3-column layout with a right-hand task manager reading/writing `<vault>/index.md`, active buffer ```` ```tasks ```` blocks, and `<project_dir>/index.md`.

**Tech Stack:** Rust, `egui`, `tree-sitter-md`, `pulldown-cmark`, `crop::Rope`.

---

### Task 1: Fix Nested List Rendering & Normalize List Indentation

**Files:**
- Modify: `crates/core/src/markdown/presentation.rs`
- Modify: `crates/ui/src/editor/display/markers.rs`
- Modify: `crates/ui/src/native/editor/paragraph/paint.rs`
- Modify: `crates/ui/src/native/editor/viewport.rs`
- Test: `crates/ui/tests/native.rs`
- Test: `crates/core/tests/markdown_presentation.rs`

**Step 1: Write failing tests**
Add tests in `crates/core/tests/markdown_presentation.rs` and `crates/ui/tests/native.rs` for mixed tab/space nested lists (`\t- l1\n    \t- l2\n    \t\t\t- l3`) and ```` ```tasks ```` blocks.

**Step 2: Implement fix**
- Update `SyntaxSnapshot::line_presentation` to accept line text (or inspect via `SyntaxSnapshot` + line text) so over-indented list items inside `list_item -> indented_code_block` and task items inside ```` ```tasks ```` fences produce `LinePresentation { list: Some(...), code: false, .. }`.
- Normalize leading whitespace `0..list.range.start` in `DisplayMap` to `"    ".repeat(list.depth)` when `self.presentation` is present.
- Simplify `Paragraph::paint` guide stick calculation to always use 4-space `STANDARD_PREFIXES`.

---

### Task 2: Unify Markdown Rendering in Preview Panel

**Files:**
- Modify: `crates/ui/src/native/editor/mod.rs`
- Modify: `crates/ui/src/navigator/render/preview.rs`
- Modify: `crates/ui/src/navigator/render.rs`
- Delete: `crates/ui/src/navigator/render/markdown.rs`
- Modify: `crates/ui/tests/markdown_preview.rs`

**Step 1: Expose shared markdown preview painter**
Add `render_markdown_document(ui: &mut egui::Ui, text: &str, theme: &Theme)` in `crates/ui/src/native/editor/` using `IncrementalSyntaxTree`, `DisplayMap`, and `Paragraph::paint`.

**Step 2: Switch `preview.rs` and `markdown_preview.rs` to the shared painter and delete `navigator/render/markdown.rs`**
Ensure heading section previews and file previews render through the main editor's `Paragraph` pipeline.

---

### Task 3: `<Space> Enter` Mode with Outline/Backlinks, Preview, and 3 Task Categories

**Files:**
- Modify: `crates/core/src/commands/defaults.rs`
- Modify: `crates/core/src/config/defaults.rs`
- Modify: `crates/ui/src/native/outline.rs`
- Modify: `crates/ui/src/navigator/types.rs`
- Modify: `crates/ui/src/navigator/model.rs`
- Modify: `crates/ui/src/navigator/input.rs`
- Modify: `crates/ui/src/navigator/render.rs`
- Modify: `crates/ui/src/navigator/render/preview.rs`
- Modify: `crates/ui/src/native/navigator.rs`
- Test: `crates/ui/tests/navigator.rs`
- Test: `crates/core/tests/core_upd_features.rs`

**Step 1: Update keybindings and `Tab` switching**
- Replace `Space o` and `Space l` with `Space Enter` (`panel.outline`).
- When in `NavigatorMode::Outline` or `NavigatorMode::Links`, `Tab` / `BackTab` toggles between `Outline` and `Links`.
- Populate `preview` on `Outline` heading items (section text from the active buffer) and `Links` items (target/source snippet).

**Step 2: Add 3-category Task Hub (`Vault`, `File`, `Project`)**
- Parse and mutate human-readable tasks (`- [ ]`, `- [/]`, `- [x]`) in:
  1. `<vault_root>/index.md` (Общие задачи)
  2. ```` ```tasks ... ``` ```` in the active note buffer (Задачи файла)
  3. `<active_file_dir>/index.md` (Задачи проекта)
- Add focus toggle (`h`/`l` or `Left`/`Right`), category selection (`1`/`2`/`3` or `H`/`L`), task navigation (`j`/`k`, `Up`/`Down`, `gg`, `G`), status toggle (`Space`/`x`), add (`a`), edit (`e`/`i`), delete (`d`).
