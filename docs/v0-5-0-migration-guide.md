# v0.5.0 migration guide

This guide highlights changes required to adopt v0.5.0, focusing on fallible
scenario body support and return-kind handling in `#[scenario]`.

## Summary of changes

- Scenario bodies may now return `Result<(), E>` or `StepResult<(), E>`.
- Scenario bodies returning non-unit payloads are rejected at compile time.
- Skip handling for fallible scenarios returns `Ok(())` to keep signatures
  type-correct.
- Manual async wrappers for sync steps should use
  `rstest_bdd::async_step::sync_to_async`; concise signature aliases are
  available for wrapper parameters.

## Affected cases

Projects are affected if any of the following are true:

- A `#[scenario]` function returns a non-unit type (for example, `Result<T, E>`
  where `T != ()`).
- A `#[scenario]` function returns a type alias to `Result` or `StepResult`.
- Scenario bodies use `?` or propagate errors directly.
- You maintain explicit async wrapper functions for synchronous step handlers.

## Required changes

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

Values needed by later steps should be returned from step functions and
injected via fixtures or slots.

### 2) Use explicit `Result`/`StepResult` in scenario signatures

Scenario return classification does not resolve type aliases. When using an
alias like `type MyResult<T> = Result<T, MyError>`, the scenario signature must
spell out `Result<(), MyError>` or use `rstest_bdd::StepResult<(), MyError>`.

```rust
# use rstest_bdd::StepResult;
# use rstest_bdd_macros::scenario;
#[scenario(path = "tests/features/example.feature")]
fn scenario_step_result() -> StepResult<(), &'static str> {
    Ok(())
}
```

### 3) Fallible async scenarios are supported

Async scenario bodies may return `Result<(), E>` and use `?` directly. The
`#[scenario]` macro emits the required test runtime attribute; manual Tokio
boilerplate is only required when an existing `#[tokio::test]` attribute is
already applied.

```rust
# use rstest_bdd_macros::scenario;
#[scenario(path = "tests/features/example.feature")]
async fn async_scenario() -> Result<(), &'static str> {
    do_async_work().await?;
    Ok(())
}
```

### 4) Skipped scenarios remain type-correct

When a scenario is skipped (via `rstest_bdd::skip!`), the generated test
returns `Ok(())` for fallible signatures. This keeps the signature valid and
ensures the skip short-circuit continues to work without additional user code.

### 5) Use the stable async wrapper helper path

If you write explicit async wrappers around synchronous step functions, prefer
the stable helper at `rstest_bdd::async_step::sync_to_async`.

**Before:**

```rust
use rstest_bdd::sync_to_async;
```

**After:**

```rust
use rstest_bdd::async_step::sync_to_async;
```

For cleaner signatures, wrappers can use `StepCtx`, `StepTextRef`, `StepDoc`,
and `StepTable`:

```rust
use rstest_bdd::async_step::sync_to_async;
use rstest_bdd::{StepCtx, StepDoc, StepFuture, StepTable, StepTextRef};

fn my_async_wrapper<'ctx>(
    ctx: StepCtx<'ctx, '_>,
    text: StepTextRef<'ctx>,
    docstring: StepDoc<'ctx>,
    table: StepTable<'ctx>,
) -> StepFuture<'ctx> {
    sync_to_async(my_sync_step)(ctx, text, docstring, table)
}
```

## Migration checklist

- [ ] Every `#[scenario]` returns `()` or `Result<(), E>`/`StepResult<(), E>`.
- [ ] Scenario return type aliases are replaced with explicit `Result` or
  `StepResult` signatures.
- [ ] Non-unit return values are moved into steps, fixtures, or
  `ScenarioState` slots when previously returned from scenario bodies.
- [ ] Explicit sync-to-async wrappers import
  `rstest_bdd::async_step::sync_to_async`.
- [ ] Documentation or internal templates describing scenario return types are
  updated.

## Common errors and fixes

- **Error:** `#[scenario] bodies must return () or a unit Result/StepResult`
  - **Fix:** Scenario signatures return `Result<(), E>` or `StepResult<(), E>`,
    with payload values moved into steps.
- **Error:** `no \`sync_to_async\` in the root` when importing from
  `rstest_bdd::sync_to_async`
  - **Fix:** Update imports to `rstest_bdd::async_step::sync_to_async`.

For migration issues not covered here, see
[ADR-006](docs/adr-006-fallible-scenario-functions.md).[^adr]

[^adr]: docs/adr-006-fallible-scenario-functions.md
