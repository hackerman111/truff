# GUI Vault Creation with Path Autocompletion Design

**Date:** 2026-10-11  
**Status:** Approved  
**Goal:** Provide an intuitive graphical user interface for creating and opening vaults when none are open, with real-time filesystem path autocompletion supporting `~`, `./`, and `../`.

---

## 1. User Interface Components

### 1.1 Welcome Screen (`CentralPanel`)
When `Application::vault_path()` is `None`:
- Display a centered welcome screen inside `CentralPanel` instead of an empty editor.
- Header: **Truff v2 — Markdown Knowledge Workspace**.
- Action Buttons:
  - `[ + Создать новое хранилище ]` — triggers the vault creation dialog modal.
  - `[ 📂 Открыть существующее ]` — opens path entry dialog.
- Recent Vaults list:
  - Rendered from `GlobalConfig::load().recent_vaults`.
  - Clicking any recent vault entry immediately opens it via `Application::open_vault`.

### 1.2 Sidebar Quick Actions (`egui::Panel::left("vault")`)
When no vault is open:
- Show quick action buttons `[+ Создать]` and `[Открыть]` in the sidebar heading.

### 1.3 Vault Creation Dialog Window (`egui::Window`)
A modal dialog titled **Создание нового хранилища**:
- **Path Input Field**:
  - Text input for target directory path.
  - Default preset: `~/TruffVault` (or `./TruffVault`).
  - Supports `~`, `./`, `../`, and absolute paths.
- **Autocompletion Overlay / Suggestions**:
  - Live scanning of directories matching the current typed prefix.
  - Keyboard `Tab` completion support and clickable suggestion chips/list.
- **Initial Note Checkbox**:
  - Option "Создать стартовую заметку" (checked by default, filename `Welcome.md`).
- **Actions**:
  - `[ Создать и открыть ]` — validates path, creates directory, writes initial note, opens vault, records to `GlobalConfig`.
  - `[ Отмена ]` — closes the modal dialog.
- **Error display**:
  - Displays inline errors (permission denied, path is file, already locked).

---

## 2. Path Autocompletion Architecture

Module: `crates/ui/src/native/path_completion.rs` (or `crates/ui/src/app/path_completion.rs`):
1. **Expansion**:
   - `expand_path(input: &str) -> PathBuf`:
     - `~/...` expanded using `$HOME`.
     - `./...`, `../...`, or relative names expanded using `std::env::current_dir()`.
2. **Directory Scanner**:
   - Given an input prefix, resolves the parent directory to scan and the partial name filter.
   - Reads directory entries, keeps only directories, filters by prefix.
   - Returns up to 8 matched directory suggestions.
3. **Tab Completion**:
   - Computes longest common prefix among candidates or completes the first candidate.

---

## 3. Data Flow & Execution

1. User clicks **"Создать новое хранилище"**.
2. Modal window opens with path suggestions.
3. User types (e.g. `./my_no` -> autocompletes to `./my_notes`).
4. User clicks **"Создать и открыть"**:
   - Resolves path to canonical/normalized form.
   - Creates directory tree with `std::fs::create_dir_all`.
   - Optionally creates `Welcome.md` with starter Markdown template.
   - Invokes `app.open_vault(&path)`.
   - Vault is opened, persisted in `GlobalConfig`, modal closes.
