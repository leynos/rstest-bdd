# Architectural decision record (ADR) 013: adopt Whitaker `no_unwrap_or_else_panic`

## Status

Accepted (2026-06-21): Adopt Whitaker `no_unwrap_or_else_panic` as the
workspace lint gate for panic-only `unwrap_or_else` closures.

Amended (2026-07-20): the delivery mechanism and pinned toolchain recorded
below describe the original 2026-06-21 delivery and are retained as a
historical record. The current operational contract is captured in
[Update (2026-07-20): current compatibility contract](#update-2026-07-20-current-compatibility-contract),
which supersedes the toolchain and mechanism details in the *Decision outcome*
and *Consequences* sections.

## Date

2026-06-21 (amended 2026-07-20)

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

> **Historical (as originally delivered, 2026-06-21).** The mechanism and
> pinned toolchain in this section reflect the initial delivery under roadmap
> item 10.2.5. They have since been superseded; see
> [Update (2026-07-20): current compatibility contract](#update-2026-07-20-current-compatibility-contract).

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

> **Historical (as originally delivered, 2026-06-21).** The toolchain versions
> and out-of-scope note below reflect the initial delivery. See
> [Update (2026-07-20): current compatibility contract](#update-2026-07-20-current-compatibility-contract)
> for the current tooling and suite scope.

- `make lint` required `cargo-dylint` and `dylint-link` version `5.0.0`.
- First local or CI runs might download `nightly-2025-09-18` and build the lint
  library under `target/whitaker`.
- Contributors writing invariant checks should use `let … else { panic!(…) }`
  or return `Result` and use `?`; `.expect(...)`, `.unwrap()`, and
  `unwrap_or_else(|| panic!(…))` fail the lint profile.
- The CI tools lanes installed and cached the Dylint tools and the built
  Whitaker library.
- The full Whitaker suite remained out of scope until the v0.6.1 follow-up
  (roadmap item 11.2.5), which has since adopted it.

## Update (2026-07-20): current compatibility contract

The single-lint, self-built mechanism recorded above has been superseded. The
repository now consumes the **published Whitaker suite** through the
`whitaker-installer` flow rather than building a pinned Whitaker tag itself.
This change followed roadmap item 11.2.5 (full-suite adoption, completed
2026-07-09) and the estate-wide rollout that began with leynos/netsuke#410.

Whitaker's [PR #238](https://github.com/leynos/whitaker/pull/238) advanced the
suite's toolchain to `nightly-2026-05-28` with Dylint `6.0.1`
(`dylint_linting = 6`). The current contract is:

- **Suite scope:** the full Whitaker Dylint suite, run via `whitaker --all`,
  not the single `no_unwrap_or_else_panic` lint. `no_unwrap_or_else_panic`
  remains enforced as part of that suite.
- **Installation:** `whitaker-installer`, pinned in CI by
  `WHITAKER_INSTALLER_VERSION` in `.github/workflows/ci.yml` (currently
  `0.2.6`). Local setup mirrors CI:

  ```bash
  cargo install --locked whitaker-installer --version 0.2.6
  whitaker-installer
  ```

- **Lint invocation:** `make lint` runs `make lint-whitaker`, which invokes
  `whitaker --all -- --workspace --all-targets --all-features` with
  `RUSTFLAGS="-D warnings"`. The installer-provided `whitaker` wrapper sets
  `DYLINT_LIBRARY_PATH` to the bundled lint library and execs `cargo dylint`,
  so the gate still runs through Dylint; the repository no longer builds or
  copies the library itself.
- **Toolchain:** the pinned nightly (`nightly-2026-05-28`) and the Dylint
  driver (`cargo-dylint` / `dylint_linting` `6.0.1`) are managed by
  `whitaker-installer` and scoped to lint runs. The repository's own build,
  test, and Clippy commands remain on `stable` (`rust-toolchain.toml`).
- **Configuration:** per-lint configuration, including the
  `no_std_fs_operations` `excluded_crates` list with rationale, lives in the
  root `dylint.toml`.

The obsolete artefacts of the original mechanism — Whitaker tag `v0.2.5`,
`nightly-2025-09-18`, Dylint `5.0.0` / `dylint_linting = 5`, and the
`target/whitaker` build-and-copy step — no longer apply and are retained above
only as a historical record.

### Validation (2026-07-20)

The contract was validated against the installed tooling in the development
environment:

- The `whitaker` wrapper resolves `DYLINT_LIBRARY_PATH` to
  `…/whitaker/lints/nightly-2026-05-28/x86_64-unknown-linux-gnu/lib`.
- The bundled suite's `rust-toolchain.toml` pins `nightly-2026-05-28`.
- `cargo-dylint --version` reports `6.0.1`, and the suite's lockfile pins
  `dylint`, `dylint_internal`, `dylint_linting`, and `dylint_testing` at
  `6.0.1`.
- CI installs `whitaker-installer@0.2.6` and runs `make lint`, which drives
  `whitaker --all` through this same wrapper.

Contributor-facing setup and maintenance steps are documented in
`docs/developers-guide.md` under "Whitaker Dylint suite lint gate (ADR-013)".

## Known limitations

The adopted lint does not replace Clippy. `clippy::shadow_reuse`,
`clippy::expect_used`, and `clippy::unwrap_used` remain separate policy
surfaces. The playbook form is chosen because it satisfies all of them
together, not because Whitaker enforces shadowing or `.expect(...)` directly.
