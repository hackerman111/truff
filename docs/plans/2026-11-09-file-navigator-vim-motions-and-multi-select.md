# File Navigator Vim Motions and Multi-Select Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Implement Vim motions, visual range selection (`v`/`V`), toggle marks (`x`), and register clipboard operations (`y` copy, `m` move, `p` paste, `d` delete, `r` rename) in the `Space f` Universal Navigator file window (`NavigatorMode::Files`).

**Architecture:** Extend `NavigatorState` in `crates/ui` with visual anchor, marked indices, and file clipboard state. Dispatch Vim motions and operator keys in `navigator/input.rs`. Connect paste and delete actions to transactional filesystem and link-rewrite operations in `truff_store` and `crates/ui/src/app`. Render visual selection highlights, mark badges, and dynamic footer status in `navigator/render/`.

**Tech Stack:** Rust (edition 2024), `egui`, `truff_core::vim`, `truff_store` (SQLite, `FileOpPlan`).

---

### Task 1: Core Selection & Clipboard State in `NavigatorState`

**Files:**
- Modify: `crates/ui/src/navigator/types.rs`
- Modify: `crates/ui/src/navigator/model.rs`
- Create: `crates/ui/tests/navigator_selection.rs`

**Step 1: Write the failing test**

```rust
// crates/ui/tests/navigator_selection.rs
use truff_ui::navigator::{ClipboardOp, FileClipboard, NavigatorItem, NavigatorItemPayload, NavigatorMode, NavigatorState};

#[test]
fn test_navigator_selection_model() {
    let mut state = NavigatorState::new();
    state.open_mode(
        NavigatorMode::Files,
        "Vault",
        vec![
            NavigatorItem::new("a.md", NavigatorItemPayload::File { path: "a.md".into(), is_dir: false }),
            NavigatorItem::new("b.md", NavigatorItemPayload::File { path: "b.md".into(), is_dir: false }),
            NavigatorItem::new("c.md", NavigatorItemPayload::File { path: "c.md".into(), is_dir: false }),
        ],
    );

    assert!(!state.in_visual_mode());
    assert_eq!(state.selected_file_paths(), vec!["a.md"]);

    // Enter visual mode at index 0, move to index 2
    state.visual_anchor = Some(0);
    state.selected_index = 2;
    assert!(state.in_visual_mode());
    assert!(state.is_index_selected(0));
    assert!(state.is_index_selected(1));
    assert!(state.is_index_selected(2));
    assert_eq!(state.selected_file_paths(), vec!["a.md", "b.md", "c.md"]);

    // Toggle mark on index 1 and clear visual mode
    state.visual_anchor = None;
    state.marked_indices.insert(1);
    assert!(!state.in_visual_mode());
    assert!(state.is_index_selected(1));
    assert_eq!(state.selected_file_paths(), vec!["b.md"]);

    // Clear selection
    state.clear_selection();
    assert_eq!(state.marked_indices.len(), 0);
    assert!(state.visual_anchor.is_none());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test navigator_selection`
Expected: FAIL with missing fields/methods on `NavigatorState`

**Step 3: Write minimal implementation**

In `crates/ui/src/navigator/types.rs`:
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
```

In `crates/ui/src/navigator/model.rs`:
Add fields to `NavigatorState`:
```rust
    pub visual_anchor: Option<usize>,
    pub marked_indices: std::collections::BTreeSet<usize>,
    pub clipboard: Option<FileClipboard>,
```
And methods:
```rust
    pub fn in_visual_mode(&self) -> bool {
        self.visual_anchor.is_some()
    }

    pub fn is_index_selected(&self, index: usize) -> bool {
        if self.marked_indices.contains(&index) {
            return true;
        }
        if let Some(anchor) = self.visual_anchor {
            let start = anchor.min(self.selected_index);
            let end = anchor.max(self.selected_index);
            return index >= start && index <= end;
        }
        false
    }

    pub fn clear_selection(&mut self) {
        self.visual_anchor = None;
        self.marked_indices.clear();
    }

    pub fn selected_file_paths(&self) -> Vec<String> {
        let mut paths = Vec::new();
        let total = self.visible_count();
        for i in 0..total {
            if self.is_index_selected(i) {
                if let Some(item) = self.item_at(i) {
                    if let NavigatorItemPayload::File { path, .. } = &item.payload {
                        paths.push(path.clone());
                    }
                }
            }
        }
        if paths.is_empty() {
            if let Some(item) = self.selected_item() {
                if let NavigatorItemPayload::File { path, .. } = &item.payload {
                    paths.push(path.clone());
                }
            }
        }
        paths
    }
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test navigator_selection`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/ui/src/navigator/ crates/ui/tests/navigator_selection.rs
git commit -m "feat(ui): add visual selection and clipboard state to NavigatorState"
```

---

### Task 2: Keyboard Motions Dispatch & Actions

**Files:**
- Modify: `crates/ui/src/navigator/types.rs`
- Modify: `crates/ui/src/navigator/input.rs`
- Modify: `crates/ui/src/navigator/actions.rs`
- Modify: `crates/ui/tests/navigator_selection.rs`

**Step 1: Write the failing test**

```rust
// Append to crates/ui/tests/navigator_selection.rs
#[test]
fn test_navigator_keyboard_motions_and_clipboard() {
    let mut state = NavigatorState::new();
    state.open_mode(
        NavigatorMode::Files,
        "Vault",
        vec![
            NavigatorItem::new("a.md", NavigatorItemPayload::File { path: "a.md".into(), is_dir: false }),
            NavigatorItem::new("b.md", NavigatorItemPayload::File { path: "b.md".into(), is_dir: false }),
            NavigatorItem::new("c.md", NavigatorItemPayload::File { path: "c.md".into(), is_dir: false }),
            NavigatorItem::new("folder", NavigatorItemPayload::File { path: "folder".into(), is_dir: true }),
        ],
    );

    // Press 'v' to enter visual mode
    state.handle_key_event(truff_ui::navigator::NavKey::Char('v'), false);
    assert!(state.in_visual_mode());

    // Press 'j' to move down -> selects a.md and b.md
    state.handle_key_event(truff_ui::navigator::NavKey::Char('j'), false);
    assert_eq!(state.selected_index, 1);
    assert_eq!(state.selected_file_paths(), vec!["a.md", "b.md"]);

    // Press 'y' to yank -> copies into clipboard and exits visual mode
    state.handle_key_event(truff_ui::navigator::NavKey::Char('y'), false);
    assert!(!state.in_visual_mode());
    assert_eq!(state.clipboard.as_ref().unwrap().op, ClipboardOp::Copy);
    assert_eq!(state.clipboard.as_ref().unwrap().paths, vec!["a.md", "b.md"]);

    // Move to folder (index 3) and press 'p' -> triggers paste into folder
    state.selected_index = 3;
    let action = state.handle_key_event(truff_ui::navigator::NavKey::Char('p'), false);
    assert!(matches!(action, truff_ui::navigator::NavigatorAction::PasteFiles { .. }));

    // Press 'x' on index 3 -> marks index 3 and moves to next line
    state.handle_key_event(truff_ui::navigator::NavKey::Char('x'), false);
    assert!(state.marked_indices.contains(&3));

    // Press Escape -> clears selection without closing
    let action = state.handle_key_event(truff_ui::navigator::NavKey::Escape, false);
    assert_eq!(action, truff_ui::navigator::NavigatorAction::None);
    assert!(!state.is_index_selected(3));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test navigator_selection`
Expected: FAIL

**Step 3: Implement minimal code**

In `crates/ui/src/navigator/types.rs`:
Add new `NavigatorAction` variants:
```rust
    PasteFiles {
        target_dir: String,
        op: ClipboardOp,
        paths: Vec<String>,
    },
    DeleteFiles {
        paths: Vec<String>,
    },
    BatchRenameFiles {
        paths: Vec<String>,
    },
```

In `crates/ui/src/navigator/input.rs`:
Handle `v`/`V`, `x`, `Escape`, `y`, `m`, `p`, `d`, `r` in `handle_normal_mode_key`.

**Step 4: Run test to verify it passes**

Run: `cargo test --test navigator_selection`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/ui/src/navigator/ crates/ui/tests/navigator_selection.rs
git commit -m "feat(ui): add vim motions visual selection and clipboard key dispatch"
```

---

### Task 3: Backend Operations Execution in `Application`

**Files:**
- Modify: `crates/ui/src/app/commands/files.rs`
- Modify: `crates/ui/src/native/navigator/mod.rs`
- Modify: `crates/ui/src/native/events/actions.rs`
- Test: `crates/ui/tests/navigator_workflows.rs`

**Step 1: Write the failing test**

```rust
// In crates/ui/tests/navigator_workflows.rs
// Test handling of NavigatorAction::PasteFiles for Move and Copy
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test navigator_workflows`
Expected: FAIL

**Step 3: Implement minimal code**

In `crates/ui/src/app/commands/files.rs`:
- Implement `paste_files(&mut self, target_dir: &str, op: ClipboardOp, paths: &[String]) -> AppResult<()>`:
  - If `op == ClipboardOp::Move`: for each path, compute new destination `target_dir / filename` and invoke `rename_path`.
  - If `op == ClipboardOp::Copy`: for each path, copy file and register in store.
- In `crates/ui/src/native/navigator/mod.rs`: dispatch `NavigatorAction::PasteFiles`, `NavigatorAction::DeleteFiles`, and `NavigatorAction::BatchRenameFiles`.

**Step 4: Run test to verify it passes**

Run: `cargo test --test navigator_workflows`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/ui/src/app/commands/files.rs crates/ui/src/native/navigator/
git commit -m "feat(ui): implement file paste and batch operations in application"
```

---

### Task 4: UI Rendering (Row Highlighting, Badges, Footer)

**Files:**
- Modify: `crates/ui/src/navigator/render/list.rs`
- Modify: `crates/ui/src/navigator/render/chrome.rs`
- Modify: `crates/ui/src/theme/style.rs`

**Step 1: Write test/check for rendering helpers**

Verify compilation and styling properties.

**Step 2: Implement minimal UI updates**

In `crates/ui/src/navigator/render/list.rs`:
- Check `state.is_index_selected(index)`. If true, apply `theme.selection_bg` fill or accent highlight.
- If `state.marked_indices.contains(&index)`, draw `[✓]` badge.

In `crates/ui/src/navigator/render/chrome.rs`:
- If `state.in_visual_mode()`, render `VISUAL (N selected)` badge.
- If `state.clipboard.is_some()`, render `[Buffer: Move/Copy N files · p: Paste]` badge.

**Step 3: Run UI tests**

Run: `cargo test -p truff_ui`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/ui/src/navigator/render/
git commit -m "feat(ui): render visual selection highlights and clipboard status in navigator"
```

---

### Task 5: Batch Trash & Batch Rename Dialogs

**Files:**
- Modify: `crates/ui/src/native/creation/operation.rs`
- Modify: `crates/ui/tests/gui.rs`

**Step 1: Write the failing test**

Test `OperationDialog` opening with `BatchTrash` and `BatchRename`.

**Step 2: Run test to verify it fails**

Run: `cargo test --test gui`
Expected: FAIL

**Step 3: Implement minimal code**

In `crates/ui/src/native/creation/operation.rs`:
- Add `Operation::BatchTrash { paths: Vec<String> }`.
- Add `Operation::BatchRename { paths: Vec<String>, pattern: String, replace: String }`.
- Render preview table and Enter confirmation.

**Step 4: Run test to verify it passes**

Run: `cargo test --test gui`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/ui/src/native/creation/operation.rs crates/ui/tests/gui.rs
git commit -m "feat(ui): add batch trash and batch rename dialogs"
```

---

### Task 6: Full Workspace Verification

**Step 1: Run workspace checks**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

**Step 2: Commit any final touch-ups**

```bash
git commit -m "chore: workspace verification for file navigator vim motions"
```
