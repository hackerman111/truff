# Markdown Editor Behavior Contract (MEBC-1) Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Implement the full normative behavior contract specified in `00_MARKDOWN_EDITOR_BEHAVIOR_CONTRACT.md` (MEBC-1) across `truff-core` and `truff-ui`, ensuring correctness, Vim-first keyboard control, smart Markdown transformations, and strict non-regression.

**Architecture:** Core owns purely semantic Markdown edit planners (`plan_enter`, `plan_tab`, `plan_shift_tab`, `toggle_task`), Vim motions (`zz`, `zt`, `zb`, `*`, `#`, `%`, `gf`), and syntax snapshots. UI owns input dispatch, caret/selection projection, viewport positioning, link navigation and completion overlay, respecting single-focus input boundaries and atomic transactions.

**Tech Stack:** Rust 2024, `truff-core`, `truff-ui`, `crop::Rope`, `pulldown-cmark`, `egui 0.36.2`.

---

### Task 1: Refine Smart Edit Core Semantics according to §3.1 and §3.2

**Files:**
- Modify: `crates/core/src/markdown/smart_edit.rs`
- Test: `crates/core/tests/smart_edit_mebc1.rs`

**Step 1: Write the failing tests**
Create comprehensive table-driven tests in `crates/core/tests/smart_edit_mebc1.rs` covering:
1. Bullet lists (`- `, `* `, `+ `, `  - `) with `Enter` continuation and mid-line splitting (`- ab|cd` → `- ab\n- |cd`).
2. Tasks: `- [ ] todo` → `- [ ] todo\n- [ ] `, `- [x] done` → `- [x] done\n- [ ] ` (unchecked continuation).
3. Numbering: `1. first` → `1. first\n2. `, `7) later` → `7) later\n8) `; verify subsequent numbers are NOT mass-renumbered automatically (per §3.1 contract).
4. Blockquotes: `> quote` → `> quote\n> `, and blockquote with list: `> - item` → `> - item\n> - `.
5. Empty root list item `- |` and empty quote `> |` exits list/quote.
6. Empty nested item `  - |`: single Enter unindents one level to parent `- |`; second Enter exits.
7. Plain text `plain|`: normal `\n`, returns `None`.
8. Fenced code block: Enter inside ```code``` does NOT create list/quote markers.
9. YAML frontmatter: Enter inside `---` does NOT create list/quote markers.
10. `Tab` and `Shift+Tab`: Indent/outdent with default 2 spaces, preserving child subtree hierarchy.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-core --test smart_edit_mebc1`
Expected: FAIL due to missing code fence/YAML detection, renumbering differences, or 2-space subtree indenting.

**Step 3: Implement minimal changes in smart_edit.rs**
- Update `plan_enter` to check whether cursor/line is in frontmatter or fenced code block (using `MarkdownSnapshot` or fence scanning).
- Stop mass-renumbering subsequent items on `Enter` (increment only the new item per §3.1).
- In empty item handling, if nested (`indent.len() > 0`), lift nesting level by indent step (2 spaces) rather than deleting the entire line.
- Update `plan_tab` and `plan_shift_tab` to use 2 spaces as standard indent unit and adjust child lines in the subtree if present.
- Support mid-line split preserving marker on next line.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-core --test smart_edit_mebc1`
Expected: PASS.

**Step 5: Verify workspace**
Run: `cargo test -p truff-core`
Expected: PASS.

---

### Task 2: Core Vim Motions: `zz`, `zt`, `zb`, `*`, `#`, `%`, `gf` (§2, §7)

**Files:**
- Modify: `crates/core/src/vim/parser/mod.rs`
- Modify: `crates/core/src/vim/parser/command.rs`
- Modify: `crates/core/src/vim/motion/types.rs`
- Modify: `crates/core/src/vim/motion/cursor.rs`
- Modify: `crates/core/src/vim/execute/command.rs`
- Test: `crates/core/tests/vim_mebc1_motions.rs`

**Step 1: Write the failing tests**
In `crates/core/tests/vim_mebc1_motions.rs`, test:
1. `zz`, `zt`, `zb` parsed as `Command::ScrollCursor(ScrollPosition::Center | Top | Bottom)` or motion.
2. `*`: searches forward for word under cursor.
3. `#`: searches backward for word under cursor.
4. `%`: jumps to matching bracket `()`, `[]`, `{}`.
5. `gf`: parsed as `Command::FollowLink` or `Motion::FollowLink`.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-core --test vim_mebc1_motions`
Expected: FAIL (types / parsing not yet implemented).

**Step 3: Implement minimal code in Vim parser & execute**
- Add `ScrollPosition` (`Center`, `Top`, `Bottom`) and `Command::ScrollCursor(ScrollPosition)` or `Motion::ScrollCenter/ScrollTop/ScrollBottom`.
- Add `Command::SearchWordUnderCursor { forward: bool }` triggered by `*` and `#`.
- Add `Motion::MatchingBracket` triggered by `%`, evaluating position of corresponding bracket `()`, `[]`, `{}` in buffer.
- Add `Command::FollowLink` triggered by `gf`.
- Implement execution in `crates/core/src/vim/execute/command.rs` and `cursor.rs`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-core --test vim_mebc1_motions`
Expected: PASS.

---

### Task 3: Editor Key & Viewport Integration in UI (§1, §2, §3.2, §7)

**Files:**
- Modify: `crates/ui/src/native/events.rs`
- Modify: `crates/ui/src/editor/input.rs`
- Modify: `crates/ui/src/native/editor/viewport.rs`
- Modify: `crates/ui/src/native/editor/mod.rs`
- Test: `crates/ui/tests/editor_mebc1_keys.rs`

**Step 1: Write the failing tests**
In `crates/ui/tests/editor_mebc1_keys.rs`, test:
1. `zz` centers viewport around cursor without changing source cursor byte pos.
2. `zt` scrolls cursor to top of viewport without changing cursor byte pos.
3. `zb` scrolls cursor to bottom of viewport without changing cursor byte pos.
4. `Shift+Enter` in Insert mode produces a literal newline `\n` without smart list continuation.
5. `Ctrl+Enter` on a task line toggles `- [ ]` ↔ `- [x]`.
6. Clicking checkbox in LivePreview generates the exact same toggle transaction.
7. `gf` on `[[Note]]` or `[link](path)` triggers link following.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test editor_mebc1_keys`
Expected: FAIL.

**Step 3: Implement minimal code in UI**
- Handle `Command::ScrollCursor` in UI editor viewport:
  - `Center`: `self.scroll = caret_y - rect.height() / 2.0;`
  - `Top`: `self.scroll = caret_y;`
  - `Bottom`: `self.scroll = caret_y - rect.height() + LINE_HEIGHT;`
  - Clamp scroll safely.
- In `native/events.rs`:
  - When `Key::Enter` with `modifiers.shift`: emit `EditorInput::InsertText("\n")` directly instead of smart newline.
  - When `Key::Enter` with `modifiers.ctrl` in Editor: call `toggle_task`.
  - Handle `Command::FollowLink`: find link at cursor using `MarkdownFacts::extract` / `MarkdownSnapshot`, and dispatch `OpenPath` or navigation action.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test editor_mebc1_keys`
Expected: PASS.

---

### Task 4: Link Completion & Knowledge Navigation (§5)

**Files:**
- Modify: `crates/ui/src/app/completion.rs`
- Modify: `crates/ui/src/native/events.rs`
- Modify: `crates/ui/src/native/content.rs`
- Test: `crates/ui/tests/link_mebc1_completion.rs`

**Step 1: Write the failing tests**
In `crates/ui/tests/link_mebc1_completion.rs`, test:
1. Typing `[[` inside editor Insert triggers link completion popup.
2. `[[Note#` offers headings from that note.
3. Selecting a candidate replaces `[[...` with `[[Target]]` or `[[Target|alias]]`.
4. Visual mode selection + `Space l` / `link.create`: wraps selected text as `[[Target|selected]]`.
5. `gf` jump on link updates jump history; `Esc` or jump back returns.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test link_mebc1_completion`
Expected: FAIL.

**Step 3: Implement minimal code in UI completion & link creation**
- Ensure completion triggers on `[[` in editor Insert mode, pulling available notes from Catalog / Index.
- Support heading suggestions after `#`.
- Ensure `note.link_selection` correctly wraps selected text as `[[Target|text]]`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test link_mebc1_completion`
Expected: PASS.

---

### Task 5: Display & Presentation Contract (§4, §6)

**Files:**
- Modify: `crates/ui/src/editor/display/projection.rs`
- Modify: `crates/ui/src/editor/display/markers.rs`
- Modify: `crates/ui/src/native/editor/paragraph/paint.rs`
- Test: `crates/ui/tests/display_mebc1_contract.rs`

**Step 1: Write the failing tests**
In `crates/ui/tests/display_mebc1_contract.rs`, test:
1. Source mode shows all raw markdown characters without any marker hiding.
2. Live Preview reveals raw markers when cursor/selection intersects the token.
3. Reading mode unconditionally hides all markers and renders clean typography.
4. Unicode selection geometry on `W`, `Ш`, `🧑🏽‍💻`, `e\u{0301}` correctly matches shaped glyph boundaries.
5. Single-step Undo restores exact bytes and selection range across formatting operations.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui --test display_mebc1_contract`
Expected: FAIL or passes if already covered, ensure all edge cases are tested.

**Step 3: Implement minimal code adjustments**
- Ensure all token kinds in §4.3 matrix have proper display presentation or fallback.
- Verify selection rendering under glyphs does not degrade contrast with custom span/mark colors.

**Step 4: Run test to verify it passes**
Run: `cargo test -p truff-ui --test display_mebc1_contract`
Expected: PASS.

---

### Task 6: Full Workspace Verification & Acceptance Matrix (§9)

**Files:**
- Review: `crates/core`, `crates/ui`, `crates/store`
- Test: all workspace tests

**Step 1: Run all workspace tests**
`cargo test --workspace`

**Step 2: Run formatting and clippy**
`cargo fmt --all -- --check`
`cargo clippy --workspace --all-targets --all-features -- -D warnings`

**Step 3: Acceptance Matrix (§9) Check-off**
Verify all P0, P1, P2, P3 criteria.
