use truff_core::markdown::facts::MarkdownFacts;
use truff_store::index::{FileId, IndexDatabase, Resolution, SearchQuery, SearchSession, Target};

#[test]
fn resolver_ambiguous() {
    let mut db = IndexDatabase::open_in_memory().expect("open in memory db");

    let doc1 = MarkdownFacts::extract("# First\nContent 1").unwrap();
    let doc2 = MarkdownFacts::extract("# Second\nContent 2").unwrap();
    let source_doc = MarkdownFacts::extract("# Source\nContent source").unwrap();

    let id1 = FileId(1);
    let id2 = FileId(2);
    let source_id = FileId(3);

    db.index_file(
        id1,
        "folder_a/target.md",
        "First",
        "fp1",
        1,
        &doc1,
        "Content 1",
    )
    .unwrap();
    db.index_file(
        id2,
        "folder_b/target.md",
        "Second",
        "fp2",
        1,
        &doc2,
        "Content 2",
    )
    .unwrap();
    db.index_file(
        source_id,
        "notes/source.md",
        "Source",
        "fp3",
        1,
        &source_doc,
        "Content source",
    )
    .unwrap();

    let resolver = db.resolver();

    // 1. Target with basename "target" is ambiguous because both folder_a/target.md and folder_b/target.md match
    let target_by_basename = Target::Note {
        path_or_name: "target".to_string(),
        heading_or_block: None,
        alias: None,
    };
    let res = resolver.resolve(source_id, &target_by_basename);
    match res {
        Resolution::Ambiguous(files) => {
            assert_eq!(files.len(), 2);
            assert!(files.contains(&id1));
            assert!(files.contains(&id2));
        }
        other => panic!("expected Ambiguous, got {other:?}"),
    }

    // 2. Exact relative path has priority and disambiguates
    let target_exact = Target::Note {
        path_or_name: "folder_a/target.md".to_string(),
        heading_or_block: None,
        alias: None,
    };
    let res_exact = resolver.resolve(source_id, &target_exact);
    assert_eq!(res_exact, Resolution::Found(id1, None));
}

#[test]
fn heading_block_target() {
    let mut db = IndexDatabase::open_in_memory().expect("open in memory db");

    let target_text = "# Introduction\nSome text.\n\nParagraph with block ^quote-123\n";
    let target_facts = MarkdownFacts::extract(target_text).unwrap();
    let target_id = FileId(10);
    db.index_file(
        target_id,
        "doc.md",
        "Introduction",
        "fp10",
        1,
        &target_facts,
        target_text,
    )
    .unwrap();

    let source_id = FileId(20);
    let resolver = db.resolver();

    // 1. Heading target
    let target_heading = Target::Note {
        path_or_name: "doc".to_string(),
        heading_or_block: Some("#Introduction".to_string()),
        alias: None,
    };
    let res_h = resolver.resolve(source_id, &target_heading);
    assert_eq!(
        res_h,
        Resolution::Found(target_id, Some("introduction".to_string()))
    );

    // 2. Block target
    let target_block = Target::Note {
        path_or_name: "doc".to_string(),
        heading_or_block: Some("#^quote-123".to_string()),
        alias: None,
    };
    let res_b = resolver.resolve(source_id, &target_block);
    assert_eq!(
        res_b,
        Resolution::Found(target_id, Some("^quote-123".to_string()))
    );

    // 3. Nonexistent heading
    let target_bad_h = Target::Note {
        path_or_name: "doc".to_string(),
        heading_or_block: Some("#Nonexistent".to_string()),
        alias: None,
    };
    let res_bad_h = resolver.resolve(source_id, &target_bad_h);
    assert_eq!(res_bad_h, Resolution::Found(target_id, None));
}

#[test]
fn repeated_link_occurrences() {
    let mut db = IndexDatabase::open_in_memory().expect("open in memory db");

    let target_id = FileId(1);
    let target_facts = MarkdownFacts::extract("# Target\nBody").unwrap();
    db.index_file(
        target_id,
        "target.md",
        "Target",
        "fp1",
        1,
        &target_facts,
        "Body",
    )
    .unwrap();

    // Source document contains two links to the same note at different ranges
    let source_text = "Here is [[target]] and later here is [[target|alias]].";
    let source_facts = MarkdownFacts::extract(source_text).unwrap();
    assert_eq!(source_facts.links.len(), 2);

    let source_id = FileId(2);
    db.index_file(
        source_id,
        "source.md",
        "Source",
        "fp2",
        1,
        &source_facts,
        source_text,
    )
    .unwrap();

    let backlinks = db.get_backlinks(target_id, None, 0, 100).unwrap();
    assert_eq!(backlinks.total_matches, 2);
    assert_eq!(backlinks.occurrences.len(), 2);

    let occ0 = &backlinks.occurrences[0];
    let occ1 = &backlinks.occurrences[1];

    assert_eq!(occ0.source_file_id, source_id);
    assert_eq!(occ0.occurrence_ordinal, 0);
    assert_eq!(occ0.raw_target, "target");
    assert_eq!(occ0.label, None);

    assert_eq!(occ1.source_file_id, source_id);
    assert_eq!(occ1.occurrence_ordinal, 1);
    assert_eq!(occ1.raw_target, "target");
    assert_eq!(occ1.label, Some("alias".to_string()));

    // Different source ranges
    assert_ne!(occ0.range, occ1.range);
}

#[test]
fn stale_search_query() {
    let mut session = SearchSession::new();

    // User types query 1
    let q1: SearchQuery = session.next_query("rust", 0, 100);
    assert_eq!(q1.id, 1);

    // User rapidly types query 2 before query 1 completes
    let q2: SearchQuery = session.next_query("rust async", 0, 100);
    assert_eq!(q2.id, 2);

    // Query 1 finishes late and returns a page
    let page1 = truff_store::index::SearchPage {
        query_id: q1.id,
        page: 0,
        total_matches: 1,
        results: vec![truff_store::index::SearchResult {
            file_id: FileId(10),
            path: "doc1.md".to_string(),
            title: "Doc 1".to_string(),
            snippet: "rust".to_string(),
        }],
    };

    // Stale page from q1 should be ignored (None)
    assert!(session.accept_page(page1).is_none());

    // Query 2 finishes and returns a page
    let page2 = truff_store::index::SearchPage {
        query_id: q2.id,
        page: 0,
        total_matches: 1,
        results: vec![truff_store::index::SearchResult {
            file_id: FileId(10),
            path: "doc1.md".to_string(),
            title: "Doc 1".to_string(),
            snippet: "rust async".to_string(),
        }],
    };

    // Up-to-date page from q2 is accepted
    let accepted = session.accept_page(page2);
    assert!(accepted.is_some());
    assert_eq!(accepted.unwrap().query_id, q2.id);
}

#[test]
fn dirty_facts_replace_disk() {
    let mut db = IndexDatabase::open_in_memory().expect("open in memory db");

    let target_id = FileId(1);
    let target_facts = MarkdownFacts::extract("# Target\nBody").unwrap();
    db.index_file(
        target_id,
        "target.md",
        "Target",
        "fp1",
        1,
        &target_facts,
        "Body",
    )
    .unwrap();

    let source_id = FileId(2);
    // Disk version has 1 link to target
    let disk_text = "Disk version linking [[target]].";
    let disk_facts = MarkdownFacts::extract(disk_text).unwrap();
    db.index_file(
        source_id,
        "source.md",
        "Source",
        "fp2",
        1,
        &disk_facts,
        disk_text,
    )
    .unwrap();

    // Verify disk backlinks has 1 match
    let backlinks_disk = db.get_backlinks(target_id, None, 0, 100).unwrap();
    assert_eq!(backlinks_disk.total_matches, 1);

    // Now user edits source.md in active editor: adds a second link and changes text
    let dirty_text = "Dirty version linking [[target]] and again [[target|new_link]].";
    let dirty_facts = MarkdownFacts::extract(dirty_text).unwrap();
    assert_eq!(dirty_facts.links.len(), 2);

    // Set dirty overlay for source.md
    db.set_dirty_overlay(source_id, "source.md".to_string(), dirty_facts);

    // Query backlinks: dirty overlay MUST replace disk version without duplication!
    let backlinks_dirty = db.get_backlinks(target_id, None, 0, 100).unwrap();
    assert_eq!(backlinks_dirty.total_matches, 2);
    assert_eq!(backlinks_dirty.occurrences.len(), 2);
    assert_eq!(backlinks_dirty.occurrences[0].occurrence_ordinal, 0);
    assert_eq!(backlinks_dirty.occurrences[1].occurrence_ordinal, 1);
    assert_eq!(
        backlinks_dirty.occurrences[1].label,
        Some("new_link".to_string())
    );

    // Clearing dirty overlay restores disk version
    db.clear_dirty_overlay(source_id);
    let backlinks_cleared = db.get_backlinks(target_id, None, 0, 100).unwrap();
    assert_eq!(backlinks_cleared.total_matches, 1);
}
