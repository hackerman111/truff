# Heading Folding in Obsidian Style & Vim Motions Design

## Overview
This document specifies the design for Markdown heading folding (`#`, `##`, `###`, etc.) in Truff, mirroring the collapsible heading behavior in Obsidian, complete with interactive UI indicators (clickable chevron, fold badge) and standard Vim folding motions (`za`, `zc`, `zo`, `zM`, `zR`).

## 1. Heading Folding Semantics
- **Fold Units**: Each Markdown heading (`#`..`######`) defines a foldable section.
- **Fold Range**: For a heading at 0-based buffer line $L$ with level $N$:
  - The hidden line range begins at line $L + 1$.
  - It extends until the line immediately preceding the next heading with level $\le N$ (or the end of the document).
  - Child headings (e.g. `###` within `##`) and their contents are subsumed within the fold.
- **Fold State Representation**:
  - `folded_headings: BTreeSet<usize>` tracks line indices of folded headings.
  - When lines are queried, a helper function `is_line_folded(line, &facts.headings, &folded_headings)` determines if a line is within any active fold.

## 2. Editor & Viewport Integration
- **Zero Document Mutation**: Folding never alters the underlying `Buffer` or `crop::Rope`. Edits, transactions, undo/redo, and formatting remain 100% stable.
- **Height Calculation (`HeightIndex`)**:
  - Folded lines receive height `0.0` in `HeightIndex`.
  - Ensures accurate scrollbars and virtual line calculations without dummy allocations.
- **Viewport Layout & Rendering**:
  - `EditorWidget::show` skips folded lines in layout calculation and paint passes.
  - The heading line itself renders:
    - A chevron indicator to the left of the heading: `▼` if unfolded, `▶` if folded.
    - An ellipsis badge `...` at the end of the heading line when folded.
    - Clicking the chevron or badge toggles the fold via `EditorInput::ToggleFold { heading_line }`.

## 3. Vim Motions (`z + ...`)
Vim parser and execution support:
- `za` (`FoldCommand::Toggle`): Toggle fold under cursor (or parent heading if cursor is in body).
- `zc` (`FoldCommand::Close`): Close (fold) heading under cursor.
- `zo` (`FoldCommand::Open`): Open (unfold) heading under cursor.
- `zM` (`FoldCommand::CloseAll`): Close all headings in current document.
- `zR` (`FoldCommand::OpenAll`): Open all headings in current document.

When closing a fold (`zc` or `za`), if the cursor is currently on a hidden line within the folded section, the cursor moves to the heading line.
