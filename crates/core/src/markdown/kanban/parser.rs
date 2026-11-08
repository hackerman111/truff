use std::ops::Range;

/// Status of a kanban card/task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KanbanTaskStatus {
    Todo,
    InProgress,
    Done,
}

impl KanbanTaskStatus {
    /// Returns the character marker representing this status inside `[ ]`.
    pub fn marker_char(&self) -> char {
        match self {
            Self::Todo => ' ',
            Self::InProgress => '/',
            Self::Done => 'x',
        }
    }

    /// Cycles status: `Todo` -> `InProgress` -> `Done` -> `Todo`.
    pub fn cycle(&self) -> Self {
        match self {
            Self::Todo => Self::InProgress,
            Self::InProgress => Self::Done,
            Self::Done => Self::Todo,
        }
    }

    /// Toggles done: `Done` -> `Todo`, and `Todo`/`InProgress` -> `Done`.
    pub fn toggle_done(&self) -> Self {
        match self {
            Self::Done => Self::Todo,
            Self::Todo | Self::InProgress => Self::Done,
        }
    }

    /// Parses a task status from the marker character inside `[ ]`.
    pub fn from_marker_char(c: char) -> Self {
        match c {
            '/' => Self::InProgress,
            'x' | 'X' => Self::Done,
            _ => Self::Todo,
        }
    }
}

/// A wiki-link extracted from a Kanban card title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanbanCardLink {
    /// Target note name or path (without surrounding `[[` and `]]`).
    pub target: String,
    /// Optional heading anchor (e.g. `heading` from `[[note#heading]]`).
    pub heading: Option<String>,
    /// Optional display alias (e.g. `alias` from `[[note|alias]]`).
    pub alias: Option<String>,
    /// Exact byte range of the `[[...]]` token in the card's line.
    pub raw_range_in_line: Range<usize>,
}

impl KanbanCardLink {
    /// Returns the display string: the alias if present, otherwise the file stem of `target`.
    pub fn display(&self) -> &str {
        if let Some(alias) = &self.alias {
            alias.as_str()
        } else {
            let base = self.target.rsplit('/').next().unwrap_or(&self.target);
            let base = base.rsplit('\\').next().unwrap_or(base);
            base.strip_suffix(".md").unwrap_or(base)
        }
    }

    /// Returns the navigation target (e.g. `target#heading` or `target`).
    pub fn navigation_target(&self) -> String {
        match &self.heading {
            Some(h) if self.target.is_empty() => format!("#{h}"),
            Some(h) => format!("{}#{h}", self.target),
            None => self.target.clone(),
        }
    }
}

/// A card representing a task item under a column in a Kanban board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanbanCard {
    /// Card status (Todo, InProgress, Done).
    pub status: KanbanTaskStatus,
    /// Raw text on the card line after `- [ ] `.
    pub raw_title: String,
    /// Clean title with `[[...]]` links and `#tags` stripped/collapsed.
    pub clean_title: String,
    /// Continuation body lines indented under the card.
    pub body_lines: Vec<String>,
    /// Wiki-links extracted from the card title.
    pub links: Vec<KanbanCardLink>,
    /// Hashtags extracted from the card title (without leading `#`).
    pub tags: Vec<String>,
    /// Full byte range from start of card line through trailing newline of its last line.
    pub full_range: Range<usize>,
    /// Exact byte offset of the status character inside `[ ]`.
    pub status_char_byte: usize,
    /// Exact byte range of `raw_title` on the card's first line.
    pub title_range: Range<usize>,
    /// 0-based line number of the card's first line.
    pub line_index: usize,
}

/// A column (level 2 heading `## ...`) in a Kanban board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanbanColumn {
    /// Column title text.
    pub title: String,
    /// Full byte range of the heading line including trailing newline.
    pub heading_range: Range<usize>,
    /// Byte range of the title text within the heading line.
    pub title_range: Range<usize>,
    /// Full section byte range up to the next `##` / `#` heading or EOF.
    pub section_range: Range<usize>,
    /// Offset where a new card should be inserted (after existing cards or heading line).
    pub insert_card_offset: usize,
    /// Cards in this column.
    pub cards: Vec<KanbanCard>,
}

/// Parsed representation of a Markdown Kanban board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanbanBoard {
    /// Optional board title from the first `# <Title>` heading.
    pub title: Option<String>,
    /// Board columns.
    pub columns: Vec<KanbanColumn>,
    /// Full byte range of the board document (`0..text.len()`).
    pub full_range: Range<usize>,
}

impl KanbanBoard {
    /// Parses a Markdown document into a `KanbanBoard`.
    pub fn parse(text: &str) -> Self {
        let lines = split_lines(text);
        let mut board_title = None;
        let mut columns = Vec::new();

        let mut in_code_fence = false;
        let mut fence_char = ' ';
        let mut fence_len = 0;

        let mut current_column: Option<KanbanColumn> = None;
        let mut current_col_start = 0;
        let mut current_card: Option<KanbanCard> = None;

        for line in &lines {
            let trimmed = line.content.trim_start();

            // Track fenced code blocks (``` or ~~~) so code content is ignored
            if !in_code_fence {
                if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                    let c = trimmed.chars().next().unwrap();
                    let count = trimmed.chars().take_while(|ch| *ch == c).count();
                    if count >= 3 {
                        in_code_fence = true;
                        fence_char = c;
                        fence_len = count;
                        continue;
                    }
                }
            } else {
                if trimmed.starts_with(fence_char) {
                    let count = trimmed.chars().take_while(|ch| *ch == fence_char).count();
                    if count >= fence_len && trimmed[count..].trim().is_empty() {
                        in_code_fence = false;
                    }
                }
                continue;
            }

            // Check for level 1 heading `# `
            if let Some(stripped) = trimmed.strip_prefix("# ")
                && !stripped.starts_with('#')
            {
                // If a card is active, finalize it
                if let Some(card) = current_card.take()
                    && let Some(col) = &mut current_column
                {
                    col.cards.push(card);
                }
                // If a column is active, close its section
                if let Some(mut col) = current_column.take() {
                    col.section_range = current_col_start..line.full_range.start;
                    col.insert_card_offset = col
                        .cards
                        .last()
                        .map(|c| c.full_range.end)
                        .unwrap_or(col.heading_range.end);
                    columns.push(col);
                }

                if board_title.is_none() {
                    board_title = Some(stripped.trim().to_string());
                }
                continue;
            }

            // Check for level 2 heading `## `
            if let Some(stripped) = trimmed.strip_prefix("## ") {
                // Finalize active card
                if let Some(card) = current_card.take()
                    && let Some(col) = &mut current_column
                {
                    col.cards.push(card);
                }
                // Finalize active column
                if let Some(mut col) = current_column.take() {
                    col.section_range = current_col_start..line.full_range.start;
                    col.insert_card_offset = col
                        .cards
                        .last()
                        .map(|c| c.full_range.end)
                        .unwrap_or(col.heading_range.end);
                    columns.push(col);
                }

                current_col_start = line.full_range.start;
                let title = stripped.trim().to_string();
                let hash_pos = line.content.find("##").unwrap_or(0);
                let after_hashes = hash_pos + 2;
                let title_start_in_line = after_hashes
                    + (line.content[after_hashes..].len()
                        - line.content[after_hashes..].trim_start().len());
                let title_start = line.content_range.start + title_start_in_line;
                let title_end = title_start + title.len();

                current_column = Some(KanbanColumn {
                    title,
                    heading_range: line.full_range.clone(),
                    title_range: title_start..title_end,
                    section_range: 0..0,
                    insert_card_offset: 0,
                    cards: Vec::new(),
                });
                continue;
            }

            // If we are inside a column, process card lines and continuation lines
            if let Some(col) = &mut current_column {
                if let Some(new_card) = parse_card_line(line) {
                    if let Some(card) = current_card.take() {
                        col.cards.push(card);
                    }
                    current_card = Some(new_card);
                    continue;
                }

                // Check for indented continuation line of current card
                if let Some(card) = &mut current_card {
                    let is_indented =
                        line.content.starts_with(' ') || line.content.starts_with('\t');
                    if is_indented && !line.content.trim().is_empty() {
                        card.body_lines.push(line.content.trim().to_string());
                        card.full_range.end = line.full_range.end;
                        continue;
                    }
                }

                // Any other line ends the current card
                if let Some(card) = current_card.take() {
                    col.cards.push(card);
                }
            }
        }

        // Finalize last card and column
        if let Some(card) = current_card.take()
            && let Some(col) = &mut current_column
        {
            col.cards.push(card);
        }
        if let Some(mut col) = current_column.take() {
            col.section_range = current_col_start..text.len();
            col.insert_card_offset = col
                .cards
                .last()
                .map(|c| c.full_range.end)
                .unwrap_or(col.heading_range.end);
            columns.push(col);
        }

        KanbanBoard {
            title: board_title,
            columns,
            full_range: 0..text.len(),
        }
    }

    /// Returns the total number of cards across all columns.
    pub fn total_card_count(&self) -> usize {
        self.columns.iter().map(|col| col.cards.len()).sum()
    }

    /// Returns the number of completed cards across all columns.
    pub fn done_card_count(&self) -> usize {
        self.columns
            .iter()
            .flat_map(|col| &col.cards)
            .filter(|card| card.status == KanbanTaskStatus::Done)
            .count()
    }
}

/// Checks whether a document represents a Kanban board based on its file path or content.
pub fn is_kanban_document(path: Option<&str>, text: &str) -> bool {
    if let Some(p) = path {
        let file_name = p.rsplit(['/', '\\']).next().unwrap_or(p);
        if file_name == "kanban.md" || file_name.ends_with(".kanban.md") {
            return true;
        }
    }
    text.contains("<!-- truff:kanban -->")
}

#[derive(Debug, Clone)]
struct LineSpan<'a> {
    line_index: usize,
    content_range: Range<usize>,
    full_range: Range<usize>,
    content: &'a str,
}

fn split_lines(text: &str) -> Vec<LineSpan<'_>> {
    let mut spans = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut line_index = 0;

    while start < len {
        let mut end = start;
        while end < len && bytes[end] != b'\n' {
            end += 1;
        }
        let next_start = if end < len { end + 1 } else { end };
        let mut content_end = end;
        if content_end > start && bytes[content_end - 1] == b'\r' {
            content_end -= 1;
        }
        spans.push(LineSpan {
            line_index,
            content_range: start..content_end,
            full_range: start..next_start,
            content: &text[start..content_end],
        });
        line_index += 1;
        start = next_start;
    }
    spans
}

fn parse_card_line(line: &LineSpan<'_>) -> Option<KanbanCard> {
    let trimmed_start = line.content.len() - line.content.trim_start().len();
    // Cards in kanban columns should not have deep indentation (which indicates continuation lines)
    if trimmed_start > 1 {
        return None;
    }

    let rem = &line.content[trimmed_start..];
    let bytes = rem.as_bytes();
    if bytes.len() >= 5
        && matches!(bytes[0], b'-' | b'*' | b'+')
        && bytes[1] == b' '
        && bytes[2] == b'['
        && bytes[4] == b']'
        && (bytes.len() == 5 || bytes[5] == b' ' || bytes[5] == b'\t')
    {
        let status = KanbanTaskStatus::from_marker_char(bytes[3] as char);
        let status_char_byte = line.content_range.start + trimmed_start + 3;

        let after_marker = if bytes.len() >= 6 {
            trimmed_start + 6
        } else {
            trimmed_start + 5
        };
        let raw_title = if after_marker <= line.content.len() {
            &line.content[after_marker..]
        } else {
            ""
        };
        let title_start = line.content_range.start + after_marker;
        let title_range = title_start..line.content_range.end;

        let (links, tags, clean_title) = extract_links_and_tags(raw_title, after_marker);

        Some(KanbanCard {
            status,
            raw_title: raw_title.to_string(),
            clean_title,
            body_lines: Vec::new(),
            links,
            tags,
            full_range: line.full_range.clone(),
            status_char_byte,
            title_range,
            line_index: line.line_index,
        })
    } else {
        None
    }
}

fn extract_links_and_tags(
    raw_title: &str,
    title_start_in_line: usize,
) -> (Vec<KanbanCardLink>, Vec<String>, String) {
    let mut links = Vec::new();
    let mut link_ranges = Vec::new();
    let mut cur = 0;

    while let Some(open) = raw_title[cur..].find("[[") {
        let open_idx = cur + open;
        if let Some(close) = raw_title[open_idx + 2..].find("]]") {
            let close_idx = open_idx + 2 + close + 2;
            let inner = &raw_title[open_idx + 2..close_idx - 2];
            let raw_range_in_line =
                (title_start_in_line + open_idx)..(title_start_in_line + close_idx);
            let link = parse_wiki_inner(inner, raw_range_in_line);
            links.push(link);
            link_ranges.push(open_idx..close_idx);
            cur = close_idx;
        } else {
            break;
        }
    }

    let mut tags = Vec::new();
    let mut tag_ranges = Vec::new();
    let bytes = raw_title.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'#' {
            let inside_link = link_ranges.iter().any(|r| r.contains(&i));
            let preceded_by_boundary = i == 0 || bytes[i - 1].is_ascii_whitespace();

            if !inside_link && preceded_by_boundary {
                let tag_start = i + 1;
                let mut tag_end = tag_start;
                while tag_end < raw_title.len() {
                    let ch = raw_title[tag_end..].chars().next().unwrap();
                    if ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '/' {
                        tag_end += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                if tag_end > tag_start {
                    let tag_candidate = &raw_title[tag_start..tag_end];
                    if tag_candidate.chars().any(|c| c.is_alphanumeric()) {
                        tags.push(tag_candidate.to_string());
                        tag_ranges.push(i..tag_end);
                        i = tag_end;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    // Build clean_title by stripping wiki-links and tags
    let mut all_ranges = link_ranges;
    all_ranges.extend(tag_ranges);
    all_ranges.sort_by_key(|r| r.start);

    let mut cleaned = String::with_capacity(raw_title.len());
    let mut last_end = 0;
    for r in all_ranges {
        if r.start > last_end {
            cleaned.push_str(&raw_title[last_end..r.start]);
        }
        cleaned.push(' ');
        last_end = last_end.max(r.end);
    }
    if last_end < raw_title.len() {
        cleaned.push_str(&raw_title[last_end..]);
    }

    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean_title = if collapsed.is_empty() && !links.is_empty() {
        links
            .iter()
            .map(|l| l.display())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        collapsed
    };

    (links, tags, clean_title)
}

fn parse_wiki_inner(inner: &str, raw_range_in_line: Range<usize>) -> KanbanCardLink {
    let (target_part, alias) = match inner.find('|') {
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

    let (target, heading) = match target_part.find('#') {
        Some(idx) => (
            target_part[..idx].trim().to_string(),
            Some(target_part[idx + 1..].trim().to_string()),
        ),
        None => (target_part.trim().to_string(), None),
    };

    KanbanCardLink {
        target,
        heading,
        alias,
        raw_range_in_line,
    }
}
