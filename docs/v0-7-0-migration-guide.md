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
[RFC 0002](rfcs/rfc-0002-named-step-argument-binding.md).

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
steps. See [RFC 0001](rfcs/rfc-0001-explicit-step-library-scopes.md).

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

## Further reading

- [Users' guide: registry lookups][users-guide-lookups]
- [RFC 0001: explicit step library scopes](rfcs/rfc-0001-explicit-step-library-scopes.md)
- [RFC 0002: named step argument binding](rfcs/rfc-0002-named-step-argument-binding.md)
- [Developer's guide](developers-guide.md)
- [rstest-bdd design](rstest-bdd-design.md)
- [ADR 0024: structural usage tracking in registry lookups][adr-0024]

[users-guide-lookups]: users-guide.md#registry-lookups
[adr-0024]: adrs/adr-0024-structural-usage-tracking-in-registry-lookups.md
