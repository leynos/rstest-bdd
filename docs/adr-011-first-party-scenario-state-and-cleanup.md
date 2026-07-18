# Architectural decision record (ADR) 011: first-party scenario-state helpers and per-scenario cleanup

## Status

Proposed

## Date

2026-06-10

## Context and problem statement

Every stateful GPUI scenario in v0.6 must hand-roll the same scaffolding:

```rust
thread_local! {
    static SCENARIO_STATE: RefCell<ScenarioState> =
        RefCell::new(ScenarioState::default());
}

fn reset_state_before_assignment() { /* ... */ }
fn reset_state_after_scenario()    { /* ... */ }

struct ScenarioStateCleanup;
impl Drop for ScenarioStateCleanup {
    fn drop(&mut self) { reset_state_after_scenario(); }
}

#[fixture]
fn scenario_state_cleanup() -> ScenarioStateCleanup {
    reset_state_before_assignment();
    ScenarioStateCleanup
}
```

This is approximately 50 lines of boilerplate per consuming crate (as measured
in the first downstream GPUI adopter migration). Every `#[scenario]` that
participates in the protocol must also declare the cleanup fixture parameter,
which adds another two lines per scenario. The pattern is correct but
mechanical, and a mistake in any of the four parts (missing reset-before,
missing `Drop`, wrong reset order, or omitting the fixture parameter) silently
corrupts subsequent scenarios on the same test thread.

Roadmap items 11.1.3 and 11.1.4 propose a generic state helper and cleanup
registration. The first downstream adopter specifically requests a
GPUI-specialized helper and a cleanup-guard fixture macro so the 50-line
block is replaced with a first-party type.

### Naming constraint

`rstest-bdd` already ships two public types in `crates/rstest-bdd/src/state.rs`
that would collide with naive names:

- `pub trait ScenarioState: Default` — the existing per-scenario state trait.
- `pub struct Slot<T>` — the typed container in the same module.

Both are re-exported from the crate root. The new generic helper must
therefore **not** be named `ScenarioState` and must be designed to compose
with — not shadow — `Slot<T>`. This ADR proposes `ScenarioStore<T>` as the
generic core and `GpuiScenarioStore` as the GPUI-specific specialization.

## Decision drivers

- Remove the 50-line boilerplate from every consuming crate.
- Keep the generic helper reusable for future harnesses (for example a Bevy
  `World`) without coupling it to GPUI internals.
- Maintain acyclic crate dependencies: `rstest-bdd-harness-gpui` already
  depends on `rstest-bdd`; the reverse must not occur.
- Fix the cleanup-ordering contract in one place rather than leaving it to
  prose.
- Provide a tested three-state lifecycle (success, assertion failure, skip).
- State clearly which API is current per v0.6.x / v0.7.0 release, so
  adopters know the thread-local interim (`§2.7.6.2`) is still supported in
  v0.6.x while `ScenarioStore<T>` is the recommended additive alternative
  from v0.6.1 onward.

## Options considered

### Option A: generic `ScenarioStore<T>` in `rstest-bdd` only

Expose only the generic `ScenarioStore<T>` from the core `rstest-bdd` crate.
GPUI adopters use it directly without a GPUI-specific wrapper.

Pros:

- One implementation, minimal surface.
- No harness-crate change required.

Cons:

- Adopters still wire up the `#[fixture]` cleanup guard by hand; the macro
  is not provided.
- No GPUI-contextual API names.
- Misses the opportunity for a zero-boilerplate GPUI path.

### Option B: GPUI-specific helper in `rstest-bdd-harness-gpui` only

Ship only a GPUI-specific `GpuiScenarioStore` in `rstest-bdd-harness-gpui`,
without a generic counterpart.

Pros:

- GPUI adopters get a tailored API.
- No change to the core crate.

Cons:

- Future harnesses (Bevy, Winit, Slint) each re-implement the same logic.
- Not reusable: the cleanup-ordering contract lives in GPUI-specific code.
- No core abstraction to test the lifecycle contract against.

### Option C: generic core in `rstest-bdd` plus GPUI specialization in `rstest-bdd-harness-gpui` (selected)

Ship a generic `ScenarioStore<T>` in `rstest-bdd` (implementing the
`set`/`with`/`with_mut`/`take`/`reset` operations plus cleanup registration)
and re-export a `GpuiScenarioStore` specialization and a cleanup-guard
fixture-generating macro from `rstest-bdd-harness-gpui`.

Pros:

- Reusable: any future harness can build on the same generic core.
- GPUI adopters get zero-boilerplate: import `GpuiScenarioStore` and the
  fixture macro; no 50-line block.
- Cleanup-ordering contract is tested once in the generic core.
- Layering is acyclic: `rstest-bdd-harness-gpui` already depends on
  `rstest-bdd`.

Cons:

- Two crates must be updated.
- The generic core must be designed before the GPUI layer can be shipped;
  they are not independent.

### Option D: proc-macro code generation via `#[scenario_store]` derive

Generate the `thread_local!`, reset helpers, `Drop` guard, and `#[fixture]`
entirely from a derive or attribute macro in `rstest-bdd-macros`.

Pros:

- Zero runtime types added.
- Codegen can be placed on the state struct itself.

Cons:

- No central cleanup-coordination point; the contract is distributed across
  every call site.
- Codegen for `#[fixture]` inside a derive is unusual and increases macro
  complexity.
- Harder to test the three-state lifecycle as a unit.
- Recorded as the lighter-weight rejected alternative.

| Axis | A | B | C | D |
| --- | --- | --- | --- | --- |
| Boilerplate reduction | Medium | High | High | High |
| Reusable across harnesses | High | Low | High | Medium |
| Lifecycle contract tested centrally | Low | Medium | High | Low |
| Crate coupling risk | None | None | None | None |
| Consumer-invisible cleanup | No | Yes | Yes | Yes |

*Table 1: Trade-offs for scenario-state helper placement.*

## Decision outcome

Adopt Option C.

### Generic `ScenarioStore<T>` in `rstest-bdd`

`ScenarioStore<T>` wraps a `thread_local! { static …: RefCell<T> }` and
exposes:

- `set(value: T)` — reset and assign.
- `with<R>(f: impl FnOnce(&T) -> R) -> R` — borrow shared.
- `with_mut<R>(f: impl FnOnce(&mut T) -> R) -> R` — borrow exclusive.
- `take() -> T` — consume and reset.
- `reset()` — reset to `T::default()`.

Cleanup is registered through an associated cleanup guard type. The store
wraps the `thread_local!` so consumers never write the thread-local
boilerplate directly.

`ScenarioStore<T>` is designed to sit beside — not replace — the existing
`ScenarioState` trait and `Slot<T>`. Adopters who already use `Slot<T>` for
their world state are not affected; `ScenarioStore<T>` is an additive
alternative for the thread-local interim pattern.

### `GpuiScenarioStore` and fixture macro in `rstest-bdd-harness-gpui`

`GpuiScenarioStore` is a re-export or thin wrapper of `ScenarioStore<T>` with
`T = GpuiWorld` (or a user-supplied type parameter), shipping with:

- A `ScenarioCleanupGuard` type (the `Drop` guard).
- A `#[fixture]`-generating macro (or a ready-made `#[fixture]` function)
  that implements the two-sided reset protocol: `reset_before_assignment()` in
  the constructor, `reset_after_scenario()` in `Drop`.

This replaces the handwritten 50-line block with an import and a one-line
scenario parameter.

### Lifecycle contract

The ADR fixes the cleanup-ordering contract:

1. The cleanup fixture's constructor calls `reset_before_assignment()` before
   the first step runs.
2. `Drop` calls `reset_after_scenario()` after the scenario, regardless of
   outcome.
3. Steps that store handles call `reset_before_assignment()` defensively
   before each assignment.
4. A regression test proves the three-state lifecycle — success, assertion
   failure, and skip — each leave the store in the default state for the
   next scenario.

### Cross-version stance

| Version | Recommended pattern | Support status |
| --- | --- | --- |
| v0.6.0 (current) | Thread-local interim (`§2.7.6.2`) | Supported |
| v0.6.1 (additive) | `ScenarioStore<T>` / `GpuiScenarioStore` | Preferred |
| v0.7.0 (breaking) | Guard-based borrow redesign (ADR-012) | Supersedes both |

The v0.6.0 thread-local interim pattern remains supported throughout v0.6.x.
`ScenarioStore<T>` is the recommended additive alternative from v0.6.1.
ADR-012's guard-based redesign supersedes both at v0.7.0 and provides a
migration mapping.

## Testing strategy

`ScenarioStore<T>` is a small state machine — `set`/`with`/`with_mut`/`take`/
`reset` plus the two-sided reset protocol — so example-based unit tests alone
under-sample the operation orderings that matter. The implementing ExecPlan
(roadmap items 11.1.3/11.1.4) should layer:

1. **Unit tests (required).** Exercise each of the five operations directly,
   and the three-state cleanup lifecycle (success, assertion failure, skip),
   asserting the store returns to `T::default()` for the next scenario in
   every case.
2. **Property-based tests (recommended).** Use `proptest` to generate random
   sequences of store operations (`set`, `with_mut`, `take`, `reset`,
   interleaved with simulated scenario boundaries) and assert the invariants
   that must hold for *any* sequence: a scenario boundary always observes a
   reset store (no handle leaks across the boundary); `take` followed by a read
   without an intervening `set` yields the default; and `with`/`with_mut`
   never observe state from a prior scenario. This is the class of
   state-transition invariant that example tests systematically miss, and it is
   the cheapest guard against a future refactor reordering the reset protocol.
   The crate already depends on `proptest` (see
   `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`), so this adds no
   new dependency.
3. **Thread-isolation coverage (recommended).** Because the store is
   thread-local, include a `serial_test`-guarded test that proves two
   sequential scenarios on the same test thread do not share state, mirroring
   the existing `stateful_window.rs` `stale_window_count == 0` assertion.

Formal proof (Kani/Verus) is **not** recommended here: the invariants are
sequential and bounded, so `proptest` gives adequate coverage without the
harness cost of a model checker.

## Consequences

- Additive and semver-compatible change for v0.6.1.
- GPUI adopters can replace ~50 lines of boilerplate with a single import.
- The cleanup-ordering contract is tested and cannot silently regress.
- The GPUI re-export depends on the generic core landing first (11.1.3 before
  11.1.4).
- `rstest-bdd`'s public API gains `ScenarioStore<T>`; naming is verified not
  to collide with existing `ScenarioState` trait and `Slot<T>`.

## Governs

- Roadmap items: re-scoped 11.1.3 (`ScenarioStore<T>` generic core) and 11.1.4
  (`GpuiScenarioStore` + cleanup-guard fixture macro, three-state lifecycle
  test), both targeted at v0.6.0 final per the maintainer pull-forward decision.
- Design document: `§2.7.6.4` (v0.6.1 early-life support helpers).
