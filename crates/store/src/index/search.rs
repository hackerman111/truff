use rusqlite::{Connection, params};

use super::resolve::FileId;

/// Maximum page size allowed for search queries (§10.1).
pub const DEFAULT_MAX_SEARCH_PAGE_SIZE: usize = 100;

/// Specification for a search query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub id: u64,
    pub text: String,
    pub page: usize,
    pub page_size: usize,
}

/// A page of search results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPage {
    pub query_id: u64,
    pub page: usize,
    pub total_matches: usize,
    pub results: Vec<SearchResult>,
}

/// A single matched note item in search results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub file_id: FileId,
    pub path: String,
    pub title: String,
    pub snippet: String,
}

/// Coordinates query sequence numbers to discard stale responses from out-of-order queries.
#[derive(Debug, Default)]
pub struct SearchSession {
    latest_query_id: u64,
}

impl SearchSession {
    /// Creates a new search session starting at query id 0.
    pub fn new() -> Self {
        Self { latest_query_id: 0 }
    }

    /// Generates next query with monotonic ID, capping page size to 100.
    pub fn next_query(&mut self, text: &str, page: usize, page_size: usize) -> SearchQuery {
        self.latest_query_id += 1;
        let bounded_page_size = page_size.clamp(1, DEFAULT_MAX_SEARCH_PAGE_SIZE);
        SearchQuery {
            id: self.latest_query_id,
            text: text.to_string(),
            page,
            page_size: bounded_page_size,
        }
    }

    /// Returns `Some(page)` if `page.query_id >= self.latest_query_id`, otherwise drops stale page (`None`).
    pub fn accept_page(&self, page: SearchPage) -> Option<SearchPage> {
        if page.query_id >= self.latest_query_id {
            Some(page)
        } else {
            None
        }
    }

    /// Returns the current latest query ID.
    pub fn latest_query_id(&self) -> u64 {
        self.latest_query_id
    }
}

/// Escapes raw user search text into safe FTS5 query syntax.
pub fn escape_fts5_query(raw: &str) -> String {
    let mut terms = Vec::new();
    for token in raw.split_whitespace() {
        let cleaned: String = token.chars().filter(|c| *c != '"').collect();
        if !cleaned.is_empty() {
            // Quote the token to treat punctuation / keywords literally, add * for prefix matching
            terms.push(format!("\"{cleaned}\"*"));
        }
    }
    terms.join(" ")
}

/// Executes a search query against SQLite FTS5 table.
pub fn execute_search(conn: &Connection, query: &SearchQuery) -> rusqlite::Result<SearchPage> {
    let fts_query = escape_fts5_query(&query.text);
    if fts_query.is_empty() {
        return Ok(SearchPage {
            query_id: query.id,
            page: query.page,
            total_matches: 0,
            results: Vec::new(),
        });
    }

    let total_matches: i64 = conn
        .prepare_cached("SELECT count(*) FROM notes_fts WHERE notes_fts MATCH ?1")?
        .query_row(params![fts_query], |row| row.get::<_, i64>(0))?;
    let total_matches = total_matches as usize;

    let page_size = query.page_size.clamp(1, DEFAULT_MAX_SEARCH_PAGE_SIZE);
    let Some(offset) = query
        .page
        .checked_mul(page_size)
        .and_then(|offset| i64::try_from(offset).ok())
    else {
        return Ok(SearchPage {
            query_id: query.id,
            page: query.page,
            total_matches,
            results: Vec::new(),
        });
    };

    let mut stmt = conn.prepare_cached(
        "SELECT f.id, f.relative_path, f.title, snippet(notes_fts, 2, '<b>', '</b>', '...', 32)
         FROM notes_fts
         JOIN files f ON f.id = notes_fts.rowid
         WHERE notes_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2 OFFSET ?3",
    )?;

    let rows = stmt.query_map(params![fts_query, page_size as i64, offset], |row| {
        let file_id: i64 = row.get(0)?;
        let path: String = row.get(1)?;
        let title: String = row.get(2)?;
        let snippet: String = row.get(3)?;
        Ok(SearchResult {
            file_id: FileId(file_id),
            path,
            title,
            snippet,
        })
    })?;

    let mut results = Vec::new();
    for item in rows {
        results.push(item?);
    }

    Ok(SearchPage {
        query_id: query.id,
        page: query.page,
        total_matches,
        results,
    })
}
