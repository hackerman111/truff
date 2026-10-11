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
            let facts = MarkdownFacts::extract(&self.shell.editor_text())?;
            let link = facts
                .links
                .iter()
                .find(|link| link.range.start <= head && head <= link.range.end)
                .ok_or("Place the cursor inside a link")?;
            match &link.heading {
                Some(heading) => format!("{}#{}", link.target, heading),
                None => link.target.clone(),
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
