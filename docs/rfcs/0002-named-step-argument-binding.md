# RFC 0002: Named step-argument binding

## Preamble

- **RFC number:** 0002
- **Status:** Implemented
- **Created:** 2026-08-30
- **Target release:** v0.7.0
- **Issue:** [#659](https://github.com/leynos/rstest-bdd/issues/659)

## Summary

This RFC makes placeholder names load-bearing for `#[derive(StepArgs)]`.
Derived aggregates bind each Rust field to its named step capture, rather than
consuming captures in declaration order. The implementation also establishes a
shared field-metadata and scalar-conversion model with named `DataTableRow`
derivation, while preserving source-specific table semantics and diagnostics.

## Problem

A step pattern already describes a named record:

```text
{sender} transfers {amount} to {recipient}
```

The former positional `StepArgs` contract silently coupled the placeholder
order to struct-field declaration order. Reordering same-typed fields could
exchange their meaning, and idiomatic Rust names could not differ from business
language without relying on position. It also duplicated field-policy and
conversion machinery that `DataTableRow` had already developed.

## Goals and non-goals

- Goals:
  - Bind derived `StepArgs` fields by placeholder name.
  - Make field declaration order immaterial when mappings agree.
  - Support explicit placeholder names, `rename_all`, trimming, and custom
    scalar parsers.
  - Share field discovery, rename rules, and scalar conversion generation with
    named data-table rows without requiring an owned capture map.
  - Keep source-aware diagnostics for steps and data-table rows.
- Non-goals:
  - Runtime parser registries, hidden I/O, or entity resolution.
  - Treating table rows and captures as one container type.
  - Automatically binding arbitrary struct parameters; `#[step_args]` remains
    explicit.
  - Positional fallback for derived structs whose names do not map.
  - Adding table-only `optional`, `default`, or `truthy` policy to captures.

## Proposed design

### Name-based captures

A field uses its Rust identifier as the placeholder name by default. A
field-level override names a different business-language placeholder, and a
struct-level rename rule runs before an override.

```rust,no_run
use rstest_bdd_macros::{StepArgs, when};

#[derive(StepArgs)]
#[step_args(rename_all = "camelCase")]
struct Transfer {
    #[step_args(placeholder = "sender")]
    from: AccountId,
    #[step_args(trim, parse_with = parse_money)]
    amount: Money,
    destination_account: AccountId,
}

#[when("{sender} transfers {amount} to {destinationAccount}")]
fn transfer(#[step_args] transfer: Transfer) {}
```

`parse_with` accepts a function with the same practical contract as a
data-table custom parser: `fn(&str) -> Result<T, E>`, where the error can be
rendered in the enclosing step diagnostic. `trim` runs before that conversion.
Pattern hint normalization, including `:string` quote stripping, completes
before field conversion.

### Shared conversion model

The macro layer describes each field through a neutral field specification: the
Rust accessor and type, source name, normalization policy, conversion policy,
and missing-value policy. Capture and table adapters supply raw textual fields
and their own context; shared generation applies the mapping and scalar
conversion. This keeps `RowSpec` row and column diagnostics, as well as tuple
row positional semantics, outside capture binding.

The generated capture path uses a borrowed search over the small capture slice,
not an owned `HashMap<String, String>`. It verifies that every named field is
present and that every source capture is claimed.

### Mapping failures

The closed aggregate contract rejects a missing required field, an unconsumed
capture, a duplicate source mapping, invalid helper configuration, and scalar
or custom-parser failure. Diagnostics identify the aggregate type, Rust field,
expected placeholder, raw value where useful, the step pattern, and the step
definition source. Data-table adapters retain row, column, missing-header, and
uneven-row context.

## Compatibility and migration

Existing derived structs remain source-compatible when their field names match
their placeholders. Reordered matching fields now retain their meaning. Code
that previously depended on positional coincidence must add
`#[step_args(placeholder = "...")]`; it no longer gets a silent fallback.

Manual `StepArgs` implementations retain the ordered `from_captures` entry
point for source compatibility. Derived implementations override the named
path, and callers that implement the trait manually can adopt named captures
when they need this contract.

## Verification

Coverage must prove reordered fields, same-typed fields, explicit renames,
struct-level renames, trimming, custom-parser success and failure, `:string`
normalization, missing and unconsumed captures, generics, and manual trait
implementations. Property tests exercise capture-order permutations. UI tests
cover valid derives plus duplicate and unsupported helper attributes. Existing
named and tuple `DataTableRow` tests continue to prove table-only defaults,
optionals, truthy parsing, and row diagnostics.

## Alternatives considered

- **Leave `StepArgs` positional:** retains the silent-swap hazard.
- **Add attributes only to `StepArgs`:** leaves duplicate conversion and
  validation machinery beside `DataTableRow`.
- **Use a synthetic one-row table:** imposes headers, row context, and likely
  allocation on a source with different semantics.
- **Use Serde:** adds a serialization model and error surface without naturally
  preserving step-pattern or row diagnostics.
- **Keep positional fallback:** accepts stale names and typos silently.

## Open questions

Future structured textual front ends may use the neutral field model. They must
retain explicit source adapters and must not turn scalar conversion into a
runtime dictionary or a service lookup.

## Recommendation

Adopt named binding as the sole generated `StepArgs` contract. It makes the
pattern's record shape explicit, supports domain-friendly field names, and
shares the durable parts of conversion logic without flattening step and table
diagnostics into one abstraction.
