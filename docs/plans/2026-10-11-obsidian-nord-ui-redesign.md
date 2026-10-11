# Obsidian Minimalist UI & Nord Theme System Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Transform Truff v2 from a monospace terminal-like appearance into an elegant, minimalist Obsidian-inspired interface with proportional typography, heading scaling, a hierarchical vault tree, and a configurable theme system with Nord as the default theme.

**Architecture:** Extend `Theme` with the Nord palette as default and add custom theme TOML serialization in `GlobalConfig`. Upgrade paragraph shaping in `crates/ui/src/native/paint.rs` from hardcoded monospace to proportional typography with heading scale (H1-H3), while keeping monospace for code and commands. Polish the file tree in `crates/ui/src/native/mod.rs` to render indented rows, folder disclosure markers, and rounded selection pills.

**Tech Stack:** Rust, egui 0.36, toml, serde, unicode-segmentation.

---

### Task 1: Extend Theme and GlobalConfig with Nord Default and Custom Theme Serialization

**Files:**
- Modify: `crates/ui/src/theme.rs`
- Modify: `crates/ui/src/app/global_config.rs`
- Test: `crates/ui/tests/global_config.rs`

**Step 1: Write failing test in `crates/ui/tests/global_config.rs`**
Add test verifying that `GlobalConfig` defaults to theme `"nord"` and can serialize/deserialize custom theme definitions.

```rust
#[test]
fn test_theme_config_defaults_to_nord_and_deserializes_custom() {
    let toml_str = r#"
        theme = "custom_nord"

        [themes.custom_nord]
        bg_primary = "#2e3440"
        bg_secondary = "#242933"
        accent = "#88c0d0"
    "#;
    let config: GlobalConfig = toml::from_str(toml_str).expect("parse config with themes");
    assert_eq!(config.theme, "nord"); // when default, but here "custom_nord"
    assert!(config.themes.contains_key("custom_nord"));
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test global_config`
Expected: FAIL due to missing fields `theme` and `themes` on `GlobalConfig`.

**Step 3: Implement `Theme::nord()` and config fields**
1. In `crates/ui/src/theme.rs`:
   - Implement `Theme::nord() -> Self` using Polar Night (`#2e3440`, `#242933`, `#1f232a`), Snow Storm (`#eceff4`, `#9aa5b5`), and Frost (`#88c0d0`).
   - Change `Theme::default()` to return `Theme::nord()`.
   - Provide helper `Theme::from_config(config: &CustomThemeConfig, fallback: &Theme) -> Self`.
2. In `crates/ui/src/app/global_config.rs`:
   - Add `#[serde(default = "default_theme")] pub theme: String` to `GlobalConfig`.
   - Add `#[serde(default)] pub themes: std::collections::BTreeMap<String, CustomThemeConfig>` to `GlobalConfig`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test global_config`
Expected: PASS.

**Step 5: Commit**
```bash
git add crates/ui/src/theme.rs crates/ui/src/app/global_config.rs crates/ui/tests/global_config.rs
git commit -m "feat(ui): add nord theme and configurable themes in global config"
```

---

### Task 2: Runtime Theme Switching and Persistence

**Files:**
- Modify: `crates/ui/src/app/commands.rs:241-248`
- Modify: `crates/ui/src/app/mod.rs`
- Modify: `crates/ui/tests/editor_input.rs`

**Step 1: Write test for theme switching and resolution**
In `crates/ui/tests/editor_input.rs`:
- Verify `shell.theme()` matches `Theme::nord()` by default.
- Verify `:theme` switches to `nord`, `obsidian-dark`, `light`, or custom themes from `GlobalConfig`.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test editor_input`
Expected: FAIL if `Theme::default()` or `:theme` dispatch does not recognize `nord`.

**Step 3: Implement `:theme` resolution and persistence**
In `crates/ui/src/app/commands.rs`:
- When executing `:theme <name>`:
  - Check built-in names: `nord` -> `Theme::nord()`, `dark` / `obsidian-dark` -> `Theme::obsidian_dark()`, `light` -> `Theme::light()`.
  - If not built-in, look up `<name>` in `GlobalConfig::load().themes`.
  - Apply theme to `self.shell.set_theme(theme)`.
  - Update `config.theme = name.to_string()` and call `config.save()`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test editor_input`
Expected: PASS.

**Step 5: Commit**
```bash
git add crates/ui/src/app/commands.rs crates/ui/src/app/mod.rs crates/ui/tests/editor_input.rs
git commit -m "feat(ui): support :theme nord/obsidian-dark and persist active theme"
```

---

### Task 3: Proportional Typography and Heading Scale in Markdown Editor

**Files:**
- Modify: `crates/ui/src/native/paint.rs`
- Modify: `crates/ui/src/native/editor.rs`
- Modify: `crates/ui/src/editor/layout.rs`
- Test: `crates/ui/tests/display.rs`

**Step 1: Write test for proportional layout with heading scales**
In `crates/ui/tests/display.rs`, assert that heading lines produce appropriate visual metrics and do not break grapheme cluster hit-testing.

**Step 2: Run test to verify behavior**
Run: `cargo test -p truff-ui --test display`
Expected: Current baseline passes.

**Step 3: Implement proportional typography and heading scaling**
1. In `crates/ui/src/native/paint.rs`:
   - Inspect text prefix to detect Markdown headers (`# `, `## `, `### `) or code fence (` ``` `).
   - For `# `: `FontId::proportional(22.0)` with `line_height = Some(32.0)` and bold color.
   - For `## `: `FontId::proportional(18.5)` with `line_height = Some(28.0)`.
   - For `### `: `FontId::proportional(16.5)` with `line_height = Some(25.0)`.
   - For fenced/inline code: `FontId::monospace(14.0)` with `line_height = Some(22.0)`.
   - For standard text: `FontId::proportional(15.0)` with `line_height = Some(24.0)`.
2. In `crates/ui/src/native/editor.rs`:
   - Add horizontal padding (18.0px left/right) to editor origin so notes breathe naturally.
   - Smooth caret rect with subtle corner rounding (1.5px) and adaptive height.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test display`
Expected: PASS.

**Step 5: Commit**
```bash
git add crates/ui/src/native/paint.rs crates/ui/src/native/editor.rs crates/ui/src/editor/layout.rs crates/ui/tests/display.rs
git commit -m "feat(ui): implement proportional typography and markdown heading scale"
```

---

### Task 4: Polish Vault File Tree, Panel Layout, and Status Bar

**Files:**
- Modify: `crates/ui/src/native/mod.rs`
- Modify: `crates/ui/tests/navigation.rs`

**Step 1: Write UI layout test**
Verify sidebar rows expose tree hierarchy (`depth`, `is_folder`, `is_collapsed`) properly.

**Step 2: Run test to verify current state**
Run: `cargo test -p truff-ui --test navigation`
Expected: PASS.

**Step 3: Implement tree hierarchy and minimalist panels**
In `crates/ui/src/native/mod.rs`:
1. Top Bar:
   - Clean, compact header with `Truff` logo/title in muted accent, current relative path, and `●` dirty dot.
   - Use `egui::Frame` with `theme.bg_secondary` and subtle bottom stroke `(1.0, theme.border)`.
2. Vault Explorer Panel:
   - For each visible row in `sidebar_model`:
     - Calculate indent: `8.0 + (row.depth as f32) * 14.0`.
     - Display disclosure glyph: if `row.is_folder` -> `if row.is_collapsed { "▸ " } else { "▾ " }` else `" "`.
     - Render row with `Rounding::same(4.0)`.
     - Active selection uses `theme.selection` with rounded background and `theme.text_primary`.
     - Hover uses subtle background tint.
3. Status Bar:
   - Vim mode rendered as a neat pill badge with border radius 4px (`NORMAL`, `INSERT`, `VISUAL`).
   - Muted secondary stats (`text_muted`) for focus and status.
   - Panel background `theme.bg_tertiary` with subtle top separator line `(1.0, theme.border)`.

**Step 4: Run tests to verify all pass**
Run: `cargo test -p truff-ui --test navigation`
Expected: PASS.

**Step 5: Commit**
```bash
git add crates/ui/src/native/mod.rs crates/ui/tests/navigation.rs
git commit -m "feat(ui): polish vault explorer tree, panels and minimalist status bar"
```

---

### Task 5: Full Workspace Verification and GUI Smoke Test

**Files:**
- Entire workspace

**Step 1: Run workspace checks**
```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
Expected: All checks PASS with 0 warnings.

**Step 2: Build release binary and verify running app**
```bash
cargo build --release -p truff-ui
```
Expected: Clean release build.

**Step 3: Commit release readiness**
```bash
git commit --allow-empty -m "chore: complete obsidian minimalist UI and nord theme implementation"
```
