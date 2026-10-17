use rusqlite::{Connection, params};
use truff_core::markdown::facts::DocFacts;

use super::resolve::{BatchResolverCache, FileId, Resolution, Resolver, Target};
use super::schema::filename_keys;

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

    index_document_in(
        &tx,
        file_id,
        relative_path,
        title,
        fingerprint,
        generation,
        facts,
        body_text,
    )?;
    tx.commit()
}

/// The caller owns the transaction, allowing a catalog refresh to commit atomically.
#[allow(clippy::too_many_arguments)]
pub(crate) fn index_document_in(
    conn: &Connection,
    file_id: FileId,
    relative_path: &str,
    title: &str,
    fingerprint: &str,
    generation: u64,
    facts: &DocFacts,
    body_text: &str,
) -> rusqlite::Result<()> {
    index_document_meta_with_resolver_in(
        conn,
        file_id,
        relative_path,
        title,
        fingerprint,
        generation,
        -1,
        -1,
        facts,
        body_text,
        None,
    )
}

#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn index_document_meta_in(
    conn: &Connection,
    file_id: FileId,
    relative_path: &str,
    title: &str,
    fingerprint: &str,
    generation: u64,
    mtime_ns: i64,
    file_size: i64,
    facts: &DocFacts,
    body_text: &str,
) -> rusqlite::Result<()> {
    index_document_meta_with_resolver_in(
        conn,
        file_id,
        relative_path,
        title,
        fingerprint,
        generation,
        mtime_ns,
        file_size,
        facts,
        body_text,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn index_document_meta_with_resolver_in(
    conn: &Connection,
    file_id: FileId,
    relative_path: &str,
    title: &str,
    fingerprint: &str,
    generation: u64,
    mtime_ns: i64,
    file_size: i64,
    facts: &DocFacts,
    body_text: &str,
    resolver: Option<&Resolver<'_>>,
) -> rusqlite::Result<()> {
    // 1. Clean previous facts and fts for this file
    conn.prepare_cached("DELETE FROM headings WHERE file_id = ?1")?
        .execute(params![file_id.0])?;
    conn.prepare_cached("DELETE FROM blocks WHERE file_id = ?1")?
        .execute(params![file_id.0])?;
    conn.prepare_cached("DELETE FROM links WHERE source_file_id = ?1")?
        .execute(params![file_id.0])?;
    conn.prepare_cached("DELETE FROM notes_fts WHERE rowid = ?1")?
        .execute(params![file_id.0])?;

    let (stem, name) = filename_keys(relative_path);
    // 2. Upsert files record with mtime_ns and file_size
    conn.prepare_cached(
        "INSERT INTO files (id, relative_path, title, fingerprint, index_generation, file_stem, file_name, mtime_ns, file_size)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
             relative_path = excluded.relative_path,
             title = excluded.title,
             fingerprint = excluded.fingerprint,
             index_generation = excluded.index_generation,
             file_stem = excluded.file_stem,
             file_name = excluded.file_name,
             mtime_ns = CASE WHEN excluded.mtime_ns >= 0 THEN excluded.mtime_ns ELSE files.mtime_ns END,
             file_size = CASE WHEN excluded.file_size >= 0 THEN excluded.file_size ELSE files.file_size END")?.execute(params![
            file_id.0,
            relative_path,
            title,
            fingerprint,
            generation as i64,
            stem,
            name,
            mtime_ns,
            file_size,
        ],
    )?;

    // 3. Insert headings
    for (ordinal, h) in facts.headings.iter().enumerate() {
        conn.prepare_cached(
            "INSERT INTO headings (file_id, ordinal, level, text, anchor, byte_start, byte_end)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?
        .execute(params![
            file_id.0,
            ordinal as i64,
            h.level as i64,
            h.text,
            h.anchor,
            h.range.start as i64,
            h.range.end as i64,
        ])?;
    }

    // 4. Insert blocks
    for b in &facts.blocks {
        conn.prepare_cached(
            "INSERT OR REPLACE INTO blocks (file_id, block_anchor, byte_start, byte_end)
             VALUES (?1, ?2, ?3, ?4)",
        )?
        .execute(params![
            file_id.0,
            b.id,
            b.range.start as i64,
            b.range.end as i64,
        ])?;
    }

    // 5. Insert links with resolution
    let default_resolver;
    let active_resolver = match resolver {
        Some(r) => r,
        None => {
            default_resolver = Resolver::new(conn);
            &default_resolver
        }
    };
    for link in &facts.links {
        let target = Target::Note {
            path_or_name: link.target.clone(),
            heading_or_block: link.heading.clone(),
            alias: link.label.clone(),
        };

        let (resolved_id, resolved_anchor) = match active_resolver.resolve(file_id, &target) {
            Resolution::Found(id, anchor) => (Some(id.0), anchor),
            _ => (None, None),
        };

        conn.prepare_cached(
            "INSERT INTO links (source_file_id, occurrence_ordinal, byte_start, byte_end,
                                raw_target, raw_heading, raw_label, is_wiki,
                                resolved_file_id, resolved_anchor)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?
        .execute(params![
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
        ])?;
    }

    // 6. Insert into FTS5
    conn.prepare_cached(
        "INSERT INTO notes_fts (rowid, file_id, title, body) VALUES (?1, ?1, ?2, ?3)",
    )?
    .execute(params![file_id.0, title, body_text])?;

    Ok(())
}

/// Updates file metadata without reindexing document body.
pub(crate) fn update_file_meta(
    conn: &Connection,
    file_id: FileId,
    mtime_ns: i64,
    file_size: i64,
) -> rusqlite::Result<()> {
    conn.prepare_cached("UPDATE files SET mtime_ns = ?2, file_size = ?3 WHERE id = ?1")?
        .execute(params![file_id.0, mtime_ns, file_size])?;
    Ok(())
}

/// Removes a file and all its associated indexed facts.
pub fn remove_document(conn: &mut Connection, file_id: FileId) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    remove_document_in(&tx, file_id)?;
    tx.commit()
}

pub(crate) fn remove_document_in(conn: &Connection, file_id: FileId) -> rusqlite::Result<()> {
    conn.prepare_cached("DELETE FROM headings WHERE file_id = ?1")?
        .execute(params![file_id.0])?;
    conn.prepare_cached("DELETE FROM blocks WHERE file_id = ?1")?
        .execute(params![file_id.0])?;
    conn.prepare_cached("DELETE FROM links WHERE source_file_id = ?1")?
        .execute(params![file_id.0])?;
    conn.prepare_cached("DELETE FROM notes_fts WHERE rowid = ?1")?
        .execute(params![file_id.0])?;
    conn.prepare_cached("DELETE FROM files WHERE id = ?1")?
        .execute(params![file_id.0])?;
    Ok(())
}

/// Register all new paths before resolving links to files later in a scan.
pub(crate) fn register_document(
    conn: &Connection,
    file_id: FileId,
    relative_path: &str,
    mtime_ns: i64,
    file_size: i64,
) -> rusqlite::Result<()> {
    let (stem, name) = filename_keys(relative_path);
    conn.prepare_cached(
        "INSERT INTO files (id, relative_path, title, fingerprint, index_generation, file_stem, file_name, mtime_ns, file_size)
         VALUES (?1, ?2, ?2, '', 0, ?3, ?4, ?5, ?6)",
    )?.execute(params![file_id.0, relative_path, stem, name, mtime_ns, file_size])?;
    Ok(())
}

/// Fast targeted refresh of resolutions for a single modified document:
/// checks only incoming links with anchor targets pointing to this file.
pub(crate) fn refresh_resolutions_for_content(
    conn: &Connection,
    file_id: FileId,
) -> rusqlite::Result<()> {
    let resolver = Resolver::new(conn);
    let mut links = conn.prepare_cached(
        "SELECT source_file_id, occurrence_ordinal, raw_target, raw_heading,
                resolved_file_id, resolved_anchor
         FROM links
         WHERE resolved_file_id = ?1 AND raw_heading IS NOT NULL",
    )?;
    let mut rows = links.query(params![file_id.0])?;
    let mut update = conn.prepare_cached(
        "UPDATE links SET resolved_file_id = ?3, resolved_anchor = ?4
         WHERE source_file_id = ?1 AND occurrence_ordinal = ?2",
    )?;
    while let Some(row) = rows.next()? {
        let source = FileId(row.get(0)?);
        let ordinal: i64 = row.get(1)?;
        let target = Target::Note {
            path_or_name: row.get(2)?,
            heading_or_block: row.get(3)?,
            alias: None,
        };
        let previous: (Option<i64>, Option<String>) = (row.get(4)?, row.get(5)?);
        let resolved = match resolver.resolve(source, &target) {
            Resolution::Found(id, anchor) => (Some(id.0), anchor),
            _ => (None, None),
        };
        if previous != resolved {
            update.execute(params![source.0, ordinal, resolved.0, resolved.1])?;
        }
    }
    Ok(())
}

/// Refreshes only unresolved links (`resolved_file_id IS NULL`) using index `idx_links_unresolved`.
pub(crate) fn refresh_unresolved_links(
    conn: &Connection,
    resolver: &Resolver<'_>,
) -> rusqlite::Result<()> {
    let mut links = conn.prepare_cached(
        "SELECT source_file_id, occurrence_ordinal, raw_target, raw_heading,
                resolved_file_id, resolved_anchor
         FROM links
         WHERE resolved_file_id IS NULL OR (raw_heading IS NOT NULL AND resolved_anchor IS NULL)",
    )?;
    let mut rows = links.query([])?;
    let mut update = conn.prepare_cached(
        "UPDATE links SET resolved_file_id = ?3, resolved_anchor = ?4
         WHERE source_file_id = ?1 AND occurrence_ordinal = ?2",
    )?;
    while let Some(row) = rows.next()? {
        let source = FileId(row.get(0)?);
        let ordinal: i64 = row.get(1)?;
        let target = Target::Note {
            path_or_name: row.get(2)?,
            heading_or_block: row.get(3)?,
            alias: None,
        };
        let previous: (Option<i64>, Option<String>) = (row.get(4)?, row.get(5)?);
        let resolved = match resolver.resolve(source, &target) {
            Resolution::Found(id, anchor) => (Some(id.0), anchor),
            _ => (None, None),
        };
        if previous != resolved {
            update.execute(params![source.0, ordinal, resolved.0, resolved.1])?;
        }
    }
    Ok(())
}

/// Facts may refer to newly added/removed files or headings indexed later in a batch.
/// Reuses BatchResolverCache when available for orders-of-magnitude faster resolution.
#[allow(dead_code)]
pub(crate) fn refresh_resolutions(conn: &Connection) -> rusqlite::Result<()> {
    let cache = BatchResolverCache::load(conn).ok();
    let resolver = match &cache {
        Some(c) => Resolver::with_cache(conn, c),
        None => Resolver::new(conn),
    };
    let mut links = conn.prepare_cached(
        "SELECT source_file_id, occurrence_ordinal, raw_target, raw_heading,
                resolved_file_id, resolved_anchor FROM links",
    )?;
    let mut rows = links.query([])?;
    let mut update = conn.prepare_cached(
        "UPDATE links SET resolved_file_id = ?3, resolved_anchor = ?4
         WHERE source_file_id = ?1 AND occurrence_ordinal = ?2",
    )?;
    while let Some(row) = rows.next()? {
        let source = FileId(row.get(0)?);
        let ordinal: i64 = row.get(1)?;
        let target = Target::Note {
            path_or_name: row.get(2)?,
            heading_or_block: row.get(3)?,
            alias: None,
        };
        let previous: (Option<i64>, Option<String>) = (row.get(4)?, row.get(5)?);
        let resolved = match resolver.resolve(source, &target) {
            Resolution::Found(id, anchor) => (Some(id.0), anchor),
            _ => (None, None),
        };
        if previous != resolved {
            update.execute(params![source.0, ordinal, resolved.0, resolved.1])?;
        }
    }
    Ok(())
}
