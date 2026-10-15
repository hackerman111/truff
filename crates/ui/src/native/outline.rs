use truff_core::{Buffer, markdown::facts::DocFacts};

pub(super) fn items(buffer: &Buffer, facts: &DocFacts) -> Vec<crate::navigator::NavigatorItem> {
    facts
        .headings
        .iter()
        .map(|heading| {
            let line = buffer.line_of_byte(heading.range.start) + 1;
            crate::navigator::NavigatorItem::new_heading(
                line as u64,
                line,
                heading.level as usize,
                heading.text.clone(),
            )
        })
        .collect()
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
        let facts = truff_core::markdown::facts::MarkdownFacts::extract(&text).unwrap();
        let actual = items(&buffer, &facts);
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
