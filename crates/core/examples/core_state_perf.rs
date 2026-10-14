use std::hint::black_box;
use std::time::Instant;

use truff_core::{Buffer, Change, ChangeSet, Register, RegisterKind, Registers, Transaction};

fn run(
    name: &str,
    batches: usize,
    ops_per_batch: usize,
    multiplier: usize,
    mut operation: impl FnMut(usize) -> usize,
) -> Result<(), Box<dyn std::error::Error>> {
    for warmup in 0..2 {
        black_box(operation(warmup));
    }
    let batches = batches
        .checked_mul(multiplier)
        .ok_or("iteration overflow")?;
    let iterations = batches
        .checked_mul(ops_per_batch)
        .ok_or("iteration overflow")?;
    let start = Instant::now();
    for iteration in 0..batches {
        black_box(operation(iteration + 2));
    }
    let total_ns = start.elapsed().as_nanos();
    println!(
        "{name},{iterations},{total_ns},{}",
        total_ns / iterations as u128
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let multiplier = std::env::var("TRUFF_PERF_ITERS")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    if multiplier == 0 {
        return Err("TRUFF_PERF_ITERS must be greater than zero".into());
    }
    println!("scenario,iterations,total_ns,ns_per_op");

    let changes: Vec<_> = (0..5_000)
        .map(|index| Change::new((4 * index)..(4 * index), 1))
        .collect();
    run("changeset_new_5000", 10, 1, multiplier, |_| {
        ChangeSet::new(black_box(changes.clone())).len()
    })?;

    let changes = ChangeSet::new(changes);
    let anchors: Vec<_> = (0..512).map(|index| index * 32).collect();
    run(
        "changeset_map_pos_512_anchors",
        1,
        anchors.len(),
        multiplier,
        |_| {
            anchors.iter().fold(0usize, |sum, &anchor| {
                sum.wrapping_add(black_box(changes.map_pos(black_box(anchor))))
            })
        },
    )?;

    let mut buffer = Buffer::from_str("");
    buffer.history_mut().set_max_bytes(128 * 1024);
    run("history_buffer_insert0", 1, 3_000, multiplier, |_| {
        for _ in 0..3_000 {
            black_box(
                buffer
                    .apply(Transaction::insert(0, "x"))
                    .expect("valid insertion"),
            );
        }
        buffer.history().total_bytes()
    })?;

    let original = Register::new("x".repeat(64 * 1024), RegisterKind::Characterwise);
    let mut registers = Registers::new();
    run("register_set_named_a_64k", 100, 1, multiplier, |_| {
        registers.set(Some('a'), black_box(original.clone()));
        black_box(
            registers
                .get(Some('a'))
                .expect("register was set")
                .text
                .len(),
        )
    })?;
    Ok(())
}
