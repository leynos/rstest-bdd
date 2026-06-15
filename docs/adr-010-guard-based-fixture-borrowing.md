# Architectural decision record (ADR) 010: guard-based fixture borrowing

## Status

Accepted (2026-06-11): Borrow `StepContext` fixtures through `&self`-receiver
guard methods with a typed `FixtureBorrowError` surface, and make the guard
types opaque.

## Date

2026-06-11

## Context and problem statement

`StepContext::borrow_mut` took `&mut self` and returned a guard tied to that
exclusive borrow. Generated step wrappers therefore could not hold two
mutable fixture guards at once: a step requesting both
`#[from(rstest_bdd_harness_context)] cx: &mut gpui::TestAppContext` and
`world: &mut UiWorld` failed with `E0499`/`E0502`, even though the fixtures
are distinct entries behind separate `RefCell`s
([design document][design-2761] §2.7.6.1).

Downstream GPUI adoption worked around this with thread-local domain state
plus `RefCell`, relying on caller-enforced reset-at-scenario-boundary
discipline — a framework constraint leaking into domain modelling. The v0.7.0
roadmap (items 12.1.1–12.1.3) committed to retiring that workaround.

Two secondary problems shared the same surface:

- Borrow conflicts on the same fixture panicked via `RefCell::borrow_mut`
  instead of returning an inspectable error.
- `FixtureRef`/`FixtureRefMut` were public enums whose variants exposed the
  backing-storage representation, freezing implementation details into the
  public API.

## Decision drivers

- Generated wrappers must borrow distinct mutable fixtures concurrently.
- Borrow failures must be diagnosable (`Result`-style), not panics.
- The guard types must be stable across internal representation changes.
- The scenario-boundary reset that the thread-local workaround simulated must
  be guaranteed by the framework.
- Existing step definitions and generated code should keep compiling where
  possible.

## Options considered

### Option A: keep `&mut self`, generate sequential scoped borrows

Make the macros restructure step bodies so mutable borrows never overlap.

Pros: no runtime API change. Cons: impossible in general — the step body is
user code that needs both `&mut` parameters alive simultaneously; the
constraint would simply resurface inside the generated closure.

### Option B: `&self`-receiver guard-based borrowing (selected)

Borrow methods take `&self`; aliasing is enforced per fixture by the
underlying `RefCell`, surfaced as guards. Step-returned override values move
behind `RefCell` so the mutable override path no longer needs `&mut self`.
New `try_borrow`/`try_borrow_mut` methods return
`Result<_, FixtureBorrowError>`; the existing `borrow_ref`/`borrow_mut`
remain as `Option`-returning conveniences delegating to them. Guards become
opaque structs with `Deref`/`DerefMut` plus the existing accessor methods.

Pros: distinct-fixture concurrent mutable borrows work; same-fixture
conflicts become typed errors; generated code is source-compatible (it calls
`borrow_mut(...)` on an owned context, and `&self` accepts that call). Cons:
`get` can no longer serve step-returned overrides (they now live behind
`RefCell`, so handing out a plain `&T` is unsound); override reads must use
the guard API.

### Option C: lock-free per-fixture cells with raw pointers

Hand-rolled aliasing checks over `UnsafeCell`. Rejected: `unsafe` without
measurable benefit over `RefCell`, and harder to audit.

## Decision outcome

Adopt Option B.

- `StepContext::try_borrow` / `try_borrow_mut` take `&self` and return
  `Result<FixtureRef<T>, FixtureBorrowError>` /
  `Result<FixtureRefMut<T>, FixtureBorrowError>` with variants `NotFound`,
  `TypeMismatch`, `AlreadyBorrowed`, and `NotMutable`.
- `borrow_ref` / `borrow_mut` / the harness-context wrappers keep their
  `Option` signatures as conveniences over the `try_*` methods; `borrow_mut`
  no longer panics on conflicting borrows.
- `FixtureRef` / `FixtureRefMut` are opaque structs (internals private) with
  `Deref`/`DerefMut`, `AsRef`/`AsMut`, `Debug`, and the existing
  `value`/`value_mut` accessors.
- Step-returned override values are stored as `RefCell<Box<dyn Any>>`;
  `StepContext::get` serves shared fixtures only, and override reads go
  through the guard API.

## World lifecycle contract

The framework — not caller discipline — guarantees scenario-boundary reset:

- The generated test constructs a fresh `StepContext` and fresh fixture
  values per scenario run; nothing is shared across scenarios.
- Owned fixture cells live in the generated test function body and are
  dropped when the scenario finishes — on success, on failure (unwinding),
  and on skip. The cleanup-probe behavioural suite pins drop-on-success and
  drop-on-failure; the skip path drops through the same scope exit.
- The v0.6.x thread-local + manual-reset pattern is superseded; migration
  guidance lives in the users' guide.

## Consequences

- Generated wrappers and ordinary step definitions compile unchanged.
- Code that matched the previously public guard enum variants must switch to
  the accessor methods (none existed in this workspace outside the context
  module).
- Code that read step-returned overrides via `get` must use
  `try_borrow`/`borrow_ref`.
- The borrow semantics are pinned by unit tests, a model-based property
  suite (`crates/rstest-bdd/tests/context_borrow_props.rs`), and end-to-end
  scenarios covering the dual-`&mut` and mutable-harness-context-plus-world
  shapes (`crates/rstest-bdd/tests/concurrent_mut_fixtures.rs`).

[design-2761]: rstest-bdd-design.md
