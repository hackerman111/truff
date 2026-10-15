use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};

use super::facts::{DocFacts, HeadingFact, MarkdownFacts};
use super::links::LinkOccurrence;
use super::style::{CalloutKind, ColorSpan, color_spans};
use crate::text::{Buffer, BufferId};
use crate::vim::motion::{SyntaxObjectRange, TextObjectTarget};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionFact {
    pub heading_index: usize,
    pub level: u8,
    pub parent_section: Option<usize>,
    pub heading_range: Range<usize>,
    /// Range from the end of the heading line to the end of the section.
    pub body_range: Range<usize>,
    /// Full section range including heading and nested subsections.
    pub full_range: Range<usize>,
    pub line_start: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFact {
    pub checked: bool,
    /// Byte offset of the character inside `[ ]` / `[x]`.
    pub checkbox_offset: usize,
    pub marker_range: Range<usize>,
    pub content_range: Range<usize>,
    pub line_range: Range<usize>,
    pub line_start: usize,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalloutFact {
    pub kind: CalloutKind,
    pub title: Option<String>,
    pub range: Range<usize>,
    pub content_range: Range<usize>,
    pub line_start: usize,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeBlockFact {
    pub language: Option<String>,
    pub range: Range<usize>,
    pub content_range: Range<usize>,
    pub line_start: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRowFact {
    pub range: Range<usize>,
    pub is_header: bool,
    pub cells: Vec<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableFact {
    pub range: Range<usize>,
    pub rows: Vec<TableRowFact>,
    pub line_start: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterFact {
    pub range: Range<usize>,
    pub content_range: Range<usize>,
    pub properties: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticKind {
    Frontmatter,
    Heading(u8),
    Section(u8),
    Link,
    WikiLink,
    BlockId,
    TaskItem,
    Callout,
    BlockQuote,
    CodeFence,
    Table,
    TableRow,
    TableCell,
    ColorSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticNode {
    pub kind: SemanticKind,
    pub range: Range<usize>,
    pub inner_range: Range<usize>,
    pub line_start: usize,
    pub depth: usize,
    pub semantic_id: Option<String>,
}

/// Immutable per-revision semantic snapshot of a Markdown buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownSnapshot {
    pub buffer_id: BufferId,
    pub revision: u64,
    pub facts: DocFacts,
    pub sections: Vec<SectionFact>,
    pub tasks: Vec<TaskFact>,
    pub callouts: Vec<CalloutFact>,
    pub quotes: Vec<Range<usize>>,
    pub code_blocks: Vec<CodeBlockFact>,
    pub tables: Vec<TableFact>,
    pub frontmatter: Option<FrontmatterFact>,
    pub color_spans: Vec<ColorSpan>,
    pub semantic_nodes: Vec<SemanticNode>,
}

impl MarkdownSnapshot {
    /// Builds a revision-tagged snapshot from a `Buffer`.
    pub fn from_buffer(buffer: &Buffer) -> Self {
        let text = buffer.as_str();
        Self::parse(buffer.id(), buffer.revision(), &text)
    }

    /// Parses a complete `MarkdownSnapshot` from raw text for `(buffer_id, revision)`.
    pub fn parse(buffer_id: BufferId, revision: u64, text: &str) -> Self {
        let facts = MarkdownFacts::extract(text).unwrap_or_default();
        let line_starts = compute_line_starts(text);
        let line_of = |offset: usize| -> usize {
            line_starts
                .partition_point(|&start| start <= offset)
                .saturating_sub(1)
        };

        let frontmatter = extract_frontmatter(text);
        let (code_blocks, tables, quotes) = extract_cmark_blocks(text, &line_of);
        let sections = build_sections(text, &facts.headings, &line_of);
        let (tasks, callouts) = extract_lines_tasks_and_callouts(text, &code_blocks);
        let colors = color_spans(text);

        let mut semantic_nodes = Vec::new();
        if let Some(ref fm) = frontmatter {
            semantic_nodes.push(SemanticNode {
                kind: SemanticKind::Frontmatter,
                range: fm.range.clone(),
                inner_range: fm.content_range.clone(),
                line_start: 0,
                depth: 0,
                semantic_id: None,
            });
        }
        for (idx, h) in facts.headings.iter().enumerate() {
            let inner_start = text[h.range.clone()]
                .find(|c: char| c != '#' && !c.is_whitespace())
                .map_or(h.range.start, |rel| h.range.start + rel);
            let inner_end = text[h.range.clone()].trim_end().len() + h.range.start;
            semantic_nodes.push(SemanticNode {
                kind: SemanticKind::Heading(h.level),
                range: h.range.clone(),
                inner_range: inner_start..inner_end.max(inner_start),
                line_start: line_of(h.range.start),
                depth: usize::from(h.level),
                semantic_id: Some(h.anchor.clone()),
            });
            if let Some(sec) = sections.get(idx) {
                semantic_nodes.push(SemanticNode {
                    kind: SemanticKind::Section(sec.level),
                    range: sec.full_range.clone(),
                    inner_range: sec.body_range.clone(),
                    line_start: sec.line_start,
                    depth: usize::from(sec.level),
                    semantic_id: Some(h.anchor.clone()),
                });
            }
        }
        for link in &facts.links {
            let inner = if link.is_wiki {
                (link.range.start + 2)..(link.range.end.saturating_sub(2))
            } else {
                link.range.clone()
            };
            semantic_nodes.push(SemanticNode {
                kind: if link.is_wiki {
                    SemanticKind::WikiLink
                } else {
                    SemanticKind::Link
                },
                range: link.range.clone(),
                inner_range: inner,
                line_start: line_of(link.range.start),
                depth: 0,
                semantic_id: Some(link.target.clone()),
            });
        }
        for block in &facts.blocks {
            semantic_nodes.push(SemanticNode {
                kind: SemanticKind::BlockId,
                range: block.range.clone(),
                inner_range: (block.range.start + 1)..block.range.end,
                line_start: line_of(block.range.start),
                depth: 0,
                semantic_id: Some(block.id.clone()),
            });
        }
        for task in &tasks {
            semantic_nodes.push(SemanticNode {
                kind: SemanticKind::TaskItem,
                range: task.line_range.clone(),
                inner_range: task.content_range.clone(),
                line_start: task.line_start,
                depth: task.depth,
                semantic_id: None,
            });
        }
        for callout in &callouts {
            semantic_nodes.push(SemanticNode {
                kind: SemanticKind::Callout,
                range: callout.range.clone(),
                inner_range: callout.content_range.clone(),
                line_start: callout.line_start,
                depth: callout.depth,
                semantic_id: Some(callout.kind.tag().to_string()),
            });
        }
        for q in &quotes {
            semantic_nodes.push(SemanticNode {
                kind: SemanticKind::BlockQuote,
                range: q.clone(),
                inner_range: q.clone(),
                line_start: line_of(q.start),
                depth: 0,
                semantic_id: None,
            });
        }
        for cb in &code_blocks {
            semantic_nodes.push(SemanticNode {
                kind: SemanticKind::CodeFence,
                range: cb.range.clone(),
                inner_range: cb.content_range.clone(),
                line_start: cb.line_start,
                depth: 0,
                semantic_id: cb.language.clone(),
            });
        }
        for table in &tables {
            semantic_nodes.push(SemanticNode {
                kind: SemanticKind::Table,
                range: table.range.clone(),
                inner_range: table.range.clone(),
                line_start: table.line_start,
                depth: 0,
                semantic_id: None,
            });
            for row in &table.rows {
                semantic_nodes.push(SemanticNode {
                    kind: SemanticKind::TableRow,
                    range: row.range.clone(),
                    inner_range: row.range.clone(),
                    line_start: line_of(row.range.start),
                    depth: 1,
                    semantic_id: None,
                });
                for cell in &row.cells {
                    let slice = &text[cell.clone()];
                    let trimmed = slice.trim();
                    let inner_start =
                        cell.start + (trimmed.as_ptr() as usize - slice.as_ptr() as usize);
                    let inner_end = inner_start + trimmed.len();
                    semantic_nodes.push(SemanticNode {
                        kind: SemanticKind::TableCell,
                        range: cell.clone(),
                        inner_range: inner_start..inner_end,
                        line_start: line_of(cell.start),
                        depth: 2,
                        semantic_id: None,
                    });
                }
            }
        }
        for span in &colors {
            semantic_nodes.push(SemanticNode {
                kind: SemanticKind::ColorSpan,
                range: span.opening.start..span.closing.end,
                inner_range: span.content.clone(),
                line_start: line_of(span.opening.start),
                depth: 0,
                semantic_id: None,
            });
        }
        semantic_nodes.sort_by_key(|n| (n.range.start, n.range.end));

        Self {
            buffer_id,
            revision,
            facts,
            sections,
            tasks,
            callouts,
            quotes,
            code_blocks,
            tables,
            frontmatter,
            color_spans: colors,
            semantic_nodes,
        }
    }

    /// Returns all headings intersecting `range`.
    pub fn headings_in(&self, range: Range<usize>) -> Vec<&HeadingFact> {
        self.facts
            .headings
            .iter()
            .filter(|h| h.range.start < range.end && h.range.end > range.start)
            .collect()
    }

    /// Returns the heading directly covering `offset`, or the nearest enclosing section's heading.
    pub fn heading_under_cursor(&self, offset: usize) -> Option<&HeadingFact> {
        if let Some(direct) = self
            .facts
            .headings
            .iter()
            .find(|h| h.range.start <= offset && offset <= h.range.end)
        {
            return Some(direct);
        }
        self.sections
            .iter()
            .filter(|s| s.full_range.start <= offset && offset <= s.full_range.end)
            .min_by_key(|s| s.full_range.len())
            .and_then(|s| self.facts.headings.get(s.heading_index))
    }

    /// Returns the full byte range of the innermost section covering `offset`.
    pub fn section_range(&self, offset: usize) -> Option<Range<usize>> {
        self.sections
            .iter()
            .filter(|s| s.full_range.start <= offset && offset <= s.full_range.end)
            .min_by_key(|s| s.full_range.len())
            .map(|s| s.full_range.clone())
    }

    /// Returns the link or wikilink covering `offset`.
    pub fn linked_target_at(&self, offset: usize) -> Option<&LinkOccurrence> {
        self.facts
            .links
            .iter()
            .find(|l| l.range.start <= offset && offset < l.range.end)
    }

    /// Returns all task items intersecting `range`.
    pub fn tasks_in(&self, range: Range<usize>) -> Vec<&TaskFact> {
        self.tasks
            .iter()
            .filter(|t| t.line_range.start < range.end && t.line_range.end > range.start)
            .collect()
    }

    /// Returns all semantic objects intersecting `range`.
    pub fn semantic_objects_in(&self, range: Range<usize>) -> Vec<&SemanticNode> {
        self.semantic_nodes
            .iter()
            .filter(|n| n.range.start < range.end && n.range.end > range.start)
            .collect()
    }

    /// Converts extracted Markdown objects into `SyntaxObjectRange` items for `VimState::set_syntax_objects`.
    pub fn syntax_object_ranges(&self) -> Vec<SyntaxObjectRange> {
        let mut out = Vec::new();
        for node in &self.semantic_nodes {
            let target = match node.kind {
                SemanticKind::Heading(_) => Some(TextObjectTarget::MarkdownHeading),
                SemanticKind::Section(_) => Some(TextObjectTarget::MarkdownSection),
                SemanticKind::Link | SemanticKind::WikiLink => Some(TextObjectTarget::MarkdownLink),
                SemanticKind::CodeFence => Some(TextObjectTarget::MarkdownCodeBlock),
                SemanticKind::TaskItem => Some(TextObjectTarget::MarkdownListItem),
                SemanticKind::Callout | SemanticKind::BlockQuote => {
                    Some(TextObjectTarget::MarkdownQuote)
                }
                SemanticKind::TableCell => Some(TextObjectTarget::MarkdownTableCell),
                SemanticKind::TableRow => Some(TextObjectTarget::MarkdownTableRow),
                _ => None,
            };
            if let Some(target) = target {
                out.push(SyntaxObjectRange {
                    target,
                    inner: node.inner_range.clone(),
                    around: node.range.clone(),
                });
            }
        }
        out
    }
}

fn compute_line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

fn extract_frontmatter(text: &str) -> Option<FrontmatterFact> {
    if !text.starts_with("---") {
        return None;
    }
    let after_first_line = if let Some(idx) = text.find('\n') {
        idx + 1
    } else {
        return None;
    };
    let rest = &text[after_first_line..];
    let mut close_offset = None;
    let mut cursor = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            close_offset = Some(after_first_line + cursor);
            break;
        }
        cursor += line.len();
    }
    let close_start = close_offset?;
    let close_line_len = if let Some(idx) = text[close_start..].find('\n') {
        idx + 1
    } else {
        text[close_start..].len()
    };
    let content_range = after_first_line..close_start;
    let range = 0..(close_start + close_line_len);

    let mut properties = Vec::new();
    for line in text[content_range.clone()].lines() {
        let trimmed = line.trim();
        if let Some((k, v)) = trimmed.split_once(':') {
            properties.push((k.trim().to_string(), v.trim().to_string()));
        }
    }

    Some(FrontmatterFact {
        range,
        content_range,
        properties,
    })
}

fn extract_cmark_blocks(
    text: &str,
    line_of: &impl Fn(usize) -> usize,
) -> (Vec<CodeBlockFact>, Vec<TableFact>, Vec<Range<usize>>) {
    let mut code_blocks = Vec::new();
    let mut tables = Vec::new();
    let mut quotes = Vec::new();

    let mut options = Options::all();
    options.remove(Options::ENABLE_WIKILINKS);
    let parser = Parser::new_ext(text, options).into_offset_iter();

    let mut current_code_block: Option<(Option<String>, usize)> = None;
    let mut current_quote: Option<usize> = None;
    let mut current_table: Option<(usize, Vec<TableRowFact>)> = None;
    let mut current_row: Option<(usize, bool, Vec<Range<usize>>)> = None;

    for (event, range) in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(l) => {
                        let trimmed = l.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_string())
                        }
                    }
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
                current_code_block = Some((lang, range.start));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((lang, start)) = current_code_block.take() {
                    let block_range = start..range.end;
                    let block_text = &text[block_range.clone()];
                    let content_range =
                        if block_text.starts_with("```") || block_text.starts_with("~~~") {
                            let first_nl = block_text.find('\n').map_or(0, |i| i + 1);
                            let last_fence = block_text
                                .rfind("```")
                                .or_else(|| block_text.rfind("~~~"))
                                .unwrap_or(block_text.len());
                            (start + first_nl)..(start + last_fence)
                        } else {
                            block_range.clone()
                        };
                    code_blocks.push(CodeBlockFact {
                        language: lang,
                        range: block_range,
                        content_range,
                        line_start: line_of(start),
                    });
                }
            }
            Event::Start(Tag::BlockQuote(_)) if current_quote.is_none() => {
                current_quote = Some(range.start);
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                if let Some(start) = current_quote.take() {
                    quotes.push(start..range.end);
                }
            }
            Event::Start(Tag::Table(_)) => {
                current_table = Some((range.start, Vec::new()));
            }
            Event::End(TagEnd::Table) => {
                if let Some((start, rows)) = current_table.take() {
                    tables.push(TableFact {
                        range: start..range.end,
                        rows,
                        line_start: line_of(start),
                    });
                }
            }
            Event::Start(Tag::TableHead) => {
                current_row = Some((range.start, true, Vec::new()));
            }
            Event::End(TagEnd::TableHead) => {
                if let Some((start, is_header, cells)) = current_row.take()
                    && let Some((_, ref mut rows)) = current_table
                {
                    rows.push(TableRowFact {
                        range: start..range.end,
                        is_header,
                        cells,
                    });
                }
            }
            Event::Start(Tag::TableRow) => {
                current_row = Some((range.start, false, Vec::new()));
            }
            Event::End(TagEnd::TableRow) => {
                if let Some((start, is_header, cells)) = current_row.take()
                    && let Some((_, ref mut rows)) = current_table
                {
                    rows.push(TableRowFact {
                        range: start..range.end,
                        is_header,
                        cells,
                    });
                }
            }
            Event::End(TagEnd::TableCell) => {
                if let Some((_, _, ref mut cells)) = current_row {
                    cells.push(range);
                }
            }
            _ => {}
        }
    }

    (code_blocks, tables, quotes)
}

fn build_sections(
    text: &str,
    headings: &[HeadingFact],
    line_of: &impl Fn(usize) -> usize,
) -> Vec<SectionFact> {
    let mut sections = Vec::with_capacity(headings.len());
    for (i, h) in headings.iter().enumerate() {
        let next_pos = headings[(i + 1)..]
            .iter()
            .find(|next| next.level <= h.level)
            .map_or(text.len(), |next| next.range.start);

        let full_range = h.range.start..next_pos;
        let body_start = h.range.end.min(next_pos);
        let body_range = body_start..next_pos;

        let parent_section = (0..i).rev().find(|&j| headings[j].level < h.level);

        sections.push(SectionFact {
            heading_index: i,
            level: h.level,
            parent_section,
            heading_range: h.range.clone(),
            body_range,
            full_range,
            line_start: line_of(h.range.start),
        });
    }
    sections
}

fn extract_lines_tasks_and_callouts(
    text: &str,
    code_blocks: &[CodeBlockFact],
) -> (Vec<TaskFact>, Vec<CalloutFact>) {
    let mut tasks = Vec::new();
    let mut callouts = Vec::new();
    let mut line_start = 0;

    for (line_idx, line) in text.split_inclusive('\n').enumerate() {
        let line_len = line.len();
        let line_end = line_start + line_len;
        let trimmed_line_end = line_start + line.trim_end_matches(['\r', '\n']).len();

        let in_code = code_blocks
            .iter()
            .any(|cb| cb.content_range.start <= line_start && line_start < cb.content_range.end);

        if !in_code {
            let line_bytes = line.as_bytes();

            // Check Callout: `> [!note] Title`
            let mut scan = 0;
            let mut depth = 0;
            while scan < line_bytes.len() {
                while scan < line_bytes.len()
                    && (line_bytes[scan] == b' ' || line_bytes[scan] == b'\t')
                {
                    scan += 1;
                }
                if scan < line_bytes.len() && line_bytes[scan] == b'>' {
                    depth += 1;
                    scan += 1;
                    if scan < line_bytes.len() && line_bytes[scan] == b' ' {
                        scan += 1;
                    }
                } else {
                    break;
                }
            }

            let after_quote = &line[scan..];
            if after_quote.starts_with("[!")
                && let Some(close_bracket) = after_quote.find(']')
            {
                let tag = &after_quote[2..close_bracket];
                let kind = CalloutKind::from_tag(tag);
                let title_part = after_quote[close_bracket + 1..].trim();
                let title = if title_part.is_empty() {
                    None
                } else {
                    Some(title_part.to_string())
                };
                let content_start = line_start + scan + close_bracket + 1;
                callouts.push(CalloutFact {
                    kind,
                    title,
                    range: line_start..trimmed_line_end,
                    content_range: content_start..trimmed_line_end,
                    line_start: line_idx,
                    depth,
                });
            }

            // Check Task item: `[indent]- [ ] ` or `[indent]* [x] ` etc.
            let mut ws_len = 0;
            while ws_len < line_bytes.len()
                && (line_bytes[ws_len] == b' ' || line_bytes[ws_len] == b'\t')
            {
                ws_len += 1;
            }
            let task_depth = ws_len / 2;
            let after_indent = &line[ws_len..];
            if let Some(first) = after_indent.chars().next()
                && matches!(first, '-' | '*' | '+')
                && after_indent.as_bytes().get(1) == Some(&b' ')
            {
                let after_bullet = &after_indent[2..];
                if after_bullet.len() >= 4
                    && after_bullet.as_bytes()[0] == b'['
                    && matches!(after_bullet.as_bytes()[1], b' ' | b'x' | b'X')
                    && after_bullet.as_bytes()[2] == b']'
                    && after_bullet.as_bytes()[3] == b' '
                {
                    let checked = after_bullet.as_bytes()[1] != b' ';
                    let checkbox_offset = line_start + ws_len + 2 + 1;
                    let marker_range = (line_start + ws_len)..(line_start + ws_len + 6);
                    let content_start = line_start + ws_len + 6;
                    tasks.push(TaskFact {
                        checked,
                        checkbox_offset,
                        marker_range,
                        content_range: content_start..trimmed_line_end,
                        line_range: line_start..line_end,
                        line_start: line_idx,
                        depth: task_depth,
                    });
                }
            }
        }

        line_start = line_end;
    }

    (tasks, callouts)
}
