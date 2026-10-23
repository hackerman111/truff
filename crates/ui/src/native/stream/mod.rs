pub mod model;
pub mod render;

use crate::app::Application;
use crate::theme::Theme;
use truff_core::vim::Key;

pub use model::{NoteStreamModel, StreamCard, StreamFilter};
pub use render::{StreamAction, render_note_stream};

/// Controller managing the Note Stream feed.
#[derive(Debug, Clone, Default)]
pub struct NoteStream {
    pub model: NoteStreamModel,
    pub is_open: bool,
}

impl NoteStream {
    pub fn new() -> Self {
        Self {
            model: NoteStreamModel::new(),
            is_open: false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn open(&mut self, app: &Application) {
        self.model.load_from_vault(app);
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn show(&mut self, ctx: &egui::Context, theme: &Theme) -> Option<StreamAction> {
        if !self.is_open {
            return None;
        }
        let action = render_note_stream(&mut self.model, ctx, theme);
        if let Some(StreamAction::Close) = &action {
            self.close();
        }
        action
    }

    pub fn feed_key(&mut self, key: Key) -> Option<StreamAction> {
        if !self.is_open {
            return None;
        }

        match key {
            Key::Char('j') | Key::Down => {
                self.model.move_down(1);
                None
            }
            Key::Char('k') | Key::Up => {
                self.model.move_up(1);
                None
            }
            Key::Tab => {
                self.model.filter = self.model.filter.next();
                self.model.clamp_selection();
                Some(StreamAction::SwitchFilter(self.model.filter))
            }
            Key::ShiftTab => {
                self.model.filter = self.model.filter.prev();
                self.model.clamp_selection();
                Some(StreamAction::SwitchFilter(self.model.filter))
            }
            Key::Char('1') => {
                self.model.filter = StreamFilter::Today;
                self.model.clamp_selection();
                Some(StreamAction::SwitchFilter(self.model.filter))
            }
            Key::Char('2') => {
                self.model.filter = StreamFilter::Unfinished;
                self.model.clamp_selection();
                Some(StreamAction::SwitchFilter(self.model.filter))
            }
            Key::Char('3') => {
                self.model.filter = StreamFilter::Related;
                self.model.clamp_selection();
                Some(StreamAction::SwitchFilter(self.model.filter))
            }
            Key::Char('4') => {
                self.model.filter = StreamFilter::Revisit;
                self.model.clamp_selection();
                Some(StreamAction::SwitchFilter(self.model.filter))
            }
            Key::Char('5') => {
                self.model.filter = StreamFilter::All;
                self.model.clamp_selection();
                Some(StreamAction::SwitchFilter(self.model.filter))
            }
            Key::Char('a') => Some(StreamAction::CreateNewNote),
            Key::Enter => self
                .model
                .selected_card()
                .map(|card| StreamAction::OpenNote {
                    path: card.path.clone(),
                }),
            Key::Esc => {
                self.close();
                Some(StreamAction::Close)
            }
            _ => None,
        }
    }
}
