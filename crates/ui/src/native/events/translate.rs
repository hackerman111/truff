use super::super::NativeApp;
use super::{FieldOwner, input};
use crate::editor::EditorInput;
use crate::shell::{Focus, ShellAction};
use egui::Event;
use truff_core::vim::{Key, Mode};

impl NativeApp {
    pub(in crate::native) fn flush_field_keys(&mut self, context: &egui::Context) {
        for (owner, key, backwards) in std::mem::take(&mut self.field_keys) {
            let active = match owner {
                FieldOwner::Navigator => self.navigator.is_open() && !self.dialogs.is_any_open(),
                FieldOwner::Command => self.application.shell.command().is_some(),
                FieldOwner::Help => self.application.shell.which_key().is_visible(),
                FieldOwner::TextAction => self.application.shell.text_action().is_open(),
                FieldOwner::Toc => self.toc.is_open && self.toc.model.search_active,
                FieldOwner::Wiki => self.wiki.is_open() && self.wiki.data.search_active,
                FieldOwner::Stream => self.stream.is_open() && self.stream.model.search_active,
            };
            if !active {
                continue;
            }
            match owner {
                FieldOwner::Toc => match key {
                    Key::Esc => {
                        self.toc.model.search_active = false;
                        self.toc.model.search_query.clear();
                        self.toc.model.refresh_visible();
                    }
                    Key::Enter => self.toc.model.search_active = false,
                    Key::Up => self.toc.model.move_up(1),
                    Key::Down => self.toc.model.move_down(1),
                    _ => {}
                },
                FieldOwner::Wiki => match key {
                    Key::Esc => {
                        self.wiki.data.search_active = false;
                        self.wiki.data.search_query.clear();
                    }
                    Key::Enter => self.wiki.data.search_active = false,
                    Key::Up => self.wiki.data.move_up(1),
                    Key::Down => self.wiki.data.move_down(1),
                    _ => {}
                },
                FieldOwner::Stream => match key {
                    Key::Esc => {
                        self.stream.model.search_active = false;
                        self.stream.model.search_query.clear();
                        self.stream.model.clamp_selection();
                    }
                    Key::Enter => self.stream.model.search_active = false,
                    Key::Up => self.stream.model.move_up(1),
                    Key::Down => self.stream.model.move_down(1),
                    _ => {}
                },
                FieldOwner::Navigator if key == Key::Tab && backwards => {
                    let action = self
                        .navigator
                        .handle_key_event(crate::navigator::NavKey::BackTab, false);
                    self.handle_navigator_action(action);
                }
                _ => self.key(key),
            }
            context.request_repaint();
        }
    }

    pub(in crate::native) fn events(&mut self, context: &egui::Context) {
        if !self.field_event_tail.is_empty() {
            let mut tail = std::mem::take(&mut self.field_event_tail);
            context.input_mut(|input| {
                tail.append(&mut input.events);
                input.events = tail;
            });
        }
        if self.dialogs.is_any_open() {
            return;
        }
        let mut help_owned = self.application.shell.which_key().is_visible()
            && self.application.shell.which_key().mode() == crate::overlays::WhichKeyMode::Help;
        let mut navigator_owned = self.navigator.is_open();
        let text_action_owned = self.application.shell.text_action().is_open();
        let panel_search_owned = (self.toc.is_open && self.toc.model.search_active)
            || (self.wiki.is_open() && self.wiki.data.search_active)
            || (self.stream.is_open() && self.stream.model.search_active);
        if !help_owned
            && !navigator_owned
            && !text_action_owned
            && !panel_search_owned
            && self.application.shell.command().is_none()
            && context.egui_wants_keyboard_input()
        {
            // The only egui text fields live in these overlays; a closed field can retain focus for one frame.
            context.memory_mut(|memory| {
                if let Some(id) = memory.focused() {
                    memory.surrender_focus(id);
                }
            });
        }
        // Text events are authoritative for printable characters; OS Key events would duplicate them.
        let mut events = context
            .input_mut(|input| std::mem::take(&mut input.events))
            .into_iter();
        let mut field_events = Vec::new();
        while let Some(event) = events.next() {
            if self.dialogs.is_any_open() {
                field_events.push(event);
                field_events.extend(events);
                break;
            }
            let help = self.application.shell.which_key().is_visible()
                && self.application.shell.which_key().mode() == crate::overlays::WhichKeyMode::Help;
            if self.application.file_operation_pending()
                || self.dialogs.is_any_open()
                || self.application.settings_requested
                || (navigator_owned && !self.navigator.is_open())
                || (help_owned && !help)
            {
                break;
            }
            help_owned |= help;
            navigator_owned |= self.navigator.is_open();
            let is_text_action_input = self.application.shell.text_action().is_open()
                && matches!(
                    self.application.shell.text_action().mode(),
                    crate::overlays::text_action::TextActionMode::LinkInput { .. }
                );
            let field_owner = if self.navigator.is_open() && self.navigator.search_mode {
                Some(FieldOwner::Navigator)
            } else if self
                .application
                .shell
                .command_input()
                .is_some_and(|input| input.inserting)
            {
                Some(FieldOwner::Command)
            } else if help && self.application.shell.which_key().help.inserting {
                Some(FieldOwner::Help)
            } else if is_text_action_input {
                Some(FieldOwner::TextAction)
            } else if self.wiki.is_open() && self.wiki.data.search_active {
                Some(FieldOwner::Wiki)
            } else if self.stream.is_open() && self.stream.model.search_active {
                Some(FieldOwner::Stream)
            } else if self.toc.is_open && self.toc.model.search_active {
                Some(FieldOwner::Toc)
            } else {
                None
            };
            if let Some(owner) = field_owner {
                if let Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = &event
                    && matches!(
                        key,
                        egui::Key::Escape
                            | egui::Key::Enter
                            | egui::Key::Tab
                            | egui::Key::ArrowUp
                            | egui::Key::ArrowDown
                    )
                {
                    let key = match key {
                        egui::Key::Escape => Key::Esc,
                        egui::Key::Enter => Key::Enter,
                        egui::Key::Tab if modifiers.shift => Key::ShiftTab,
                        egui::Key::Tab => Key::Tab,
                        egui::Key::ArrowUp => Key::Up,
                        _ => Key::Down,
                    };
                    let backwards = owner == FieldOwner::Navigator
                        && matches!(key, Key::Tab | Key::ShiftTab)
                        && modifiers.shift;
                    self.field_keys.push((owner, key, backwards));
                    self.field_event_tail.extend(events);
                    break;
                }
                if matches!(
                    event,
                    Event::Text(_)
                        | Event::Paste(_)
                        | Event::Ime(_)
                        | Event::Key { .. }
                        | Event::Copy
                        | Event::Cut
                ) {
                    field_events.push(event);
                    continue;
                }
            }
            if help
                && self.application.shell.which_key().help.inserting
                && !matches!(
                    event,
                    Event::Key {
                        key: egui::Key::Escape,
                        pressed: true,
                        ..
                    }
                )
            {
                field_events.push(event);
                continue;
            }
            match event {
                Event::Text(text) => {
                    if help || self.navigator.is_open() {
                        for (offset, character) in text.char_indices() {
                            let inserting = self.navigator.search_mode
                                || (help && self.application.shell.which_key().help.inserting);
                            if inserting {
                                field_events.push(Event::Text(text[offset..].to_owned()));
                                break;
                            }
                            self.key(Key::Char(character));
                            if self.dialogs.is_any_open() {
                                let next = offset + character.len_utf8();
                                if next < text.len() {
                                    field_events.push(Event::Text(text[next..].to_owned()));
                                }
                                break;
                            }
                            if self.navigator.is_open() {
                                navigator_owned = true;
                            }
                            if navigator_owned && !self.navigator.is_open() {
                                break;
                            }
                        }
                        continue;
                    }
                    if self.application.shell.mode() == Mode::Insert
                        && self.application.shell.command().is_none()
                    {
                        if self.application.shell.focus() != Focus::Editor {
                            self.application.shell.set_focus(Focus::Editor);
                        }
                        input(&mut self.application, EditorInput::InsertText(text));
                    } else {
                        for (offset, character) in text.char_indices() {
                            if self.navigator.search_mode || self.dialogs.is_any_open() {
                                field_events.push(Event::Text(text[offset..].to_owned()));
                                break;
                            }
                            navigator_owned |= self.navigator.is_open();
                            self.key(Key::Char(character));
                            navigator_owned |= self.navigator.is_open();
                            if navigator_owned && !self.navigator.is_open() {
                                break;
                            }
                        }
                    }
                }
                Event::Paste(text) => {
                    if help || self.navigator.is_open() {
                        if help && self.application.shell.which_key().help.inserting {
                            field_events.push(Event::Paste(text));
                        }
                        continue;
                    }
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
                }) if !help && self.application.shell.mode() == Mode::Insert => {
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
                    if !help && self.application.shell.mode() == Mode::Insert =>
                {
                    input(&mut self.application, EditorInput::ImeCommit(text));
                }
                Event::Ime(egui::ImeEvent::DeleteSurrounding {
                    before_chars,
                    after_chars,
                }) if !help && self.application.shell.mode() == Mode::Insert => {
                    input(
                        &mut self.application,
                        EditorInput::ImeDeleteSurrounding {
                            before_chars,
                            after_chars,
                        },
                    );
                }
                Event::Copy | Event::Cut => {
                    if help
                        || self.navigator.is_open()
                        || self.application.shell.focus() != Focus::Editor
                    {
                        continue;
                    }
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
                    if key == egui::Key::Tab && self.navigator.is_open() {
                        let action = self.navigator.handle_key_event(
                            if modifiers.shift {
                                crate::navigator::NavKey::BackTab
                            } else {
                                crate::navigator::NavKey::Tab
                            },
                            false,
                        );
                        self.handle_navigator_action(action);
                        continue;
                    }
                    let Some(mapped) = map_egui_key(key, modifiers) else {
                        continue;
                    };
                    self.key(mapped);
                }
                event => field_events.push(event),
            }
        }
        context.input_mut(|input| input.events = field_events);
        if let Some(yanked) = self.application.shell.take_last_yank() {
            context.copy_text(yanked);
        }
    }
}

fn map_egui_key(key: egui::Key, modifiers: egui::Modifiers) -> Option<Key> {
    Some(match key {
        egui::Key::Escape => Key::Esc,
        egui::Key::Enter if modifiers.shift => Key::ShiftEnter,
        egui::Key::Enter if modifiers.command || modifiers.ctrl => Key::CtrlEnter,
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
        egui::Key::Tab if modifiers.shift => Key::ShiftTab,
        egui::Key::Tab => Key::Tab,
        value if modifiers.command || modifiers.ctrl || modifiers.alt => {
            let mut characters = value.name().chars();
            let character = characters.next()?;
            if characters.next().is_some() {
                return None;
            }
            crate::driver::map_key_code(
                character.to_ascii_lowercase(),
                modifiers.command || modifiers.ctrl,
                modifiers.alt,
            )
        }
        _ => return None,
    })
}
