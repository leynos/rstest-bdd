# v0.5.0 migration guide

This guide highlights changes required to adopt v0.5.0, focusing on the new
fallible scenario body support and return-kind handling in `#[scenario]`.

## Summary of changes

- Scenario bodies may now return `Result<(), E>` or `StepResult<(), E>`.
- Scenario bodies returning non-unit payloads are rejected at compile time.
- Skip handling for fallible scenarios returns `Ok(())` to keep signatures
  type-correct.

## Who is affected

You are affected if any of the following are true:

- A `#[scenario]` function returns a non-unit type (for example, `Result<T, E>`
  where `T != ()`).
- A `#[scenario]` function returns a type alias to `Result` or `StepResult`.
- You want to use `?` or propagate errors directly from a scenario body.

## What you need to change

### 1) Update scenario return types

**Before (unsupported in v0.5.0):**

```rust
# use rstest_bdd_macros::scenario;
#[scenario(path = "tests/features/example.feature")]
fn scenario_returns_value() -> Result<u32, &'static str> {
    Ok(42)
}
```

**After (supported):**

```rust
# use rstest_bdd_macros::scenario;
#[scenario(path = "tests/features/example.feature")]
fn scenario_returns_unit() -> Result<(), &'static str> {
    do_setup()?;
    Ok(())
}
```

If you need to surface values to later steps, return them from a step function
instead and inject them via fixtures or slots.

### 2) Use explicit `Result`/`StepResult` in scenario signatures

Scenario return classification does not resolve type aliases. If you previously
used an alias like `type MyResult<T> = Result<T, MyError>`, you must spell out
`Result<(), MyError>` or use `rstest_bdd::StepResult<(), MyError>` in the
scenario signature.

```rust
# use rstest_bdd::StepResult;
# use rstest_bdd_macros::scenario;
#[scenario(path = "tests/features/example.feature")]
fn scenario_step_result() -> StepResult<(), &'static str> {
    Ok(())
}
```

### 3) Fallible async scenarios are now supported

Async scenario bodies may return `Result<(), E>` and use `?` directly. The
`#[scenario]` macro will emit the required test runtime attribute, so no manual
Tokio boilerplate is needed unless you already apply your own `#[tokio::test]`.

```rust
# use rstest_bdd_macros::scenario;
#[scenario(path = "tests/features/example.feature")]
async fn async_scenario() -> Result<(), &'static str> {
    do_async_work().await?;
    Ok(())
}
```

### 4) Skipped scenarios remain type-correct

If a scenario is skipped (via `rstest_bdd::skip!`), the generated test returns
`Ok(())` for fallible signatures. This keeps the signature valid and ensures
the skip short-circuit continues to work without additional user code.

## Migration checklist

- [ ] Ensure every `#[scenario]` returns `()` or `Result<(), E>`/
  `StepResult<(), E>`.
- [ ] Replace any scenario return type aliases with explicit `Result` or
  `StepResult` signatures.
- [ ] Move non-unit return values into steps, fixtures, or `ScenarioState`
  slots if you previously returned them from scenario bodies.
- [ ] Update any documentation or internal templates that describe scenario
  return types.

## Common errors and fixes

- **Error:** `fallible scenarios must return Result<(), E> or StepResult<(), E>`
  - **Fix:** Change the scenario signature to return `Result<(), E>` or
    `StepResult<(), E>` and move any payload values into steps.

If you run into migration issues not covered here, check
`docs/adr-006-fallible-scenario-functions.md` for the full design rationale and
expected behaviour.
