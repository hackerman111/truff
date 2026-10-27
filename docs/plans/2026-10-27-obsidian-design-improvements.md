# Obsidian Design Improvements Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Transform Truff v2's visual appearance to match Obsidian's calm, readable, and well-organized interface: soft panel contrast, clean tabs, generous file tree rows with pill highlights, 15px typography with 1.55 line height, 720px centered text, warm note title, distinct H1–H6 and list hierarchies, sharp-cornered selections with cap-height bounds, and visible current line highlight.

**Architecture:** 
1. `crates/ui/src/theme.rs` & `geometry.rs`: Harmonize Nord palette so sidebar lightness is close to editor canvas; set compact/comfortable geometry tokens (720px markdown width, 15px base editor font, 30px tree row height, 0.0 selection radius).
2. `crates/ui/src/app/global_config.rs`: Update typography defaults to 15px font size and 1.55 line height multiplier; add `show_inline_title` configuration flag.
3. `crates/ui/src/native/sidebar.rs`: Render active file with a wide, soft pill plate (`CornerRadius::same(6)`) without IDE-style vertical left line; soften indentation guide lines with reduced alpha; increase breathing room.
4. `crates/ui/src/native/tabs.rs`: Remove top 2px accent stroke on active tab; blend active tab seamlessly into the canvas background; soften borders.
5. `crates/ui/src/native/paint.rs`: Differentiate H1–H6 colors (warm gold H1 `#ebcb8b`, purple H2 `#b48ead`, cyan H3 `#88c0d0`, blue H4 `#81a1c1`, sage H5 `#a3be8c`); ensure heading text is bolder/stronger.
6. `crates/ui/src/native/editor/paragraph/paint.rs`: Render sharp-cornered selections and cursor (`corner_radius: 0.0`); bound selection to cap-height (scaling for headings); render active line highlight in both Normal and Insert modes; draw nested list guide lines; add soft background cards for quotes and code blocks.
7. `crates/ui/src/native/editor/viewport.rs` & `content.rs`: Render inline note title in warm gold at the top of the note (when enabled and not already starting with H1).

**Tech Stack:** Rust, egui 0.36, epaint, truff-core, truff-ui.

---

### Task 1: Update Theme Tokens & Geometry

**Files:**
- Modify: `crates/ui/src/theme.rs:136-192`
- Modify: `crates/ui/src/theme/geometry.rs:38-78`
- Modify: `crates/ui/tests/editor_input.rs:215-219`

**Step 1: Update `Theme::nord()` palette and geometry tokens**
- In `Theme::nord()`: set `bg_canvas = #2e3440`, `bg_panel = #313642` (close in lightness, soft transition), `border_subtle = #3b4252`, `selection = #3d4758`.
- In `GeometryTokens::compact()`:
  - `tree_row_height: 30.0` (was 28.0)
  - `markdown_max_width: 720.0` (was 800.0)
  - `editor_font_size: 15.0` (was 14.0)
- In `GeometryTokens::comfortable()`:
  - `markdown_max_width: 750.0` (was 850.0)
  - `editor_font_size: 16.0` (was 15.0)
- Update test assertion in `editor_input.rs`: `assert_eq!(nord.geometry.tree_row_height, 30.0);`.

**Step 2: Verify compilation and tests**
Run: `cargo test -p truff-ui --test editor_input`

---

### Task 2: Typography Config & Settings Option

**Files:**
- Modify: `crates/ui/src/app/global_config.rs:21-49`
- Modify: `crates/ui/src/native/creation/settings.rs:15-60, 200-260`

**Step 1: Add `show_inline_title` to TypographyConfig**
- Set `default_line_height_multiplier() -> 1.55`.
- Add `pub show_inline_title: bool` (default `true`) to `TypographyConfig`.
- Expose checkbox in `SettingsDialog` UI.

**Step 2: Verify compilation and tests**
Run: `cargo test -p truff-ui --test settings_dialog`

---

### Task 3: Sidebar File Tree & Active Pill Styling

**Files:**
- Modify: `crates/ui/src/native/sidebar.rs:180-200, 290-312`

**Step 1: Replace IDE active stroke with soft rounded pill**
- When `is_active`: set `frame.fill` to `color(theme.selection).gamma_multiply(0.7)` with `corner_radius: CornerRadius::same(6)` and generous padding.
- Remove `painter.line_segment` vertical stroke at `rect.left()`.
- Soften vertical guide lines: use `color(theme.border_subtle).gamma_multiply(0.3)`.

**Step 2: Verify compilation and sidebar tests**
Run: `cargo test -p truff-ui --test native sidebar_frame_response_toggles_folders_and_opens_files`

---

### Task 4: Tab Bar & Active Tab De-cluttering

**Files:**
- Modify: `crates/ui/src/native/tabs.rs:10-20, 80-95, 177-186`

**Step 1: Soften active tab and remove top stroke**
- Remove top 2px line segment on active tab.
- Frame stroke on active tab set to `egui::Stroke::NONE`.
- Tab bar top panel stroke set to `egui::Stroke::NONE` or subtle bottom divider only.
- Inactive tabs: muted text, hover fill with low alpha.

**Step 2: Verify compilation**
Run: `cargo check -p truff-ui`

---

### Task 5: Markdown Hierarchy & Heading Styles in `paint.rs`

**Files:**
- Modify: `crates/ui/src/native/paint.rs:8-46, 58-90, 105-115`
- Modify: `crates/ui/tests/native.rs:175-202`

**Step 1: Update heading sizes and colors**
- H1: `(base * 1.8).round()` (~27px), `#ebcb8b` (Aurora Gold), bold.
- H2: `(base * 1.45).round()` (~22px), `#b48ead` (Aurora Purple), bold.
- H3: `(base * 1.2).round()` (~18px), `#88c0d0` (Frost Cyan), bold.
- H4: `(base * 1.2).round()` (~18px), `#81a1c1` (Frost Blue), bold.
- H5/H6: `base` (15px), `#a3be8c` / `#8fbcbb`.
- Ensure 4-level font scale test in `native.rs` stays valid (`sizes.len() <= 4`).

**Step 2: Verify tests pass**
Run: `cargo test -p truff-ui --test native editor_font_sizes_share_four_levels_and_keep_configured_base`

---

### Task 6: Sharp Selection, Cap-Height Bounds & Current Line Highlight

**Files:**
- Modify: `crates/ui/src/native/editor/paragraph/paint.rs:90-155, 200-275`

**Step 1: Selection & Cursor sharp corners and cap-height bounds**
- In `paint_selection_range`: `painter.rect_filled(rect, 0.0, color)` (sharp 90-degree corners).
- In `paint` selection / cursor: set corner radius to `0.0`.
- Vertical bounds for selections: compute bounds based on cap-height of the glyphs / heading size, so body text is tightly bounded by capital letters, and heading selections grow proportionally with font size.
- Active line highlight: enable when `view.cursor` is on the line in BOTH `Normal` and `Insert` modes. Use a clearly visible yet calm tinted bar: `Color32::from_rgba_unmultiplied(180, 142, 173, 28)` (soft lilac tint matching Obsidian).
- Nested list guidelines: if `self.presentation.list.depth > 0`, paint subtle vertical guide lines connecting list indent levels.
- Blockquote & code block cards: add soft background card behind blockquote and code block paragraphs.

**Step 2: Verify compilation and visual mode tests**
Run: `cargo test -p truff-ui --test native test_visual_mode_character_and_line_highlighting_dimensions`

---

### Task 7: Inline Note Title Rendering in Viewport

**Files:**
- Modify: `crates/ui/src/native/editor/viewport.rs:360-400`
- Modify: `crates/ui/src/native/content.rs:25-45`

**Step 1: Display Note Title at Top of Editor Canvas**
- In `content.rs`: extract active buffer title (stripping `.md`).
- Pass title into `editor.show(..., note_title)`.
- In `viewport.rs`: if `typography.show_inline_title` is true and document does not start with `# `, paint note title at `rect.top() + Self::TOP_MARGIN - 34.0 - self.scroll` using font size 24.0 in warm gold `#ebcb8b`.

**Step 2: Run all tests and checks**
Run:
- `cargo check --tests -p truff-ui`
- `cargo test -p truff-ui --test native`
