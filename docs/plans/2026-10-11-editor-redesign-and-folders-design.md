# Design Document: Truff Redesign & Hierarchical Folder Tree

## 1. Overview & Context

Truff is a minimalist, keyboard-driven Markdown knowledge base editor with Vim bindings and wiki-links.
While functionally robust, Truff currently resembles a technical prototype rather than a finished desktop editor like Obsidian.
This design document establishes the visual standard, spatial composition, typography hierarchy, tabbed navigation, and hierarchical folder tree support for Truff v2.

---

## 2. Design Standards & Principles

### Minimalist Desktop Editor · Subtle Terminal Aesthetic

1. **Strict Spatial Composition**:
   - **Readable Column (Markdown Editor)**: Constrained to 720–800 px (default 780 px) with symmetric left and right margins when available window width permits.
   - **Stable Positioning**: The horizontal start position of text (`origin.x = rect.left() + margin_x`) remains completely stable when switching notes.
   - **Sidebar Tree Width**: 240–300 px resizable.
   - **Spacing Scale**: 4, 8, 12, 16, 24, 32 px.
   - **Tree Row Height**: 28 px uniform.
   - **Tab Bar Height**: 36–38 px.
   - **Dividers**: Exactly 1.0 px low-contrast lines between panels.

2. **Typography Hierarchy**:
   - Two font families maximum: Proportional for reading & UI, Monospace for code, coordinates, and Vim keys.
   - **H1**: 28 px Semibold (line height 36 px)
   - **H2**: 22 px Semibold (line height 30 px)
   - **H3**: 18 px Medium (line height 26 px)
   - **Body text**: 15.5 px Regular (line height 24 px, ~1.55 ratio)
   - **Code blocks**: 13.5 px Monospace (line height 20 px)
   - **File tree items**: 12.5 px Regular
   - **Tabs**: 12.5 px Medium
   - **Status bar**: 11.5 px Regular / Monospace

3. **Nord Semantic Palette**:
   - **Editor Canvas**: `#2E3440` (Nord Polar Night 0)
   - **Panels (Sidebar, Right panel)**: `#242933` (subtle contrast to editor canvas)
   - **Dividers & Borders**: `#3B4252` / `#4C566A` (1 px)
   - **Selection / Hover**: `#434C5E` (35% alpha for hover, 70% for active selection)
   - **Primary text**: `#ECEFF4` (contrast 10+:1)
   - **Muted / metadata text**: `#9AA5B5` (contrast > 4.5:1)
   - **Accent**: `#88C0D0` (Nord Frost)
   - **Warm accents**: `#EBCB8B`, `#A3BE8C`, `#BF616A`

4. **Restrained Geometry**:
   - **Panels**: 0 px corner radius (monolithic dock without disconnected floating panels).
   - **Tabs**: 4–6 px top corner radius.
   - **File tree selection**: 4 px corner radius.
   - **Buttons & Badges**: 4 px corner radius.
   - **Popups & Dialogs (Which-key, Vault creation)**: 6–8 px corner radius with soft shadow.

---

## 3. Component Architecture & Data Flow

### 3.1. Hierarchical Folder Tree in Sidebar

- **Data Representation**:
  - `truff-store::catalog` enumerates all relative markdown paths recursively (`"folder/sub/note.md"`, `"Welcome.md"`).
  - `Shell::set_sidebar_files` parses these paths into a virtual prefix tree of `RowItem`s:
    - Folders: `RowItem { id, title: "folder", depth, is_folder: true, is_collapsed: true, data: None }`
    - Notes: `RowItem { id, title: "note.md", depth, is_folder: false, is_collapsed: false, data: Some("folder/note.md") }`
    - Items are sorted hierarchically: folders first alphabetically, then files alphabetically.
- **Tree Expansion & Auto-reveal**:
  - Folders containing the currently active open note are automatically expanded on vault load and note switch (`expand(ancestor_id)`).
  - Clicking a folder toggles expansion (`toggle_collapse`).
  - Clicking a note opens the note.
- **Vim Navigation**:
  - `h`: Collapses folder if open; jumps to parent if closed or if current item is a file.
  - `l`: Expands folder if closed; jumps into first child if open.
  - `Enter`: Toggles folder if folder; opens note if file.
  - `j` / `k`: Moves up and down across visible rows.
- **Row Virtualization**:
  - Retained at 28 px fixed row height via `egui::ScrollArea::vertical().show_rows`.

### 3.2. Centered Editor Layout (Readable Line Length)

- **Calculations in `EditorWidget::show`**:
  - `content_width = (rect.width() - 48.0).clamp(320.0, 780.0)`
  - `margin_x = ((rect.width() - content_width) / 2.0).max(24.0)`
  - `origin = rect.left_top() + Vec2::new(margin_x, line_y - scroll)`
- **Interactions**:
  - Mouse clicks and drag selection are transformed relative to `margin_x`.
  - Vim caret in `NORMAL` mode is a block (`8.5 px` width), in `INSERT` mode is a line (`1.5 px` width).
  - Flash navigation badges render accurately above glyphs.
  - Text position is stable across buffer switches.

### 3.3. Horizontal Tab Bar

- **Position**: Located directly above the editor area (height 36–38 px).
- **Items**:
  - All open buffers in `Workspace::buffers()`.
  - For each buffer: File icon `📄`, note title, dirty dot indicator `●` (`#88C0D0`), close button `×`.
- **States**:
  - Active: seamless with editor canvas (`#2E3440`), 4–6 px top corner radius, bright title `#ECEFF4`.
  - Inactive: subtle muted text `#9AA5B5`, hover highlight `#434C5E`.
- **Interactions**:
  - Click to activate buffer.
  - Click `×` to close view/buffer.

### 3.4. Refined Status Bar

- **Height**: 26–28 px, background `#1F232A`.
- **Left**: Vim mode pill badge (`NORMAL` / `INSERT` / `VISUAL`), dynamic command prompt `:cmd`, brief status messages.
- **Right**: Cursor coordinates (`Ln {line}, Col {col}` in monospace), `Markdown · UTF-8`.

---

## 4. Verification & Testing Plan

1. **Unit Tests**:
   - Path tree construction in `crates/ui/src/shell.rs` / `crates/core/src/navigation/state.rs`: verify folders and files are sorted, parent-child relationships and depths are correctly set, auto-expand correctly reveals deep notes.
   - Centered editor coordinate conversions: verify mouse click targeting and caret positioning.
2. **Integration / GUI Tests**:
   - `crates/ui/tests/gui.rs`: verify tab bar rendering, tab click switching, and folder expand/collapse via mouse and Vim motions.
   - `crates/ui/tests/navigation.rs`: test `h`/`l`/`Enter` in the hierarchical sidebar.
3. **Workspace Verification**:
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo test --workspace`
   - `cargo fmt --all -- --check`
