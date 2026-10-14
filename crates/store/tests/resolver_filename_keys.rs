use std::path::Path;

use truff_core::markdown::facts::DocFacts;
use truff_store::index::{FileId, IndexDatabase, Resolution, Target};

#[test]
fn basename_lookup_matches_previous_path_fallback() {
    let paths = [
        (10, "folder_a/a.md"),
        (11, "folder_b/a.md"),
        (12, "space dir/file.a.md"),
        (13, "folder_c/a.txt"),
        (14, "folder_d/a.md.md"),
        (15, "folder_e/.hidden.md"),
        (16, "папка/Журнал заметок.md"),
        (17, "folder_g/Case.MD"),
        (18, "folder_h/source.md"),
        (19, "folder_h/a.md"),
    ];
    let mut db = IndexDatabase::open_in_memory().unwrap();
    for (id, path) in paths {
        db.index_file(FileId(id), path, "", "fp", 1, &DocFacts::default(), "")
            .unwrap();
    }

    let queries = [
        "a",
        "a.md",
        "a.md.md",
        ".hidden",
        "file.a",
        "Журнал заметок",
        "Case",
        "Case.MD",
        "отсутствует",
        "folder/nonexistent",
    ];
    for query in queries {
        let with_md = if query.ends_with(".md") {
            query.to_string()
        } else {
            format!("{query}.md")
        };
        let target_stem = query.strip_suffix(".md").unwrap_or(query);
        let exact = paths
            .iter()
            .filter(|(_, path)| *path == query || *path == with_md)
            .min_by_key(|(id, _)| id);
        let expected = if let Some((id, _)) = exact {
            Resolution::Found(FileId(*id), None)
        } else {
            let ids: Vec<_> = paths
                .iter()
                .filter(|(_, path)| {
                    let path = Path::new(path);
                    let stem = path
                        .file_stem()
                        .and_then(|part| part.to_str())
                        .unwrap_or("");
                    let name = path
                        .file_name()
                        .and_then(|part| part.to_str())
                        .unwrap_or("");
                    stem == target_stem || stem == query || name == query || name == with_md
                })
                .map(|(id, _)| FileId(*id))
                .collect();
            match ids.as_slice() {
                [] => Resolution::Missing,
                [id] => Resolution::Found(*id, None),
                _ => Resolution::Ambiguous(ids),
            }
        };
        assert_eq!(
            db.resolver().resolve(FileId(0), &Target::parse(query)),
            expected,
            "{query}"
        );
    }

    assert_eq!(
        db.resolver().resolve(FileId(18), &Target::parse("a")),
        Resolution::Found(FileId(19), None)
    );
    assert_eq!(
        db.resolver().resolve(FileId(18), &Target::parse("a.md")),
        Resolution::Found(FileId(19), None)
    );
}
