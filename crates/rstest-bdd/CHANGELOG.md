# Changelog

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
