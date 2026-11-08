# Editor Typography and Spacing Hierarchy Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Implement increased document title size, expanded title-to-first-line spacing, persistent title on H1 first lines, 3/4 harmonic reduction heading spacings, and expanded TOC item rows.

**Architecture:** Update `EditorWidget::TOP_MARGIN` in `crates/ui/src/native/editor/mod.rs` and title painting in `crates/ui/src/native/editor/viewport.rs`. Update heading line height calculations in `crates/ui/src/native/paint.rs` using a $k = 0.75$ scale. Expand TOC list virtualized row height to 30.0 pt in `crates/ui/src/native/toc/render.rs`. Update matching unit tests in `crates/ui/tests/display.rs`.

**Tech Stack:** Rust, egui, Truff UI editor virtualized layout.

---

### Task 1: Update Heading Line Heights and Typographic Scale

**Files:**
- Modify: `crates/ui/src/native/paint.rs:15-28`
- Test: `crates/ui/tests/display.rs:317-364`

**Step 1: Write the updated test expectations in display.rs**
Update `crates/ui/tests/display.rs` to test the new line heights for H1 through H6 with $k = 0.75$.

**Step 2: Run test to verify failure**
Run: `cargo test -p truff_ui --test display test_typography_hierarchy_scale`
Expected: FAIL due to mismatched line heights.

**Step 3: Update `resolve_font_and_line_height` in `crates/ui/src/native/paint.rs`**
Implement the $k = 0.75$ harmonic spacing scale for headings.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff_ui --test display test_typography_hierarchy_scale`
Expected: PASS.

**Step 5: Commit**
```bash
git add crates/ui/src/native/paint.rs crates/ui/tests/display.rs
git commit -m "style: apply 3/4 harmonic reduction to heading line heights and spacing"
```

---

### Task 2: Update Document Title Size, Top Margin, and H1 Persistence

**Files:**
- Modify: `crates/ui/src/native/editor/mod.rs:121`
- Modify: `crates/ui/src/native/editor/viewport.rs:393-424`
- Test: `crates/ui/tests/display.rs:301-315`

**Step 1: Update test expectation for `TOP_MARGIN` in `crates/ui/tests/display.rs`**
Update `assert_eq!(truff_ui::native::EditorWidget::TOP_MARGIN, 84.0);`

**Step 2: Run test to verify failure**
Run: `cargo test -p truff_ui --test display test_editor_readable_column_and_margin`
Expected: FAIL asserting 48.0 != 84.0.

**Step 3: Implement updates in `mod.rs` and `viewport.rs`**
1. In `crates/ui/src/native/editor/mod.rs`, set `pub const TOP_MARGIN: f32 = 84.0;`.
2. In `crates/ui/src/native/editor/viewport.rs`:
   - Remove `let first_is_h1 = ...; if !first_is_h1 { ... }`.
   - Update title font to `(typography.base_font_size * 2.0).round().max(28.0)`.
   - Update `title_y = rect.top() + Self::TOP_MARGIN - 62.0 - self.scroll;`.

**Step 4: Run tests to verify they pass**
Run: `cargo test -p truff_ui --test display`
Expected: PASS.

**Step 5: Commit**
```bash
git add crates/ui/src/native/editor/mod.rs crates/ui/src/native/editor/viewport.rs crates/ui/tests/display.rs
git commit -m "feat: increase title size and spacing, preserve title above H1"
```

---

### Task 3: Increase Table of Contents (TOC) Item Spacing

**Files:**
- Modify: `crates/ui/src/native/toc/render.rs:267-300`
- Test: `cargo test -p truff_ui --test p4_workflows`

**Step 1: Modify `crates/ui/src/native/toc/render.rs`**
Update `row_height` from 24.0 to 30.0 pt and adjust `Margin::symmetric(6, 4)`.

**Step 2: Run TOC integration tests**
Run: `cargo test -p truff_ui --test p4_workflows`
Expected: PASS.

**Step 3: Commit**
```bash
git add crates/ui/src/native/toc/render.rs
git commit -m "style: increase TOC row height and vertical spacing"
```

---

### Task 4: Full Workspace Verification

**Files:**
- Check: workspace tests, formatting, and clippy

**Step 1: Run workspace test suite**
Run: `cargo test -p truff_ui`
Expected: PASS.

**Step 2: Run workspace clippy**
Run: `cargo clippy -p truff_ui --all-targets -- -D warnings`
Expected: PASS with 0 warnings.
