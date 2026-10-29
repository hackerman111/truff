use crate::{components::color, theme::Theme};
use egui::{Color32, FontId, Margin, Stroke, TextFormat, text::LayoutJob};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// State for active inline formatting styles.
#[derive(Default, Clone)]
struct InlineState {
    bold: bool,
    italic: bool,
    strikethrough: bool,
    code: bool,
    link: bool,
    highlight: bool,
    color: Option<Color32>,
}

impl InlineState {
    fn to_format(&self, theme: &Theme, base_size: f32) -> TextFormat {
        let text_color = if let Some(custom) = self.color {
            custom
        } else if self.link || self.code {
            color(theme.accent_primary)
        } else {
            color(theme.text_primary)
        };

        let font_id = if self.code {
            FontId::monospace(base_size - 1.0)
        } else {
            FontId::proportional(base_size)
        };

        let background = if self.highlight {
            Color32::from_rgba_premultiplied(235, 203, 139, 100)
        } else if self.code {
            color(theme.bg_secondary)
        } else {
            Color32::TRANSPARENT
        };

        let underline = if self.link {
            Stroke::new(1.0, color(theme.accent_primary))
        } else {
            Stroke::NONE
        };

        let strikethrough = if self.strikethrough {
            Stroke::new(1.0, color(theme.text_muted))
        } else {
            Stroke::NONE
        };

        TextFormat {
            font_id,
            color: text_color,
            background,
            italics: self.italic,
            underline,
            strikethrough,
            ..Default::default()
        }
    }
}

/// Appends text to a `LayoutJob`, handling inline wikilinks `[[...]]`, highlights `==...==`,
/// and color spans `<span style="color:#...">...</span>`.
fn append_styled_text(
    job: &mut LayoutJob,
    raw: &str,
    state: &InlineState,
    theme: &Theme,
    base_size: f32,
) {
    if raw.is_empty() {
        return;
    }

    // Fast path: if no special wiki/highlight/color syntax exists, append directly.
    if !raw.contains("[[") && !raw.contains("==") && !raw.contains("<span style=\"color:#") {
        job.append(raw, 0.0, state.to_format(theme, base_size));
        return;
    }

    let mut remaining = raw;
    while !remaining.is_empty() {
        // Find nearest special marker
        let wiki_pos = remaining.find("[[");
        let hl_pos = remaining.find("==");
        let span_pos = remaining.find("<span style=\"color:#");

        let mut earliest: Option<(usize, &str)> = None;
        for (pos, marker) in [
            (wiki_pos, "[["),
            (hl_pos, "=="),
            (span_pos, "<span style=\"color:#"),
        ] {
            if let Some(p) = pos
                && earliest.is_none_or(|(best_pos, _)| p < best_pos)
            {
                earliest = Some((p, marker));
            }
        }

        let Some((pos, marker)) = earliest else {
            job.append(remaining, 0.0, state.to_format(theme, base_size));
            break;
        };

        // Append text before marker
        if pos > 0 {
            job.append(&remaining[..pos], 0.0, state.to_format(theme, base_size));
        }

        match marker {
            "[[" => {
                let tail = &remaining[pos + 2..];
                if let Some(close) = tail.find("]]") {
                    let link_text = &tail[..close];
                    // Support [[target|alias]]
                    let display_text = link_text.rsplit_once('|').map_or(link_text, |(_, a)| a);
                    let mut link_state = state.clone();
                    link_state.link = true;
                    job.append(display_text, 0.0, link_state.to_format(theme, base_size));
                    remaining = &tail[close + 2..];
                } else {
                    job.append("[[", 0.0, state.to_format(theme, base_size));
                    remaining = tail;
                }
            }
            "==" => {
                let tail = &remaining[pos + 2..];
                if let Some(close) = tail.find("==") {
                    let hl_text = &tail[..close];
                    let mut hl_state = state.clone();
                    hl_state.highlight = true;
                    job.append(hl_text, 0.0, hl_state.to_format(theme, base_size));
                    remaining = &tail[close + 2..];
                } else {
                    job.append("==", 0.0, state.to_format(theme, base_size));
                    remaining = tail;
                }
            }
            "<span style=\"color:#" => {
                let prefix_len = "<span style=\"color:#".len();
                let tail = &remaining[pos + prefix_len..];
                if tail.len() >= 8 && &tail[6..8] == "\">" {
                    let hex = &tail[..6];
                    let content_tail = &tail[8..];
                    if let Some(close) = content_tail.find("</span>") {
                        let span_content = &content_tail[..close];
                        let mut span_state = state.clone();
                        if let Ok(c) = Color32::from_hex(&format!("#{hex}")) {
                            span_state.color = Some(c);
                        }
                        job.append(span_content, 0.0, span_state.to_format(theme, base_size));
                        remaining = &content_tail[close + "</span>".len()..];
                        continue;
                    }
                }
                job.append(
                    "<span style=\"color:#",
                    0.0,
                    state.to_format(theme, base_size),
                );
                remaining = tail;
            }
            _ => unreachable!(),
        }
    }
}

/// Renders markdown text into an `egui::Ui`.
pub fn render(ui: &mut egui::Ui, text: &str, theme: &Theme) {
    if text.is_empty() {
        return;
    }

    let mut options = Options::all();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(text, options);
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);

    let mut inline = InlineState::default();
    let mut current_heading: Option<(u8, LayoutJob)> = None;
    let mut current_paragraph: Option<LayoutJob> = None;
    let mut current_code_block: Option<(String, Option<String>)> = None;
    let mut current_quote: Option<(Vec<LayoutJob>, Option<String>, Option<String>)> = None; // (jobs, callout_tag, callout_title)
    let mut list_stack: Vec<(usize, Option<u64>)> = Vec::new(); // (depth, current_ordered_counter)
    let mut current_item: Option<(LayoutJob, Option<bool>)> = None; // (job, task_checked)
    let mut table_rows: Option<Vec<Vec<String>>> = None;
    let mut current_cell: Option<String> = None;

    let base_font_size = theme.geometry.ui_font_size;

    for event in parser {
        match event {
            // --- Headings ---
            Event::Start(Tag::Heading { level, .. }) => {
                let lvl = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                current_heading = Some((lvl, LayoutJob::default()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((lvl, job)) = current_heading.take() {
                    let (font_size, space_above, space_below, heading_color) = match lvl {
                        1 => (base_font_size + 4.0, 8.0, 4.0, color(theme.accent_primary)),
                        2 => (base_font_size + 2.5, 6.0, 3.0, color(theme.text_primary)),
                        3 => (base_font_size + 1.5, 5.0, 2.0, color(theme.text_primary)),
                        _ => (base_font_size, 4.0, 2.0, color(theme.text_secondary)),
                    };

                    let mut job = job;
                    for section in &mut job.sections {
                        section.format.font_id = FontId::proportional(font_size);
                        if lvl == 1 {
                            section.format.color = heading_color;
                        }
                    }

                    ui.add_space(space_above);
                    ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Wrap));
                    if lvl <= 2 {
                        ui.add_space(2.0);
                        ui.separator();
                    }
                    ui.add_space(space_below);
                }
            }

            // --- Paragraphs ---
            Event::Start(Tag::Paragraph) => {
                current_paragraph = Some(LayoutJob::default());
            }
            Event::End(TagEnd::Paragraph) => {
                if let Some(job) = current_paragraph.take() {
                    if let Some((jobs, callout_tag, callout_title)) = &mut current_quote {
                        // Check if first line of quote is an Obsidian callout tag: `[!note]` or `[!tip] Title`
                        if jobs.is_empty() && callout_tag.is_none() {
                            let plain_text = job.text.trim();
                            if plain_text.starts_with("[!")
                                && let Some(tag_end) = plain_text.find(']')
                            {
                                let tag = plain_text[2..tag_end].to_lowercase();
                                let title_part = plain_text[tag_end + 1..].trim();
                                *callout_tag = Some(tag);
                                *callout_title = if title_part.is_empty() {
                                    None
                                } else {
                                    Some(title_part.to_string())
                                };
                                continue;
                            }
                        }
                        jobs.push(job);
                    } else if let Some((item_job, _)) = &mut current_item {
                        if !item_job.text.is_empty() && !job.text.is_empty() {
                            item_job.append(
                                " ",
                                0.0,
                                TextFormat {
                                    font_id: FontId::proportional(base_font_size),
                                    color: color(theme.text_primary),
                                    ..Default::default()
                                },
                            );
                        }
                        for section in job.sections {
                            let text =
                                &job.text[section.byte_range.start.0..section.byte_range.end.0];
                            item_job.append(text, section.leading_space, section.format);
                        }
                    } else {
                        ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Wrap));
                        ui.add_space(3.0);
                    }
                }
            }

            // --- Code Blocks ---
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => {
                        let s = l.as_ref().trim();
                        if s.is_empty() {
                            None
                        } else {
                            Some(s.to_string())
                        }
                    }
                    CodeBlockKind::Indented => None,
                };
                current_code_block = Some((String::new(), lang));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((code_str, lang)) = current_code_block.take() {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .fill(color(theme.bg_secondary))
                        .stroke(Stroke::new(1.0, color(theme.border_subtle)))
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(Margin::same(8))
                        .show(ui, |ui| {
                            if let Some(l) = lang {
                                ui.horizontal(|ui| {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(l)
                                                    .size(base_font_size - 3.0)
                                                    .color(color(theme.text_muted)),
                                            );
                                        },
                                    );
                                });
                            }
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(code_str.trim_end())
                                        .monospace()
                                        .size(base_font_size - 1.0)
                                        .color(color(theme.text_primary)),
                                )
                                .wrap_mode(egui::TextWrapMode::Wrap),
                            );
                        });
                    ui.add_space(4.0);
                }
            }

            // --- Blockquotes & Callouts ---
            Event::Start(Tag::BlockQuote(_)) => {
                current_quote = Some((Vec::new(), None, None));
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                if let Some((jobs, callout_tag, callout_title)) = current_quote.take() {
                    ui.add_space(4.0);
                    let (border_color, default_title, icon_str) = match callout_tag.as_deref() {
                        Some("tip") => (color(theme.callout_tip), "Tip", "💡"),
                        Some("warning" | "caution") => {
                            (color(theme.callout_warning), "Warning", "⚠️")
                        }
                        Some("important") => (Color32::from_rgb(180, 142, 173), "Important", "❗"),
                        Some("danger") => (color(theme.callout_warning), "Danger", "🔥"),
                        Some(_) => (color(theme.accent_primary), "Note", "📝"),
                        None => (color(theme.border_subtle), "", ""),
                    };

                    egui::Frame::new()
                        .fill(color(theme.bg_secondary))
                        .stroke(Stroke::new(1.0, color(theme.border_subtle)))
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(Margin::same(8))
                        .show(ui, |ui| {
                            let rect = ui.max_rect();
                            // Left accent line
                            ui.painter().line_segment(
                                [
                                    egui::pos2(rect.left() + 2.0, rect.top() + 4.0),
                                    egui::pos2(rect.left() + 2.0, rect.bottom() - 4.0),
                                ],
                                Stroke::new(3.0, border_color),
                            );

                            ui.allocate_ui_with_layout(
                                egui::vec2(ui.available_width() - 8.0, 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    if callout_tag.is_some() {
                                        let title =
                                            callout_title.as_deref().unwrap_or(default_title);
                                        ui.horizontal(|ui| {
                                            ui.label(icon_str);
                                            ui.label(
                                                egui::RichText::new(title)
                                                    .strong()
                                                    .color(border_color),
                                            );
                                        });
                                        ui.add_space(2.0);
                                    }

                                    for job in jobs {
                                        ui.add(
                                            egui::Label::new(job)
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                    }
                                },
                            );
                        });
                    ui.add_space(4.0);
                }
            }

            // --- Lists ---
            Event::Start(Tag::List(start_num)) => {
                let depth = list_stack.len();
                list_stack.push((depth, start_num));
            }
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
            }
            Event::Start(Tag::Item) => {
                current_item = Some((LayoutJob::default(), None));
            }
            Event::TaskListMarker(checked) => {
                if let Some((_, task)) = &mut current_item {
                    *task = Some(checked);
                }
            }
            Event::End(TagEnd::Item) => {
                if let Some((job, task)) = current_item.take() {
                    let depth = list_stack.len().saturating_sub(1);
                    let is_ordered = list_stack.last().and_then(|(_, num)| *num);

                    ui.horizontal_top(|ui| {
                        ui.add_space((depth as f32) * 14.0);

                        if let Some(checked) = task {
                            let (icon, color) = if checked {
                                ("☑ ", color(theme.callout_tip))
                            } else {
                                ("☐ ", color(theme.callout_warning))
                            };
                            ui.label(egui::RichText::new(icon).strong().color(color));
                        } else if let Some(num) = is_ordered {
                            ui.label(
                                egui::RichText::new(format!("{num}. "))
                                    .color(color(theme.text_muted)),
                            );
                            if let Some((_, counter)) = list_stack.last_mut() {
                                *counter = Some(num + 1);
                            }
                        } else {
                            let bullet = if depth.is_multiple_of(2) {
                                "• "
                            } else {
                                "◦ "
                            };
                            ui.label(
                                egui::RichText::new(bullet).color(color(theme.accent_primary)),
                            );
                        }

                        ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Wrap));
                    });
                    ui.add_space(1.0);
                }
            }

            // --- Horizontal Rule ---
            Event::Rule => {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
            }

            // --- Tables ---
            Event::Start(Tag::Table(_)) => {
                table_rows = Some(Vec::new());
            }
            Event::End(TagEnd::Table) => {
                if let Some(rows) = table_rows.take()
                    && !rows.is_empty()
                {
                    ui.add_space(4.0);
                    egui::Grid::new("markdown_preview_table")
                        .striped(true)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            for (row_idx, row) in rows.iter().enumerate() {
                                for cell in row {
                                    let text = egui::RichText::new(cell).color(if row_idx == 0 {
                                        color(theme.text_primary)
                                    } else {
                                        color(theme.text_secondary)
                                    });
                                    let text = if row_idx == 0 { text.strong() } else { text };
                                    ui.add(
                                        egui::Label::new(text).wrap_mode(egui::TextWrapMode::Wrap),
                                    );
                                }
                                ui.end_row();
                            }
                        });
                    ui.add_space(4.0);
                }
            }
            Event::Start(Tag::TableHead | Tag::TableRow) => {
                if let Some(rows) = &mut table_rows {
                    rows.push(Vec::new());
                }
            }
            Event::Start(Tag::TableCell) => {
                current_cell = Some(String::new());
            }
            Event::End(TagEnd::TableCell) => {
                if let Some(cell_text) = current_cell.take()
                    && let Some(rows) = &mut table_rows
                    && let Some(last_row) = rows.last_mut()
                {
                    last_row.push(cell_text);
                }
            }

            // --- Inline Tags ---
            Event::Start(Tag::Strong) => inline.bold = true,
            Event::End(TagEnd::Strong) => inline.bold = false,
            Event::Start(Tag::Emphasis) => inline.italic = true,
            Event::End(TagEnd::Emphasis) => inline.italic = false,
            Event::Start(Tag::Strikethrough) => inline.strikethrough = true,
            Event::End(TagEnd::Strikethrough) => inline.strikethrough = false,
            Event::Start(Tag::Link { .. }) => inline.link = true,
            Event::End(TagEnd::Link) => inline.link = false,

            // --- Text Content ---
            Event::Text(txt) => {
                if let Some((code_str, _)) = &mut current_code_block {
                    code_str.push_str(&txt);
                } else if let Some(cell) = &mut current_cell {
                    cell.push_str(&txt);
                } else if let Some((_, job)) = &mut current_heading {
                    append_styled_text(job, &txt, &inline, theme, base_font_size + 2.0);
                } else if let Some((job, _)) = &mut current_item {
                    append_styled_text(job, &txt, &inline, theme, base_font_size);
                } else if let Some(job) = &mut current_paragraph {
                    append_styled_text(job, &txt, &inline, theme, base_font_size);
                }
            }
            Event::Code(code) => {
                let mut code_state = inline.clone();
                code_state.code = true;
                if let Some(cell) = &mut current_cell {
                    cell.push_str(&code);
                } else if let Some((_, job)) = &mut current_heading {
                    job.append(
                        &code,
                        0.0,
                        code_state.to_format(theme, base_font_size + 2.0),
                    );
                } else if let Some((job, _)) = &mut current_item {
                    job.append(&code, 0.0, code_state.to_format(theme, base_font_size));
                } else if let Some(job) = &mut current_paragraph {
                    job.append(&code, 0.0, code_state.to_format(theme, base_font_size));
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                let sep = " ";
                if let Some((_, job)) = &mut current_heading {
                    job.append(sep, 0.0, inline.to_format(theme, base_font_size + 2.0));
                } else if let Some((job, _)) = &mut current_item {
                    job.append(sep, 0.0, inline.to_format(theme, base_font_size));
                } else if let Some(job) = &mut current_paragraph {
                    job.append(sep, 0.0, inline.to_format(theme, base_font_size));
                }
            }
            _ => {}
        }
    }
}
