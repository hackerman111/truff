# Benchmarking and Profiling

Read this file when making performance claims or investigating a bottleneck.

## Benchmark Before Optimization

Establish a baseline before changing performance-sensitive code.

A useful benchmark has representative inputs, repeatable setup, equivalent work between versions, enough runtime to reduce measurement noise, and optimized builds where appropriate.

Do not benchmark debug builds for runtime performance conclusions.

## Optimization Loop

Use:

```text
Hypothesis:
Baseline:
Change:
New result:
Memory change:
Decision:
```

Change one important variable at a time when practical.

Revert failed experiments rather than stacking them.

## Compare Equivalent Work

Do not produce faster benchmarks by changing semantics, validation, output quality, workload, required processing, or correctness checks unless that semantic change is part of the intended optimization.

## Microbenchmarks

Use Criterion or `cargo bench` for isolated operations when appropriate.

Make sure setup cost does not dominate the function under measurement unless setup itself is part of the target workload.

Prevent the optimizer from eliminating benchmark work.

## Command-Level Latency

For CLI/startup/process benchmarks, use a tool such as `hyperfine`.

Use warmup runs when appropriate.

Measure cold-start separately when cold-start itself matters.

## CPU Profiling

Useful tools include `perf`, `cargo flamegraph`, and `samply`.

Do not optimize a function merely because it appears prominently in a profile.

Determine whether it is expensive because it is called too often, each call is expensive, upstream code creates unnecessary work, data layout is poor, or synchronization stalls execution.

Fix the cause.

## Memory Profiling

When memory matters, inspect allocation count, total allocated bytes, peak resident memory, retained memory, and object lifetime.

Tools may include `heaptrack`, `dhat`, and `massif`.

Do not infer memory efficiency solely from source-level allocation count.

## Assembly

Inspect generated code only when the bottleneck is sufficiently small and established.

Useful tools may include `cargo asm` and `llvm-mca`.

Do not begin optimization from assembly when algorithm or allocation behavior dominates.

## Reporting

For a performance-sensitive change, report workload, command, baseline, new result, relative change, relevant variance, and machine/configuration when important.

Do not report excessive precision unsupported by benchmark noise.

For stable benchmark suites, a regression around or above 1% deserves investigation unless normal variance is of comparable magnitude.

## Performance Regressions

A regression can be accepted for correctness, safety, required functionality, or a larger improvement elsewhere.

State the trade-off explicitly.

Do not silently accept measurable regressions in an established hot path.
