# Design Document: Obsidian Minimalist UI & Nord Theme System

- **Date**: 2026-10-11
- **Status**: Approved
- **Topic**: `obsidian-nord-ui-redesign`

## 1. Problem Statement & Motivation
Currently, Truff v2 renders all interface elements and Markdown notes using a single monospace font (`FontId::monospace(16.0)`), flat unpadded rectangular panels, and stark borders. This produces a raw terminal/TUI aesthetic rather than a neat, modern Markdown knowledge workspace like Obsidian.

The goal is to transform the UI into a clean, minimalist design modeled after Obsidian:
1. Replace pervasive monospace fonts with clean proportional typography for UI and Markdown content, while preserving monospace strictly for code blocks, inline code, and Vim command input.
2. Introduce heading scale hierarchy for Markdown `# H1`, `## H2`, `### H3`.
3. Provide a tree-like file explorer with hierarchy indentation, folder disclosure icons (`▾`/`▸`), and rounded selection badges.
4. Implement a configurable theme system in `~/.config/truff/config.toml` supporting custom themes, with the **Nord** palette as the default theme.

---

## 2. Typography & Markdown Editor Styling

### 2.1 Font Stack Strategy
In `crates/ui/src/native/paint.rs`, `editor.rs`, and `layout.rs`:
- UI labels, headings, and note text use `FontId::proportional(...)` (egui's proportional font family).
- Monospace font `FontId::monospace(...)` is retained only for:
  - Markdown fenced code blocks (` ``` `) and inline code (`` `code` ``).
  - Vim command buffer (`:command`).
  - Flash navigation badges (`s` key in Normal mode).

### 2.2 Heading Hierarchy & Line Heights
In `paint.rs` during layout job construction:
- **H1** (`# `): `FontId::proportional(22.0)`, line-height `32.0px`, bold weight, `text_primary` color.
- **H2** (`## `): `FontId::proportional(18.5)`, line-height `28.0px`, semibold weight.
- **H3** (`### `): `FontId::proportional(16.5)`, line-height `25.0px`, medium weight.
- **Body Text**: `FontId::proportional(15.0)`, line-height `24.0px`.
- **Code**: `FontId::monospace(14.0)`, line-height `22.0px`.

Line height calculation in `HeightIndex` adjusts per paragraph galley size, preserving stable scroll anchoring without content jumping.

### 2.3 Editor Spacing & Caret
- Add horizontal margin/padding (18–24px) to the editor canvas to prevent text from clinging to panel borders.
- Smooth caret rendering with subtle corner radius (1.5px).

---

## 3. Vault File Tree & UI Panel Polish

### 3.1 Vault Explorer (`crates/ui/src/native/mod.rs`)
Leverage existing `RowItem` metadata (`depth`, `is_folder`, `is_collapsed`):
- Indentation: `10.0 + depth * 14.0px`.
- Disclosure icons:
  - Folders: `▾ ` (open) or `▸ ` (collapsed).
  - Notes: subtle document indicator or clean text label.
- Selection indicator: Rounded pill rect (`Rounding::same(4.0)`), using `theme.selection` with subtle alpha, replacing raw rectangular full-row highlight.
- Smooth hover highlights on unselected items.

### 3.2 Top Bar & Status Bar
- **Top bar**: Minimalist header showing active note name, muted path context, and a subtle dirty indicator `●` when modified.
- **Status bar**:
  - Left: Vim mode as a neat badge/pill (`NORMAL`, `INSERT`, `VISUAL`).
  - Center/Right: Subtle word & character statistics or active status message, using `text_muted`.
  - Command mode: Clean `:command` prompt without visual clutter.

---

## 4. Theme System & Configuration

### 4.1 Global Config (`~/.config/truff/config.toml`)
Extend `GlobalConfig` in `crates/ui/src/app/global_config.rs`:
```toml
# Active theme (default: "nord")
theme = "nord"

# Optional custom user themes
[themes.custom_example]
bg_primary = "#2e3440"
bg_secondary = "#242933"
bg_tertiary = "#1f232a"
bg_surface = "#3b4252"
border = "#3b4252"
text_primary = "#eceff4"
text_muted = "#9aa5b5"
accent = "#88c0d0"
selection = "#434c5e80"
cursor = "#88c0d0"
```

### 4.2 Nord Palette (Default)
Based on Nord Polar Night, Snow Storm, and Frost:
- `bg_primary`: `#2e3440` (editor background)
- `bg_secondary`: `#242933` (sidebar / vault / backlinks)
- `bg_tertiary`: `#1f232a` (status bar)
- `bg_surface`: `#3b4252` (popups, floating dialogs)
- `border`: `#3b4252` (subtle panel boundaries)
- `text_primary`: `#eceff4` (readable foreground)
- `text_muted`: `#9aa5b5` (dimmed text)
- `accent`: `#88c0d0` (Nord Frost cyan)
- `selection`: `rgba(67, 76, 94, 0.5)`
- `cursor`: `#88c0d0`

Built-in themes:
- `nord` (Default)
- `obsidian-dark` (Obsidian Charcoal `#1e1e1e` + Violet `#7c3aed`)
- `light` (Clean modern light theme)

### 4.3 Runtime Theme Switching & Persistence
- The `:theme <name>` command:
  1. Looks up `<name>` in config `themes` or built-in presets.
  2. Applies the theme without reparsing Markdown syntax or invalidating buffer trees.
  3. Updates `GlobalConfig::theme` and saves to `config.toml`.

---

## 5. Architectural Invariants & Performance Constraints
In accordance with `AGENTS.md`:
1. **Zero Reparsing**: Switching themes or fonts must never invalidate Markdown parse trees or rebuild the text Rope.
2. **Virtualization Intact**: Virtualized paragraph layout and viewport bounding (`visible_window`) remain strictly enforced.
3. **Soundness & Test Pass Rate**: All existing workspace tests and GUI regression tests must pass without regressions.
