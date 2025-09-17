# Changelog

## Unreleased

- Behaviour change: Inferred step patterns now capitalise the first character
  when it is a lowercase ASCII letter, which may affect callers that depended
  on lower-case output.

- Mandated `cap-std` and `camino` for cross-platform file system access.

- Deprecated `From<&str>` for `StepKeyword`; use `StepKeyword::try_from` or
  `StepKeyword::from_str` instead.
- Helper macros `assert_step_ok!` and `assert_step_err!` to streamline tests for
  `Result`-returning steps.
