# GUI Vault Creation with Path Autocompletion Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Add a convenient GUI to create and open vaults without using the command line, featuring a Welcome screen when no vault is loaded, a creation modal, and path autocompletion supporting `~`, `./`, `../`, and absolute paths.

**Architecture:**
1. Autocompletion Engine: pure filesystem helper in `crates/ui/src/app/path_completion.rs` that expands `~` ($HOME) and `./` / `../` (current working directory), scans candidate directories, and returns autocomplete suggestions and longest common prefixes.
2. Creation Operation: method on `Application` (`create_and_open_vault`) that creates the target directory, optionally generates starter Markdown note, opens the vault via `Store`, and registers it in `GlobalConfig`.
3. Native UI Integration: in `crates/ui/src/native/mod.rs`, when `vault_path().is_none()`, render a Welcome screen in `CentralPanel` and quick action buttons in the sidebar. Render a modal dialog (`egui::Window`) with interactive path autocompletion, suggestion chips, and error display.

**Tech Stack:** Rust 2024 edition, egui / eframe 0.36.2, std::fs, std::path.

---

### Task 1: Path Expansion and Directory Autocompletion Engine

**Files:**
- Create: `crates/ui/src/app/path_completion.rs`
- Modify: `crates/ui/src/app/mod.rs`
- Create: `crates/ui/tests/path_completion.rs`

**Step 1: Write unit tests for path expansion and autocompletion**
In `crates/ui/tests/path_completion.rs`:
- Test expanding `~` to home directory.
- Test expanding `./` to current working directory.
- Test expanding `../` to parent of current working directory.
- Test directory completion given a prefix matching existing subdirectories.
- Test Tab-completion helper returning common prefix.

**Step 2: Implement `path_completion.rs`**
Functions:
- `pub fn expand_path(input: &str) -> PathBuf`
- `pub fn complete_directory_path(input: &str) -> Vec<String>`
- `pub fn tab_complete(input: &str) -> Option<String>`

**Step 3: Run tests**
Run: `cargo test -p truff-ui --test path_completion`
Expected: PASS

---

### Task 2: Implement `create_and_open_vault` on Application

**Files:**
- Modify: `crates/ui/src/app/mod.rs`
- Modify: `crates/ui/tests/global_config.rs`

**Step 1: Write integration test**
In `crates/ui/tests/global_config.rs`:
- Verify calling `create_and_open_vault` on a non-existent directory creates the directory, writes starter note, opens the vault, and sets `vault_path()`.

**Step 2: Implement `create_and_open_vault`**
In `crates/ui/src/app/mod.rs`:
- Accepts `path: &Path` and `initial_note: Option<&str>`.
- Expands path, runs `fs::create_dir_all(path)`.
- If `initial_note` is specified, writes starter Markdown content (e.g. `# Welcome to your new Vault`).
- Calls `self.open_vault(path)`.

**Step 3: Run tests**
Run: `cargo test -p truff-ui --test global_config`
Expected: PASS

---

### Task 3: Build Welcome Screen & Creation Dialog Modal in Native UI

**Files:**
- Modify: `crates/ui/src/native/mod.rs`
- Test: `crates/ui/tests/gui.rs`

**Step 1: Write test for Welcome screen & GUI vault creation**
In `crates/ui/tests/gui.rs`:
- Verify that without a vault, CentralPanel displays welcome screen.
- Verify opening vault via GUI action succeeds.

**Step 2: Add Dialog State and UI Rendering to `NativeApp`**
In `crates/ui/src/native/mod.rs`:
- Add fields to `NativeApp`:
  - `show_create_vault_dialog: bool`
  - `show_open_vault_dialog: bool`
  - `vault_path_input: String`
  - `initial_note_name: String`
  - `create_starter_note: bool`
  - `create_vault_error: Option<String>`
- In `CentralPanel`: when `self.application.vault_path().is_none()`, render:
  - Header, description.
  - Buttons: `[ + Создать новое хранилище ]`, `[ 📂 Открыть хранилище ]`.
  - Saved/recent vaults list.
- In `Panel::left("vault")`: when no vault open, render quick action buttons.
- Render `egui::Window::new("Создать новое хранилище")`:
  - Text input with path suggestions dropdown/chips.
  - Tab key handler for completion.
  - Starter note checkbox & name.
  - Submit & Cancel buttons.

**Step 3: Run full verification suite**
Run:
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --all -- --check`
Expected: PASS
