use super::super::{AppResult, Application};
use truff_core::markdown::facts::MarkdownFacts;
use truff_store::catalog::CatalogJob;

impl Application {
    pub(super) fn query_backlinks(&mut self, argument: &str) -> AppResult<()> {
        let path = if argument.is_empty() {
            self.active_relative_path().ok_or("Open a note first")?
        } else {
            argument.to_owned()
        };
        self.submit_catalog(CatalogJob::Backlinks {
            path,
            dirty: self.dirty_texts(),
        })
    }
    pub(super) fn follow_link(&mut self, argument: &str) -> AppResult<()> {
        let target = if argument.is_empty() {
            let head = self
                .shell
                .editor_view()
                .ok_or("Open a note first")?
                .head()
                .get();
            let buffer = self.shell.editor_buffer().ok_or("Open a note first")?;
            let line_idx = buffer.line_of_byte(head);
            let line_start = buffer.byte_of_line(line_idx);
            let line_end = if line_idx + 1 < buffer.line_len() {
                buffer.byte_of_line(line_idx + 1)
            } else {
                buffer.byte_len()
            };
            let local_head = head.saturating_sub(line_start);
            let line_text = buffer.slice(line_start..line_end).to_string();
            let line_facts = MarkdownFacts::extract(&line_text)?;
            let link = line_facts
                .links
                .into_iter()
                .find(|link| link.range.start <= local_head && local_head <= link.range.end)
                .or_else(|| {
                    MarkdownFacts::extract(&buffer.as_str())
                        .ok()
                        .and_then(|facts| {
                            facts
                                .links
                                .into_iter()
                                .find(|link| link.range.start <= head && head <= link.range.end)
                        })
                })
                .ok_or("Place the cursor inside a link")?;
            match &link.heading {
                Some(heading) => format!("{}#{}", link.target, heading),
                None => link.target,
            }
        } else {
            argument.to_owned()
        };
        self.submit_catalog(CatalogJob::Follow {
            source: self.active_relative_path().ok_or("Open a note first")?,
            target,
        })
    }
}
