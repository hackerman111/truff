use rusqlite::{Connection, params};
use truff_core::markdown::facts::DocFacts;

use super::resolve::{FileId, Resolution, Resolver, Target};

// Preserve the previous public paths while backlink queries live in their own module.
pub use super::backlinks::{BacklinkOccurrence, BacklinkPage, query_backlinks};

/// Indexes a document's extracted facts and full text into the SQLite index.
#[allow(clippy::too_many_arguments)]
pub fn index_document(
    conn: &mut Connection,
    file_id: FileId,
    relative_path: &str,
    title: &str,
    fingerprint: &str,
    generation: u64,
    facts: &DocFacts,
    body_text: &str,
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;

    // 1. Clean previous facts and fts for this file
    tx.execute(
        "DELETE FROM headings WHERE file_id = ?1",
        params![file_id.0],
    )?;
    tx.execute("DELETE FROM blocks WHERE file_id = ?1", params![file_id.0])?;
    tx.execute(
        "DELETE FROM links WHERE source_file_id = ?1",
        params![file_id.0],
    )?;
    tx.execute(
        "DELETE FROM notes_fts WHERE file_id = ?1",
        params![file_id.0],
    )?;

    // 2. Upsert files record
    tx.execute(
        "INSERT INTO files (id, relative_path, title, fingerprint, index_generation)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
             relative_path = excluded.relative_path,
             title = excluded.title,
             fingerprint = excluded.fingerprint,
             index_generation = excluded.index_generation",
        params![
            file_id.0,
            relative_path,
            title,
            fingerprint,
            generation as i64
        ],
    )?;

    // 3. Insert headings
    for (ordinal, h) in facts.headings.iter().enumerate() {
        tx.execute(
            "INSERT INTO headings (file_id, ordinal, level, text, anchor, byte_start, byte_end)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                file_id.0,
                ordinal as i64,
                h.level as i64,
                h.text,
                h.anchor,
                h.range.start as i64,
                h.range.end as i64,
            ],
        )?;
    }

    // 4. Insert blocks
    for b in &facts.blocks {
        tx.execute(
            "INSERT OR REPLACE INTO blocks (file_id, block_anchor, byte_start, byte_end)
             VALUES (?1, ?2, ?3, ?4)",
            params![file_id.0, b.id, b.range.start as i64, b.range.end as i64,],
        )?;
    }

    // 5. Insert links with resolution
    let resolver = Resolver::new(&tx);
    for link in &facts.links {
        let target = Target::Note {
            path_or_name: link.target.clone(),
            heading_or_block: link.heading.clone(),
            alias: link.label.clone(),
        };

        let (resolved_id, resolved_anchor) = match resolver.resolve(file_id, &target) {
            Resolution::Found(id, anchor) => (Some(id.0), anchor),
            _ => (None, None),
        };

        tx.execute(
            "INSERT INTO links (source_file_id, occurrence_ordinal, byte_start, byte_end,
                                raw_target, raw_heading, raw_label, is_wiki,
                                resolved_file_id, resolved_anchor)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                file_id.0,
                link.ordinal as i64,
                link.range.start as i64,
                link.range.end as i64,
                link.target,
                link.heading,
                link.label,
                if link.is_wiki { 1 } else { 0 },
                resolved_id,
                resolved_anchor,
            ],
        )?;
    }

    // 6. Insert into FTS5
    tx.execute(
        "INSERT INTO notes_fts (file_id, title, body) VALUES (?1, ?2, ?3)",
        params![file_id.0, title, body_text],
    )?;

    tx.commit()?;
    Ok(())
}

/// Removes a file and all its associated indexed facts.
pub fn remove_document(conn: &mut Connection, file_id: FileId) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM headings WHERE file_id = ?1",
        params![file_id.0],
    )?;
    tx.execute("DELETE FROM blocks WHERE file_id = ?1", params![file_id.0])?;
    tx.execute(
        "DELETE FROM links WHERE source_file_id = ?1",
        params![file_id.0],
    )?;
    tx.execute(
        "DELETE FROM notes_fts WHERE file_id = ?1",
        params![file_id.0],
    )?;
    tx.execute("DELETE FROM files WHERE id = ?1", params![file_id.0])?;
    tx.commit()?;
    Ok(())
}
