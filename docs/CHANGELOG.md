# Changelog

## Unreleased

- Raised the nextest compile budget for the trybuild-based tests from ten to
  twenty minutes, and the default profile's global timeout from twenty to
  forty minutes so the larger per-test budget can be spent. A CI run with a
  cold compiler cache exceeded the old budget while healthily compiling the
  fixture dependency tree; the same test takes about three minutes once the
  cache is warm. A passing test never spends the budget.

- Scenarios that previously passed may now correctly fail: unhinted steps that
  return a local alias of `Result<T, E>` now propagate `Err` rather than boxing
  it as a payload. `Ok(T)` injects `T`, and `E` must implement `Display`.

- `StepContext::insert_value` now returns an `InsertOutcome` enum instead of
  `Option<Box<dyn Any>>`, distinguishing a recorded override (carrying any
  displaced previous override) from values dropped because no fixture matches
  the type (`NoMatch`) or because multiple fixtures match (`AmbiguousIgnored`).
  The previous `None` conflated all three cases. Use `into_previous()` where
  the old `Option` behaviour is needed.
- Redesigned `StepContext` borrowing to be guard-based (ADR-012): borrow
  methods take `&self`, so one step can hold mutable guards for distinct
  fixtures concurrently — including mutable harness context plus mutable world
  state — without `E0499`/`E0502`. New `try_borrow` and `try_borrow_mut`
  methods return a typed `FixtureBorrowError` (`NotFound`, `TypeMismatch`,
  `AlreadyBorrowed`, `NotMutable`) instead of panicking on conflicting borrows;
  the `Option`-based borrow methods remain as conveniences. `FixtureRef`/
  `FixtureRefMut` are now opaque structs with `Deref`/`DerefMut` (their enum
  variants are no longer public), and `StepContext::get` serves shared fixtures
  only — read step-returned overrides through `try_borrow`/`borrow_ref`. The
  v0.6 thread-local GPUI workaround is superseded.
- Fixed fallible `#[scenario]` functions generated with `#[gpui::test]` so the
  scenario result is consumed instead of triggering `unused_must_use` under
  `-D warnings`.
- Mandated `cap-std` and `camino` for cross-platform file system access.
- Documented `E0499`/`E0502` troubleshooting for two mutable `StepContext`
  fixtures in the v0.6.0 migration guide, with workarounds and a cross-link to
  the stateful GPUI playbook.
- Added `ExecutionError` enum for structured step execution failures, replacing
  string-encoded skip messages with proper error variants: `Skip`,
  `StepNotFound`, `MissingFixtures`, and `HandlerFailed`. The `ExecutionError`
  type and `MissingFixturesDetails` struct are now public re-exports from
  `rstest_bdd`.
- Deprecated `encode_skip_message` and `decode_skip_message` functions; use
  `ExecutionError::Skip` variant and `ExecutionError::skip_message()` method
  instead.

- Deprecated `From<&str>` for `StepKeyword`; use `StepKeyword::try_from` or
  `StepKeyword::from_str` instead.
- Helper macros `assert_step_ok!` and `assert_step_err!` to streamline tests for
  `Result`-returning steps.
- Added `assert_step_skipped!` and `assert_scenario_skipped!` to assert skipped
  outcomes in unit and behaviour tests without manual pattern matching.
- Steps can now request `&mut Fixture` when the fixture is provided by value in
  a `#[scenario]` test, eliminating the need for `Cell`/`RefCell` wrappers when
  modelling a mutable “world” object.
- Documented the v0.6 interim playbook for stateful GPUI scenarios in the
  user's guide and the v0.6.0 migration guide, covering durable `Entity<T>`/
  `AnyWindowHandle` storage, `VisualTestContext` reconstruction, and the
  two-sided thread-local reset protocol.
