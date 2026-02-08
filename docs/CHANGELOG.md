# Changelog

## Unreleased

- Mandated `cap-std` and `camino` for cross-platform file system access.
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
