pub mod model;
pub mod render;

use crate::theme::Theme;
use truff_core::vim::Key;

pub use model::{TocItem, TocItemKind, TocModel, TocScope};
pub use render::{TocAction, render_toc_dock, render_toc_overlay};

/// Component managing the Floating / Docked Table of Contents.
#[derive(Debug, Clone, Default)]
pub struct FloatingToc {
    pub model: TocModel,
    pub is_open: bool,
    pub is_pinned: bool,
    pending_g: bool,
}

impl FloatingToc {
    pub fn new() -> Self {
        Self {
            model: TocModel::new(),
            is_open: false,
            is_pinned: false,
            pending_g: false,
        }
    }

    pub fn toggle_open(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.pending_g = false;
    }

    pub fn set_pinned(&mut self, pinned: bool) {
        self.is_pinned = pinned;
    }

    pub fn toggle_pin(&mut self) {
        self.is_pinned = !self.is_pinned;
    }

    /// Renders docked in the right panel.
    pub fn show_dock(&mut self, ui: &mut egui::Ui, theme: &Theme) -> Option<TocAction> {
        let action = render_toc_dock(&mut self.model, ui, theme);
        self.handle_internal_action(&action);
        action
    }

    /// Renders as a floating overlay window.
    pub fn show_overlay(&mut self, ctx: &egui::Context, theme: &Theme) -> Option<TocAction> {
        if !self.is_open || self.is_pinned {
            return None;
        }
        let action = render_toc_overlay(&mut self.model, ctx, theme);
        self.handle_internal_action(&action);
        action
    }

    fn handle_internal_action(&mut self, action: &Option<TocAction>) {
        if let Some(act) = action {
            match act {
                TocAction::Close => {
                    self.close();
                }
                TocAction::TogglePin => {
                    self.toggle_pin();
                }
                TocAction::SwitchScope(scope) => {
                    self.model.scope = *scope;
                }
                _ => {}
            }
        }
    }

    /// Handles keyboard events when TOC is active/focused.
    pub fn feed_key(&mut self, key: Key) -> Option<TocAction> {
        if self.model.search_active {
            if key == Key::Esc {
                self.model.search_active = false;
                self.model.search_query.clear();
                self.model.refresh_visible();
                return None;
            }
            return None;
        }

        // Handle `gg`
        if self.pending_g {
            self.pending_g = false;
            if key == Key::Char('g') {
                self.model.move_top();
                return None;
            }
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
            Key::Char('h') | Key::Left => {
                self.model.collapse();
                None
            }
            Key::Char('l') | Key::Right => {
                self.model.expand();
                None
            }
            Key::Char('g') => {
                self.pending_g = true;
                None
            }
            Key::Char('G') | Key::End => {
                self.model.move_bottom();
                None
            }
            Key::Home => {
                self.model.move_top();
                None
            }
            Key::Char('/') => {
                self.model.search_active = true;
                None
            }
            Key::Char('p') => {
                self.toggle_pin();
                Some(TocAction::TogglePin)
            }
            Key::Tab => {
                self.model.scope = self.model.scope.next();
                Some(TocAction::SwitchScope(self.model.scope))
            }
            Key::ShiftTab => {
                self.model.scope = self.model.scope.prev();
                Some(TocAction::SwitchScope(self.model.scope))
            }
            Key::Enter => {
                if let Some(item) = self.model.selected_item() {
                    match &item.kind {
                        TocItemKind::Heading { line, .. } | TocItemKind::Block { line, .. } => {
                            Some(TocAction::JumpHeading {
                                line: *line,
                                path: item.path.clone(),
                            })
                        }
                        TocItemKind::Note { path } | TocItemKind::Related { path } => {
                            Some(TocAction::OpenFile { path: path.clone() })
                        }
                        TocItemKind::Folder { .. } => {
                            self.model.toggle_collapse();
                            None
                        }
                    }
                } else {
                    None
                }
            }
            Key::Char('o') => self
                .model
                .selected_item()
                .map(|item| TocAction::OpenAdjacent {
                    line: item.line,
                    path: item.path.clone(),
                }),
            Key::Esc => {
                self.close();
                Some(TocAction::Close)
            }
            _ => None,
        }
    }
}
