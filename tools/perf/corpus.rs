//! Standalone test corpus generator for Truff v2 performance benchmarks.
//!
//! Uses ONLY `std` so it can be compiled directly via:
//! `rustc --edition=2024 -O tools/perf/corpus.rs -o /tmp/truff-corpus`
//!
//! Accepts arguments:
//!   `--out <DIR>`: Target directory for the vault
//!   `--notes <COUNT>`: Number of notes to generate (default: 10000)
//!   `--bytes <BYTES>`: Approximate size per note in bytes (default: 4096)
//!   `--seed <SEED>`: Deterministic PRNG seed (default: 20261010)

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

/// Simple deterministic 64-bit PRNG (SplitMix64).
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u64;
        min + (self.next_u64() % span) as usize
    }

    fn choose<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
        let idx = self.next_range(0, slice.len() - 1);
        &slice[idx]
    }
}

const WORDS: &[&str] = &[
    "architecture", "performance", "memory", "latency", "benchmark",
    "markdown", "buffer", "editor", "rope", "transaction", "snapshot",
    "indexing", "sqlite", "fts5", "tree-sitter", "parley", "vello",
    "заметка", "производительность", "текст", "архитектура", "память",
    "быстродействие", "структура", "поиск", "ссылка", "хранилище",
];

const HEADINGS: &[&str] = &[
    "Введение и постановка задачи",
    "Архитектурные ограничения",
    "Анализ потребления памяти",
    "Результаты измерений",
    "План оптимизации",
    "Detailed Architecture Overview",
    "System Requirements and Guarantees",
    "Benchmark Summary and Metrics",
];

fn generate_note_content(rng: &mut Rng, note_id: usize, target_bytes: usize) -> String {
    let mut out = String::with_capacity(target_bytes + 256);

    let title = *rng.choose(HEADINGS);
    out.push_str(&format!("# Note {note_id:06}: {title}\n\n"));
    out.push_str(&format!("Created with seed-based generator. ID: `{note_id}`.\n\n"));

    while out.len() < target_bytes {
        let section_type = rng.next_range(0, 5);
        match section_type {
            0 => {
                // Section heading
                let h = *rng.choose(HEADINGS);
                out.push_str(&format!("## {h}\n\n"));
            }
            1 => {
                // Paragraph with Cyrillic, English, and emojis
                let word_count = rng.next_range(15, 40);
                for i in 0..word_count {
                    if i > 0 {
                        out.push(' ');
                    }
                    out.push_str(*rng.choose(WORDS));
                }
                if rng.next_range(0, 3) == 0 {
                    out.push_str(" 🦀🚀✨");
                }
                out.push_str(".\n\n");
            }
            2 => {
                // Bullet list with wikilinks
                let items = rng.next_range(3, 7);
                for _ in 0..items {
                    let target_note = rng.next_range(0, note_id + 10);
                    let related_word = *rng.choose(WORDS);
                    out.push_str(&format!("- Related reference: [[Note {target_note:06}]] and `{related_word}`\n"));
                }
                out.push('\n');
            }
            3 => {
                // Code block
                out.push_str("```rust\n");
                out.push_str("fn verify_invariants(buffer: &Buffer) -> bool {\n");
                out.push_str("    buffer.is_valid() && buffer.revision() > 0\n");
                out.push_str("}\n");
                out.push_str("```\n\n");
            }
            4 => {
                // Callout block (Obsidian style)
                out.push_str("> [!note] Ключевой инвариант\n");
                out.push_str("> Markdown-файлы являются единственным источником истины.\n\n");
            }
            _ => {
                out.push_str("Дополнительные сведения о структуре данных.\n\n");
            }
        }
    }

    out
}

struct Config {
    out_dir: PathBuf,
    notes: usize,
    bytes: usize,
    seed: u64,
}

fn parse_args() -> Result<Config, String> {
    let mut args = std::env::args().skip(1);
    let mut out_dir = None;
    let mut notes = 10_000;
    let mut bytes = 4_096;
    let mut seed = 20261010;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                let val = args.next().ok_or("Missing value for --out")?;
                out_dir = Some(PathBuf::from(val));
            }
            "--notes" => {
                let val = args.next().ok_or("Missing value for --notes")?;
                notes = val.parse().map_err(|e| format!("Invalid --notes: {e}"))?;
            }
            "--bytes" => {
                let val = args.next().ok_or("Missing value for --bytes")?;
                bytes = val.parse().map_err(|e| format!("Invalid --bytes: {e}"))?;
            }
            "--seed" => {
                let val = args.next().ok_or("Missing value for --seed")?;
                seed = val.parse().map_err(|e| format!("Invalid --seed: {e}"))?;
            }
            "--help" | "-h" => {
                println!("Usage: corpus --out <DIR> [--notes <N>] [--bytes <B>] [--seed <S>]");
                std::process::exit(0);
            }
            other => return Err(format!("Unknown argument: {other}")),
        }
    }

    let out_dir = out_dir.ok_or("Required argument --out was not provided")?;
    Ok(Config {
        out_dir,
        notes,
        bytes,
        seed,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!("Run with --help for usage information.");
            std::process::exit(1);
        }
    };

    // Refuse to overwrite non-empty target directory.
    if cfg.out_dir.exists() {
        let mut entries = fs::read_dir(&cfg.out_dir)?;
        if entries.next().is_some() {
            eprintln!(
                "Error: Output directory '{}' exists and is not empty. Refusing to overwrite.",
                cfg.out_dir.display()
            );
            std::process::exit(1);
        }
    } else {
        fs::create_dir_all(&cfg.out_dir)?;
    }

    println!(
        "Generating Truff corpus: notes={}, bytes_per_note={}, seed={}, out='{}'",
        cfg.notes,
        cfg.bytes,
        cfg.seed,
        cfg.out_dir.display()
    );

    let mut rng = Rng::new(cfg.seed);
    let mut total_bytes = 0usize;

    // Distribute into hierarchical folders (up to 20 folders)
    let num_folders = (cfg.notes / 500).clamp(1, 20);
    for i in 0..num_folders {
        let folder = cfg.out_dir.join(format!("folder_{i:02}"));
        fs::create_dir_all(&folder)?;
    }

    for note_id in 0..cfg.notes {
        let folder_id = note_id % num_folders;
        let filename = format!("note_{note_id:06}.md");
        let file_path = cfg.out_dir.join(format!("folder_{folder_id:02}")).join(filename);

        let content = generate_note_content(&mut rng, note_id, cfg.bytes);
        total_bytes += content.len();

        let mut file = File::create(&file_path)?;
        file.write_all(content.as_bytes())?;

        if (note_id + 1) % 2500 == 0 || note_id + 1 == cfg.notes {
            println!("  Generated {}/{} notes...", note_id + 1, cfg.notes);
        }
    }

    println!(
        "Corpus generation complete: {} notes written, total {:.2} MiB.",
        cfg.notes,
        total_bytes as f64 / (1024.0 * 1024.0)
    );

    Ok(())
}
