# Smart Note and Folder Import Design

## Context & Problem Statement
Users frequently assemble composite documents, summaries, or daily notes by importing contents of existing notes or entire folders. Currently, users have to manually open notes, copy text, and paste it into the destination document. 

The goal is to provide a smart import mechanism using the intuitive syntax `![[target]]!`.
- When targeting a note (`![[My Note]]!`), the macro is atomically replaced in-place with the note's full contents (literal copy-paste mode).
- When targeting a folder (`![[projects/]]!` or `![[projects]]!`), all `.md` files in that folder and its subfolders are imported recursively in alphabetical order, each prefixed by an `# {Note Title}` header.
- The trigger works both upon typing the closing `!` after `![[target]]` and via the auto-completion popup triggered by `![[`.
- Self-import (importing the current active note or a folder containing it) is prevented to avoid recursion/duplication.
- Extensible structure (`ImportOptions`) for future metadata/frontmatter selective filtering.
- Atomic undo: pressing `u` in Normal mode reverts the entire import operation in one step.

---

## Architecture & Component Responsibilities

### 1. `truff_core::markdown::smart_import` (`crates/core/src/markdown/smart_import.rs`)
Pure domain logic:
- `ImportMacroMatch`: start, end byte offsets, and target string.
- `detect_import_macro(line_str: &str, line_start: usize, cursor_in_line: usize) -> Option<ImportMacroMatch>`: scans for `![[` and `]]!` on the current line ending at the cursor.
- `ImportOptions`: configuration struct (e.g. `include_frontmatter: bool = true`, with placeholders for future tag/metadata filters).
- `FolderImportEntry`: struct containing `relative_path`, `title`, and `content`.
- `format_folder_import(entries: &[FolderImportEntry], options: &ImportOptions) -> String`: formats multiple entries into `# {title}\n\n{content}\n\n`.
- `build_import_transaction(range: Range<usize>, content: String) -> Transaction`: creates atomic single-undo transaction.

### 2. Vault Resolver (`crates/ui/src/app/completion.rs` & `crates/ui/src/app/mod.rs`)
- `Application::resolve_smart_import(&self, target: &str) -> Result<String, String>`:
  - Validates active vault presence.
  - Resolves target as single file (`target` or `target.md`) or folder (`target` or `target/`).
  - Guards against self-import by checking `self.active_path()`.
  - Reads contents directly and formats using `smart_import`.

### 3. Autocomplete & Keystroke Trigger (`crates/ui/src/overlays/link_completion.rs`, `crates/ui/src/native/events.rs`)
- `LinkCompletionOverlay::detect_trigger`: distinguishes between `[[` (WikiLink) and `![[` (SmartImport).
- `refresh_link_completions`: populates notes and folders when import mode is active.
- `apply_link_completion`: when triggered from `![[`, directly replaces `![[query` with the resolved content.
- `native/events.rs`: typing `!` in Insert mode checks `detect_import_macro` and triggers immediate replacement if matched.

---

## Invariants & Safety
- **No panics or unwraps**: IO and path resolution fail gracefully with a status bar message.
- **Cycle / Self-import safety**: The currently active file cannot import itself. When importing a folder containing the current file, the current file is excluded.
- **Atomic Undo**: Single `Transaction` with `UndoGroup::next()`.
- **Minimal allocations**: String operations use borrowed slices where possible; folder scan only reads `.md` files.

---

## Testing Strategy
- Unit tests in `crates/core/tests/smart_import.rs` for macro detection, folder formatting, and option handling.
- Integration tests in `crates/ui/tests/smart_import.rs` verifying single note import, folder recursive import, autocomplete selection, self-import prevention, and undo behavior.
