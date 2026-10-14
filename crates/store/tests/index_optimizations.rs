use std::time::{SystemTime, UNIX_EPOCH};

use truff_core::markdown::facts::MarkdownFacts;
use truff_store::index::{FileId, IndexDatabase, Resolution, SearchQuery, Target};

#[test]
fn legacy_index_migrates_fts_rowids_and_filename_keys_without_losing_content() {
    let root = std::env::temp_dir().join(format!(
        "truff_index_migration_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("index.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(include_str!("../src/index/schema.sql"))
        .unwrap();
    conn.execute_batch(
        "INSERT INTO files VALUES (42, 'folder/target.md', 'Legacy', 'fp', 1);
        INSERT INTO notes_fts(rowid, file_id, title, body) VALUES (7, 42, 'Legacy', 'alpha');",
    )
    .unwrap();
    drop(conn);
    let mut db = IndexDatabase::open(&path).unwrap();
    let rowid: i64 = db
        .connection()
        .query_row("SELECT rowid FROM notes_fts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rowid, 42);
    assert_eq!(
        db.resolver().resolve(FileId(0), &Target::parse("target")),
        Resolution::Found(FileId(42), None)
    );
    let query = |text: &str| SearchQuery {
        id: 1,
        text: text.into(),
        page: 0,
        page_size: 100,
    };
    assert_eq!(db.search(&query("alpha")).unwrap().total_matches, 1);
    let facts = MarkdownFacts::extract("beta").unwrap();
    db.index_file(
        FileId(42),
        "new/changed.md",
        "Changed",
        "fp2",
        2,
        &facts,
        "beta",
    )
    .unwrap();
    assert_eq!(db.search(&query("alpha")).unwrap().total_matches, 0);
    assert_eq!(db.search(&query("beta")).unwrap().total_matches, 1);
    assert_eq!(
        db.resolver().resolve(FileId(0), &Target::parse("target")),
        Resolution::Missing
    );
    assert_eq!(
        db.resolver().resolve(FileId(0), &Target::parse("changed")),
        Resolution::Found(FileId(42), None)
    );
    drop(db);
    let mut db = IndexDatabase::open(&path).unwrap();
    assert_eq!(db.search(&query("beta")).unwrap().total_matches, 1);
    db.remove_file(FileId(42)).unwrap();
    assert_eq!(db.search(&query("beta")).unwrap().total_matches, 0);
    drop(db);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn fts_pages_keep_count_past_end_and_do_not_leak_removed_or_replaced_rows() {
    let mut db = IndexDatabase::open_in_memory().unwrap();
    let facts = MarkdownFacts::extract("alpha").unwrap();
    for id in [10, 20, 30] {
        db.index_file(
            FileId(id),
            &format!("note{id}.md"),
            "Title",
            "fp",
            1,
            &facts,
            "alpha",
        )
        .unwrap();
    }
    let query = |page| SearchQuery {
        id: 9,
        text: "alpha".into(),
        page,
        page_size: 2,
    };
    let first = db.search(&query(0)).unwrap();
    let second = db.search(&query(1)).unwrap();
    assert_eq!((first.total_matches, first.results.len()), (3, 2));
    assert_eq!((second.total_matches, second.results.len()), (3, 1));
    assert_eq!(db.search(&query(50)).unwrap().total_matches, 3);
    assert!(db.search(&query(50)).unwrap().results.is_empty());
    db.remove_file(FileId(20)).unwrap();
    db.index_file(FileId(10), "note10.md", "Title", "fp2", 2, &facts, "beta")
        .unwrap();
    let page = db.search(&query(0)).unwrap();
    assert_eq!(page.total_matches, 1);
    assert_eq!(page.results[0].file_id, FileId(30));
}
