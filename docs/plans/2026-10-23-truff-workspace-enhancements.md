# Truff v2 Workspace & Editor Enhancements Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Implement comprehensive workspace, editor, navigation, and UX enhancements for Truff v2 covering adjacent note creation, Which-Key color and link workflows, file tree drag/motions, list editing, lifecycle confirmations, and an Obsidian-style right dock.

**Architecture:** Enhancements are partitioned cleanly across three layers: `truff-core` for pure Markdown and Vim text transformations, `truff-store` for directory and filesystem creation, and `truff-ui` for egui rendering, input routing, dialogs, and dock panels.

**Tech Stack:** Rust 2024, `egui` / `eframe` (Glow), `truff-core`, `truff-store`, `pulldown-cmark`, `crop`.

---

### Task 1: Core Text, List Continuation & HTML Style Newline Fix

**Files:**
- Modify: `crates/core/src/markdown/smart_edit.rs`
- Modify: `crates/core/src/markdown/style.rs`
- Modify: `crates/core/src/vim/execute/editing.rs`
- Test: `crates/core/tests/smart_edit_mebc1.rs`
- Test: `crates/core/tests/style.rs`

**Step 1: Write the failing tests**
- In `crates/core/tests/smart_edit_mebc1.rs`, add test `test_plan_enter_does_not_duplicate_existing_bullet`:
  - When splitting a line where text after the cursor already starts with `- `, verify resulting lines do not have `- - `.
- In `crates/core/tests/style.rs`, add test `test_apply_text_color_excludes_trailing_newline`:
  - When applying color to a range that ends with `\n` or `\r\n`, verify `</span>` is placed before the newline.

**Step 2: Run tests to verify they fail**
Run: `cargo test -p truff-core --test smart_edit_mebc1 test_plan_enter_does_not_duplicate_existing_bullet`
Expected: FAIL

**Step 3: Implement the fixes in `truff-core`**
- In `crates/core/src/markdown/smart_edit.rs` (`plan_enter`):
  - Check the text slice after `split_pos` up to line end.
  - If the slice starts with the same list marker (e.g. `- `, `* `, `+ `), skip the marker in the rest of the text or adjust insertion so duplicate marker is not created.
- In `crates/core/src/markdown/style.rs` (`apply_text_color`):
  - Trim trailing `\r?\n` from `range` before calculating `outer_range` and wrapping in `<span>`.
- In `crates/core/src/vim/execute/editing.rs` (`insert_key` on `Key::Backspace`):
  - If characters preceding cursor on the current line are spaces matching tab indentation (2 or 4 spaces up to tab stop), delete all spaces of that indent step.

**Step 4: Run tests to verify they pass**
Run: `cargo test -p truff-core --test smart_edit_mebc1 && cargo test -p truff-core --test style`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/core/src/markdown/smart_edit.rs crates/core/src/markdown/style.rs crates/core/src/vim/execute/editing.rs crates/core/tests/smart_edit_mebc1.rs crates/core/tests/style.rs
git commit -m "fix(core): de-duplicate list markers on enter, exclude trailing newlines in color tags, and add soft-tab backspace"
```

---

### Task 2: Editor Display (Arrow Glyphs, Indent Guides & Accent Rule)

**Files:**
- Modify: `crates/ui/src/editor/display/markers.rs`
- Modify: `crates/ui/src/native/editor/paragraph/paint.rs`
- Test: `crates/ui/tests/display.rs`

**Step 1: Write the failing tests**
- In `crates/ui/tests/display.rs`, add test verifying `-->`, `->`, `<--`, `<-`, `<-->`, `==>` are mapped to corresponding arrow symbols in projection / markers.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test display`
Expected: FAIL

**Step 3: Implement editor display enhancements**
- In `crates/ui/src/editor/display/markers.rs`:
  - Add text replacements for `-->` (`→`), `->` (`→`), `<--` (`←`), `<-` (`←`), `<-->` (`↔`), `<->` (`↔`), `==>` (`⇒`), `<==>` (`⇔`).
- In `crates/ui/src/native/editor/paragraph/paint.rs`:
  - Paint vertical indent guide lines `|` for list items matching indentation depth.
  - Increase rule stroke width for `---` (`Event::Rule`) to 1.5–2.0px with visible border color.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test display`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/editor/display/markers.rs crates/ui/src/native/editor/paragraph/paint.rs crates/ui/tests/display.rs
git commit -m "feat(ui): render arrow ligatures, list vertical indent guides, and prominent horizontal rules"
```

---

### Task 3: Vim Motion Yank to System Clipboard

**Files:**
- Modify: `crates/ui/src/native/events.rs`
- Modify: `crates/ui/src/shell/editor_input.rs`
- Test: `crates/ui/tests/editor_input.rs`

**Step 1: Write test for yank clipboard synchronization**
- Verify that yanking text via Vim operator motion places the yanked string into clipboard state.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test editor_input test_yank_to_clipboard`
Expected: FAIL

**Step 3: Implement clipboard sync on Yank**
- In `crates/ui/src/native/events.rs`:
  - When handling ShellActions or checking register updates following Vim execution, if a yank operator executed, invoke `context.copy_text(yanked_text)`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test editor_input`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/native/events.rs crates/ui/src/shell/editor_input.rs crates/ui/tests/editor_input.rs
git commit -m "feat(ui): copy yanked text to system clipboard on vim motions"
```

---

### Task 4: Note Creation & Dialog Usability Enhancements

**Files:**
- Modify: `crates/ui/src/app/global_config.rs`
- Modify: `crates/ui/src/native/creation/settings.rs`
- Modify: `crates/ui/src/native/creation/file.rs`
- Modify: `crates/ui/src/native/creation/folder.rs`
- Modify: `crates/ui/src/native/creation/operation.rs`
- Modify: `crates/ui/src/native/creation/vault.rs`
- Modify: `crates/ui/src/app/commands/files.rs`
- Test: `crates/ui/tests/settings_dialog.rs`
- Test: `crates/ui/tests/global_config.rs`

**Step 1: Write failing test for adjacent note setting and intermediate dir creation**
- In `crates/ui/tests/global_config.rs`, test `create_notes_adjacent` serialization and default.
- In `crates/ui/tests/settings_dialog.rs`, test toggling adjacent creation setting.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test global_config`
Expected: FAIL

**Step 3: Implement dialog enhancements**
- Add `create_notes_adjacent: bool` to `GlobalConfig`.
- Add checkbox in `SettingsDialog` for *"Создавать новые заметки рядом с активным файлом"*.
- In `FileDialog` and `file_command`: when `create_notes_adjacent` is true and an active note is open, prefill `self.path` with the parent folder of the active note.
- In `application.create_note`: call `std::fs::create_dir_all` for intermediate directories before creating file.
- In `FileDialog`: ensure text cursor is positioned at the end of `self.path` on open.
- In all dialogs (`file.rs`, `folder.rs`, `operation.rs`, `vault.rs`): handle `Enter` key to submit positive action.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test global_config && cargo test -p truff-ui --test settings_dialog`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/app/global_config.rs crates/ui/src/native/creation/ crates/ui/src/app/commands/files.rs crates/ui/tests/global_config.rs crates/ui/tests/settings_dialog.rs
git commit -m "feat(ui): add adjacent note creation setting, enter confirmation, and end caret in dialogs"
```

---

### Task 5: Which-Key `<Space> c` Menu & Note Link Workflow

**Files:**
- Modify: `crates/ui/src/shell/routing.rs`
- Modify: `crates/ui/src/overlays/which_key.rs`
- Modify: `crates/ui/src/native/events.rs`
- Modify: `crates/ui/src/app/selection.rs`
- Test: `crates/ui/tests/which_key.rs`

**Step 1: Write test for `<Space> c` Which-Key menu**
- Test that `<Space> c` activates a Which-Key menu with color options and `[n]` Note Link.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test which_key`
Expected: FAIL

**Step 3: Implement Which-Key `<Space> c` menu and 2-stage link creation**
- In `crates/ui/src/shell/routing.rs`: route `<Space> c` to show WhichKey prefix menu with items `[r] Red`, `[g] Green`, `[b] Blue`, `[y] Yellow`, `[p] Purple`, `[c] Cyan`, `[x] Reset`, `[n] Note link`.
- Implement `[n]`: open prompt for link name/alias (defaulting to note name or selection on Enter/Tab), then open target selector over vault notes to insert `[[target|alias]]`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test which_key`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/shell/routing.rs crates/ui/src/overlays/which_key.rs crates/ui/src/native/events.rs crates/ui/src/app/selection.rs crates/ui/tests/which_key.rs
git commit -m "feat(ui): add which-key color menu on <space> c with two-stage note link creation"
```

---

### Task 6: File Tree Operations, Drag & Drop, and Modern Vault Header

**Files:**
- Modify: `crates/ui/src/native/sidebar.rs`
- Modify: `crates/ui/src/shell/panels.rs`
- Modify: `crates/ui/src/shell/routing.rs`
- Test: `crates/ui/tests/navigator_workflows.rs`

**Step 1: Write test for sidebar file move and focus retention**
- Test `d`/`x` cut, `j`/`k` navigation, `p` paste to move file in tree.
- Test active file selection on sidebar focus.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test navigator_workflows`
Expected: FAIL

**Step 3: Implement tree enhancements**
- In `crates/ui/src/shell/panels.rs`: on `set_focus(Focus::Sidebar)`, locate active file, expand parent folders in `sidebar_model`, and set `selected_index`.
- In `crates/ui/src/native/sidebar.rs`:
  - Implement Vim motions `d`/`x` (cut), `p` (paste move into folder), `m` (move dialog).
  - Add mouse Drag & Drop row interaction: dragging file onto folder row triggers `application.rename_path`.
  - Replace static vault button with clickable vault header displaying vault name, chevron icon, and popover menu (recent vaults, Open, New).

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test navigator_workflows`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/native/sidebar.rs crates/ui/src/shell/panels.rs crates/ui/src/shell/routing.rs crates/ui/tests/navigator_workflows.rs
git commit -m "feat(ui): add tree drag-and-drop and vim moves, active file focus, and modern vault header"
```

---

### Task 7: Workspace Lifecycle (No Startup Scratch, Auto-Save, Exit Prompt)

**Files:**
- Modify: `crates/ui/src/shell/mod.rs`
- Modify: `crates/ui/src/app/vault.rs`
- Modify: `crates/ui/src/app/session.rs`
- Modify: `crates/ui/src/native/mod.rs`
- Modify: `crates/ui/src/native/creation/mod.rs`
- Test: `crates/ui/tests/e2e_workflow.rs`

**Step 1: Write test for clean startup without Scratch buffer and auto-save on Normal mode**
- Test startup when no vault/session note exists: verify no `Scratch` buffer is shown.
- Test returning from Insert to Normal mode triggers save of modified buffer.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test e2e_workflow`
Expected: FAIL

**Step 3: Implement lifecycle improvements**
- In `crates/ui/src/shell/mod.rs` & `app/vault.rs`: allow empty workspace without auto-generating a `Scratch` buffer.
- In `crates/ui/src/native/events.rs` & `shell/editor_input.rs`: when switching mode to `Mode::Normal`, invoke `application.save_dirty()`.
- In `crates/ui/src/native/mod.rs`: on `close_requested()` or `:q`, if `application.has_any_dirty_buffer()` is true, cancel immediate exit and display confirmation dialog ("Save changes before exit?").

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test e2e_workflow`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/shell/mod.rs crates/ui/src/app/vault.rs crates/ui/src/app/session.rs crates/ui/src/native/mod.rs crates/ui/src/native/creation/mod.rs crates/ui/tests/e2e_workflow.rs
git commit -m "feat(ui): remove startup scratch buffer, auto-save on normal mode, and confirm save on exit"
```

---

### Task 8: Obsidian-Style Right Sidebar Panel (Outline, Links, Backlinks)

**Files:**
- Modify: `crates/ui/src/native/backlinks.rs`
- Modify: `crates/ui/src/native/outline.rs`
- Modify: `crates/ui/src/native/content.rs`
- Modify: `crates/ui/src/native/mod.rs`
- Test: `crates/ui/tests/native.rs`

**Step 1: Write test for right dock tabs and click-to-navigate**
- Test that right panel renders Outline tabs, outgoing Links, and Backlinks, and clicking an item dispatches navigation.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test native`
Expected: FAIL

**Step 3: Implement right panel**
- Upgrade `crates/ui/src/native/backlinks.rs` to a full right dock with 3 selectable tabs:
  1. **Outline**: Headings tree (H1–H6) extracted from document facts, clickable to jump cursor to heading.
  2. **Links**: Outgoing wiki links from document facts, clickable to open target notes.
  3. **Backlinks**: Incoming references with line snippets.
- Ensure panel styling matches left sidebar geometry and theme.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test native`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/native/backlinks.rs crates/ui/src/native/outline.rs crates/ui/src/native/content.rs crates/ui/src/native/mod.rs crates/ui/tests/native.rs
git commit -m "feat(ui): implement obsidian-style right dock with outline, links, and backlinks tabs"
```

---

### Task 9: Full Workspace Verification & Quality Audit

**Files:**
- All touched files

**Step 1: Format check**
Run: `cargo fmt --all -- --check`
Expected: PASS with no diffs.

**Step 2: Check workspace targets**
Run: `cargo check --workspace --all-targets`
Expected: Compiles with 0 errors.

**Step 3: Full test suite**
Run: `cargo test --workspace`
Expected: All unit, integration, and UI tests pass.

**Step 4: Clippy audit**
Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: Clean with 0 warnings.

**Step 5: Commit**
```bash
git commit --allow-empty -m "chore: verify all workspace checks pass"
```
