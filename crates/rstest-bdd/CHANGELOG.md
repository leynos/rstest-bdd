# Changelog

## [Unreleased]

### Added (Unreleased)

- Added `ExecutionError` enum for structured step execution failures, replacing
  string-encoded skip messages with proper error variants: `Skip`, `StepNotFound`,
  `MissingFixtures`, and `HandlerFailed`. Both `ExecutionError` and
  `MissingFixturesDetails` are publicly re-exported from the crate root.

  **Migration**: Import these types from the crate root rather than via the
  `execution` submodule:

  ```rust
  // Before (still works, but prefer the crate root)
  use rstest_bdd::execution::{ExecutionError, MissingFixturesDetails};

  // After (preferred)
  use rstest_bdd::{ExecutionError, MissingFixturesDetails};
  ```

- Added `assert_step_skipped!` and `assert_scenario_skipped!` macros to assert
  skipped steps and scenario records, reducing boilerplate in behaviour tests.
- `#[scenario]` fixtures passed by value are now registered mutably, so step
  functions can declare `&mut Fixture` parameters and mutate world state
  without interior mutability wrappers. A new `StepContext::insert_owned`
  helper underpins the change and keeps borrows scoped to a single scenario.

  ```rust
  #[derive(Default)]
  struct PredicateWorld { limit: usize, branches: usize }

  #[given("the branch limit is {limit}")]
  fn limit(world: &mut PredicateWorld, limit: usize) { world.limit = limit; }
  ```

  This aligns the runner with typical BDD "world" usage: plain structs, mutable
  steps, and compile‑time borrow checking instead of `Cell`/`RefCell` wrappers.

### Deprecated (Unreleased)

- Deprecated `encode_skip_message` and `decode_skip_message` functions in favour
  of `ExecutionError::Skip` variant. Use `ExecutionError::skip_message()` to
  extract the optional skip message. These functions will be removed in a future
  release.

### Known issues

- A rustc internal compiler error (ICE) on some nightly compilers affects
  macro‑driven scenarios using `&mut` fixtures. See
  `crates/rstest-bdd/tests/mutable_world_macro.rs` for the guarded regression
  test and `crates/rstest-bdd/tests/mutable_fixture.rs` for the underlying
  `StepContext` coverage. Tracking lives at
  `docs/known-issues.md#rustc-ice-with-mutable-world-macro`. The feature
  remains opt‑in and additive once the upstream fix lands.

### Performance

- Optimized data table conversion by caching parsed tables per step definition,
  reusing the cached rows across executions to avoid repeated string
  allocations for identical tables. (#50)

## [0.1.0-alpha4] - 2025-09-30

- Helper macros `assert_step_ok!` and `assert_step_err!` for concise assertions
  on `Result`-returning steps.

## [0.1.0-alpha3] - 2025-09-03

### Added

- Implicit fixture injection when parameter names match fixture names.
- `MissingFixture` variant in `StepError` to report absent fixtures at runtime.

## [0.1.0-alpha3] - 2025-09-02

### Added (0.1.0-alpha3)

- `MissingFixture` variant in `StepError` to report absent fixtures at runtime.
