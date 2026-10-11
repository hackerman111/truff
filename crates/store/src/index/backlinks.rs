use std::collections::HashMap;
use std::ops::Range;

use rusqlite::{Connection, params};
use truff_core::markdown::facts::DocFacts;

use super::resolve::{FileId, Resolution, Resolver, Target};

/// Occurrence of a backlink pointing to a target note, heading, or block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklinkOccurrence {
    pub source_file_id: FileId,
    pub source_path: String,
    pub occurrence_ordinal: usize,
    pub range: Range<usize>,
    pub raw_target: String,
    pub heading: Option<String>,
    pub label: Option<String>,
    pub is_wiki: bool,
}

/// Paginated backlinks response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklinkPage {
    pub page: usize,
    pub total_matches: usize,
    pub occurrences: Vec<BacklinkOccurrence>,
}

/// Queries paginated backlinks for a target file, substituting dirty overlay facts for modified active documents.
pub fn query_backlinks(
    conn: &Connection,
    dirty_overlays: &HashMap<FileId, (String, DocFacts)>,
    target_file_id: FileId,
    target_anchor: Option<&str>,
    page: usize,
    page_size: usize,
) -> rusqlite::Result<BacklinkPage> {
    let mut all_occurrences = Vec::new();

    // 1. Fetch disk links where source is NOT in dirty_overlays
    let mut stmt = conn.prepare(
        "SELECT l.source_file_id, f.relative_path, l.occurrence_ordinal,
                l.byte_start, l.byte_end, l.raw_target, l.raw_heading,
                l.raw_label, l.is_wiki, l.resolved_anchor
         FROM links l
         JOIN files f ON f.id = l.source_file_id
         WHERE l.resolved_file_id = ?1
         ORDER BY f.relative_path ASC, l.occurrence_ordinal ASC",
    )?;

    let rows = stmt.query_map(params![target_file_id.0], |row| {
        let source_id: i64 = row.get(0)?;
        let source_path: String = row.get(1)?;
        let ordinal: i64 = row.get(2)?;
        let byte_start: i64 = row.get(3)?;
        let byte_end: i64 = row.get(4)?;
        let raw_target: String = row.get(5)?;
        let heading: Option<String> = row.get(6)?;
        let label: Option<String> = row.get(7)?;
        let is_wiki_int: i64 = row.get(8)?;
        let resolved_anchor: Option<String> = row.get(9)?;

        Ok((
            FileId(source_id),
            source_path,
            ordinal as usize,
            byte_start as usize..byte_end as usize,
            raw_target,
            heading,
            label,
            is_wiki_int != 0,
            resolved_anchor,
        ))
    })?;

    for item in rows {
        let (
            source_id,
            source_path,
            ordinal,
            range,
            raw_target,
            heading,
            label,
            is_wiki,
            resolved_anchor,
        ) = item?;

        // If this source document is currently dirty, DISK version is skipped to avoid duplication!
        if dirty_overlays.contains_key(&source_id) {
            continue;
        }

        // Anchor filter if requested
        if let Some(req_anchor) = target_anchor
            && resolved_anchor.as_deref() != Some(req_anchor)
        {
            continue;
        }

        all_occurrences.push(BacklinkOccurrence {
            source_file_id: source_id,
            source_path,
            occurrence_ordinal: ordinal,
            range,
            raw_target,
            heading,
            label,
            is_wiki,
        });
    }

    // 2. Evaluate dirty overlay facts
    let resolver = Resolver::new(conn);
    for (&dirty_id, (dirty_path, dirty_facts)) in dirty_overlays {
        for link in &dirty_facts.links {
            let target = Target::Note {
                path_or_name: link.target.clone(),
                heading_or_block: link.heading.clone(),
                alias: link.label.clone(),
            };

            if let Resolution::Found(found_id, found_anchor) = resolver.resolve(dirty_id, &target)
                && found_id == target_file_id
            {
                if let Some(req_anchor) = target_anchor
                    && found_anchor.as_deref() != Some(req_anchor)
                {
                    continue;
                }

                all_occurrences.push(BacklinkOccurrence {
                    source_file_id: dirty_id,
                    source_path: dirty_path.clone(),
                    occurrence_ordinal: link.ordinal,
                    range: link.range.clone(),
                    raw_target: link.target.clone(),
                    heading: link.heading.clone(),
                    label: link.label.clone(),
                    is_wiki: link.is_wiki,
                });
            }
        }
    }

    // Stable sort by path and ordinal
    all_occurrences.sort_by(|a, b| {
        a.source_path
            .cmp(&b.source_path)
            .then_with(|| a.occurrence_ordinal.cmp(&b.occurrence_ordinal))
    });

    let total_matches = all_occurrences.len();
    let capped_page_size = page_size.max(1);
    let start = page * capped_page_size;
    let occurrences = if start < total_matches {
        let end = (start + capped_page_size).min(total_matches);
        all_occurrences[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok(BacklinkPage {
        page,
        total_matches,
        occurrences,
    })
}
