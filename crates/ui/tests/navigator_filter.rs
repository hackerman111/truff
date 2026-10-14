use truff_ui::navigator::{NavigatorItem, NavigatorState};

fn old_filter(state: &NavigatorState) -> Vec<usize> {
    let query = state.query.trim().to_lowercase();
    if query.is_empty() {
        return (0..state.items.len()).collect();
    }

    let mut scored = state
        .items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let title = item.title.to_lowercase();
            let subtitle = item.subtitle.as_deref().unwrap_or("").to_lowercase();
            let category = item.category.as_deref().unwrap_or("").to_lowercase();
            let score = if title == query {
                200
            } else if title.starts_with(&query) {
                100
            } else if title.contains(&query) {
                50
            } else if subtitle.contains(&query) {
                25
            } else if category.contains(&query) {
                15
            } else if subsequence(&query, &title) {
                10
            } else {
                return None;
            };
            Some((idx, score))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    scored.into_iter().map(|(idx, _)| idx).collect()
}

fn subsequence(sub: &str, target: &str) -> bool {
    let mut chars = target.chars();
    sub.chars().all(|c| chars.any(|target| target == c))
}

#[test]
fn borrowed_lowercase_filter_matches_previous_algorithm() {
    let titles = [
        "Alpha.md",
        "Русский.md",
        "Σ.md",
        "ΟΣ.md",
        "οσ.md",
        "ος.md",
        "İ.md",
        "i\u{307}.md",
        "Other.md",
    ];
    let mut state = NavigatorState::new();
    state.items = titles
        .iter()
        .enumerate()
        .map(|(id, title)| {
            NavigatorItem::new_file(id as u64, (*title).into(), format!("dir/{title}"), false, 0)
        })
        .collect();
    state.items[8].subtitle = Some("Subtitle target".into());
    state.items[8].category = Some("Category target".into());

    for query in [
        "",
        "ALPHA",
        "Русский",
        "Σ",
        "ΟΣ",
        "οσ",
        "ος",
        "İ",
        "i\u{307}",
        "subtitle",
        "category",
        "alp",
        "am",
        "missing",
    ] {
        state.set_query(query);
        assert_eq!(
            state.filtered_indices,
            old_filter(&state),
            "query {query:?}"
        );
    }

    state.set_query("renamed");
    state.items[8].title = "Renamed.md".into();
    state.recompute_filter();
    assert_eq!(state.filtered_indices, old_filter(&state));
}
