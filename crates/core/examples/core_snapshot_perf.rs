use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

use truff_core::commands::Action;
use truff_core::workspace::Workspace;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut workspace = Workspace::new();
    let text = "x".repeat(1024 * 1024);
    let id = workspace.open_buffer_from_text(PathBuf::from("note.md"), &text)?;
    let iterations = 1000;
    for _ in 0..2 {
        black_box(workspace.dispatch(Action::SaveBuffer(id))?);
    }
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(workspace.dispatch(Action::SaveBuffer(black_box(id)))?);
    }
    let elapsed = start.elapsed().as_nanos();
    println!("scenario,iterations,total_ns,ns_per_op");
    println!(
        "save_dispatch_1mib,{iterations},{elapsed},{}",
        elapsed / iterations
    );
    Ok(())
}
