use std::ops::Range;
use truff_core::vim::Key;

/// Visual palette option for color and highlight choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorOption {
    pub name: &'static str,
    pub hex: &'static str,
    pub key: char,
}

pub const PALETTE: &[ColorOption] = &[
    ColorOption {
        name: "Красный",
        hex: "#BF616A",
        key: 'r',
    },
    ColorOption {
        name: "Оранжевый",
        hex: "#D08770",
        key: 'o',
    },
    ColorOption {
        name: "Жёлтый",
        hex: "#EBCB8B",
        key: 'y',
    },
    ColorOption {
        name: "Зелёный",
        hex: "#A3BE8C",
        key: 'g',
    },
    ColorOption {
        name: "Голубой",
        hex: "#88C0D0",
        key: 'b',
    },
    ColorOption {
        name: "Синий",
        hex: "#81A1C1",
        key: 's',
    },
    ColorOption {
        name: "Фиолетовый",
        hex: "#B48EAD",
        key: 'p',
    },
];

/// Mode / step of the TextAction overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextActionMode {
    Root {
        has_selection: bool,
    },
    ColorSelect {
        is_highlight: bool,
    },
    LinkInput {
        is_url: bool,
        has_selection: bool,
        target_input: String,
        title_input: String,
        active_field: usize,
    },
}

impl Default for TextActionMode {
    fn default() -> Self {
        Self::Root {
            has_selection: false,
        }
    }
}

/// Action produced by the TextAction overlay for the editor to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextActionResponse {
    None,
    Close,
    ApplyColor {
        hex: &'static str,
    },
    ApplyHighlight {
        hex: &'static str,
    },
    ApplyLink {
        is_url: bool,
        target: String,
        title: Option<String>,
    },
    RemoveStyle,
}

/// Controller for the inline text action menu triggered by `<Space> c`.
#[derive(Debug, Clone, Default)]
pub struct TextActionOverlay {
    is_open: bool,
    mode: TextActionMode,
    cursor_pos: Option<[f32; 2]>,
    color_index: usize,
    initial_text: Option<String>,
}

impl TextActionOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn mode(&self) -> &TextActionMode {
        &self.mode
    }

    pub fn mode_mut(&mut self) -> &mut TextActionMode {
        &mut self.mode
    }

    pub fn cursor_pos(&self) -> Option<[f32; 2]> {
        self.cursor_pos
    }

    pub fn selected_color_index(&self) -> usize {
        self.color_index
    }

    pub fn open(&mut self, initial_text: Option<String>, cursor_pos: Option<[f32; 2]>) {
        self.is_open = true;
        let has_selection = initial_text.as_ref().is_some_and(|s| !s.trim().is_empty());
        self.mode = TextActionMode::Root { has_selection };
        self.cursor_pos = cursor_pos;
        self.color_index = 0;
        self.initial_text = initial_text;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.mode = TextActionMode::Root {
            has_selection: false,
        };
        self.cursor_pos = None;
        self.initial_text = None;
    }

    pub fn initial_text(&self) -> Option<&str> {
        self.initial_text.as_deref()
    }

    pub fn handle_key(&mut self, key: Key) -> TextActionResponse {
        if !self.is_open {
            return TextActionResponse::None;
        }

        match &mut self.mode {
            TextActionMode::Root { has_selection } => {
                let has_sel = *has_selection;
                match key {
                    Key::Char('c') | Key::Char('C') if has_sel => {
                        self.mode = TextActionMode::ColorSelect {
                            is_highlight: false,
                        };
                        self.color_index = 0;
                        TextActionResponse::None
                    }
                    Key::Char('h') | Key::Char('H') if has_sel => {
                        self.mode = TextActionMode::ColorSelect { is_highlight: true };
                        self.color_index = 0;
                        TextActionResponse::None
                    }
                    Key::Char('n') | Key::Char('N') => {
                        if has_sel {
                            let text = self.initial_text.clone().unwrap_or_default();
                            self.close();
                            TextActionResponse::ApplyLink {
                                is_url: false,
                                target: text,
                                title: None,
                            }
                        } else {
                            self.mode = TextActionMode::LinkInput {
                                is_url: false,
                                has_selection: false,
                                target_input: String::new(),
                                title_input: String::new(),
                                active_field: 0,
                            };
                            TextActionResponse::None
                        }
                    }
                    Key::Char('u') | Key::Char('U') => {
                        if has_sel {
                            let text = self.initial_text.clone().unwrap_or_default();
                            self.mode = TextActionMode::LinkInput {
                                is_url: true,
                                has_selection: true,
                                target_input: String::new(),
                                title_input: text,
                                active_field: 0,
                            };
                            TextActionResponse::None
                        } else {
                            self.mode = TextActionMode::LinkInput {
                                is_url: true,
                                has_selection: false,
                                target_input: String::new(),
                                title_input: String::new(),
                                active_field: 0,
                            };
                            TextActionResponse::None
                        }
                    }
                    Key::Char('x') | Key::Char('X') | Key::Char('0') if has_sel => {
                        self.close();
                        TextActionResponse::RemoveStyle
                    }
                    Key::Esc => {
                        self.close();
                        TextActionResponse::Close
                    }
                    _ => TextActionResponse::None,
                }
            }
            TextActionMode::ColorSelect { is_highlight } => {
                let is_highlight = *is_highlight;
                match key {
                    Key::Left | Key::Up => {
                        if self.color_index == 0 {
                            self.color_index = PALETTE.len().saturating_sub(1);
                        } else {
                            self.color_index -= 1;
                        }
                        TextActionResponse::None
                    }
                    Key::Right | Key::Down => {
                        self.color_index = (self.color_index + 1) % PALETTE.len();
                        TextActionResponse::None
                    }
                    Key::Enter => {
                        let hex = PALETTE[self.color_index].hex;
                        self.close();
                        if is_highlight {
                            TextActionResponse::ApplyHighlight { hex }
                        } else {
                            TextActionResponse::ApplyColor { hex }
                        }
                    }
                    Key::Char(ch) => {
                        let lower = ch.to_ascii_lowercase();
                        if let Some(opt) = PALETTE.iter().find(|o| o.key == lower) {
                            let hex = opt.hex;
                            self.close();
                            if is_highlight {
                                TextActionResponse::ApplyHighlight { hex }
                            } else {
                                TextActionResponse::ApplyColor { hex }
                            }
                        } else {
                            TextActionResponse::None
                        }
                    }
                    Key::Esc | Key::Backspace => {
                        self.mode = TextActionMode::Root {
                            has_selection: true,
                        };
                        TextActionResponse::None
                    }
                    _ => TextActionResponse::None,
                }
            }
            TextActionMode::LinkInput {
                is_url,
                has_selection,
                target_input,
                title_input,
                active_field,
            } => {
                let is_url = *is_url;
                let has_sel = *has_selection;
                match key {
                    Key::Esc => {
                        self.close();
                        TextActionResponse::Close
                    }
                    Key::Tab if !has_sel => {
                        *active_field = (*active_field + 1) % 2;
                        TextActionResponse::None
                    }
                    Key::Enter => {
                        let target = target_input.trim().to_string();
                        if target.is_empty() {
                            return TextActionResponse::None;
                        }
                        let title = if has_sel {
                            if is_url {
                                let t = title_input.trim();
                                if t.is_empty() {
                                    None
                                } else {
                                    Some(t.to_string())
                                }
                            } else {
                                None
                            }
                        } else {
                            let t = title_input.trim();
                            if t.is_empty() {
                                None
                            } else {
                                Some(t.to_string())
                            }
                        };
                        self.close();
                        TextActionResponse::ApplyLink {
                            is_url,
                            target,
                            title,
                        }
                    }
                    _ => TextActionResponse::None,
                }
            }
        }
    }
}

/// Identifies word boundary `[start..end]` around `byte_pos` in `buffer`.
pub fn word_boundary_at(buffer: &truff_core::Buffer, byte_pos: usize) -> Option<Range<usize>> {
    let len = buffer.byte_len();
    if len == 0 || byte_pos >= len {
        return None;
    }

    let line_idx = buffer.line_of_byte(byte_pos);
    let line_start = buffer.byte_of_line(line_idx);
    let next_line_start = if line_idx + 1 < buffer.line_len() {
        buffer.byte_of_line(line_idx + 1)
    } else {
        len
    };

    let line_text = buffer.slice(line_start..next_line_start).to_string();
    let col = byte_pos.saturating_sub(line_start);

    let char_indices: Vec<(usize, char)> = line_text.char_indices().collect();
    if char_indices.is_empty() {
        return None;
    }

    let target_idx = char_indices
        .iter()
        .position(|&(idx, ch)| idx <= col && col < idx + ch.len_utf8())?;

    let (_, target_char) = char_indices[target_idx];
    if !is_word_char(target_char) {
        return None;
    }

    let mut start_idx = target_idx;
    while start_idx > 0 && is_word_char(char_indices[start_idx - 1].1) {
        start_idx -= 1;
    }

    let mut end_idx = target_idx;
    while end_idx + 1 < char_indices.len() && is_word_char(char_indices[end_idx + 1].1) {
        end_idx += 1;
    }

    let word_start = line_start + char_indices[start_idx].0;
    let (last_char_offset, last_char) = char_indices[end_idx];
    let word_end = line_start + last_char_offset + last_char.len_utf8();

    if word_start < word_end {
        Some(word_start..word_end)
    } else {
        None
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use truff_core::{BufferId, text::Buffer};

    #[test]
    fn test_menu_initial_state_and_navigation_to_color() {
        let mut menu = TextActionOverlay::new();
        assert!(!menu.is_open());

        menu.open(Some("Sample text".into()), Some([100.0, 150.0]));
        assert!(menu.is_open());
        assert_eq!(
            menu.mode(),
            &TextActionMode::Root {
                has_selection: true
            }
        );

        let action = menu.handle_key(Key::Char('c'));
        assert_eq!(action, TextActionResponse::None);
        assert!(matches!(
            menu.mode(),
            TextActionMode::ColorSelect {
                is_highlight: false
            }
        ));

        assert_eq!(menu.selected_color_index(), 0);
        menu.handle_key(Key::Right);
        assert_eq!(menu.selected_color_index(), 1);

        let action = menu.handle_key(Key::Enter);
        assert!(matches!(action, TextActionResponse::ApplyColor { hex } if hex == PALETTE[1].hex));
        assert!(!menu.is_open());
    }

    #[test]
    fn test_menu_without_selection_rejects_styles() {
        let mut menu = TextActionOverlay::new();
        menu.open(None, None);
        assert_eq!(
            menu.mode(),
            &TextActionMode::Root {
                has_selection: false
            }
        );

        // c, h, x should not change mode when there is no selection
        assert_eq!(menu.handle_key(Key::Char('c')), TextActionResponse::None);
        assert_eq!(
            menu.mode(),
            &TextActionMode::Root {
                has_selection: false
            }
        );

        assert_eq!(menu.handle_key(Key::Char('h')), TextActionResponse::None);
        assert_eq!(
            menu.mode(),
            &TextActionMode::Root {
                has_selection: false
            }
        );

        assert_eq!(menu.handle_key(Key::Char('x')), TextActionResponse::None);
        assert_eq!(
            menu.mode(),
            &TextActionMode::Root {
                has_selection: false
            }
        );
    }

    #[test]
    fn test_menu_with_selection_n_instant_apply() {
        let mut menu = TextActionOverlay::new();
        menu.open(Some("My Note".into()), None);

        let action = menu.handle_key(Key::Char('n'));
        assert_eq!(
            action,
            TextActionResponse::ApplyLink {
                is_url: false,
                target: "My Note".into(),
                title: None,
            }
        );
        assert!(!menu.is_open());
    }

    #[test]
    fn test_menu_with_selection_u_opens_single_url_input() {
        let mut menu = TextActionOverlay::new();
        menu.open(Some("My Site".into()), None);

        let action = menu.handle_key(Key::Char('u'));
        assert_eq!(action, TextActionResponse::None);
        assert!(matches!(
            menu.mode(),
            TextActionMode::LinkInput {
                is_url: true,
                has_selection: true,
                title_input,
                active_field: 0,
                ..
            } if title_input == "My Site"
        ));

        // In LinkInput, simulate entering url
        if let TextActionMode::LinkInput { target_input, .. } = menu.mode_mut() {
            *target_input = "https://example.com".into();
        }

        let action = menu.handle_key(Key::Enter);
        assert_eq!(
            action,
            TextActionResponse::ApplyLink {
                is_url: true,
                target: "https://example.com".into(),
                title: Some("My Site".into()),
            }
        );
        assert!(!menu.is_open());
    }

    #[test]
    fn test_menu_without_selection_n_and_u_open_dual_inputs() {
        let mut menu = TextActionOverlay::new();
        menu.open(None, None);

        menu.handle_key(Key::Char('n'));
        assert!(matches!(
            menu.mode(),
            TextActionMode::LinkInput {
                is_url: false,
                has_selection: false,
                active_field: 0,
                ..
            }
        ));

        // Tab toggles field
        menu.handle_key(Key::Tab);
        assert!(matches!(
            menu.mode(),
            TextActionMode::LinkInput {
                active_field: 1,
                ..
            }
        ));
        menu.handle_key(Key::Tab);
        assert!(matches!(
            menu.mode(),
            TextActionMode::LinkInput {
                active_field: 0,
                ..
            }
        ));

        // Set inputs
        if let TextActionMode::LinkInput {
            target_input,
            title_input,
            ..
        } = menu.mode_mut()
        {
            *target_input = "TargetNote".into();
            *title_input = "CustomTitle".into();
        }

        let action = menu.handle_key(Key::Enter);
        assert_eq!(
            action,
            TextActionResponse::ApplyLink {
                is_url: false,
                target: "TargetNote".into(),
                title: Some("CustomTitle".into()),
            }
        );
        assert!(!menu.is_open());
    }

    #[test]
    fn test_menu_color_direct_mnemonic_key() {
        let mut menu = TextActionOverlay::new();
        menu.open(Some("Sample text".into()), Some([50.0, 50.0]));
        menu.handle_key(Key::Char('h'));
        assert!(matches!(
            menu.mode(),
            TextActionMode::ColorSelect { is_highlight: true }
        ));

        let action = menu.handle_key(Key::Char('g'));
        assert!(matches!(action, TextActionResponse::ApplyHighlight { hex } if hex == "#A3BE8C"));
        assert!(!menu.is_open());
    }

    #[test]
    fn test_word_boundary_at_cursor() {
        let buffer =
            Buffer::from_text(BufferId::new(1), "Hello world, rust programming!").expect("buffer");
        let range = word_boundary_at(&buffer, 7);
        assert_eq!(range, Some(6..11));

        let space_range = word_boundary_at(&buffer, 5);
        assert_eq!(space_range, None);
    }
}
