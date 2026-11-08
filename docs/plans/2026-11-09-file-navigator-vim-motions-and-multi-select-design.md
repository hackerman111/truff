# File Navigator Vim Motions, Multi-Selection, and File Operations Design

## Overview

Support full Vim motion navigation, visual multi-selection (`v`/`V`, `x`), and clipboard-based file operations (copy with `y`, move/cut with `m`, paste with `p`, delete with `d`, rename/batch-rename with `r`) in the Truff v2 `Space f` Universal Navigator file window (`NavigatorMode::Files`).

## Goals

1. **Vim Visual Selection & Marks**:
   - `v` / `V`: Toggle visual range selection mode.
   - Motions (`j`, `k`, `count + j/k`, `G`, `gg`, `PageUp`, `PageDown`) dynamically expand the visual range `[min(anchor, current)..=max(anchor, current)]`.
   - `x`: Toggle mark for individual files/folders and move down one row (supports cherry-picking non-contiguous items or combining with visual mode).
   - `Escape`: Clear active selection/marks without closing the navigator; a second `Escape` closes the navigator.

2. **Vim Register File Operations (Yank / Cut / Paste)**:
   - `y` (Yank / Copy): Copy selected files/folders to the file clipboard (`ClipboardOp::Copy`).
   - `m` (Move / Cut): Cut selected files/folders for moving (`ClipboardOp::Move`).
   - `p` (Paste / Put): Paste into the current target folder (into the highlighted folder, or parent directory if highlighted item is a file).
     - Moving executes safe atomic rename/move via `FileOpPlan` and updates all Markdown `[[wikilinks]]` across the vault.
     - Copying duplicates files safely with collision handling (e.g. `(copy)` suffix).

3. **Delete & Rename**:
   - `d`: Delete single file or prompt with confirmation dialog for multiple selected files (`BatchTrash`), moving them safely to `.trash`.
   - `r`: Rename single file using the standard dialog, or open `BatchRename` dialog with pattern/replace and numbering `{n}` live preview for multiple selected files.

4. **UI & Visual Feedback**:
   - Highlight selected rows with selection background tint (`theme.selection_bg`).
   - Display `[✓]` badges for marked items.
   - Show dynamic footer status: `VISUAL (N selected)` and clipboard status pills (`[Buffer: Move N files | p: Paste here]`).

## Architecture & Data Structures

### 1. Navigator State (`crates/ui/src/navigator/model.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardOp {
    Copy,
    Move,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileClipboard {
    pub op: ClipboardOp,
    pub paths: Vec<String>,
}

pub struct NavigatorState {
    // ... existing fields ...
    pub visual_anchor: Option<usize>,
    pub marked_indices: std::collections::BTreeSet<usize>,
    pub clipboard: Option<FileClipboard>,
}
```

### 2. Selection Helper Methods

- `in_visual_mode(&self) -> bool`: Checks if `visual_anchor.is_some()`.
- `is_index_selected(&self, index: usize) -> bool`: Checks whether `index` is in visual range or in `marked_indices`.
- `selected_file_paths(&self) -> Vec<String>`: Returns deduplicated list of relative paths for all selected items, or single path under cursor if no multi-selection.
- `clear_selection(&mut self)`: Clears `visual_anchor` and `marked_indices`.
- `has_multi_selection(&self) -> bool`: True if more than one item is selected.

### 3. Keybindings Dispatch (`crates/ui/src/navigator/input.rs`)

| Key | Mode / Context | Action |
|---|---|---|
| `v` / `V` | Normal mode (`Files` / `Folders`) | Toggle visual range selection (`visual_anchor`). |
| `x` | Normal mode (`Files` / `Folders`) | Toggle mark on current row and advance cursor by 1. |
| `Escape` | Selection active | Clear selection (`clear_selection()`), keep window open. |
| `Escape` | Selection empty | Close navigator (`NavigatorAction::Close`). |
| `j`/`k`/motions | Visual mode active | Move cursor and expand visual selection range. |
| `y` | Selection active | Yank/copy selected paths to `clipboard`. |
| `m` | Selection active | Cut/mark selected paths for move in `clipboard`. |
| `p` | Clipboard active | Paste files into target folder. |
| `d` | Item(s) selected | Trash single file or open batch trash confirmation. |
| `r` | Item(s) selected | Rename single file or open batch rename dialog. |

### 4. Actions & Dialogs (`crates/ui/src/navigator/types.rs`, `native/creation/operation.rs`)

- `NavigatorAction::PasteFiles { target_dir: String, op: ClipboardOp, paths: Vec<String> }`
- `NavigatorAction::DeleteFiles { paths: Vec<String> }`
- `NavigatorAction::BatchRenameFiles { paths: Vec<String> }`
- Dialogs:
  - `OperationDialog::BatchTrash { paths: Vec<String> }`
  - `OperationDialog::BatchRename { paths: Vec<String>, pattern: String, replace: String }`

### 5. Backend Store Integration (`crates/store`, `crates/ui/src/app`)

- Moving files uses `FileOpPlan::move_path` or `CatalogJob::Rename`, updating all incoming backlinks and open buffers.
- Copying files creates new entries and registers them with the catalog indexer.
- Trashing uses `FileOpPlan::trash` / `CatalogJob::Trash`.

## Error Handling & Edge Cases

- **Cycle Prevention**: Cannot move a folder into itself or its subdirectories.
- **Collision Handling**: Moving warns if destination exists; copying automatically appends numeric copy suffix.
- **Dirty Buffers**: Files with unsaved changes in editor buffers are validated and preserved without data loss.
- **Batch Rename Collisions**: Live preview detects conflicting target names and blocks apply until resolved.

## Verification & Testing

- Unit tests for visual mode transitions, vim motions (`3j`, `G`, `gg`), marks (`x`), yank/cut/paste actions.
- Store integration tests for link rewriting when multiple files are moved.
- Strict workspace checks: `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`.
