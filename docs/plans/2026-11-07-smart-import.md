# Smart Note and Folder Import Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Implement smart import of notes and folders via `![[target]]!` syntax, replacing the macro in-place with note text (literal copy-paste mode) or `# Title` blocks for all `.md` files in a folder, with autocomplete support on `![[` and atomic undo.

**Architecture:** A pure core module `truff_core::markdown::smart_import` handles syntax detection and text formatting. `truff_ui` integrates vault file/folder resolution, self-import safety, autocomplete candidate generation for `![[`, and keystroke-triggered replacement on typing `!`.

**Tech Stack:** Rust, `truff_core` (transaction, markdown), `truff_ui` (application, overlay, events), `egui`.

---

### Task 1: Core Smart Import Module (`truff_core::markdown::smart_import`)

**Files:**
- Create: `crates/core/src/markdown/smart_import.rs`
- Modify: `crates/core/src/markdown/mod.rs`
- Create: `crates/core/tests/smart_import.rs`

**Step 1: Write the failing unit tests for core smart import**

```rust
// crates/core/tests/smart_import.rs
use truff_core::markdown::smart_import::{
    detect_import_macro, format_folder_import, FolderImportEntry, ImportOptions,
};

#[test]
fn test_detect_import_macro() {
    let line = "Here is ![[My Note]]! and more";
    let cursor = 22; // Right after trailing '!'
    let matched = detect_import_macro(line, 0, cursor).expect("should match macro");
    assert_eq!(matched.start, 8);
    assert_eq!(matched.end, 22);
    assert_eq!(matched.target, "My Note");

    // Negative tests
    assert!(detect_import_macro("![[Unclosed", 0, 11).is_none());
    assert!(detect_import_macro("[[Normal Link]]!", 0, 16).is_none());
    assert!(detect_import_macro("![[Empty]]!", 0, 5).is_none()); // cursor before '!'
}

#[test]
fn test_format_folder_import() {
    let entries = vec![
        FolderImportEntry {
            relative_path: "b.md".into(),
            title: "b".into(),
            content: "Content B".into(),
        },
        FolderImportEntry {
            relative_path: "a.md".into(),
            title: "a".into(),
            content: "Content A".into(),
        },
    ];
    let opts = ImportOptions::default();
    let formatted = format_folder_import(&entries, &opts);
    assert_eq!(
        formatted,
        "# a\n\nContent A\n\n# b\n\nContent B\n"
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-core --test smart_import`
Expected: FAIL with "unresolved import `truff_core::markdown::smart_import`"

**Step 3: Implement `truff_core::markdown::smart_import`**

Create `crates/core/src/markdown/smart_import.rs`:
- Define `ImportMacroMatch { pub start: usize, pub end: usize, pub target: String }`.
- Implement `detect_import_macro(line_str: &str, line_start: usize, cursor_in_line: usize) -> Option<ImportMacroMatch>`.
- Define `ImportOptions { pub include_frontmatter: bool }` with `Default`.
- Define `FolderImportEntry { pub relative_path: String, pub title: String, pub content: String }`.
- Implement `format_folder_import(entries: &[FolderImportEntry], options: &ImportOptions) -> String`:
  - Sorts entries by `relative_path` (alphabetical).
  - Emits `# {title}\n\n{content}\n\n`.
- Re-export in `crates/core/src/markdown/mod.rs`.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-core --test smart_import`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/core/src/markdown/smart_import.rs crates/core/src/markdown/mod.rs crates/core/tests/smart_import.rs
git commit -m "feat(core): add smart import macro detection and formatting"
```

---

### Task 2: Vault File & Folder Import Resolver (`truff_ui`)

**Files:**
- Modify: `crates/ui/src/app/completion.rs`
- Modify: `crates/ui/src/app/mod.rs`
- Modify: `crates/ui/src/overlays/link_completion.rs`

**Step 1: Write unit tests for resolver and link trigger detection**

Add tests to `crates/ui/src/overlays/link_completion.rs`:
```rust
#[test]
fn test_detect_import_trigger() {
    let buf = truff_core::Buffer::from_str("Hello ![[my note");
    let (trigger, query, is_import) = LinkCompletionOverlay::detect_trigger_mode(&buf, 16).unwrap();
    assert_eq!(trigger, 6);
    assert_eq!(query, "my note");
    assert!(is_import);

    let buf_normal = truff_core::Buffer::from_str("Hello [[my note");
    let (trigger2, query2, is_import2) = LinkCompletionOverlay::detect_trigger_mode(&buf_normal, 15).unwrap();
    assert_eq!(trigger2, 6);
    assert_eq!(query2, "my note");
    assert!(!is_import2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-ui link_completion::tests::test_detect_import_trigger`
Expected: FAIL

**Step 3: Implement `LinkCompletionOverlay::detect_trigger_mode` and `Application::resolve_smart_import`**

- In `crates/ui/src/overlays/link_completion.rs`:
  - Add `is_import: bool` field to `LinkCompletionOverlay`.
  - Provide `detect_trigger_mode(buffer: &Buffer, cursor: usize) -> Option<(usize, String, bool)>` detecting both `![[` and `[[`.
- In `crates/ui/src/app/completion.rs`:
  - Update `refresh_link_completions` to accept `is_import: bool`. When `is_import` is true:
    - Include note files from vault (`.md` stripped).
    - Include subfolders from vault directory tree (suffixed with `/`).
  - Add `Application::resolve_smart_import(&self, target: &str) -> Result<String, String>`:
    - Get vault root directory from `self.vault_path()`.
    - Compare target canonical path with `self.active_path()` to reject self-import.
    - If target is a file (or `target.md` exists): read content.
    - If target is a directory: recursively read `.md` files, exclude active file, sort alphabetically, format using `truff_core::markdown::smart_import::format_folder_import`.
  - Update `apply_link_completion`:
    - When `is_import` is true: call `resolve_smart_import(&candidate.target)`. Replace `trigger..cursor` directly with resolved text! If error, set `self.status` and dismiss.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui link_completion::tests::test_detect_import_trigger`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/ui/src/overlays/link_completion.rs crates/ui/src/app/completion.rs crates/ui/src/app/mod.rs
git commit -m "feat(ui): add smart import vault resolver and completion overlay mode"
```

---

### Task 3: In-line Typing Trigger (`!`) & Editor Event Handling

**Files:**
- Modify: `crates/ui/src/native/events.rs`
- Modify: `crates/ui/src/app/completion.rs`

**Step 1: Write integration test for typing trigger and smart import**

Create `crates/ui/tests/smart_import.rs`:
```rust
#[test]
fn test_typing_import_macro_replaces_with_note_content() {
    // Setup temporary vault with "source.md" containing "Hello World\nLine 2"
    // Open "dest.md"
    // Send insert mode text: "![[source]]!"
    // Verify "dest.md" content becomes "Hello World\nLine 2"
    // Send "u" in Normal mode -> verify reverts cleanly
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-ui --test smart_import`
Expected: FAIL

**Step 3: Implement trigger in `events.rs`**

- In `crates/ui/src/native/events.rs`:
  - When in `Mode::Insert`, after handling text insertion or `!` character:
    - Call `detect_import_macro` on the line up to the current head cursor.
    - If matched:
      - Call `self.application.resolve_smart_import(&matched.target)`.
      - On success: construct single `Transaction::replace(matched.start..matched.end, content)`.
      - Apply transaction to shell, publish changes, update vim cursor position to end of inserted text.
      - Dismiss completion popup.
      - If error: show message in status bar without replacing macro.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui --test smart_import`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/ui/src/native/events.rs crates/ui/tests/smart_import.rs
git commit -m "feat(ui): integrate smart import typing trigger and atomic replacement"
```

---

### Task 4: Full Integration Tests & Verification

**Files:**
- Modify: `crates/ui/tests/smart_import.rs`

**Step 1: Add comprehensive integration tests**
- `test_import_folder_recursive_alphabetical`: folder with `intro.md` and `sub/details.md` produces `# intro\n\n...\n\n# details\n\n...`.
- `test_import_self_prevention`: note trying to import itself or its parent folder excludes itself and displays warning.
- `test_import_via_completion_popup`: typing `![[` and selecting item from candidates expands immediately.
- `test_import_undo_redo`: single `u` reverts import in full, `Ctrl-R` redoes it.

**Step 2: Run integration tests**

Run: `cargo test -p truff-ui --test smart_import`
Expected: PASS

**Step 3: Run full workspace checks**

Run:
```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```
Expected: All checks exit code 0.

**Step 4: Commit**

```bash
git add crates/ui/tests/smart_import.rs
git commit -m "test(ui): add comprehensive test suite for smart note and folder import"
```
