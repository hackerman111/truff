use std::fmt;
use std::path::{Component, Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

/// Strongly typed identifier for a file in the SQLite index.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct FileId(pub i64);

impl FileId {
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Target representation of a link in Markdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// Note reference, e.g. `[[path/to/Note#Heading|Alias]]`.
    Note {
        path_or_name: String,
        heading_or_block: Option<String>,
        alias: Option<String>,
    },
    /// External URL reference, e.g. `https://example.com`.
    ExternalUrl(String),
}

impl Target {
    /// Parses a raw link target string into a `Target`.
    pub fn parse(raw: &str) -> Self {
        let trimmed = raw.trim();
        if trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("mailto:")
            || trimmed.starts_with("ftp://")
        {
            return Self::ExternalUrl(trimmed.to_string());
        }

        let (path_part, alias_part) = match trimmed.find('|') {
            Some(idx) => (
                trimmed[..idx].trim(),
                Some(trimmed[idx + 1..].trim().to_string()),
            ),
            None => (trimmed, None),
        };

        let (path_or_name, heading_or_block) = match path_part.find('#') {
            Some(idx) => (
                path_part[..idx].trim().to_string(),
                Some(path_part[idx..].trim().to_string()),
            ),
            None => (path_part.to_string(), None),
        };

        Self::Note {
            path_or_name,
            heading_or_block,
            alias: alias_part,
        }
    }
}

/// Result of resolving a target link against the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Resolved unambiguously to a specific file and optional heading/block anchor.
    Found(FileId, Option<String>),
    /// Target file was not found in the index.
    Missing,
    /// Target is ambiguous because multiple files match the basename.
    Ambiguous(Vec<FileId>),
    /// Index is stale or in an incompatible generation.
    StaleIndex,
}

use std::collections::HashMap;

/// Preloaded cache of file paths and stems for fast batch resolution in memory.
#[derive(Debug, Default, Clone)]
pub struct BatchResolverCache {
    exact_paths: HashMap<String, FileId>,
    source_paths: HashMap<FileId, String>,
    stems: HashMap<String, Vec<FileId>>,
}

impl BatchResolverCache {
    /// Loads all file paths, stems and names from SQLite into memory for bulk resolution.
    pub fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut stmt = conn.prepare_cached(
            "SELECT id, relative_path, file_stem, file_name FROM files",
        )?;
        let mut exact_paths = HashMap::new();
        let mut source_paths = HashMap::new();
        let mut stems: HashMap<String, Vec<FileId>> = HashMap::new();
        let rows = stmt.query_map([], |row| {
            Ok((
                FileId(row.get(0)?),
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (id, rel_path, stem, name) = row?;
            exact_paths.insert(rel_path.clone(), id);
            source_paths.insert(id, rel_path);
            if !stem.is_empty() {
                let entry = stems.entry(stem).or_default();
                if !entry.contains(&id) {
                    entry.push(id);
                }
            }
            if !name.is_empty() {
                let entry = stems.entry(name).or_default();
                if !entry.contains(&id) {
                    entry.push(id);
                }
            }
        }
        Ok(Self {
            exact_paths,
            source_paths,
            stems,
        })
    }

    /// Fast in-memory resolution of a note reference without SQLite FFI queries.
    pub(crate) fn find_file(&self, source: FileId, path_or_name: &str) -> FileFindResult {
        let with_md = if path_or_name.ends_with(".md") {
            path_or_name.to_string()
        } else {
            format!("{path_or_name}.md")
        };

        // 1. Exact match by relative_path
        if let Some(&id) = self.exact_paths.get(path_or_name).or_else(|| self.exact_paths.get(&with_md)) {
            return FileFindResult::Found(id);
        }

        // 2. Exact match relative to source file's directory
        if source.0 > 0
            && let Some(source_rel) = self.source_paths.get(&source)
        {
            let source_path = Path::new(source_rel);
            if let Some(parent) = source_path.parent() {
                let resolved = normalize_path(&parent.join(path_or_name));
                let resolved_str = resolved.to_string_lossy();
                let resolved_with_md = if resolved_str.ends_with(".md") {
                    resolved_str.to_string()
                } else {
                    format!("{resolved_str}.md")
                };
                if let Some(&id) = self.exact_paths.get(resolved_str.as_ref()).or_else(|| self.exact_paths.get(&resolved_with_md)) {
                    return FileFindResult::Found(id);
                }
            }
        }

        // 3. Basename matching
        let target_stem = if let Some(stripped) = path_or_name.strip_suffix(".md") {
            stripped
        } else {
            path_or_name
        };

        let mut matches = Vec::new();
        if let Some(ids) = self.stems.get(target_stem) {
            for &id in ids {
                if !matches.contains(&id) {
                    matches.push(id);
                }
            }
        }
        if target_stem != path_or_name
            && let Some(ids) = self.stems.get(path_or_name)
        {
            for &id in ids {
                if !matches.contains(&id) {
                    matches.push(id);
                }
            }
        }
        if let Some(ids) = self.stems.get(&with_md) {
            for &id in ids {
                if !matches.contains(&id) {
                    matches.push(id);
                }
            }
        }

        if matches.is_empty() {
            FileFindResult::Missing
        } else if matches.len() == 1 {
            FileFindResult::Found(matches[0])
        } else {
            matches.sort_unstable();
            FileFindResult::Ambiguous(matches)
        }
    }
}

/// Link and target resolver backed by SQLite index.
pub struct Resolver<'a> {
    conn: &'a Connection,
    cache: Option<&'a BatchResolverCache>,
}

impl<'a> Resolver<'a> {
    /// Creates a resolver borrowing an SQLite connection.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn, cache: None }
    }

    /// Creates a resolver with a preloaded batch cache for in-memory resolution.
    pub fn with_cache(conn: &'a Connection, cache: &'a BatchResolverCache) -> Self {
        Self {
            conn,
            cache: Some(cache),
        }
    }

    /// Resolves a `Target` originating from `source` file against indexed files.
    pub fn resolve(&self, source: FileId, target: &Target) -> Resolution {
        match target {
            Target::ExternalUrl(_) => Resolution::Missing,
            Target::Note {
                path_or_name,
                heading_or_block,
                ..
            } => self.resolve_note(source, path_or_name, heading_or_block.as_deref()),
        }
    }

    fn resolve_note(
        &self,
        source: FileId,
        path_or_name: &str,
        heading_or_block: Option<&str>,
    ) -> Resolution {
        let trimmed_name = path_or_name.trim();

        // 1. If target is empty, it refers to the source document itself (e.g. [[#Heading]])
        let target_file_id = if trimmed_name.is_empty() {
            if source.0 <= 0 {
                return Resolution::Missing;
            }
            source
        } else {
            // Find target file: exact relative path has priority
            match self.find_file(source, trimmed_name) {
                Ok(res) => match res {
                    FileFindResult::Found(id) => id,
                    FileFindResult::Missing => return Resolution::Missing,
                    FileFindResult::Ambiguous(ids) => return Resolution::Ambiguous(ids),
                },
                Err(_) => return Resolution::StaleIndex,
            }
        };

        // 2. Resolve heading or block if specified
        let resolved_anchor = if let Some(h_or_b) = heading_or_block {
            self.resolve_anchor(target_file_id, h_or_b)
        } else {
            None
        };

        Resolution::Found(target_file_id, resolved_anchor)
    }

    fn find_file(&self, source: FileId, path_or_name: &str) -> rusqlite::Result<FileFindResult> {
        if let Some(cache) = self.cache {
            return Ok(cache.find_file(source, path_or_name));
        }
        let with_md = if path_or_name.ends_with(".md") {
            path_or_name.to_string()
        } else {
            format!("{path_or_name}.md")
        };

        // 1. Exact match by relative_path (as given, or with .md)
        let exact_opt: Option<i64> = self
            .conn
            .prepare_cached("SELECT id FROM files WHERE relative_path = ?1 OR relative_path = ?2")?
            .query_row(params![path_or_name, with_md], |row| row.get(0))
            .optional()?;

        if let Some(id) = exact_opt {
            return Ok(FileFindResult::Found(FileId(id)));
        }

        // 2. Exact match relative to source file's directory
        if source.0 > 0 {
            let source_rel_opt: Option<String> = self
                .conn
                .prepare_cached("SELECT relative_path FROM files WHERE id = ?1")?
                .query_row(params![source.0], |row| row.get(0))
                .optional()?;

            if let Some(source_rel) = source_rel_opt {
                let source_path = Path::new(&source_rel);
                if let Some(parent) = source_path.parent() {
                    let resolved = normalize_path(&parent.join(path_or_name));
                    let resolved_str = resolved.to_string_lossy();
                    let resolved_with_md = if resolved_str.ends_with(".md") {
                        resolved_str.to_string()
                    } else {
                        format!("{resolved_str}.md")
                    };

                    let rel_exact_opt: Option<i64> = self
                        .conn
                        .prepare_cached(
                            "SELECT id FROM files WHERE relative_path = ?1 OR relative_path = ?2",
                        )?
                        .query_row(params![resolved_str, resolved_with_md], |row| row.get(0))
                        .optional()?;

                    if let Some(id) = rel_exact_opt {
                        return Ok(FileFindResult::Found(FileId(id)));
                    }
                }
            }
        }

        // 3. Basename matching: exact relative path was not found, check basename match
        let target_stem = if let Some(stripped) = path_or_name.strip_suffix(".md") {
            stripped
        } else {
            path_or_name
        };

        let mut stmt = self.conn.prepare_cached(
            "SELECT id FROM files
             WHERE file_stem IN (?1, ?2) OR file_name IN (?2, ?3)
             ORDER BY id",
        )?;
        let matches = stmt
            .query_map(params![target_stem, path_or_name, with_md], |row| {
                Ok(FileId(row.get(0)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        if matches.is_empty() {
            Ok(FileFindResult::Missing)
        } else if matches.len() == 1 {
            Ok(FileFindResult::Found(matches[0]))
        } else {
            Ok(FileFindResult::Ambiguous(matches))
        }
    }

    fn resolve_anchor(&self, file_id: FileId, anchor: &str) -> Option<String> {
        let clean = anchor.trim_start_matches('#');
        if clean.is_empty() {
            return None;
        }

        // Check if block reference
        if clean.starts_with('^') || anchor.starts_with("#^") {
            let block_key = if clean.starts_with('^') {
                clean
            } else {
                &clean[1..]
            };
            let with_caret = if block_key.starts_with('^') {
                block_key.to_string()
            } else {
                format!("^{block_key}")
            };
            let without_caret = block_key.trim_start_matches('^');

            let opt: Option<String> = self
                .conn
                .prepare_cached("SELECT block_anchor FROM blocks WHERE file_id = ?1 AND (block_anchor = ?2 OR block_anchor = ?3)")
                .and_then(|mut stmt| stmt.query_row(
                    params![file_id.0, with_caret, without_caret],
                    |_row| Ok(with_caret.clone()),
                ))
                .optional()
                .ok()
                .flatten();

            return opt;
        }

        // Check heading anchor
        let slug = slugify(clean);
        let opt: Option<String> = self
            .conn
            .prepare_cached("SELECT anchor FROM headings WHERE file_id = ?1 AND (anchor = ?2 OR text = ?3 COLLATE NOCASE)")
            .and_then(|mut stmt| stmt.query_row(
                params![file_id.0, slug, clean],
                |row| row.get(0),
            ))
            .optional()
            .ok()
            .flatten();

        opt
    }
}

pub(crate) enum FileFindResult {
    Found(FileId),
    Missing,
    Ambiguous(Vec<FileId>),
}

/// Normalizes path components without resolving against filesystem.
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Normal(c) => normalized.push(c),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    normalized
}

/// Normalizes text into an anchor slug.
pub fn slugify(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .map(|c| {
            if c == ' ' {
                '-'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
