use std::borrow::Cow;

use truff_core::commands::CommandId;
use truff_core::text::BufferId;

use super::types::NavigatorItemPayload;

/// A single selectable item in the Universal Navigator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigatorItem {
    pub id: u64,
    pub title: Box<str>,
    pub subtitle: Option<Box<str>>,
    pub category: Option<Cow<'static, str>>,
    pub shortcut: Option<Box<str>>,
    pub is_folder: bool,
    pub is_collapsed: bool,
    pub depth: usize,
    pub preview: Option<Box<str>>,
    pub disabled_reason: Option<Box<str>>,
    pub payload: NavigatorItemPayload,
    pub char_mask: u64,
}

#[inline]
pub fn compute_char_mask(s: &str) -> u64 {
    if s.is_ascii() {
        let mut mask = 0u64;
        for &b in s.as_bytes() {
            let lower = b.to_ascii_lowercase();
            let bit = match lower {
                b'a'..=b'z' => (lower - b'a') as u32,
                b'0'..=b'9' => 26 + (lower - b'0') as u32,
                b'_' => 36,
                b'-' => 37,
                b'/' | b'\\' => 38,
                b'.' => 39,
                b' ' => 40,
                _ => continue,
            };
            mask |= 1u64 << bit;
        }
        return mask;
    }

    let mut mask = 0u64;
    for c in s.chars() {
        if c.is_ascii() {
            let lower = (c as u8).to_ascii_lowercase();
            let bit = match lower {
                b'a'..=b'z' => (lower - b'a') as u32,
                b'0'..=b'9' => 26 + (lower - b'0') as u32,
                b'_' => 36,
                b'-' => 37,
                b'/' | b'\\' => 38,
                b'.' => 39,
                b' ' => 40,
                _ => continue,
            };
            mask |= 1u64 << bit;
            continue;
        }
        for lower_c in c.to_lowercase() {
            let bit = match lower_c {
                'а'..='я' => 41 + (((lower_c as u32) - ('а' as u32)) % 22),
                'ё' => 63,
                _ => continue,
            };
            mask |= 1u64 << bit;
        }
    }
    mask
}

#[inline]
fn category_char_mask(cat: &str) -> u64 {
    const FILES_MASK: u64 = (1u64 << (41 + ((('ф' as u32) - ('а' as u32)) % 22)))
        | (1u64 << 41)
        | (1u64 << (41 + ((('й' as u32) - ('а' as u32)) % 22)))
        | (1u64 << (41 + ((('л' as u32) - ('а' as u32)) % 22)))
        | (1u64 << (41 + ((('ы' as u32) - ('а' as u32)) % 22)));
    const FOLDER_MASK: u64 = (1u64 << (41 + ((('п' as u32) - ('а' as u32)) % 22)))
        | (1u64 << 41)
        | (1u64 << (41 + ((('к' as u32) - ('а' as u32)) % 22)));
    match cat {
        "Файлы" => FILES_MASK,
        "Папка" => FOLDER_MASK,
        _ => compute_char_mask(cat),
    }
}

fn file_name_start(path: &str) -> usize {
    // Separators are ASCII bytes, so the result is always a UTF-8 boundary.
    path.as_bytes()
        .iter()
        .rposition(|&byte| byte == b'/' || byte == b'\\')
        .map_or(0, |index| index + 1)
}

impl NavigatorItem {
    pub fn update_char_mask(&mut self) {
        let mut mask = if self.title.is_empty()
            && self.subtitle.is_none()
            && let NavigatorItemPayload::File { path, .. } = &self.payload
        {
            compute_char_mask(path)
        } else {
            let mut m = compute_char_mask(self.display_title());
            if let Some(sub) = self.display_subtitle() {
                m |= category_char_mask(sub);
            }
            m
        };
        if let Some(cat) = self.category.as_deref() {
            mask |= category_char_mask(cat);
        }
        self.char_mask = mask;
    }

    pub fn new_file(
        id: u64,
        name: &str,
        relative_path: String,
        is_dir: bool,
        depth: usize,
    ) -> Self {
        let basename = &relative_path[file_name_start(&relative_path)..];
        let title = if name == basename {
            Box::default()
        } else {
            name.into()
        };

        let mut item = Self {
            id,
            title,
            subtitle: None,
            category: Some(Cow::Borrowed("Файлы")),
            shortcut: None,
            is_folder: is_dir,
            is_collapsed: false,
            depth,
            preview: None,
            disabled_reason: None,
            payload: NavigatorItemPayload::File {
                path: relative_path,
                is_dir,
            },
            char_mask: 0,
        };
        item.update_char_mask();
        item
    }

    pub fn display_title(&self) -> &str {
        if !self.title.is_empty() {
            return &self.title;
        }
        match &self.payload {
            NavigatorItemPayload::File { path, .. }
            | NavigatorItemPayload::Location { path, .. } => &path[file_name_start(path)..],
            NavigatorItemPayload::Vault { path } => path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Vault"),
            NavigatorItemPayload::Link { target } => target,
            _ => &self.title,
        }
    }

    pub fn display_subtitle(&self) -> Option<&str> {
        if self.subtitle.is_some() {
            return self.subtitle.as_deref();
        }
        match &self.payload {
            NavigatorItemPayload::File { is_dir: true, .. } => Some("Папка"),
            NavigatorItemPayload::File { path, .. } => file_name_start(path)
                .checked_sub(1)
                .map(|index| &path[..index])
                .filter(|parent| !parent.is_empty()),
            NavigatorItemPayload::Command { .. } => self.preview.as_deref(),
            _ => None,
        }
    }

    pub fn new_buffer(
        id: u64,
        buffer_id: BufferId,
        title: String,
        path: Option<String>,
        is_dirty: bool,
        line_count: usize,
    ) -> Self {
        let mut subtitle = format!("{line_count} строк");
        if is_dirty {
            subtitle.push_str(" · Не сохранено");
        }
        if let Some(p) = &path {
            subtitle.push_str(&format!(" · {p}"));
        }

        let mut item = Self {
            id,
            title: title.into_boxed_str(),
            subtitle: Some(subtitle.into_boxed_str()),
            category: Some(Cow::Borrowed("Буферы")),
            shortcut: None,
            is_folder: false,
            is_collapsed: false,
            depth: 0,
            preview: None,
            disabled_reason: None,
            payload: NavigatorItemPayload::Buffer { buffer_id, path },
            char_mask: 0,
        };
        item.update_char_mask();
        item
    }

    pub fn new_link(id: u64, title: &str, target: String) -> Self {
        let mut item = Self {
            id,
            title: title.into(),
            subtitle: None,
            category: Some(Cow::Borrowed("Исходящие")),
            shortcut: None,
            is_folder: false,
            is_collapsed: false,
            depth: 0,
            preview: Some(target.clone().into_boxed_str()),
            disabled_reason: None,
            payload: NavigatorItemPayload::Link { target },
            char_mask: 0,
        };
        item.update_char_mask();
        item
    }

    pub fn new_command(
        id: u64,
        command_id: CommandId,
        title: String,
        description: String,
        category: Cow<'static, str>,
        shortcut: Option<String>,
    ) -> Self {
        let mut item = Self {
            id,
            title: title.into_boxed_str(),
            subtitle: None,
            category: Some(category),
            shortcut: shortcut.map(String::into_boxed_str),
            is_folder: false,
            is_collapsed: false,
            depth: 0,
            preview: Some(description.into_boxed_str()),
            disabled_reason: None,
            payload: NavigatorItemPayload::Command { command_id },
            char_mask: 0,
        };
        item.update_char_mask();
        item
    }

    pub fn new_heading(id: u64, line: usize, level: usize, text: String) -> Self {
        let prefix = "#".repeat(level);
        let title = format!("{prefix} {text}");
        let mut item = Self {
            id,
            title: title.into_boxed_str(),
            subtitle: Some(format!("Строка {line}").into_boxed_str()),
            category: Some(Cow::Borrowed("Заголовки")),
            shortcut: None,
            is_folder: false,
            is_collapsed: false,
            depth: level.saturating_sub(1),
            preview: Some(
                format!("Заголовок уровня {level}: {text}\nСтрока: {line}").into_boxed_str(),
            ),
            disabled_reason: None,
            payload: NavigatorItemPayload::Heading { line, level, text },
            char_mask: 0,
        };
        item.update_char_mask();
        item
    }

    pub fn new_task(id: u64, line: usize, checked: bool, text: String) -> Self {
        let state = if checked {
            "Выполнено"
        } else {
            "Не выполнено"
        };
        let mut item = Self {
            id,
            title: text.into_boxed_str(),
            subtitle: Some(state.into()),
            category: Some(Cow::Borrowed("Задачи")),
            shortcut: None,
            is_folder: false,
            is_collapsed: false,
            depth: 0,
            preview: Some(
                format!("{state}\nСтрока: {line}\nEnter — перейти к задаче").into_boxed_str(),
            ),
            disabled_reason: None,
            payload: NavigatorItemPayload::Task { line, checked },
            char_mask: 0,
        };
        item.update_char_mask();
        item
    }
}
