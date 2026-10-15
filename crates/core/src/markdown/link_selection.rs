use std::ops::Range;
use std::path::PathBuf;

use crate::text::{Buffer, BytePos, Transaction, UndoGroup};

/// Target specification for creating a Wiki-link or Markdown link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkTarget {
    pub note: String,
    pub heading: Option<String>,
    pub block_id: Option<String>,
    pub alias: Option<String>,
    pub use_wikilink: bool,
    pub inside_table: bool,
}

/// Pure source-level plan for extracting a selection into a new note and replacing it with a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractNotePlan {
    pub source_transaction: Transaction,
    pub target_path: PathBuf,
    pub extracted_content: String,
}

/// Helper for constructing link creation transactions from editor selections.
pub struct LinkSelection;

impl LinkSelection {
    /// Creates a transaction replacing `range` with `[[target]]` or `[[target|label]]`.
    pub fn wikilink(range: Range<usize>, target: &str, label: Option<&str>) -> Transaction {
        let text = match label {
            Some(lbl) if lbl != target => format!("[[{target}|{lbl}]]"),
            _ => format!("[[{target}]]"),
        };
        Transaction::replace(range, text)
    }

    /// Creates a transaction replacing `range` with `[label](target)`.
    pub fn markdown_link(range: Range<usize>, target: &str, label: &str) -> Transaction {
        let text = format!("[{label}]({target})");
        Transaction::replace(range, text)
    }

    /// Plans wrapping `selected_range` in `buffer` into a Wiki-link or standard Markdown link.
    /// Preserves the selected text as alias/label when `target.alias` is `None` and selection is non-empty.
    pub fn plan_wrap_selection(
        buffer: &Buffer,
        selected_range: Range<usize>,
        target: &LinkTarget,
    ) -> Option<Transaction> {
        if selected_range.start > selected_range.end
            || selected_range.end > buffer.byte_len()
            || !buffer.is_char_boundary(selected_range.start)
            || !buffer.is_char_boundary(selected_range.end)
        {
            return None;
        }
        let selected_text = buffer.slice(selected_range.clone()).to_string();
        let effective_alias = target
            .alias
            .clone()
            .or_else(|| (!selected_text.is_empty()).then_some(selected_text));

        let mut dest = target.note.trim().to_string();
        if let Some(ref block) = target.block_id {
            dest.push_str("#^");
            dest.push_str(block.trim().trim_start_matches('^'));
        } else if let Some(ref heading) = target.heading {
            dest.push('#');
            dest.push_str(heading.trim().trim_start_matches('#'));
        }

        let replacement = if target.use_wikilink {
            match effective_alias {
                Some(alias) if alias != dest => {
                    let clean_alias = alias.replace("]]", "");
                    let sep = if target.inside_table { "\\|" } else { "|" };
                    format!("[[{dest}{sep}{clean_alias}]]")
                }
                _ => format!("[[{dest}]]"),
            }
        } else {
            let label = effective_alias
                .unwrap_or_else(|| target.note.clone())
                .replace(']', "\\]");
            let escaped_dest = dest.replace(')', "%29");
            format!("[{label}]({escaped_dest})")
        };

        let end_cursor = BytePos::new(selected_range.start + replacement.len());
        Some(
            Transaction::replace(selected_range, replacement)
                .with_cursor(end_cursor, end_cursor)
                .with_undo_group(UndoGroup::next()),
        )
    }

    /// Produces a pure refactor plan to extract `selected_range` into `target_path` and replace
    /// the selection with a link to `note_name`. Does not perform disk I/O.
    pub fn plan_extract_note(
        buffer: &Buffer,
        selected_range: Range<usize>,
        target_path: PathBuf,
        note_name: &str,
        alias: Option<String>,
    ) -> Option<ExtractNotePlan> {
        if selected_range.is_empty()
            || selected_range.start > selected_range.end
            || selected_range.end > buffer.byte_len()
            || !buffer.is_char_boundary(selected_range.start)
            || !buffer.is_char_boundary(selected_range.end)
        {
            return None;
        }
        let extracted_content = buffer.slice(selected_range.clone()).to_string();
        let target = LinkTarget {
            note: note_name.to_string(),
            heading: None,
            block_id: None,
            alias,
            use_wikilink: true,
            inside_table: false,
        };
        let source_transaction = Self::plan_wrap_selection(buffer, selected_range, &target)?;
        Some(ExtractNotePlan {
            source_transaction,
            target_path,
            extracted_content,
        })
    }
}
