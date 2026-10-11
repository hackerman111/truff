# Typography, Monospace Font, and Basic Settings Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Implement font configuration with monospace as default, dynamic editor scaling from a base font size, and a basic modal Settings dialog persisting to `config.toml`.

**Architecture:** Extend `GlobalConfig` with `TypographyConfig`, resolve dynamically scaled fonts in `crates/ui/src/native/paint.rs`, register custom/system fonts in `egui::FontDefinitions`, create a procedural `Icon::Gear` icon and modal `SettingsDialog` inside `crates/ui/src/native/creation/`, and wire `:settings`/`:config` commands.

**Tech Stack:** Rust (edition 2024), eframe / egui 0.36.2, serde, toml.

---

### Task 1: Add `TypographyConfig` and Integrate into `GlobalConfig` and `Shell`

**Files:**
- Modify: `crates/ui/src/app/global_config.rs`
- Modify: `crates/ui/src/shell/mod.rs`
- Modify: `crates/ui/src/app/mod.rs`
- Test: `crates/ui/tests/global_config.rs`

**Step 1: Write the failing test**
In `crates/ui/tests/global_config.rs`, add a test for `TypographyConfig` defaults and custom TOML parsing:
```rust
#[test]
fn typography_config_defaults_and_parsing() {
    let toml_str = r#"
        theme = "nord"
        [typography]
        font_family = "monospace"
        base_font_size = 18.0
        line_height_multiplier = 1.6
    "#;
    let config: truff_ui::app::global_config::GlobalConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.typography.font_family, "monospace");
    assert_eq!(config.typography.base_font_size, 18.0);
    assert_eq!(config.typography.line_height_multiplier, 1.6);
    assert_eq!(config.typography.custom_font_path, None);

    let default_config = truff_ui::app::global_config::GlobalConfig::default();
    assert_eq!(default_config.typography.font_family, "monospace");
    assert_eq!(default_config.typography.base_font_size, 15.0);
    assert_eq!(default_config.typography.line_height_multiplier, 1.5);
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test global_config typography_config_defaults_and_parsing`
Expected: FAIL (no field `typography` on `GlobalConfig`)

**Step 3: Write minimal implementation**
1. In `crates/ui/src/app/global_config.rs`:
   - Define `TypographyConfig` with `font_family`, `base_font_size`, `line_height_multiplier`, `custom_font_path`.
   - Implement `Default` for `TypographyConfig`.
   - Add `#[serde(default)] pub typography: TypographyConfig` to `GlobalConfig`.
2. In `crates/ui/src/shell/mod.rs`:
   - Add `typography: crate::app::global_config::TypographyConfig` to `Shell`.
   - Add `pub fn typography(&self) -> &TypographyConfig` and `pub fn set_typography(&mut self, config: TypographyConfig)`.
3. In `crates/ui/src/app/mod.rs`:
   - In `Application::new()`, pass `shell.set_typography(config.typography.clone())`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test global_config`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/app/global_config.rs crates/ui/src/shell/mod.rs crates/ui/src/app/mod.rs crates/ui/tests/global_config.rs
git commit -m "feat(ui): add TypographyConfig to GlobalConfig and Shell"
```

---

### Task 2: Implement Dynamic Typography Resolution and Scaling

**Files:**
- Modify: `crates/ui/src/native/paint.rs`
- Modify: `crates/ui/src/native/editor/paragraph/mod.rs`
- Modify: `crates/ui/src/native/editor/viewport.rs`
- Modify: `crates/ui/tests/display.rs`

**Step 1: Write the failing test**
Update `crates/ui/tests/display.rs` to verify font resolution using `TypographyConfig`:
```rust
#[test]
fn typography_scaling_and_monospace_default() {
    use eframe::egui::{FontFamily, FontId};
    use truff_ui::app::global_config::TypographyConfig;
    use truff_ui::native::resolve_font_and_line_height;

    let typo_15 = TypographyConfig {
        font_family: "monospace".into(),
        base_font_size: 15.0,
        line_height_multiplier: 1.5,
        custom_font_path: None,
    };

    let (f_h1, lh_h1) = resolve_font_and_line_height("# Heading 1", &typo_15);
    assert_eq!(f_h1, FontId::new(27.0, FontFamily::Monospace));
    assert_eq!(lh_h1, 35.0);

    let (f_body, lh_body) = resolve_font_and_line_height("Regular markdown text", &typo_15);
    assert_eq!(f_body, FontId::new(15.0, FontFamily::Monospace));
    assert_eq!(lh_body, 23.0);

    let (f_code, lh_code) = resolve_font_and_line_height("```rust", &typo_15);
    assert_eq!(f_code, FontId::new(14.0, FontFamily::Monospace));
    assert_eq!(lh_code, 21.0);

    // Verify scaled sizes with base 20.0
    let typo_20 = TypographyConfig {
        font_family: "monospace".into(),
        base_font_size: 20.0,
        line_height_multiplier: 1.5,
        custom_font_path: None,
    };
    let (f_h1_20, _) = resolve_font_and_line_height("# Heading 1", &typo_20);
    assert_eq!(f_h1_20, FontId::new(36.0, FontFamily::Monospace));
    let (f_body_20, _) = resolve_font_and_line_height("Regular text", &typo_20);
    assert_eq!(f_body_20, FontId::new(20.0, FontFamily::Monospace));
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test display typography_scaling_and_monospace_default`
Expected: FAIL (`resolve_font_and_line_height` takes 1 argument instead of 2)

**Step 3: Write minimal implementation**
1. In `crates/ui/src/native/paint.rs`:
   - Update `resolve_font_and_line_height(source: &str, typography: &TypographyConfig) -> (FontId, f32)`.
   - Update `shape(painter, display, foreground, width, typography)`.
2. In `crates/ui/src/native/editor/paragraph/mod.rs`:
   - Add `pub(super) typography: TypographyConfig` to `ParagraphStyle`.
   - Pass `&style.typography` to `paint::shape`.
3. In `crates/ui/src/native/editor/viewport.rs`:
   - Pass `application.shell.typography().clone()` into `ParagraphStyle`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test display`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/native/paint.rs crates/ui/src/native/editor/paragraph/mod.rs crates/ui/src/native/editor/viewport.rs crates/ui/tests/display.rs
git commit -m "feat(ui): scale typography dynamically from base font size with monospace default"
```

---

### Task 3: Font Registration and Custom Font Loading in NativeApp

**Files:**
- Modify: `crates/ui/src/native/mod.rs`
- Modify: `crates/ui/src/main.rs`

**Step 1: Write test for font configuration helper**
Verify in a headless test that `configure_fonts` successfully registers fonts and applies custom path if valid.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test native font_configuration`
Expected: FAIL (method not found)

**Step 3: Write minimal implementation**
1. In `crates/ui/src/native/mod.rs`:
   - Implement `pub fn configure_fonts(ctx: &egui::Context, typography: &crate::app::global_config::TypographyConfig)`:
     - Initializes `egui::FontDefinitions::default()`.
     - Sets up `FontFamily::Monospace` as primary.
     - If `typography.custom_font_path` is specified and exists:
       - Reads font bytes using `std::fs::read`.
       - Inserts into `font_data` and prepends to `FontFamily::Monospace` (or `Proportional`).
     - Calls `ctx.set_fonts(fonts)`.
   - In `NativeApp::new()`, call `configure_fonts(context, application.shell.typography())`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/native/mod.rs crates/ui/src/main.rs
git commit -m "feat(ui): configure egui fonts with custom font loading"
```

---

### Task 4: Vector Gear Icon and Sidebar Settings Trigger

**Files:**
- Modify: `crates/ui/src/native/icons.rs`
- Modify: `crates/ui/src/native/sidebar.rs`

**Step 1: Write test for Icon::Gear**
Verify in `crates/ui/tests/native.rs` that `Icon::Gear` can be rendered without panic into a test painter.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test native test_gear_icon`
Expected: FAIL (variant `Icon::Gear` not found)

**Step 3: Write minimal implementation**
1. In `crates/ui/src/native/icons.rs`:
   - Add `Gear` to `pub enum Icon`.
   - Add procedural rendering for `Icon::Gear` in `render_icon` (center circle hub, 8 outer teeth).
2. In `crates/ui/src/native/sidebar.rs`:
   - In the sidebar header `horizontal` group, add an icon button for `Icon::Gear` with tooltip "Настройки":
     ```rust
     if widgets::icon_button(ui, icons::Icon::Gear, "Настройки").clicked() {
         dialogs.settings.open = true;
         dialogs.settings.init_from_config(application.shell.typography(), &application.shell.theme());
     }
     ```

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test native`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/native/icons.rs crates/ui/src/native/sidebar.rs
git commit -m "feat(ui): add procedural Gear icon and sidebar settings button"
```

---

### Task 5: Implement `SettingsDialog` UI Component

**Files:**
- Create: `crates/ui/src/native/creation/settings.rs`
- Modify: `crates/ui/src/native/creation/mod.rs`

**Step 1: Write unit test for SettingsDialog initialization and apply**
In `crates/ui/tests/settings_dialog.rs`:
- Verify `SettingsDialog::default()`, `init_from_config`, and apply logic.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test settings_dialog`
Expected: FAIL (module not found)

**Step 3: Write minimal implementation**
1. Create `crates/ui/src/native/creation/settings.rs`:
   - Implement `SettingsDialog` struct:
     - `open: bool`
     - `font_family: String`
     - `base_font_size: f32`
     - `custom_font_path: String`
     - `active_theme: String`
     - `error_message: Option<String>`
   - Implement `show(&mut self, application: &mut Application, context: &egui::Context, theme: &Theme)`:
     - Modal window anchored at `CENTER_CENTER`.
     - Font family choices: Monospace, Proportional, Custom TTF/OTF.
     - Base font size: slider (10.0..=32.0 px), `[-]` and `[+]` buttons, reset to 15.0 px.
     - Live preview sample: "# Заголовок\n`code_example()`\nОсновной текст заметки".
     - Theme selector.
     - Buttons: "Применить и сохранить", "Отмена".
     - On apply: saves to `GlobalConfig`, updates `application.shell.set_typography`, calls `configure_fonts`.
2. In `crates/ui/src/native/creation/mod.rs`:
   - Add `mod settings; pub use settings::SettingsDialog;`
   - Add `pub settings: SettingsDialog` to `CreationDialogs`.
   - Update `is_any_open()` and `show()`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test settings_dialog`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/native/creation/settings.rs crates/ui/src/native/creation/mod.rs
git commit -m "feat(ui): implement SettingsDialog component in CreationDialogs"
```

---

### Task 6: Add `:settings` and `:config` Commands

**Files:**
- Modify: `crates/ui/src/app/commands/appearance.rs`
- Modify: `crates/ui/src/app/commands/mod.rs`
- Modify: `crates/ui/src/native/events.rs` / `crates/ui/src/native/mod.rs`
- Test: `crates/ui/tests/global_config.rs`

**Step 1: Write test for `:settings` command**
In `crates/ui/tests/global_config.rs`:
```rust
#[test]
fn execute_settings_command() {
    let mut app = truff_ui::app::Application::new();
    assert!(app.execute_command("settings").is_ok());
    assert!(app.execute_command("config").is_ok());
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test global_config execute_settings_command`
Expected: FAIL (unknown command)

**Step 3: Write minimal implementation**
1. In `crates/ui/src/app/commands/mod.rs`:
   - Match `"settings" | "config" => self.execute_settings_command(argument)`
2. In `crates/ui/src/app/commands/appearance.rs`:
   - Implement `pub(super) fn execute_settings_command(&mut self, argument: &str) -> AppResult<()>`:
     - If argument is empty, sets status indicating settings opened, or if argument sets font size directly (e.g. `size 18`), updates config.
3. In `crates/ui/src/native/events.rs`:
   - If `:settings` or `:config` command executed, trigger `dialogs.settings.open = true`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test global_config execute_settings_command`
Expected: PASS

**Step 5: Commit**
```bash
git add crates/ui/src/app/commands/ crates/ui/src/native/events.rs crates/ui/tests/global_config.rs
git commit -m "feat(ui): add :settings and :config commands"
```

---

### Task 7: Full Workspace Verification and Regressions Check

**Files:**
- Entire workspace

**Step 1: Check formatting**
Run: `cargo fmt --all -- --check`

**Step 2: Check workspace compilation**
Run: `cargo check --workspace --all-targets`

**Step 3: Run all workspace tests**
Run: `cargo test --workspace`

**Step 4: Run clippy**
Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`

**Step 5: Commit any cleanups**
```bash
git commit -m "chore: final verification for typography and settings"
```
