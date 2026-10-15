use std::path::Path;

use rusqlite::{Connection, params};

pub(super) fn initialize(conn: &mut Connection) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch(include_str!("schema.sql"))?;

    let has_file_stem: bool = {
        let mut pragma = tx.prepare("PRAGMA table_info(files)")?;
        let columns = pragma.query_map([], |row| row.get::<_, String>(1))?;
        let mut found = false;
        for col in columns {
            if col? == "file_stem" {
                found = true;
                break;
            }
        }
        found
    };

    if !has_file_stem {
        tx.execute_batch(
            "ALTER TABLE files ADD COLUMN file_stem TEXT NOT NULL DEFAULT '';
             ALTER TABLE files ADD COLUMN file_name TEXT NOT NULL DEFAULT '';",
        )?;
    }
    tx.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_files_stem ON files(file_stem);
         CREATE INDEX IF NOT EXISTS idx_files_name ON files(file_name);",
    )?;

    {
        let mut files = tx.prepare("SELECT id, relative_path FROM files WHERE file_stem = ''")?;
        let mut update =
            tx.prepare("UPDATE files SET file_stem = ?2, file_name = ?3 WHERE id = ?1")?;
        let rows = files.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, relative_path) = row?;
            let (stem, name) = filename_keys(&relative_path);
            update.execute(params![id, stem, name])?;
        }
    }

    // Legacy FTS rowids were unrelated to FileId. Rebuild atomically before
    // using rowid for deletes and joins; retain the newest row for duplicate IDs.
    let needs_fts_migration: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM notes_fts WHERE rowid != file_id LIMIT 1)",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if needs_fts_migration {
        tx.execute_batch(
            "CREATE VIRTUAL TABLE notes_fts_v2 USING fts5(
                 file_id UNINDEXED, title, body, tokenize = 'unicode61');
             INSERT OR REPLACE INTO notes_fts_v2(rowid, file_id, title, body)
                 SELECT f.id, f.id, n.title, n.body FROM notes_fts n
                 JOIN files f ON f.id = n.file_id ORDER BY n.rowid;
             DROP TABLE notes_fts;
             ALTER TABLE notes_fts_v2 RENAME TO notes_fts;",
        )?;
    }

    let version: i64 = tx.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < 4 {
        tx.execute_batch("PRAGMA user_version = 4;")?;
    }

    tx.commit()
}

pub(super) fn filename_keys(relative_path: &str) -> (&str, &str) {
    let path = Path::new(relative_path);
    (
        path.file_stem().and_then(|s| s.to_str()).unwrap_or(""),
        path.file_name().and_then(|s| s.to_str()).unwrap_or(""),
    )
}
