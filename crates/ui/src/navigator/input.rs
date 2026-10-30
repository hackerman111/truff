use super::model::*;

impl NavigatorState {
    /// Handles keyboard input in a unified, deterministic manner.
    pub fn handle_key_event(&mut self, key: NavKey, ctrl: bool) -> NavigatorAction {
        let is_hub = matches!(self.mode, NavigatorMode::Outline | NavigatorMode::Links);
        if is_hub && self.hub_tasks.editing.is_some() {
            return self.hub_tasks.handle_key(key);
        }
        if matches!(key, NavKey::Tab | NavKey::BackTab) {
            let mode = if is_hub {
                if self.mode == NavigatorMode::Outline {
                    NavigatorMode::Links
                } else {
                    NavigatorMode::Outline
                }
            } else {
                self.mode.next(key == NavKey::BackTab)
            };
            return NavigatorAction::SwitchProvider { mode };
        }
        if self.actions_menu_open {
            return self.handle_actions_menu_key(key);
        }

        if self.search_mode {
            self.handle_search_mode_key(key, ctrl)
        } else if is_hub && self.hub_tasks.focus_tasks {
            if key == NavKey::Escape {
                self.close();
                NavigatorAction::Close
            } else {
                self.hub_tasks.handle_key(key)
            }
        } else if is_hub
            && !self.jump_labels_active
            && matches!(
                key,
                NavKey::Char(
                    '1' | '2' | '3' | 'a' | 'd' | 'D' | 'e' | 'x' | 'ф' | 'в' | 'В' | 'у' | 'ч'
                ) | NavKey::Delete
                    | NavKey::Backspace
            )
        {
            self.motion_parser.reset();
            self.hub_tasks.focus_tasks = true;
            self.hub_tasks.handle_key(key)
        } else {
            self.handle_normal_mode_key(key)
        }
    }

    fn handle_actions_menu_key(&mut self, key: NavKey) -> NavigatorAction {
        match key {
            NavKey::Escape => {
                self.actions_menu_open = false;
                NavigatorAction::None
            }
            NavKey::Down | NavKey::Char('j') => {
                let count = self.context_actions().len();
                if count > 0 {
                    self.actions_menu_selected = (self.actions_menu_selected + 1).min(count - 1);
                }
                NavigatorAction::None
            }
            NavKey::Up | NavKey::Char('k') => {
                self.actions_menu_selected = self.actions_menu_selected.saturating_sub(1);
                NavigatorAction::None
            }
            NavKey::Enter => {
                let actions = self.context_actions();
                if let Some(act) = actions.get(self.actions_menu_selected) {
                    let action = act.action.clone();
                    self.actions_menu_open = false;
                    action
                } else {
                    NavigatorAction::None
                }
            }
            NavKey::Char(c) => {
                let c_str = c.to_string();
                let actions = self.context_actions();
                if let Some(act) = actions.iter().find(|a| {
                    a.key == c_str
                        || (c.eq_ignore_ascii_case(&'p') && (a.key == "p" || a.key == "P"))
                }) {
                    let action = act.action.clone();
                    self.actions_menu_open = false;
                    action
                } else {
                    NavigatorAction::None
                }
            }
            _ => NavigatorAction::None,
        }
    }

    fn handle_search_mode_key(&mut self, key: NavKey, ctrl: bool) -> NavigatorAction {
        match key {
            NavKey::Escape => {
                self.search_mode = false;
                NavigatorAction::None
            }
            NavKey::Enter => {
                let action = self.primary_action();
                if action.closes_navigator() {
                    self.close();
                }
                action
            }
            NavKey::Down | NavKey::Char('j') if ctrl => {
                self.move_down();
                NavigatorAction::None
            }
            NavKey::Up | NavKey::Char('k') if ctrl => {
                self.move_up();
                NavigatorAction::None
            }
            NavKey::Down => {
                self.move_down();
                NavigatorAction::None
            }
            NavKey::Up => {
                self.move_up();
                NavigatorAction::None
            }
            NavKey::Backspace => {
                self.query.pop();
                self.recompute_filter();
                NavigatorAction::None
            }
            NavKey::Char(c) => {
                self.query.push(c);
                self.recompute_filter();
                NavigatorAction::None
            }
            _ => NavigatorAction::None,
        }
    }

    fn handle_normal_mode_key(&mut self, key: NavKey) -> NavigatorAction {
        if self.jump_labels_active {
            if key == NavKey::Escape || key == NavKey::Char('s') {
                self.jump_labels_active = false;
                return NavigatorAction::None;
            }
            if let NavKey::Char(c) = key {
                const LABELS: &[char] = &[
                    'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'q', 'w', 'e', 'r', 't', 'y', 'u',
                    'i', 'o', 'p', 'z', 'x', 'c', 'v', 'b', 'n', 'm',
                ];
                if let Some(pos) = LABELS.iter().position(|&l| l == c) {
                    let start = self.scroll.visible_range.start;
                    let target = start + pos;
                    if target < self.visible_count() {
                        self.selected_index = target;
                        self.jump_labels_active = false;
                        return NavigatorAction::None;
                    }
                }
            }
        }
        use truff_core::vim::{Command, Key, Mode, Motion, parser::ParseResult};
        let motion_key = match key {
            NavKey::Char(c @ ('0'..='9' | 'j' | 'k' | 'g' | 'G')) => Some(Key::Char(c)),
            NavKey::Down => Some(Key::Down),
            NavKey::Up => Some(Key::Up),
            NavKey::Home => {
                self.move_top();
                return NavigatorAction::None;
            }
            NavKey::End => {
                self.move_bottom();
                return NavigatorAction::None;
            }
            NavKey::PageUp | NavKey::PageDown => {
                let rows = self.scroll.page_rows();
                self.selected_index = if key == NavKey::PageDown {
                    self.selected_index
                        .saturating_add(rows)
                        .min(self.visible_count().saturating_sub(1))
                } else {
                    self.selected_index.saturating_sub(rows)
                };
                self.motion_parser.reset();
                return NavigatorAction::None;
            }
            _ => None,
        };
        if let Some(key) = motion_key {
            if let ParseResult::Complete(Command::Motion { motion, count }) =
                self.motion_parser.feed(key, Mode::Normal)
            {
                match motion {
                    Motion::Down | Motion::ScreenDown => {
                        self.selected_index = self
                            .selected_index
                            .saturating_add(count)
                            .min(self.visible_count().saturating_sub(1))
                    }
                    Motion::Up | Motion::ScreenUp => {
                        self.selected_index = self.selected_index.saturating_sub(count)
                    }
                    Motion::FirstLine => self.move_top(),
                    Motion::LastLine => self.move_bottom(),
                    Motion::Line(line) => {
                        self.selected_index = line
                            .saturating_sub(1)
                            .min(self.visible_count().saturating_sub(1))
                    }
                    _ => {}
                }
            }
            return NavigatorAction::None;
        }
        self.motion_parser.reset();
        match key {
            NavKey::Char('[' | ']')
                if matches!(self.mode, NavigatorMode::Content | NavigatorMode::Mixed) =>
            {
                NavigatorAction::SearchPage {
                    forward: key == NavKey::Char(']'),
                }
            }
            NavKey::Char('/' | 'i') => {
                self.search_mode = true;
                NavigatorAction::None
            }
            NavKey::Char('h') | NavKey::Left => {
                self.collapse_or_parent();
                NavigatorAction::None
            }
            NavKey::Char('l' | 'д') | NavKey::Right => {
                if matches!(self.mode, NavigatorMode::Outline | NavigatorMode::Links) {
                    self.hub_tasks.focus_tasks = true;
                } else {
                    self.expand_or_child();
                }
                NavigatorAction::None
            }
            NavKey::Char('p') if self.mode != NavigatorMode::Sessions => {
                self.preview_visible = !self.preview_visible;
                NavigatorAction::None
            }
            NavKey::Char(' ') => {
                self.preview_visible = !self.preview_visible;
                NavigatorAction::None
            }
            NavKey::Char('?') => {
                self.actions_menu_open = !self.actions_menu_open;
                self.actions_menu_selected = 0;
                NavigatorAction::None
            }
            NavKey::Enter => {
                let action = self.primary_action();
                if action.closes_navigator() {
                    self.close();
                }
                action
            }
            NavKey::Char('a')
                if matches!(self.mode, NavigatorMode::Files | NavigatorMode::Folders) =>
            {
                let parent_dir = self.current_parent_dir();
                NavigatorAction::CreateFile { parent_dir }
            }
            NavKey::Char('A')
                if matches!(self.mode, NavigatorMode::Files | NavigatorMode::Folders) =>
            {
                let parent_dir = self.current_parent_dir();
                NavigatorAction::CreateFolder { parent_dir }
            }
            NavKey::Char('s') => {
                self.jump_labels_active = !self.jump_labels_active;
                NavigatorAction::None
            }
            NavKey::Char('y') => self.quick_action("y"),
            NavKey::Char('r') => self.quick_action("r"),
            NavKey::Char('d') => self.quick_action("d"),
            NavKey::Char('p' | 'P' | 'o' | 'u') if self.mode == NavigatorMode::Sessions => {
                let wanted = match key {
                    NavKey::Char('p' | 'P') => "p",
                    NavKey::Char('o') => "o",
                    _ => "u",
                };
                if wanted == "u" {
                    return NavigatorAction::ReopenView;
                }
                self.context_actions()
                    .into_iter()
                    .find(|action| action.key == wanted || (wanted == "p" && action.key == "P"))
                    .map_or(NavigatorAction::None, |action| action.action)
            }
            NavKey::Escape => {
                self.close();
                NavigatorAction::Close
            }
            _ => NavigatorAction::None,
        }
    }
}
