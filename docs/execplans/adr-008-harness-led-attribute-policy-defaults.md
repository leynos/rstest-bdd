# ExecPlan: implement ADR-008 harness-led attribute-policy defaults

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IMPLEMENTED

`PLANS.md` is not present in this repository at the time of writing, so this
ExecPlan is the governing plan for ADR-008 delivery once the ADR is accepted
and the user approves execution.

## Purpose / big picture

ADR-008 changes the common first-party integration workflow for `#[scenario]`
and `scenarios!`. After this work, a user who selects a first-party harness
does not need to repeat the matching first-party attribute policy just to get
the normal emitted test attributes. These forms should work as the preferred
documentation path:

```rust,no_run
# use rstest_bdd_macros::scenario;
#[scenario(
    path = "tests/features/my_async.feature",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn my_tokio_scenario() {}
```

and:

```rust,no_run
# use rstest_bdd_macros::scenario;
#[scenario(
    path = "tests/features/my_ui.feature",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
)]
fn my_gpui_scenario() {}
```

Explicit `attributes = ...` remains authoritative, `attributes`-only
configuration remains supported, and unknown third-party harnesses must still
fall back to explicit policy selection rather than pretending the macro can
infer arbitrary defaults.

Success is observable in three ways:

1. Unit tests prove the exact precedence order from ADR-008:
   explicit `attributes` override, known first-party harness defaults come
   next, the deprecated Tokio runtime alias remains below explicit harness
   selection, and the final fallback is the existing runtime-mode or
   synchronous behaviour.
2. Public-macro coverage proves the harness-only first-party forms compile and
   run for both `#[scenario]` and `scenarios!`, while explicit override cases
   still behave correctly.
3. `docs/users-guide.md` clearly teaches harness-led defaults as the normal
   first-party configuration and still documents `attributes = ...` as the
   override and third-party escape hatch.

## Constraints

- Implement only ADR-008 and roadmap workstream 9.7. Do not fold unrelated
  harness redesign, runtime alias redesign, or new third-party extension
  machinery into this change.
- Preserve the architectural separation from ADR-005: `HarnessAdapter`
  remains the runtime delegation boundary and `AttributePolicy` remains the
  emitted-attribute boundary.
- Preserve the exact ADR-008 precedence order:
  1. explicit `attributes = ...`
  2. known first-party `harness = ...` mapping
  3. deprecated `runtime = "tokio-current-thread"` compatibility alias
  4. existing runtime-mode or synchronous fallback
- Keep `attributes`-only and `harness`-only configurations supported.
- Keep the current path-based trust model. The macro must not attempt to
  execute arbitrary third-party `AttributePolicy::test_attributes()`
  implementations at expansion time.
- Preserve the current deduplication rules for emitted `#[tokio::test]` and
  `#[gpui::test]`.
- Do not add new external dependencies.
- Cover the feature with unit tests and behavioural tests. Because emitted
  attributes are a compile-time concern, include trybuild coverage as the
  public-macro proof layer between those two.
- Update `docs/users-guide.md` as part of the implementation. Update
  `docs/rstest-bdd-design.md` as well so the internal design record matches the
  shipped behaviour.
- Run all applicable gates before considering the work complete:
  `make fmt`, `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
  and `make nixie`.
- Run long-lived commands with `set -o pipefail` and `tee` so failures are
  preserved in log files.

## Tolerances

- Scope: if the implementation grows beyond 16 files changed or 900 net
  lines, stop and re-check whether the work has turned into a broader
  harness-policy refactor.
- Interface: if shipping ADR-008 requires new user-facing macro syntax,
  changes to `HarnessAdapter` or `AttributePolicy`, or a new public crate API,
  stop and split that API work into a separate decision.
- Dependencies: if any new external crate is required, stop and escalate.
- Resolution model: if the first-party harness mapping cannot live in shared
  non-proc-macro code without introducing a dependency cycle, stop and record
  the exact cycle before continuing.
- Behavioural proof: if the public-macro behaviour cannot be shown with the
  existing `rstest-bdd` integration suite and trybuild harness, stop before
  inventing a second bespoke test runner.
- Validation: if `make test` fails for an unrelated pre-existing reason, such
  as the known workspace-level nextest timeout in `cargo-bdd::cli`, capture the
  log and stop before claiming completion.
- Iterations: if the same gate fails three consecutive times after attempted
  fixes, stop and escalate with the log path and current hypothesis.
- Governance: do not mark roadmap item 9.7 complete unless ADR-008 has moved
  out of `Proposed` status or the maintainers explicitly authorize delivery
  while the ADR status lags.

## Risks

- Risk: the macro currently resolves test attributes from explicit
  `attributes` or `RuntimeMode` only, so a narrow patch in one call site could
  leave `#[scenario]` and `scenarios!` with different precedence. Severity:
  high. Likelihood: medium. Mitigation: drive both macro entry points through
  one shared resolution helper and test the full precedence matrix.
- Risk: Tokio harness-only scenarios remain synchronous, so inferred Tokio
  attributes are filtered out for sync signatures. A runtime-only test could
  pass without proving the new inference logic. Severity: high. Likelihood:
  high. Mitigation: add unit tests for hint resolution and trybuild coverage
  for emitted-attribute cases, then use behavioural tests as smoke tests over
  the public macros.
- Risk: the deprecated runtime alias already resolves to `TokioHarness` inside
  `scenarios!`. If the new harness-led default logic keys off the wrong value,
  explicit harnesses and the alias could interfere with one another. Severity:
  medium. Likelihood: medium. Mitigation: write direct helper tests for
  explicit-harness precedence over the alias before changing the code.
- Risk: GPUI and Tokio tests rely on feature-gated or framework-specific
  suites that are easy to edit without running. Severity: medium. Likelihood:
  medium. Mitigation: include focused `cargo test` commands for the exact
  suites as mandatory validation, not optional spot checks.
- Risk: `make test` uses `cargo-nextest`, which skips
  `crates/rstest-bdd/tests/trybuild_macros.rs`. Severity: high. Likelihood:
  high. Mitigation: keep a separate
  `cargo test -p rstest-bdd --test trybuild_macros step_macros_compile -- --exact`
   command in the required validation recipe.
- Risk: the user guide could drift if only the ADR and design doc are updated.
  Severity: medium. Likelihood: medium. Mitigation: make the user-guide update
  a first-class milestone and do not close the work until its examples and
  preference order are updated.

## Progress

- [x] (2026-04-09) Reviewed ADR-008, roadmap item 9.7, and adjacent execplans
      for policy emission and GPUI coverage.
- [x] (2026-04-09) Reviewed the current user-guide and design-doc sections for
      harness and attribute-policy behaviour.
- [x] (2026-04-09) Inspected the current implementation surface in
      `rstest-bdd-policy`, `rstest-bdd-macros`, and the existing Tokio/GPUI
      test suites.
- [x] (2026-04-09) Drafted this ExecPlan.
- [x] (2026-04-11) Stage A: add shared first-party harness-path hint
      resolution in `rstest-bdd-policy`.
- [x] (2026-04-11) Stage B: refactor macro test-attribute resolution to honour
      ADR-008 precedence for both `#[scenario]` and `scenarios!`.
- [x] (2026-04-11) Stage C: add unit, trybuild, and behavioural coverage for
      harness-led defaults and explicit overrides.
- [x] (2026-04-11) Stage D: update `docs/users-guide.md` and
      `docs/rstest-bdd-design.md` to teach the delivered behaviour.
- [x] (2026-04-11) Stage E: run focused validation plus repository-wide gates.

## Surprises & Discoveries

- Observation: `crates/rstest-bdd-policy/src/lib.rs` already centralizes the
  canonical first-party attribute-policy path mapping through
  `resolve_test_attribute_hint_for_policy_path()`. Impact: the smallest,
  lowest-risk ADR-008 implementation is to add first-party harness mapping in
  the same shared crate instead of reintroducing macro-local mapping tables.
- Observation: `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs`
  currently resolves attributes with
  `attributes.map_or_else(|| runtime .test_attribute_hint(), ...)`. Impact:
  harness-led defaults do not exist yet, and there is one clear helper boundary
  to refactor.
- Observation: `scenarios!` already resolves the deprecated runtime alias to a
  concrete harness path in
  `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs` before it
  constructs `ScenarioConfig`. Impact: the shared precedence helper should
  operate on `ScenarioConfig.harness`, not duplicate alias logic inside
  `generate_test_attrs()`.
- Observation: `crates/rstest-bdd/tests/scenario_harness_tokio.rs` and
  `crates/rstest-bdd/tests/scenario_harness_gpui.rs` already contain harness
  smoke tests for `#[scenario]`, but they do not define the full harness-led
  default and override matrix. Impact: extend those suites rather than create
  new standalone integration binaries.
- Observation: `make test` skips the trybuild macro suite under nextest.
  Impact: the implementation must always run a focused `cargo test` command for
  `trybuild_macros` in addition to the normal repository gates.
- Observation: `scenarios!` lowers the deprecated Tokio runtime alias to a
  synchronous effective runtime before `ScenarioConfig` reaches shared codegen.
  Impact: preserving ADR-008 precedence required a second runtime field
  dedicated to attribute resolution so the alias hint survives while the
  generated function signature remains synchronous.

## Decision Log

- Decision: place first-party harness-to-hint mapping in
  `crates/rstest-bdd-policy/src/lib.rs` next to the existing policy-path
  mapping. Rationale: the mapping is shared semantic policy data, not macro
  codegen logic, and keeping it in the shared crate prevents `#[scenario]` and
  `scenarios!` from drifting apart. Date/Author: 2026-04-09 / Codex.
- Decision: implement ADR-008 precedence in one macro-local helper that
  receives runtime, resolved harness path, and explicit attribute-policy path.
  Rationale: the precedence rules are subtle enough that duplicating them
  across call sites would be a regression trap. Date/Author: 2026-04-09 / Codex.
- Decision: use trybuild fixtures as the primary public proof of emitted
  attribute inference, then use behavioural tests as end-to-end smoke tests.
  Rationale: emitted attributes are mostly visible at compile time, especially
  for Tokio where sync signatures filter out `#[tokio::test]`, so runtime-only
  assertions are not sufficient. Date/Author: 2026-04-09 / Codex.
- Decision: update the user guide to lead with harness-only first-party
  examples, while still retaining one explicit-override example per framework
  and the third-party caveat. Rationale: this is the user-visible point of the
  ADR and must be obvious in the primary guide, not hidden only in the design
  doc. Date/Author: 2026-04-09 / Codex.

## Outcomes & Retrospective

ADR-008 shipped on 2026-04-11 with the intended harness-led default behaviour.
`rstest-bdd-policy` now owns both first-party policy-path and harness-path
resolution, mapping:

- `rstest_bdd_harness::StdHarness` -> rstest-only
- `rstest_bdd_harness_tokio::TokioHarness` -> Tokio current-thread
- `rstest_bdd_harness_gpui::GpuiHarness` -> GPUI test

`rstest-bdd-macros` now resolves emitted test attributes in the documented
precedence order:

1. explicit `attributes = ...`
2. known first-party `harness = ...` default mapping
3. deprecated `runtime = "tokio-current-thread"` compatibility alias
4. runtime-mode or synchronous fallback

The macro layer gained focused unit coverage for the full precedence matrix, a
separate test module to keep file sizes within repository limits, compile-pass
fixtures for first-party harness-only forms, and behavioural integration
coverage for harness-only `scenarios!` flows in both Tokio and GPUI suites. The
user guide and design document now teach harness-only first-party configuration
as the normal path while retaining explicit `attributes = ...` as the override
and third-party escape hatch.

Validation completed with:

- `set -o pipefail; cargo test -p rstest-bdd-policy 2>&1 | tee /tmp/adr008-policy-test.log`
- `set -o pipefail; cargo test -p rstest-bdd-macros --lib 2>&1 | tee /tmp/adr008-macros-lib-test.log`
- `set -o pipefail; cargo test -p rstest-bdd --test scenario_harness_tokio`
  `2>&1 | tee /tmp/adr008-tokio-int-test.log`
- `set -o pipefail; cargo test -p rstest-bdd --test scenario_harness_gpui`
  `--features gpui-harness-tests` `2>&1 | tee /tmp/adr008-gpui-int-test.log`
- `set -o pipefail; cargo test -p rstest-bdd --test trybuild_macros`
  `step_macros_compile -- --exact 2>&1 | tee /tmp/adr008-trybuild.log`
- `set -o pipefail; make fmt 2>&1 | tee /tmp/adr008-make-fmt-2.log`
- `set -o pipefail; make check-fmt 2>&1 | tee /tmp/adr008-make-check-fmt-2.log`
- `set -o pipefail; make lint 2>&1 | tee /tmp/adr008-make-lint-3.log`
- `set -o pipefail; make markdownlint 2>&1 | tee /tmp/adr008-make-markdownlint.log`
- `set -o pipefail; make nixie 2>&1 | tee /tmp/adr008-make-nixie.log`
- `set -o pipefail; make test 2>&1 | tee /tmp/adr008-make-test.log`

No follow-on implementation was required to close this ADR scope.

## Context and orientation

The implementation touches one shared policy crate, one macro-layer resolution
module, the existing public test suites, and the main user-facing documents.

`crates/rstest-bdd-policy/src/lib.rs` currently defines:

- `RuntimeMode`
- `TestAttributeHint`
- `DEFAULT_ATTRIBUTE_POLICY_PATH`
- `TOKIO_ATTRIBUTE_POLICY_PATH`
- `GPUI_ATTRIBUTE_POLICY_PATH`
- `resolve_test_attribute_hint_for_policy_path()`

This is the natural place to add canonical first-party harness path constants
and a matching harness-path resolver.

`crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs` currently owns
attribute emission:

- `resolve_attribute_hint_from_policy_path()`
- `resolve_attribute_policy()`
- `generate_test_attrs()`

Today, `resolve_attribute_policy()` only sees `RuntimeMode` and the explicit
`attributes` path. ADR-008 requires this module to honour explicit
`attributes`, then inferred first-party harness defaults, then the runtime
fallbacks.

`crates/rstest-bdd-macros/src/codegen/scenario.rs` calls
`generate_test_attrs()` from both `generate_regular_scenario_code()` and
`generate_outline_scenario_code()`. `ScenarioConfig` already carries both
`harness` and `attributes`, so the `#[scenario]` path can pass the selected
harness directly once the helper signature is updated.

`crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs` resolves the
deprecated `runtime = "tokio-current-thread"` alias through
`resolve_harness_path()` before it builds `ScenarioConfig`. That means the
`scenarios!` flow can reuse the same `generate_test_attrs()` precedence logic
without learning anything special about the alias.

The public test surface is already in place:

- `crates/rstest-bdd-macros/src/codegen/scenario/tests.rs`
- `crates/rstest-bdd-macros/src/codegen/scenario/tests/gpui_policy.rs`
- `crates/rstest-bdd-macros/src/macros/scenarios/test_generation/tests.rs`
- `crates/rstest-bdd/tests/trybuild_macros.rs`
- `crates/rstest-bdd/tests/scenario_harness_tokio.rs`
- `crates/rstest-bdd/tests/scenario_harness_gpui.rs`
- `crates/rstest-bdd/tests/fixtures_macros/`

The documentation that must change is:

- `docs/users-guide.md`
- `docs/rstest-bdd-design.md`

If roadmap status is updated after execution, the relevant file is
`docs/roadmap.md`, but only if ADR-008 has been accepted or the maintainers
explicitly authorize roadmap closure while the ADR status trails behind.

## Plan of work

### Stage A: add shared harness-path hint resolution

Goal: extend the shared policy crate so first-party harness types map to the
same `TestAttributeHint` values already used by the first-party attribute
policies.

Implementation details:

1. Add canonical path constants for the first-party harness types:
   `StdHarness`, `TokioHarness`, and `GpuiHarness`.
2. Add a shared lookup table and a public helper such as
   `resolve_test_attribute_hint_for_harness_path()`.
3. Keep the helper path-based and segment-based, matching the current
   `resolve_test_attribute_hint_for_policy_path()` contract.
4. Add unit tests for:
   - the three canonical first-party harness paths
   - unknown third-party harness paths
   - partial-name or wrong-prefix paths that must not match

Go/no-go validation:

- `cargo test -p rstest-bdd-policy` passes with the new resolver tests.

### Stage B: teach macro codegen ADR-008 precedence

Goal: make `generate_test_attrs()` resolve the effective attribute hint from
explicit policy, inferred first-party harness default, and the existing runtime
fallback, in that order.

Implementation details:

1. Refactor `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs` so
   the resolution helper accepts:
   - `runtime: RuntimeMode`
   - `harness: Option<&syn::Path>`
   - `attributes: Option<&syn::Path>`
2. Keep the exact ADR-008 order in one helper:
   - explicit `attributes`
   - known first-party harness
   - runtime-derived hint
3. Reuse the existing `syn::Path` segment extraction approach for both policy
   and harness paths so absolute paths remain normalized naturally through
   `syn::Path::segments`.
4. Update both `generate_regular_scenario_code()` and
   `generate_outline_scenario_code()` to pass `config.harness`.
5. Preserve the current emitted-attribute filtering:
   - do not emit `#[tokio::test]` for sync signatures
   - do not duplicate an explicit user-written `#[tokio::test]`
   - do not duplicate an explicit user-written `#[gpui::test]`
6. Add unit tests in the macro crate for:
   - `StdHarness` infers rstest-only output
   - `TokioHarness` infers Tokio policy when the signature is async
   - `GpuiHarness` infers GPUI policy for sync and async cases
   - explicit `attributes = rstest_bdd_harness::DefaultAttributePolicy`
     overrides `TokioHarness` and `GpuiHarness`
   - explicit first-party framework policies override `StdHarness`
   - unknown third-party harnesses fall back to the runtime-derived hint
   - explicit harness still outranks the deprecated runtime alias once the
     resolved harness path reaches `ScenarioConfig`

Go/no-go validation:

- `cargo test -p rstest-bdd-macros --lib` passes.
- The new unit tests fail before the code change and pass after it.

### Stage C: add trybuild and behavioural coverage

Goal: prove the new behaviour through the public macro surface instead of only
through helper-level unit tests.

Implementation details:

1. Extend `crates/rstest-bdd/tests/trybuild_macros.rs` with new compile-pass
   fixtures for the harness-led default paths. The fixture set should cover:
   - `#[scenario]` with `TokioHarness` and no `attributes`
   - `#[scenario]` with `GpuiHarness` and no `attributes`
   - `scenarios!` with `TokioHarness` and no `attributes`
   - `scenarios!` with `GpuiHarness` and no `attributes`
   - at least one explicit override case where a first-party harness is paired
     with `rstest_bdd_harness::DefaultAttributePolicy`
2. Prefer compile-pass fixtures over snapshot-based compile-fail fixtures here,
   because the question is "does the generated code compile with the inferred
   test attributes?" rather than "what exact diagnostic text is emitted?"
3. Extend `crates/rstest-bdd/tests/scenario_harness_tokio.rs` with a
   `scenarios!`-based harness-only smoke test so both public entry points are
   exercised after the precedence refactor.
4. Extend `crates/rstest-bdd/tests/scenario_harness_gpui.rs` with a matching
   `scenarios!`-based harness-only smoke test and one explicit-override smoke
   test if needed to keep the override path live at runtime.
5. Keep the behavioural assertions small and framework-specific:
   Tokio tests should keep proving active runtime plus `spawn_local` support;
   GPUI tests should keep proving injected `TestAppContext` access and scenario
   execution.
6. Treat trybuild as the primary proof of inferred or overridden emitted
   attributes. Behavioural tests are there to prove that the public macros
   still execute correctly once the resolution logic is refactored.

Go/no-go validation:

- `RUSTFLAGS="-D warnings" cargo test -p rstest-bdd --test trybuild_macros
  step_macros_compile -- --exact` passes.
- `RUSTFLAGS="-D warnings" cargo test -p rstest-bdd --test
  scenario_harness_tokio` passes.
- `RUSTFLAGS="-D warnings" cargo test -p rstest-bdd --test
  scenario_harness_gpui --features gpui-harness-tests` passes.

### Stage D: update the user guide and design doc

Goal: make the shipped behaviour obvious to users and keep the design doc in
sync with the implementation.

Implementation details:

1. Update `docs/users-guide.md` in the "Harness adapter and attribute policy",
   "Using the Tokio harness", and "Using the GPUI harness" sections so the
   preference order leads with:
   - omit both for the default synchronous path
   - use `harness = ...` alone for first-party Tokio and GPUI
   - add `attributes = ...` only when intentionally overriding the default
2. Keep one explicit override example for Tokio and one for GPUI.
3. Keep the third-party caveat explicit: unknown third-party harnesses do not
   imply attribute defaults today, so custom integrations still need explicit
   `attributes = ...` when emitted framework attributes matter.
4. Update `docs/rstest-bdd-design.md` section 2.7.3 so it no longer says the
   user-facing guidance should lead with paired `harness = ...` and
   `attributes = ...` configuration.
5. Document the exact precedence order from ADR-008 in both docs so runtime
   alias behaviour stays unambiguous.

Go/no-go validation:

- The revised examples in `docs/users-guide.md` use harness-only first-party
  configuration by default.
- The docs still describe explicit overrides and third-party limitations
  clearly.

### Stage E: final validation and close-out

Goal: prove the repository is in a releasable state after the change.

Run these commands and inspect the logs before closing the work:

```bash
set -o pipefail; cargo test -p rstest-bdd-policy 2>&1 | tee /tmp/adr-008-policy.log
set -o pipefail; cargo test -p rstest-bdd-macros --lib 2>&1 | tee /tmp/adr-008-macros.log
set -o pipefail; RUSTFLAGS="-D warnings" cargo test -p rstest-bdd \
  --test trybuild_macros step_macros_compile -- --exact 2>&1 | \
  tee /tmp/adr-008-trybuild.log
set -o pipefail; RUSTFLAGS="-D warnings" cargo test -p rstest-bdd \
  --test scenario_harness_tokio 2>&1 | tee /tmp/adr-008-tokio.log
set -o pipefail; RUSTFLAGS="-D warnings" cargo test -p rstest-bdd \
  --test scenario_harness_gpui --features gpui-harness-tests 2>&1 | \
  tee /tmp/adr-008-gpui.log
set -o pipefail; make fmt 2>&1 | tee /tmp/adr-008-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/adr-008-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/adr-008-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/adr-008-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/adr-008-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/adr-008-make-test.log
```

If `make fmt`, `make check-fmt`, `make lint`, or `make test` fails because
`mdformat-all`, `uvx`, or `uv` is missing in the environment, stop and restore
the documented wrapper setup before rerunning the gates. Do not silently skip
those targets.

Close-out checklist for the implementing agent:

1. Update the `Progress` section with actual completion timestamps.
2. Replace `Outcomes & Retrospective` with the delivered results and any
   follow-on work.
3. If ADR-008 is accepted during execution, update the relevant roadmap items
   in `docs/roadmap.md`. If it is still only proposed, leave the roadmap as a
   contingent plan and record that decision in `Decision Log`.
