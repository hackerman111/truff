use std::hint::black_box;
use std::time::Instant;

use truff_core::commands::CommandContext;
use truff_core::config::KeymapConfig;
use truff_core::vim::{Key, Mode};

fn main() {
    let config = KeymapConfig::default();
    let map = config.compile().expect("default keymap compiles");
    let empty = [];
    let g = [Key::Char('g')];

    for iteration in 0..100 {
        let prefix = if iteration % 2 == 0 {
            &empty[..]
        } else {
            &g[..]
        };
        black_box(map.hints(Mode::Normal, prefix, CommandContext::Editor));
    }

    let iterations = 100_000;
    let start = Instant::now();
    for iteration in 0..iterations {
        let prefix = if iteration % 2 == 0 {
            &empty[..]
        } else {
            &g[..]
        };
        black_box(map.hints(Mode::Normal, prefix, CommandContext::Editor));
    }
    let hints_ns = start.elapsed().as_nanos();

    for _ in 0..100 {
        black_box(config.compile().expect("default keymap compiles"));
    }
    let compile_iterations = 1_000;
    let start = Instant::now();
    for _ in 0..compile_iterations {
        black_box(config.compile().expect("default keymap compiles"));
    }
    let compile_ns = start.elapsed().as_nanos();

    println!("scenario,iterations,ns_per_op");
    println!(
        "alternating_empty_g,{iterations},{}",
        hints_ns / iterations as u128
    );
    println!(
        "compiled_keymap,{compile_iterations},{}",
        compile_ns / compile_iterations as u128
    );
}
