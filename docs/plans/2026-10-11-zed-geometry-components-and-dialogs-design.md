# Design: Zed Geometry, Component System & Styled Dialogs

**Date:** 2026-10-11  
**Status:** Approved  
**Topic:** Unified Component System, Vector Icons, Ghost Buttons, and Styled Dialogs for Truff v2

---

## 1. Context & Motivation

While Truff v2 adopted a clean four-tier palette, several UI details retained standard `egui` defaults:
1. **Sidebar header buttons:** Rendered using `egui::small_button`, creating visible grey rectangles and borders in the header rather than smooth ghost icon actions.
2. **Missing glyphs (Tofu boxes):** Unicode circles (`●`) and other symbols rendered as empty rectangular boxes depending on system font fallback.
3. **Dialog windows:** The vault creation dialog relied on egui's default OS-style window frame with a grey title-bar, pitch-black input boxes (`#000000`), and small default buttons.

## 2. Core Architecture & Component System

A dedicated module `crates/ui/src/native/widgets.rs` and `crates/ui/src/native/icons.rs` unifies visual geometry across all windows and panels.

### Component Tokens & Metrics

| Component | Dimensions | Corner Radius | Visual States |
|-----------|------------|---------------|---------------|
| `IconButton` | 28 × 28 px | 5 px | Default: transparent; Hover: `#394351`; Pressed: `#2D333F` |
| `Button` (Primary) | Height 32 px | 5 px | Fill `#88A9C1`, text `#20242C` bold; Hover: `#9DBFD6` |
| `Button` (Secondary) | Height 32 px | 5 px | Default: transparent; Hover: `#394351` |
| `TextInput` | Height 36 px | 5 px | Fill `#20242C`, border 1px `#333945`, focus border 1.5px `#88A9C1` |
| `TreeItem` | Height 28 px | 4 px | Default: transparent; Hover: 0.4 `#394351`; Selected: `#394351`; Active: 2px left accent `#88A9C1` |
| `Tab` | Height 34 px | 0 px | Active: `#2D333F` with 2px bottom accent `#88A9C1`; Inactive: `#20242C` |
| `PanelHeader` | Height 36 px | 0 px | Clean label + right-aligned `IconButton` row |
| `Dialog` | Width 460 px | 8 px | Fill `#252A34`, border 1px `#333945`, padding 20 px |

### Vector Procedural Icons (`icons.rs`)

To eliminate missing font glyphs and guarantee razor-sharp rendering on all platforms:
- Lines drawn with `egui::Painter` with exact `1.5` px stroke thickness.
- Palette-aware (`#8B95A5` muted, `#E2E6ED` active, `#88A9C1` accent).
- Supported glyphs:
  - `Icon::File`: crisp document outline.
  - `Icon::Folder`: clean folder outline.
  - `Icon::ChevronRight` / `Icon::ChevronDown`: 1.5 px angle brackets.
  - `Icon::Plus`: symmetric cross.
  - `Icon::Close`: 45° cross.
  - `Icon::Vault`: library/storage outline.
  - `Icon::MoreHorizontal`: three dots `⋯`.
- `draw_dirty_dot(painter, center, color)`: solid circle with diameter 5.5 px (radius 2.75 px) rendered directly into the canvas.

## 3. Sidebar & Tabs Polish

1. **Header (36 px):**
   - Left: `VAULT` label (11 px, `#8B95A5`, bold).
   - Right: seamless 28×28 px ghost `IconButton`s:
     - `Icon::Plus` / Note
     - `Icon::Folder` + Plus
     - `Icon::Vault`
     - `Icon::MoreHorizontal` (collapse all, toggle)
   - Zero background frames in default state; 2 px gap between buttons.
2. **File Tree (28 px):**
   - 16 px indent per depth.
   - Vector chevron and folder/note icons.
   - Distinct active (2 px left accent line `#88A9C1`), selected (`#394351`), and dirty dot (5.5 px circle).
3. **Tabs (34 px):**
   - Active tab connects seamlessly to `#2D333F` editor with 2 px bottom accent.
   - Inactive tabs on `#20242C` shell with 1 px subtle divider.
   - Hover-only close button with vector `Icon::Close`.

## 4. Modal Dialogs Overhaul

1. **Window Frame:**
   - `title_bar(false)`: system title bar removed.
   - Background `#252A34`, border `#333945`, corner radius 8 px, inner margin 20 px, width 460 px.
2. **Left-aligned Title & Close Action:**
   - Title: 16 px `#E2E6ED` bold.
   - Subtitle: 12 px `#8B95A5`.
   - Top-right `IconButton` with `Icon::Close`.
3. **Inputs:**
   - 36 px height, `#20242C` background, 5 px radius, 1 px `#333945` border, clear hints.
4. **Action Buttons:**
   - Right-aligned: `SecondaryButton` ("Отмена") + `PrimaryButton` ("Создать и открыть" / "Создать").
   - Keyboard navigation: `Enter` submits, `Esc` dismisses.
5. Consistent styling across:
   - Vault Creation & Selection Dialog
   - New Note Dialog
   - New Folder Dialog
