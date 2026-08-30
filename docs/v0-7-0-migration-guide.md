# v0.7.0 migration guide

This guide covers user-facing changes made after the `v0.6.0` tag and before
`v0.7.0`. It groups the changes by the amount of migration work they require:
breaking changes and deprecations that need action, followed by a checklist.
Read the [users' guide](users-guide.md) for routine use and the linked RFCs for
design rationale.

## Breaking changes

### Registry lookups return resolved metadata

`find_step_with_metadata` now returns `Option<ResolvedStep>` rather than
`Option<&'static Step>`. `ResolvedStep` dereferences to `Step`, so existing
field access keeps compiling; call `.as_step()` where a `&'static Step` is
required explicitly.

```rust,no_run
use rstest_bdd::{Step, StepKeyword, StepText, find_step_with_metadata};

let resolved = find_step_with_metadata(StepKeyword::When, StepText::from("the balance is 10"))
    .expect("a matching step exists");
let step: &'static Step = resolved.as_step();
let _ = step.pattern.as_str();
```

`ResolvedStep` records the step as used when it is constructed. A metadata
lookup therefore has the same usage-tracking effect as the deprecated helper it
replaces: a resolved step no longer appears in `unused_steps()`. Code that
relied on a lookup being read-only should move its query to the point where the
step is actually selected for execution.

### Bind derived `StepArgs` by placeholder name

`StepArgs` derives bind captures by placeholder name. Fields whose Rust names
differ from placeholders must add
`#[step_args(placeholder = "placeholder_name")]`; positional coincidence is no
longer accepted by derived bindings. See
[RFC 0002](rfcs/0002-named-step-argument-binding.md).

## New scoped-vocabulary model

Unannotated definitions and scenarios remain compatible through the built-in
`rstest_bdd::global` library. A scenario that supplies `libraries = [...]`
selects exactly those libraries; it does not add global definitions implicitly.
Add `rstest_bdd::global` explicitly when a scoped scenario needs compatibility
steps. See [RFC 0001](rfcs/0001-explicit-step-library-scopes.md).

## Deprecations

### Replace the projection lookup helpers

Seven lookup helpers now carry `#[deprecated]`. Each one delegates to a
metadata lookup and projects a single field, so the replacement below is
source-compatible with the helper it replaces. In the mapping, `keyword` is a
`StepKeyword`, `pattern` is a `PatternStr`, and `text` is a `StepText`:

```text
lookup_step(keyword, pattern)
    -> lookup_step_with_metadata(keyword, pattern).map(|step| step.run)
lookup_step_async(keyword, pattern)
    -> lookup_step_with_metadata(keyword, pattern).map(|step| step.run_async)
lookup_step_async_with_mode(keyword, pattern)
    -> lookup_step_with_metadata(keyword, pattern)
        .map(|step| (step.run_async, step.execution_mode))
find_step(keyword, text)
    -> find_step_with_metadata(keyword, text).map(|step| step.run)
find_step_async(keyword, text)
    -> find_step_with_metadata(keyword, text).map(|step| step.run_async)
find_step_async_with_mode(keyword, text)
    -> find_step_with_metadata(keyword, text)
        .map(|step| (step.run_async, step.execution_mode))
find_step_with_mode(keyword, text)
    -> find_step_with_metadata(keyword, text).map(|step| step.as_step())
```

A synchronous lookup that previously used the projection helper began like this:

```rust,no_run
use rstest_bdd::{StepKeyword, StepText, find_step};

let handler = find_step(StepKeyword::When, StepText::from("the balance is 10"));
```

The metadata lookup replaces it directly:

```rust,no_run
use rstest_bdd::{StepKeyword, StepText, find_step_with_metadata};

let handler = find_step_with_metadata(StepKeyword::When, StepText::from("the balance is 10"))
    .map(|step| step.run);
```

Choose the exact lookup, `lookup_step_with_metadata`, when the call already
holds a registered pattern. Choose the fuzzy lookup, `find_step_with_metadata`,
when the call holds step text and placeholder matching is wanted.

## Migration checklist

- [ ] Replace each call to a deprecated lookup helper with its metadata
      counterpart, projecting the field the caller needs.
- [ ] Replace a `&'static Step` result with `ResolvedStep::as_step()` where the
      explicit type is required.
- [ ] Review lookups that were expected to leave usage tracking unchanged; the
      metadata pair records usage when it resolves a step.
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

- [Users' guide: migrating from deprecated lookups][users-guide-lookups]
- [RFC 0001: explicit step library scopes](rfcs/0001-explicit-step-library-scopes.md)
- [RFC 0002: named step argument binding](rfcs/0002-named-step-argument-binding.md)
- [Developer's guide](developers-guide.md)
- [rstest-bdd design](rstest-bdd-design.md)
- [ADR 024: structural usage tracking in registry lookups][adr-024]

[users-guide-lookups]: users-guide.md#migrating-from-deprecated-lookups-v070
[adr-024]: adr-024-structural-usage-tracking-in-registry-lookups.md
