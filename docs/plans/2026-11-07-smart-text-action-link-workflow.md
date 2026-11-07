# Smart Text Action Link Workflow Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Streamline link creation in `<Space> c` so selected text immediately wraps as `[[selected]]` on `n`, URL creation prompts only for URL, and empty cursor positions allow entering both target and custom title.

**Architecture:** Refactor `TextActionOverlay` state machine to track `has_selection`, eliminate `LinkTitleMode`, allow opening `<Space> c` at empty cursor locations (`cursor..cursor`), render dimmed styling for unavailable style actions when no text is selected, and support dual-field (`Target` + `Title`) input with `Tab` navigation.

**Tech Stack:** Rust 2024 edition, `egui`, `truff-core` buffers/transactions, `truff-ui` shell and overlays.

---

### Task 1: Update `TextActionOverlay` State Machine

**Files:**
- Modify: `crates/ui/src/overlays/text_action.rs`
- Test: `crates/ui/src/overlays/text_action.rs`

**Step 1: Write the failing unit tests**
In `crates/ui/src/overlays/text_action.rs::tests`:
- `test_root_menu_with_selection_n_immediately_applies`: pressing `n` with selection returns `ApplyLink { is_url: false, target, title: None }`.
- `test_root_menu_with_selection_u_opens_single_url_input`: pressing `u` transitions directly to `LinkInput { is_url: true, has_selection: true, .. }`.
- `test_root_menu_without_selection_rejects_styles`: pressing `c`, `h`, or `x` returns `TextActionResponse::None` (or stays in Root).
- `test_root_menu_without_selection_n_opens_dual_input`: pressing `n` without selection transitions to `LinkInput { is_url: false, has_selection: false, .. }`.
- `test_link_input_tab_and_enter`: Tab switches `active_field` between 0 and 1; Enter emits `ApplyLink`.

**Step 2: Run test to verify it fails**
Run: `cargo test -p truff-ui overlays::text_action`
Expected: Compilation errors or test failures due to old `LinkTitleMode` and missing `has_selection`.

**Step 3: Implement state machine changes**
In `crates/ui/src/overlays/text_action.rs`:
- Update `TextActionMode`:
  ```rust
  pub enum TextActionMode {
      Root { has_selection: bool },
      ColorSelect { is_highlight: bool },
      LinkInput {
          is_url: bool,
          has_selection: bool,
          target_input: String,
          title_input: String,
          active_field: usize,
      },
  }
  ```
- In `open(&mut self, initial_text: Option<String>, cursor_pos: Option<[f32; 2]>)`:
  Set `has_selection = initial_text.as_ref().map_or(false, |s| !s.trim().is_empty())`.
  Set `self.mode = TextActionMode::Root { has_selection }`.
- Handle `Key::Char('n')` in Root:
  - If `has_selection`: return `TextActionResponse::ApplyLink { is_url: false, target: self.initial_text.clone().unwrap_or_default(), title: None }` and `self.close()`.
  - If `!has_selection`: transition to `LinkInput { is_url: false, has_selection: false, target_input: String::new(), title_input: String::new(), active_field: 0 }`.
- Handle `Key::Char('u')` in Root:
  - If `has_selection`: transition to `LinkInput { is_url: true, has_selection: true, target_input: String::new(), title_input: self.initial_text.clone().unwrap_or_default(), active_field: 0 }`.
  - If `!has_selection`: transition to `LinkInput { is_url: true, has_selection: false, target_input: String::new(), title_input: String::new(), active_field: 0 }`.
- Handle `Key::Char('c')`, `'h'`, `'x'` in Root:
  - If `!has_selection`: return `TextActionResponse::None` (or status message handled by caller).
- Handle `Key::Tab` in `LinkInput`:
  - If `!has_selection`: `*active_field = (*active_field + 1) % 2`.

**Step 4: Run unit tests to verify they pass**
Run: `cargo test -p truff-ui overlays::text_action`
Expected: PASS (all tests pass).

**Step 5: Commit**
```bash
git add crates/ui/src/overlays/text_action.rs
git commit -m "feat(ui): update text action overlay state machine for smart link workflow"
```

---

### Task 2: Support Empty Cursor Range and Link Insertion in `SelectionTarget`

**Files:**
- Modify: `crates/ui/src/app/selection.rs`

**Step 1: Write test for empty cursor range link insertion**
In `crates/ui/src/app/selection.rs` or test suite:
Verify `apply_note_link_target` and `apply_url_link_target` handle empty `target.range` correctly.

**Step 2: Implement selection capture and link application**
In `crates/ui/src/app/selection.rs`:
- Add `pub(crate) fn capture_selection_or_cursor(&self) -> AppResult<(SelectionTarget, bool)>`:
  - Try `self.capture_selection()`. If `Ok(t)`, return `Ok((t, true))`.
  - Try `word_boundary_at(buffer, cursor)`. If `Some(range)`, return `Ok((SelectionTarget { view, buffer, revision, range }, true))`.
  - Otherwise, return `Ok((SelectionTarget { view, buffer, revision, range: cursor..cursor }, false))`.
- Update `apply_note_link_target`:
  - When `target.range.is_empty()`:
    - If `custom_title` is `Some(title)` and `!title.is_empty()`: insert `[[note|title]]`.
    - Else: insert `[[note]]`.
  - When `!target.range.is_empty()`:
    - If `custom_title` is `Some(title)`: insert `[[note|title]]`.
    - Else: insert `[[note]]`.
- Update `apply_url_link_target`:
  - When `target.range.is_empty()`:
    - Label is `custom_title.unwrap_or(url)`.
    - Insert `[label](url)`.
  - When `!target.range.is_empty()`:
    - Label is `custom_title.unwrap_or(buffer text)`.
    - Insert `[label](url)`.

**Step 3: Run existing and new selection tests**
Run: `cargo test -p truff-ui selection`
Expected: PASS.

**Step 4: Commit**
```bash
git add crates/ui/src/app/selection.rs
git commit -m "feat(ui): support empty cursor link insertion and capture_selection_or_cursor"
```

---

### Task 3: Update Native UI Rendering and Keyboard Routing

**Files:**
- Modify: `crates/ui/src/native/events.rs`
- Modify: `crates/ui/src/native/text_action.rs`

**Step 1: Update event routing for `<Space> c`**
In `crates/ui/src/native/events.rs`:
- In `open_text_action_menu`:
  - Call `self.app.capture_selection_or_cursor()`.
  - If `has_selection`: pass `Some(text)` to `open()`.
  - If `!has_selection`: pass `None` to `open()`.
- In `handle_text_action_response`:
  - Handle `TextActionResponse::ApplyLink { is_url, target, title }`:
    - If `is_url`: call `self.application.apply_url_link_target(&target_sel, &target, title.as_deref())`.
    - If `!is_url`: call `self.application.apply_note_link_target(&target_sel, &target, title.as_deref())`.
    - Switch mode back to `Mode::Normal`.
- In `key()` interceptor for `TextActionOverlay`:
  - If `mode` is `Root { has_selection: false }` and user presses `c`, `h`, or `x`:
    Set `self.status_message = Some("Выделите текст или поместите курсор на слово для применения стиля".into())`.

**Step 2: Update rendering in `text_action.rs`**
In `crates/ui/src/native/text_action.rs`:
- In `show_root`:
  - If `has_selection`: render `[n] Note link [[text]]` with preview of `initial_text`.
  - If `!has_selection`:
    - Render `[n] Note link` and `[u] URL link` with normal styling.
    - Render `[c] Color`, `[h] Highlight`, `[x] Remove` with muted dimmed text color.
- In `show_link_input`:
  - If `has_selection` and `is_url`:
    - Show `Text: "<selected_text>"`
    - Render single input: `URL: [ ... ]`
  - If `!has_selection`:
    - If `!is_url`:
      - Input 0: `Заметка:`
      - Input 1: `Заголовок (опционально):`
    - If `is_url`:
      - Input 0: `URL:`
      - Input 1: `Заголовок:`
    - Show hint: `[Tab] Сменить поле · [Enter] Применить · [Esc] Отмена`
  - Request focus on the input corresponding to `active_field`.

**Step 3: Run UI test suite**
Run: `cargo test -p truff-ui --test text_action_menu`
Expected: Compile check and initial test feedback.

**Step 4: Commit**
```bash
git add crates/ui/src/native/events.rs crates/ui/src/native/text_action.rs
git commit -m "feat(ui): update native UI rendering for smart text action menu"
```

---

### Task 4: Integration Tests and Workspace Verification

**Files:**
- Modify: `crates/ui/tests/text_action_menu.rs`

**Step 1: Update and add integration tests**
In `crates/ui/tests/text_action_menu.rs`:
- `test_visual_selection_n_instant_wikilink`: verify visual selection turns into `[[selected]]` on `n` without extra prompts.
- `test_normal_mode_word_n_instant_wikilink`: verify word under cursor turns into `[[word]]` on `n`.
- `test_selection_u_single_input`: verify URL prompt appears, typing URL + Enter produces `[selected](url)`.
- `test_empty_cursor_note_link_dual_fields`: verify on empty line typing note name + Tab + title produces `[[note|title]]`.
- `test_empty_cursor_note_link_no_title`: verify typing note name + Enter produces `[[note]]`.
- `test_empty_cursor_url_link_dual_fields`: verify on empty line typing url + Tab + title produces `[title](url)`.
- `test_empty_cursor_style_warning`: verify pressing `c` on empty cursor produces status message and does not crash.

**Step 2: Run all text action tests**
Run: `cargo test -p truff-ui --test text_action_menu`
Expected: PASS (all tests pass).

**Step 3: Run full verification suite**
Run:
```bash
cargo fmt --all -- --check
cargo clippy -p truff-ui --all-targets --all-features -- -D warnings
cargo test -p truff-ui --test text_action_menu
cargo check --workspace
```
Expected: All exit 0 with clean output.

**Step 4: Commit**
```bash
git add crates/ui/tests/text_action_menu.rs
git commit -m "test(ui): add comprehensive tests for smart text action link workflow"
```
