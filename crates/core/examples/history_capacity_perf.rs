use std::hint::black_box;
use std::time::Instant;

use truff_core::{Buffer, Transaction};

const ITERATIONS: usize = 1000;

fn oversized_transaction() -> Transaction {
    let mut text = String::with_capacity(1024 * 1024);
    text.push_str("字abc");
    Transaction::insert(0, text)
}

fn ordinary_transaction() -> Transaction {
    Transaction::insert(0, String::from("ordinary"))
}

fn run(
    name: &str,
    make_transaction: fn() -> Transaction,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut total_ns = 0_u128;
    let mut history_bytes = 0;
    for _ in 0..ITERATIONS {
        let mut buffer = Buffer::from_str("");
        let transaction = make_transaction();
        let start = Instant::now();
        black_box(buffer.apply(transaction)?);
        history_bytes = black_box(buffer.history().total_bytes());
        total_ns += start.elapsed().as_nanos();
    }
    println!(
        "{name},{ITERATIONS},{total_ns},{},{history_bytes}",
        total_ns / ITERATIONS as u128
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("scenario,iterations,total_ns,ns_per_op,history_bytes");
    run("oversized_capacity_insert", oversized_transaction)?;
    run("ordinary_insert", ordinary_transaction)?;
    Ok(())
}
