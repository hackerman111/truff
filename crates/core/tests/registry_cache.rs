use truff_core::commands::{CommandId, CommandRegistry, RegistryError};

fn uncached_search<'a>(
    specs: &'a [truff_core::commands::CommandSpec],
    query: &str,
) -> Vec<&'a truff_core::commands::CommandSpec> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return specs.iter().collect();
    }

    let mut scored = Vec::new();
    for spec in specs {
        let title = spec.title.to_lowercase();
        let description = spec.description.to_lowercase();
        let id = spec.id.as_str().to_lowercase();
        let group = spec.group.to_lowercase();
        let score = if title == query || id == query {
            100
        } else if title.starts_with(&query) {
            80
        } else if title.contains(&query) {
            60
        } else if description.contains(&query) {
            40
        } else if group.contains(&query) || id.contains(&query) {
            20
        } else {
            0
        };
        if score > 0 {
            scored.push((score, spec));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.title.cmp(&b.1.title)));
    scored.into_iter().map(|(_, spec)| spec).collect()
}

#[test]
fn cached_search_matches_uncached_scoring_and_order() {
    let registry = CommandRegistry::default();
    let queries = [
        "",
        "   ",
        "СОХРАНИТЬ",
        "buffer.save",
        "файлы",
        "контекст",
        "panel.",
        "нет такой команды",
        "  note.open  ",
    ];

    for query in queries {
        let actual: Vec<_> = registry
            .search(query)
            .into_iter()
            .map(|spec| spec.id.as_str())
            .collect();
        let expected: Vec<_> = uncached_search(registry.specs(), query)
            .into_iter()
            .map(|spec| spec.id.as_str())
            .collect();
        assert_eq!(actual, expected, "query: {query:?}");
    }
}

#[test]
fn every_registered_id_is_retrievable_and_unknown_id_is_preserved() {
    let registry = CommandRegistry::default();
    for spec in registry.specs() {
        assert_eq!(registry.get(&spec.id).unwrap(), spec);
    }

    let unknown = CommandId::new("missing.command");
    assert_eq!(
        registry.get(&unknown),
        Err(RegistryError::UnknownCommand(unknown))
    );
}
