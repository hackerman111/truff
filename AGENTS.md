# AGENTS.md

## Purpose

Write Rust that is correct, fast, memory-efficient, readable, and easy to modify.

Correctness and soundness are mandatory.

For performance-sensitive code, runtime and memory efficiency are first-class requirements. Do not sacrifice meaningful measured performance merely for shorter or more abstract code.

Do not sacrifice readability for hypothetical performance.

## Before Changing Code

Before making a non-trivial change:

1. Read the owning module.
2. Read relevant callers.
3. Read relevant tests.
4. Understand the data flow and ownership.
5. Search for an existing implementation before adding another one.
6. Determine whether the affected path is performance-sensitive.

Do not infer architecture from filenames alone.

Do not create a second implementation of an existing concept unless there is a real architectural reason.

## Changes

Make the smallest complete change that solves the task.

Do not:

- perform unrelated refactors
- rewrite surrounding code merely for style
- introduce infrastructure for hypothetical future requirements
- add wrappers or traits without a real boundary
- duplicate functionality that already exists
- leave obsolete implementation paths after replacing them

Prefer one canonical implementation for one concept.

## Architecture

Every concern should have one clear canonical home.

Before adding code, decide which module or crate owns the responsibility.

Prefer focused modules and directory modules for feature-sized areas.

Module roots should primarily compose submodules and expose a narrow public API rather than accumulate implementation logic.

Do not create reverse or cross-layer dependencies merely because they are convenient.

Do not place domain logic in UI, transport, IPC, CLI, examples, adapters, or integration glue.

Do not create `utils`, `helpers`, or `common` dumping grounds when the behavior has a natural owner.

Large files are a review trigger, not an automatic failure. When a production module grows beyond roughly 300-500 lines, reassess whether it contains multiple responsibilities. Files approaching 800-1000+ lines of mixed production logic should normally be decomposed unless cohesion clearly justifies the size.

Do not split cohesive code solely to satisfy a line-count target.

For architecture, module layout, dependency direction, and decomposition rules, read `.agent/ARCHITECTURE.md`.

## Performance

Eliminate unnecessary work before optimizing necessary work.

Consider performance roughly in this order:

1. algorithmic complexity
2. amount of work performed
3. data structures
4. data layout and cache locality
5. I/O
6. allocations and copying
7. synchronization
8. parallelism
9. branch behavior
10. SIMD or unsafe micro-optimization

Do not optimize from intuition when measurement is practical.

In hot paths aggressively question:

- allocation
- cloning
- formatting
- temporary collections
- repeated parsing
- repeated hashing
- repeated conversion
- dynamic dispatch
- synchronization
- I/O
- redundant validation
- unnecessary indirection

Do not contort cold or insignificant code merely to eliminate a tiny allocation.

For detailed performance work, read `.agent/PERFORMANCE.md`.

## Ownership

Prefer borrowing when ownership is unnecessary.

Prefer `&T`, `&str`, and `&[T]` over unnecessarily owned inputs.

Do not call `.clone()`, `.to_owned()`, `.to_string()`, or `.to_vec()` merely to satisfy the borrow checker.

Clone when ownership genuinely branches.

Do not create complicated lifetime structures merely to remove an insignificant cheap clone.

Keep ownership explicit and local.

Do not default to `Arc<Mutex<T>>`.

Prefer single ownership or immutable shared state when practical.

## Data Structures

Choose data structures from actual access patterns and expected scale.

Prefer contiguous memory when it fits the problem.

Do not automatically reach for `HashMap`.

Watch for:

- accidental O(n²) behavior
- repeated full scans
- repeated sorting
- pointer-heavy structures
- cloning collections inside loops
- building temporary collections for one traversal

Change the algorithm before micro-optimizing an inferior algorithm.

## Types and Invariants

Model important states explicitly.

Prefer:

- enums for alternatives
- structs for coherent state
- newtypes for semantically distinct primitives
- validated constructors for constrained values
- exhaustive matches where practical

Make invalid states difficult or impossible to construct.

Avoid state represented by unrelated booleans or arbitrary strings.

Validate external input at boundaries.

Do not repeatedly revalidate an invariant internally when the type already guarantees it.

## Control Flow

Prefer straightforward Rust.

Keep control flow flat.

Prefer:

- early returns
- `let ... else`
- exhaustive `match`
- small cohesive functions
- explicit ownership transitions

Avoid:

- deep nesting
- clever expression chains
- compound negation
- unnecessary callbacks
- abstraction whose only purpose is reducing line count

Optimized code must remain understandable enough to modify safely.

## Abstractions

Every abstraction must justify itself through at least one of:

- stronger invariant
- ownership boundary
- current reuse
- testability
- portability
- removal of real duplication

Do not introduce traits merely because there may be another implementation someday.

Do not create `utils`, `common`, or `helpers` dumping grounds.

Use static dispatch by default for performance-sensitive generic code.

Use dynamic dispatch when runtime heterogeneity or another concrete requirement justifies it.

## Errors

Use `Result` or `Option` for recoverable conditions.

Do not use `unwrap()`, `expect()`, or `panic!()` for normal caller-reachable failures in library code.

Preserve useful error context.

Do not reduce structured errors to strings unnecessarily.

Panics are appropriate only for programming errors or proven internal invariants.

## Concurrency

Do not default to async, threads, or parallelism.

Use the concurrency model appropriate to the workload.

Async is primarily for concurrent waiting and I/O.

CPU-heavy work should not occupy async executor threads for long periods.

Prefer bounded queues and explicit backpressure.

Do not hold an ordinary lock across `.await`.

Keep critical sections small.

Every long-lived task or thread must have clear ownership and shutdown behavior.

For concurrency-sensitive work, read `.agent/CONCURRENCY.md`.

## Unsafe

Safe Rust is the default.

Unsafe is acceptable for:

- FFI
- platform interfaces
- SIMD/intrinsics
- low-level representation
- a measured bottleneck that cannot reasonably be removed with safe Rust

Every unsafe block requires an adjacent `SAFETY:` comment explaining the concrete invariant.

Keep unsafe blocks small.

Do not introduce unsafe for theoretical performance.

For unsafe or low-level optimization work, read `.agent/UNSAFE.md`.

## Comments

Comments explain information that the code cannot express clearly.

Comment:

- invariants
- safety requirements
- concurrency assumptions
- non-obvious algorithmic choices
- performance trade-offs
- why a simpler implementation is incorrect or measurably slower

Do not narrate obvious code.

## Dependencies

Before adding a dependency:

1. Check whether the project already provides the functionality.
2. Check existing dependencies.
3. Consider runtime cost.
4. Consider compile-time and binary-size cost.
5. Enable only necessary features where practical.

Do not add a large dependency for a trivial helper.

Do not reimplement a complex mature primitive without a concrete reason.

## Verification

Run focused checks while iterating.

Typical workflow:

```bash
cargo check -p <affected-crate>
cargo test -p <affected-crate>
cargo clippy -p <affected-crate> --all-targets --all-features -- -D warnings
```

Before completing substantial changes, run the applicable workspace checks:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Use repository-specific commands when they differ.

Never claim a check passed unless it was actually run.

Performance-sensitive changes require measurement.

For benchmarking work, read `.agent/BENCHMARKING.md`.

## Final Review

Before finishing, inspect the diff.

Check for:

- incorrect behavior
- unnecessary complexity
- duplicate implementations
- new allocations or clones on hot paths
- accidental repeated work
- unnecessary I/O
- lock contention
- unbounded queues
- accidental complexity regressions
- undocumented unsafe assumptions
- unrelated changes

Remove temporary diagnostics and obsolete code.

## Additional Rules

For substantial Rust implementation work, follow `.agent/RUST_STYLE.md`.

For architecture, module layout, dependency direction, or large-file decomposition:
`.agent/ARCHITECTURE.md`

For performance-sensitive work:
`.agent/PERFORMANCE.md`

For async, threads, locks, atomics, or channels:
`.agent/CONCURRENCY.md`

For unsafe, FFI, SIMD, intrinsics, or manual memory handling:
`.agent/UNSAFE.md`

For benchmarks, profiling, or claimed performance improvements:
`.agent/BENCHMARKING.md`
