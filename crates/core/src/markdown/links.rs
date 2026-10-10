use std::ops::Range;

use serde::{Deserialize, Serialize};

/// Occurrence of a link (wiki-link or standard Markdown link) in a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkOccurrence {
    /// Target note name, path, or URL.
    pub target: String,
    /// Optional heading anchor inside the target document (e.g. `#Heading`).
    pub heading: Option<String>,
    /// Optional display alias or Markdown link label.
    pub label: Option<String>,
    /// Exact byte range in the source document.
    pub range: Range<usize>,
    /// Sequential 0-indexed ordinal of this link occurrence in the document.
    pub ordinal: usize,
    /// `true` if this link was specified using `[[...]]` syntax.
    pub is_wiki: bool,
}

impl LinkOccurrence {
    /// Parses a wiki-link string without surrounding `[[` and `]]`.
    pub fn parse_wiki_inner(inner: &str, full_range: Range<usize>, ordinal: usize) -> Self {
        let (path_part, alias_part) = match inner.find('|') {
            Some(idx) => (&inner[..idx], Some(inner[idx + 1..].trim().to_string())),
            None => (inner, None),
        };

        let (target, heading) = match path_part.find('#') {
            Some(idx) => (
                path_part[..idx].trim().to_string(),
                Some(path_part[idx + 1..].trim().to_string()),
            ),
            None => (path_part.trim().to_string(), None),
        };

        Self {
            target,
            heading,
            label: alias_part,
            range: full_range,
            ordinal,
            is_wiki: true,
        }
    }
}
