use truff_core::markdown::{IncrementalSyntaxTree, presentation::ListKind};

#[test]
fn presentation_uses_real_list_depth_and_excludes_fenced_code() {
    let text = "1. parent\n   - child\n     - [X] done\n\n```md\n- [ ] literal\n```\n";
    let snapshot = IncrementalSyntaxTree::parse_fresh(text).snapshot();
    let mut start = 0;
    let lines: Vec<_> = text
        .split_inclusive('\n')
        .map(|line| {
            let range = start..start + line.trim_end_matches('\n').len();
            start += line.len();
            snapshot.line_presentation(range)
        })
        .collect();
    assert_eq!(lines[0].list.as_ref().unwrap().kind, ListKind::Ordered);
    assert_eq!(lines[1].list.as_ref().unwrap().depth, 1);
    assert_eq!(lines[2].list.as_ref().unwrap().depth, 2);
    assert!(matches!(
        lines[2].list.as_ref().unwrap().kind,
        ListKind::Task { checked: true, .. }
    ));
    assert!(lines[5].code);
    assert!(lines[5].list.is_none());
}

#[test]
fn incremental_block_context_matches_fresh_parse_after_fence_edits() {
    let mut old = crop::Rope::from("before\n- child\n");
    let mut parser = IncrementalSyntaxTree::new();
    parser.init_rope(&old);
    for (range, replacement) in [(0..6, "```md"), (0..5, "before"), (0..0, "- [ ] task\n")] {
        let mut new = old.clone();
        new.replace(range.clone(), replacement);
        parser.replace_rope_range(
            &old,
            &new,
            range.clone(),
            range.start..range.start + replacement.len(),
        );
        assert_eq!(
            parser.snapshot().syntax_tree_sexp(),
            IncrementalSyntaxTree::parse_fresh(&new.to_string())
                .snapshot()
                .syntax_tree_sexp()
        );
        old = new;
    }
}
