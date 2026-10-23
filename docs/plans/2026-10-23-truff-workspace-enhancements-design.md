# Truff v2 Workspace & Editor Enhancements Design

**Date**: 2026-10-23  
**Status**: Approved  

---

## 1. Executive Summary

This design document specifies a cohesive set of workspace, editor, navigation, and UX enhancements for Truff v2. It addresses usability friction across note creation, link management, styling, file tree operations, list editing, application lifecycle, and inspection panels.

---

## 2. Detailed Requirements & Design

### 2.1 Editor, Markdown & Text Operations

1. **Soft-Tab Backspace (`crates/core/src/vim/execute/editing.rs`, `crates/ui/src/editor/input.rs`)**:
   - In Insert mode, pressing `Backspace` when the cursor is positioned after leading or soft-tab whitespace (e.g. 2 or 4 spaces matching tab stop boundaries) deletes the entire tab indentation step at once instead of requiring repeated single-space deletions.

2. **List Continuation De-duplication (`crates/core/src/markdown/smart_edit.rs`)**:
   - In `plan_enter`: when splitting a list item, inspect the remaining text on the line following `split_pos`.
   - If the trailing text already begins with a list marker of the same kind (`- `, `* `, `+ `, `1. `, `[ ] `), do not prepend a duplicate marker into the inserted string.

3. **Arrow Symbol Rendering (`crates/ui/src/editor/display/markers.rs`)**:
   - Extend inline marker replacements to render common ASCII arrow patterns as Unicode glyphs:
     - `-->` and `->` $\rightarrow$ `→`
     - `<--` and `<-` $\rightarrow$ `←`
     - `<-->` and `<->` $\rightarrow$ `↔`
     - `==>` $\rightarrow$ `⇒`
     - `<==>` $\rightarrow$ `⇔`

4. **Yank to OS Clipboard (`crates/ui/src/native/events.rs`, `crates/core/src/vim/execute/editing.rs`)**:
   - Any Vim yank action (`Operator::Yank`, `edit.yank_line`, visual mode `y`) immediately updates the OS system clipboard (`egui::Context::copy_text`), mirroring `clipboard=unnamedplus`.

5. **HTML Color Tag Newline Formatting (`crates/core/src/markdown/style.rs`)**:
   - In `apply_text_color`, when operating on ranges produced by line-wise selections (`VisualLine`) or selections ending with newline characters (`\r?\n`), strip the trailing newline from the styled span before wrapping.
   - Result: `<span style="color:#RRGGBB">Content</span>\n` rather than `<span style="color:#RRGGBB">Content\n</span>`.

6. **Vertical Indentation Guides & Prominent Horizontal Rules (`crates/ui/src/native/editor/paragraph/paint.rs`)**:
   - For multi-level / nested list items, render subtle vertical indent lines `|` corresponding to list depth, matching the tree style in the sidebar.
   - For horizontal rules `---` (`Event::Rule`), increase line stroke width to 1.5–2.0px and use a more visible theme boundary color (`border` / `accent` blend).

---

### 2.2 Links & Which-Key Color Menu

1. **`<Space> c` Which-Key Menu (`crates/ui/src/shell/routing.rs`, `crates/ui/src/overlays/which_key.rs`)**:
   - Pressing `<Space> c` in Normal or Visual mode presents a Which-Key submenu with:
     - `[r]` Red (`#bf616a`)
     - `[g]` Green (`#a3be8c`)
     - `[b]` Blue (`#81a1c1`)
     - `[y]` Yellow (`#ebcb8b`)
     - `[p]` Purple (`#b48ead`)
     - `[c]` Cyan (`#88c0d0`)
     - `[x]` Clear styling / reset
     - `[n]` Note Link (`[[target|alias]]`)

2. **Two-Stage Note Link Creation**:
   - Selecting `[n]` from the `<Space> c` menu executes a 2-stage workflow:
     - **Stage 1 (Alias / Display Name)**: Prompts for the display title. If the user submits without typing (Enter or Tab), it defaults to the currently active note's name (or the visually selected text).
     - **Stage 2 (Target Selection)**: Advances to a fuzzy-search target picker over vault notes.
     - Inserts the formatted link `[[target|alias]]` (or `[[target]]` if target equals alias) at the cursor position.

---

### 2.3 Note Creation & Dialogs

1. **Adjacent Note Creation Setting (`crates/ui/src/app/global_config.rs`, `crates/ui/src/native/creation/settings.rs`)**:
   - Add `create_notes_adjacent: bool` to `GlobalConfig` (persisted in config).
   - Add a checkbox to `SettingsDialog`: *"Создавать новые заметки рядом с активным файлом"*.
   - When enabled, creating a new note defaults its path prefix to the folder containing the currently open note (e.g. `folder/sub/`).
   - When disabled, defaults to the vault root or sidebar-selected directory.

2. **Auto-creation of Intermediate Directories (`crates/ui/src/app/commands/files.rs`, `crates/ui/src/app/vault.rs`)**:
   - When creating a note whose specified path includes nested non-existent directories, automatically create parent directories using `std::fs::create_dir_all`.

3. **Input Caret at End of Text in Creation Dialogs (`crates/ui/src/native/creation/file.rs`, `folder.rs`)**:
   - When opening file/folder creation dialogs with a prefilled folder prefix, set the `egui::TextEdit` cursor position to the end of the text so typing begins immediately after the trailing slash.

4. **Enter Key Confirmation Across All Dialogs (`crates/ui/src/native/creation/`)**:
   - In all dialogs (`FileDialog`, `FolderDialog`, `VaultDialog`, `OperationDialog`), pressing `Enter` triggers the positive submission action (Create / Move to Trash / OK), while `Esc` cancels.

---

### 2.4 File Tree & Vault Management

1. **Tree File Movement via Vim Motions and Mouse (`crates/ui/src/native/sidebar.rs`, `crates/ui/src/shell/`)**:
   - **Vim Motions**: In `Focus::Sidebar`, `d` or `x` marks the selected file for cut (visual indicator); navigating to a target folder and pressing `p` moves the file into that folder. `m` opens the quick move dialog.
   - **Mouse**: Enable row dragging; dropping a file row onto a folder row triggers an atomic note move operation via `Application::rename_path`.

2. **Focus Retention on Active File (`crates/ui/src/shell/panels.rs`, `crates/ui/src/native/sidebar.rs`)**:
   - When focusing `Focus::Sidebar`, automatically locate the currently active note in the tree, expand any collapsed ancestor folders, and set `selected_index` on the active file.

3. **Elimination of Startup Scratch Buffer (`crates/ui/src/shell/mod.rs`, `crates/ui/src/app/session.rs`, `crates/ui/src/main.rs`)**:
   - Remove automatic creation of an empty `Scratch` buffer when launching without an open note.
   - Present a clean welcome state or empty workspace instead of a placeholder tab.

4. **Modern Vault Selector Header (`crates/ui/src/native/sidebar.rs`)**:
   - Replace the legacy static icon button with a clickable vault header at the top of the sidebar showing the current vault name and a chevron icon.
   - Clicking opens a popover menu listing recent vaults, "Open Folder...", and "New Vault...".

---

### 2.5 Lifecycle & Right Panel

1. **Auto-save on Transition to Normal Mode (`crates/ui/src/shell/editor_input.rs`, `crates/ui/src/app/mod.rs`)**:
   - When returning from `Insert` to `Normal` mode (via `Esc` or `Ctrl+[`), trigger `save_dirty` on the active buffer.

2. **Confirm Save on Exit Dialog (`crates/ui/src/native/mod.rs`, `crates/ui/src/native/creation/mod.rs`)**:
   - When closing the application window or issuing `:q` while unsaved buffers exist, intercept the close event and present a confirmation dialog:
     *"Есть несохранённые изменения. Сохранить перед выходом? [Сохранить и выйти] [Выйти без сохранения] [Отмена]"*.

3. **Obsidian-Style Right Panel (`crates/ui/src/native/backlinks.rs`, `crates/ui/src/native/outline.rs`)**:
   - Implement a permanent or toggleable right sidebar dock with three tabs:
     - **Outline**: Full hierarchical headings tree (H1–H6) extracted from document facts, clickable to navigate cursor directly to the heading line.
     - **Links**: Outgoing wiki links from the current note, clickable to open target notes.
     - **Backlinks**: Incoming references to the active note.

---

## 3. Verification Strategy

1. **Unit & Core Tests**:
   - `core::markdown::smart_edit`: test `plan_enter` with de-duplication of `- `.
   - `core::markdown::style`: test `apply_text_color` with line selections avoiding trailing newline inclusion.
   - `core::vim`: test soft-tab deletion via Backspace.
2. **UI & State Integration Tests**:
   - Config persistence of `create_notes_adjacent`.
   - Vault header dropdown and dialog Enter confirmation.
   - Which-key `<Space> c` menu routing and link creation workflow.
   - Right dock Outline, Links, and Backlinks rendering and mouse navigation.
3. **Workspace Check**:
   - `cargo check --workspace --all-targets`
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
