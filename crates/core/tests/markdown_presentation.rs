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

#[test]
fn over_indented_tab_nested_lists_and_tasks_fence_are_recognized_as_lists() {
    let text = "- Реализовать\n\t- Что такое\n    \t- Почему\n    \t\t\t- Смысл\n\n```tasks\n- [ ] first\n- [/] second\n- [x] third\n```\n";
    let rope = crop::Rope::from(text);
    let snapshot = IncrementalSyntaxTree::parse_fresh(text).snapshot();
    let mut start = 0;
    let lines: Vec<_> = text
        .split_inclusive('\n')
        .map(|line| {
            let range = start..start + line.trim_end_matches('\n').len();
            start += line.len();
            snapshot.line_presentation_in_rope(&rope, range)
        })
        .collect();

    assert_eq!(lines[0].list.as_ref().unwrap().depth, 0);
    assert_eq!(lines[1].list.as_ref().unwrap().depth, 1);
    assert_eq!(lines[2].list.as_ref().unwrap().depth, 2);
    assert!(
        !lines[3].code,
        "Over-indented list item should not be treated as code"
    );
    assert_eq!(lines[3].list.as_ref().unwrap().depth, 3);

    // Service ```tasks fence lines are marked as task_fence and not rendered as code
    assert!(lines[5].task_fence);
    assert!(!lines[5].code);
    assert!(lines[9].task_fence);
    assert!(!lines[9].code);
    // Tasks inside ```tasks fence are recognized as task list items, not raw code
    assert!(!lines[6].code);
    assert!(lines[6].in_tasks_block);
    assert!(matches!(
        lines[6].list.as_ref().unwrap().kind,
        ListKind::Task { checked: false, .. }
    ));
    assert!(!lines[7].code);
    assert!(lines[7].in_tasks_block);
    assert!(matches!(
        lines[7].list.as_ref().unwrap().kind,
        ListKind::Task { checked: false, .. }
    ));
    assert!(!lines[8].code);
    assert!(lines[8].in_tasks_block);
    assert!(matches!(
        lines[8].list.as_ref().unwrap().kind,
        ListKind::Task { checked: true, .. }
    ));
}

#[test]
fn test_presentation_recognizes_colored_and_highlighted_lists() {
    let text = "<span style=\"color:#3388ff\">- blue</span>\n<mark style=\"background-color:#ffff00\">- [ ] highlighted task</mark>\n- <span style=\"color:#ff0000\">red item</span>\n";
    let snapshot = IncrementalSyntaxTree::parse_fresh(text).snapshot();
    let mut start = 0;
    let lines: Vec<_> = text
        .split_inclusive('\n')
        .map(|line| {
            let range = start..start + line.trim_end_matches('\n').len();
            start += line.len();
            snapshot.line_presentation_in_str(text, range)
        })
        .collect();

    assert!(lines[0].list.is_some());
    assert_eq!(lines[0].list.as_ref().unwrap().kind, ListKind::Bullet);

    assert!(lines[1].list.is_some());
    assert!(matches!(
        lines[1].list.as_ref().unwrap().kind,
        ListKind::Task { checked: false, .. }
    ));

    assert!(lines[2].list.is_some());
    assert_eq!(lines[2].list.as_ref().unwrap().kind, ListKind::Bullet);
}

#[test]
fn test_tab_nested_lists_render_correct_depth() {
    // 1. Two tabs on a second item without intermediate 4-space item
    let text = "- first\n        - second\n";
    let snapshot = IncrementalSyntaxTree::parse_fresh(text).snapshot();
    let l1 = snapshot.line_presentation_in_str(text, 0..7);
    let l2 = snapshot.line_presentation_in_str(text, 8..text.len());
    assert_eq!(l1.list.as_ref().unwrap().depth, 0);
    assert_eq!(
        l2.list.as_ref().unwrap().depth,
        2,
        "8 spaces must be recognized as depth 2"
    );

    let text_trailing_empty = "- item 1\n    - ";
    let snap_empty = IncrementalSyntaxTree::parse_fresh(text_trailing_empty).snapshot();
    let l_empty =
        snap_empty.line_presentation_in_str(text_trailing_empty, 9..text_trailing_empty.len());
    assert_eq!(l_empty.list.as_ref().unwrap().depth, 1);
    assert_eq!(l_empty.list.as_ref().unwrap().range, 13..15);

    // 2. Single item indented by 1 tab (4 spaces) at start of doc
    let text_tab1 = "    - standalone\n";
    let snap_tab1 = IncrementalSyntaxTree::parse_fresh(text_tab1).snapshot();
    let l_tab1 = snap_tab1.line_presentation_in_str(text_tab1, 0..text_tab1.len());
    assert!(
        !l_tab1.code,
        "Indented list item must not be treated as code"
    );
    assert_eq!(l_tab1.list.as_ref().unwrap().depth, 1);

    // 3. Single item indented by 2 tabs (8 spaces) at start of doc
    let text_tab2 = "        - standalone\n";
    let snap_tab2 = IncrementalSyntaxTree::parse_fresh(text_tab2).snapshot();
    let l_tab2 = snap_tab2.line_presentation_in_str(text_tab2, 0..text_tab2.len());
    assert!(
        !l_tab2.code,
        "8-space indented list item must not be treated as code"
    );
    assert_eq!(l_tab2.list.as_ref().unwrap().depth, 2);
}

#[test]
fn test_colored_list_tab_indentation_and_presentation() {
    use truff_core::markdown::plan_tab;
    use truff_core::{Buffer, BytePos};

    // 1. Line 1 colored, Line 2 plain -> press Tab on Line 2
    let text = "<span style=\"color:#BF616A\">- item 1</span>\n- item 2\n";
    let buffer = Buffer::from_str(text);
    let cursor = BytePos::new(text.find("- item 2").unwrap());
    let tx = plan_tab(&buffer, cursor).expect("Tab on item 2 below colored item 1 must succeed");
    let mut b_after = Buffer::from_str(text);
    b_after.apply(tx).unwrap();
    assert_eq!(
        b_after.as_str(),
        "<span style=\"color:#BF616A\">- item 1</span>\n    - item 2\n"
    );

    let s_after = b_after.as_str();
    let snap = IncrementalSyntaxTree::parse_fresh(&s_after).snapshot();
    let l2 = snap.line_presentation_in_str(
        &s_after,
        s_after.find("    - item 2").unwrap()..s_after.len(),
    );
    assert!(
        !l2.code,
        "Lower list item below colored item must not be treated as code"
    );
    assert!(
        l2.list.is_some(),
        "Lower list item below colored item must be recognized as list"
    );
    assert_eq!(
        l2.list.as_ref().unwrap().depth,
        1,
        "Lower list item indented with 4 spaces must have depth 1"
    );

    // 2. Both items colored -> press Tab on Line 2
    let text_both = "<span style=\"color:#BF616A\">- item 1</span>\n<span style=\"color:#BF616A\">- item 2</span>\n";
    let buffer_both = Buffer::from_str(text_both);
    let cursor_both = BytePos::new(text_both.rfind("<span").unwrap());
    let tx_both = plan_tab(&buffer_both, cursor_both).expect("Tab on colored item 2 must succeed");
    let mut b_both_after = Buffer::from_str(text_both);
    b_both_after.apply(tx_both).unwrap();
    assert_eq!(
        b_both_after.as_str(),
        "<span style=\"color:#BF616A\">- item 1</span>\n    <span style=\"color:#BF616A\">- item 2</span>\n"
    );

    let s_both_after = b_both_after.as_str();
    let snap_both = IncrementalSyntaxTree::parse_fresh(&s_both_after).snapshot();
    let l2_both = snap_both.line_presentation_in_str(
        &s_both_after,
        s_both_after.rfind("    <span").unwrap()..s_both_after.len(),
    );
    assert!(!l2_both.code);
    assert!(
        l2_both.list.is_some(),
        "Indented colored list item must be recognized as list"
    );
    assert_eq!(l2_both.list.as_ref().unwrap().depth, 1);
}
