# Architectural decision record (ADR) 019: type-directed step-return classification

## Status

Accepted (2026-08-29): Dispatch unhinted non-unit step returns through a
type-directed bridge while preserving explicit hints and value aliases.

## Date

2026-08-29.

## Context and problem statement

ADR-002 deliberately used macro-time syntax to distinguish unit, value, and
known `Result` step returns. That stable-Rust compromise did not resolve local
type aliases. A fallible alias could therefore be emitted as a value, box its
`Err` as an unused payload, and let the scenario pass.

Requiring a `result` hint avoided the defect only when an author already knew
about it. It also added ceremony where the compiler can determine intent. The
ergonomics principle is to reduce ceremony where intent can be clearly inferred.

## Decision drivers

- Make local aliases of `Result<T, E>` fail a scenario on `Err`.
- Preserve genuine value aliases and result-containing wrapper values.
- Stay on stable Rust without overlapping implementation tricks.
- Keep `#[scenario]` return classification unchanged.
- Give non-`Display` error types a focused compiler diagnostic.

## Decision outcome

Step macros still classify literal `()` and `!`, and honour explicit `value` and
`result` hints. Every other unhinted step return, apart from
classifier-rejected nested `Result` and `impl Trait` shapes, is passed to the
hidden `rstest_bdd::step_return` bridge. `StepReturnProbe<'_, Result<T, E>>`
exposes an inherent selector method, while a blanket trait supplies the value
fallback. The selected zero-sized tag then normalizes the original value:
`Result` errors become scenario failures, and successful payloads retain
ordinary override behaviour.

The mechanism sidesteps ADR-002's stable-Rust constraint through
method-resolution precedence: the `Result` arm is an inherent implementation,
the fallback is a blanket trait, so the two never meet in implementation
coherence, and where both apply, inherent candidates outrank trait candidates.

The error type of a dispatched `Result` must implement `Display`. The hidden
`StepErrorDisplay` bound carries an `on_unimplemented` diagnostic explaining
that `rstest-bdd` renders a step's `Err` through `Display`.

`StepReturnNormalize` is sealed and is called by path, not method syntax. The
bridge belongs to `rstest-bdd`; generated wrappers in `rstest-bdd-macros` are
its only permitted call-sites. Changing it is breaking for existing macro
expansions even though its module is `#[doc(hidden)]`.

## Rationale

An earlier dual-trait autoref design was rejected. Its arms occupied different
probe positions, and EP-M0 demonstrated that a caller blanket trait can capture
the earlier position silently. Inherent-plus-blanket selection leaves no such
caller-owned extension point in the chosen position.

This type-directed choice removes the need for `result` hints for aliases whose
underlying type is `Result`, satisfying the ergonomics goal without guessing
that every named return type is fallible. `Box<Result<..>>`,
`Option<Result<..>>`, references, newtypes, and deref wrappers remain values;
the bridge does not dereference or unwrap user types.

## Consequences

- Scenarios that previously passed after an aliased step returned `Err` now
  correctly fail.
- `Ok(T)` from an aliased result supplies `T`, rather than a boxed `Result`.
- Error types for dispatched results must implement `Display`.
- Nested `Result` and `impl Trait` remain explicit-hint cases, so the macro can
  reject ambiguous unhinted forms before runtime dispatch.
- The structural tag table proves an enumerated partition, not totality over
  every Rust type.

## Alternatives considered

### Require explicit `result` hints for aliases

Rejected. The false green is most dangerous when an author does not know a hint
is needed, and the hint adds ceremony where the compiler can infer the type.

### Classify every unresolved path as a `Result`

Rejected. A local alias can deliberately name a payload type. Treating all
aliases as fallible would break ordinary value-returning steps.

### Dual-trait autoref specialization

Rejected. The EP-M0 collision probe showed that an external blanket trait can
capture the earlier autoref position silently.

### Stable specialization or negative bounds

Rejected. They are unavailable on stable Rust and would reintroduce the
compatibility problem ADR-002 removed.

## References

- [ADR-002: stable step return classification][adr-002]
- [Ergonomics and developer experience][ergonomics]
- [Step-return dispatch regressions][dispatch-tests]
- [Rust reference: method-call expressions][method-resolution]
- [Autoref specialization case study][autoref-specialization]

[adr-002]: adr-002-stable-step-return-classification.md
[autoref-specialization]: https://github.com/dtolnay/case-studies/tree/master/autoref-specialization
[dispatch-tests]: ../crates/rstest-bdd/tests/step_return_dispatch.rs
[ergonomics]: ergonomics-and-developer-experience.md
[method-resolution]: https://doc.rust-lang.org/reference/expressions/method-call-expr.html
