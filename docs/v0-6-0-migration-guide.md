# v0.6.0 migration guide

This guide covers user-facing changes made after the `v0.5.0` tag and before
`v0.6.0`. It groups the changes by the amount of migration work they require:
breaking changes, additive features that extend existing practice, and features
that need a new testing practice to be useful.

## Breaking changes

- Implicit fixture names now normalize exactly one leading underscore.
  Parameters named `world` and `_world` both resolve to the implicit fixture
  key `world`; `__world` resolves to `_world`. Explicit `#[from(...)]` fixture
  names remain exact.
- The legacy `scenarios!(..., runtime = "tokio-current-thread")` form now acts
  as a deprecated compatibility alias for
  `harness = rstest_bdd_harness_tokio::TokioHarness`. Generated tests are
  synchronous and run inside the Tokio harness.
- Custom implementations of the unreleased `HarnessAdapter` development API
  must return `HarnessResult<T>` from `run`. This affects projects that adopted
  the harness API from the `v0.6.0` development branch before the final release.

### Update underscore-prefixed implicit fixtures

If a scenario or step parameter used a leading underscore only to silence the
Rust unused-variable lint, no source change is needed:

```rust,no_run
#[scenario(path = "tests/features/search.feature")]
fn search_works(_world: SearchWorld) {}
```

The parameter above now requests the `world` fixture key. If the code intended
to request a literal `_world` fixture key, make that intent explicit:

```rust,no_run
#[scenario(path = "tests/features/search.feature")]
fn search_works(#[from(_world)] world: SearchWorld) {}
```

Use the same pattern in step functions when a literal underscore-prefixed key
is required. This keeps unused-binding naming and fixture selection separate.

### Update custom harness adapters

`HarnessAdapter::run` now returns `HarnessResult<T>`, an alias for
`Result<T, HarnessError>`, instead of returning `T` directly. This makes
harness infrastructure failures explicit: runtime construction failures, for
example, are propagated as `Err(HarnessError::RuntimeBuildFailed(_))` rather
than surfacing as opaque panics.

Before:

```rust,no_run
use rstest_bdd_harness::{HarnessAdapter, StdScenarioRunRequest};

struct MyHarness;

impl HarnessAdapter for MyHarness {
    type Context = ();

    fn run<T>(&self, request: StdScenarioRunRequest<'_, T>) -> T {
        request.run_without_context()
    }
}
```

After:

```rust,no_run
use rstest_bdd_harness::{HarnessAdapter, HarnessResult, StdScenarioRunRequest};

struct MyHarness;

impl HarnessAdapter for MyHarness {
    type Context = ();

    fn run<T>(
        &self,
        request: StdScenarioRunRequest<'_, T>,
    ) -> HarnessResult<T> {
        Ok(request.run_without_context())
    }
}
```

Harnesses that build runtimes or other infrastructure should map construction
errors into `HarnessError` and use `?`:

```rust,no_run
use rstest_bdd_harness::{HarnessError, HarnessResult};

fn build_runtime() -> HarnessResult<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(HarnessError::RuntimeBuildFailed)
}
```

Harnesses selected by `#[scenario(..., harness = ...)]` or
`scenarios!(..., harness = ...)` are instantiated with `Default`, so custom
harness types used through the macros must implement both `HarnessAdapter` and
`Default`.

## New features available by extending existing practice

- `rstest-bdd-harness` provides shared harness adapter and attribute-policy
  interfaces. Suites that already use `#[scenario]` or `scenarios!` can keep
  their existing test shape and add harness configuration only where needed.
- `#[scenario]` and `scenarios!` accept `harness = ...` and `attributes = ...`
  arguments. These are additive macro arguments; existing scenario definitions
  without them continue to run inline.
- Known first-party harness paths infer their matching attribute policies when
  `attributes = ...` is omitted: `rstest_bdd_harness::StdHarness`,
  `rstest_bdd_harness_tokio::TokioHarness`, and
  `rstest_bdd_harness_gpui::GpuiHarness`.
- The language server accepts `--workspace-root` and
  `RSTEST_BDD_LSP_WORKSPACE_ROOT` to override workspace discovery, and
  `--debounce-ms` to tune file-change processing delay. Existing editor
  integrations can adopt these as normal command-line or environment
  configuration.
- The `examples/tokio-reminders` and `examples/gpui-counter` crates provide
  working examples for the new Tokio and GPUI harness integrations.

## New features requiring new practices

- Result-returning fixtures can now be passed to scenario functions as
  `Result<T, E>` or `StepResult<T, E>`, but the scenario must also return a
  fallible type so generated fixture unwrapping can use `?`.
- Harness adapters can inject typed context through
  `HarnessAdapter::Context`. Steps request that value with the reserved
  `rstest_bdd_harness_context` fixture key.
- Tokio integration should use the explicit
  `rstest_bdd_harness_tokio::TokioHarness` form instead of the deprecated
  `runtime = "tokio-current-thread"` compatibility syntax.
- GPUI integration should use the opt-in `rstest-bdd-harness-gpui` crate and,
  when native GPUI is used outside this workspace's shim, account for the
  platform libraries required by upstream GPUI.
- Third-party attribute policies still need explicit user documentation. The
  macros trait-check user-provided policy types, but path-based code generation
  only recognizes first-party policy paths and imported first-party policy type
  names today.

### Harness dependency matrix

Downstream `Cargo.toml` files should list the smallest crate set that matches
the harness surface they use:

- **Plain BDD scenarios:** add `rstest`, `rstest-bdd`, and
  `rstest-bdd-macros`. Add `rstest-bdd-harness` directly only when test code
  imports base harness API types.
- **Tokio first-party harness:** add `rstest`, `rstest-bdd`,
  `rstest-bdd-macros`, `rstest-bdd-harness-tokio`, and `tokio`. Add
  `rstest-bdd-harness` directly only when implementing a custom harness or
  importing base API types.
- **GPUI first-party harness:** add `rstest`, `rstest-bdd`,
  `rstest-bdd-macros`, `rstest-bdd-harness-gpui`, and `gpui`. Add
  `rstest-bdd-harness` directly only when implementing a custom harness or
  importing base API types.
- **Custom harness implementation:** add `rstest`, `rstest-bdd`,
  `rstest-bdd-macros`, and `rstest-bdd-harness`. Custom harnesses implement
  `HarnessAdapter` and usually use `ScenarioRunRequest`.

First-party adapter crates re-export the base harness API used by generated
tests, so selecting `rstest_bdd_harness_tokio::TokioHarness` or
`rstest_bdd_harness_gpui::GpuiHarness` does not require a separate direct
`rstest-bdd-harness` entry in the consuming crate. The
`examples/tokio-reminders` and `examples/gpui-counter` manifests intentionally
omit that direct dependency and compile as workspace proof points.

> **Canonical-path requirement:** the macro detects first-party adapters
> by matching the crate-root identifier in the supplied path against the
> known adapter crate names. When the Tokio or GPUI adapter crate is
> renamed in `Cargo.toml` (for example `tok = { package =
> "rstest-bdd-harness-tokio", … }`) or the harness type is re-exported
> under a different module path, the macro cannot identify it as a
> first-party adapter and falls back to resolving base API types through
> `rstest-bdd-harness`. In those cases add `rstest-bdd-harness` as a
> direct dev-dependency.

Adapter-only manifests work when macro arguments use first-party crate-root
paths, such as `rstest_bdd_harness::StdHarness`,
`rstest_bdd_harness_tokio::TokioHarness`, or
`rstest_bdd_harness_gpui::GpuiHarness`. They also work when the adapter type is
imported directly and the macro argument is the single-segment first-party type
name, such as `TokioHarness`, `TokioAttributePolicy`, `GpuiHarness`, or
`GpuiAttributePolicy`. Local type aliases and matching type names under other
module roots are not recognized as first-party paths. When the macro call uses
one of those non-recognized forms, or omits `attributes = ...` while the harness
argument is not recognized as first-party, generated code falls back to
`rstest-bdd-harness` and therefore requires a direct base harness dependency.

### Workspace dependency migration for contributors

Workspace contributors should not restore the old root `[patch.crates-io]`
table after publishing v0.6.0. The workspace now keeps development on the
current checkout through `version` plus `path` entries in
`[workspace.dependencies]`, and member crates inherit those entries with
`.workspace = true`.

This means local development continues to use the latest in-tree `rstest-bdd-*`
crates even after the same version exists on crates.io. During packaging, Cargo
uses the version requirement for the published dependency surface. The GPUI
shim uses the same approach: local builds use `vendor/gpui`, while the
publish-check validator strips that path and checks `rstest-bdd-harness-gpui`
against upstream `gpui`.

External users should not copy the workspace paths. Downstream projects should
depend on published crates by version only, for example
`rstest-bdd-harness-tokio = "0.6.0"` or `rstest-bdd-harness-gpui = "0.6.0"`.

### Adopt fallible fixtures

Use result-like fixture parameters when fixture construction can fail and the
scenario should propagate that error directly:

```rust,no_run
use rstest::fixture;
use rstest_bdd::StepResult;
use rstest_bdd_macros::scenario;

struct World;

#[fixture]
fn world() -> Result<World, String> {
    Ok(World)
}

#[scenario(path = "tests/features/search.feature")]
fn search_works(world: Result<World, String>) -> Result<(), String> {
    Ok(())
}
```

The generated scenario unwraps `world` with `?` before inserting the inner
`World` value into `StepContext`. Borrowed result-like fixtures are rejected:
use `Result<T, E>` or `StepResult<T, E>` by value rather than `&Result<T, E>`
or `&StepResult<T, E>`.

### Adopt harness context

Harnesses that provide framework or application state should expose it through
`HarnessAdapter::Context` and call `request.run(context)`:

```rust,no_run
use rstest_bdd_harness::{HarnessAdapter, HarnessResult, ScenarioRunRequest};

#[derive(Default)]
struct AppHarness;

struct AppContext {
    counter: usize,
}

impl HarnessAdapter for AppHarness {
    type Context = AppContext;

    fn run<T>(
        &self,
        request: ScenarioRunRequest<'_, Self::Context, T>,
    ) -> HarnessResult<T> {
        Ok(request.run(AppContext { counter: 7 }))
    }
}
```

Step functions request the harness-provided context with `#[from(...)]`:

```rust,no_run
use rstest_bdd_macros::given;

#[given("the app counter starts at {n}")]
fn starts_at(
    #[from(rstest_bdd_harness_context)] app: &AppContext,
    n: usize,
) {
    assert_eq!(app.counter, n);
}
```

Harnesses that do not inject context should keep `type Context = ()` and call
`request.run_without_context()`.

### Migrate Tokio scenarios to explicit harness configuration

Replace the deprecated `runtime = "tokio-current-thread"` syntax:

```rust,no_run
use rstest_bdd_macros::scenarios;

scenarios!(
    "tests/features/reminders",
    runtime = "tokio-current-thread"
);
```

with explicit harness selection:

```rust,no_run
use rstest_bdd_macros::scenarios;

scenarios!(
    "tests/features/reminders",
    harness = rstest_bdd_harness_tokio::TokioHarness,
);
```

`TokioHarness` runs synchronous scenario closures inside a Tokio current-thread
runtime with a `LocalSet`. Step functions can use
`tokio::runtime::Handle::current()` and `tokio::task::spawn_local`. Immediate
`async fn` step definitions can complete under the harness, but multi-poll
async steps that yield `Pending` are not supported in this mode. Use explicit
`.await` coordination in the code under test, or use an async scenario with an
external Tokio test attribute when the scenario itself must be asynchronous.

### Adopt GPUI harness configuration

Add the GPUI harness crate as a dev-dependency and select the first-party
harness in scenarios that need GPUI test context injection:

```toml
[dev-dependencies]
rstest-bdd-harness-gpui = "0.6.0"
```

```rust,no_run
use rstest_bdd_macros::scenario;

#[scenario(
    path = "tests/features/counter.feature",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
)]
fn counter_updates() {}
```

When `attributes = ...` is omitted, the macro infers
`rstest_bdd_harness_gpui::GpuiAttributePolicy` for the canonical `GpuiHarness`
path. Steps can request the injected `gpui::TestAppContext` with
`#[from(rstest_bdd_harness_context)]`.

## Migration checklist

- [ ] Review scenario and step parameters that start with `_`; add explicit
  `#[from(_name)]` where the literal underscore-prefixed fixture key is
  required.
- [ ] Replace `scenarios!(..., runtime = "tokio-current-thread")` with
  `harness = rstest_bdd_harness_tokio::TokioHarness`.
- [ ] Update custom `HarnessAdapter` implementations to return
  `HarnessResult<T>` and wrap infallible paths in `Ok(...)`.
- [ ] Use `request.run(context)` for harnesses with typed context and
  `request.run_without_context()` for unit-context harnesses.
- [ ] Make scenarios return `Result` or `StepResult` before passing
  `Result<T, E>` or `StepResult<T, E>` fixtures by value.
- [ ] Add `rstest-bdd-harness-tokio` or `rstest-bdd-harness-gpui` only to test
  targets that need those framework integrations.

## Common errors and fixes

- **Error:** type mismatch: expected `HarnessResult<T>`, found `T`
  - **Fix:** Wrap the return expression in `Ok(...)`.
- **Error:** the trait bound `MyHarness: Default` is not satisfied
  - **Fix:** Derive or implement `Default` for harness types selected by macro
    `harness = ...` arguments.
- **Error:** fixture parameter borrows a result-like type
  - **Fix:** Pass the fixture as owned `Result<T, E>` or `StepResult<T, E>`,
    and make the scenario return a compatible fallible type.
- **Error:** an underscore-prefixed parameter no longer resolves to the
  expected fixture
  - **Fix:** Use `#[from(_fixture_name)]` when the literal fixture key starts
    with an underscore.

## Further reading

- [Developer's guide](developers-guide.md)
- [ADR 006 – Fallible scenario functions](adr-006-fallible-scenario-functions.md)
- [ADR 007 – Harness context injection](adr-007-harness-context-injection.md)
- [ADR 009 – Consistent implicit fixture-name normalisation](adr-009-consistent-implicit-fixture-name-normalization.md)
