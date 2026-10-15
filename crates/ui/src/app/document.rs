use super::Application;
use std::sync::Arc;
use truff_core::{
    BufferId,
    markdown::facts::{DocFacts, MarkdownFacts},
};

pub(super) struct DocumentFacts {
    id: BufferId,
    revision: u64,
    facts: Arc<DocFacts>,
}

impl Application {
    pub(crate) fn document_facts(&mut self) -> Option<Arc<DocFacts>> {
        let buffer = self.shell.editor_buffer()?;
        if let Some(cache) = &self.document_facts
            && cache.id == buffer.id()
            && cache.revision == buffer.revision()
        {
            return Some(cache.facts.clone());
        }
        let facts = Arc::new(MarkdownFacts::extract(&buffer.as_str()).ok()?);
        self.document_facts = Some(DocumentFacts {
            id: buffer.id(),
            revision: buffer.revision(),
            facts: facts.clone(),
        });
        Some(facts)
    }
}
