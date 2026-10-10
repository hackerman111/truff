# Rust Style and Maintainability

Read this file for substantial implementation or refactoring work.

## General Style

Prefer direct, idiomatic Rust.

Write code that exposes ownership, state transitions, invariants, fallibility, and algorithmic structure.

Avoid cleverness whose behavior requires reconstructing compiler inference mentally.

## Functions

Functions should represent coherent operations.

Do not split a function solely because it is long.

Split when doing so isolates an invariant, improves ownership, names a meaningful operation, separates hot and cold paths, enables testing, or removes real duplication.

Avoid one-line forwarding helpers with no semantic value.

## Types

Prefer domain types over primitive mixtures.

Use newtypes when two primitive values have different semantics.

Prefer enums over boolean mode flags.

Avoid giant structs that accumulate unrelated responsibilities.

Keep frequently used core types compact where practical.

## Error Types

Reusable code should prefer typed errors.

Preserve underlying errors.

Attach context at architectural boundaries rather than every call frame.

Do not erase structured failures into strings prematurely.

## Traits

Create a trait for a real abstraction boundary.

Good reasons include multiple current implementations, platform boundaries, backend boundaries, test substitution, and dependency inversion with concrete value.

Do not create a trait solely because another implementation might exist someday.

## Generics

Use generics when the abstraction is natural and useful.

Avoid generic complexity that substantially worsens readability, error messages, compile time, or binary size.

Concrete internal APIs are often preferable when only one representation is required.

## Naming

Use one name for one concept.

Prefer precise domain terms.

Avoid vague catch-all names such as `Manager`, `Helper`, `Utils`, `Common`, `Misc`, `Thing`, and `Stuff` unless they are actual domain concepts.

Follow normal Rust naming conventions.

Use `as_*`, `to_*`, `into_*`, and `try_*` according to their established semantics.

## Comments

Do not comment what the code already states.

Comment why.

Especially document unusual optimizations, non-obvious invariants, safety assumptions, tricky ownership, concurrency ordering, and protocol constraints.

Delete stale comments when behavior changes.

## Modules

Modules should own coherent concepts.

Do not create arbitrary layers solely to make files smaller.

Do not create dumping-ground modules such as `utils` when functionality has a natural owner.

Keep dependencies pointed toward the layer that owns the semantics.

## Public APIs

Expose stable concepts rather than temporary implementation details.

Prefer borrowed inputs when ownership is unnecessary.

Do not expose synchronization primitives or cache internals without reason.

Search the whole workspace before changing a public API.

Update implementations, callers, tests, examples, and documentation.

Do not retain aliases or compatibility wrappers unless compatibility is required.

## Tests

Test behavior and invariants rather than private implementation structure.

Bug fixes should receive regression tests when a stable reproduction exists.

Include boundary cases where relevant.

Use property testing for broad algebraic or structural invariants when it provides meaningful coverage.

Use fuzzing for parsers and other hostile-input boundaries where practical.
