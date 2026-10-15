use crate::rows::{Alignment, RowScroll};
use truff_core::vim::{
    Command, Key, Mode, Motion,
    parser::{ParseResult, VimParser},
};

#[derive(Clone, Debug)]
pub(crate) struct HelpItem {
    pub category: String,
    pub keys: String,
    pub description: String,
}

#[derive(Clone, Default, Debug)]
pub(crate) struct HelpState {
    pub items: Vec<HelpItem>,
    pub filtered: Vec<usize>,
    pub query: String,
    pub inserting: bool,
    pub selected: usize,
    pub scroll: RowScroll,
    parser: VimParser,
}

impl HelpState {
    pub fn set_items(&mut self, items: Vec<HelpItem>) {
        self.items = items;
        self.filter();
    }

    pub fn filter(&mut self) {
        let query = self.query.to_lowercase();
        self.filtered = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                (query.is_empty()
                    || item.description.to_lowercase().contains(&query)
                    || item.keys.to_lowercase().contains(&query)
                    || item.category.to_lowercase().contains(&query))
                .then_some(i)
            })
            .collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
        self.scroll.request(self.selected, Alignment::Nearest);
    }

    /// Returns true only when Escape closes Help; Insert Escape exits the filter first.
    pub fn key(&mut self, key: Key) -> bool {
        if key == Key::Esc {
            self.parser.reset();
            if self.inserting {
                self.inserting = false;
                return false;
            }
            return true;
        }
        if self.inserting {
            return false;
        }
        match key {
            Key::Char('/') => {
                self.inserting = true;
                self.parser.reset();
            }
            Key::PageDown | Key::Ctrl('d') | Key::PageUp | Key::Ctrl('u') => {
                let rows = self.scroll.page_rows();
                let rows = if matches!(key, Key::Ctrl(_)) {
                    (rows / 2).max(1)
                } else {
                    rows
                };
                self.selected = if matches!(key, Key::PageDown | Key::Ctrl('d')) {
                    self.selected
                        .saturating_add(rows)
                        .min(self.filtered.len().saturating_sub(1))
                } else {
                    self.selected.saturating_sub(rows)
                };
            }
            Key::Home => {
                self.selected = 0;
                self.scroll.request(0, Alignment::Start);
            }
            Key::End => {
                self.selected = self.filtered.len().saturating_sub(1);
                self.scroll.request(self.selected, Alignment::End);
            }
            Key::Char('j' | 'k' | 'g' | 'G' | '0'..='9') | Key::Up | Key::Down => {
                if let ParseResult::Complete(Command::Motion { motion, count }) =
                    self.parser.feed(key, Mode::Normal)
                {
                    match motion {
                        Motion::Down => {
                            self.selected = self
                                .selected
                                .saturating_add(count)
                                .min(self.filtered.len().saturating_sub(1))
                        }
                        Motion::Up => self.selected = self.selected.saturating_sub(count),
                        Motion::FirstLine => {
                            self.selected = 0;
                            self.scroll.request(0, Alignment::Start);
                        }
                        Motion::LastLine => {
                            self.selected = self.filtered.len().saturating_sub(1);
                            self.scroll.request(self.selected, Alignment::End);
                        }
                        Motion::Line(line) => {
                            self.selected = line
                                .saturating_sub(1)
                                .min(self.filtered.len().saturating_sub(1))
                        }
                        _ => {}
                    }
                }
            }
            _ => self.parser.reset(),
        }
        false
    }
}
