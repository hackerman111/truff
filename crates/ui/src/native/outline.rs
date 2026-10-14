use std::borrow::Cow;

use truff_core::Buffer;

pub(super) fn items(buffer: &Buffer) -> Vec<crate::navigator::NavigatorItem> {
    let mut items = Vec::new();
    for (line_idx, line) in buffer.rope().lines().enumerate() {
        let mut chunks = line.chunks();
        let Some(first) = chunks.next() else {
            continue;
        };
        let line_text = if chunks.next().is_none() {
            Cow::Borrowed(first)
        } else {
            Cow::Owned(line.to_string())
        };
        let trimmed = line_text.trim_start();
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            let heading_text = trimmed[level..].trim();
            if !heading_text.is_empty() && level <= 6 {
                items.push(crate::navigator::NavigatorItem::new_heading(
                    line_idx as u64 + 1,
                    line_idx + 1,
                    level,
                    heading_text.to_string(),
                ));
            }
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::items;
    use crate::navigator::NavigatorItemPayload;
    use truff_core::{Buffer, text::BufferId};

    #[test]
    fn rope_outline_matches_string_lines() {
        let text = format!(
            "# Привет\r\n####### пропуск\n# {}\n## конец",
            "я".repeat(100_000)
        );
        let buffer = Buffer::from_text(BufferId(1), &text).unwrap();
        let actual = items(&buffer);
        let expected: Vec<_> = text
            .lines()
            .enumerate()
            .filter_map(|(line_idx, line)| {
                let trimmed = line.trim_start();
                let level = trimmed.chars().take_while(|&c| c == '#').count();
                let heading = trimmed[level..].trim();
                (!heading.is_empty() && level > 0 && level <= 6).then(|| {
                    (
                        line_idx as u64 + 1,
                        line_idx + 1,
                        level,
                        heading.to_string(),
                    )
                })
            })
            .collect();
        let actual: Vec<_> = actual
            .into_iter()
            .map(|item| match item.payload {
                NavigatorItemPayload::Heading { line, level, text } => (item.id, line, level, text),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(actual, expected);
    }
}
