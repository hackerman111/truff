use std::fmt;
use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};

use super::links::LinkOccurrence;

/// Information extracted about a Markdown heading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadingFact {
    /// Heading level (1 to 6).
    pub level: u8,
    /// Heading plain text without markdown syntax.
    pub text: String,
    /// Normalized anchor slug for cross-referencing.
    pub anchor: String,
    /// Source byte range of the heading.
    pub range: Range<usize>,
}

/// Occurrence of a block identifier `^block-id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockFact {
    /// Extracted block identifier.
    pub id: String,
    /// Source byte range of the block identifier marker.
    pub range: Range<usize>,
}

/// Structured document facts extracted from a Markdown file.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DocFacts {
    pub headings: Vec<HeadingFact>,
    pub links: Vec<LinkOccurrence>,
    pub blocks: Vec<BlockFact>,
}

/// Errors occurring during document facts extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownError {
    ParseFailed(String),
}

impl fmt::Display for MarkdownError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseFailed(msg) => write!(f, "markdown parse failed: {msg}"),
        }
    }
}

impl std::error::Error for MarkdownError {}

/// Canonical document facts extractor.
pub struct MarkdownFacts;

impl MarkdownFacts {
    /// Extracts headings, links, and block IDs from markdown text.
    pub fn extract(text: &str) -> Result<DocFacts, MarkdownError> {
        let mut code_ranges: Vec<Range<usize>> = Vec::new();
        let mut headings: Vec<HeadingFact> = Vec::new();
        let mut standard_links: Vec<LinkOccurrence> = Vec::new();

        // 1. Pass 1: Extract code ranges, headings, and standard markdown links via pulldown-cmark
        let mut options = Options::all();
        options.remove(Options::ENABLE_WIKILINKS);
        let parser = Parser::new_ext(text, options).into_offset_iter();

        let mut in_code_block_start = None;
        let mut current_heading: Option<(u8, usize, String)> = None;
        let mut current_link: Option<(usize, String, String)> = None;

        for (event, range) in parser {
            match event {
                Event::Start(Tag::CodeBlock(_)) => {
                    in_code_block_start = Some(range.start);
                }
                Event::End(TagEnd::CodeBlock) => {
                    if let Some(start) = in_code_block_start.take() {
                        code_ranges.push(start..range.end);
                    }
                }
                Event::Code(code) => {
                    code_ranges.push(range);
                    if let Some((_, _, ref mut h_text)) = current_heading {
                        h_text.push_str(&code);
                    }
                    if let Some((_, _, ref mut l_text)) = current_link {
                        l_text.push_str(&code);
                    }
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    let lvl = level as u8;
                    current_heading = Some((lvl, range.start, String::new()));
                }
                Event::End(TagEnd::Heading(_)) => {
                    if let Some((level, start, heading_text)) = current_heading.take() {
                        let trimmed = heading_text.trim().to_string();
                        let anchor = slugify(&trimmed);
                        headings.push(HeadingFact {
                            level,
                            text: trimmed,
                            anchor,
                            range: start..range.end,
                        });
                    }
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    current_link = Some((range.start, dest_url.to_string(), String::new()));
                }
                Event::End(TagEnd::Link) => {
                    if let Some((start, dest, label)) = current_link.take() {
                        let label_opt = if label.is_empty() {
                            None
                        } else {
                            Some(label.trim().to_string())
                        };
                        standard_links.push(LinkOccurrence {
                            target: dest,
                            heading: None,
                            label: label_opt,
                            range: start..range.end,
                            ordinal: 0,
                            is_wiki: false,
                        });
                    }
                }
                Event::Text(txt) => {
                    if let Some((_, _, ref mut h_text)) = current_heading {
                        h_text.push_str(&txt);
                    }
                    if let Some((_, _, ref mut l_text)) = current_link {
                        l_text.push_str(&txt);
                    }
                }
                _ => {}
            }
        }

        // 2. Pass 2: Extract Wiki-links `[[...]]` with code block and escape exclusion
        let mut wiki_links: Vec<LinkOccurrence> = Vec::new();
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[' {
                // Check if escaped: \[[
                if i > 0 && bytes[i - 1] == b'\\' {
                    i += 2;
                    continue;
                }

                // Check if inside any code range
                let is_inside_code = in_code(&code_ranges, i);
                if is_inside_code {
                    i += 2;
                    continue;
                }

                // Find matching closing `]]`
                let search_slice = &text[i + 2..];
                if let Some(close_idx) = search_slice.find("]]") {
                    let end = i + 2 + close_idx + 2;
                    let inner = &search_slice[..close_idx];
                    let link = LinkOccurrence::parse_wiki_inner(inner, i..end, 0);
                    wiki_links.push(link);
                    i = end;
                    continue;
                }
                // No subsequent opening can have a closing delimiter either.
                break;
            }
            i += 1;
        }

        // 3. Combine and sort all links chronologically by byte position, assigning ordinal
        let mut all_links: Vec<LinkOccurrence> =
            Vec::with_capacity(standard_links.len() + wiki_links.len());
        all_links.extend(standard_links);
        all_links.extend(wiki_links);
        all_links.sort_by_key(|l| l.range.start);
        for (idx, link) in all_links.iter_mut().enumerate() {
            link.ordinal = idx;
        }

        // 4. Pass 3: Extract block IDs `^block-id`
        let mut blocks: Vec<BlockFact> = Vec::new();
        let mut byte_offset = 0;
        for line in text.lines() {
            let line_len = line.len();
            let line_start = byte_offset;
            let line_end = byte_offset + line_len;

            // Check if line is inside a code block
            let inside_code = in_code(&code_ranges, line_start) || in_code(&code_ranges, line_end);
            if !inside_code && let Some(hat_pos) = line.rfind('^') {
                let candidate = &line[hat_pos + 1..].trim();
                if !candidate.is_empty()
                    && candidate
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    let match_start = line_start + hat_pos;
                    let match_end = match_start + 1 + candidate.len();
                    blocks.push(BlockFact {
                        id: candidate.to_string(),
                        range: match_start..match_end,
                    });
                }
            }

            // +1 for '\n' or +2 for '\r\n'
            byte_offset += line_len;
            if byte_offset < bytes.len() && bytes[byte_offset] == b'\r' {
                byte_offset += 1;
            }
            if byte_offset < bytes.len() && bytes[byte_offset] == b'\n' {
                byte_offset += 1;
            }
        }

        Ok(DocFacts {
            headings,
            links: all_links,
            blocks,
        })
    }
}

// Parser events produce disjoint source ranges in ascending order.
fn in_code(ranges: &[Range<usize>], pos: usize) -> bool {
    let index = ranges.partition_point(|range| range.end <= pos);
    ranges.get(index).is_some_and(|range| range.contains(&pos))
}

fn slugify(text: &str) -> String {
    let mut slug = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if (c.is_whitespace() || c == '-' || c == '_')
            && !slug.ends_with('-')
            && !slug.is_empty()
        {
            slug.push('-');
        }
    }
    slug.trim_end_matches('-').to_string()
}
