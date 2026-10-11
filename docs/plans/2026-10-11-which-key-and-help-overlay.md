# Which-Key Support and '?' Help Overlay Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Implement a responsive, theme-consistent Which-Key overlay for prefix key combinations, support entering help mode via `?` in Normal mode and `:help`, and provide categorized hotkey/command cheatsheet viewing with `Esc` dismissal.

**Architecture:** Extend `WhichKeyOverlay` in `crates/ui/src/overlays.rs` with prefix and categorized help modes; route `?` in Normal mode and `:help` command in `crates/ui/src/shell.rs` and `crates/ui/src/app/commands.rs` to show help; render Which-Key as a modern floating bottom panel in `crates/ui/src/native/mod.rs` with obsidian/nord styling tokens; close with `Esc`.

**Tech Stack:** Rust, egui 0.28 / eframe, truff-core, truff-ui.

---

### Task 1: Extend WhichKeyOverlay Model in `crates/ui/src/overlays.rs`

**Files:**
- Modify: `crates/ui/src/overlays.rs`
- Test: `crates/ui/tests/which_key.rs`

**Step 1: Write the failing test**
Create `crates/ui/tests/which_key.rs`:
```rust
use truff_core::config::KeyHint;
use truff_core::vim::Key;
use truff_ui::overlays::{WhichKeyCategory, WhichKeyMode, WhichKeyOverlay};

#[test]
fn test_which_key_modes_and_help_categories() {
    let mut overlay = WhichKeyOverlay::new();
    assert!(!overlay.is_visible());
    assert_eq!(overlay.mode(), WhichKeyMode::Prefix);

    let default_cats = WhichKeyOverlay::default_help_categories();
    assert!(!default_cats.is_empty());
    assert!(default_cats.iter().any(|c| c.name.contains("Навигация") || c.name.contains("Motions")));

    overlay.show_help(default_cats);
    assert!(overlay.is_visible());
    assert_eq!(overlay.mode(), WhichKeyMode::Help);
    assert!(!overlay.categories().is_empty());

    overlay.hide();
    assert!(!overlay.is_visible());
    assert!(overlay.hints().is_empty());
    assert!(overlay.categories().is_empty());
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test --test which_key test_which_key_modes_and_help_categories`
Expected: FAIL due to missing `WhichKeyMode`, `WhichKeyCategory`, `mode()`, `show_help()`, etc.

**Step 3: Write minimal implementation in `crates/ui/src/overlays.rs`**
Add `WhichKeyMode`, `WhichKeyCategory`, extend `WhichKeyOverlay` with `categories`, `title`, `mode`, and implement `default_help_categories()`, `mode()`, `title()`, `categories()`, `show_help()`, `set_prefix_hints_with_title()`.

**Step 4: Run test to verify it passes**
Run: `cargo test --test which_key test_which_key_modes_and_help_categories`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/overlays.rs crates/ui/tests/which_key.rs
git commit -m "feat(ui): extend WhichKeyOverlay model with categories and help mode"
```

---

### Task 2: Connect `?` Key and `:help` Command to Which-Key Help

**Files:**
- Modify: `crates/ui/src/shell.rs`
- Modify: `crates/ui/src/app/commands.rs`
- Test: `crates/ui/tests/which_key.rs`

**Step 1: Write the failing test**
In `crates/ui/tests/which_key.rs`, add:
```rust
use truff_core::vim::{Key, Mode};
use truff_ui::shell::{Focus, Shell, ShellAction};
use truff_ui::app::Application;

#[test]
fn test_question_mark_in_normal_mode_opens_help() {
    let mut shell = Shell::new();
    shell.set_editor_text("sample markdown\n");
    assert_eq!(shell.mode(), Mode::Normal);

    let action = shell.feed_key(Key::Char('?'));
    assert_eq!(action, ShellAction::Incomplete);
    assert!(shell.which_key().is_visible());
    assert_eq!(shell.which_key().mode(), WhichKeyMode::Help);
    assert_eq!(shell.editor_text(), "sample markdown\n");

    // Esc closes it
    let esc_action = shell.feed_key(Key::Esc);
    assert!(!shell.which_key().is_visible());
}

#[test]
fn test_question_mark_in_insert_mode_inserts_char() {
    let mut shell = Shell::new();
    shell.set_editor_text("sample\n");
    shell.feed_key(Key::Char('i'));
    assert_eq!(shell.mode(), Mode::Insert);

    // In insert mode, ? is an edit, not which-key help
    shell.feed_key(Key::Char('?'));
    assert!(!shell.which_key().is_visible());
}

#[test]
fn test_help_command_opens_which_key_help() {
    let mut app = Application::new();
    let res = app.execute_command("help");
    assert!(res.is_ok());
    assert!(app.shell.which_key().is_visible());
    assert_eq!(app.shell.which_key().mode(), WhichKeyMode::Help);
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test --test which_key test_question_mark_in_normal_mode_opens_help`
Expected: FAIL

**Step 3: Write minimal implementation**
1. In `crates/ui/src/shell.rs`:
   - Intercept `Key::Esc` when `self.which_key.is_visible()` to call `self.which_key.hide()`.
   - In Normal mode, if not in colon or search mode, intercept `Key::Char('?')` to call `self.which_key.show_help(WhichKeyOverlay::default_help_categories())` and return `ShellAction::Incomplete`.
   - Update `<C-w>` prefix handling to pass title `"<C-w> Окна и панели"`.
2. In `crates/ui/src/app/commands.rs`:
   - Update `"help"` command handler to invoke `self.shell.which_key_mut().show_help(WhichKeyOverlay::default_help_categories())` and set status `"Which-key справка открыта (Esc для закрытия)"`.

**Step 4: Run test to verify it passes**
Run: `cargo test --test which_key`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/shell.rs crates/ui/src/app/commands.rs crates/ui/tests/which_key.rs
git commit -m "feat(ui): route '?' in normal mode and :help command to which-key overlay"
```

---

### Task 3: Render Bottom Floating Which-Key Overlay in `crates/ui/src/native/mod.rs`

**Files:**
- Modify: `crates/ui/src/native/mod.rs`
- Test: `crates/ui/tests/which_key.rs`

**Step 1: Write the failing test**
In `crates/ui/tests/which_key.rs`, add a GUI render test:
```rust
use truff_ui::native::NativeApp;

#[test]
fn test_gui_renders_which_key_overlay() {
    let mut app = NativeApp::new(Application::new());
    app.application.shell.feed_key(Key::Char('?'));
    assert!(app.application.shell.which_key().is_visible());

    let ctx = egui::Context::default();
    let _ = ctx.run(Default::default(), |ui| {
        app.show(ui);
    });
}
```

**Step 2: Run test to verify it fails or tests existing layout**
Run: `cargo test --test which_key test_gui_renders_which_key_overlay`

**Step 3: Write minimal implementation**
1. Remove plain `ui.label` output from `egui::Panel::top("title")`.
2. Add a dedicated `show_which_key_overlay(&mut self, context: &egui::Context)` helper in `crates/ui/src/native/mod.rs`:
   - Checks `if self.application.shell.which_key().is_visible()`.
   - Uses `egui::Window::new("which_key_window")` anchored at `egui::Align2::CENTER_BOTTOM, [0.0, -28.0]`.
   - Sets frame with `theme.bg_secondary` fill, `theme.border` stroke, 8.0 corner radius, inner margin 12.0.
   - For `WhichKeyMode::Prefix`: displays the prefix title and horizontal/grid badges for available key continuations.
   - For `WhichKeyMode::Help`: displays categories in a multi-column layout with category headings and key badges.

**Step 4: Run test to verify it passes**
Run: `cargo test --test which_key`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/native/mod.rs crates/ui/tests/which_key.rs
git commit -m "feat(ui): render stylish bottom floating which-key and help overlay in native UI"
```

---

### Task 4: Workspace Verification and Full Test Suite

**Files:**
- Verify: workspace crates
- Fix any pending issues in `crates/ui/tests/navigation.rs`

**Step 1: Run workspace checks**
Run:
```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

**Step 2: Fix any remaining warnings or test failures**
Ensure all tests across `truff-core`, `truff-store`, and `truff-ui` pass cleanly.

**Step 3: Commit**
```bash
git commit -m "chore(ui): finalize which-key and help tests and workspace verification"
```
