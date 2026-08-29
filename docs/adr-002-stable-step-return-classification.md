# Architectural decision record (ADR) 002: stable step return classification

## Status

Accepted.

> Amended by ADR-019 (2026-08-29). Step wrappers now use type-directed
> dispatch for unhinted non-unit returns; the historical macro-only contract
> below remains to explain the original stable-Rust decision.

## Date

2025-12-19.

## Context

The `#[given]`, `#[when]`, and `#[then]` macros generate wrapper functions that
normalize the user step’s return value into a common representation understood
by the runtime:

- `()` -> success, no payload
- `T` -> success, payload
- `Result<(), E>` -> success/no payload or error
- `Result<T, E>` -> success/payload or error

Historically, `rstest-bdd` implemented this normalization via a runtime trait
(`IntoStepResult`) with overlapping impls differentiated using nightly-only
auto traits and negative impls. This forced the entire workspace onto nightly,
blocking downstream users pinned to stable Rust.

On stable Rust, it is not possible to express a blanket impl like “for all `T`
except `Result<_, _>` and `()`” without overlapping-impl conflicts.

## Decision

Move literal return-value normalization into macro expansion:

- The step macros inspect the user function signature and classify the return
  type as one of: unit, value, result-unit, result-value.
- The generated wrapper body contains a specialized code path for literal
  unit returns and explicit overrides, avoiding overlapping trait
  implementations.

To preserve ergonomics where possible, the macro recognizes these `Result`
shapes during expansion:

- `Result<..>`, `std::result::Result<..>`, and `core::result::Result<..>`
- `rstest_bdd::StepResult<..>` (a runtime-provided alias)

For cases callers need to override the default classification, provide an
explicit escape hatch on the step attribute:

- `#[given("pattern", result)]` / `#[given("pattern", value)]`
- `#[given(result)]` / `#[given(value)]` (when using the inferred pattern)

The `result` hint is validated for obvious misconfigurations (for example,
primitive return types). Where the hint is present but the return type is a
type alias the macro cannot resolve, the macro trusts the hint and assumes
`Result<..>` semantics; if the return type is not actually `Result`-like, the
compiler will surface a type error.

Without a hint, an unresolved alias is now sent to the type-directed bridge
defined by ADR-019. A fallible alias therefore behaves as its underlying
`Result`, while a genuine value alias remains a value. `value` still forces a
payload interpretation, including for a genuine `Result`.

## Consequences

- The `rstest-bdd` runtime crate builds on stable Rust and no longer requires
  `#![feature(auto_traits, negative_impls)]`.
- Macro-time parsing remains best-effort, but the generated bridge resolves
  ordinary unhinted aliases by their concrete type.
- The “local compromise” is explicit and limited to the affected step
  definition, rather than forcing a global nightly toolchain.
- A downstream v0.6.0-beta3 trial exposed a correctness failure in the former
  best-effort default: a local alias of `Result<T, E>` was classified as a
  value, so an `Err` became an opaque payload and the scenario passed. ADR-019
  closes that false green with inherent-method precedence, while retaining
  explicit hints for deliberate overrides and ambiguous syntactic forms.

## Alternatives considered

- Keep the nightly-only auto-trait + negative-impl design: rejected because it
  blocks stable toolchains, and cannot be feature-gated without breaking macro
  expansion for downstream crates.
- Require all fallible steps to return a dedicated wrapper type: rejected as
  unnecessarily disruptive for common `Result<T, E>` usage.
- Wait for stable specialization/negative bounds: rejected because there is no
  stable timeline and it does not address immediate downstream compatibility.
- Autoref specialization: not considered in 2025-12. ADR-019 evaluates and
  rejects its caller-trait collision risk after an empirical probe.

## Amendments

### 2026-08-29: type-directed alias dispatch

ADR-019 supplements macro-time syntax with a hidden runtime bridge for
unhinted non-unit step returns. It does not reopen the scenario `ReturnKind`
contract. The new bridge makes local aliases of `Result<T, E>` fallible without
misclassifying value aliases, and replaces the historical required-hint rule.
