use std::hint::black_box;
use std::time::Instant;

use truff_core::config::KeymapConfig;
use truff_core::markdown::{IncrementalSyntaxTree, MarkdownFacts};
use truff_core::vim::Motion;
use truff_core::vim::motion::{eval_motion, literal_search};
use truff_core::{Buffer, BytePos};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filter = std::env::args().nth(1);
    let multiplier = std::env::var("TRUFF_PERF_ITERS")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    if multiplier == 0 {
        return Err("TRUFF_PERF_ITERS must be greater than zero".into());
    }

    println!("scenario,iterations,total_ns,ns_per_op");
    let run = |name: &str, base_iterations: usize, operation: &mut dyn FnMut(usize) -> usize| {
        if filter.as_deref().is_some_and(|selected| selected != name) {
            return;
        }

        for warmup in 0..2 {
            black_box(operation(warmup));
        }
        let iterations = base_iterations.saturating_mul(multiplier);
        let start = Instant::now();
        for iteration in 0..iterations {
            black_box(operation(iteration + 2));
        }
        let total_ns = start.elapsed().as_nanos();
        println!(
            "{name},{iterations},{total_ns},{}",
            total_ns / iterations as u128
        );
    };

    let search_text = "word alpha beta\n".repeat(96 * 1024);
    let search_buffer = Buffer::from_str(&search_text);
    let search_pos = search_buffer.byte_len().saturating_sub(32);
    run("literal_search_forward_1", 1, &mut |_| {
        literal_search(&search_buffer, search_pos, "alpha", true, 1)
            .expect("search fixture contains a forward match")
    });
    run("literal_search_backward_50", 1, &mut |_| {
        literal_search(&search_buffer, search_pos, "alpha", false, 50)
            .expect("search fixture contains 50 backward matches")
    });

    let mut markdown = String::with_capacity(160_000);
    for _ in 0..5_000 {
        markdown.push_str("`code [[Hidden]] #tag` [[Visible]]\n");
    }
    run("markdown_facts_5000_code_ranges", 1, &mut |_| {
        MarkdownFacts::extract(&markdown)
            .expect("Markdown benchmark fixture parses")
            .links
            .len()
    });

    run("keymap_default_validate", 100, &mut |_| {
        let config = KeymapConfig::default();
        usize::from(config.validate().is_ok())
    });

    let motion_text = "x".repeat(10_000);
    let motion_buffer = Buffer::from_str(&motion_text);
    let motion_pos = BytePos::new(5_000);
    run("eval_motion_left_2000", 100, &mut |_| {
        eval_motion(&motion_buffer, motion_pos, &Motion::Left, 2_000, None)
            .0
            .get()
    });
    run("eval_motion_right_2000", 100, &mut |_| {
        eval_motion(&motion_buffer, motion_pos, &Motion::Right, 2_000, None)
            .0
            .get()
    });

    let syntax_initial = format!("{}a", "paragraph text\n".repeat(625));
    let syntax_alternate = format!("{}b", "paragraph text\n".repeat(625));
    let edit_start = syntax_initial.len() - 1;
    let initial_rope = crop::Rope::from(syntax_initial.as_str());
    let alternate_rope = crop::Rope::from(syntax_alternate.as_str());
    let mut syntax_tree = IncrementalSyntaxTree::parse_fresh(&syntax_initial);
    run("incremental_syntax_end_edit", 100, &mut |iteration| {
        let rope = if iteration % 2 == 0 {
            &alternate_rope
        } else {
            &initial_rope
        };
        let row = initial_rope.line_of_byte(edit_start);
        let column = edit_start - initial_rope.byte_of_line(row);
        let edit = tree_sitter::InputEdit {
            start_byte: edit_start,
            old_end_byte: edit_start + 1,
            new_end_byte: edit_start + 1,
            start_position: tree_sitter::Point::new(row, column),
            old_end_position: tree_sitter::Point::new(row, column + 1),
            new_end_position: tree_sitter::Point::new(row, column + 1),
        };
        syntax_tree
            .edit_and_parse(edit, rope)
            .root_node()
            .end_byte()
    });

    Ok(())
}
