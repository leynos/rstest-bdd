# Architectural decision record (ADR) 013: adopt Whitaker `no_unwrap_or_else_panic`

## Status

Accepted (2026-06-21): Adopt Whitaker `no_unwrap_or_else_panic` as the
workspace lint gate for panic-only `unwrap_or_else` closures.

## Date

2026-06-21

## Context and problem statement

Roadmap item 10.2.5 requires the stateful GPUI playbook to compile under a
pedantic lint profile that includes `clippy::shadow_reuse`,
`clippy::expect_used`, and the in-house `no_unwrap_or_else_panic` lint.

The in-house lint is implemented by Whitaker, a Dylint lint library, in the
`crates/no_unwrap_or_else_panic` crate. It rejects
`unwrap_or_else(|| panic!(…))` and nested `unwrap_or_else(|| value.unwrap())`
forms on `Option` and `Result`. The repository already denies
`clippy::expect_used` and `clippy::unwrap_used`, including in tests, so
contributors had used `unwrap_or_else(|| panic!(…))` as the remaining escape
hatch for invariant failures. Closing that escape hatch requires a replacement
shape that still preserves clear panic messages in test-only invariant paths.

The compatible shape is Rust's `let … else` syntax:

```rust
let Some(window) = maybe_window else {
    panic!("scenario should have stored a window handle");
};
```

This form preserves the invariant panic, uses no `.unwrap()` or `.expect()`,
and can avoid `clippy::shadow_reuse` by choosing a fresh binding name.

## Decision drivers

- Enforce the real lint rather than a textual proxy.
- Keep the repository's normal build, test, and Clippy toolchain on stable.
- Avoid enabling the full Whitaker suite as part of a narrow roadmap item.
- Keep the GPUI playbook and executable regression suite aligned.
- Make contributor and Continuous Integration (CI) setup explicit.

## Decision outcome

Enforce only Whitaker `no_unwrap_or_else_panic` workspace-wide from
`make lint`, pinned to Whitaker tag `v0.2.5`.

The Makefile builds the single Whitaker lint crate with Dylint's driver
feature under `nightly-2025-09-18`, copies the resulting dynamic library to
Dylint's expected suffixed filename under `target/whitaker`, and runs:

```bash
cargo dylint --keep-going --lib no_unwrap_or_else_panic \
  --no-metadata --no-build -- --workspace --all-targets --all-features
```

`DYLINT_LIBRARY_PATH` is absolute because `cargo dylint` rejects relative
library paths. The repository itself remains on the stable Rust toolchain;
only the lint-library build uses the pinned nightly Dylint driver.

The stateful GPUI playbook and regression suite use
`let … else { panic!(…) }` for infrastructure invariants. `.expect(...)`,
`.unwrap()`, and `unwrap_or_else(|| panic!(…))` are not accepted replacements
under the repository lint profile.

## Options considered

### Option A: textual proxy check

A repository-local search or script could reject the visible
`unwrap_or_else(|| panic!(…))` text pattern.

Rejected. Whitaker already implements the semantic lint, including patterns
that a simple text search misses. A proxy would be weaker and would drift from
the maintained lint.

### Option B: `cargo dylint` metadata integration

The preferred first attempt was a `[workspace.metadata.dylint]` entry pinned to
Whitaker tag `v0.2.5` and the single lint crate path.

Rejected for this repository. The metadata path built the crate but did not
produce the suffixed loadable lint library. Adding `features =
["dylint-driver"]` to the metadata entry was rejected by `cargo-dylint`
metadata handling. The explicit Makefile build/copy/run flow is therefore the
repeatable local and CI mechanism.

### Option C: Whitaker full-suite adoption

Enable the whole Whitaker suite, including lints such as `module_max_lines`,
`no_expect_outside_tests`, and `bumpy_road_function`.

Deferred. Full-suite adoption is materially larger than roadmap item 10.2.5
and is tracked as a v0.6.1 hardening item. This decision intentionally adopts
only `no_unwrap_or_else_panic`.

## Consequences

- `make lint` now requires `cargo-dylint` and `dylint-link` version `5.0.0`.
- First local or CI runs may download `nightly-2025-09-18` and build the lint
  library under `target/whitaker`.
- Contributors writing invariant checks should use `let … else { panic!(…) }`
  or return `Result` and use `?`; `.expect(...)`, `.unwrap()`, and
  `unwrap_or_else(|| panic!(…))` fail the lint profile.
- The CI tools lanes install and cache the Dylint tools and the built Whitaker
  library.
- The full Whitaker suite remains out of scope until the v0.6.1 follow-up.

## Known limitations

The adopted lint does not replace Clippy. `clippy::shadow_reuse`,
`clippy::expect_used`, and `clippy::unwrap_used` remain separate policy
surfaces. The playbook form is chosen because it satisfies all of them
together, not because Whitaker enforces shadowing or `.expect(...)` directly.
