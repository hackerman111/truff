use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use rusqlite::Connection;
use truff_core::markdown::facts::DocFacts;

pub mod backlinks;
pub mod resolve;
pub mod search;
pub mod update;

pub use backlinks::{BacklinkOccurrence, BacklinkPage};
pub use resolve::{FileId, Resolution, Resolver, Target, slugify};
pub use search::{
    DEFAULT_MAX_SEARCH_PAGE_SIZE, SearchPage, SearchQuery, SearchResult, SearchSession,
    escape_fts5_query,
};

const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Errors occurring during SQLite index operations.
#[derive(Debug)]
pub enum IndexError {
    Sqlite(rusqlite::Error),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "sqlite index error: {e}"),
        }
    }
}

impl std::error::Error for IndexError {}

impl From<rusqlite::Error> for IndexError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

/// Persistent SQLite knowledge index maintaining files, headings, blocks, links, and FTS5 search.
pub struct IndexDatabase {
    conn: Connection,
    dirty_overlays: HashMap<FileId, (String, DocFacts)>,
}

impl IndexDatabase {
    /// Opens or creates an index database at the specified filesystem path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, IndexError> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// Opens an in-memory index database for isolated testing.
    pub fn open_in_memory() -> Result<Self, IndexError> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self, IndexError> {
        // Enforce required PRAGMAs: WAL, cache limit 8 MiB (-8000 KiB), synchronous NORMAL
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -8000;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;",
        )?;

        conn.execute_batch(SCHEMA_SQL)?;

        Ok(Self {
            conn,
            dirty_overlays: HashMap::new(),
        })
    }

    /// Access to SQLite resolver borrowing connection.
    pub fn resolver(&self) -> Resolver<'_> {
        Resolver::new(&self.conn)
    }

    /// Indexes or re-indexes document facts and body.
    #[allow(clippy::too_many_arguments)]
    pub fn index_file(
        &mut self,
        file_id: FileId,
        relative_path: &str,
        title: &str,
        fingerprint: &str,
        generation: u64,
        facts: &DocFacts,
        body_text: &str,
    ) -> Result<(), IndexError> {
        update::index_document(
            &mut self.conn,
            file_id,
            relative_path,
            title,
            fingerprint,
            generation,
            facts,
            body_text,
        )?;
        Ok(())
    }

    /// Removes a file and all its indexed facts from the database.
    pub fn remove_file(&mut self, file_id: FileId) -> Result<(), IndexError> {
        self.dirty_overlays.remove(&file_id);
        update::remove_document(&mut self.conn, file_id)?;
        Ok(())
    }

    /// Sets or updates the dirty in-memory overlay facts for an actively edited document.
    pub fn set_dirty_overlay(&mut self, file_id: FileId, relative_path: String, facts: DocFacts) {
        self.dirty_overlays.insert(file_id, (relative_path, facts));
    }

    /// Clears any in-memory dirty overlay for a file after save or discard.
    pub fn clear_dirty_overlay(&mut self, file_id: FileId) {
        self.dirty_overlays.remove(&file_id);
    }

    /// Queries backlinks pointing to the target file or heading/block.
    pub fn get_backlinks(
        &self,
        target_file_id: FileId,
        target_anchor: Option<&str>,
        page: usize,
        page_size: usize,
    ) -> Result<BacklinkPage, IndexError> {
        let res = update::query_backlinks(
            &self.conn,
            &self.dirty_overlays,
            target_file_id,
            target_anchor,
            page,
            page_size,
        )?;
        Ok(res)
    }

    /// Executes full-text search against indexed documents.
    pub fn search(&self, query: &SearchQuery) -> Result<SearchPage, IndexError> {
        let res = search::execute_search(&self.conn, query)?;
        Ok(res)
    }

    /// Returns direct reference to underlying SQLite connection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Returns direct mutable reference to underlying SQLite connection.
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}
