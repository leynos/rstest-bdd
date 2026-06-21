# Lint-clean GPUI playbook variant under a pedantic lint profile, enforced by Whitaker (10.2.5)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: EXECUTING

## Purpose / big picture

Roadmap item 10.2.5 asks for a playbook variant that "compiles under a pedantic
lint profile, including `clippy::shadow_reuse`, `clippy::expect_used`, and the
in-house `no_unwrap_or_else_panic` lint", with the finish line that the
`docs/users-guide.md` playbook "offers a no-shadowing,
no-`unwrap_or_else`-panic accessor variant using `let … else { panic!(…) }`".

The in-house `no_unwrap_or_else_panic` lint is **real**: it is one of the lints
in **Whitaker** (<https://github.com/leynos/whitaker>), a Dylint lint library
(current stable tag `v0.2.5`). It denies `unwrap_or_else(|| panic!(…))` (and
the nested `unwrap_or_else(|| value.unwrap())` form) on `Option`/`Result`,
including in tests. So this item is not a documentation-only change: we will
**enforce the real lint in the gate**, workspace-wide, and remove the pattern
from the codebase.

A pivotal discovery sharpens the design. This workspace denies
`clippy::unwrap_used` and `clippy::expect_used` with **no**
`allow-expect-in-tests` exemption (`clippy.toml` only sets
`cognitive-complexity-threshold`). That is exactly why the 38 existing
`unwrap_or_else(|| panic!(…))` sites exist — once `.unwrap()`/`.expect()` were
banned, contributors reached for `unwrap_or_else(|| panic!())` as the escape
hatch. Whitaker now closes that hatch. The single form that satisfies **all
three** lints at once — `clippy::expect_used`, `clippy::unwrap_used`, and
Whitaker `no_unwrap_or_else_panic` — and also `clippy::shadow_reuse` when a
fresh binding name is used, is:

```text
let Some(x) = option else { panic!("invariant message"); };
```

(Or `?`/error propagation where a `Result` is in scope.) So the conversion rule
is uniform, and the playbook's recommended accessor becomes this `let … else`
form — verified by the real lint, not asserted in prose.

Scope decision (confirmed with the maintainer): adopt **only**
`no_unwrap_or_else_panic` now, workspace-wide, wired into the core `make lint`
gate. Full adoption of the rest of the Whitaker suite (eight further lints such
as `module_max_lines`, `no_expect_outside_tests`, `bumpy_road_function`) is a
**separate, larger initiative** and is added to the roadmap as a new v0.6.1
item, not done here.

You can observe success as follows:

1. `make lint` runs the real Whitaker `no_unwrap_or_else_panic` lint over the
   workspace and passes; before the conversion it fails on the 38 known sites
   (red proof the lint is live).
2. `make test` continues to pass; the GPUI regression suite
   (`crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`) now uses the
   `let … else { panic!(…) }` accessor and still exercises the durable-handle
   scenarios.
3. Reading `docs/users-guide.md`, the stateful-GPUI playbook now teaches the
   `let … else { panic!(…) }` accessor as the form that passes the pedantic
   profile, and the "Lint-clean variant" teaser is replaced by the delivered,
   lint-enforced form. `docs/roadmap.md` marks 10.2.5 done and adds a v0.6.1
   full-Whitaker-adoption item.

## Constraints

Hard invariants; violation requires escalation, not a workaround.

- Public trait contracts must not change. This is a v0.6.0-beta3 quick-win
  (roadmap §10). Do not alter `StepContext`, `HarnessAdapter`, `GpuiHarness`,
  the reserved `rstest_bdd_harness_context` fixture key, or any macro surface.
- Behaviour must not change. Converting `unwrap_or_else(|| panic!(m))` to
  `let … else { panic!(m) }` (or `?`) must preserve the same panic/identical
  failure semantics and messages. The GPUI suite must still pass.
- Adopt **only** `no_unwrap_or_else_panic` from Whitaker in this item. Do
  **not**
  enable the rest of the suite (that is the new v0.6.1 roadmap item). The gate
  must be scoped to that single lint
  (`pattern = "crates/no_unwrap_or_else_panic"`).
- The repository stays on the **stable** toolchain for build, test, clippy, and
  docs (`rust-toolchain.toml` `channel = "stable"`). Dylint manages its own
  driver toolchain; do not migrate the whole repo to nightly.
- Do not suppress the new lint with blanket allows. Per-site `#[allow]` would
  also trip the workspace's `allow_attributes_without_reason = deny`; convert
  the code instead. A documented, narrowly-scoped exception (e.g. `dylint.toml`
  config) is permitted only if a site genuinely cannot be converted, and must
  be recorded in the Decision Log.
- British English with Oxford spelling in all prose.
- `make check-fmt`, `make lint`, and `make test` must all succeed before any
  CodeRabbit review and before each commit (commit gating).

## Tolerances (exception triggers)

- Feasibility spike (Stage 0): if the Whitaker/Dylint integration cannot be made
  to run locally and in a CI-equivalent within the spike, stop and escalate
  before converting any code. This is the highest-risk part; de-risk it first.
- Scope: the conversion is expected to touch ~14 Rust files (~38 sites) plus the
  Makefile, CI workflow, root `Cargo.toml`/`dylint.toml`, and docs. If it
  balloons past ~25 files or ~600 net lines (excluding docs/ADR prose), stop
  and escalate.
- Lint set: if delivering 10.2.5 appears to require enabling any Whitaker lint
  **other than** `no_unwrap_or_else_panic`, stop and escalate (that is the
  v0.6.1 item).
- Toolchain: if making the gate green requires migrating the repository off
  stable, or pulling in `rustc-codegen-cranelift`/extra components into the
  repo's own toolchain (as opposed to Dylint's managed driver), stop and
  escalate.
- Dependencies: a new Rust crate dependency in the workspace graph (beyond the
  dev-tooling `cargo-dylint`/`dylint-link` binaries and the git-sourced
  Whitaker lint library) is an escalation trigger.
- Iterations: if `make lint`/`make test` still fail after 3 focused attempts on
  a milestone, stop and escalate.

## Risks

- Risk R1 (central): Dylint integration is fragile. Whitaker's lint library at
  `v0.2.5` is built against a pinned nightly (`nightly-2025-09-18` per its
  `rust-toolchain.toml`) and needs `rustc-dev`/`rust-src`/`llvm-tools-preview`;
  `cargo-dylint` must build and load that driver. Weaver
  (<https://github.com/leynos/weaver>) found the `whitaker-installer` path
  (which also adds `rustc-codegen-cranelift`) more robust than raw
  `cargo dylint`. Severity: high. Likelihood: medium. Mitigation: **Stage 0
  spike** proves the integration before any conversion. Prefer the
  maintainer-chosen `[workspace.metadata.dylint]` + `cargo dylint` path scoped
  to `crates/no_unwrap_or_else_panic`; if that cannot be made to build the
  driver reliably, the spike evaluates the `whitaker-installer` path
  constrained to the single lint, and the choice is recorded in the Decision
  Log. Pin Whitaker by `tag = "v0.2.5"` and pin `cargo-dylint`/`dylint-link`
  versions (compatible with `dylint_linting = 5`).

- Risk R2: CI cost and flakiness. The first `cargo dylint` run builds the lint
  driver (minutes) and downloads a nightly. Severity: medium. Likelihood:
  medium. Mitigation: cache `~/.dylint`, the Cargo registry/git, and the dylint
  build target; pin everything; run the dylint step after the cheap gates so
  failures surface early. Mirror Weaver's CI install step shape.

- Risk R3: contributor friction. Every developer running `make lint` now needs
  `cargo-dylint`, `dylint-link`, and the pinned nightly installed. Severity:
  medium. Likelihood: high. Mitigation: document setup in
  `docs/developers-guide.md`; provide a `make` helper or documented one-liner
  to install the tooling; keep the dylint step a clearly labelled part of
  `make lint` so a failure message points at setup.

- Risk R4: hidden non-`panic!` forms. `crates/rstest-bdd/src/state.rs:71` uses
  `unwrap_or_else(|| unreachable!(…))` (production code). Whitaker's docs
  enumerate `panic!(..)` and nested `.unwrap()`, not `unreachable!`/`todo!`.
  Severity: low. Likelihood: medium. Mitigation: the spike and the red run
  reveal empirically whether the lint fires on `unreachable!`; convert it to
  `let … else { unreachable!(…) }` if so, otherwise leave it and note the
  boundary in Surprises.

- Risk R5: behaviour drift during conversion. A careless `let … else` rewrite
  could change control flow (e.g. binding scope, early-return vs panic).
  Severity: medium. Likelihood: low. Mitigation: keep each rewrite mechanical
  and message-preserving; rely on the green `make test` run (including the GPUI
  suite) to prove behaviour is intact; convert in small commits per file.

- Risk R6: `make fmt` markdown step is not idempotent (memory
  `make-fmt-markdown-not-idempotent`). Severity: low. Likelihood: medium.
  Mitigation: run `make markdownlint` after any `make fmt`, before committing
  doc changes.

## Progress

- [x] (Stage 0 — spike) Whitaker/Dylint integration proven locally and in a
  CI-equivalent; mechanism and pins chosen; go/no-go recorded. Local spike on
  2026-06-21 proved the real lint can run only via an explicit Whitaker
  build/copy/library-path flow for tag `v0.2.5`; implementation paused because
  the real lint expands the Rust conversion scope beyond this plan's original
  tolerance. Maintainer accepted the increased tolerance on 2026-06-21.
- [x] (Stage A) Orientation and this plan approved. Implementation requested on
  2026-06-21; branch tracking configured to
  `origin/10-2-5-playbook-variant-compiles-under-pedantic-lint-profile`; PR
  #546 title/body and Lody session title updated before code changes.
- [x] (Stage B — red) Gate wired so `make lint` runs the lint and fails on the
  38 known sites (proof the lint is active). The branch had already converted
  the Rust sites before the Makefile target was committed, so the committed
  `make lint` gate is green. Red evidence was captured during Stage 0 and early
  Stage C by running the same
  `cargo dylint --keep-going --lib no_unwrap_or_else_panic --no-metadata --no-build`
  invocation manually against the pre-conversion worktree, where it emitted
  real `no_unwrap_or_else_panic` diagnostics.
- [x] (Stage C — green) All Rust sites converted to
  `let … else { panic!(…) }` (or `?`); GPUI suite converted; `make lint`/
  `make test` green. Rust conversion portion completed on 2026-06-21 across all
  real Whitaker hits; playbook documentation remains pending. Evidence: the
  manual Dylint invocation for `no_unwrap_or_else_panic` over
  `--workspace --all-targets --all-features` passed with no lint diagnostics;
  `make check-fmt`, `make lint`, and `make test` passed. The Dylint run still
  reports the existing Tokio compatibility-alias deprecation warning under the
  nightly driver, but not as a failing diagnostic.
- [x] (Stage D — docs) design §2.7.6.2, ADR-013, developers guide, users-guide
  playbook, CI, roadmap tick 10.2.5 + new v0.6.1 item; full gates green;
  CodeRabbit clean.
  Documentation implementation on 2026-06-21 updated the user-guide playbook
  to make `let … else { panic!(…) }` the primary accessor form, updated
  design §2.7.6.2, added ADR-013, documented local/CI Whitaker setup in the
  developer guide, ticked roadmap 10.2.5, and added the v0.6.1 full-Whitaker
  suite follow-up item. Validation passed with `make markdownlint`,
  `make check-fmt`, `make lint`, `make test`, and `make nixie`; CodeRabbit
  review completed with zero findings.

## Surprises & discoveries

- Observation: `no_unwrap_or_else_panic` is a real Whitaker (Dylint) lint, not a
  hypothetical. Evidence: Whitaker README + User's Guide; the maintainer
  pointed to it and to Weaver as a reference integration. Impact: the
  verification is the real lint, not a textual proxy (the earlier draft's proxy
  is dropped).
- Observation: this repo bans `.unwrap()`/`.expect()` even in tests (no
  `allow-expect-in-tests`), so `unwrap_or_else(|| panic!())` was the only
  escape hatch; closing it forces `let … else { panic!(…) }` as the universal
  compliant form. Evidence: `Cargo.toml` lints + `clippy.toml`; 38 existing
  sites. Impact: uniform conversion rule; the playbook accessor is unambiguous.

- Observation: the documented `[workspace.metadata.dylint]` path builds
  Whitaker `v0.2.5`'s `no_unwrap_or_else_panic` crate, but it does not produce
  a loadable lint library in this repository. Without explicit features, the
  crate lacks the `dylint_version` entrypoint; Dylint metadata also rejected
  adding `features = ["dylint-driver"]` to the git library entry. The working
  local mechanism is Whitaker's own Makefile shape: clone tag `v0.2.5`, build
  package `no_unwrap_or_else_panic` with `--features dylint-driver` under
  `nightly-2025-09-18`, copy `libno_unwrap_or_else_panic.so` to
  `libno_unwrap_or_else_panic@nightly-2025-09-18-x86_64-unknown-linux-gnu.so`,
  then run
  `cargo dylint --lib no_unwrap_or_else_panic --no-metadata --no-build` with
  `DYLINT_LIBRARY_PATH` pointing at that directory. Impact: Stage 0 should
  choose the explicit build/copy/library-path gate unless the maintainer
  prefers a Whitaker installer wrapper.

- Observation: the real lint found additional panicking `unwrap_or_else` sites
  beyond the 38 originally identified by the single-line text search. A broad
  search for `unwrap_or_else` forms whose closure panics or is unreachable
  spans about 41 Rust files, including
  `crates/rstest-bdd-harness/src/macrotest_support.rs` and
  `crates/rstest-bdd-patterns/src/capture.rs` in the first red Dylint run.
  Impact: the conversion scope breaches the plan's ~25-file tolerance, so
  implementation is paused for maintainer direction before widening the cleanup.

- Observation: the committed `make lint-whitaker` target must pass an absolute
  `DYLINT_LIBRARY_PATH`; a relative `target/whitaker/.../release` path fails
  before linting begins. Impact: the Makefile derives `WHITAKER_LIBRARY_DIR`
  with `abspath`, while leaving the checkout and build target under
  `target/whitaker` for cache locality.

- Observation: `make fmt` is currently not a clean documentation formatter for
  the whole repository. It completed Rust and Python formatting, then
  `markdownlint-cli2 --fix` reported pre-existing MD013/MD039 issues in
  unrelated documents. Impact: the unrelated formatter churn was reverted, the
  active ExecPlan line-length issue was fixed manually, and `make markdownlint`
  remains the authoritative Markdown gate for this task.

(Append further discoveries during execution.)

## Decision log

- Decision: enforce the real Whitaker `no_unwrap_or_else_panic` lint in
  `make lint` workspace-wide, rather than a textual proxy or doctest.
  Rationale: the lint exists and the maintainer chose workspace-wide
  enforcement in the core gate. The real lint is authoritative; a proxy would
  be redundant and weaker. Date/Author: 2026-06-18, planning agent
  (incorporating maintainer direction).

- Decision: adopt only `no_unwrap_or_else_panic` now; defer the remaining
  Whitaker suite to a new v0.6.1 roadmap item. Rationale: the full
  `whitaker --all` suite (nine lints) workspace-wide is a large multi-lint
  cleanup and likely a nightly migration — far beyond 10.2.5. The maintainer
  confirmed single-lint now + a subsequent v0.6.1 task. Date/Author:
  2026-06-18, planning agent (maintainer-confirmed).

- Decision: the converted `let … else { panic!(…) }` form becomes the playbook's
  primary accessor (not merely an alternative beside
  `unwrap_or_else(|| panic!())`). Rationale: once the lint is enforced
  workspace-wide, the regression suite can no longer use
  `unwrap_or_else(|| panic!())`, so the default form must convert too; the
  existing users-guide text already anticipated promoting the lint-clean
  variant to primary once verified against the suite. Date/Author: 2026-06-18,
  planning agent.

- Decision: keep the repository on stable; let Dylint manage its driver
  toolchain. Do not add a bespoke textual doc-sync script (the earlier draft's
  proxy): the real lint gates the compiled suite, and the playbook snippet
  follows the existing "mirror the suite identifier-for-identifier" discipline.
  Rationale: minimise blast radius and avoid redundant machinery. Date/Author:
  2026-06-18, planning agent.

- Decision: pause implementation after Stage 0 local proof because the real lint
  scope exceeds the ExecPlan tolerance. Rationale: the plan requires escalation
  if the conversion balloons past about 25 files or 600 net lines. The real
  lint's broad `unwrap_or_else` coverage expands the Rust conversion to roughly
  41 files before documentation, CI, and gate wiring. Continuing would violate
  the approved manage-by-exception rule. Options: (1) widen this ExecPlan to
  convert all real lint hits now, (2) narrow the lint gate temporarily with
  documented Dylint configuration if available, or (3) split the
  repository-wide conversion into staged commits/PRs and keep 10.2.5 focused on
  the GPUI playbook plus a smaller gate. Option (1) is the most direct route to
  a clean real lint gate; options (2) and (3) reduce immediate diff size but
  defer full enforcement. Date/Author: 2026-06-21, implementation agent.

- Decision: widen this ExecPlan to convert all real Whitaker
  `no_unwrap_or_else_panic` hits now. Rationale: the maintainer accepted the
  increased tolerance after the Stage 0 discovery. Workspace-wide enforcement
  remains the target, so partial cleanup would create a weaker gate and defer
  the main behavioural proof. Continue with the explicit Whitaker
  build/copy/library-path mechanism and iterative Dylint runs until the real
  lint is clean. Date/Author: 2026-06-21, implementation agent with maintainer
  approval.

- Decision: implement the Whitaker gate as an explicit Makefile build/copy/run
  flow rather than `[workspace.metadata.dylint]`. Rationale: Whitaker tag
  `v0.2.5` requires the lint crate to be built with the `dylint-driver`
  feature, and this repository's `cargo dylint` metadata path rejected that
  feature configuration and did not produce the suffixed library name Dylint
  loads. The Makefile target clones the pinned tag to
  `target/whitaker/no_unwrap_or_else_panic-src`, builds only the
  `no_unwrap_or_else_panic` package with `nightly-2025-09-18`, copies the
  release library to Dylint's expected suffixed name, and runs `cargo dylint`
  with `--lib no_unwrap_or_else_panic --no-metadata --no-build`. Date/Author:
  2026-06-21, implementation agent.

(Append further decisions during execution, especially the Stage 0 mechanism
choice and any `unreachable!` boundary finding.)

## Outcomes & retrospective

Roadmap item 10.2.5 is delivered. The workspace no longer contains real
Whitaker `no_unwrap_or_else_panic` hits, `make lint` now runs the pinned
Whitaker lint after Clippy, and the GPUI playbook teaches the tested
`let … else { panic!(…) }` accessor form as primary rather than as a future
variant. ADR-013 records the decision and contributor tooling cost; the roadmap
tracks full Whitaker-suite adoption separately under v0.6.1.

Validation passed on 2026-06-21 with `mbake validate Makefile`,
`make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
`make nixie`. CodeRabbit reviewed the Rust conversion, Whitaker gate, and docs
milestones with zero findings.

Retrospective: the highest-risk part was Dylint integration, not the mechanical
Rust rewrite. The metadata path was insufficient for Whitaker `v0.2.5`, so the
durable integration is an explicit build/copy/run target with an absolute
`DYLINT_LIBRARY_PATH`. Future Whitaker bumps should treat that path as the
contract to revalidate first.

## Context and orientation

You are in the `rstest-bdd` workspace (currently on the **stable** toolchain).
Key pieces:

- Whitaker (<https://github.com/leynos/whitaker>, tag `v0.2.5`): a Dylint lint
  library. The relevant lint, `no_unwrap_or_else_panic`, denies
  `unwrap_or_else(|| panic!(…))` and `unwrap_or_else(|| value.unwrap())` on
  `Option`/`Result`, including in tests (doctests exempt; `allow_in_main`
  configurable via `dylint.toml`). It lives in its own crate
  `crates/no_unwrap_or_else_panic` within Whitaker. Whitaker does **not**
  provide a shadowing lint — `clippy::shadow_reuse` is an upstream Clippy
  restriction lint.
- Weaver (<https://github.com/leynos/weaver>): the reference integration. It
  runs
  the **whole** Whitaker suite via the `whitaker-installer` + `whitaker --all`
  wrapper inside its `make lint`, pins a nightly repo-wide, and has no
  `[workspace.metadata.dylint]` block. We deliberately diverge: single lint, via
  `[workspace.metadata.dylint]` + `cargo dylint`, repo stays on stable.
- `Cargo.toml` (root): `[workspace.lints.clippy]` denies `unwrap_used`,
  `expect_used`, sets `pedantic = warn`,
  `allow_attributes_without_reason = deny`,
  `blanket_clippy_restriction_lints = deny`. `shadow_reuse` is **not** enabled
  (restriction, allow-by-default). `clippy.toml` has **no**
  `allow-expect-in-tests`.
- `Makefile`: `RUST_FLAGS ?= -D warnings`,
  `CARGO_FLAGS ?= --workspace --all-targets --all-features`. The `lint` target
  runs `cargo clippy $(CLIPPY_FLAGS)`, then `lint-python`, then the
  `check_*.py` scripts. `PATH` already includes `~/.cargo/bin`. The Whitaker
  step is added here.
- `.github/workflows/ci.yml`: a `build-test` matrix; the `Lint` step runs
  `make lint` under `if: matrix.tools` (Ubuntu lanes). Rust is set up via the
  shared `leynos/shared-actions/.github/actions/setup-rust` action. A new
  "Install Whitaker/Dylint" step is added before `Lint`, mirroring Weaver's
  install step shape (pinned revisions).
- The 38 `unwrap_or_else(|| panic!(…))` sites (by file):
  `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs` (11),
  `crates/rstest-bdd/tests/dump_registry.rs` (8),
  `crates/rstest-bdd/tests/skip.rs` (5),
  `crates/rstest-bdd/tests/diagnostics_fixture.rs` (2),
  `crates/rstest-bdd/tests/common/async_semantic_behaviour_support.rs` (2),
  `crates/cargo-bdd/src/registry/tests.rs` (2), and one each in
  `crates/rstest-bdd/tests/trybuild_macros.rs`, `step_error_common.rs`,
  `scenario_harness.rs`, `localization.rs`, `diagnostic_unused.rs`,
  `crates/rstest-bdd-macros/src/validation/steps/tests.rs`,
  `crates/rstest-bdd-macros/src/codegen/scenario/runtime/tests/support.rs`,
  `crates/rstest-bdd-harness-gpui/tests/scenario_name_in_logs.rs`. Plus one
  `unwrap_or_else(|| unreachable!(…))` in `crates/rstest-bdd/src/state.rs:71`
  (production; see Risk R4).
- `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs` and the playbook in
  `docs/users-guide.md` ("Stateful GPUI scenarios with durable handles", ~lines
  1088–1460) mirror each other identifier-for-identifier; the playbook's
  "Lint-clean variant" subsection (~1409–1433) is currently a teaser pointing
  at this roadmap item.
- `docs/rstest-bdd-design.md` §2.7.6.2 carries the schematic interim pattern
  (uses `.ok_or`/`unwrap_or_else`) and already mentions a future lint-clean
  variant.
- `docs/documentation-style-guide.md`: ADR naming `adr-NNN-short-description.md`
  in `docs/`; next free number is **ADR-013**.
- `docs/developers-guide.md`: home for internally facing conventions (it already
  documents the GPUI mapping-table check); the Whitaker setup/run convention
  belongs here.

Terms defined:

- **Dylint**: a tool (Trail of Bits) that runs custom lints from dynamic
  libraries via `cargo dylint`, managing its own driver toolchain.
- **Restriction lint** (Clippy): allow-by-default, fires only when enabled (e.g.
  `shadow_reuse`, `expect_used`, `unwrap_used`).
- **let-else**: `let PATTERN = EXPR else { DIVERGING_BLOCK };` (stable since
  Rust
  1.65). With a fresh binding name it introduces no shadow and uses no `unwrap`/
  `expect`/`unwrap_or_else`.

## Plan of work

Stage 0 — prototyping spike (de-risk the integration; no production conversion).

1. Locally install `cargo-dylint` and `dylint-link` (pinned versions compatible
   with `dylint_linting = 5`).
2. Add a throwaway `[workspace.metadata.dylint]` entry pinned to Whitaker
   `tag = "v0.2.5"`, `pattern = "crates/no_unwrap_or_else_panic"`, and run
   `cargo dylint --all -- --workspace --all-targets --all-features`. Confirm it
   (a) builds/loads the driver, (b) **fails** on a known
   `unwrap_or_else(|| panic!())` site, and (c) **passes** a
   `let … else { panic!() }` sample. Record the exact working command and any
   required toolchain components.
3. Decide the mechanism: `[workspace.metadata.dylint]` + `cargo dylint`
   (preferred)
   versus the `whitaker-installer` path constrained to the single lint. Record
   in the Decision Log. Determine whether `unreachable!` is flagged (Risk R4).
   Go/no-go: do not proceed to conversion until the lint runs green/red as
   expected.

Stage A — orientation (this plan; approval gate).

Stage B — red. Wire the gate so the lint is live and failing:

1. Add the chosen `[workspace.metadata.dylint]` block to root `Cargo.toml`
   (pinned `tag = "v0.2.5"`, single-lint `pattern`), and an optional
   `dylint.toml` only if a documented config is needed.
2. Add the Whitaker step to the `make lint` recipe (after the existing checks),
   e.g. `cargo dylint --all -- $(CARGO_FLAGS)` with `RUSTFLAGS=$(RUST_FLAGS)`
   and any env the spike found necessary; add a `WHITAKER`/dylint variable
   block if helpful. Run `make lint` and observe it **fail** on the 38 sites —
   this is the red proof the lint is enforced.

Stage C — green. Convert and make the gate pass:

1. Convert all 38 `unwrap_or_else(|| panic!(m))` sites to
   `let … else { panic!(m); }` with a fresh binding name (preserving messages),
   or to `?`/error propagation where a `Result` is already threaded. Handle the
   `state.rs` `unreachable!` site per the Stage 0 finding. Commit per file or
   per small group; run the focused tests after each.
2. In `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`, convert
   `current_handles`, `read_counter_from_window`, and the two step bodies to the
   `let … else` accessor form. This becomes the lint-clean reference suite the
   playbook mirrors.
3. Run `make lint` (expect green, lint now satisfied) and `make test` (expect
   the
   GPUI suite and all unit/behavioural tests green — behaviour preserved).

Stage D — docs, ADR, roadmap, CI:

1. `docs/users-guide.md`: rewrite the stateful-GPUI worked example to use the
   `let … else { panic!(…) }` accessor (mirroring the converted suite
   identifier-for-identifier), and replace the "Lint-clean variant" teaser with
   the delivered, lint-enforced form. Note that the workspace now enforces
   `no_unwrap_or_else_panic` via Whitaker, so the form is mandatory, not
   optional. Keep the "which gpui" banner. Explain why `let … else` (not
   `.expect()`): `.expect()` is denied here too.
2. `docs/rstest-bdd-design.md` §2.7.6.2: convert the schematic to the
   `let … else`
   form and update the lint-clean note to state the gate is delivered; cite
   ADR-013.
3. Create `docs/adr-013-adopt-whitaker-no-unwrap-or-else-panic.md` (Y-Statement
   / style-guide format): Status `Accepted` + date; Context (downstream
   pedantic profiles; this repo's own `unwrap_or_else(|| panic!())` escape
   hatch arising from denying `expect`/`unwrap` even in tests; `let … else` is
   the universal compliant form); Decision (enforce the single Whitaker lint
   workspace-wide in `make lint` via Dylint, pinned to `v0.2.5`, repo stays on
   stable; full-suite adoption deferred to v0.6.1); Options Considered (textual
   proxy — rejected as redundant now the lint exists; full `whitaker --all`
   suite — deferred; `whitaker-installer` vs `cargo dylint` mechanism — per
   Stage 0); Consequences (contributor tooling + CI install + pin maintenance);
   Known Limitations (`unreachable!`/`map_or_else` coverage per Stage 0
   finding).
4. `docs/developers-guide.md`: add a section on installing and running Whitaker
   locally (the `cargo install` one-liner or `make` helper, the pinned nightly
   note, and the exact `make lint` behaviour), and the pin-maintenance
   procedure when bumping the Whitaker tag.
5. `.github/workflows/ci.yml`: add an "Install Whitaker/Dylint" step before the
   `Lint` step on the Ubuntu `tools` lanes, with pinned revisions and caching
   (mirror Weaver's install-step shape). Confirm Windows lanes (which skip
   `tools`) are unaffected.
6. `docs/roadmap.md`: mark 10.2.5 done (`- [x] 10.2.5. …` with a "Delivered
   (date):" note pointing at this ExecPlan), and **add a new v0.6.1 item**
   under §11 for full Whitaker-suite adoption (enumerate the remaining eight
   lints as the scope, note the likely nightly migration, and reference this
   ExecPlan as the precedent integration).
7. Run the full gates sequentially; request CodeRabbit; clear all concerns.

Each stage ends with validation; do not proceed past a failing stage.

## Concrete steps

Run from the worktree root. Log to `/tmp` per convention:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-rstest-bdd-$(git branch --show-current).out
make lint      2>&1 | tee /tmp/lint-rstest-bdd-$(git branch --show-current).out
make test      2>&1 | tee /tmp/test-rstest-bdd-$(git branch --show-current).out
make markdownlint 2>&1 | tee /tmp/mdlint-rstest-bdd-$(git branch --show-current).out
```

Stage 0 spike (record the exact working invocation):

```bash
cargo install --locked cargo-dylint dylint-link   # pin versions per Stage 0
# after adding the throwaway [workspace.metadata.dylint] entry:
cargo dylint --all -- --workspace --all-targets --all-features
```

Find remaining sites during conversion:

```bash
grep -rnE "unwrap_or_else\(\|\| *(panic!|unreachable!|todo!|unimplemented!)" \
  --include=*.rs .
grep -rnE "unwrap_or_else\(\|\|[^)]*\.unwrap\(\)" --include=*.rs .
```

Do not run lint/format/test suites in parallel (shared build cache); run
sequentially.

## Validation and acceptance

Red-Green-Refactor evidence to record:

- Red: after Stage B wires the gate, `make lint` fails with
  `no_unwrap_or_else_panic` diagnostics on the known sites (capture a short
  transcript). This proves the lint is enforced before any conversion.
- Green: after Stage C, `make lint` passes (lint satisfied) and `make test`
  passes (GPUI suite + all tests — behaviour preserved). Capture the focused
  GPUI suite result.
- Refactor/docs: after Stage D, `make check-fmt`, `make lint`, `make test`, and
  `make markdownlint` all pass; CI install step verified (or escalated if the
  sandbox cannot run it — see below).

Two-tier acceptance (separate what every environment can prove from what needs
CI / native GPUI):

- Always (this sandbox): `cargo dylint` runs the lint and the converted
  workspace passes it; `make test` passes for non-GPUI crates and the ungated
  tests; `make check-fmt`/`make markdownlint` pass.
- CI / native-GPUI: the `native-gpui-tests` suite runs and passes (roadmap
  §10.1.3 confirms the environment exists in CI); the CI Whitaker install step
  succeeds. If the local sandbox cannot run native GPUI or the dylint CI step,
  do **not** mark those done from compilation alone — escalate per Tolerances.

Quality criteria ("done"):

- Lint: `make lint` passes, including
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  **and** the Whitaker `no_unwrap_or_else_panic`
  pass over the workspace.
- Tests: `make test` passes (including the converted GPUI suite); no behaviour
  change.
- Docs: `make markdownlint` passes; users-guide playbook teaches the
  `let … else`
  accessor; design §2.7.6.2, ADR-013, developers guide updated; roadmap 10.2.5
  ticked and the v0.6.1 full-suite item added.
- No new workspace dependency; no public API change; repo stays on stable.

Test rigour judgement (per the task's testing menu):

- `rstest-bdd` behavioural test: the existing GPUI durable-handle scenarios are
  the behavioural coverage; they must still pass after conversion. No new
  scenario is required — the change is a lint-shape refactor, not new behaviour.
- `rstest` unit tests: add a focused unit test only if the conversion introduces
  a reusable accessor helper (e.g. an `expect_stored<T>(Option<T>, &str) -> T`
  used by the GPUI suite) — then cover happy + `#[should_panic]` unhappy paths
  with `googletest`/`pretty_assertions`. If no shared helper is introduced, the
  per-site `let … else` conversions are covered by existing tests.
- The Whitaker gate itself is the verification of `no_unwrap_or_else_panic`;
  the red run is its falsification evidence.
- `insta`/`proptest`/`kani`/`verus`: not warranted — no multivariant rendered
  output and no new invariant over an input range; conversions are mechanical
  and message-preserving, covered by the existing suite.

## Idempotence and recovery

Conversions are mechanical and committed per file/group, so any step reverts via
`git revert`. The Makefile/CI/`Cargo.toml` edits are localised. If `make fmt`
trips Markdown lints, run `make markdownlint` and fix before committing (Risk
R6). The Stage 0 spike changes are throwaway — remove the experimental
`metadata.dylint` entry before committing the real one.

## Artifacts and notes

Conversion shape (the one form that satisfies clippy `expect_used`/
`unwrap_used`, clippy `shadow_reuse`, and Whitaker `no_unwrap_or_else_panic`):

```text
opt.unwrap_or_else(|| panic!("m"))
  → let Some(x) = opt else { panic!("m"); };     // fresh binding `x`

VisualTestContext::from_window(window, cx).unwrap_or_else(|| panic!("m"))
  → let Some(mut visual_context) = VisualTestContext::from_window(window, cx)
        else { panic!("m"); };
```

Research citations to fold into ADR-013:

- Whitaker (Dylint lint library), tag `v0.2.5`, lint `no_unwrap_or_else_panic`
  — <https://github.com/leynos/whitaker>.
- Weaver reference integration (full-suite, `make lint`) —
  <https://github.com/leynos/weaver>.
- Trail of Bits Dylint — <https://github.com/trailofbits/dylint>.
- Clippy `shadow_reuse`/`expect_used`/`unwrap_used`; `expect_fun_call`
  recommends
  `unwrap_or_else(|| panic!())` (which is why no built-in Clippy lint catches
  it) — <https://rust-lang.github.io/rust-clippy/master/index.html>.
- let-else stabilised in Rust 1.65 —
  <https://blog.rust-lang.org/2022/11/03/Rust-1.65.0/>.

## Interfaces and dependencies

Edited files: root `Cargo.toml` (`[workspace.metadata.dylint]`), optional
`dylint.toml`, `Makefile` (Whitaker step in `lint`), `.github/workflows/ci.yml`
(install step + cache), the ~14 Rust files containing the 38 sites,
`crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`,
`docs/users-guide.md`, `docs/rstest-bdd-design.md` (§2.7.6.2),
`docs/developers-guide.md`, `docs/roadmap.md` (tick 10.2.5 + new v0.6.1 item).

New files: `docs/adr-013-adopt-whitaker-no-unwrap-or-else-panic.md`.

Tooling dependencies (dev/CI only, not in the workspace crate graph):
`cargo-dylint`, `dylint-link`, and the git-sourced Whitaker lint library pinned
to `tag = "v0.2.5"`. The repository toolchain stays `stable`; Dylint manages
its own nightly driver.

## Signposted documentation and skills

- Skills: `leta`, `rust-router` (loaded); `execplans` (this doc); `rust-errors`
  (panic boundary / `?` vs panic when converting sites); `rust-unit-testing`
  (if a shared accessor helper warrants happy/unhappy tests);
  `arch-decision-records` (ADR-013); `commit-message`, `pr-creation` (delivery);
  `logisphere-design-review` (used to review this plan).
- Docs: `docs/users-guide.md` (deliverable), `docs/rstest-bdd-design.md`
  §2.7.6.1/§2.7.6.2, `docs/developers-guide.md`,
  `docs/documentation-style-guide.md`,
  `docs/complexity-antipatterns-and-refactoring-strategies.md` (guard-clause /
  small-function rationale for any extracted accessor),
  `docs/rust-testing-with-rstest-fixtures.md`. External: Whitaker and Weaker
  repos above.

## Revision note

Initial draft (2026-06-17): textual-proxy verification (assumed no real lint).

Revision 2 (2026-06-17, Logisphere review): ungated accessor, reuse existing
feature file, explicit doc-sync normalisation, two-tier acceptance.

Revision 3 (2026-06-18): **major redirection.** The maintainer identified
`no_unwrap_or_else_panic` as a real Whitaker (Dylint) lint and chose
workspace-wide enforcement in core `make lint`, with full-suite adoption
deferred to a new v0.6.1 roadmap item. The plan now enforces the real lint (not
a proxy), converts the 38 existing `unwrap_or_else(|| panic!())` sites to
`let … else { panic!(…) }` (the only form that also satisfies the workspace's
`expect_used`/`unwrap_used`/`shadow_reuse` profile, since `.expect()`/
`.unwrap()` are denied even in tests), adds a Stage 0 integration spike to
de-risk Dylint, keeps the repo on stable, and reframes ADR-013 as "adopt
Whitaker no_unwrap_or_else_panic in the lint gate". The bespoke textual
doc-sync script is dropped. Awaiting user approval before implementation.
