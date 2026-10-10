# Performance

Read this file when changing code where latency, throughput, CPU usage, memory usage, or scalability matters.

## Principle

Prefer removing work over making work faster.

Optimize in this order unless profiling gives a concrete reason otherwise:

1. algorithm
2. redundant work
3. data structure
4. data layout
5. I/O
6. allocation
7. copying
8. synchronization
9. parallelism
10. branches
11. vectorization
12. instruction-level optimization

Do not start with SIMD, unsafe, or manual micro-optimization.

## Hot Paths

Identify the hot path explicitly.

Avoid in hot paths unless justified:

- allocation
- formatting
- deep clones
- temporary collections
- repeated hashing
- repeated parsing
- repeated normalization
- dynamic dispatch
- frequent reference-count modifications
- logging
- filesystem or network I/O
- blocking locks
- unnecessary atomics
- runtime configuration lookup

Keep uncommon error work outside the common success path when useful.

## Algorithmic Complexity

Determine expected and worst relevant input sizes.

Inspect code for nested scans, repeated search, repeated sorting, repeated allocation, repeated traversal, N+1 operations, and work whose result does not change between iterations.

Avoid optimizing constants while leaving poor asymptotic behavior intact.

## Data Layout

Consider memory layout part of algorithm design.

Inspect object size, alignment, padding, pointer indirection, traversal order, cache locality, and hot/cold fields.

Prefer contiguous representations for sequentially processed data.

Small arrays or vectors can outperform maps or trees due to locality.

Do not use pointer-heavy structures without a concrete need.

Separate cold data from heavily accessed structures when profiling shows that structure size matters.

Consider AoS versus SoA based on actual access patterns.

## Allocation

Do not allocate repeatedly when useful capacity is known.

Prefer `Vec::with_capacity(n)` and `String::with_capacity(n)` where `n` is a meaningful estimate.

Reuse buffers when ownership remains understandable.

Prefer one contiguous allocation over many tiny allocations when the data model allows it.

For high-frequency transforms, consider caller-owned output buffers.

Avoid intermediate `Vec`s used only for the next operation.

Do not use `collect()` merely to satisfy an API if processing can remain streaming.

## Copying

Avoid deep copying large structures without a reason.

Prefer borrowing or ownership transfer.

Cheap handles such as `Arc`, `Rc`, or shared byte buffers may be appropriate when actual shared ownership exists.

Do not create difficult lifetime designs solely to remove an insignificant copy.

Zero-copy is a tool, not a requirement.

One explicit copy may be preferable to complicated ownership or unsafe aliasing.

## Branches

Hoist loop-invariant branches out of important loops.

Keep hot-loop control flow predictable and simple.

Do not duplicate large algorithms merely to remove an irrelevant branch.

## Iterators and Loops

Neither iterators nor manual loops are inherently faster.

Use the clearer form unless measurements justify another form.

Manual loops are appropriate when they provide better control over multiple buffers, indexing, vectorization, chunking, or branches.

Avoid iterator chains that allocate intermediate collections.

## Dispatch

Prefer static dispatch in tight generic algorithms when code-size effects are acceptable.

Dynamic dispatch is appropriate when runtime heterogeneity or API design requires it.

Do not convert a measured hot loop to dynamic dispatch without evaluating the cost.

Do not create excessive monomorphization either.

## I/O

Reduce syscalls, tiny reads/writes, repeated file opens, metadata calls, network round trips, database queries, and unnecessary flushes.

Use buffering and batching where semantics allow it.

Do not hold locks around slow I/O unless atomicity requires it.

## Caching

Cache only when recomputation is sufficiently expensive.

Every cache needs a clear owner, lifetime, size policy, invalidation rule, and concurrency model.

Do not add unbounded caches.

## Inlining

Do not add `#[inline(always)]` everywhere.

Let LLVM decide normally.

Explicit inlining requires a concrete reason such as measured call overhead, a small cross-crate helper preventing optimization, or generated-code evidence.

Inlining can increase binary size and instruction-cache pressure.

Use `#[cold]` for genuinely uncommon paths only when useful.

## Performance Changes

Do not retain complexity that gives no meaningful improvement.

If an optimization materially complicates code, document what cost it removes, why the simpler form is slower, and benchmark evidence where practical.

Preserve a simple reference implementation for complex optimized kernels when useful for testing.
