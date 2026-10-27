# UX & Editor Improvements Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Implement 11 key UX, vim motion, and visual improvements in Truff v2 (continuous contrasting sidebar tree guides, smart backspace for tabs, vim yank to clipboard, auto-creation in folder, enter-as-confirm, auto-save on normal mode, exit save confirmation, arrow rendering, scratch cleanup, and distinct horizontal rules).

**Architecture:** 
- `truff-core`: Add `plan_backspace` in `smart_edit.rs` for atomic unindent; track `last_yanked` in `VimState`.
- `truff-ui`: 
  - Sidebar: extend indentation lines across `item_spacing.y` with high contrast; use creation folder parent; focus active note on sidebar open.
  - Editor: render arrows `-->` to `→` in `DisplayMap`; thicken `---` in paragraph paint; synchronize yanked text to egui clipboard; auto-save on `Insert -> Normal`.
  - App & Dialogs: Enter as default submit in dialogs with caret at end of filename; close scratch buffer on vault open; show save-on-exit dialog.

**Tech Stack:** Rust 2024, egui 0.36, truff-core, truff-ui.

---

### Task 1: Continuous & Distinct Sidebar Tree Guide Lines (`|`)
- **Modify:** `crates/ui/src/native/sidebar.rs:288-305`
- **Verify:** `cargo test -p truff-ui --test native sidebar_frame_response_toggles_folders_and_opens_files`

### Task 2: Smart Backspace in Insert Mode (Tab / Indent Deletion)
- **Modify:** `crates/core/src/markdown/smart_edit.rs`
- **Modify:** `crates/ui/src/shell/routing.rs:491-515`
- **Test:** `crates/core/tests/smart_edit.rs` or `crates/core/tests/markdown.rs`
- **Verify:** `cargo test -p truff-core`

### Task 3: Vim Motion Yank to Clipboard
- **Modify:** `crates/core/src/vim/mod.rs` & `crates/core/src/vim/execute/editing.rs`
- **Modify:** `crates/ui/src/native/events.rs`
- **Test:** `crates/core/tests/vim.rs` & `crates/ui/tests/native.rs`

### Task 4: Auto-create Notes in Selected Folder & Caret at End of Filename
- **Modify:** `crates/ui/src/native/sidebar.rs` (prepopulate folder on `+`)
- **Modify:** `crates/ui/src/native/creation/file.rs` (set cursor to end of string)

### Task 5: Enter as Positive Action in Dialogs
- **Modify:** `crates/ui/src/native/creation/file.rs`, `folder.rs`, `operation.rs`
- **Verify:** `cargo test -p truff-ui --test settings_dialog`

### Task 6: Auto-save on Exit to Normal Mode
- **Modify:** `crates/ui/src/native/events.rs`
- **Verify:** `cargo test -p truff-ui --test native`

### Task 7: Confirm Save Changes Before Exit
- **Modify:** `crates/ui/src/native/creation/mod.rs` & new quit confirmation dialog or `operation.rs`
- **Modify:** `crates/ui/src/native/mod.rs`

### Task 8: Render Arrows `-->` to `→`
- **Modify:** `crates/ui/src/editor/display/markers.rs`
- **Test:** `crates/ui/tests/display.rs`

### Task 9: Caret on Active File When Opening Sidebar
- **Modify:** `crates/ui/src/shell/panels.rs` / `crates/ui/src/native/events.rs` / `sidebar.rs`

### Task 10: Clean Up Scratch Buffer on Startup
- **Modify:** `crates/ui/src/app/vault.rs` & `crates/ui/src/app/session.rs`

### Task 11: Prominent Horizontal Rule `---`
- **Modify:** `crates/ui/src/native/editor/paragraph/paint.rs`
- **Test:** `crates/ui/tests/display.rs`
