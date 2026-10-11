use crate::text::Transaction;
use std::ops::Range;

/// Helper for constructing link creation transactions from editor selections.
pub struct LinkSelection;

impl LinkSelection {
    /// Creates a transaction replacing `range` with `[[target]]` or `[[target|label]]`.
    pub fn wikilink(range: Range<usize>, target: &str, label: Option<&str>) -> Transaction {
        let text = match label {
            Some(lbl) => format!("[[{target}|{lbl}]]"),
            None => format!("[[{target}]]"),
        };
        Transaction::replace(range, text)
    }

    /// Creates a transaction replacing `range` with `[label](target)`.
    pub fn markdown_link(range: Range<usize>, target: &str, label: &str) -> Transaction {
        let text = format!("[{label}]({target})");
        Transaction::replace(range, text)
    }
}
