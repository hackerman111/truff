-- Truff SQLite schema v1

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    relative_path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    index_generation INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS headings (
    file_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL,
    level INTEGER NOT NULL,
    text TEXT NOT NULL,
    anchor TEXT NOT NULL,
    byte_start INTEGER NOT NULL,
    byte_end INTEGER NOT NULL,
    PRIMARY KEY (file_id, ordinal),
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS blocks (
    file_id INTEGER NOT NULL,
    block_anchor TEXT NOT NULL,
    byte_start INTEGER NOT NULL,
    byte_end INTEGER NOT NULL,
    PRIMARY KEY (file_id, block_anchor),
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS links (
    source_file_id INTEGER NOT NULL,
    occurrence_ordinal INTEGER NOT NULL,
    byte_start INTEGER NOT NULL,
    byte_end INTEGER NOT NULL,
    raw_target TEXT NOT NULL,
    raw_heading TEXT,
    raw_label TEXT,
    is_wiki INTEGER NOT NULL,
    resolved_file_id INTEGER,
    resolved_anchor TEXT,
    PRIMARY KEY (source_file_id, occurrence_ordinal),
    FOREIGN KEY (source_file_id) REFERENCES files(id) ON DELETE CASCADE,
    FOREIGN KEY (resolved_file_id) REFERENCES files(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_links_resolved ON links(resolved_file_id);
CREATE INDEX IF NOT EXISTS idx_links_unresolved ON links(raw_target) WHERE resolved_file_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_headings_anchor ON headings(file_id, anchor);
CREATE INDEX IF NOT EXISTS idx_blocks_anchor ON blocks(file_id, block_anchor);

CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    file_id UNINDEXED,
    title,
    body,
    tokenize = 'unicode61'
);

CREATE TABLE IF NOT EXISTS operation_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    op_type TEXT NOT NULL,
    status TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
