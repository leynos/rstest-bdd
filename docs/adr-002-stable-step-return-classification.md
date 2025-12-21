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

The `result` hint is validated and rejected for non-`Result` return types so
misconfiguration surfaces as a macro diagnostic rather than a confusing wrapper
error.

## Consequences

- The `rstest-bdd` runtime crate builds on stable Rust and no longer requires
  `#![feature(auto_traits, negative_impls)]`.
- Return type inference is best-effort: macros cannot resolve arbitrary type
  aliases, so callers occasionally need an explicit `result`/`value` hint.
- The “local compromise” is explicit and limited to the affected step
  definition, rather than forcing a global nightly toolchain.

## Alternatives considered

- Keep the nightly-only auto-trait + negative-impl design: rejected because it
  blocks stable toolchains and cannot be feature-gated without breaking macro
  expansion for downstream crates.
- Require all fallible steps to return a dedicated wrapper type: rejected as
  unnecessarily disruptive for common `Result<T, E>` usage.
- Wait for stable specialization/negative bounds: rejected because there is no
  stable timeline and it does not address immediate downstream compatibility.
