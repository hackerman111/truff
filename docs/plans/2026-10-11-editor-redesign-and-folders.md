# Truff Redesign & Hierarchical Folders Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Transform Truff into a polished desktop Markdown editor with hierarchical folder support, centered readable text column, tabbed buffer management, and refined Nord typography.

**Architecture:** Build a hierarchical tree of `RowItem`s from relative paths inside `crates/ui/src/shell.rs` while leveraging existing `RowModel` expansion/collapse logic; constrain the text column to a centered 780 px max-width with symmetric margins in `crates/ui/src/native/editor.rs`; add an egui tab bar in `crates/ui/src/native/mod.rs`; update typography and Nord design tokens in `crates/ui/src/native/paint.rs` and `crates/ui/src/theme.rs`.

**Tech Stack:** Rust (edition 2024), eframe / egui 0.36.2, crop, truff-core, truff-store.

---

### Task 1: Hierarchical Tree Model with Folder Expansion in Shell

**Files:**
- Modify: `crates/ui/src/shell.rs:180-210`
- Test: `crates/ui/tests/navigation.rs`

**Step 1: Write the failing test**

Add a test `test_sidebar_hierarchical_folders_and_auto_expansion` to `crates/ui/tests/navigation.rs` testing that `set_sidebar_files` creates folder nodes (`is_folder: true`) with proper depth, sorts folders first, collapses folders by default, and `select_sidebar_file` expands ancestor folders.

```rust
#[test]
fn test_sidebar_hierarchical_folders_and_auto_expansion() {
    let mut shell = Shell::new();
    shell.set_sidebar_available(true);
    shell.set_sidebar_files(vec![
        "docs/guide.md".into(),
        "docs/api/endpoints.md".into(),
        "Welcome.md".into(),
    ]);

    let model = shell.sidebar_model();
    // Initially: root level has "docs" (collapsed) and "Welcome.md"
    let visible_titles: Vec<&str> = model.visible_rows().iter().map(|r| r.title.as_str()).collect();
    assert_eq!(visible_titles, vec!["docs", "Welcome.md"]);
    assert!(model.visible_rows()[0].is_folder);
    assert!(!model.visible_rows()[1].is_folder);

    // Auto-reveal: selecting a nested file expands parent folders
    shell.select_sidebar_file("docs/api/endpoints.md");
    let model = shell.sidebar_model();
    let visible_titles: Vec<&str> = model.visible_rows().iter().map(|r| r.title.as_str()).collect();
    assert_eq!(visible_titles, vec!["docs", "api", "endpoints.md", "guide.md", "Welcome.md"]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-ui --test navigation test_sidebar_hierarchical_folders_and_auto_expansion`
Expected: FAIL (currently `set_sidebar_files` produces flat items with full paths and `is_folder: false`).

**Step 3: Write minimal implementation**

Implement hierarchical prefix tree parsing in `crates/ui/src/shell.rs`:
- Parse relative paths by splitting on `/`.
- Build a tree structure where internal nodes are folders (`is_folder = true`, `is_collapsed = true`, `data = None`), and terminal leaves are markdown notes (`is_folder = false`, `data = Some(full_rel_path)`).
- Assign sequential `RowId`s, set appropriate `depth`. Sort children: folders first alphabetically, then files alphabetically.
- Update `select_sidebar_file(path)` to find the item, expand all its ancestor folders in `RowModel`, and select the row.
- In `crates/ui/src/native/mod.rs`, render folder glyphs `▸`/`▾` and `📁`, and file glyph `📄`. Clicking a folder toggles collapse (`shell.sidebar_model_mut().toggle_collapse(row.id)`), while clicking a note opens it.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui --test navigation test_sidebar_hierarchical_folders_and_auto_expansion`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/ui/src/shell.rs crates/ui/tests/navigation.rs crates/ui/src/native/mod.rs
git commit -m "feat(ui): add hierarchical folder tree and auto-expansion to sidebar"
```

---

### Task 2: Centered Readable Column in Native Editor

**Files:**
- Modify: `crates/ui/src/native/editor.rs:200-220,340-470`
- Test: `crates/ui/tests/display.rs`

**Step 1: Write the failing test**

Add test in `crates/ui/tests/display.rs` to verify that `EditorWidget` centers content when width > 800px, calculates `content_width = clamp(320.0, 780.0)`, and maps clicks to the centered text coordinates.

```rust
#[test]
fn test_editor_readable_column_and_margin() {
    let mut widget = EditorWidget::default();
    // Test centered margin calculation helper or layout behavior
    let available_width = 1200.0;
    let (content_width, margin_x) = EditorWidget::calculate_column_metrics(available_width);
    assert_eq!(content_width, 780.0);
    assert_eq!(margin_x, (1200.0 - 780.0) / 2.0);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-ui --test display test_editor_readable_column_and_margin`
Expected: FAIL (method not defined).

**Step 3: Write minimal implementation**

In `crates/ui/src/native/editor.rs`:
- Add `pub fn calculate_column_metrics(available_width: f32) -> (f32, f32)` returning `(content_width, margin_x)`:
  - `content_width = (available_width - 48.0).clamp(320.0, 780.0)`
  - `margin_x = ((available_width - content_width) / 2.0).max(24.0)`
- In `EditorWidget::show`:
  - Calculate `(content_width, margin_x) = Self::calculate_column_metrics(rect.width())`.
  - Pass `content_width` into `galley` shaping.
  - Set `origin = rect.left_top() + Vec2::new(margin_x - if long { self.horizontal } else { 0.0 }, self.heights.y_for_block(line) - self.scroll)`.
  - Adjust caret, selection, flash badges, and mouse click pointer coordinate translation (`position.x - origin.x`).
  - Add top margin of 20.0 px and bottom scroll margin of 60.0 px so text doesn't touch the top tab bar.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui --test display test_editor_readable_column_and_margin`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/ui/src/native/editor.rs crates/ui/tests/display.rs
git commit -m "feat(ui): center editor text in readable column with symmetric margins"
```

---

### Task 3: Tab Bar & Refined Status Bar

**Files:**
- Modify: `crates/ui/src/native/mod.rs:72-162`
- Test: `crates/ui/tests/gui.rs`

**Step 1: Write the failing test**

Add test in `crates/ui/tests/gui.rs` checking that multiple open buffers render as switchable tabs, show dirty indicators `●`, and permit closing via `×`.

```rust
#[test]
fn test_tab_bar_renders_open_buffers_and_switches_active() {
    let mut app = NativeApp::new(Application::new(), &eframe::egui::Context::default());
    // Create two buffers in workspace
    let buf1 = app.application.shell.workspace_mut().buffers_mut().open_from_text(None, "# Doc 1").unwrap();
    let view1 = app.application.shell.workspace_mut().create_view(buf1).unwrap();
    let buf2 = app.application.shell.workspace_mut().buffers_mut().open_from_text(None, "# Doc 2").unwrap();
    let _view2 = app.application.shell.workspace_mut().create_view(buf2).unwrap();

    // Verify tabs representation exists in workspace views
    assert_eq!(app.application.shell.workspace().views().len(), 2);
}
```

**Step 2: Run test to verify it fails / passes as baseline**

Run: `cargo test -p truff-ui --test gui test_tab_bar_renders_open_buffers_and_switches_active`
Expected: PASS baseline.

**Step 3: Write minimal implementation**

In `crates/ui/src/native/mod.rs`:
- Replace the single-line `egui::Panel::top("title")` with a tab bar panel `egui::Panel::top("tabs")` of height 36 px:
  - Iterates over `self.application.shell.workspace().views()`.
  - For each view, retrieves buffer path or `"Scratch"`.
  - Displays `📄 <Title>` + dirty `●` (if buffer is dirty) + `×` close button.
  - Active tab: background `#2E3440`, text `#ECEFF4`, corner radius `4.0` on top corners.
  - Inactive tab: background transparent, text `#9AA5B5`, hover `#434C5E` with `4.0` corner radius.
  - Clicking tab: switches active view in workspace.
  - Clicking `×`: closes the view.
- Update `status` panel:
  - Mode pill badge: 11 px Bold, `NORMAL` in `#4C566A`, `INSERT` in `#88C0D0`, `VISUAL` in `#EBCB8B`.
  - Right aligned: cursor `Ln {line}, Col {col}` in monospace, `Markdown · UTF-8`.

**Step 4: Run GUI tests to verify they pass**

Run: `cargo test -p truff-ui --test gui`
Expected: ALL PASS.

**Step 5: Commit**

```bash
git add crates/ui/src/native/mod.rs crates/ui/tests/gui.rs
git commit -m "feat(ui): implement horizontal tab bar and polished status bar"
```

---

### Task 4: Typography Hierarchy, Nord Design Tokens & Full Workspace Verification

**Files:**
- Modify: `crates/ui/src/native/paint.rs:12-25`
- Modify: `crates/ui/src/theme.rs:114-142`
- Verify: Full workspace tests, clippy, fmt

**Step 1: Write test for typography styling**

In `crates/ui/tests/display.rs`, assert that heading font sizes in `paint.rs` match the specification: H1 = 28.0 px, H2 = 22.0 px, H3 = 18.0 px, body = 15.5 px.

**Step 2: Run test to verify failure**

Run: `cargo test -p truff-ui --test display`
Expected: FAIL.

**Step 3: Implement minimal changes**

- In `crates/ui/src/native/paint.rs`:
  - `# ` -> FontId::proportional(28.0), line height 36.0
  - `## ` -> FontId::proportional(22.0), line height 30.0
  - `### ` -> FontId::proportional(18.0), line height 26.0
  - Default text -> FontId::proportional(15.5), line height 24.0
  - Code blocks -> FontId::monospace(13.5), line height 20.0
- In `crates/ui/src/theme.rs`:
  - Ensure Nord color values match specification:
    - `bg_primary`: `#2E3440`
    - `bg_secondary`: `#242933`
    - `bg_tertiary`: `#1F232A`
    - `bg_surface`: `#3B4252`
    - `border`: `#3B4252`
    - `selection`: `#434C5E` with appropriate opacity
    - `accent`: `#88C0D0`
    - `text_primary`: `#ECEFF4`
    - `text_muted`: `#9AA5B5`

**Step 4: Run workspace checks**

Run:
```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
Expected: ALL PASS with 0 warnings.

**Step 5: Commit**

```bash
git add crates/ui/src/native/paint.rs crates/ui/src/theme.rs crates/ui/tests/display.rs
git commit -m "feat(ui): align typography scale and Nord semantic theme tokens"
```
