# Inline Text Action Which-Key Menu Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Replace the global `<Space> c` navigator modal with an inline, floating which-key menu at the editor cursor/selection for text colors, highlights, note links, and URL links (with keep/change title modes), navigable by letters and arrow keys.

**Architecture:** Introduce `TextActionOverlay` in `crates/ui/src/overlays/text_action.rs` managing a hierarchical state machine (`Root`, `ColorSelect`, `LinkTitleMode`, `LinkInput`). Render the menu in `crates/ui/src/native/text_action.rs` as a floating `egui::Area` clamped to viewport bounds. Route `<Space> c` in Normal and Visual modes to capture the selection (or current word in Normal mode) and route keyboard events through the overlay before they reach the editor.

**Tech Stack:** Rust (2024 edition), `egui`, `truff-core` (`Buffer`, `Transaction`, `style`, `link_selection`).

---

### Task 1: Core State Machine & Models for `TextActionOverlay`

**Files:**
- Create: `crates/ui/src/overlays/text_action.rs`
- Modify: `crates/ui/src/overlays.rs`
- Test: `crates/ui/src/overlays/text_action.rs` (unit tests in module)

**Step 1: Write the failing tests**

In `crates/ui/src/overlays/text_action.rs`, define tests for state transitions, arrow navigation, mnemonic keys, and Normal mode word boundary detection:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use truff_core::vim::Key;
    use truff_core::text::Buffer;

    #[test]
    fn test_menu_initial_state_and_navigation_to_color() {
        let mut menu = TextActionOverlay::new();
        assert!(!menu.is_open());

        menu.open(None, Some(egui::pos2(100.0, 150.0)));
        assert!(menu.is_open());
        assert_eq!(menu.mode(), TextActionMode::Root);

        // Pressing 'c' moves to ColorSelect (foreground)
        let action = menu.handle_key(Key::Char('c'));
        assert_eq!(action, TextActionResponse::None);
        assert!(matches!(menu.mode(), TextActionMode::ColorSelect { is_highlight: false }));

        // Arrow navigation in color palette
        assert_eq!(menu.selected_color_index(), 0);
        menu.handle_key(Key::Right);
        assert_eq!(menu.selected_color_index(), 1);

        // Enter applies the selected color
        let action = menu.handle_key(Key::Enter);
        assert!(matches!(action, TextActionResponse::ApplyColor { hex } if hex == PALETTE[1].hex));
        assert!(!menu.is_open());
    }

    #[test]
    fn test_menu_color_direct_mnemonic_key() {
        let mut menu = TextActionOverlay::new();
        menu.open(None, Some(egui::pos2(50.0, 50.0)));
        menu.handle_key(Key::Char('h')); // Highlight
        assert!(matches!(menu.mode(), TextActionMode::ColorSelect { is_highlight: true }));

        // Press 'g' (Green) directly applies highlight
        let action = menu.handle_key(Key::Char('g'));
        assert!(matches!(action, TextActionResponse::ApplyHighlight { hex } if hex == "#A3BE8C"));
        assert!(!menu.is_open());
    }

    #[test]
    fn test_link_modes_keep_and_change_title() {
        let mut menu = TextActionOverlay::new();
        menu.open(None, None);
        menu.handle_key(Key::Char('n')); // Note link
        assert!(matches!(menu.mode(), TextActionMode::LinkTitleMode { is_url: false }));

        // 'k' chooses keep title
        menu.handle_key(Key::Char('k'));
        assert!(matches!(menu.mode(), TextActionMode::LinkInput { is_url: false, change_title: false, .. }));
    }

    #[test]
    fn test_word_boundary_at_cursor() {
        let buffer = Buffer::from("Hello world, rust programming!");
        // Position at index 7 ('o' in "world")
        let range = word_boundary_at(&buffer, 7);
        assert_eq!(range, Some(6..11));

        // Position on space
        let space_range = word_boundary_at(&buffer, 5);
        assert_eq!(space_range, None);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-ui overlays::text_action::tests`  
Expected: FAIL with compilation error (module or types not found)

**Step 3: Write minimal implementation**

In `crates/ui/src/overlays/text_action.rs`:
- Define `ColorOption`: `name`, `hex`, `key: char`.
- Define `PALETTE` with the 7 Nord colors (`#BF616A`, `#D08770`, `#EBCB8B`, `#A3BE8C`, `#88C0D0`, `#81A1C1`, `#B48EAD`).
- Define `TextActionMode`: `Root`, `ColorSelect { is_highlight: bool }`, `LinkTitleMode { is_url: bool }`, `LinkInput { is_url: bool, change_title: bool, target: String, title: String, active_field: usize }`.
- Define `TextActionResponse`: `None`, `Close`, `ApplyColor { hex: &'static str }`, `ApplyHighlight { hex: &'static str }`, `ApplyLink { is_url: bool, target: String, title: Option<String> }`, `RemoveStyle`.
- Define `word_boundary_at(buffer: &Buffer, byte_pos: usize) -> Option<Range<usize>>`.
- Implement `TextActionOverlay` methods: `new()`, `open(target, pos)`, `close()`, `handle_key(Key) -> TextActionResponse`.
- Export `TextActionOverlay`, `TextActionMode`, `TextActionResponse`, `word_boundary_at` in `crates/ui/src/overlays.rs`.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui overlays::text_action::tests`  
Expected: PASS

**Step 5: Commit**

```bash
git add crates/ui/src/overlays/text_action.rs crates/ui/src/overlays.rs
git commit -m "feat(ui): implement core state machine for text action which-key overlay"
```

---

### Task 2: Floating UI Rendering for `TextActionOverlay`

**Files:**
- Create: `crates/ui/src/native/text_action.rs`
- Modify: `crates/ui/src/native/mod.rs`
- Modify: `crates/ui/src/native/content.rs`

**Step 1: Write UI component**

In `crates/ui/src/native/text_action.rs`:
- Implement `pub(super) fn show(app: &mut NativeApp, ctx: &egui::Context)`:
  - If `!app.application.shell.text_action_overlay().is_open()`, return early.
  - Determine anchor position: `cursor_pos` provided by overlay, clamped to window rect `ctx.screen_rect()`.
  - Use `egui::Area::new(egui::Id::new("text_action_overlay")).fixed_pos(anchor).order(egui::Order::Foreground)`.
  - Frame styled with `crate::components::overlay_frame(theme)`.
  - Content switch based on `overlay.mode()`:
    - **`Root`**: Vertical stack of rows with colored key pills (`c`, `h`, `n`, `u`, `x`, `Esc`) and descriptive titles.
    - **`ColorSelect`**: Header with title, horizontal swatch row of colors with colored circle, hotkey pill, and border on active index (`selected_color_index`).
    - **`LinkTitleMode`**: Two options: `[k] Сохранить исходное название` и `[c] Изменить название`.
    - **`LinkInput`**: Single or dual text fields for target and optional custom title with focus management and submit buttons.

**Step 2: Connect rendering in `crates/ui/src/native/mod.rs`**

In `NativeApp::ui`:
```rust
text_action::show(self, context);
```

**Step 3: Run check to verify build**

Run: `cargo check -p truff-ui`  
Expected: PASS

**Step 4: Commit**

```bash
git add crates/ui/src/native/text_action.rs crates/ui/src/native/mod.rs crates/ui/src/native/content.rs
git commit -m "feat(ui): render floating text action overlay at editor cursor"
```

---

### Task 3: Key Event Routing and Editor Shell Integration

**Files:**
- Modify: `crates/ui/src/shell/mod.rs`
- Modify: `crates/ui/src/shell/routing.rs`
- Modify: `crates/ui/src/native/events.rs`
- Modify: `crates/ui/src/app/selection.rs`

**Step 1: Write failing integration test for `<Space> c` opening overlay**

In `crates/ui/tests/text_action_menu.rs`:
```rust
#[test]
fn test_space_c_opens_text_action_overlay() {
    let mut app = setup_test_app();
    app.set_content("hello world test");
    // Place cursor on 'world'
    app.set_cursor(6);
    app.feed_key(Key::Char(' '));
    app.feed_key(Key::Char('c'));

    assert!(app.application.shell.text_action_overlay().is_open());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p truff-ui --test text_action_menu`  
Expected: FAIL (opens Navigator or doesn't open TextActionOverlay)

**Step 3: Implement routing in `crates/ui/src/shell/routing.rs` & `native/events.rs`**

- In `Shell`: add `text_action_overlay: TextActionOverlay`.
- In `routing.rs`: intercept `<Space> c` in `Normal`, `VisualChar`, `VisualLine`. Instead of `style.text_color` opening Navigator, return `ShellAction::OpenTextActionMenu`.
- In `native/events.rs`:
  - When `ShellAction::OpenTextActionMenu` arrives:
    - If in `VisualChar` or `VisualLine`: capture target via `self.application.capture_selection()`.
    - If in `Normal`: scan current word under cursor via `word_boundary_at(buffer, cursor)`. If found, construct `SelectionTarget { view, range }`. If on whitespace, set `self.application.status = "Поместите курсор на слово или выделите текст через v/V"`.
    - Retrieve screen cursor coordinates via `self.last_cursor_rect` and call `shell.text_action_overlay_mut().open(target, pos)`.
  - In `NativeApp::key`:
    - If `shell.text_action_overlay().is_open()`:
      - Intercept keys and pass to `shell.text_action_overlay_mut().handle_key(key)`.
      - On `TextActionResponse::ApplyColor { hex }`: invoke `self.application.color_target(target, hex)`.
      - On `TextActionResponse::ApplyHighlight { hex }`: invoke `self.application.highlight_target(target, hex)`.
      - On `TextActionResponse::ApplyLink { is_url, target, title }`: format link and apply transaction to editor.
      - On `TextActionResponse::RemoveStyle`: invoke `self.application.remove_style_target(target)`.
      - Restore editor focus and switch back to Normal mode.

**Step 4: Run test to verify it passes**

Run: `cargo test -p truff-ui --test text_action_menu`  
Expected: PASS

**Step 5: Commit**

```bash
git add crates/ui/src/shell/mod.rs crates/ui/src/shell/routing.rs crates/ui/src/native/events.rs crates/ui/src/app/selection.rs crates/ui/tests/text_action_menu.rs
git commit -m "feat(ui): route Space c to inline text action overlay with word and selection capture"
```

---

### Task 4: Comprehensive Tests for Colors, Highlights, Links, and Undo

**Files:**
- Modify: `crates/ui/tests/text_action_menu.rs`

**Step 1: Write integration tests covering all features**

In `crates/ui/tests/text_action_menu.rs`:
1. `test_text_color_via_mnemonic_key`: Select text with `v`, press `<Space> c c r`, verify text wrapped in `<span style="color:#BF616A">...</span>`.
2. `test_text_highlight_via_arrow_keys`: Select text with `v`, press `<Space> c h Right Enter`, verify text wrapped in `<mark style="background-color:#D08770">...</span>`.
3. `test_note_link_keep_title`: Select "my note", press `<Space> c n k`, input "TargetNote", press `Enter`, verify `[[TargetNote|my note]]`.
4. `test_note_link_change_title`: Select "my note", press `<Space> c n c`, input "TargetNote" Tab "New Title" Enter, verify `[[TargetNote|New Title]]`.
5. `test_url_link_keep_title`: Select "docs", press `<Space> c u k`, input "https://example.com" Enter, verify `[docs](https://example.com)`.
6. `test_normal_mode_auto_word_capture`: In Normal mode on word "truff", press `<Space> c c g`, verify "truff" colored without visual selection.
7. `test_undo_reverts_styling_atomically`: Press `u`, verify text returns to original untouched string in a single step.

**Step 2: Run tests to verify**

Run: `cargo test -p truff-ui --test text_action_menu`  
Expected: PASS

**Step 3: Run full workspace checks**

Run: `cargo fmt --all -- --check`  
Run: `cargo check --workspace --all-targets`  
Run: `cargo test --workspace`  
Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`  
Expected: All PASS with 0 warnings.

**Step 4: Commit**

```bash
git add crates/ui/tests/text_action_menu.rs
git commit -m "test(ui): add end-to-end tests for inline text action which-key menu"
```
