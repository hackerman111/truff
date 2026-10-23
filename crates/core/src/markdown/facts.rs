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

/// Occurrence of a `#tag` in a Markdown document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagFact {
    /// Normalized tag name without the leading `#` (e.g. `zettelkasten`, `project/v2`).
    pub name: String,
    /// Source byte range of the tag including `#`.
    pub range: Range<usize>,
}

/// Structured document facts extracted from a Markdown file.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DocFacts {
    pub headings: Vec<HeadingFact>,
    pub links: Vec<LinkOccurrence>,
    pub blocks: Vec<BlockFact>,
    #[serde(default)]
    pub tags: Vec<TagFact>,
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
        let est_cap = (text.len() / 1024).clamp(8, 2048);
        let mut code_ranges: Vec<Range<usize>> = Vec::with_capacity(est_cap);
        let mut headings: Vec<HeadingFact> = Vec::with_capacity(est_cap);
        let mut standard_links: Vec<LinkOccurrence> = Vec::with_capacity(est_cap);

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
        let mut wiki_links: Vec<LinkOccurrence> = Vec::with_capacity(est_cap);
        let bytes = text.as_bytes();
        let mut i = 0;
        let mut code_cursor = CodeRangeCursor::new(&code_ranges);
        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[' {
                // Check if escaped: \[[
                if i > 0 && bytes[i - 1] == b'\\' {
                    i += 2;
                    continue;
                }

                // Check if inside any code range
                if code_cursor.contains(i) {
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

        // 3. Combine already sorted links chronologically by byte position, assigning ordinal in O(L)
        let all_links = merge_sorted_links(standard_links, wiki_links);

        // 4. Pass 3: Extract block IDs `^block-id`
        let mut blocks: Vec<BlockFact> = Vec::new();
        let mut byte_offset = 0;
        let mut block_cursor = CodeRangeCursor::new(&code_ranges);
        for line in text.lines() {
            let line_len = line.len();
            let line_start = byte_offset;
            let line_end = byte_offset + line_len;

            // Check if line is inside a code block
            let inside_code = block_cursor.contains_range_boundary(line_start, line_end);
            if !inside_code && let Some(hat_pos) = line.rfind('^') {
                let candidate = line[hat_pos + 1..].trim();
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

        // 5. Pass 4: Extract inline `#tags` outside code blocks, links, HTML color hex, and headings
        let tags = extract_inline_tags(text, &code_ranges, &all_links);

        Ok(DocFacts {
            headings,
            links: all_links,
            blocks,
            tags,
        })
    }

    /// Fast-path extraction of headings only (used for outline and TOC navigation).
    pub fn extract_headings_only(text: &str) -> Result<Vec<HeadingFact>, MarkdownError> {
        let mut headings = Vec::new();
        let options = Options::empty();
        let parser = Parser::new_ext(text, options).into_offset_iter();
        let mut current_heading: Option<(u8, usize, String)> = None;

        for (event, range) in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    current_heading = Some((level as u8, range.start, String::new()));
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
                Event::Text(txt) | Event::Code(txt) => {
                    if let Some((_, _, ref mut h_text)) = current_heading {
                        h_text.push_str(&txt);
                    }
                }
                _ => {}
            }
        }
        Ok(headings)
    }
}

struct CodeRangeCursor<'a> {
    ranges: &'a [Range<usize>],
    idx: usize,
}

impl<'a> CodeRangeCursor<'a> {
    #[inline]
    fn new(ranges: &'a [Range<usize>]) -> Self {
        Self { ranges, idx: 0 }
    }

    #[inline]
    fn contains(&mut self, pos: usize) -> bool {
        while self.idx < self.ranges.len() && self.ranges[self.idx].end <= pos {
            self.idx += 1;
        }
        self.idx < self.ranges.len() && self.ranges[self.idx].contains(&pos)
    }

    #[inline]
    fn contains_range_boundary(&mut self, start: usize, end: usize) -> bool {
        while self.idx < self.ranges.len() && self.ranges[self.idx].end <= start {
            self.idx += 1;
        }
        if self.idx >= self.ranges.len() {
            return false;
        }
        self.ranges[self.idx].contains(&start) || self.ranges[self.idx].contains(&end)
    }
}

fn merge_sorted_links(
    standard: Vec<LinkOccurrence>,
    wiki: Vec<LinkOccurrence>,
) -> Vec<LinkOccurrence> {
    let mut merged = Vec::with_capacity(standard.len() + wiki.len());
    let mut std_it = standard.into_iter().peekable();
    let mut wiki_it = wiki.into_iter().peekable();

    while let (Some(s), Some(w)) = (std_it.peek(), wiki_it.peek()) {
        if s.range.start <= w.range.start {
            merged.push(std_it.next().unwrap());
        } else {
            merged.push(wiki_it.next().unwrap());
        }
    }
    merged.extend(std_it);
    merged.extend(wiki_it);

    for (idx, link) in merged.iter_mut().enumerate() {
        link.ordinal = idx;
    }
    merged
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

/// Extracts `#tag` occurrences from markdown text, excluding code blocks, links, headings, and hex colors.
pub fn extract_inline_tags(
    text: &str,
    code_ranges: &[Range<usize>],
    links: &[LinkOccurrence],
) -> Vec<TagFact> {
    let mut tags = Vec::new();
    let mut code_cursor = CodeRangeCursor::new(code_ranges);
    let mut link_idx = 0;
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'#' {
            i += 1;
            continue;
        }

        // Skip multiple `#` (headings or `##`)
        if i + 1 < bytes.len() && bytes[i + 1] == b'#' {
            while i < bytes.len() && bytes[i] == b'#' {
                i += 1;
            }
            continue;
        }

        // Must be preceded by start-of-line, whitespace, or opening punctuation `(` / `[` / `"`
        if i > 0 {
            let prev = text[..i].chars().next_back().unwrap_or(' ');
            if !prev.is_whitespace() && !matches!(prev, '(' | '[' | '{' | '"' | '\'') {
                i += 1;
                continue;
            }
        }

        // Skip if inside code block / inline code
        if code_cursor.contains(i) {
            i += 1;
            continue;
        }

        // Skip if inside a link
        while link_idx < links.len() && links[link_idx].range.end <= i {
            link_idx += 1;
        }
        if link_idx < links.len() && links[link_idx].range.contains(&i) {
            i = links[link_idx].range.end;
            continue;
        }

        // Parse tag characters after `#`: must start with alphabetic or `_`
        let after = &text[i + 1..];
        let mut chars = after.char_indices();
        let Some((_, first_ch)) = chars.next() else {
            break;
        };
        if !first_ch.is_alphabetic() && first_ch != '_' {
            i += 1;
            continue;
        }

        let mut end_rel = first_ch.len_utf8();
        let mut has_alpha = first_ch.is_alphabetic();
        for (idx, ch) in chars {
            if ch.is_alphanumeric() || matches!(ch, '_' | '-' | '/') {
                if ch.is_alphabetic() {
                    has_alpha = true;
                }
                end_rel = idx + ch.len_utf8();
            } else {
                break;
            }
        }

        let tag_slice = after[..end_rel].trim_end_matches(['/', '-', '_']);
        if !tag_slice.is_empty() && has_alpha {
            let tag_end = i + 1 + tag_slice.len();
            tags.push(TagFact {
                name: tag_slice.to_string(),
                range: i..tag_end,
            });
            i = tag_end;
        } else {
            i += 1;
        }
    }

    tags
}
