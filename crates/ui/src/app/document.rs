use super::Application;
use std::sync::Arc;
use truff_core::{
    Buffer, BufferId,
    markdown::facts::{DocFacts, MarkdownFacts},
    workspace::BufferChange,
};

pub(super) struct DocumentFacts {
    id: BufferId,
    revision: u64,
    facts: Arc<DocFacts>,
}

impl DocumentFacts {
    pub(crate) fn notify_buffer_change(&mut self, buffer: &Buffer, change: &BufferChange) {
        if self.id != buffer.id() || change.invalidate_suffix {
            return;
        }
        if self.revision + 1 != change.revision {
            return;
        }
        if buffer.byte_len() == 0 {
            return;
        }
        let line_idx = buffer.line_of_byte(
            change
                .new_range
                .start
                .min(buffer.byte_len().saturating_sub(1)),
        );
        let line_start = buffer.byte_of_line(line_idx);
        let line_end = if line_idx + 1 < buffer.line_len() {
            buffer.byte_of_line(line_idx + 1)
        } else {
            buffer.byte_len()
        };
        let line_slice = buffer.slice(line_start..line_end);

        let touches_heading_syntax = line_slice
            .chars()
            .next()
            .is_some_and(|c| c == '#' || c == '=' || c == '-')
            || self.facts.headings.iter().any(|h| {
                h.range.start <= change.old_range.end && change.old_range.start <= h.range.end
            });

        if !touches_heading_syntax
            && change.new_range.len() + change.old_range.len() < 64
            && !line_slice
                .chars()
                .any(|c| matches!(c, '[' | ']' | '^' | '\n'))
        {
            let delta = change.new_range.len() as isize - change.old_range.len() as isize;
            if delta != 0 {
                let facts = Arc::make_mut(&mut self.facts);
                for h in &mut facts.headings {
                    if h.range.start >= change.old_range.end {
                        h.range.start = h.range.start.saturating_add_signed(delta);
                        h.range.end = h.range.end.saturating_add_signed(delta);
                    }
                }
                for l in &mut facts.links {
                    if l.range.start >= change.old_range.end {
                        l.range.start = l.range.start.saturating_add_signed(delta);
                        l.range.end = l.range.end.saturating_add_signed(delta);
                    }
                }
                for b in &mut facts.blocks {
                    if b.range.start >= change.old_range.end {
                        b.range.start = b.range.start.saturating_add_signed(delta);
                        b.range.end = b.range.end.saturating_add_signed(delta);
                    }
                }
            }
            self.revision = change.revision;
        }
    }
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

    /// Fast update of cached document facts for small edits not affecting headings/links/blocks.
    pub(crate) fn notify_buffer_change_for_facts(&mut self, change: &BufferChange) {
        let Some(cached) = &mut self.document_facts else {
            return;
        };
        let Some(buffer) = self
            .shell
            .workspace()
            .buffers()
            .get_buffer(change.buffer_id)
        else {
            return;
        };
        cached.notify_buffer_change(buffer, change);
    }

    pub fn apply_active_transaction(
        &mut self,
        transaction: truff_core::Transaction,
    ) -> super::AppResult<()> {
        self.shell.apply_transaction(transaction)?;
        self.handle_shell_action(crate::shell::ShellAction::EditorEdit);
        Ok(())
    }

    pub fn read_note_content(&self, rel_path: &str) -> Option<String> {
        let buffers = self.shell.workspace().buffers();
        if let Some(root) = self.vault_path() {
            let abs = root.join(rel_path);
            if let Some(id) = buffers.get_by_path(&abs)
                && let Some(buf) = buffers.get_buffer(id)
            {
                return Some(buf.as_str());
            }
            if let Ok(text) = std::fs::read_to_string(&abs) {
                return Some(text);
            }
        }
        if let Some(id) = buffers.get_by_path(std::path::Path::new(rel_path))
            && let Some(buf) = buffers.get_buffer(id)
        {
            return Some(buf.as_str());
        }
        None
    }

    pub fn all_vault_note_paths(&self) -> Vec<String> {
        let mut paths: Vec<String> = self.shell.sidebar_file_paths().map(str::to_owned).collect();
        if let Some(active) = self.active_relative_path()
            && !paths.contains(&active)
        {
            paths.push(active);
        }
        paths.sort();
        paths.dedup();
        paths
    }

    pub fn collect_vault_tags(&self) -> std::collections::BTreeMap<String, Vec<String>> {
        let mut map: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for path in self.all_vault_note_paths() {
            let Some(text) = self.read_note_content(&path) else {
                continue;
            };
            if let Ok(facts) = MarkdownFacts::extract(&text) {
                for tag in facts.tags {
                    map.entry(tag.name).or_default().push(path.clone());
                }
            }
        }
        if map.is_empty()
            && let Some(buf) = self.shell.editor_buffer()
            && let Ok(facts) = MarkdownFacts::extract(&buf.as_str())
        {
            let name = self
                .active_relative_path()
                .unwrap_or_else(|| "scratch.md".to_string());
            for tag in facts.tags {
                map.entry(tag.name).or_default().push(name.clone());
            }
        }
        map
    }

    pub fn collect_database_records(&self) -> Vec<truff_core::markdown::DatabaseRecord> {
        let mut paths = self.all_vault_note_paths();
        if paths.is_empty()
            && let Some(buf) = self.shell.editor_buffer()
        {
            return vec![build_database_record_from_text(
                "scratch.md",
                &buf.as_str(),
                0,
            )];
        }
        paths.sort();

        // First pass: collect raw texts and outgoing targets so we can also count incoming backlinks
        let mut texts: Vec<(String, String)> = Vec::with_capacity(paths.len());
        let mut incoming_counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();

        for path in paths {
            if let Some(text) = self.read_note_content(&path) {
                if let Ok(facts) = MarkdownFacts::extract(&text) {
                    for link in facts.links {
                        let target = link.target.trim();
                        if !target.is_empty() {
                            let key = if target.ends_with(".md") {
                                target.to_string()
                            } else {
                                format!("{target}.md")
                            };
                            *incoming_counts.entry(key).or_insert(0) += 1;
                        }
                    }
                }
                texts.push((path, text));
            }
        }

        texts
            .into_iter()
            .map(|(path, text)| {
                let filename = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&path);
                let in_links = incoming_counts
                    .get(&path)
                    .copied()
                    .unwrap_or_else(|| incoming_counts.get(filename).copied().unwrap_or(0));
                build_database_record_from_text(&path, &text, in_links)
            })
            .collect()
    }

    pub fn build_vault_graph(
        &self,
        cfg: &truff_core::markdown::GraphConfig,
    ) -> truff_core::markdown::GraphModel {
        use truff_core::markdown::{GraphEdge, GraphModel, GraphNode};

        let records = self.collect_database_records();
        let mut filtered_records: Vec<&truff_core::markdown::DatabaseRecord> = records
            .iter()
            .filter(|r| {
                if let Some(tag_filter) = &cfg.tag {
                    let clean = tag_filter.trim_start_matches('#').to_lowercase();
                    r.tags.iter().any(|t| t.to_lowercase() == clean)
                } else {
                    true
                }
            })
            .collect();

        if filtered_records.is_empty() {
            filtered_records = records.iter().collect();
        }

        let mut nodes: Vec<GraphNode> = Vec::with_capacity(filtered_records.len());
        let mut path_to_idx: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();

        for (idx, rec) in filtered_records.iter().enumerate() {
            nodes.push(GraphNode::new(
                rec.path.clone(),
                rec.title.clone(),
                rec.tags.clone(),
            ));
            path_to_idx.insert(rec.path.to_lowercase(), idx);
            if let Some(fname) = std::path::Path::new(&rec.path)
                .file_name()
                .and_then(|n| n.to_str())
            {
                path_to_idx.entry(fname.to_lowercase()).or_insert(idx);
                let stem = fname.trim_end_matches(".md").to_lowercase();
                path_to_idx.entry(stem).or_insert(idx);
            }
        }

        let mut edges: Vec<GraphEdge> = Vec::new();
        for (from_idx, rec) in filtered_records.iter().enumerate() {
            if let Some(text) = self.read_note_content(&rec.path)
                && let Ok(facts) = MarkdownFacts::extract(&text)
            {
                for link in facts.links {
                    let raw = link.target.trim().to_lowercase();
                    let target_idx = path_to_idx
                        .get(&raw)
                        .copied()
                        .or_else(|| path_to_idx.get(&format!("{raw}.md")).copied());
                    if let Some(to_idx) = target_idx
                        && from_idx != to_idx
                        && !edges.iter().any(|e| {
                            (e.from == from_idx && e.to == to_idx)
                                || (e.from == to_idx && e.to == from_idx)
                        })
                    {
                        edges.push(GraphEdge {
                            from: from_idx,
                            to: to_idx,
                            label: link.label,
                        });
                    }
                }
            }
        }

        // Also connect notes sharing a primary tag if there are no explicit wikilink edges yet
        if edges.is_empty() && nodes.len() > 1 {
            for i in 0..nodes.len() {
                for j in (i + 1)..nodes.len() {
                    if nodes[i].tags.iter().any(|t| nodes[j].tags.contains(t)) {
                        edges.push(GraphEdge {
                            from: i,
                            to: j,
                            label: None,
                        });
                    }
                }
            }
        }

        let focus_target = cfg
            .root
            .clone()
            .or_else(|| self.active_relative_path())
            .map(|s| s.to_lowercase());
        let focus_idx = focus_target.and_then(|target| {
            path_to_idx.get(&target).copied().or_else(|| {
                nodes
                    .iter()
                    .position(|n| n.id.to_lowercase() == target || n.label.to_lowercase() == target)
            })
        });

        let mut model = GraphModel::new(nodes, edges, focus_idx);
        model.apply_layout(cfg);
        model
    }

    pub fn sync_folder_wiki_master(&mut self, folder: &str) -> super::AppResult<String> {
        use truff_core::markdown::{
            WikiNoteSummary, is_wiki_master_filename, sync_wiki_master_content,
        };

        let clean_folder = folder.trim_matches('/');
        let folder_prefix = if clean_folder.is_empty() {
            String::new()
        } else {
            format!("{clean_folder}/")
        };
        let master_rel = if clean_folder.is_empty() {
            "index.md".to_string()
        } else {
            format!("{clean_folder}/index.md")
        };

        let mut summaries = Vec::new();
        for path in self.all_vault_note_paths() {
            let in_folder = if folder_prefix.is_empty() {
                !path.contains('/')
            } else {
                path.starts_with(&folder_prefix) && !path[folder_prefix.len()..].contains('/')
            };
            if !in_folder {
                continue;
            }
            let fname = std::path::Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&path);
            if is_wiki_master_filename(fname) {
                continue;
            }
            let Some(text) = self.read_note_content(&path) else {
                continue;
            };
            let rec = build_database_record_from_text(&path, &text, 0);
            let mut headings = Vec::new();
            let mut outgoing = Vec::new();
            if let Ok(facts) = MarkdownFacts::extract(&text) {
                for h in facts.headings {
                    if h.level >= 2 {
                        headings.push(h.text);
                    }
                }
                for l in facts.links {
                    outgoing.push(l.target);
                }
            }
            summaries.push(WikiNoteSummary {
                rel_path: path.clone(),
                title: rec.title,
                tags: rec.tags,
                headings,
                open_tasks: rec.tasks_open,
                done_tasks: rec.tasks_done,
                outgoing_links: outgoing,
            });
        }

        let folder_title = if clean_folder.is_empty() {
            "База знаний хранилища"
        } else {
            clean_folder
        };
        let existing = self.read_note_content(&master_rel).unwrap_or_default();
        let synced = sync_wiki_master_content(&existing, folder_title, &summaries);

        if let Some(root) = self.vault_path().map(std::path::Path::to_path_buf) {
            let full_path = root.join(&master_rel);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&full_path, &synced)?;
            // If resident in workspace, update buffer too
            if let Some(id) = self.shell.workspace().buffers().get_by_path(&full_path)
                && let Some(buf) = self.shell.workspace_mut().buffers_mut().get_buffer_mut(id)
            {
                let len = buf.byte_len();
                let _ = buf.apply(truff_core::Transaction::replace(0..len, synced));
            }
        }
        Ok(master_rel)
    }
}

fn build_database_record_from_text(
    path: &str,
    text: &str,
    links_in: usize,
) -> truff_core::markdown::DatabaseRecord {
    let mut properties = std::collections::BTreeMap::new();
    let mut in_frontmatter = false;
    let mut title: Option<String> = None;
    let mut tasks_open = 0usize;
    let mut tasks_done = 0usize;

    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if idx == 0 && trimmed == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if trimmed == "---" || trimmed == "..." {
                in_frontmatter = false;
                continue;
            }
            if let Some((k, v)) = trimmed.split_once(':') {
                let key = k.trim().to_ascii_lowercase();
                let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
                if !key.is_empty() && !val.is_empty() {
                    properties.insert(key, val);
                }
            }
            continue;
        }
        if title.is_none()
            && let Some(h1) = trimmed.strip_prefix("# ")
        {
            let clean = h1.trim();
            if !clean.is_empty() {
                title = Some(clean.to_string());
            }
        }
        let rest = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "));
        if let Some(r) = rest {
            if r.starts_with("[ ]") {
                tasks_open += 1;
            } else if r.starts_with("[x]") || r.starts_with("[X]") {
                tasks_done += 1;
            }
        }
    }

    let fallback_title = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string();
    let folder = std::path::Path::new(path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .to_string();

    let (tags, links_out) = match MarkdownFacts::extract(text) {
        Ok(facts) => (
            facts.tags.into_iter().map(|t| t.name).collect(),
            facts.links.len(),
        ),
        Err(_) => (Vec::new(), 0),
    };

    let status = properties.get("status").cloned().unwrap_or_else(|| {
        if tasks_open == 0 && tasks_done > 0 {
            "done".to_string()
        } else {
            "active".to_string()
        }
    });
    let updated = properties
        .get("updated")
        .or_else(|| properties.get("created"))
        .or_else(|| properties.get("date"))
        .cloned()
        .unwrap_or_else(|| "2026-10-23".to_string());

    truff_core::markdown::DatabaseRecord {
        path: path.to_string(),
        title: title.unwrap_or(fallback_title),
        folder,
        tags,
        status,
        tasks_open,
        tasks_done,
        links_out,
        links_in,
        updated,
        properties,
    }
}
