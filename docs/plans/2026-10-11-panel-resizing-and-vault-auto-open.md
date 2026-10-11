# Resizable Panels and Vault Auto-Open Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Enhance UI panel resizing with smooth min/max boundaries and cross-session size persistence, and add global configuration to automatically remember and reopen saved vaults on application startup.

**Architecture:** 
1. UI panel layout: configure left (`vault`) and right (`references`) egui side panels with explicit `min_size(120.0)` and `max_size(600.0)`, visible separator lines, and inner `ui.set_min_width(ui.available_width())` for stable layout without auto-shrinking. Enable `persistence` on `eframe` in `crates/ui/Cargo.toml` so egui panel sizes and positions persist across app restarts.
2. Global configuration: implement a dedicated global configuration manager in `crates/ui/src/app/global_config.rs` that reads and writes `~/.config/truff/config.toml` (respecting `XDG_CONFIG_HOME`). Record the last opened vault and recent vaults whenever a vault is opened.
3. Startup auto-open: during `Application` startup in `main.rs`, if no `--vault` flag is supplied, load the last saved vault from global configuration and automatically open it if the directory exists.

**Tech Stack:** Rust 2024 edition, egui / eframe 0.36.2 with `persistence`, serde / toml, xdg / home path resolution.

---

### Task 1: Fix Baseline Test Regressions in `crates/ui`

**Files:**
- Modify: `crates/ui/src/overlays.rs:205-220`
- Modify: `crates/ui/src/shell.rs:380-390`
- Modify: `crates/ui/tests/vim_routing.rs:110-115`

**Step 1: Write / verify minimal fixes**
- In `crates/ui/src/overlays.rs`, support `activate_candidates` taking `(Range<usize>, String)` so Flash targets preserve word text.
- In `crates/ui/src/shell.rs`, slice word text into `activate_candidates`.
- In `crates/ui/tests/vim_routing.rs`, use `anchor`/`head` in `EditorInput::SetSelection`.

**Step 2: Run verification**
Run: `cargo test -p truff-ui --test gui --test vim_routing`
Expected: PASS

---

### Task 2: Enable eframe Persistence & Improve Panel Resizing

**Files:**
- Modify: `crates/ui/Cargo.toml:17-25`
- Modify: `crates/ui/src/native/mod.rs:60-90`
- Test: `crates/ui/tests/native.rs`

**Step 1: Write test for panel stability and width boundaries**
Verify that panels do not shrink to content minimums and that layouts remain stable across frames.

**Step 2: Update `Cargo.toml`**
Enable `persistence` feature on `eframe`:
```toml
eframe = { version = "=0.36.2", default-features = false, features = ["glow", "default_fonts", "wayland", "x11", "persistence"] }
```

**Step 3: Update `crates/ui/src/native/mod.rs`**
Add:
- `.min_size(120.0).max_size(600.0).show_separator_line(true)` to left and right panels.
- `ui.set_min_width(ui.available_width());` inside both panels to prevent unexpected shrinking.
- Ensure `persist_egui_memory(&self) -> bool { true }` in `eframe::App` implementation.

**Step 4: Run verification**
Run: `cargo test -p truff-ui --test native`
Expected: PASS (all 3 tests pass)

---

### Task 3: Implement Global Config (`~/.config/truff/config.toml`)

**Files:**
- Create: `crates/ui/src/app/global_config.rs`
- Modify: `crates/ui/src/app/mod.rs`
- Test: `crates/ui/tests/global_config.rs`

**Step 1: Write unit tests for GlobalConfig**
Tests for:
- Loading non-existent config returns default
- Saving and loading `last_vault` and `recent_vaults`
- Updating `last_vault` moves it to the front of `recent_vaults` without duplicates

**Step 2: Implement GlobalConfig**
```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub last_vault: Option<PathBuf>,
    pub recent_vaults: Vec<PathBuf>,
}
```
With methods `load()`, `save()`, `record_vault(path: &Path)`, and `global_config_path()`.

**Step 3: Run test**
Run: `cargo test -p truff-ui --test global_config`
Expected: PASS

---

### Task 4: Hook Auto-Open into Application and CLI

**Files:**
- Modify: `crates/ui/src/app/mod.rs`
- Modify: `crates/ui/src/app/commands.rs`
- Modify: `crates/ui/src/main.rs`
- Test: `crates/ui/tests/app_storage.rs`

**Step 1: Update Application**
- In `Application::open_vault`: when a vault successfully opens, call `GlobalConfig::record_vault(&path)`.
- In `Application::new_with_auto_open()` or startup in `main.rs`: if no vault is opened via CLI, inspect `GlobalConfig::load().last_vault`. If it exists on disk, open it automatically.

**Step 2: Add `:vaults` command to show recent vaults**
In `crates/ui/src/app/commands.rs`, handle `:vaults` to list saved vaults in `status`.

**Step 3: Run full verification suite**
Run:
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: PASS
