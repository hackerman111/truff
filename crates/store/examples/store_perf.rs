use std::hint::black_box;
use std::time::Instant;

use truff_core::markdown::facts::MarkdownFacts;
use truff_store::index::{FileId, IndexDatabase, Resolution, SearchQuery, Target};

fn run(
    name: &str,
    iterations: usize,
    mut operation: impl FnMut() -> Result<usize, Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    for _ in 0..2 {
        black_box(operation()?);
    }
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(operation()?);
    }
    let elapsed = start.elapsed().as_nanos();
    println!(
        "{name},{iterations},{elapsed},{}",
        elapsed / iterations as u128
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let multiplier = std::env::var("TRUFF_PERF_ITERS")
        .ok()
        .map(|v| v.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    if multiplier == 0 {
        return Err("TRUFF_PERF_ITERS must be positive".into());
    }
    let mut db = IndexDatabase::open_in_memory()?;
    let body = "# Heading\nalpha beta gamma\n";
    let facts = MarkdownFacts::extract(body)?;
    for id in 1..=1000 {
        let path = format!("folder/note{id}.md");
        db.index_file(FileId(id), &path, &path, "fp", 1, &facts, body)?;
    }
    if std::env::args().any(|arg| arg == "--probe-window") {
        let result = db.connection().query_row(
            "SELECT snippet(notes_fts, 2, '<b>', '</b>', '...', 32), count(*) OVER()
             FROM notes_fts JOIN files f ON f.id = notes_fts.rowid
             WHERE notes_fts MATCH 'alpha' ORDER BY rank LIMIT 100",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        );
        println!("window query: {result:?}");
        return Ok(());
    }
    println!("scenario,iterations,total_ns,ns_per_op");
    let target = Target::parse("note999");
    run("resolve_basename_1000", 500 * multiplier, || {
        match db.resolver().resolve(FileId(0), black_box(&target)) {
            Resolution::Found(id, _) => Ok(id.0 as usize),
            other => Err(format!("unexpected resolution: {other:?}").into()),
        }
    })?;
    let query = SearchQuery {
        id: 1,
        text: "alpha".into(),
        page: 0,
        page_size: 100,
    };
    run("fts_search_page_1000", 100 * multiplier, || {
        let page = db.search(black_box(&query))?;
        assert_eq!(page.total_matches, 1000);
        Ok(page.results.len())
    })?;
    run("reindex_single_in_1000", 100 * multiplier, || {
        db.index_file(
            FileId(500),
            "folder/note500.md",
            "Note",
            "fp",
            1,
            &facts,
            body,
        )?;
        Ok(black_box(db.connection().last_insert_rowid()) as usize)
    })?;
    let links = "[[note999]] ".repeat(100);
    let link_facts = MarkdownFacts::extract(&links)?;
    run("index_100_basename_links", 5 * multiplier, || {
        db.index_file(
            FileId(1001),
            "source.md",
            "Source",
            "fp",
            1,
            &link_facts,
            &links,
        )?;
        Ok(black_box(db.connection().last_insert_rowid()) as usize)
    })?;
    Ok(())
}
