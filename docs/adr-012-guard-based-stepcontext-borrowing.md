# Architectural decision record (ADR) 012: guard-based `StepContext` borrowing committed for v0.7.0

## Status

Proposed

## Date

2026-06-10

## Context and problem statement

`StepContext::borrow_mut(&mut self, key)` returns a guard whose lifetime is
tied to the `&mut self` borrow. A generated step wrapper that needs to borrow
two different mutable fixtures simultaneously — for example, both
`&mut TestAppContext` (under the key `rstest_bdd_harness_context`) and
`&mut World` (under a user-supplied key) — cannot do so: the first borrow pins
`StepContext` and the second borrow of the same `StepContext` fails at compile
time with `E0499` or `E0502`.

This is the root cause of the thread-local workaround tax that every stateful
GPUI adopter pays in v0.6. ADR-007 selected the current `borrow_mut` contract;
the adopter feedback, the downstream migration report, and the v0.7.0 roadmap
(items 12.1.1–12.1.3) all converge on the same redesign direction. Roadmap
item 12.1.1 currently records this as a v0.7.0 *ambition*. This ADR converts
it to a *committed direction* so adopters can plan their v0.6→v0.7 migration.

## Decision drivers

- Remove the fundamental constraint that forces every stateful GPUI scenario
  to use thread-local `RefCell` workarounds.
- Provide concurrent distinct-key mutable borrows from `StepContext` without
  violating Rust's aliasing rules.
- Make the redesign a deliberate, migration-guided breaking change at v0.7.0,
  not a patch.
- Supply an explicit v0.6→v0.7 migration mapping so adopters who build on the
  v0.6 interim can plan ahead.
- Ensure the v0.6.1 additive helpers (`ScenarioStore<T>`, ADR-011) are a
  stepping stone, not a dead end.

## Decision outcome

Adopt guard-based interior borrowing as the v0.7.0 `StepContext` redesign, as
described by roadmap items 12.1.1–12.1.3. This is a committed direction, not
an ambition.

### Core changes (v0.7.0)

**Guard-based interior borrowing (12.1.1).**
`StepContext` replaces the `&mut self` `borrow_mut` API with interior
borrowing that returns `FixtureRefMut` guards. The `StepContext` value itself
is no longer exclusively borrowed by each extraction; only the individual
fixture slot is locked while its guard is live. Two guards for *distinct* keys
can coexist; two guards for the *same* key fail with a `FixtureBorrowError`.

```rust
// v0.7.0 shape (illustrative, not final API)
fn my_step(ctx: &StepContext) {
    let mut harness: FixtureRefMut<TestAppContext> =
        ctx.borrow_mut_keyed("rstest_bdd_harness_context")?;
    let mut world: FixtureRefMut<MyWorld> =
        ctx.borrow_mut_keyed("my_world")?;
    // Both guards are live concurrently — this is now legal.
    harness.do_something();
    world.update();
}
```

**`FixtureRefMut` stable opaque API (12.1.2).**
`FixtureRefMut<T>` exposes stable value-accessor methods (`as_ref`,
`as_mut`, `DerefMut`) without exposing internal enum variants or storage
details, so changes to the representation do not become semver breaks.

**Stable world lifecycle contract (12.1.3).**
`StepContext` gains a first-class lifecycle that guarantees:

- a *before-scenario* hook runs before the first step (reset);
- an *after-scenario* hook runs after the last step (cleanup), including on
  assertion failure and skip.

This makes the thread-local two-sided reset protocol (reset-before-assignment
plus `Drop` cleanup guard) obsolete: the lifecycle hooks cover both halves
automatically.

### v0.6 → v0.7 migration mapping

| v0.6 pattern | v0.7 equivalent |
| --- | --- |
| `thread_local! { static WORLD: RefCell<World> }` | `ctx.borrow_mut::<World>()` (distinct key from harness context) |
| `reset_state_before_assignment()` | before-scenario lifecycle hook; `StepContext` resets the world slot automatically |
| `ScenarioStateCleanup` `Drop` guard | after-scenario lifecycle hook; fires on success, failure, and skip |
| `WORLD.with(\|w\| w.borrow_mut())` in every step | `let mut world = ctx.borrow_mut::<World>()?;` — legal because guard-based borrowing allows concurrent distinct-key borrows |
| `#[from(scenario_state_cleanup)] _cleanup: …` fixture parameter | Removed; cleanup is registered through the lifecycle API |

`ScenarioStore<T>` (ADR-011, v0.6.1) is also superseded by the lifecycle
API. Code written against `ScenarioStore<T>` migrates by replacing the
thread-local store with a direct `ctx.borrow_mut` call and registering the
reset through the lifecycle hook.

### `FixtureBorrowError` surface

`Result`-returning borrow APIs carry a structured `FixtureBorrowError` with
variants for:

- `MissingFixture` — the requested key is not registered.
- `TypeMismatch` — the registered value cannot be downcast to the requested
  type.
- `AlreadyBorrowed` — a mutable borrow is requested while another mutable
  guard for the same key is live.

Roadmap item 11.1.1 adds an early version of this surface in v0.6.1 to begin
the transition; v0.7.0 completes it.

## Consequences

- v0.7.0 is a migration-guide-worthy breaking change: the `&mut self`
  `borrow_mut` API is replaced by guard-based interior borrowing.
- Adopters building on the v0.6 thread-local interim pattern or
  `ScenarioStore<T>` have a documented migration path (table above).
- The v0.7.0 migration guide must include the mapping table from this ADR.
- ADR-011 (`ScenarioStore<T>`, v0.6.1) is a stepping stone: it removes the
  boilerplate while the interim pattern is still active, and it is superseded
  at v0.7.0 by the lifecycle API.
- Roadmap item 12.1.1 transitions from "ambition" to "committed direction".

## Governs

- Roadmap items: Phase 12 introduction and item 12.1.1 (guard-based interior
  borrowing, committed v0.7.0); items 12.1.2 and 12.1.3 remain open and are
  planned under the same v0.7.0 milestone.
- Design document: `§2.7.6.5` (v0.7.0 pre-1.0.0 redesign).
