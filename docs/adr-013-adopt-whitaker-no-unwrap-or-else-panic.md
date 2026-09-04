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
forms on `Option` and `Result`. The repository denies `clippy::expect_used` and
`clippy::unwrap_used` outside recognized tests. `clippy.toml` permits
`.expect(...)` and `panic!(...)` in test cases, including `rstest` cases, where
unexpected setup failures should fail the test. `unwrap_or_else(|| panic!(…))`
remains rejected, so non-test invariant failures need a replacement shape that
preserves clear panic messages.

The compatible shape is Rust's `let … else` syntax:

```rust
let Some(window) = maybe_window else {
    panic!("scenario should have stored a window handle");
};
```

This form preserves the invariant panic, uses no `.unwrap()` or `.expect()`,
and can avoid `clippy::shadow_reuse` by choosing a fresh binding name.

## Decision drivers

These drivers applied to the original 2026-06-21 decision. The full-suite
scoping driver has since been overtaken by events (see
[Update (2026-07-20)](#update-2026-07-20-current-compatibility-contract)); the
remainder still hold.

- Enforce the real lint rather than a textual proxy.
- Keep the repository's normal build, test, and Clippy toolchain on stable.
- Avoid enabling the full Whitaker suite as part of a narrow roadmap item.
  (Historical: the full suite was subsequently adopted on 2026-07-09 under
  roadmap item 11.2.5.)
- Keep the GPUI playbook and executable regression suite aligned.
- Make contributor and Continuous Integration (CI) setup explicit.

## Decision outcome

> **Historical (as originally delivered, 2026-06-21).** The mechanism and
> pinned toolchain in this section reflect the initial delivery under roadmap
> item 10.2.5. They have since been superseded; see
> [Update (2026-07-20): current compatibility contract](#update-2026-07-20-current-compatibility-contract).

Enforce only Whitaker `no_unwrap_or_else_panic` workspace-wide from
`make lint`, pinned to Whitaker tag `v0.2.5`.

The Makefile builds the single Whitaker lint crate with Dylint's driver feature
under `nightly-2025-09-18`, copies the resulting dynamic library to Dylint's
expected suffixed filename under `target/whitaker`, and runs:

```bash
cargo dylint --keep-going --lib no_unwrap_or_else_panic \
  --no-metadata --no-build -- --workspace --all-targets --all-features
```

`DYLINT_LIBRARY_PATH` is absolute because `cargo dylint` rejects relative
library paths. The repository itself remains on the stable Rust toolchain; only
the lint-library build uses the pinned nightly Dylint driver.

The stateful GPUI playbook and regression suite use `let … else { panic!(…) }`
for infrastructure invariants. `.expect(...)`, `.unwrap()`, and
`unwrap_or_else(|| panic!(…))` are not accepted replacements under the
repository lint profile.

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
produce the suffixed loadable lint library. Adding
`features = ["dylint-driver"]` to the metadata entry was rejected by
`cargo-dylint` metadata handling. The explicit Makefile build/copy/run flow is
therefore the repeatable local and CI mechanism.

### Option C: Whitaker full-suite adoption

Enable the whole Whitaker suite, including lints such as `module_max_lines`,
`no_expect_outside_tests`, and `bumpy_road_function`.

Deferred at the time of this decision (2026-06-21). Full-suite adoption was
materially larger than roadmap item 10.2.5 and was tracked as a v0.6.1
hardening item, so this decision intentionally adopted only
`no_unwrap_or_else_panic`.

> **Superseded (2026-07-09).** The deferral no longer applies: the full
> Whitaker suite was adopted under roadmap item 11.2.5 and is now the current
> gate. Contributors should follow the
> [Update (2026-07-20): current compatibility contract](#update-2026-07-20-current-compatibility-contract),
> not this out-of-scope framing.

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

## Addendum (2026-09-03): Ubicloud CI runner migration

The repository-owned CI matrix now runs its Linux lanes on Ubicloud managed
runners and its Windows lanes on GitHub-hosted runners. This deployment changes
the runner environment, and it reduces the lane count from four to three:

- The Linux lane uses `ubicloud-standard-2`, an Ubuntu 24.04, amd64 shape with
  2 vCPU, 8 GB, and a 72 GB disk. Disk is the binding constraint, not memory:
  `ubicloud-standard-4` offers 145 GB, and two silent deaths in the publish
  step occurred only on the smaller shape, with peak memory of 2,841 MiB and
  3,294 MiB against the 8 GB available. The lane now discards every spent
  build tree, the lint tree, the two GPUI fixture trees and the instrumented
  coverage tree, before the publish build compiles the workspace again, and
  samples disk as well as memory. Escalate only if the reclaimed shape still
  peaks within 5 GB of a full disk. The Ubuntu 24.04 GNU C Library baseline can
  execute Whitaker's repository-hosted Dylint dependency binaries.
- CI pins `whitaker-installer` at `0.2.7` and invokes the SHA-pinned
  `leynos/shared-actions/.github/actions/install-whitaker` action. The shared
  action normalizes Cargo home and executes the installer by absolute path.
- The shared Rust setup publishes `${CARGO_HOME:-$HOME/.cargo}/bin` through
  `GITHUB_PATH`, and the pinned `setup-uv` action publishes `$HOME/.local/bin`
  before Whitaker installation. Those existing setup actions provide user
  binary-directory discovery; the repository does not add another `PATH`
  override.
- Both Windows lanes use the GitHub-hosted `windows-latest` label. Ubicloud
  offers Linux runners only, and GitHub-hosted Windows capacity has not been
  the contention problem. The lane stays lean: it installs GNU Make, which the
  image does not ship, and runs the platform build, coverage, and publish
  checks rather than the full Linux gate.
- The migration preserves the two default-feature and two strict-validation
  lanes, `contents: read` permissions, coverage and ratchet semantics, and all
  action SHA pins. Reusable workflow calls remain free of `runs-on`, so their
  external callees continue to own runner selection.

Every runner is a fresh virtual machine with no persistent volume, so caching
is archive-based. Each mutable path has exactly one owner and an explainable
key carrying an explicit `v1` schema generation together with the operating
system, architecture, and `runner.environment`. Every lane, Ubicloud and
GitHub-hosted alike, uses `actions/cache/restore` and `actions/cache/save` at
v6.1.0: the Ubicloud cache listing on 2026-09-03 showed Linux keys from that
revision reaching Ubicloud storage, while v4.3.0 left nothing there. The
deprecated `ubicloud/cache` fork is not used. Caches are restored on every run
and saved only by the default-features lane on a `push` to `main`, so
pull-request runs read the trusted generation without competing for a
reservation. `ci.yml` therefore triggers on `push` to `main` as well as on pull
requests; before that trigger existed no run could ever publish a generation.
The first `main` run must be checked against the Ubicloud cache listing to
confirm the keys landed there.

No job archives a `target` tree, on any lane or platform. `sccache` is the
single owner of every compiler output, and one store holds the LLVM debug
objects of the test build alongside the instrumented objects of the coverage
build, because `sccache` keys its entries by compiler flags and both shapes
report no non-cacheable compilations. The shared Rust setup and coverage
actions are therefore called with `cache-provider: external` in every workflow,
which disables their own `target` archives, and `RUSTC_WRAPPER` reaches the
coverage build and the publish dry run exactly as it reaches the test build.

The coverage job is the only test execution on Linux. It runs
`cargo llvm-cov nextest --workspace --all-targets --all-features` under
`RUSTFLAGS=-D warnings`, then `cargo test --doc --workspace --all-features`
through the action's `doctests` input, because `cargo llvm-cov nextest` cannot
execute doc tests and nothing in CI ran them before. No bespoke test step sits
beside it.

The Linux `strict-compile-time-validation` lane is folded into that run.
`strict-compile-time-validation` only implies `compile-time-validation` and
conflicts with no other feature, so `--all-features` already enables it and the
separate lane executed nothing new. The two Windows lanes keep their feature
split because Windows uses a different test driver and the Linux run does not
cover it. A former `coverage-main.yml` duplicated the Linux lane exactly once
`ci.yml` gained its `push` trigger; it is removed and its ratchet write and
CodeScene upload moved into the surviving lane.

The Linux lanes use `sccache`'s GitHub Actions cache backend. Its objects reach
Ubicloud storage when the `sccache` process holds the Actions cache
credentials, which the Cuprum cache listing confirmed on 2026-09-03. A plain
`run:` step never sees those credentials, so a pinned `actions/github-script`
step re-exports `ACTIONS_CACHE_URL` and `ACTIONS_RUNTIME_TOKEN`, and clears
`ACTIONS_CACHE_SERVICE_V2` to select the v1 API that Ubicloud's runner-local
cache proxy serves, before the workflow's own checksum-verified `sccache`
binary starts. `ACTIONS_RESULTS_URL` addresses GitHub's own results service
rather than that proxy, and every write failed against it: 6,934 of 6,934 on
the last run that used it. After the change the same workload reported 2,372
hits, a 34 percent hit rate, and 5 write errors in 4,601, and the Ubicloud
listing held 385 `sccache/*` objects under this branch's scope, verified on
2026-09-03 at 23:15 UTC with `ubi gh leynos/rstest-bdd list-cache-entries`.

The shared Rust setup is called with `use-sccache: false` on every lane, and on
Linux that is the decision, not an omission: the action's `sccache` wiring
re-exports `ACTIONS_CACHE_SERVICE_V2=on` with GitHub's results URL and token to
`GITHUB_ENV` as its last act, clobbering the re-export for later steps and
sending every write to the wrong service. The Windows lane has no backend of
that kind at all, so it uses the workspace directory that the cache step
owns. Setting the `RSTEST_BDD_SCCACHE_LOCAL` repository variable moves the
Linux lanes onto that same local directory as a documented fallback. Check the
first `main` run with `ubi gh leynos/rstest-bdd list-cache-entries` to confirm
that the keys and the `sccache` objects landed on Ubicloud. The runner
assignments, cache ownership, save policy, and prerequisite ordering are
enforced by `tests/workflow_contracts/runner_placement_test.py` and
`tests/workflow_contracts/runner_cache_test.py`.

### Validation (2026-09-03)

The focused `make test-workflow-contracts` gate passes, including the
byte-for-byte baseline that separates this addendum from the historical update.
The complete deterministic repository gate also passed locally. Exact-head CI
evidence on the migrated runners is recorded in PR #710 once the shared-actions
installer revision it depends on is merged and the branch is pushed.

## Known limitations

The adopted lint does not replace Clippy. `clippy::shadow_reuse`,
`clippy::expect_used`, and `clippy::unwrap_used` remain separate policy
surfaces. `.expect(...)` and `panic!(...)` are allowed in recognized tests,
while `.unwrap()` remains denied. The playbook form is chosen because it
satisfies the non-test policy surfaces together, not because Whitaker enforces
shadowing or `.expect(...)` directly.
