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

