# v0.7.0 migration guide

This guide covers user-facing changes made after the `v0.6.0` tag and before
`v0.7.0`. It groups the changes by the amount of migration work they require:
breaking changes and deprecations that need action, followed by a checklist.
Read the [users' guide](users-guide.md) for routine use and the linked RFCs for
design rationale.

## Breaking changes

### Registry lookups are fallible

Every public lookup returns `Result<Option<_>, StepLookupError>`.
`StepLookupError` reports equally specific definitions matching the same step,
which the scoped-vocabulary model treats as a configuration error rather than
silently picking one. Handle the error before the optional result:

```rust,no_run
use rstest_bdd::{StepKeyword, StepText, find_step_with_metadata};

let resolved = find_step_with_metadata(StepKeyword::When, StepText::from("the balance is 10"))
    .expect("the selected vocabulary is unambiguous")
    .expect("a matching step exists");
let synchronous_handler = resolved.run;
```

Once the ambiguity is resolved, `find_step_with_metadata` yields an
`Option<ResolvedStep>` rather than an `Option<&'static Step>`. `ResolvedStep`
dereferences to `Step`, so existing field access keeps compiling; call
`.as_step()` where a `&'static Step` is required explicitly.

`ResolvedStep` records the step as used when it is constructed, so a successful
metadata lookup has the same usage-tracking effect as executing the step: a
resolved step no longer appears in `unused_steps()`.
`find_step_with_metadata_in_scope` is the exception. It is the pure query that
scenario execution uses, so it marks nothing; execution marks a step only after
it resolves successfully.

### Bind derived `StepArgs` by placeholder name

`StepArgs` derives bind captures by placeholder name. Fields whose Rust names
differ from placeholders must add
`#[step_args(placeholder = "placeholder_name")]`; positional coincidence is no
longer accepted by derived bindings. See
[RFC 0002](rfcs/0002-named-step-argument-binding.md).

### `StepExecutionRequest` carries a scope

`StepExecutionRequest` now requires a `scope` field. Manual request
construction should use `StepScope::global()` for the previous unscoped
behaviour, or the selected scope for a named-library scenario:

```rust,no_run
use rstest_bdd::{StepKeyword, StepScope};
use rstest_bdd::execution::StepExecutionRequest;

let request = StepExecutionRequest {
    index: 0,
    keyword: StepKeyword::Given,
    text: "the service is running",
    docstring: None,
    table: None,
    feature_path: "tests/features/service.feature",
    scenario_name: "Running service",
    scope: StepScope::global(),
};
```

## New scoped-vocabulary model

Unannotated definitions and scenarios remain compatible through the built-in
`rstest_bdd::global` library. A scenario that supplies `libraries = [...]`
selects exactly those libraries; it does not add global definitions implicitly.
Add `rstest_bdd::global` explicitly when a scoped scenario needs compatibility
steps. See [RFC 0001](rfcs/0001-explicit-step-library-scopes.md).

### Projection helpers are fallible too

The seven field-projection helpers keep their names and remain supported API,
but each now returns `Result<Option<_>, StepLookupError>` instead of
`Option<_>`. Each delegates to a metadata lookup and projects a single field,
so the behaviour and the projected type are unchanged; only the outer shape
differs. In the mapping, `keyword` is a `StepKeyword`, `pattern` is a
`PatternStr`, and `text` is a `StepText`:

```text
lookup_step(keyword, pattern)
    -> lookup_step_with_metadata(keyword, pattern)
        .map(|step| step.map(|found| found.run))
lookup_step_async(keyword, pattern)
    -> lookup_step_with_metadata(keyword, pattern)
        .map(|step| step.map(|found| found.run_async))
lookup_step_async_with_mode(keyword, pattern)
    -> lookup_step_with_metadata(keyword, pattern)
        .map(|step| step.map(|found| (found.run_async, found.execution_mode)))
find_step(keyword, text)
    -> find_step_with_metadata(keyword, text)
        .map(|step| step.map(|found| found.run))
find_step_async(keyword, text)
    -> find_step_with_metadata(keyword, text)
        .map(|step| step.map(|found| found.run_async))
find_step_async_with_mode(keyword, text)
    -> find_step_with_metadata(keyword, text)
        .map(|step| step.map(|found| (found.run_async, found.execution_mode)))
find_step_with_mode(keyword, text)
    -> find_step_with_metadata(keyword, text)
        .map(|step| step.map(|found| found.as_step()))
```

The outer `map` handles the ambiguity error, and the inner one handles the
optional match. A synchronous lookup that previously used the projection helper
gained one more `?`-able step:

```rust,no_run
use rstest_bdd::{StepKeyword, StepText, find_step};

let handler = find_step(StepKeyword::When, StepText::from("the balance is 10"));
```

Its fallible replacement:

```rust,no_run
use rstest_bdd::{StepKeyword, StepText, find_step_with_metadata};

let handler = find_step_with_metadata(StepKeyword::When, StepText::from("the balance is 10"))
    .expect("the selected vocabulary is unambiguous")
    .map(|step| step.run);
```

Choose the exact lookup, `lookup_step_with_metadata`, when the call already
holds a registered pattern. Choose the fuzzy lookup, `find_step_with_metadata`,
when the call holds step text and placeholder matching is wanted.

## Nested fixture preparation is a separate pass

`rstest_bdd_harness::nested_cargo` gained a preparation gate for suites that
shell out to a nested `cargo`. It is additive — nothing that compiled before
stops compiling — but a suite that adopts it changes how it is run.

### Timed tests no longer prepare their own nested builds

`ensure_prepared` returns `Ok(())` when the family's stamp matches and every
directory in `Preparation::artefacts` still holds its outputs. Otherwise it
behaves differently per runner:

- under `cargo nextest`, whose every test runs in its own process, it returns
  `io::ErrorKind::NotFound` naming the command that prepares the family;
- under plain `cargo test` it runs the closure lazily behind a process-wide
  lock.

A suite that used to build its fixture inside a timed test therefore needs a
named preparation test — run before the timed suite, and matched by
`.config/nextest.toml` so it gets the longer `slow-timeout` — plus a
`make prepare-nested-fixtures`-style target. `prepare_now` is the unconditional
entry point that target reaches; it is idempotent, so a warm target directory
costs one stamp read.

No assertion deadline needs to change. The point of the split is that the cold
work leaves the timed test rather than that the timed test waits longer.

### `CARGO_TARGET_DIR` is resolved once, before the child sees it

Both constructors resolve a relative `CARGO_TARGET_DIR` to an absolute path
against the current directory, because a nested build does not run in this
process's directory and Cargo would otherwise resolve the same value somewhere
else. They differ in what they do with an inherited value:

- `env_from_vars` **preserves** an inherited `CARGO_TARGET_DIR`, so nested
  builds keep reusing the outer build cache, and falls back to the directory
  the caller supplies only when the parent named none.
- `env_for_directory` **drops** the inherited value and uses its own
  `directory` instead, because that directory is the one being stamped and a
  stale inherited value would point the build somewhere the stamp does not
  describe. It is **internal**: it exists for `Preparation::for_fixture`, and a
  nested build that should keep the outer cache wants `build_child_env`.

This resolution is fallible. `build_child_env`, `env_from_vars`,
`absolute_target_directory`, `NestedCargoEnv::target_directory`,
`Preparation::new`, `Preparation::for_fixture` and `env_for_directory` now
return `io::Result` where they previously returned the value directly. The
error is the one case that used to be silent: a *relative* `CARGO_TARGET_DIR`
whose working directory cannot be read. The previous behaviour was to return
the value unresolved, which handed the child a path naming the fixture
directory rather than the caller's — a stamp guarding a tree the build never
writes. Absolute values never consult the working directory and never fail. Add
`?` at each call site; no other signature changed.

A caller that previously passed a relative `CARGO_TARGET_DIR` through its own
environment now has an absolute path in the child. `Preparation::new` derives
its `artefacts` list from the target directory resolved in the child
environment rather than from the stamp directory, so the stamp and the compiled
units it vouches for stay in step when the target directory moves.

### A bounded run's captured output is the direct child's

`run_bounded` and `run_bounded_with_timeout` cap output collection a short
grace period after the direct child exits. Without the cap, a successful child
that leaves a descendant holding the inherited pipe write ends would block the
call for that descendant's whole lifetime; with it, the call returns promptly.
The trade is that text a *descendant* writes after the direct child exits is
not collected, so `Output::stdout` and `Output::stderr` describe the direct
child rather than the whole process tree. The exit status is unaffected.

If a nested build hands its diagnostics to a helper that outlives it, expect
less captured text than the tree produced. Wrap the invocation in a supervisor
that waits for the whole tree when the complete output is required.

## `cargo bdd` configuration and Cargo invocation

`cargo bdd <subcommand>` now works from inside a Cargo workspace. The existing
flat `cargo-bdd <subcommand>` spelling is unchanged: it keeps its original
parser, so help, usage and parse failures print exactly as they did in v0.6.0.

The `steps` and `skipped` options can also come from a TOML configuration file
or from `CARGO_BDD_` environment variables. Values merge in the order defaults,
configuration files, environment, then explicitly supplied command-line flags,
so a flag the user typed always wins. For example,
`CARGO_BDD_CMDS_STEPS_SKIPPED=true` supplies `steps --skipped`, and
`[cmds.skipped] json = true` supplies `skipped --json`. Discovery reads
`~/.cargo_bdd.toml`, the XDG `cargo_bdd/config.toml`, and `./.cargo_bdd.toml`;
the [users' guide](users-guide.md#configuration-files-and-environment) names
every location, variable and precedence rule.

Building `cargo-bdd` now requires Rust 1.89 because its configuration
dependency raises the floor. The reusable library crates keep the 1.88 minimum,
so a workspace build needs 1.89 only when it builds the binary. That 1.88 floor
holds for a dependency resolution that keeps the libraries' own requirements
satisfied — in this repository, a lockfile pinning `cargo-platform` v0.3.2. An
independent consumer lockfile may instead resolve a newer `cargo-platform`,
whose higher floor applies to that consumer. See
[ADR 025](adr-025-adopt-ortho-config-for-the-cargo-bdd-cli.md).

## Migration checklist

- [ ] Add ambiguity handling wherever a lookup result is consumed:
      `Result<Option<_>, StepLookupError>` replaced `Option<_>`.
- [ ] Replace a `&'static Step` result with `ResolvedStep::as_step()` where the
      explicit type is required.
- [ ] Review metadata lookups that were expected to leave usage tracking
      unchanged; a resolving `lookup_step_with_metadata` or
      `find_step_with_metadata` records usage. `find_step_with_metadata_in_scope`
      remains a pure query.
- [ ] Move reusable domain steps into `#[step_library]` modules.
- [ ] Add every required library to each scoped `#[scenario]` or `scenarios!`
  binding, including `rstest_bdd::global` where needed.
- [ ] Resolve scoped ambiguity by making the selected vocabulary unambiguous;
  do not rely on library-list order.
- [ ] Audit every derived `StepArgs` struct. Add `placeholder = "..."` to a
  field whose Rust name does not equal its pattern placeholder.
- [ ] Use `trim` and `parse_with` only for capture-specific normalization and
  conversion; retain `#[datatable(...)]` policies for table fields.
- [ ] If a suite shells out to a nested `cargo`, move the cold build into a
      named preparation test reached by `prepare_now` and gate the timed tests
      on `ensure_prepared`; run that preparation pass under Nextest before the
      timed suite.
- [ ] Add `?` to every `nested_cargo` environment, target-directory and
      `Preparation` constructor call that now returns `io::Result`.
- [ ] Re-check any code that parses a bounded run's captured output: it holds
      what the direct child wrote, not what its descendants wrote after it
      exited.
- [ ] Install Rust 1.89 or newer to build `cargo-bdd`; the library crates still
  build with Rust 1.88 under a resolution that honours their own floors.
- [ ] Review the `cargo_bdd` configuration files and `CARGO_BDD_` variables on
  any machine that runs `cargo bdd`, then remove local wrappers that emulated
  the Cargo-dispatched form.

## Common errors

- **Error:** a scoped scenario has no matching definition.
  - **Fix:** select the library that owns the definition, or add
    `rstest_bdd::global` explicitly for an unannotated definition.
- **Error:** selected libraries have equally specific matching definitions.
  - **Fix:** remove one library from the scenario scope or make the patterns
    semantically distinct. Reordering the list cannot resolve the ambiguity.
- **Error:** a derived `StepArgs` field requires a placeholder.
  - **Fix:** align the field and placeholder names or add
    `#[step_args(placeholder = "...")]`.
- **Error:** under Nextest a timed test reports that a nested fixture family is
  not prepared and names `make prepare-nested-fixtures`.
  - **Fix:** run the preparation pass first. The stamp also covers `RUSTFLAGS`
    and the feature selection, so prepare under the same environment as the run
    that reads it.

## Further reading

- [Users' guide: registry lookups][users-guide-lookups]
- [Users' guide: configuration files and environment][users-guide-configuration]
- [RFC 0001: explicit step library scopes](rfcs/0001-explicit-step-library-scopes.md)
- [RFC 0002: named step argument binding](rfcs/0002-named-step-argument-binding.md)
- [Developer's guide](developers-guide.md)
- [rstest-bdd design](rstest-bdd-design.md)
- [ADR 024: structural usage tracking in registry lookups][adr-024]

[users-guide-lookups]: users-guide.md#registry-lookups
[users-guide-configuration]: users-guide.md#configuration-files-and-environment
[adr-024]: adr-024-structural-usage-tracking-in-registry-lookups.md
