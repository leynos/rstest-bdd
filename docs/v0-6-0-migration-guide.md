# v0.6.0 migration guide

This guide covers user-facing changes made after the `v0.5.0` tag and before
`v0.6.0`. It groups the changes by the amount of migration work they require:
breaking changes, additive features that extend existing practice, and features
that need a new testing practice to be useful.

## Breaking changes

- Implicit fixture names now normalize exactly one leading underscore.
  Parameters named `world` and `_world` both resolve to the implicit fixture key
  `world`; `__world` resolves to `_world`. Explicit `#[from(...)]` fixture
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
  `runtime = "tokio-current-thread"` compatibility syntax. The first-party
  Tokio attribute policy is inferred from the canonical harness path, so new
  examples no longer need a paired `attributes = ...` argument by default.
- GPUI integration should use the opt-in `rstest-bdd-harness-gpui` crate and
  the canonical `rstest_bdd_harness_gpui::GpuiHarness` path. The first-party
  GPUI attribute policy is inferred when `attributes = ...` is omitted. When
  native GPUI is used outside this workspace's shim, account for the platform
  libraries required by upstream GPUI.
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
> renamed in `Cargo.toml` (for example
> `tok = { package = "rstest-bdd-harness-tokio", … }`) or the harness type is
> re-exported
> under a different module path, the macro cannot identify it as a
> first-party adapter and falls back to resolving base API types through
> `rstest-bdd-harness`. In those cases, add `rstest-bdd-harness` as a
> direct dev-dependency.

Adapter-only manifests work when macro arguments use first-party crate-root
paths, such as `rstest_bdd_harness::StdHarness`,
`rstest_bdd_harness_tokio::TokioHarness`, or
`rstest_bdd_harness_gpui::GpuiHarness`. They also work when the adapter type is
imported directly and the macro argument is the single-segment first-party type
name, such as `TokioHarness`, `TokioAttributePolicy`, `GpuiHarness`, or
`GpuiAttributePolicy`. Local type aliases and matching type names under other
module roots are not recognized as first-party paths. When the macro call uses
one of those non-recognized forms, or omits `attributes = ...` while the
harness argument is not recognized as first-party, generated code falls back to
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
`lading publish` release workflow strips local patches from the staged
workspace before packaging against upstream `gpui`.

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
use `Result<T, E>` or `StepResult<T, E>` by value rather than `&Result<T, E>` or
`&StepResult<T, E>`.

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
When `attributes = ...` is omitted, the macro infers
`rstest_bdd_harness_tokio::TokioAttributePolicy` for the canonical
`TokioHarness` path. Keep `attributes = ...` only for overrides,
attributes-only configuration, or non-recognized harness paths.

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
`#[from(rstest_bdd_harness_context)]`. Keep `attributes = ...` only for
overrides, attributes-only configuration, or non-recognized harness paths.

Stateful GPUI scenarios — those that share durable view and window handles
across steps and need mutable access to `TestAppContext` — also need the v0.6
interim thread-local pattern documented under
[Migrate a stateful GPUI test](#migrate-a-stateful-gpui-test) below.

#### Migrate a stateful GPUI test

> **Note: this is a v0.6 interim shape.**
>
> The thread-local scenario-state pattern below works around the current
> `StepContext::borrow_mut` contract ([ADR-007][adr-007]); §2.7.6.5 of the
> [rstest-bdd design](rstest-bdd-design.md) and roadmap items 12.1.x track
> the v0.7.0 redesign that will replace it.

Apply this migration when an existing scenario stored a `VisualTestContext`
between steps or relied on a non-thread-local mutable world together with
`#[from(rstest_bdd_harness_context)]`. The [Stateful GPUI scenarios with
durable handles][users-guide-playbook] subsection of the user guide is the
in-depth reference; the steps below mirror its outline:

1. **Update the dev-dependency.** In `Cargo.toml`, depend on
   `rstest-bdd-harness-gpui = "0.6.0"` and add `serial_test` and `rstest` as
   dev-dependencies if they are not already present.
2. **Introduce scenario state and reset helpers.** Add a `ScenarioState`
   struct that stores `Option<gpui::Entity<T>>` and
   `Option<gpui::AnyWindowHandle>` instead of a `VisualTestContext`, hold it in
   a `thread_local!` `RefCell`, and define `reset_state_before_assignment` and
   `reset_state_after_scenario` helpers that clear the cell.
3. **Wire a `Drop`-based cleanup fixture.** Add a
   `ScenarioStateCleanup` value whose `Drop` impl calls
   `reset_state_after_scenario`, and a
   `#[fixture] fn scenario_state_cleanup() -> ScenarioStateCleanup` that calls
   `reset_state_before_assignment` before returning the guard. Pull the fixture
   into every stateful `#[scenario]` and apply `#[serial]` from the
   `serial_test` crate.
4. **Reset before assigning fresh handles.** In the `#[given]` that opens
   a fresh window, call `reset_state_before_assignment` before the call to
   `cx.add_window_view(...)`; both the constructor-side reset and the
   `Drop`-side reset are required to cover panic, skip, and reused-thread paths.
5. **Rebuild `VisualTestContext` per step.** Replace any stored
   `VisualTestContext` field with the durable handles, and in each subsequent
   step reconstruct the visual context with
   `gpui::VisualTestContext::from_window(window, cx)`. The return is
   `Option<VisualTestContext>`; treat `None` as an invariant violation (for
   example, with `let Some(visual_cx) = ... else { panic!(...) };`).

For a worked-out example, see the regression suite at
`crates/rstest-bdd-harness-gpui/tests/stateful_window.rs` and the [stateful
playbook][users-guide-playbook] subsection. The pattern's rationale lives in
§§2.7.6.1–2.7.6.2 of the [rstest-bdd design](rstest-bdd-design.md).

[adr-007]: adr-007-harness-context-injection.md

[design-beta2-quick-wins]: rstest-bdd-design.md#2763-v060-beta2-quick-wins

[design-borrow-constraint]:
rstest-bdd-design.md#2761-borrow-constraint-exposed-by-gpui-adoption

[design-interim-gpui]: rstest-bdd-design.md#2762-interim-gpui-state-pattern

[design-redesign]: rstest-bdd-design.md#2765-v070-pre-100-redesign

[rustc-e0499]: https://doc.rust-lang.org/error_codes/E0499.html

[rustc-e0502]: https://doc.rust-lang.org/error_codes/E0502.html

[rustonomicon-splitting]:
https://doc.rust-lang.org/nomicon/borrow-splitting.html

[users-guide-playbook]:
users-guide.md#stateful-gpui-scenarios-with-durable-handles

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
- [ ] Remove redundant paired first-party `attributes = ...` arguments from
  Tokio and GPUI examples unless the scenario is intentionally demonstrating an
  override.
- [ ] Before promoting any GPUI scenario from non-stateful to stateful, apply
  the two-sided reset protocol from
  [Migrate a stateful GPUI test](#migrate-a-stateful-gpui-test): wire
  `scenario_state_cleanup` into every stateful `#[scenario]`, mark the scenario
  `#[serial]`, and reset the thread-local state before assigning fresh handles.
- [ ] Run feature-gated downstream tests before assuming v0.6.0 broke the API:
  use `cargo test --workspace --all-features`, or the project's Continuous
  Integration (CI)-equivalent gate such as `make test` when a Make-based gate
  wraps the same feature set; the design note tracks this [v0.6.0-beta2 quick
  win][design-beta2-quick-wins].

## Common errors and fixes

- **Error:**
  ``cannot borrow `*ctx` as mutable more than once at a time (E0499)`` or
  ``cannot borrow `*ctx` as mutable because it is also borrowed as immutable (E0502)``
  in a generated step wrapper
  - **Fix:** See [Two mutable fixtures trigger `E0499` or
    `E0502`](#two-mutable-fixtures-trigger-e0499-or-e0502).
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

### Two mutable fixtures trigger `E0499` or `E0502`

> This is a v0.6 interim workaround. The limitation is recorded in
> [rstest-bdd design §2.7.6.1][design-borrow-constraint]; the replacement
> borrow model is tracked in [§2.7.6.5][design-redesign] and roadmap items
> 12.1.x.

The symptom is a rustc borrow-checker error in generated wrapper code, not in
the step body itself. Two mutable fixture parameters usually produce [`E0499`][
rustc-e0499], ``cannot borrow `*ctx` as mutable more than once at a time``. One
mutable fixture plus one immutable fixture can produce [`E0502`][rustc-e0502],
``cannot borrow `*ctx` as mutable because it is also borrowed as immutable``.

This GPUI-shaped snippet is intentionally rejected by the v0.6 generated
wrapper; see *Why this happens* below.

```rust,ignore
#[given("the shell is open")]
fn given_shell_open(
    #[from(rstest_bdd_harness_context)] cx: &mut gpui::TestAppContext,
    world: &mut UiWorld,
) -> StepResult<()> {
    let (shell, visual_cx) = cx.add_window_view(|_context| Shell::default());
    world.shell = Some(shell);
    world.visual_cx = Some(visual_cx);
    Ok(())
}
```

The same constraint is not GPUI-specific. This non-GPUI shape is also rejected
when both parameters come from the same `StepContext`.

```rust,ignore
#[when("the account is saved")]
fn save_account(pool: &mut SqlPool, world: &mut World) -> StepResult<()> {
    pool.store(&world.account)?;
    world.saved = true;
    Ok(())
}
```

#### Why this happens

The wrapper has to borrow each requested fixture before it can call the step
function. For two `&mut T` parameters, that means two sequential
`ctx.borrow_mut::<T>(...)` calls while the first guard is still live.
`StepContext::borrow_mut` takes `&mut self`, so the second call asks for a new
exclusive borrow of `ctx` before the first one has ended. The mixed case is the
same shape with different mutability: `borrow_ref` holds a shared `&ctx` guard
while `borrow_mut` needs an exclusive `&mut ctx` guard.

Design §2.7.6.1 names this as a current `StepContext` design limitation, not a
GPUI-only behaviour. ADR-007 keeps harness context injection under the reserved
`rstest_bdd_harness_context` fixture key, so the v0.6 answer is to change the
step shape. At the Rust-language level, the borrow checker does not treat
`HashMap` lookups at different keys as automatically disjoint; see the
Rustonomicon's discussion of [splitting borrows][rustonomicon-splitting].

#### Workarounds

**Redirect to the stateful GPUI playbook** when the second mutable fixture is
the harness context. Store durable handles in resettable scenario state, then
let each step borrow only `&mut gpui::TestAppContext` from `StepContext`.
Follow [Stateful GPUI scenarios with durable handles][users-guide-playbook] for
the full pattern, or the migration guide's
[Migrate a stateful GPUI test](#migrate-a-stateful-gpui-test) subsection for
the upgrade sequence. Do not adopt this thread-local shape for scenarios that
need only one mutable fixture; it is the v0.6 workaround for the two-mutable
case alone.

**Reshape both parameters to `&T`** when read-only access to both fixtures is
enough. That turns the generated wrapper into two `borrow_ref` calls, both
holding shared borrows of `ctx`, which the borrow checker accepts. Reshaping
only one parameter does not fix the conflict: `&T` plus `&mut T` is the mixed
case and still produces `E0502`.

**Split the step** when neither escape fits. Write consecutive Gherkin steps
where each step touches one fixture only, and pass durable state between them
through ordinary scenario fixtures or through the stateful GPUI playbook:

```gherkin
When the account is saved
Then the saved account is visible
```

If both halves still need both fixtures mutably, splitting has not changed the
borrow shape; the same constraint resurfaces inside one of the new steps. Use
the playbook redirect instead.

#### Where to read more

- [rstest-bdd design §2.7.6.1][design-borrow-constraint] explains the borrow
  constraint.
- [rstest-bdd design §2.7.6.2][design-interim-gpui] records the interim GPUI
  state pattern.
- [rstest-bdd design §2.7.6.5][design-redesign] tracks the v0.7.0 redesign
  target.
- [ADR-007][adr-007] records the harness-context injection contract.
- [Stateful GPUI scenarios with durable handles][users-guide-playbook] is the
  user-guide playbook.

### Feature-file edits do not trigger a rebuild

> **This caveat applies until roadmap item 10.3.3 lands.** Once the
> rebuild-invalidation fix ships, this section can be removed.

`#[scenario(path = "...")]` and `scenarios!` read `.feature` files with
ordinary filesystem I/O at macro-expansion time. Cargo does not track those
reads, so editing only a `.feature` file does not cause Cargo to recompile the
scenario binary. The stale binary and all its compiled expectations are reused
from the build cache until an unrelated `.rs` file in the crate changes.

**Symptom:** a corrupted or changed expectation in a `.feature` file appears to
pass after the edit, as if the change were not picked up.

**Fix:** force a rebuild by touching a `.rs` file in the same crate, or run:

```bash
cargo clean -p <your-crate-name>
```

before re-running tests whenever you edit a `.feature` file without also
editing any `.rs` file.

**Root cause and roadmap:** see design-document
[§2.7.6.6](rstest-bdd-design.md#2766-feature-file-rebuild-invalidation) and
[ADR-010](adr-010-feature-file-change-detection.md) for the analysis and the
chosen fix mechanism. Roadmap item 10.3.3 tracks the implementation, approved
for v0.6.0 final.

## Further reading

- [Developer's guide](developers-guide.md)
- [ADR 006 – Fallible scenario functions](adr-006-fallible-scenario-functions.md)
- [ADR 007 – Harness context injection](adr-007-harness-context-injection.md)
- [ADR 009 – Consistent implicit fixture-name normalization](adr-009-consistent-implicit-fixture-name-normalization.md)
