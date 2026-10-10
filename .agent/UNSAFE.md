# Unsafe Rust

Read this file before adding or materially changing unsafe Rust.

## Default

Use safe Rust unless unsafe provides functionality or measured performance that safe Rust cannot reasonably provide.

Typical legitimate reasons:

- FFI
- operating-system APIs
- SIMD intrinsics
- custom low-level data representation
- manual initialization
- a proven performance bottleneck

Unsafe is not an optimization by itself.

## Preconditions

Before introducing performance-motivated unsafe:

1. Have a correct safe implementation.
2. Benchmark it.
3. Identify the exact cost being removed.
4. Implement the smallest unsafe change.
5. Benchmark again.
6. Verify equivalent behavior.
7. Document the invariant.

If the improvement is negligible, prefer the safe implementation.

## Safety Comments

Every unsafe block requires an immediately adjacent `// SAFETY: ...` comment.

The comment must state the actual invariant, including relevant pointer validity, alignment, initialization, lifetime, aliasing, bounds, ownership, synchronization, or representation guarantees.

Keep unsafe blocks as small as practical.

Wrap unsafe internals in a safe abstraction when an invariant can be enforced at one boundary.

## Manual Initialization

Using capacity plus manual length changes is allowed only when initialization cost matters.

Account for early returns and unwinding.

Never expose uninitialized memory to safe code.

## Raw Pointers

Prove allocation lifetime, alignment, initialization, bounds, and aliasing.

Do not derive multiple mutable references to overlapping storage.

## SIMD

Retain or create a scalar reference implementation when practical.

SIMD implementation must preserve scalar semantics.

Test empty input, short input, exact vector width, tails, relevant alignment cases, large input, and randomized equivalence.

Runtime CPU feature detection should occur outside hot inner loops.

Provide fallback code unless the target architecture is fixed.

## Verification

For unsafe changes, use applicable tools such as `cargo miri test` plus normal tests.

Unsafe code is incomplete until its invariant has been reviewed separately from its functional behavior.
