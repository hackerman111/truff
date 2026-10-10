# Architecture and Code Organization

Read this file when:

- adding a feature
- introducing a module or crate
- reorganizing code
- changing dependency direction
- splitting a large file
- adding a platform, protocol, transport, backend, or UI integration
- deciding where new behavior belongs

## Core Principle

Every concern has one clear canonical home.

Before writing code, determine which module or crate owns the responsibility.

Place behavior according to ownership and semantics, not according to where it is easiest to call from.

Do not scatter one responsibility across unrelated modules.

Do not create multiple implementations of the same concept without a real architectural requirement.

## Module Structure

Prefer small, focused modules with clear responsibilities.

A feature-sized area should usually become a directory module rather than an indefinitely growing single file.

Prefer structures such as:

```text
feature/
├── mod.rs
├── types.rs
├── parser.rs
├── state.rs
├── service.rs
└── tests.rs
```

The exact filenames depend on the domain. Do not create files merely to imitate this layout.

A module root should primarily:

- declare submodules
- compose components
- expose the public API
- define only small shared glue when appropriate

Do not let `mod.rs`, `lib.rs`, `main.rs`, `app.rs`, or another root file become the default location for implementation logic.

## File Size

Line count is a signal, not an architectural rule.

When a production source file grows beyond roughly 300-500 lines, reassess whether it contains:

- multiple independent responsibilities
- unrelated state machines
- several large types
- protocol and domain logic together
- UI and business logic together
- repeated helper groups
- implementation sections that can evolve independently

A file approaching 800-1000+ lines of mixed production logic should normally be decomposed.

A cohesive parser, generated file, table, low-level kernel, or tightly coupled state machine may legitimately be larger.

Do not split cohesive code solely to satisfy a line-count target.

Prefer semantic decomposition over arbitrary slicing.

Bad:

```text
parser_part1.rs
parser_part2.rs
parser_part3.rs
```

Better:

```text
parser/
├── mod.rs
├── lexer.rs
├── grammar.rs
├── error.rs
└── ast_builder.rs
```

when those responsibilities genuinely exist.

## One Responsibility, One Home

A responsibility should have one canonical owner.

Examples:

```text
configuration parsing -> config
domain state          -> domain/core
database persistence  -> storage
HTTP translation      -> transport/http
CLI parsing           -> cli
platform integration  -> platform/<target>
UI rendering          -> ui
```

Do not duplicate domain rules in CLI commands, UI widgets, HTTP handlers, platform adapters, serialization code, or examples.

Adapters translate into and out of core/domain concepts.

They should not become secondary implementations of the application.

## Dependency Direction

Dependencies must follow architectural ownership.

Prefer a directed dependency graph.

A typical shape may look like:

```text
application / frontends
          ↓
      domain/core
          ↓
 infrastructure abstractions
```

or:

```text
frontend
   ↓
application
   ↓
domain
   ↑
adapter implementations
```

The exact architecture is project-specific.

The invariant is more important:

Lower-level reusable logic must not depend on a higher-level frontend or integration merely for convenience.

When a dependency would point in the wrong direction, redesign the boundary.

Do not solve dependency problems by adding imports in both directions.

Avoid dependency cycles.

## Core and Adapters

Keep protocol-, platform-, provider-, and UI-specific details at boundaries.

Core logic should operate on internal domain types whenever practical.

Examples of adapter-specific details:

- HTTP request/response types
- database row types
- Android/iOS platform handles
- terminal events
- GUI widget types
- protobuf-generated structures
- external provider SDK objects
- IPC messages

Translate these at the boundary.

Do not leak them throughout the core merely to avoid writing a conversion.

Likewise, do not invent a large internal abstraction if the external type is already the natural domain type.

## Frontends

Frontends should be thin.

CLI, TUI, GUI, HTTP, RPC, IPC, and mobile layers should primarily:

1. parse or receive external input
2. translate it into application/domain commands
3. call the owning service/core API
4. translate results back to the external representation

Do not place business rules in event handlers, widgets, command parsers, or request handlers.

If two frontends implement the same rule independently, the rule probably belongs below them.

## Platform Code

Platform-specific code should remain behind explicit platform boundaries.

Prefer:

```text
platform/
├── linux.rs
├── windows.rs
└── android.rs
```

or separate platform crates when the dependency boundary justifies them.

Do not scatter `#[cfg(...)]` across unrelated domain modules if a platform adapter can contain the differences.

Portable code must not depend on platform modules.

## Crate Boundaries

Do not create a crate merely because a module became moderately large.

Create a crate when there is a real:

- dependency boundary
- portability boundary
- compilation boundary
- reusable domain
- feature boundary
- independently testable subsystem
- platform boundary

A crate should own a coherent responsibility.

Avoid tiny crates that exist only to hold a few helpers.

Avoid giant crates containing unrelated systems merely to avoid workspace structure.

## Public Surface

Expose the smallest public API needed by consumers.

Prefer `pub(crate)` over `pub` for internals.

Do not expose internal representations, caches, synchronization primitives, or temporary orchestration types unless callers genuinely need them.

Re-export deliberately.

A module root may provide a stable public surface while internal files remain private.

Do not make every type public merely to avoid reorganizing ownership.

## Types and Placement

Place a type near the code that owns its semantics.

Shared types belong at the lowest layer that genuinely owns them.

Do not move every shared type into a global `types.rs`.

A module-level `types.rs` is appropriate when several files in the same feature share domain types.

If a type is meaningful across the whole application, it may belong in a core/domain module.

If it exists only for serialization or transport, keep it in the adapter.

## Utility Code

Avoid catch-all modules such as:

```text
utils.rs
helpers.rs
common.rs
misc.rs
shared.rs
```

when they collect unrelated functions.

Before adding a utility, ask:

1. What concept owns this behavior?
2. Is there already a module for that concept?
3. Is this actually a reusable primitive?
4. Would a small method on an existing type express ownership better?

A narrowly scoped utility module is acceptable when the utilities form a coherent domain.

For example:

```text
path/
unicode/
hashing/
time/
```

may be valid ownership boundaries.

## Feature Addition

When adding a non-trivial feature:

1. Identify the owning layer.
2. Identify the owning module.
3. Determine whether the module remains cohesive with the new responsibility.
4. If not, create a focused submodule or directory module.
5. Define the narrow boundary between the new code and existing code.
6. Keep integration glue thin.
7. Add tests near the owning behavior.

Do not begin implementation by placing all new types and functions in the nearest existing large file.

## Refactoring Large Files

Before splitting a large file, identify responsibilities.

Group code by semantic ownership, not by arbitrary size.

Look for boundaries such as:

- parsing vs evaluation
- state vs rendering
- model vs persistence
- orchestration vs worker implementation
- protocol translation vs business logic
- configuration vs runtime state
- commands vs queries
- public API vs internal algorithm

After splitting:

- keep the public API stable where practical
- minimize unnecessary visibility
- avoid cyclic module dependencies
- remove obsolete forwarding layers
- keep one canonical implementation path

Do not create a directory containing many files that still mutate the same giant shared state without clear boundaries. Moving lines into files is not architecture by itself.

## Cohesion and Coupling

Prefer high cohesion inside modules and low coupling between modules.

A module is cohesive when its contents change for related reasons.

Warning signs:

- unrelated types imported together everywhere
- a module needing knowledge of several distant subsystems
- many functions receiving a giant application state object
- frequent cross-module mutation
- circular callback chains
- broad manager objects coordinating everything
- one file importing most of the project

When these appear, reconsider boundaries before adding more code.

## State Ownership

State should have a clear owner.

Avoid giant shared mutable application state that every subsystem can edit directly.

Prefer:

- narrow methods
- commands
- immutable snapshots
- owned subsystem state
- explicit message boundaries

Do not pass a global context object everywhere merely because it is convenient.

If a function needs only two values, do not pass the entire application state unless the abstraction genuinely owns it.

## Orchestration

Orchestration code may know about several components.

It should coordinate them, not absorb their internal logic.

Keep orchestrators thin.

Bad orchestration often grows into a god object because every new feature is implemented directly inside it.

Move domain behavior to the component that owns it.

## Testing Structure

Tests should follow architectural ownership.

Prefer unit tests near the module that owns behavior.

Use integration tests for boundaries between modules/crates or external behavior.

Do not require a full application runtime to test pure domain logic.

If core behavior cannot be tested without starting UI, network, database, or platform infrastructure, the boundaries may be wrong.

## Local AGENTS.md

For large workspaces, use nested `AGENTS.md` files for subsystem-specific rules.

The root `AGENTS.md` should remain high-signal and broadly applicable.

Put crate-specific or subsystem-specific architecture rules near the code they govern.

Add a local rule only when it is:

- non-obvious
- repeatedly relevant
- specific enough to guide an action

Do not copy the same long instructions into every directory.

## Architecture Review

Before completing a substantial feature or refactor, inspect the resulting tree.

Ask:

- Does each responsibility have one clear owner?
- Did any root file become an implementation dumping ground?
- Did I introduce a reverse dependency?
- Did integration code absorb domain logic?
- Did I create a generic utility dumping ground?
- Is shared mutable state broader than necessary?
- Is a new crate justified by a real boundary?
- Can the core behavior be tested without unrelated infrastructure?
- Is the public surface narrower than the implementation?
- Did I split by semantics rather than line count?

If the answer reveals a structural problem, fix the boundary before adding more code.
