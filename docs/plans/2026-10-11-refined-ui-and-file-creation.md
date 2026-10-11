# Refined UI Hierarchy & Creation Dialogs Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Elevate Truff's aesthetics to Obsidian/Zed standards with a four-tier palette, seamless tabs, distinct file tree states, 48 px editor top margin, and modal UI for creating notes, creating folders, and opening vaults.

**Architecture:** Update `Theme` design tokens in `crates/ui/src/theme.rs`; implement seamless tab styling and hover-only close buttons in `crates/ui/src/native/mod.rs`; add note and folder creation dialogs in `crates/ui/src/native/mod.rs` backed by `Application`; enhance tree item rendering with active/selected/hover distinctions; adjust editor top breathing room in `crates/ui/src/native/editor.rs`.

**Tech Stack:** Rust (edition 2024), eframe / egui 0.36.2, truff-core, truff-store, truff-ui.

---

### Task 1: Four-Tier Palette and Theme Design Tokens

**Files:**
- Modify: `crates/ui/src/theme.rs:114-142`
- Test: `crates/ui/tests/editor_input.rs`

**Step 1: Write test verifying four-tier palette hex values**

In `crates/ui/tests/editor_input.rs`, add a test asserting that `Theme::nord()` provides the new four-tier palette:
- `bg_app` / `bg_secondary`: `#20242C` / `#252A34`
- `bg_primary`: `#2D333F`
- `selection`: `#394351`
- `border`: `#333945`
- `accent`: `#88A9C1`
- `text_primary`: `#E2E6ED`
- `text_muted`: `#8B95A5`

**Step 2: Run test to verify failure**

Run: `cargo test -p truff-ui --test editor_input test_four_tier_palette_tokens`
Expected: FAIL.

**Step 3: Implement minimal changes**

Update `Theme::nord()` in `crates/ui/src/theme.rs` with the exact hex values.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui --test editor_input test_four_tier_palette_tokens`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/ui/src/theme.rs crates/ui/tests/editor_input.rs
git commit -m "feat(ui): update Theme tokens to four-tier Obsidian/Zed palette"
```

---

### Task 2: Seamless Connected Tabs with Hover-Only Close Buttons

**Files:**
- Modify: `crates/ui/src/native/mod.rs:72-170`
- Test: `crates/ui/tests/gui.rs`

**Step 1: Write GUI test for tab hover states**

In `crates/ui/tests/gui.rs`, add test verifying that tab close button is conditionally rendered and active tab has matching background with the editor.

**Step 2: Run test to verify failure / baseline**

Run: `cargo test -p truff-ui --test gui test_tab_close_button_hover`
Expected: FAIL.

**Step 3: Implement minimal changes**

In `crates/ui/src/native/mod.rs`:
- Render tab item with interactive response:
  - Check `is_hovered = tab_response.hovered()`.
  - Only show `small_button("×")` when `is_hovered` or when dirty dot isn't enough.
  - Active tab uses `theme.bg_primary` (`#2D333F`), with a 2 px bottom accent line `#88A9C1`, seamlessly connecting with the editor panel below.
  - Inactive tabs show a 1 px divider between them.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui --test gui test_tab_close_button_hover`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/ui/src/native/mod.rs crates/ui/tests/gui.rs
git commit -m "feat(ui): render seamless connected tabs with hover-only close buttons"
```

---

### Task 3: Sidebar Header Action Buttons & Creation Modals

**Files:**
- Modify: `crates/ui/src/native/mod.rs:200-380`
- Test: `crates/ui/tests/gui.rs`

**Step 1: Write test for creating notes and folders via UI**

In `crates/ui/tests/gui.rs`, add test verifying that opening the new note dialog, entering `"subfolder/new_note.md"`, and submitting creates the file and opens it in the workspace.

**Step 2: Run test to verify failure**

Run: `cargo test -p truff-ui --test gui test_create_note_and_folder_dialogs`
Expected: FAIL.

**Step 3: Implement minimal changes**

In `crates/ui/src/native/mod.rs`:
- Replace old `+` button in `VAULT` header with action buttons: `+📄`, `+📁`, `📂`, `◫`.
- Add dialog state fields to `NativeApp`: `show_create_file_dialog`, `create_file_input`, `show_create_folder_dialog`, `create_folder_input`.
- Render modal dialogs for New Note and New Folder.
- On submit:
  - For file: create directory if necessary, write file to disk, call `application.open_note(&path)`.
  - For folder: call `fs::create_dir_all`.
  - Trigger catalog reload.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui --test gui test_create_note_and_folder_dialogs`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/ui/src/native/mod.rs crates/ui/tests/gui.rs
git commit -m "feat(ui): add note and folder creation dialogs and header actions"
```

---

### Task 4: Distinct Tree Item States (Active, Selected, Hover, Modified)

**Files:**
- Modify: `crates/ui/src/native/mod.rs:320-380`
- Test: `crates/ui/tests/navigation.rs`

**Step 1: Write test for active vs selected row styling**

In `crates/ui/tests/navigation.rs`, test that the active file (currently open in editor) is distinct from a mere row selection.

**Step 2: Run test to verify failure**

Run: `cargo test -p truff-ui --test navigation test_active_vs_selected_file_state`
Expected: FAIL.

**Step 3: Implement minimal changes**

In `crates/ui/src/native/mod.rs`:
- Determine `is_active = row.data.as_deref() == self.application.active_relative_path().as_deref()`.
- If `is_active`: draw left 2 px accent stroke `#88A9C1` and highlight text in `#E2E6ED`.
- If `is_selected`: draw background `#394351` with 4 px corner radius.
- Check if file has unsaved changes in buffer (`is_dirty`): draw `●` in `#88A9C1`.
- Use clean monochrome icons `▸ 📁` / `▾ 📁` and `📄`.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui --test navigation test_active_vs_selected_file_state`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/ui/src/native/mod.rs crates/ui/tests/navigation.rs
git commit -m "feat(ui): render distinct active, selected, hover, and modified tree states"
```

---

### Task 5: 48 px Top Margin & 720 px Document Column in Editor + Full Verification

**Files:**
- Modify: `crates/ui/src/native/editor.rs:46-55,340-365`
- Test: `crates/ui/tests/display.rs`

**Step 1: Write test verifying 720 px max column width and top margin**

In `crates/ui/tests/display.rs`, test that `calculate_column_metrics` returns max width 720.0 px and origin y has 48.0 px breathing room.

**Step 2: Run test to verify failure**

Run: `cargo test -p truff-ui --test display test_editor_readable_column_and_margin`
Expected: FAIL.

**Step 3: Implement minimal changes**

In `crates/ui/src/native/editor.rs`:
- Change `calculate_column_metrics` to clamp width to `720.0`.
- Add `const TOP_MARGIN: f32 = 48.0;` to `origin.y = rect.top() + TOP_MARGIN + ...`.
- Run full checks:
  ```bash
  cargo fmt --all -- --check
  cargo check --workspace --all-targets
  cargo test --workspace
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  ```

**Step 4: Verify all checks pass**

Expected: ALL PASS with 0 warnings.

**Step 5: Commit**

```bash
git add crates/ui/src/native/editor.rs crates/ui/tests/display.rs
git commit -m "feat(ui): add 48px editor top breathing room and clamp column to 720px"
```
