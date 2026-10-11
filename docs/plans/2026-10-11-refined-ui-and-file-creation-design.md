# Design Document: Truff Refined Visual Hierarchy, Zed/Obsidian Aesthetics & Creation UI

## 1. Overview & Goals

This document refines the visual hierarchy of Truff v2 towards the aesthetics of Obsidian and Zed:
- Dense, structured interface with clean 1px borders and no decorative clutter.
- 4-level color system for visual depth without drop-shadows or gradients.
- Seamless active tab connection with the editor canvas and hover-only close buttons.
- 240 px sidebar with distinct tree item states (`hover`, `selected`, `active`, `modified`).
- Clean minimalist `VAULT` header with action buttons (`+📄`, `+📁`, `📂`, `◫`) and modal dialogs for creating files, creating folders, and opening vaults.
- 48 px top breathing room in the editor canvas and centered 720 px document column.

---

## 2. Four-Tier Palette & Visual Hierarchy

- **Shell / Tab Bar**: `#20242C` (Deepest window tone)
- **Sidebar & Secondary Panels**: `#252A34` (Navigation background)
- **Editor Canvas**: `#2D333F` (Primary writing surface)
- **Interactive Hover & Selection**: `#394351`
- **Accent**: `#88A9C1` (Nordic blue-gray for active indicators, focus, and mode pills)
- **Dividers & Borders**: `#333945` (1.0 px low-contrast)
- **Text Primary**: `#E2E6ED`
- **Text Muted / Metadata**: `#8B95A5`

---

## 3. Component Design & Behaviors

### 3.1. Tabs (Obsidian + Zed Style)
- Height: 36 px.
- **Active Tab**:
  - Fill: `#2D333F` (matches editor canvas).
  - Seamless bottom: no separator stroke between active tab and editor canvas.
  - 2 px bottom accent line `#88A9C1`.
  - Title in `#E2E6ED` Medium 12.5 px.
- **Inactive Tabs**:
  - Transparent resting background, `#8B95A5` text.
  - Soft hover background `#394351` (corner radius 4 px).
  - 1 px vertical divider between adjacent inactive tabs.
- **Close Button `×`**:
  - Hidden in resting state.
  - Rendered only on tab hover.
  - Unsaved dot `●` (`#88A9C1`) shown whenever the buffer is dirty.

### 3.2. Sidebar Header & Creation UI
- Default sidebar width: 240 px (resizable 160–500 px).
- Header row:
  - Left: `VAULT` (11 px Bold, `#8B95A5`) + vault directory name.
  - Right: Action button group:
    - `+📄` New Note
    - `+📁` New Folder
    - `📂` Open / Switch Vault
    - `◫` Collapse / Expand All
- Creation Modals:
  - **New Note Dialog**: Name input (supports nested paths e.g. `docs/api.md`), `Enter` to create & open, `Esc` to cancel.
  - **New Folder Dialog**: Name input, creates folder, updates tree.
  - **Vault Dialog**: Path input with completion, recent vaults list.

### 3.3. File Tree States
- Row height: 28 px.
- Indent: 16 px per level.
- Monochrome icons: `▸ 📁` / `▾ 📁` for folders, `📄` for notes.
- Separate visual states:
  - `hover`: `#394351` soft background.
  - `selected`: `#394351` contrast background for tree cursor.
  - `active`: Left 2 px accent line `#88A9C1` for the note currently open in the editor.
  - `modified`: Accent dot `●` for dirty notes.

### 3.4. Editor Layout & Typography
- **Top Breathing Room**: 48 px vertical margin between tab bar and first line of text.
- **Column Width**: Centered 720 px (`clamp(320.0, 720.0)`).
- **Typography**:
  - Body: 15.0 px Regular, line height 24 px (ratio 1.6).
  - H1: 28.0 px Semibold, line height 36 px.
  - H2: 22.0 px Semibold, line height 30 px.
  - H3: 18.0 px Medium, line height 26 px.
  - Code: 13.5 px Monospace.

### 3.5. Status Bar
- Height: 26 px, fill `#1B1E24`, 1 px top border `#333945`.
- Left: Mode pill (`NORMAL` in `#333945`, `INSERT` in `#88A9C1`, `VISUAL` in `#EBCB8B`).
- Right: Monospace `Ln {line}, Col {col}` and `Markdown · UTF-8`.
