# Changelog

## [Unreleased]

- Added `assert_step_skipped!` and `assert_scenario_skipped!` macros to assert
  skipped steps and scenario records, reducing boilerplate in behaviour tests.
- `#[scenario]` fixtures that are passed by value are now registered mutably, so
  step functions can declare `&mut Fixture` parameters and mutate world state
  without interior mutability wrappers. A new `StepContext::insert_owned`
  helper underpins the change.

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
