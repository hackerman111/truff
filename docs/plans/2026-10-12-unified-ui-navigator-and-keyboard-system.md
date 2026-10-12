# Unified UI Architecture & Navigator System Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Create a unified Obsidian-style UI with Vim philosophy and tmux-sessionx workspace management, centered around a Universal Navigator, semantic design tokens, centralized Command Registry, Focus Manager, Overlay Manager, and multi-column Which-Key/Help.

**Architecture:** 
1. Semantic design tokens eliminate hardcoded colors and unify geometry/typography across all surfaces.
2. Centralized Command Registry provides user-facing titles, descriptions, categories, shortcuts, and contexts without leaking internal IDs into the UI.
3. Universal Navigator provides a single reusable overlay for Files (`Space f`), Sessions/Tabs (`Space s`), Boards (`Space b`), Outline (`Space o`), Links (`Space l`), and Commands (`Ctrl+P` / `Space c`), supporting fuzzy search, Vim navigation (`h/j/k/l`), item actions (`Enter`, `a`, `r`, `d`, `/`, `?`), and preview.
4. Focus Manager and Overlay Manager ensure deterministic focus restoration and unified geometry/theming for all floating surfaces.
5. Multi-column Which-Key with ~350ms delay and contextual Help (`?`) read from Command Registry.

**Tech Stack:** Rust 2024, `truff-core`, `truff-store`, `truff-ui` with `eframe` (egui 0.36.2), SQLite FTS5.

---

## 1. Architectural Map of Existing Codebase

### 1.1 Crates & Core Responsibilities
- **`crates/core` (`truff-core`)**:
  - `text`: Rope-backed buffer (`crop`), byte/char positions, transaction-based editing, undo/redo history.
  - `vim`: Vim state machine (`VimState`), parser, motions (`h/j/k/l/w/b/e/gg/G`), operators (`d/c/y`), registers, repeat (`.`), Normal/Insert/Visual modes.
  - `commands`: `CommandRegistry`, `CommandId`, `CommandSpec`, `CommandContext`, `Action`, `Effect`.
  - `config`: `KeymapConfig`, `CompiledKeymap`, `KeyBinding`, `KeyHint`, `LimitsConfig`.
  - `navigation`: `RowModel` (hierarchical virtualized tree/list with `RowItem`, `Move`, `collapse`, `expand`, selection, filtering).
  - `workspace`: Multi-buffer, multi-view coordinator with `Workspace`, `View`, `BufferEntry`.
  - `markdown`: `MarkdownFacts` extractor (headings, links, blocks, callouts).

- **`crates/store` (`truff-store`)**:
  - SQLite metadata store (`files`, `headings`, `links`, `backlinks`, `notes_fts` FTS5 table).
  - `SearchSession`, `SearchQuery`, `SearchPage`, `execute_search` with snippet highlighting.
  - `CatalogWorker` background thread for scanning, file operations (`FileOpPlan`), recovery.

- **`crates/ui` (`truff-ui`)**:
  - `theme`: 4-tier dark/light palette (`Theme`, `Color`, `CustomThemeConfig`).
  - `shell`: Application shell coordinating `Focus` (`Sidebar`, `Editor`, `RightPanel`), `sidebar_model`, `right_model`, `editor_vim`, `row_vim`, `which_key`, `flash`.
  - `app`: Domain application coordinating `Shell`, `Store`, `CatalogWorker`, dirty buffer tracking, recovery, `:commands`, registered commands.
  - `native`: `eframe::App` adapter: `events` (keyboard/IME/mouse), `tabs` (top bar), `sidebar` (file tree), `content` (Markdown editor or welcome screen), `backlinks` (right panel), `status` (bottom statusline), `hints` (which-key popup), `creation` (file/folder/vault/settings modals).

### 1.2 Current Problems Identified
1. **No unified overlay/popup system**: Which-Key is bottom-center, Help is bottom, creation dialogs are centered black boxes with hardcoded styles, each with independent geometry, fonts, margins.
2. **Weak information hierarchy**: File names, commands, hints, and technical instructions share similar visual weights; UI looks like a developer prototype.
3. **Internal IDs leaked into UI**: Raw identifiers like `picker.commands`, `board.column.delete`, `buffer.open` appear in Which-Key and palette instead of human-readable Russian/localized labels.
4. **Scattered keyboard handling**: Sidebar, editor, right panel, and overlays handle keys through ad-hoc branches in `events.rs` and `routing.rs` rather than a unified KeymapResolver and FocusManager.
5. **Horizontal Which-Key**: A single wrapped horizontal line that is hard to scan, without structured columns and categories.

---

## 2. Invariants & Features That Must Not Break
1. **Markdown Editing**: Full Vim Normal/Insert/Visual/VisualLine editing, operators (`d`, `c`, `y`), counts, motions, repeat (`.`), undo/redo, snap-to-grapheme.
2. **Tabs & Views**: Opening buffers, switching tabs by mouse or shortcut, closing tabs, dirty dot indicators, crash recovery.
3. **Storage & Vaults**: Opening vaults, background indexing, file operations (`FileOpPlan`), conflict detection on external change.
4. **Vim Motions & Navigation**: `h/j/k/l`, `w/b/e`, `0/$`, `gg/G`, `Ctrl-w` pane switching, Flash `s` word jump.
5. **Performance & Responsiveness**: Zero rendering allocations on hot paths, UI latency <= 60ms, FTS search <= 100ms.

---

## 3. Implementation Plan (P0: Unified UI Foundation)

### Task 1: Semantic Design Tokens & Theme System (`crates/ui/src/theme.rs`)

**Files:**
- Modify: `crates/ui/src/theme.rs`
- Test: `crates/ui/tests/editor_input.rs`

**Details:**
1. Introduce semantic tokens into `Theme`:
   - `bg_canvas`: main editor background (`#1e1e1e` in Obsidian Dark, `#2d333f` in Nord).
   - `bg_panel`: sidebar and tool panel background (`#181818` / `#252a34`).
   - `bg_elevated`: floating components, Navigator, modals, Which-Key (`#242424` / `#2a303c`).
   - `bg_selected`: selected list item, active row (`#333333` / `#394351`).
   - `border_subtle`: dividers and container borders (`#2b2b2b` / `#333945`).
   - `text_primary`: primary readable text (`#dcddde` / `#e2e6ed`).
   - `text_secondary`: descriptions and subtitles (`#a0a0a5` / `#a8b2c1`).
   - `text_muted`: auxiliary paths, shortcuts, hints (`#6e6e73` / `#8b95a5`).
   - `accent_primary`: single prominent accent color (`#7c3aed` Obsidian purple, `#88a9c1` Nord frost).
   - `state_error`, `state_warning`, `state_success`: semantic status colors.
2. Define geometry and density tokens:
   - `Density`: `Compact` (default) vs `Comfortable`.
   - Metrics: tab height (32px), tree row height (28px), status height (26px), corner radius (4-6px), border width (1px), Markdown readable width (750-850px).
3. Replace hardcoded RGB values in `crates/ui/src/native/widgets.rs` with references to `Theme` tokens.

**Verification:**
Run `cargo test -p truff-ui --test editor_input` and `cargo test -p truff-ui --test native`.

---

### Task 2: Human-Readable Command Registry (`crates/core/src/commands/`)

**Files:**
- Modify: `crates/core/src/commands/types.rs`
- Modify: `crates/core/src/commands/defaults.rs`
- Modify: `crates/core/src/commands/registry.rs`
- Test: `crates/core/tests/vim_routing.rs`

**Details:**
1. Enhance `CommandSpec`:
   - `id`: `CommandId` (stable identifier, e.g. `file.open`).
   - `title`: Human-readable user title in Russian (e.g. «Открыть файл»).
   - `description`: Detailed action description (e.g. «Найти и открыть заметку в хранилище»).
   - `category`: Human-readable category (e.g. «Файлы», «Редактирование», «Навигация», «Буферы», «Вид»).
   - `default_key`: Optional default key combination display (e.g. `Space f`, `Ctrl+P`).
   - `contexts`: `Vec<CommandContext>`.
2. Populate `CommandRegistry::default()` with complete human-readable Russian titles and descriptions:
   - No raw IDs like `buffer.open` as titles.
   - Clear categories: «Файлы и папки», «Буферы и сессии», «Редактирование», «Навигация», «Поиск», «Вид и темы».
3. Add helper methods:
   - `CommandRegistry::search(&self, query: &str) -> Vec<&CommandSpec>` (fuzzy/prefix search across title, description, and id).
   - `CommandRegistry::by_category(&self) -> BTreeMap<String, Vec<&CommandSpec>>`.

**Verification:**
Run `cargo test -p truff-core --test vim_routing`.

---

### Task 3: Focus Manager & Context Routing (`crates/ui/src/focus/` or `crates/ui/src/shell/`)

**Files:**
- Create: `crates/ui/src/focus.rs`
- Modify: `crates/ui/src/shell/mod.rs`
- Modify: `crates/ui/src/shell/routing.rs`
- Test: `crates/ui/tests/navigation.rs`

**Details:**
1. Create `FocusTarget`:
   - `Editor`: Markdown document.
   - `Sidebar`: File tree.
   - `RightPanel`: Backlinks / Outline.
   - `Navigator`: Universal Navigator overlay.
   - `CommandBar`: Vim colon command input.
   - `Dialog`: Settings or modal.
2. Create `FocusManager`:
   - Maintains current `FocusTarget`.
   - Maintains a `previous_focus: Vec<FocusTarget>` stack.
   - Deterministic `push_focus(target)` and `pop_focus() -> FocusTarget`.
   - When Navigator or CommandBar closes, focus automatically and deterministically returns to the previously active pane.
3. Update `Shell` to integrate `FocusManager`.

**Verification:**
Run `cargo test -p truff-ui --test navigation`.

---

### Task 4: Universal Navigator Core Component (`crates/ui/src/navigator/`)

**Files:**
- Create: `crates/ui/src/navigator/mod.rs`
- Create: `crates/ui/src/navigator/model.rs`
- Create: `crates/ui/src/navigator/render.rs`
- Modify: `crates/ui/src/lib.rs`
- Test: `crates/ui/tests/navigator.rs` (new test)

**Details:**
1. `NavigatorMode`:
   - `Files`: Files and folders in vault (`Space f`).
   - `Sessions`: Open buffers, windows, tabs (`Space s`).
   - `Boards`: Kanban boards (`Space b`).
   - `Outline`: Headings and note structure (`Space o`).
   - `Links`: Outgoing links and backlinks (`Space l`).
   - `Commands`: All commands / Command Palette (`Ctrl+P` / `Space c`).
2. `NavigatorState`:
   - `mode`: Current `NavigatorMode`.
   - `context_title`: e.g. "Файлы / Vault / Gomopsidian".
   - `query`: Text input for search with cursor.
   - `search_mode`: `true` (typing characters enters query) vs `false` (Vim navigation `h/j/k/l`).
   - `model`: `RowModel` storing items, selection, collapse/expand.
   - `preview`: Option<String> (Markdown text or preview summary).
   - `actions_menu_open`: `bool` (triggered by `?` to show all possible actions for selected item).
   - `action_hints`: List of available quick actions on current object (e.g. `[Enter] Открыть`, `[a] Создать`, `[r] Переименовать`, `[d] Удалить`, `[/] Поиск`, `[?] Все действия`).
3. Key handling in Navigator:
   - Navigation mode:
     - `j/k`: move selection down/up.
     - `h/l`: collapse/expand folders or navigate hierarchy.
     - `/`: focus query search input.
     - `Enter`: execute primary action on selected item (e.g. open file, execute command).
     - `a`: quick add / create file or item.
     - `r`: rename item.
     - `d`: delete / trash item.
     - `Space`: toggle preview.
     - `?`: toggle full actions menu for selected item.
     - `Esc`: close Navigator and restore focus.
   - Search input mode:
     - Characters: append to query.
     - `Backspace`: delete char.
     - `Tab`: accept first/selected completion.
     - `Enter`: confirm selected result or primary action.
     - `Esc`: exit search input mode back to Navigator navigation mode.
4. Visual Layout (Render):
   - Centered elevated window with `bg_elevated`, `border_subtle`, 6px corner radius.
   - Dimensions: width 720px, height 480px.
   - Top: Context header + Breadcrumbs.
   - Search Bar: Icon + Single-line input + mode badge (`[Поиск]` / `[Навигация]`).
   - Main area: Split horizontally (Left: virtualized `RowModel` item list; Right: Preview pane with syntax/details).
   - Bottom Action Bar: Key badges `[Enter]`, `[a]`, `[r]`, `[d]`, `[/]`, `[?]` with clean typography and spacing.

**Verification:**
Add comprehensive tests in `crates/ui/tests/navigator.rs` covering keyboard state transitions, item selection, actions, search query updates, and focus pop.

---

### Task 5: Multi-Column Which-Key & Contextual Help Overlay (`crates/ui/src/overlays/which_key.rs`, `crates/ui/src/native/hints.rs`)

**Files:**
- Modify: `crates/ui/src/overlays/which_key.rs`
- Modify: `crates/ui/src/native/hints.rs`
- Test: `crates/ui/tests/which_key.rs`

**Details:**
1. Multi-Column Grid Layout:
   - Replace the horizontal wrapped row with an organized multi-column layout (3-4 columns).
   - Each column displays items neatly aligned:
     - Left: Key badge in a styled box `[key]` (monospace, accent color, background `bg_tertiary`).
     - Right: Action description (human-readable title, text_primary).
     - Subcategory branches shown as: `[key] Название →`.
2. Delay & Triggers:
   - Configurable delay (default 300-350ms) before popping up for incomplete prefixes.
   - Immediate popup on `<Space>` after timeout or explicit trigger.
   - `Space + prefix` immediately opens corresponding panel without waiting.
3. Contextual Help (`?`):
   - Contextual: when pressed in Editor, shows Editor motions & edits; in Sidebar, shows Tree navigation; in Navigator, shows Navigator actions; in Kanban, shows Kanban controls.
   - Help and Which-Key use unified data from `CommandRegistry`.
   - Never show internal IDs (e.g. `picker.commands`).

**Verification:**
Run `cargo test -p truff-ui --test which_key`.

---

### Task 6: Integrated Overlay Manager & Integration into `NativeApp` and `Shell`

**Files:**
- Create: `crates/ui/src/overlays/manager.rs`
- Modify: `crates/ui/src/native/mod.rs`
- Modify: `crates/ui/src/native/events.rs`
- Modify: `crates/ui/src/shell/routing.rs`
- Modify: `crates/ui/src/app/mod.rs`
- Test: `crates/ui/tests/gui.rs`
- Test: `crates/ui/tests/e2e_workflow.rs`

**Details:**
1. `OverlayManager`:
   - Manages active overlays: `Navigator`, `WhichKey`, `CreationDialogs`, `Settings`.
   - Ensures consistent geometry, margins, backdrop dimming, and Esc-routing.
2. Connect Normal Mode Root Shortcuts:
   - `Space f`: Open Navigator in `Files` mode.
   - `Space s`: Open Navigator in `Sessions` mode.
   - `Space c` / `Ctrl+P`: Open Navigator in `Commands` mode.
   - `Space o`: Open Navigator in `Outline` mode.
   - `Space l`: Open Navigator in `Links` mode.
   - `Space b`: Open Navigator in `Boards` mode.
   - `:`: Open command line.
   - `?`: Open contextual help.
3. Keep backward compatibility with existing command execution and editor workflows.

**Verification:**
Run `cargo test --workspace` and run native GUI smoke tests.

---

## 4. Acceptance Criteria for P0
- [x] User opens Files Panel by pressing `Space f`, searches for a file using fuzzy/substring search, navigates with `j/k`, and opens it with `Enter` completely keyboard-driven.
- [x] User presses `Ctrl+P` to open Command Palette, sees readable titles (e.g. «Открыть хранилище», «Сохранить заметку») without internal identifiers like `picker.commands`.
- [x] Which-Key displays as a multi-column, balanced grid with individual key badges and category indicators (`→`).
- [x] Pressing `?` opens contextual help showing actions relevant to the currently focused area.
- [x] Closing any overlay restores focus to the previously focused view deterministically.
- [x] No hardcoded colors in UI components; all surfaces use unified semantic tokens (`Theme`).
- [x] All workspace tests pass (`cargo test --workspace`).
- [x] Code follows `AGENTS.md` and repository guidelines (minimal changes, high performance, zero unnecessary allocations).

---

## Execution Handoff

Plan complete and saved to `docs/plans/2026-10-12-unified-ui-navigator-and-keyboard-system.md`.
