# Editor Typography and Spacing Hierarchy Design

## Summary
Refines the visual hierarchy in Truff's editor and Table of Contents (TOC):
1. **Document Title Size**: Increased from 24 pt (`base * 1.6`) to 30 pt (`base * 2.0`, min 28 pt), distinguishing it prominently from H1.
2. **Title-to-First-Line Spacing**: Increased visual gap from 10 pt to 32 pt (with `EditorWidget::TOP_MARGIN = 84.0 pt`).
3. **Title Persistence**: Retains document title even when the first content line starts with `# Heading`.
4. **Heading Spacing Hierarchy (Factor $k = 0.75$)**:
   - `#` (H1): 24 pt gap below (line height 48 pt at base 15 pt), exactly 3/4 of title-to-first-line gap (32 pt).
   - `##` (H2): 18 pt gap below (line height 38 pt at base 15 pt), $24 \times 0.75$.
   - `###` (H3): 13.5 pt gap below (line height 31 pt at base 15 pt), $18 \times 0.75$.
   - `####` (H4): 10 pt gap below (line height 26 pt at base 15 pt), $13.5 \times 0.75$.
   - `#####` (H5): 7.5 pt gap below (line height 23 pt at base 15 pt), $10 \times 0.75$.
   - `######` (H6): 5.5 pt gap below (line height 21 pt at base 15 pt), smoothly merging into regular body text line height (21 pt).
5. **Table of Contents (TOC) Item Spacing**:
   - `row_height` increased from 24.0 pt to 30.0 pt (+25%).
   - Item frame margin adjusted to `Margin::symmetric(6, 4)`.

## Impacted Files
- `crates/ui/src/native/editor/mod.rs`: `TOP_MARGIN` updated to 84.0.
- `crates/ui/src/native/editor/viewport.rs`: title font calculation, positioning, removal of `!first_is_h1` gate.
- `crates/ui/src/native/paint.rs`: `resolve_font_and_line_height` heading heights and spacing scale.
- `crates/ui/src/native/toc/render.rs`: `row_height` and frame inner margin.
- `crates/ui/tests/display.rs`: unit assertions on `TOP_MARGIN` and `resolve_font_and_line_height`.
