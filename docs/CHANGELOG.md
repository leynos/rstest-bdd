# Changelog

## Unreleased

- Mandated `cap-std` and `camino` for cross-platform file system access.

- Deprecated `From<&str>` for `StepKeyword`; use `StepKeyword::try_from` or
  `StepKeyword::from_str` instead.
- Helper macros `assert_step_ok!` and `assert_step_err!` to streamline tests for
  `Result`-returning steps.
- Added `assert_step_skipped!` and `assert_scenario_skipped!` to assert skipped
  outcomes in unit and behaviour tests without manual pattern matching.
