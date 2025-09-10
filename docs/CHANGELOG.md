# Changelog

## Unreleased

- Mandated `cap-std` and `camino` for cross-platform file system access.

- Deprecated `From<&str>` for `StepKeyword`; use `StepKeyword::try_from` or
  `StepKeyword::from_str` instead.
