use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use truff_core::markdown::presentation::{LinePresentation, ListKind, ListMarker};
use truff_core::markdown::style::ColorSpan;

use super::DisplayMap;
use super::types::{InlineStyle, StyledSpan};

pub fn highlight_spans(text: &str) -> Vec<ColorSpan> {
    let prefix = "<mark style=\"background-color:#";
    let Some(first) = text.find(prefix) else {
        return Vec::new();
    };
    let mut spans = Vec::new();
    let mut offset = first;
    while let Some(relative) = text[offset..].find(prefix) {
        let start = offset + relative;
        let hex_start = start + prefix.len();
        let hex_end = hex_start + 6;
        offset = hex_start;
        let Some(hex) = text.get(hex_start..hex_end) else {
            continue;
        };
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) || !text[hex_end..].starts_with("\">") {
            continue;
        }
        let content_start = hex_end + 2;
        let Some(close_relative) = text[content_start..].find("</mark>") else {
            break;
        };
        let close_start = content_start + close_relative;
        let end = close_start + "</mark>".len();
        offset = end;
        let channel = |index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap_or(0);
        spans.push(ColorSpan {
            opening: start..content_start,
            content: content_start..close_start,
            closing: close_start..end,
            rgba: [channel(0), channel(2), channel(4), 255],
        });
    }
    spans
}

#[inline]
fn has_markdown_syntax(text: &str) -> bool {
    text.bytes().any(|b| {
        matches!(
            b,
            b'*' | b'_'
                | b'~'
                | b'`'
                | b'['
                | b'#'
                | b'-'
                | b'+'
                | b'='
                | b'$'
                | b'<'
                | b'|'
                | b'.'
                | b')'
        )
    })
}

impl DisplayMap {
    pub(super) fn scan_markers(&mut self) {
        self.markers.clear();
        self.lists.clear();
        self.styles.clear();
        if self.presentation.as_ref().is_some_and(|line| line.code) {
            self.colors.clear();
            return;
        }
        let text = &self.source_text;
        let has_list_presentation = self
            .presentation
            .as_ref()
            .is_some_and(|line| line.list.is_some());
        if !has_list_presentation && self.colors.is_empty() && !has_markdown_syntax(text) {
            return;
        }
        let indent = if has_list_presentation {
            text.len() - text.trim_start_matches([' ', '\t']).len()
        } else {
            0
        };
        let mut text_ranges: Vec<Range<usize>> = Vec::new();
        let mut depth = 0usize;
        let mut current_item = None;
        for (event, range) in Parser::new_ext(
            &text[indent..],
            Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
        )
        .into_offset_iter()
        {
            let range = range.start + indent..range.end + indent;
            match event {
                Event::Start(Tag::List(_)) => depth += 1,
                Event::End(TagEnd::List(_)) => depth = depth.saturating_sub(1),
                Event::Start(Tag::Item) => {
                    let tail = &text[range.start..range.end];
                    let prefix_len = tail
                        .bytes()
                        .take_while(|byte| !byte.is_ascii_whitespace())
                        .count();
                    let end = range.start + prefix_len;
                    let kind = if tail.starts_with(['-', '*', '+']) {
                        ListKind::Bullet
                    } else {
                        ListKind::Ordered
                    };
                    self.lists.push(ListMarker {
                        range: range.start..end,
                        depth: depth.saturating_sub(1),
                        kind,
                    });
                    current_item = Some(self.lists.len() - 1);
                }
                Event::TaskListMarker(checked) => {
                    if let Some(index) = current_item {
                        self.lists[index].range.end = range.end;
                        self.lists[index].kind = ListKind::Task {
                            checked,
                            checkbox: range.start + 1,
                        };
                    }
                }
                Event::Start(Tag::Strong) => {
                    let source = &text[range.clone()];
                    let marker_len = if source.starts_with("**") || source.starts_with("__") {
                        2
                    } else {
                        1
                    };
                    self.markers
                        .push((range.start..range.start + marker_len, Some(range.clone())));
                    self.markers
                        .push((range.end - marker_len..range.end, Some(range.clone())));
                    if range.len() >= marker_len * 2 {
                        self.styles.push(StyledSpan {
                            content: (range.start + marker_len)..(range.end - marker_len),
                            style: InlineStyle::Bold,
                        });
                    }
                }
                Event::Start(Tag::Emphasis) => {
                    let source = &text[range.clone()];
                    let marker_len = if source.starts_with("**") || source.starts_with("__") {
                        2
                    } else {
                        1
                    };
                    self.markers
                        .push((range.start..range.start + marker_len, Some(range.clone())));
                    self.markers
                        .push((range.end - marker_len..range.end, Some(range.clone())));
                    if range.len() >= marker_len * 2 {
                        self.styles.push(StyledSpan {
                            content: (range.start + marker_len)..(range.end - marker_len),
                            style: InlineStyle::Italic,
                        });
                    }
                }
                Event::Start(Tag::Strikethrough) => {
                    let marker_len = 2;
                    self.markers
                        .push((range.start..range.start + marker_len, Some(range.clone())));
                    self.markers
                        .push((range.end - marker_len..range.end, Some(range.clone())));
                    if range.len() >= marker_len * 2 {
                        self.styles.push(StyledSpan {
                            content: (range.start + marker_len)..(range.end - marker_len),
                            style: InlineStyle::Strikethrough,
                        });
                    }
                }
                Event::Code(_) => {
                    let count = text[range.clone()]
                        .bytes()
                        .take_while(|byte| *byte == b'`')
                        .count();
                    self.markers
                        .push((range.start..range.start + count, Some(range.clone())));
                    self.markers
                        .push((range.end - count..range.end, Some(range.clone())));
                    if range.len() >= count * 2 {
                        self.styles.push(StyledSpan {
                            content: (range.start + count)..(range.end - count),
                            style: InlineStyle::Code,
                        });
                    }
                }
                Event::Start(Tag::Link { .. }) => {
                    let source = &text[range.clone()];
                    if source.starts_with('[') {
                        if let Some(close_bracket) = source.find("](") {
                            let open_marker = range.start..range.start + 1;
                            let close_marker = (range.start + close_bracket)..range.end;
                            let content_start = range.start + 1;
                            let content_end = range.start + close_bracket;
                            self.markers.push((open_marker, Some(range.clone())));
                            self.markers.push((close_marker, Some(range.clone())));
                            if content_start < content_end {
                                self.styles.push(StyledSpan {
                                    content: content_start..content_end,
                                    style: InlineStyle::Link,
                                });
                            }
                        } else if let Some(close_bracket) = source.find("][") {
                            let open_marker = range.start..range.start + 1;
                            let close_marker = (range.start + close_bracket)..range.end;
                            let content_start = range.start + 1;
                            let content_end = range.start + close_bracket;
                            self.markers.push((open_marker, Some(range.clone())));
                            self.markers.push((close_marker, Some(range.clone())));
                            if content_start < content_end {
                                self.styles.push(StyledSpan {
                                    content: content_start..content_end,
                                    style: InlineStyle::Link,
                                });
                            }
                        }
                    }
                }
                Event::Start(Tag::Heading { .. }) => {
                    let prefix = text[range.clone()]
                        .bytes()
                        .take_while(|byte| *byte == b'#')
                        .count();
                    if prefix > 0 {
                        let end = range.start + prefix;
                        let end = end + usize::from(text.as_bytes().get(end) == Some(&b' '));
                        self.markers.push((range.start..end, Some(range)));
                    }
                }
                Event::Text(_) => {
                    if let Some(previous) = text_ranges
                        .last_mut()
                        .filter(|previous| previous.end == range.start)
                    {
                        previous.end = range.end;
                    } else {
                        text_ranges.push(range);
                    }
                }
                Event::Rule => {
                    self.markers.push((range.clone(), Some(range)));
                }
                _ => {}
            }
        }
        for range in text_ranges {
            let mut start = range.start;
            while let Some(open) = text[start..range.end].find("[[") {
                let open = start + open;
                let Some(close) = text[open + 2..range.end].find("]]") else {
                    break;
                };
                let close = open + 2 + close;
                let span = open..close + 2;
                self.markers.push((open..open + 2, Some(span.clone())));
                self.markers.push((close..close + 2, Some(span)));
                self.styles.push(StyledSpan {
                    content: (open + 2)..close,
                    style: InlineStyle::Link,
                });
                start = close + 2;
            }
            for tag in
                truff_core::markdown::facts::extract_inline_tags(&text[range.clone()], &[], &[])
            {
                self.styles.push(StyledSpan {
                    content: (range.start + tag.range.start)..(range.start + tag.range.end),
                    style: InlineStyle::Tag(tag.name),
                });
            }
        }
        // Obsidian ==highlight== syntax
        let mut h_start = 0;
        while let Some(open) = text[h_start..].find("==") {
            let open = h_start + open;
            if let Some(close) = text[open + 2..].find("==") {
                let close = open + 2 + close;
                let span = open..close + 2;
                self.markers.push((open..open + 2, Some(span.clone())));
                self.markers.push((close..close + 2, Some(span)));
                self.styles.push(StyledSpan {
                    content: (open + 2)..close,
                    style: InlineStyle::Highlight([235, 203, 139, 255]),
                });
                h_start = close + 2;
            } else {
                break;
            }
        }
        // LaTeX block math $$ ... $$
        let mut m_start = 0;
        while let Some(open) = text[m_start..].find("$$") {
            let open = m_start + open;
            if let Some(close) = text[open + 2..].find("$$") {
                let close = open + 2 + close;
                let span = open..close + 2;
                self.markers.push((span.clone(), Some(span.clone())));
                self.styles.push(StyledSpan {
                    content: span.clone(),
                    style: InlineStyle::Math,
                });
                m_start = close + 2;
            } else {
                break;
            }
        }
        // LaTeX inline math $ ... $
        let mut im_start = 0;
        while let Some(open) = text[im_start..].find('$') {
            let open = im_start + open;
            if text[open..].starts_with("$$") {
                im_start = open + 2;
                continue;
            }
            if let Some(close) = text[open + 1..].find('$') {
                let close = open + 1 + close;
                if !text[close..].starts_with("$$") && close > open + 1 {
                    let span = open..close + 1;
                    self.markers.push((span.clone(), Some(span.clone())));
                    self.styles.push(StyledSpan {
                        content: span.clone(),
                        style: InlineStyle::Math,
                    });
                    im_start = close + 1;
                    continue;
                }
            }
            im_start = open + 1;
        }
        // Table row styling
        let trimmed_text = text.trim();
        if trimmed_text.starts_with('|') && trimmed_text.ends_with('|') && trimmed_text.len() > 1 {
            let is_delimiter = trimmed_text
                .chars()
                .all(|c| c == '|' || c == '-' || c == ':' || c == ' ');
            if is_delimiter {
                self.styles.push(StyledSpan {
                    content: 0..text.len(),
                    style: InlineStyle::TextColor([76, 86, 106, 200]),
                });
            } else {
                let is_header = self.presentation.as_ref().is_some_and(|p| p.table_header);
                let mut cell_start = None;
                for (idx, b) in text.bytes().enumerate() {
                    if b == b'|' && (idx == 0 || text.as_bytes().get(idx - 1) != Some(&b'\\')) {
                        self.styles.push(StyledSpan {
                            content: idx..idx + 1,
                            style: InlineStyle::TextColor([94, 129, 172, 220]),
                        });
                        if is_header {
                            if let Some(start) = cell_start
                                && start < idx
                            {
                                self.styles.push(StyledSpan {
                                    content: start..idx,
                                    style: InlineStyle::Bold,
                                });
                            }
                            cell_start = Some(idx + 1);
                        }
                    }
                }
            }
        }
        if let Some(LinePresentation { list, .. }) = &self.presentation {
            self.lists.clear();
            self.lists.extend(list.iter().cloned());
        }
        for list in &mut self.lists {
            // Normalize marker spacing while keeping the source indentation intact.
            while matches!(text.as_bytes().get(list.range.end), Some(b' ' | b'\t')) {
                list.range.end += 1;
            }
            let end = text[list.range.end..]
                .find('\n')
                .map_or(text.len(), |len| list.range.end + len);
            self.markers
                .push((list.range.clone(), Some(list.range.start..end)));
        }
        for span in &self.colors {
            self.markers.retain(|(range, _)| {
                !ranges_touch(range, &span.opening) && !ranges_touch(range, &span.closing)
            });
            let enclosing = Some(span.opening.start..span.closing.end);
            self.markers.push((span.opening.clone(), enclosing.clone()));
            self.markers.push((span.closing.clone(), enclosing));
            self.styles.push(StyledSpan {
                content: span.content.clone(),
                style: InlineStyle::TextColor(span.rgba),
            });
        }
        for span in highlight_spans(text) {
            self.markers.retain(|(range, _)| {
                !ranges_touch(range, &span.opening) && !ranges_touch(range, &span.closing)
            });
            let enclosing = Some(span.opening.start..span.closing.end);
            self.markers.push((span.opening.clone(), enclosing.clone()));
            self.markers.push((span.closing.clone(), enclosing));
            self.styles.push(StyledSpan {
                content: span.content.clone(),
                style: InlineStyle::Highlight(span.rgba),
            });
        }
        self.markers.sort_by_key(|(range, _)| range.start);
    }

    pub(super) fn replacement(&self, range: &Range<usize>) -> Option<std::borrow::Cow<'_, str>> {
        use std::borrow::Cow;
        if let Some(slice) = self.source_text.get(range.clone()) {
            let trimmed = slice.trim();
            if trimmed == "---" || trimmed == "***" || trimmed == "___" {
                return Some(Cow::Borrowed(" "));
            }
            if (trimmed.starts_with("$$") && trimmed.ends_with("$$") && trimmed.len() >= 4)
                || (trimmed.starts_with('$') && trimmed.ends_with('$') && trimmed.len() >= 2)
            {
                return Some(Cow::Owned(format_latex_math(trimmed)));
            }
        }
        let list = self.list_at_marker(range)?;
        match list.kind {
            ListKind::Bullet => Some(Cow::Borrowed(match list.depth % 3 {
                0 => "• ",
                1 => "◦ ",
                _ => "▪ ",
            })),
            ListKind::Task { checked: false, .. } => Some(Cow::Borrowed("☐ ")),
            ListKind::Task { checked: true, .. } => Some(Cow::Borrowed("☑ ")),
            ListKind::Ordered => None,
        }
    }

    pub(super) fn list_at_marker(&self, range: &Range<usize>) -> Option<&ListMarker> {
        let index = self
            .lists
            .binary_search_by_key(&range.start, |list| list.range.start)
            .ok()?;
        self.lists.get(index).filter(|list| list.range == *range)
    }
}

pub(super) fn ranges_touch(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start <= b.end && a.end >= b.start
}

pub(super) fn format_latex_math(raw: &str) -> String {
    let mut s = raw.trim();
    if s.starts_with("$$") && s.ends_with("$$") && s.len() >= 4 {
        s = &s[2..s.len() - 2];
    } else if s.starts_with('$') && s.ends_with('$') && s.len() >= 2 {
        s = &s[1..s.len() - 1];
    }
    let s = s.trim();

    if !s.contains('\\') && !s.contains('^') && !s.contains('_') && !s.contains('{') {
        return s.to_string();
    }

    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            let mut end = i + 1;
            while end < chars.len() && chars[end].is_alphabetic() {
                end += 1;
            }
            if end > i + 1 {
                let cmd: String = chars[i + 1..end].iter().collect();
                let sym = match cmd.as_str() {
                    "alpha" => Some("α"),
                    "beta" => Some("β"),
                    "gamma" => Some("γ"),
                    "delta" => Some("δ"),
                    "epsilon" | "varepsilon" => Some("ε"),
                    "zeta" => Some("ζ"),
                    "eta" => Some("η"),
                    "theta" | "vartheta" => Some("θ"),
                    "iota" => Some("ι"),
                    "kappa" => Some("κ"),
                    "lambda" => Some("λ"),
                    "mu" => Some("μ"),
                    "nu" => Some("ν"),
                    "xi" => Some("ξ"),
                    "pi" | "varpi" => Some("π"),
                    "rho" | "varrho" => Some("ρ"),
                    "sigma" | "varsigma" => Some("σ"),
                    "tau" => Some("τ"),
                    "upsilon" => Some("υ"),
                    "phi" | "varphi" => Some("φ"),
                    "chi" => Some("χ"),
                    "psi" => Some("ψ"),
                    "omega" => Some("ω"),
                    "Gamma" => Some("Γ"),
                    "Delta" => Some("Δ"),
                    "Theta" => Some("Θ"),
                    "Lambda" => Some("Λ"),
                    "Xi" => Some("Ξ"),
                    "Pi" => Some("Π"),
                    "Sigma" => Some("Σ"),
                    "Phi" => Some("Φ"),
                    "Psi" => Some("Ψ"),
                    "Omega" => Some("Ω"),
                    "times" => Some("×"),
                    "cdot" => Some("·"),
                    "pm" => Some("±"),
                    "mp" => Some("∓"),
                    "div" => Some("÷"),
                    "neq" | "ne" => Some("≠"),
                    "leq" | "le" => Some("≤"),
                    "geq" | "ge" => Some("≥"),
                    "approx" => Some("≈"),
                    "equiv" => Some("≡"),
                    "infty" => Some("∞"),
                    "in" => Some("∈"),
                    "notin" => Some("∉"),
                    "subset" => Some("⊂"),
                    "subseteq" => Some("⊆"),
                    "cup" => Some("∪"),
                    "cap" => Some("∩"),
                    "emptyset" => Some("∅"),
                    "forall" => Some("∀"),
                    "exists" => Some("∃"),
                    "nabla" => Some("∇"),
                    "partial" => Some("∂"),
                    "sum" => Some("∑"),
                    "prod" => Some("∏"),
                    "int" => Some("∫"),
                    "to" | "rightarrow" => Some("→"),
                    "leftarrow" => Some("←"),
                    "Rightarrow" => Some("⇒"),
                    "Leftarrow" => Some("⇐"),
                    "iff" | "Leftrightarrow" => Some("⇔"),
                    "sqrt" => Some("√"),
                    "quad" | "qquad" => Some("  "),
                    _ => None,
                };
                if let Some(replacement) = sym {
                    out.push_str(replacement);
                    i = end;
                    continue;
                } else if cmd == "frac" {
                    i = end;
                    let mut num = String::new();
                    let mut den = String::new();
                    if i < chars.len() && chars[i] == '{' {
                        i += 1;
                        while i < chars.len() && chars[i] != '}' {
                            num.push(chars[i]);
                            i += 1;
                        }
                        if i < chars.len() && chars[i] == '}' {
                            i += 1;
                        }
                    }
                    if i < chars.len() && chars[i] == '{' {
                        i += 1;
                        while i < chars.len() && chars[i] != '}' {
                            den.push(chars[i]);
                            i += 1;
                        }
                        if i < chars.len() && chars[i] == '}' {
                            i += 1;
                        }
                    }
                    if !num.is_empty() || !den.is_empty() {
                        out.push_str(&format!("{num}/{den}"));
                        continue;
                    }
                }
            }
            out.push(c);
            i += 1;
        } else if c == '^' {
            i += 1;
            if i < chars.len() && chars[i] == '{' {
                i += 1;
                while i < chars.len() && chars[i] != '}' {
                    out.push(to_superscript(chars[i]));
                    i += 1;
                }
                if i < chars.len() && chars[i] == '}' {
                    i += 1;
                }
            } else if i < chars.len() {
                out.push(to_superscript(chars[i]));
                i += 1;
            }
        } else if c == '_' {
            i += 1;
            if i < chars.len() && chars[i] == '{' {
                i += 1;
                while i < chars.len() && chars[i] != '}' {
                    out.push(to_subscript(chars[i]));
                    i += 1;
                }
                if i < chars.len() && chars[i] == '}' {
                    i += 1;
                }
            } else if i < chars.len() {
                out.push(to_subscript(chars[i]));
                i += 1;
            }
        } else if c == '{' || c == '}' {
            i += 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn to_superscript(c: char) -> char {
    match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'n' => 'ⁿ',
        'i' => 'ⁱ',
        'x' => 'ˣ',
        _ => c,
    }
}

fn to_subscript(c: char) -> char {
    match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'i' => 'ᵢ',
        'o' => 'ₒ',
        'u' => 'ᵤ',
        'x' => 'ₓ',
        'n' => 'ₙ',
        _ => c,
    }
}
