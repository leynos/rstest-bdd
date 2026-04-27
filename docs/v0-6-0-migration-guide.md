# v0.6.0 migration guide

This guide highlights breaking changes between `v0.5.x` and `v0.6.0` that
affect day-to-day usage of `rstest-bdd`.

## Summary of changes

- Custom harness adapters now report harness infrastructure failures through
  `HarnessResult<T>` instead of returning `T` directly.

## Affected cases

Projects are affected if they define a custom type that implements
`rstest_bdd_harness::HarnessAdapter`.

## Required changes

### 7) Update custom `HarnessAdapter` implementations

`HarnessAdapter::run` now returns `HarnessResult<T>`, which is an alias for
`Result<T, HarnessError>`, instead of returning `T` directly. This makes
harness infrastructure failures explicit: runtime construction failures, for
example, are propagated as `Err(HarnessError::RuntimeBuildFailed(_))` rather
than surfacing as opaque panics.

**Before:**

```rust
use rstest_bdd_harness::{HarnessAdapter, StdScenarioRunRequest};

struct MyHarness;

impl HarnessAdapter for MyHarness {
    type Context = ();

    fn run<T>(&self, request: StdScenarioRunRequest<'_, T>) -> T {
        request.run_without_context()
    }
}
```

**After:**

```rust
use rstest_bdd_harness::{HarnessAdapter, HarnessResult, StdScenarioRunRequest};

struct MyHarness;

impl HarnessAdapter for MyHarness {
    type Context = ();

    fn run<T>(&self, request: StdScenarioRunRequest<'_, T>) -> HarnessResult<T> {
        Ok(request.run_without_context())
    }
}
```

Harnesses that build runtimes or other infrastructure should map construction
errors into `HarnessError` and use `?`:

```rust
use rstest_bdd_harness::{HarnessError, HarnessResult};

# fn build_runtime() -> HarnessResult<tokio::runtime::Runtime> {
let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .map_err(HarnessError::RuntimeBuildFailed)?;
# Ok(runtime)
# }
```

## Migration checklist

- [ ] Custom `HarnessAdapter` implementations updated to return
  `HarnessResult<T>` and wrap infallible paths in `Ok(...)`.

## Common errors and fixes

- **Error:** type mismatch: expected `HarnessResult<T>`, found `T`
  - **Fix:** Wrap the return expression in `Ok(...)`.
