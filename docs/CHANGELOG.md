# Changelog

## Unreleased

- Behaviour change: Inferred step patterns now capitalise the first character
  when the inferred text starts with a lowercase ASCII letter. Leading spaces
  or non-ASCII initials remain unchanged, so callers relying on lower-case
  output may need to adjust.

- Mandated `cap-std` and `camino` for cross-platform file system access.

- Deprecated `From<&str>` for `StepKeyword`; use `StepKeyword::try_from` or
  `StepKeyword::from_str` instead.
- Helper macros `assert_step_ok!` and `assert_step_err!` to streamline tests for
  `Result`-returning steps.
