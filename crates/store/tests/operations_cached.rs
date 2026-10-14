use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use truff_core::markdown::facts::MarkdownFacts;
use truff_store::VaultPaths;
use truff_store::files::compute_fingerprint;
use truff_store::index::{FileId, IndexDatabase};
use truff_store::operations::FileOpPlan;

#[test]
fn indexed_rewrites_and_stale_source_fallback_preserve_occurrences_and_self_links() {
    for stale in [false, true] {
        let root = std::env::temp_dir().join(format!(
            "truff-cached-rewrites-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let paths = VaultPaths::new(root.clone());
        let mut db = IndexDatabase::open_in_memory().unwrap();
        let original = "# Intro\n[[a]] [[a#Intro|é]] [label](a.md) [fragment](a.md#Intro) `[[a]]`\n```\n[[a]]\n```\n";
        for (id, path) in [(1, "a.md"), (2, "source.md")] {
            fs::write(root.join(path), original).unwrap();
            db.index_file(
                FileId(id),
                path,
                path,
                &compute_fingerprint(original.as_bytes()),
                1,
                &MarkdownFacts::extract(original).unwrap(),
                original,
            )
            .unwrap();
        }
        let prefix = if stale { "前置き\n" } else { "" };
        fs::write(root.join("source.md"), format!("{prefix}{original}")).unwrap();
        let mut plan = FileOpPlan::rename(&paths, "a.md", "renamed.md").unwrap();
        plan.validate(&paths, &[], &db).unwrap();
        assert_eq!(plan.rewrites.len(), 2);
        plan.stage(&paths, db.connection_mut()).unwrap();
        plan.apply(&paths, db.connection_mut()).unwrap();
        let expected = "# Intro\n[[renamed]] [[renamed#Intro|é]] [label](renamed.md) [fragment](a.md#Intro) `[[a]]`\n```\n[[a]]\n```\n";
        assert_eq!(
            fs::read_to_string(root.join("renamed.md")).unwrap(),
            expected
        );
        assert_eq!(
            fs::read_to_string(root.join("source.md")).unwrap(),
            format!("{prefix}{expected}")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
