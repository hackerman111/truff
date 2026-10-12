use super::NativeApp;
use crate::editor::EditorInput;
use crate::shell::{Focus, ShellAction};
use eframe::egui::{self, Event};
use truff_core::vim::{Key, Mode};

impl NativeApp {
    fn key(&mut self, key: Key) {
        if self.application.file_operation_pending() {
            return;
        }
        let shell = &self.application.shell;
        if shell.command().is_none() && !shell.flash().is_active() {
            match key {
                Key::Char('s')
                    if shell.focus() == Focus::Editor && shell.mode() == Mode::Normal =>
                {
                    if let Err(error) = self.application.shell.start_flash_ranges(
                        &self.editor.visible_source_windows(),
                        crate::overlays::FlashKind::Word,
                    ) {
                        self.application.status = error.to_string();
                    }
                    return;
                }
                Key::Char('j') | Key::Down if shell.focus() == Focus::RightPanel => {
                    self.application.backlink_selection = (self.application.backlink_selection + 1)
                        .min(self.application.backlinks.len().saturating_sub(1));
                    return;
                }
                Key::Char('k') | Key::Up if shell.focus() == Focus::RightPanel => {
                    self.application.backlink_selection =
                        self.application.backlink_selection.saturating_sub(1);
                    return;
                }
                Key::Enter if shell.focus() == Focus::RightPanel => {
                    if let Err(error) = self.application.open_backlink() {
                        self.application.status = error.to_string();
                    }
                    return;
                }
                Key::PageUp | Key::PageDown => {
                    self.editor
                        .scroll_by(if key == Key::PageUp { -400.0 } else { 400.0 });
                    return;
                }
                _ => {}
            }
        }
        let action = self.application.shell.feed_key(key);
        self.action(action);
    }

    fn action(&mut self, action: ShellAction) {
        match action {
            ShellAction::VisualRow { down, count } => {
                let target = self
                    .application
                    .shell
                    .editor_buffer()
                    .zip(self.application.shell.editor_view())
                    .and_then(|(buffer, view)| {
                        self.editor.visual_target(buffer, view, down, count)
                    });
                if let Some(target) = target
                    && let Err(error) = self.application.shell.apply_visual_row(target)
                {
                    self.application.status = error.to_string();
                }
            }
            ShellAction::RegisteredCommand { id, argument: _ }
                if id.as_str().starts_with("flash.") =>
            {
                use crate::overlays::FlashKind;
                let kind = match id.as_str() {
                    "flash.jump_word" => FlashKind::Word,
                    "flash.select_sentence" => FlashKind::Sentence,
                    "flash.select_paragraph" => FlashKind::Paragraph,
                    _ => FlashKind::SyntaxObject,
                };
                if let Err(error) = self
                    .application
                    .shell
                    .start_flash_ranges(&self.editor.visible_source_windows(), kind)
                {
                    self.application.status = error.to_string();
                }
            }
            ShellAction::OpenNavigator(mode) => {
                self.open_navigator(mode);
            }
            action => self.application.handle_shell_action(action),
        }
    }

    pub(super) fn events(&mut self, context: &egui::Context) {
        if self.dialogs.is_any_open()
            || self.navigator.is_open()
            || context.egui_wants_keyboard_input()
        {
            return;
        }
        // Text events are authoritative for printable characters; OS Key events would duplicate them.
        let events = context.input(|input| input.events.clone());
        for event in events {
            if self.application.file_operation_pending() {
                continue;
            }
            match event {
                Event::Text(text) => {
                    if self.application.shell.mode() == Mode::Insert
                        && self.application.shell.command().is_none()
                    {
                        if self.application.shell.focus() != Focus::Editor {
                            self.application.shell.set_focus(Focus::Editor);
                        }
                        input(&mut self.application, EditorInput::InsertText(text));
                    } else {
                        for character in text.chars() {
                            self.key(Key::Char(character));
                        }
                    }
                }
                Event::Paste(text) => {
                    if self.application.shell.command().is_some() {
                        for character in text.chars().filter(|ch| !ch.is_control()) {
                            self.key(Key::Char(character));
                        }
                    } else {
                        input(&mut self.application, EditorInput::Paste(text));
                    }
                }
                Event::Ime(egui::ImeEvent::Preedit {
                    text,
                    active_range_chars,
                }) if self.application.shell.mode() == Mode::Insert => {
                    let cursor = active_range_chars.map(|range| {
                        let byte = |index| {
                            text.char_indices()
                                .nth(index)
                                .map_or(text.len(), |(offset, _)| offset)
                        };
                        (byte(range.start), byte(range.end))
                    });
                    input(
                        &mut self.application,
                        EditorInput::ImePreedit { text, cursor },
                    );
                }
                Event::Ime(egui::ImeEvent::Commit(text))
                    if self.application.shell.mode() == Mode::Insert =>
                {
                    input(&mut self.application, EditorInput::ImeCommit(text))
                }
                Event::Ime(egui::ImeEvent::DeleteSurrounding {
                    before_chars,
                    after_chars,
                }) if self.application.shell.mode() == Mode::Insert => {
                    input(
                        &mut self.application,
                        EditorInput::ImeDeleteSurrounding {
                            before_chars,
                            after_chars,
                        },
                    );
                }
                Event::Copy | Event::Cut => {
                    if let Ok(range) = self.application.selection_range()
                        && let Some(buffer) = self.application.shell.editor_buffer()
                    {
                        context.copy_text(buffer.slice(range.clone()).to_string());
                        if matches!(event, Event::Cut) {
                            if let Err(error) = self
                                .application
                                .shell
                                .apply_transaction(truff_core::text::Transaction::delete(range))
                            {
                                self.application.status = error.to_string();
                            }
                            self.application
                                .handle_shell_action(ShellAction::EditorEdit);
                        }
                    }
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    let key = match key {
                        egui::Key::Escape => Key::Esc,
                        egui::Key::Enter => Key::Enter,
                        egui::Key::Backspace => Key::Backspace,
                        egui::Key::Delete => Key::Delete,
                        egui::Key::ArrowLeft => Key::Left,
                        egui::Key::ArrowRight => Key::Right,
                        egui::Key::ArrowUp => Key::Up,
                        egui::Key::ArrowDown => Key::Down,
                        egui::Key::Home => Key::Home,
                        egui::Key::End => Key::End,
                        egui::Key::PageUp => Key::PageUp,
                        egui::Key::PageDown => Key::PageDown,
                        egui::Key::Tab => Key::Tab,
                        value if modifiers.command || modifiers.ctrl || modifiers.alt => {
                            let name = value.name();
                            let mut characters = name.chars();
                            let Some(character) = characters.next() else {
                                continue;
                            };
                            if characters.next().is_some() {
                                continue;
                            }
                            crate::driver::map_key_code(
                                character.to_ascii_lowercase(),
                                modifiers.command || modifiers.ctrl,
                                modifiers.alt,
                            )
                        }
                        _ => continue,
                    };
                    self.key(key);
                }
                _ => {}
            }
        }
    }
}

pub(super) fn input(application: &mut crate::app::Application, input: EditorInput) {
    if application.shell.mode() == Mode::Insert && application.shell.focus() != Focus::Editor {
        application.shell.set_focus(Focus::Editor);
    }
    if application.shell.focus() != Focus::Editor || application.file_operation_pending() {
        return;
    }
    if let Err(error) = application.shell.apply_input(input) {
        application.status = error.to_string();
    }
    application.handle_shell_action(ShellAction::EditorEdit);
}
