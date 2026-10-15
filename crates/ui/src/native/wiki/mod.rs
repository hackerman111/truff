pub mod model;
pub mod render;

use crate::app::Application;
use crate::theme::Theme;
use eframe::egui;
use truff_core::vim::Key;

pub use model::{FolderWikiData, WikiBacklink, WikiNoteItem, WikiTab, WikiTask};
pub use render::{WikiAction, render_folder_wiki};

/// Controller managing the Folder Wiki dashboard.
#[derive(Debug, Clone, Default)]
pub struct FolderWiki {
    pub data: FolderWikiData,
    pub is_open: bool,
}

impl FolderWiki {
    pub fn new() -> Self {
        Self {
            data: FolderWikiData::new(""),
            is_open: false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn open_folder(&mut self, folder: &str, app: &Application) {
        self.data.load_from_vault(folder, app);
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn show(&mut self, ctx: &egui::Context, theme: &Theme) -> Option<WikiAction> {
        if !self.is_open {
            return None;
        }
        let action = render_folder_wiki(&mut self.data, ctx, theme);
        if let Some(WikiAction::Close) = &action {
            self.close();
        }
        action
    }

    pub fn feed_key(&mut self, key: Key) -> Option<WikiAction> {
        if !self.is_open {
            return None;
        }

        match key {
            Key::Char('j') | Key::Down => {
                self.data.move_down(1);
                None
            }
            Key::Char('k') | Key::Up => {
                self.data.move_up(1);
                None
            }
            Key::Tab => {
                self.data.active_tab = self.data.active_tab.next();
                self.data.selected_index = 0;
                None
            }
            Key::ShiftTab => {
                self.data.active_tab = self.data.active_tab.prev();
                self.data.selected_index = 0;
                None
            }
            Key::Char('1') => {
                self.data.active_tab = WikiTab::Overview;
                self.data.selected_index = 0;
                None
            }
            Key::Char('2') => {
                self.data.active_tab = WikiTab::NotesTree;
                self.data.selected_index = 0;
                None
            }
            Key::Char('3') => {
                self.data.active_tab = WikiTab::Tasks;
                self.data.selected_index = 0;
                None
            }
            Key::Char('4') => {
                self.data.active_tab = WikiTab::MetadataTable;
                self.data.selected_index = 0;
                None
            }
            Key::Char('5') => {
                self.data.active_tab = WikiTab::Backlinks;
                self.data.selected_index = 0;
                None
            }
            Key::Char('a') => Some(WikiAction::CreateNewNote {
                folder: self.data.folder_path.clone(),
            }),
            Key::Enter => {
                match self.data.active_tab {
                    WikiTab::Overview | WikiTab::MetadataTable => {
                        self.data.notes.get(self.data.selected_index).map(|note| {
                            WikiAction::OpenNote {
                                path: note.path.clone(),
                                line: None,
                            }
                        })
                    }
                    WikiTab::NotesTree => {
                        self.data.notes.get(self.data.selected_index).map(|note| {
                            WikiAction::OpenNote {
                                path: note.path.clone(),
                                line: None,
                            }
                        })
                    }
                    WikiTab::Tasks => self.data.tasks.get(self.data.selected_index).map(|task| {
                        WikiAction::OpenNote {
                            path: task.note_path.clone(),
                            line: Some(task.line),
                        }
                    }),
                    WikiTab::Backlinks => {
                        self.data.backlinks.get(self.data.selected_index).map(|bl| {
                            WikiAction::OpenNote {
                                path: bl.source_note.clone(),
                                line: None,
                            }
                        })
                    }
                }
            }
            Key::Esc => {
                self.close();
                Some(WikiAction::Close)
            }
            _ => None,
        }
    }
}
