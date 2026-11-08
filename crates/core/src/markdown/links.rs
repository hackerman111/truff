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

/// Extracts `(target, heading, alias)` from the inner text of a wiki-link (without surrounding `[[` and `]]`).
pub fn split_wiki_link_inner(inner: &str) -> (String, Option<String>, Option<String>) {
    let (path_part, alias_part) = match inner.find('|') {
        Some(idx) => {
            let path_end = if idx > 0 && inner.as_bytes()[idx - 1] == b'\\' {
                idx - 1
            } else {
                idx
            };
            (
                &inner[..path_end],
                Some(inner[idx + 1..].trim().to_string()),
            )
        }
        None => (inner, None),
    };

    let (target, heading) = match path_part.find('#') {
        Some(idx) => (
            path_part[..idx].trim().to_string(),
            Some(path_part[idx + 1..].trim().to_string()),
        ),
        None => (path_part.trim().to_string(), None),
    };

    (target, heading, alias_part)
}

impl LinkOccurrence {
    /// Parses a wiki-link string without surrounding `[[` and `]]`.
    /// Handles escaped table pipes (`[[note\|alias]]`) and `#heading` / `#^block` anchors.
    pub fn parse_wiki_inner(inner: &str, full_range: Range<usize>, ordinal: usize) -> Self {
        let (target, heading, label) = split_wiki_link_inner(inner);

        Self {
            target,
            heading,
            label,
            range: full_range,
            ordinal,
            is_wiki: true,
        }
    }

    /// Returns `Some(block_id)` if the link points to a `^block-id` anchor.
    pub fn block_ref(&self) -> Option<&str> {
        self.heading.as_deref()?.strip_prefix('^')
    }
}

/// Resolved destination of a clicked or followed inline link span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkDestination {
    External(String),
    InternalHeading(String),
    Note {
        path: String,
        heading: Option<String>,
    },
}

/// Extracts the destination surrounding a rendered link label/content `span_content` in `source_text`.
pub fn extract_link_destination(
    source_text: &str,
    span_content: &Range<usize>,
) -> Option<LinkDestination> {
    let suffix = source_text.get(span_content.end..)?;
    let prefix = source_text.get(..span_content.start)?;

    let target = if suffix.starts_with("]]") && prefix.ends_with("[[") {
        let inner = source_text.get(span_content.clone())?;
        inner.split('|').next().unwrap_or(inner).trim()
    } else if suffix.starts_with("]]") && prefix.ends_with('|') {
        let open_pos = prefix.rfind("[[")?;
        let inside = &prefix[open_pos + 2..];
        if inside.contains("]]") {
            return None;
        }
        inside.trim_end_matches('|').trim_end_matches('\\').trim()
    } else {
        ""
    };

    if !target.is_empty() {
        return classify_internal_target(target);
    }

    if let Some(tail) = source_text.get(span_content.end..)
        && let Some(after_paren) = tail.strip_prefix("](")
        && let Some(close_paren) = after_paren.find(')')
    {
        let raw_url = &after_paren[..close_paren];
        let url = raw_url.split_whitespace().next().unwrap_or(raw_url.trim());
        let clean = url.trim_matches(['<', '>']);
        if clean.starts_with("http://")
            || clean.starts_with("https://")
            || clean.starts_with("mailto:")
            || clean.starts_with("ftp://")
        {
            return Some(LinkDestination::External(clean.to_string()));
        }
        if clean.starts_with("www.") {
            return Some(LinkDestination::External(format!("https://{clean}")));
        }
        if let Some(dest) = classify_internal_target(clean) {
            return Some(dest);
        }
    }

    if let Some(slice) = source_text.get(span_content.clone()) {
        let trimmed = slice.trim().trim_matches(['<', '>']);
        if trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("mailto:")
        {
            return Some(LinkDestination::External(trimmed.to_string()));
        }
    }
    None
}

fn classify_internal_target(target: &str) -> Option<LinkDestination> {
    if let Some(heading) = target.strip_prefix('#') {
        let h_trim = heading.trim();
        return (!h_trim.is_empty()).then(|| LinkDestination::InternalHeading(h_trim.to_string()));
    }
    if let Some((note, heading)) = target.split_once('#') {
        let n_trim = note.trim();
        let h_trim = heading.trim();
        return (!n_trim.is_empty()).then(|| LinkDestination::Note {
            path: n_trim.to_string(),
            heading: (!h_trim.is_empty()).then(|| h_trim.to_string()),
        });
    }
    (!target.is_empty()).then(|| LinkDestination::Note {
        path: target.to_string(),
        heading: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_link_destination_markdown_internal_heading() {
        let text = "Check [TOC](#heading-1) now";
        let start = text.find("TOC").unwrap();
        let span = start..start + 3;
        assert_eq!(
            extract_link_destination(text, &span),
            Some(LinkDestination::InternalHeading("heading-1".into()))
        );
    }

    #[test]
    fn test_extract_link_destination_markdown_note_and_heading() {
        let text = "See [doc](notes/guide.md#setup) here";
        let start = text.find("doc").unwrap();
        let span = start..start + 3;
        assert_eq!(
            extract_link_destination(text, &span),
            Some(LinkDestination::Note {
                path: "notes/guide.md".into(),
                heading: Some("setup".into()),
            })
        );
    }

    #[test]
    fn test_extract_link_destination_wikilink_heading_only() {
        let text = "Go to [[#Section Two]]";
        let start = text.find("#Section Two").unwrap();
        let span = start..start + "#Section Two".len();
        assert_eq!(
            extract_link_destination(text, &span),
            Some(LinkDestination::InternalHeading("Section Two".into()))
        );
    }

    #[test]
    fn test_extract_link_destination_wikilink_note_and_heading() {
        let text = "Refer to [[My Note#Important Points|Important]]";
        let start = text.find("My Note#Important Points|Important").unwrap();
        let span = start..start + "My Note#Important Points|Important".len();
        assert_eq!(
            extract_link_destination(text, &span),
            Some(LinkDestination::Note {
                path: "My Note".into(),
                heading: Some("Important Points".into()),
            })
        );
        let alias_start = text.find("Important]]").unwrap();
        let alias_span = alias_start..alias_start + "Important".len();
        assert_eq!(
            extract_link_destination(text, &alias_span),
            Some(LinkDestination::Note {
                path: "My Note".into(),
                heading: Some("Important Points".into()),
            })
        );
    }

    #[test]
    fn test_extract_link_destination_external_url() {
        let text = "Visit [Google](https://google.com)";
        let start = text.find("Google").unwrap();
        let span = start..start + 6;
        assert_eq!(
            extract_link_destination(text, &span),
            Some(LinkDestination::External("https://google.com".into()))
        );
    }
}
