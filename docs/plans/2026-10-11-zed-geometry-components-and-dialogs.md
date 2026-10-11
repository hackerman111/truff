# Zed Geometry, Components & Dialogs Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Implement Zed-inspired geometry, procedural vector icons, ghost icon buttons, and beautifully styled dialogs without default OS window chrome or missing glyph boxes.

**Architecture:** Create `icons.rs` for procedural vector rendering of clean 1.5 px icons and canvas dirty dots; create `widgets.rs` providing standardized `icon_button`, `primary_button`, `secondary_button`, and `modal_window`; upgrade the sidebar header, file tree, and dialogs in `crates/ui/src/native/mod.rs`.

**Tech Stack:** Rust 2024, egui / eframe 0.36.2, glow.

---

### Task 1: Procedural Vector Icons & Canvas Dirty Dot (`icons.rs`)

**Files:**
- Create: `crates/ui/src/native/icons.rs`
- Modify: `crates/ui/src/native/mod.rs:1-15`
- Test: `crates/ui/tests/native.rs`

**Step 1: Write the failing test**

In `crates/ui/tests/native.rs`, add a test verifying `render_icon` and `draw_dirty_dot` can draw onto an egui painter without panic or font dependency:

```rust
#[test]
fn test_procedural_vector_icons_and_dirty_dot() {
    use truff_ui::native::icons::{Icon, render_icon, draw_dirty_dot};
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
        render_icon(ui.painter(), Icon::File, rect, egui::Color32::WHITE, 1.5);
        render_icon(ui.painter(), Icon::Folder, rect, egui::Color32::WHITE, 1.5);
        render_icon(ui.painter(), Icon::Plus, rect, egui::Color32::WHITE, 1.5);
        render_icon(ui.painter(), Icon::Close, rect, egui::Color32::WHITE, 1.5);
        render_icon(ui.painter(), Icon::ChevronRight, rect, egui::Color32::WHITE, 1.5);
        render_icon(ui.painter(), Icon::ChevronDown, rect, egui::Color32::WHITE, 1.5);
        render_icon(ui.painter(), Icon::Vault, rect, egui::Color32::WHITE, 1.5);
        render_icon(ui.painter(), Icon::MoreHorizontal, rect, egui::Color32::WHITE, 1.5);
        draw_dirty_dot(ui.painter(), rect.center(), egui::Color32::LIGHT_BLUE, 2.75);
    });
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-ui --test native test_procedural_vector_icons_and_dirty_dot`
Expected: FAIL with "unresolved import / module not found".

**Step 3: Write minimal implementation**

Create `crates/ui/src/native/icons.rs`:
- Define `pub enum Icon { File, Folder, ChevronRight, ChevronDown, Plus, Close, Vault, MoreHorizontal }`.
- Implement `pub fn render_icon(painter: &egui::Painter, icon: Icon, rect: egui::Rect, color: egui::Color32, stroke_width: f32)`.
- Implement `pub fn draw_dirty_dot(painter: &egui::Painter, center: egui::Pos2, color: egui::Color32, radius: f32)` calling `painter.circle_filled(center, radius, color)`.
- Register `pub mod icons;` in `crates/ui/src/native/mod.rs`.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui --test native test_procedural_vector_icons_and_dirty_dot`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/ui/src/native/icons.rs crates/ui/src/native/mod.rs crates/ui/tests/native.rs
git commit -m "feat(ui): add procedural vector icons and canvas dirty dot rendering"
```

---

### Task 2: Standardized Ghost & Styled Widgets (`widgets.rs`)

**Files:**
- Create: `crates/ui/src/native/widgets.rs`
- Modify: `crates/ui/src/native/mod.rs:1-15`
- Test: `crates/ui/tests/native.rs`

**Step 1: Write the failing test**

In `crates/ui/tests/native.rs`, add a test testing the widgets:

```rust
#[test]
fn test_zed_component_widgets() {
    use truff_ui::native::widgets::{icon_button, primary_button, secondary_button};
    use truff_ui::native::icons::Icon;
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        let resp1 = icon_button(ui, Icon::Plus, "Add");
        assert_eq!(resp1.rect.size(), egui::vec2(28.0, 28.0));
        let resp2 = primary_button(ui, "Save");
        assert_eq!(resp2.rect.height(), 32.0);
        let resp3 = secondary_button(ui, "Cancel");
        assert_eq!(resp3.rect.height(), 32.0);
    });
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-ui --test native test_zed_component_widgets`
Expected: FAIL.

**Step 3: Write minimal implementation**

Create `crates/ui/src/native/widgets.rs`:
- `pub fn icon_button(ui: &mut egui::Ui, icon: Icon, tooltip: &str) -> egui::Response`:
  - 28 × 28 px allocation, Sense::click().
  - Hover: fill `#394351`, radius 5 px. Default: transparent.
  - Draws `render_icon` centered at 15 px size.
- `pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response`:
  - Height 32 px, radius 5 px, fill `#88A9C1`, text `#20242C` bold.
- `pub fn secondary_button(ui: &mut egui::Ui, text: &str) -> egui::Response`:
  - Height 32 px, radius 5 px, ghost transparent default, hover `#394351`.
- Expose in `crates/ui/src/native/mod.rs`.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui --test native test_zed_component_widgets`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/ui/src/native/widgets.rs crates/ui/src/native/mod.rs crates/ui/tests/native.rs
git commit -m "feat(ui): add standardized icon_button, primary_button, and secondary_button"
```

---

### Task 3: Sidebar Header Ghost Actions & Vector File Tree Items

**Files:**
- Modify: `crates/ui/src/native/mod.rs:330-460`
- Test: `crates/ui/tests/gui.rs`

**Step 1: Write GUI test for ghost header buttons and tree vector icons**

In `crates/ui/tests/gui.rs`, add a test verifying header buttons have 28x28 bounds and tree rows render with procedural vector icons without font fallback panic.

**Step 2: Run test to verify failure**

Run: `cargo test -p truff-ui --test gui test_sidebar_header_ghost_actions`
Expected: FAIL.

**Step 3: Implement minimal changes**

In `crates/ui/src/native/mod.rs`:
- Header:
  - Height 36 px.
  - Replace `small_button`s with `icon_button(ui, Icon::Plus, "Новая заметка")`, `icon_button(ui, Icon::Folder, "Новая папка")`, `icon_button(ui, Icon::Vault, "Сменить хранилище")`, `icon_button(ui, Icon::MoreHorizontal, "Свернуть все")`.
  - Zero default button border/background frames.
- Tree Rows:
  - Height 28 px.
  - Replace text glyphs `▸ 📁` / `▾ 📁` and `📄` with `render_icon` (`ChevronRight` / `ChevronDown` + `Folder` or `File`).
  - Replace `●` label with `draw_dirty_dot(ui.painter(), dot_center, color(theme.accent), 2.75)`.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui --test gui test_sidebar_header_ghost_actions`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/ui/src/native/mod.rs crates/ui/tests/gui.rs
git commit -m "feat(ui): use ghost icon buttons in sidebar header and vector icons in file tree"
```

---

### Task 4: Styled Dialog Windows without OS Chrome

**Files:**
- Modify: `crates/ui/src/native/mod.rs:520-800`
- Test: `crates/ui/tests/gui.rs`

**Step 1: Write GUI test verifying dialog styling**

In `crates/ui/tests/gui.rs`, add test verifying dialogs render without title bar, inputs use `#20242C` backgrounds, and primary/secondary buttons are present.

**Step 2: Run test to verify failure**

Run: `cargo test -p truff-ui --test gui test_styled_vault_and_note_dialogs`
Expected: FAIL.

**Step 3: Implement minimal changes**

In `crates/ui/src/native/mod.rs`:
- Update `show_create_vault_dialog`, `show_create_file_dialog`, and `show_create_folder_dialog`:
  - `egui::Window::new(...).title_bar(false).collapsible(false).resizable(false).fixed_width(460.0)`
  - Frame: background `#252A34`, border 1px `#333945`, radius 8 px, margin 20 px.
  - Header: left-aligned clean title (16 px `#E2E6ED`) and subtitle (12 px `#8B95A5`), top-right close `icon_button(ui, Icon::Close, "Закрыть")`.
  - Inputs: 36 px height with `#20242C` background, 5 px radius, 1 px `#333945` border.
  - Footer: `secondary_button(ui, "Отмена")` + `primary_button(ui, "Создать и открыть")`.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui --test gui test_styled_vault_and_note_dialogs`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/ui/src/native/mod.rs crates/ui/tests/gui.rs
git commit -m "feat(ui): style modal dialogs with seamless frames and 36px inputs"
```

---

### Task 5: 34 px Tabs Polish & Full Workspace Verification

**Files:**
- Modify: `crates/ui/src/native/mod.rs:130-220`
- Test: `crates/ui/tests/gui.rs`

**Step 1: Write test for tab vector close button and dirty dot**

In `crates/ui/tests/gui.rs`, assert tabs use vector `Icon::Close` on hover and `draw_dirty_dot` when modified.

**Step 2: Run test to verify baseline**

Run: `cargo test -p truff-ui --test gui test_tab_vector_close_and_dirty_dot`
Expected: FAIL.

**Step 3: Implement minimal changes**

In `crates/ui/src/native/mod.rs`:
- Tab bar height 34 px.
- Replace `small_button("×")` with vector `Icon::Close` on hover.
- Replace `ui.label("●")` with `draw_dirty_dot`.
- Run full workspace verification:
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
git add crates/ui/src/native/mod.rs crates/ui/tests/gui.rs
git commit -m "feat(ui): polish tabs to 34px with vector close and canvas dirty dot"
```
