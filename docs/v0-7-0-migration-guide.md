# v0.7.0 migration guide

This guide covers the v0.7.0 step-vocabulary and step-capture contracts. Read
the [users' guide](users-guide.md) for routine use and the linked RFCs for
design rationale.

## Breaking changes

- `StepArgs` derives bind captures by placeholder name. Fields whose Rust names
  differ from placeholders must add
  `#[step_args(placeholder = "placeholder_name")]`; positional coincidence is
  no longer accepted by derived bindings. See
  [RFC 0002](rfcs/0002-named-step-argument-binding.md).

## New scoped-vocabulary model

Unannotated definitions and scenarios remain compatible through the built-in
`rstest_bdd::global` library. A scenario that supplies `libraries = [...]`
selects exactly those libraries; it does not add global definitions implicitly.
Add `rstest_bdd::global` explicitly when a scoped scenario needs compatibility
steps. See [RFC 0001](rfcs/0001-explicit-step-library-scopes.md).

## Migration checklist

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
