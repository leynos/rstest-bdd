# Architectural decision record (ADR) 002: stable step return classification

## Status

Accepted.

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

Move return-value normalization into macro expansion:

- The step macros inspect the user function signature and classify the return
  type as one of: unit, value, result-unit, result-value.
- The generated wrapper body contains a specialized code path per return kind,
  removing the need for trait trickery.

To preserve ergonomics where possible, the macro recognizes these `Result`
shapes during expansion:

- `Result<..>`, `std::result::Result<..>`, and `core::result::Result<..>`
- `rstest_bdd::StepResult<..>` (a runtime-provided alias)

For cases callers need to override the default classification, provide an
explicit escape hatch on the step attribute:

- `#[given("pattern", result)]` / `#[given("pattern", value)]`
- `#[given(result)]` / `#[given(value)]` (when using the inferred pattern)

The `result` hint is validated for obvious misconfigurations (for example,
primitive return types). For type aliases, the macro cannot validate the alias
and assumes `Result<..>` semantics; if the return type is not actually
`Result`-like, the compiler will surface a type error.

## Consequences

- The `rstest-bdd` runtime crate builds on stable Rust and no longer requires
  `#![feature(auto_traits, negative_impls)]`.
- Return type inference is best-effort: macros cannot resolve arbitrary type
  aliases, so callers occasionally need an explicit `result`/`value` hint.
- The “local compromise” is explicit and limited to the affected step
  definition, rather than forcing a global nightly toolchain.
- A downstream v0.6.0-beta3 trial exposed a correctness failure in the
  best-effort default: a local alias of `Result<T, E>` was classified as a
  value, so an `Err` became an opaque payload and the scenario passed. The
  return-kind hint avoids the defect only when the caller already knows it is
  required; prose guidance alone is not an adequate guard against a false
  green.
- The implementation must therefore make unresolved classification explicit
  without treating every named type as fallible. Roadmap item 11.4.1 tracks
  the diagnostic or required-hint contract and its compile and runtime
  regression matrix. Until it lands, fallible steps must spell `Result<...>`
  or `rstest_bdd::StepResult<...>`, or use the `result` hint.

## Alternatives considered

- Keep the nightly-only auto-trait + negative-impl design: rejected because it
  blocks stable toolchains, and cannot be feature-gated without breaking macro
  expansion for downstream crates.
- Require all fallible steps to return a dedicated wrapper type: rejected as
  unnecessarily disruptive for common `Result<T, E>` usage.
- Wait for stable specialization/negative bounds: rejected because there is no
  stable timeline and it does not address immediate downstream compatibility.
