use super::NativeApp;
use crate::editor::EditorInput;
use crate::shell::{Focus, ShellAction};
use egui::Event;
use truff_core::vim::{Key, Mode};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum FieldOwner {
    Command,
    Navigator,
    Help,
}

impl NativeApp {
    fn key(&mut self, key: Key) {
        if self.dialogs.is_any_open() {
            return;
        }
        if self.navigator.is_open() {
            if let Some(key) = crate::navigator::model::NavKey::from_core(key) {
                let action = self.navigator.handle_key_event(key, false);
                self.handle_navigator_action(action);
            }
            return;
        }
        if self.wiki.is_open() {
            let action = self.wiki.feed_key(key);
            self.handle_wiki_action(action);
            return;
        }
        if self.stream.is_open() {
            let action = self.stream.feed_key(key);
            self.handle_stream_action(action);
            return;
        }
        if self.toc.is_open && !self.toc.is_pinned {
            let action = self.toc.feed_key(key);
            self.handle_toc_action(action);
            return;
        }
        if self.application.file_operation_pending()
            || self.dialogs.is_any_open()
            || self.navigator.is_open()
            || self.application.settings_requested
        {
            return;
        }
        if self.application.shell.focus() == Focus::Editor
            && self.application.shell.mode() == Mode::Insert
            && self.application.link_completion.active
        {
            match key {
                Key::Down | Key::Ctrl('n') => {
                    self.application.link_completion.next();
                    return;
                }
                Key::Up | Key::Ctrl('p') => {
                    self.application.link_completion.prev();
                    return;
                }
                Key::Enter | Key::Tab => {
                    if let Err(error) = self.application.apply_link_completion() {
                        self.application.status = error.to_string();
                    }
                    return;
                }
                Key::Esc => {
                    self.application.link_completion.dismiss();
                    return;
                }
                _ => {}
            }
        }
        let shell = &self.application.shell;
        if shell.command().is_none()
            && shell.search_query().is_none()
            && !shell.flash().is_active()
            && !shell.which_key().is_visible()
            && shell.which_key().hints().is_empty()
        {
            if shell.focus() == Focus::Editor
                && shell.mode() == Mode::Normal
                && let Some(id) = shell.single_binding(key)
                && id.as_str().starts_with("flash.")
            {
                let action = self.application.shell.execute_registered(id, String::new());
                self.action(action);
                return;
            }
            match key {
                Key::Enter if shell.focus() == Focus::RightPanel => {
                    if let Err(error) = self.application.open_backlink() {
                        self.application.status = error.to_string();
                    }
                    return;
                }
                Key::PageUp | Key::PageDown | Key::Ctrl('d' | 'u')
                    if shell.focus() == Focus::Editor =>
                {
                    let target =
                        shell
                            .editor_buffer()
                            .zip(shell.editor_view())
                            .map(|(buffer, view)| {
                                self.editor.page_target(
                                    buffer,
                                    view,
                                    matches!(key, Key::PageDown | Key::Ctrl('d')),
                                    matches!(key, Key::Ctrl(_)),
                                )
                            });
                    if let Some(target) = target
                        && let Err(error) = self.application.shell.apply_visual_row(target)
                    {
                        self.application.status = error.to_string();
                    }
                    return;
                }
                _ => {}
            }
        }
        let prev_mode = self.application.shell.mode();
        let action = self.application.shell.feed_key(key);
        self.action(action);
        let new_mode = self.application.shell.mode();
        if prev_mode == Mode::Insert
            && new_mode == Mode::Normal
            && self.application.active_path().is_some()
            && self.application.has_any_dirty_buffer()
        {
            let _ = self.application.save(false);
        }
        if let Some((buffer, view)) = self
            .application
            .shell
            .editor_buffer()
            .zip(self.application.shell.editor_view())
        {
            if self.application.shell.mode() == Mode::Insert {
                if let Some((trigger, query)) =
                    crate::overlays::LinkCompletionOverlay::detect_trigger(
                        buffer,
                        view.head().get(),
                    )
                {
                    self.application.refresh_link_completions(trigger, &query);
                } else {
                    self.application.link_completion.dismiss();
                }
            } else {
                self.application.link_completion.dismiss();
            }
        }
    }

    pub(super) fn action(&mut self, action: ShellAction) {
        match action {
            ShellAction::FocusChanged(focus) => {
                if self.application.focus_mode {
                    self.application.shell.set_focus(Focus::Editor);
                } else if focus == Focus::Sidebar
                    && let Some(path) = self.application.active_relative_path()
                {
                    self.application.shell.select_sidebar_file(&path);
                }
            }
            ShellAction::ScrollCursor(pos) => {
                self.editor.scroll_cursor(pos);
            }
            ShellAction::EditorEdit if self.application.shell.focus() == Focus::RightPanel => {}
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
                if id.as_str() == "flash.jump_word" {
                    if let Err(error) = self
                        .application
                        .shell
                        .start_flash_search_ranges(&self.editor.visible_source_windows())
                    {
                        self.application.status = error.to_string();
                    }
                } else {
                    let kind = match id.as_str() {
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
            }
            ShellAction::RegisteredCommand { id, argument }
                if id.as_str() == "note.link_selection" && argument.is_empty() =>
            {
                match self.application.capture_selection() {
                    Ok(target) => self.dialogs.operation.link(target),
                    Err(error) => self.application.status = error.to_string(),
                }
            }
            ShellAction::SidebarDeleteFile(path) => self.dialogs.operation.trash(path),
            ShellAction::OpenNavigator(mode) => {
                self.open_navigator(mode);
            }
            ShellAction::RegisteredCommand { id, argument } => {
                if !self.file_command(id.as_str(), &argument) {
                    self.application
                        .handle_shell_action(ShellAction::RegisteredCommand { id, argument });
                }
            }
            ShellAction::Command(command) => {
                let (name, argument) = crate::app::command_parts(&command);
                if name == "trash" {
                    self.file_command("file.trash", argument);
                } else {
                    self.application
                        .handle_shell_action(ShellAction::Command(command));
                }
            }
            action => self.application.handle_shell_action(action),
        }
    }

    pub(super) fn flush_field_keys(&mut self, context: &egui::Context) {
        for (owner, key, backwards) in std::mem::take(&mut self.field_keys) {
            let active = match owner {
                FieldOwner::Navigator => self.navigator.is_open() && !self.dialogs.is_any_open(),
                FieldOwner::Command => self.application.shell.command().is_some(),
                FieldOwner::Help => self.application.shell.which_key().is_visible(),
            };
            if !active {
                continue;
            }
            if owner == FieldOwner::Navigator && key == Key::Tab && backwards {
                let action = self
                    .navigator
                    .handle_key_event(crate::navigator::NavKey::BackTab, false);
                self.handle_navigator_action(action);
            } else {
                self.key(key);
            }
            context.request_repaint();
        }
    }

    pub(super) fn events(&mut self, context: &egui::Context) {
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
        if !help_owned
            && !navigator_owned
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
                        egui::Key::Tab => Key::Tab,
                        egui::Key::ArrowUp => Key::Up,
                        _ => Key::Down,
                    };
                    if owner == FieldOwner::Navigator && key == Key::Tab && modifiers.shift {
                        self.field_keys.push((owner, key, true));
                    } else {
                        self.field_keys.push((owner, key, false));
                    }
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
                    input(&mut self.application, EditorInput::ImeCommit(text))
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
                    let key = match key {
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
                event => field_events.push(event),
            }
        }
        context.input_mut(|input| input.events = field_events);
        if let Some(yanked) = self.application.shell.take_last_yank() {
            context.copy_text(yanked);
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
    if let Some((buffer, view)) = application
        .shell
        .editor_buffer()
        .zip(application.shell.editor_view())
    {
        if application.shell.mode() == Mode::Insert {
            if let Some((trigger, query)) =
                crate::overlays::LinkCompletionOverlay::detect_trigger(buffer, view.head().get())
            {
                application.refresh_link_completions(trigger, &query);
            } else {
                application.link_completion.dismiss();
            }
        } else {
            application.link_completion.dismiss();
        }
    }
    application.handle_shell_action(ShellAction::EditorEdit);
}
