# ExecPlan 9.6.2: add GPUI attribute-policy resolution integration tests

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, and `Outcomes & retrospective` must be kept up to date as work
proceeds.

Status: DONE

`PLANS.md` is not present in this repository at the time of writing, so this
ExecPlan is the governing plan for roadmap item 9.6.2.

## Purpose / big picture

Roadmap item 9.6.2 closes the remaining GPUI validation gap in phase 9.6. Phase
9.4 already delivered `GpuiHarness`, `GpuiAttributePolicy`, macro support for
the canonical GPUI policy path, and the `examples/gpui-counter` demonstration
crate. What is still missing is a clearly defined integration test layer that
proves GPUI attribute-policy resolution works end to end at the macro boundary,
not only in unit tests.

After this work:

- `rstest-bdd` has explicit integration coverage for GPUI attribute-policy
  resolution, not just unit coverage in `rstest-bdd-policy` and
  `rstest-bdd-macros`.
- The GPUI coverage matrix includes the real macro entry points that users
  touch: `#[scenario]`, and, if the current gap analysis confirms it is
  untested, `scenarios!`.
- The integration surface proves that the first-party GPUI policy path
  resolves to `#[gpui::test]` in the cases the project documents as supported,
  and that attribute emission does not regress into duplicate or missing GPUI
  test attributes.
- `docs/rstest-bdd-design.md` records the strengthened validation layer for the
  path-based GPUI policy trust model.
- `docs/users-guide.md` records the supported GPUI policy usage in the guide's
  harness section.
- `docs/roadmap.md` marks 9.6.2 done only after the focused tests and the full
  required gates pass.

Success is observable when the new GPUI policy-resolution tests fail before the
implementation, pass afterward, and the repository gates pass:
`make check-fmt`, `make lint`, and `make test`. Because `make test` uses
`cargo-nextest` when available and therefore skips the `trybuild` compile-test
suite, this milestone also requires an explicit
`cargo test -p rstest-bdd --test trybuild_macros` run as part of focused
validation.

## Constraints

- Implement roadmap item 9.6.2 only. Do not fold 9.6.3 cookbook work into
  this change.
- Treat 9.4.3 and 9.4.4 as delivered foundations. This task strengthens
  validation and documentation around GPUI policy resolution; it is not a
  redesign of ADR-005 or ADR-007.
- Preserve the current trust model documented in `docs/users-guide.md` and
  `docs/rstest-bdd-design.md`: GPUI attribute-policy resolution during macro
  expansion is path-based for first-party policy types.
- Keep GPUI integration in opt-in crates. Core crates must not gain new
  always-on GPUI runtime responsibilities.
- Prefer extending existing `rstest-bdd` integration and trybuild suites over
  inventing a separate bespoke test harness.
- Validate the feature with both unit-level or compile-time tests and
  behavioural or runtime tests.
- Record any design decisions taken in `docs/rstest-bdd-design.md`.
- Record user-facing usage in `docs/users-guide.md`.
- Mark roadmap entry 9.6.2 done only after all validation passes.
- Because this plan touches Markdown, run the documentation gates too:
  `make fmt`, `make markdownlint`, and `make nixie`.
- Run long-lived commands with `set -o pipefail` and `tee`.

## Tolerances (exception triggers)

- Scope: if delivering 9.6.2 requires more than 12 files changed or more than
  600 net lines, stop and re-check whether the work is drifting into a broader
  policy-resolution redesign.
- Interfaces: if the test gap can only be closed by changing public APIs in
  `rstest-bdd-harness-gpui`, `rstest-bdd-harness`, `rstest-bdd-policy`,
  `rstest-bdd-macros`, `#[scenario]`, or `scenarios!`, stop and split the API
  change into a separate task unless the failure is a narrow correctness bug.
- Dependencies: if new external crates are required, stop and escalate.
- Test topology: if GPUI integration coverage cannot run inside the existing
  feature-gated `rstest-bdd` test setup, stop and document the exact blocker
  before adding a second GPUI-specific integration harness.
- Validation: if `make check-fmt`, `make lint`, or `make test` fails for an
  unrelated reason, capture logs and stop before updating the roadmap.
- Iterations: if the same gate fails three consecutive fix attempts, stop and
  escalate with the recorded log path.
- Ambiguity: if repository sources disagree on which GPUI path forms are
  supported end to end, stop and list the competing interpretations before
  continuing.

## Risks

- Risk: current coverage may already prove the canonical GPUI policy path for
  one happy path, but still miss important user-facing entry points such as
  `scenarios!` or absolute-path spelling. Severity: high. Likelihood: high.
  Mitigation: begin with a gap inventory and make the test matrix explicit
  before writing code.

- Risk: the project may over-correct by adding only more unit tests in
  `rstest-bdd-macros`, leaving the roadmap item's "integration tests" intent
  unmet. Severity: high. Likelihood: medium. Mitigation: require at least one
  real `rstest-bdd` integration test that executes generated GPUI-backed tests
  through the public macro surface.

- Risk: `make test` can pass while `trybuild` GPUI fixtures still fail,
  because `nextest` skips `tests/trybuild_macros.rs`. Severity: high.
  Likelihood: high. Mitigation: make the focused
  `cargo test -p rstest-bdd --test trybuild_macros step_macros_compile -- --exact`
   run a mandatory validation step in this plan.

- Risk: GPUI-specific tests are feature-gated and could be edited without
  actually running them. Severity: medium. Likelihood: medium. Mitigation:
  include an explicit feature-enabled focused integration command in the plan.

- Risk: documentation could drift again if the users' guide and design doc are
  not updated alongside the new test matrix. Severity: medium. Likelihood:
  medium. Mitigation: update both docs in the same milestone that lands the
  tests and reference the exact suites that now enforce the contract.

## Progress

- [x] (2026-03-26) Reviewed roadmap item 9.6.2 and prerequisite 9.4.3.
- [x] (2026-03-26) Reviewed ADR-005, the harness chapter added in 9.6.1, and
      current GPUI policy-resolution code paths.
- [x] (2026-03-26) Reviewed current GPUI unit, trybuild, and integration
      coverage to identify the remaining validation gap.
- [x] (2026-03-26) Drafted this ExecPlan.
- [x] (2026-03-28) Stage A: confirmed the missing GPUI policy-resolution
      matrix. The missing integration cases were GPUI compile-pass fixtures for
      `#[scenario]` canonical and absolute policy paths, GPUI compile-pass
      coverage for `scenarios!`, and runtime `scenarios!` coverage.
- [x] (2026-03-28) Stage B: added GPUI compile-time integration fixtures to
      the `trybuild` suite.
- [x] (2026-03-28) Stage C: extended GPUI runtime integration coverage in
      `rstest-bdd` so generated GPUI tests execute through public macros,
      including `scenarios!` and `#[scenario]` deduplication with an explicit
      `#[gpui::test]`.
- [x] (2026-03-28) Stage D: updated design and user documentation for the
      delivered validation surface.
- [x] (2026-03-28) Stage E: ran focused GPUI validation plus repository-wide
      gates.
- [x] (2026-03-28) Stage F: marked roadmap item 9.6.2 done and recorded
      outcomes.

## Surprises & discoveries

- Observation: `crates/rstest-bdd-macros` already has unit tests for GPUI
  policy-path handling, including canonical and absolute paths, in
  `crates/rstest-bdd-macros/src/codegen/scenario/tests/gpui_policy.rs`. Impact:
  9.6.2 should not duplicate that unit coverage; it should extend the
  integration layer beyond it.

- Observation: `crates/rstest-bdd/tests/scenario_harness_gpui.rs` already
  proves one canonical GPUI policy path end to end, including an
  `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy` case without a
  harness. Impact: the missing work is the untested resolution matrix around
  that happy path, not the entire GPUI integration from scratch.

- Observation: the `trybuild` passing fixtures currently include Tokio
  attribute-policy cases but no GPUI counterparts in
  `crates/rstest-bdd/tests/fixtures_macros/`. Impact: 9.6.2 should almost
  certainly add GPUI fixtures there.

- Observation: `scenario_harness_gpui.rs` can host both explicit `#[scenario]`
  tests and generated `scenarios!` GPUI tests behind the existing
  `gpui-harness-tests` feature gate. Impact: Stage C could stay within the
  existing integration binary instead of adding a second GPUI test target.

- Observation: `make test` is insufficient on its own for this milestone
  because `tests/trybuild_macros.rs` returns early when `NEXTEST_RUN_ID` is
  set. Impact: focused `cargo test` commands must remain part of the mandatory
  validation recipe.

- Observation: the prompt references `rust-doctest-dry-guide.md` without the
  `docs/` prefix, but the repository file is `docs/rust-doctest-dry-guide.md`.
  Impact: this plan uses the real path under `docs/` as the source of truth.

## Decision log

- Decision: treat 9.6.2 as a validation milestone spanning both compile-time
  macro integration and runtime integration. Rationale: the roadmap item says
  "integration tests", and the existing gap is specifically between unit-level
  policy-path checks and end-to-end macro behaviour. Date/Author: 2026-03-26 /
  Codex.

- Decision: extend the existing `rstest-bdd` trybuild and integration test
  suites rather than creating a new standalone GPUI-only test crate. Rationale:
  the repository already uses `crates/rstest-bdd/tests/trybuild_macros.rs` for
  macro integration and `crates/rstest-bdd/tests/scenario_harness_gpui.rs` for
  GPUI end-to-end coverage, so the smallest coherent change is to deepen those
  suites. Date/Author: 2026-03-26 / Codex.

- Decision: require a documented test matrix before writing fixes. Rationale:
  current sources suggest the remaining gap is not "GPUI policy resolution is
  entirely untested", but "some supported GPUI resolution paths are not tested
  at the integration layer". A matrix prevents both over-building and missing a
  real hole. Date/Author: 2026-03-26 / Codex.

- Decision: keep the docs update small and validation-focused. Rationale:
  9.6.1 already documented the trust model; 9.6.2 should add only the new
  supported usage notes and validation references needed to keep the docs
  current with the stronger GPUI test surface. Date/Author: 2026-03-26 / Codex.

- Decision: cover GPUI `#[scenario]` deduplication at runtime instead of
  adding another compile-pass fixture that would duplicate the existing
  unit-level dedup assertion. Rationale: the integration crate already depends
  on `gpui`, so an explicit `#[gpui::test]` on a
  `#[scenario(..., attributes = ...)]` function proves the public macro surface
  does not emit a conflicting second GPUI test attribute. Date/Author:
  2026-03-28 / Codex.

## Outcomes & retrospective

- Added GPUI compile-pass fixtures in
  `crates/rstest-bdd/tests/fixtures_macros/` for:
  - canonical `rstest_bdd_harness_gpui::GpuiAttributePolicy` on `#[scenario]`
  - absolute `::rstest_bdd_harness_gpui::GpuiAttributePolicy` on `#[scenario]`
  - canonical `rstest_bdd_harness_gpui::GpuiAttributePolicy` on `scenarios!`
- Extended `crates/rstest-bdd/tests/scenario_harness_gpui.rs` with:
  - a generated `scenarios!` GPUI policy runtime case
  - an explicit `#[gpui::test]` plus `#[scenario(..., attributes = ...)]`
    deduplication case
- Updated `docs/users-guide.md` and `docs/rstest-bdd-design.md` to record the
  supported GPUI path forms and the strengthened validation layer.
- Updated `docs/roadmap.md` only after the following commands passed:

```bash
set -o pipefail; cargo test -p rstest-bdd-policy 2>&1 | tee /tmp/9-6-2-policy.log
set -o pipefail; cargo test -p rstest-bdd-macros --lib gpui_policy 2>&1 | \
  tee /tmp/9-6-2-macros-gpui.log
set -o pipefail; RUSTFLAGS="-D warnings" cargo test -p rstest-bdd \
  --test trybuild_macros step_macros_compile -- --exact 2>&1 | \
  tee /tmp/9-6-2-trybuild.log
set -o pipefail; RUSTFLAGS="-D warnings" cargo test -p rstest-bdd \
  --test scenario_harness_gpui --features gpui-harness-tests 2>&1 | \
  tee /tmp/9-6-2-gpui-integration.log
set -o pipefail; make fmt 2>&1 | tee /tmp/9-6-2-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/9-6-2-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/9-6-2-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/9-6-2-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/9-6-2-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/9-6-2-make-test.log
```

- No macro or harness production code changes were required; the milestone was
  closed by filling the missing integration coverage and updating the docs.

## Context and orientation

The repository already contains the pieces that this milestone must connect:

- `crates/rstest-bdd-policy/src/lib.rs`
  - canonical policy-path mapping, including `GPUI_ATTRIBUTE_POLICY_PATH`
  - unit coverage for `resolve_test_attribute_hint_for_policy_path`
- `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs`
  - the macro-side policy resolver and attribute renderer
- `crates/rstest-bdd-macros/src/codegen/scenario/tests/gpui_policy.rs`
  - unit tests for canonical-path, absolute-path, and dedup logic
- `crates/rstest-bdd/tests/scenario_harness_gpui.rs`
  - current GPUI runtime integration tests
- `crates/rstest-bdd/tests/features/gpui_harness.feature`
  - current GPUI integration feature file
- `crates/rstest-bdd/tests/fixtures_macros/`
  - compile-pass and compile-fail macro fixtures used by `trybuild`
- `crates/rstest-bdd/tests/trybuild_macros.rs`
  - the integration entry point for compile-time macro tests
- `docs/users-guide.md`
  - user-facing harness and GPUI usage guidance
- `docs/rstest-bdd-design.md`
  - architecture and validation-layer narrative for the path-based trust model
- `docs/roadmap.md`
  - roadmap item 9.6.2, currently unchecked

Terms used in this plan:

- **Canonical GPUI policy path**: the documented first-party path
  `rstest_bdd_harness_gpui::GpuiAttributePolicy`.
- **Absolute GPUI policy path**: the same path written with a leading `::`.
- **Policy resolution**: the proc-macro step that converts an `attributes = ...`
  path into emitted test attributes such as `#[gpui::test]`.
- **Integration test**: a test that exercises public macro entry points in a
  compiled crate or through the `rstest-bdd` integration test harness, rather
  than calling internal helpers directly.

Reference documents reviewed while drafting this plan:

- `docs/roadmap.md`
- `docs/rstest-bdd-design.md`
- `docs/rstest-bdd-language-server-design.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/gherkin-syntax.md`
- `docs/adr-005-harness-adapter-crates-for-framework-specific-test-integration.md`
- `docs/execplans/9-4-2-create-rstest-bdd-harness-gpui.md`
- `docs/execplans/9-6-1-update-the-harness-adapter-chapter-in-the-users-guide.md`

## Plan of work

### Stage A: confirm the missing GPUI policy-resolution matrix

Goal: convert the vague roadmap item into an explicit, repo-local acceptance
matrix before editing tests.

Implementation details:

- Re-read the current GPUI unit coverage, `trybuild` pass fixtures, and
  feature-gated runtime integration tests.
- Write down which cases are already covered and which are still missing.
- The matrix must explicitly account for:
  - canonical GPUI policy path under `#[scenario]`,
  - absolute GPUI policy path under `#[scenario]` if supported,
  - user-supplied `#[gpui::test]` deduplication if the runtime surface can
    prove it,
  - `scenarios!` GPUI policy usage if it is not already covered elsewhere.
- Capture at least one red test before fixing anything so the milestone
  follows a red-green-refactor shape.

Go/no-go validation:

- There is a written list of GPUI policy-resolution cases to cover.
- At least one missing case is demonstrated by an absent or failing test.

### Stage B: add GPUI compile-time integration fixtures

Goal: give GPUI the same compile-time integration posture that Tokio already
has in `tests/fixtures_macros/`.

Implementation details:

- Add new compile-pass fixtures under `crates/rstest-bdd/tests/fixtures_macros/`
  for the GPUI policy-resolution cases selected in Stage A.
- Register those fixtures in
  `crates/rstest-bdd/tests/trybuild_macros.rs::run_passing_macro_tests`.
- Prefer small, single-purpose fixtures mirroring the Tokio naming style, for
  example:
  - `scenario_attributes_gpui.rs`
  - `scenario_attributes_gpui_absolute.rs`
  - `scenario_attributes_gpui_dedup.rs`
  - `scenarios_attributes_gpui.rs`
- Reuse existing feature fixtures where practical; add new `.feature` files
  only when a case cannot be expressed cleanly with the current ones.

Go/no-go validation:

- The new GPUI `trybuild` fixtures fail before the implementation and pass
  after it.
- The fixture names and comments make the supported GPUI path forms obvious to
  a future maintainer.

### Stage C: add or extend GPUI runtime integration coverage

Goal: prove that generated GPUI-backed tests actually execute through the
public macro surface with the resolved policy attributes in place.

Implementation details:

- Extend `crates/rstest-bdd/tests/scenario_harness_gpui.rs` when a new case can
  fit cleanly there; otherwise add one sibling integration file with the same
  `gpui-harness-tests` feature gate.
- Add the smallest runtime assertions needed to prove that the generated test
  was actually executed under GPUI policy resolution.
- If Stage A confirms a `scenarios!` gap, add a dedicated runtime integration
  test for `scenarios!` rather than assuming the shared helper logic makes it
  redundant.
- Keep GPUI-specific state isolated and parallel-safe, following the existing
  integration-test style instead of introducing process-global coupling beyond
  what the current GPUI tests already use.

Go/no-go validation:

- The runtime integration suite exercises every GPUI resolution case promised
  by the updated docs.
- GPUI policy tests still pass under the existing `gpui-harness-tests` feature
  gate.

### Stage D: update design and user documentation

Goal: keep the docs aligned with the stronger GPUI validation surface without
rewriting the harness chapter.

Implementation details:

- Update `docs/rstest-bdd-design.md` where it describes the validation layers
  for first-party policy resolution so it names the delivered GPUI integration
  coverage precisely.
- Update `docs/users-guide.md` in the GPUI harness section so the guide records
  the supported GPUI `attributes = ...` usage that the new tests enforce.
- Keep the wording consistent with the existing trust model: first-party GPUI
  policy resolution is path-based at macro expansion time.

Go/no-go validation:

- A reader can tell which GPUI policy paths and macro entry points are
  intentionally supported.
- The docs do not imply arbitrary third-party policy evaluation during macro
  expansion.

### Stage E: run focused validation and repository gates

Goal: validate the new GPUI coverage directly, then confirm the whole
repository remains green.

Implementation details:

- Run the focused suites first:

```bash
set -o pipefail; cargo test -p rstest-bdd-policy 2>&1 | tee /tmp/9-6-2-policy.log
set -o pipefail; cargo test -p rstest-bdd-macros --lib gpui_policy 2>&1 | tee /tmp/9-6-2-macros-gpui.log
set -o pipefail; RUSTFLAGS="-D warnings" cargo test -p rstest-bdd \
  --test trybuild_macros step_macros_compile -- --exact 2>&1 | \
  tee /tmp/9-6-2-trybuild.log
set -o pipefail; RUSTFLAGS="-D warnings" cargo test -p rstest-bdd \
  --test scenario_harness_gpui --features gpui-harness-tests 2>&1 | \
  tee /tmp/9-6-2-gpui-integration.log
```

- If Stage C adds a second GPUI integration test file, run it explicitly here
  as well.
- Then run the repository gates:

```bash
set -o pipefail; make fmt 2>&1 | tee /tmp/9-6-2-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/9-6-2-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/9-6-2-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/9-6-2-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/9-6-2-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/9-6-2-make-test.log
```

Expected signals:

- the focused `trybuild` run reports the GPUI fixtures as passing
- the GPUI feature-gated runtime integration test passes
- `make check-fmt`, `make lint`, and `make test` all exit successfully
- the documentation-only gates also pass because this milestone edits Markdown

### Stage F: close the roadmap item

Goal: update project status only after the implementation and validation are
complete.

Implementation details:

- Mark `docs/roadmap.md` item 9.6.2 as done.
- Update this ExecPlan's `Progress`, `Decision Log`, and
  `Outcomes & Retrospective` sections with the delivered facts.
- Record any important new gotcha in project memory if the work uncovers one
  that future contributors are likely to hit again.

Go/no-go validation:

- The roadmap entry is checked only after all commands in Stage E succeed.
- The ExecPlan remains self-contained for the next contributor who reads it.

## Acceptance criteria

This milestone is complete only when all of the following are true:

1. `rstest-bdd` has explicit GPUI attribute-policy resolution integration
   coverage beyond the existing macro unit tests.
2. The new tests cover the GPUI cases identified as missing in Stage A.
3. `docs/rstest-bdd-design.md` records the delivered GPUI validation surface.
4. `docs/users-guide.md` records the supported GPUI attribute-policy usage.
5. `docs/roadmap.md` marks 9.6.2 done.
6. The focused GPUI validation commands pass.
7. `make check-fmt`, `make lint`, and `make test` pass.
