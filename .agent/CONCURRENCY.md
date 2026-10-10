# Concurrency

Read this file when working with async, threads, locks, atomics, channels, background workers, or parallel computation.

## Choose the Model Deliberately

Do not add concurrency automatically.

Use synchronous code for simple sequential work, async for concurrent waiting and I/O, dedicated threads for blocking or long-lived isolated work, and CPU thread pools or data parallelism for substantial CPU-bound work.

Parallelism has overhead.

Do not parallelize tiny workloads without measurements.

## Ownership First

Prefer ownership design before synchronization.

Preference:

1. single ownership
2. borrowing
3. immutable shared state
4. message passing
5. specialized synchronization
6. shared mutable state

Do not default to `Arc<Mutex<T>>` merely because ownership is difficult.

## Async

Do not run long CPU-bound operations directly on async executor workers.

Move blocking or CPU-heavy work to the project's designated mechanism.

Do not hold ordinary `Mutex` or `RwLock` guards across `.await`.

Minimize async critical sections.

Every spawned task must have understood owner, cancellation behavior, shutdown behavior, and error handling.

Do not accidentally detach tasks.

## Channels

Prefer bounded channels.

Unbounded queues require proof that growth is bounded elsewhere or that unbounded behavior is intentional.

Backpressure must be designed explicitly.

Do not send one tiny message per item when batching substantially reduces overhead and latency remains acceptable.

Do not use a channel merely to avoid designing ownership.

## Locks

Keep lock scopes small.

Avoid filesystem I/O, network I/O, database access, expensive CPU work, and blocking waits while holding a lock unless the protected invariant requires it.

Look for contention, nested locks, inconsistent lock ordering, convoying, writer starvation, and unnecessary write locks.

Do not replace a clear mutex with lock-free code until contention is shown to matter.

## Atomics

Use atomics only with a clear synchronization model.

Choose the weakest ordering that is proven correct.

Do not weaken memory ordering merely because weaker sounds faster.

Document non-obvious ordering relationships.

False-sharing mitigation is appropriate only when relevant cache-line contention is measured or strongly established.

## Parallel CPU Work

Avoid oversubscription.

Do not create nested uncontrolled parallel pools.

Batch small work.

Compare parallel and sequential implementations at realistic sizes.

Parallel execution must preserve required determinism where the API promises it.

## Cancellation

Cancellation must leave state valid.

Do not assume spawned work always reaches its final statement.

Be careful around partially initialized state, locks, transactions, temporary files, external resources, and queued messages.

Cancellation behavior should be testable for important workflows.

## Shutdown

Long-lived workers need explicit shutdown.

Shutdown should define who signals, whether pending work drains or aborts, how tasks are joined, and how errors propagate.
