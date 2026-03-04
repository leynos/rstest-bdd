# ExecPlan 9.4.2-9.4.4: Create `rstest-bdd-harness-gpui`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT (2026-03-04)

`PLANS.md` is not present in this repository at the time of writing, so this
ExecPlan is the governing plan for roadmap items 9.4.2, 9.4.3, and 9.4.4.

## Purpose / big picture

Roadmap phase 9.4 requires the first Graphical Processing User Interface (GPUI)
harness plugin crate, built on top of ADR-005 and ADR-007. The core objective
is to keep GPUI integration out of core crates while allowing scenarios to run
inside the GPUI test harness and inject GPUI-owned fixtures (for example
`TestAppContext`) into step execution.

After this change:

- A new workspace crate `crates/rstest-bdd-harness-gpui` exists and exports a
  GPUI harness adapter plus a GPUI attribute policy type.
- Scenario execution can be delegated through that adapter, with harness
  context passed via `HarnessAdapter::Context` and consumed by steps using the
  existing reserved fixture key `rstest_bdd_harness_context`.
- Macro attribute-policy resolution recognizes the canonical GPUI policy path,
  so generated tests receive the GPUI test attribute in addition to
  `#[rstest::rstest]`.
- Unit, behavioural, and integration tests validate this end-to-end path.
- Design and user documentation reflect the delivered behaviour.
- Roadmap items 9.4.2, 9.4.3, and 9.4.4 are marked done only after all gates
  pass.

Success is observable when GPUI-backed scenarios pass through the adapter with
fixture injection, and the required gates all pass:
`make check-fmt`, `make lint`, and `make test`.

## Constraints

- Implement only roadmap items 9.4.2, 9.4.3, and 9.4.4 in this change.
- Preserve ADR-005 boundaries: GPUI dependencies must stay in the new
  `rstest-bdd-harness-gpui` crate and not leak into core runtime crates.
- Preserve ADR-007 contract: harness context must flow through
  `ScenarioRunRequest<'_, C, T>` and be passed via `request.run(context)`.
- Keep existing `StdHarness` and `TokioHarness` behaviour unchanged.
- Avoid changing user-facing macro argument syntax for `#[scenario]` and
  `scenarios!`.
- Every new Rust module must begin with a `//!` module-level comment.
- Public APIs in the new crate must include Rustdoc with examples.
- No file may exceed 400 lines.
- Record architectural and behaviour decisions in `docs/rstest-bdd-design.md`.
- Record user-facing usage in `docs/users-guide.md`.
- Mark roadmap entries as done only after all validation passes.
- Required gates before completion:
  `make check-fmt`, `make lint`, and `make test`.
- Run validation commands with `set -o pipefail` and `tee`.

## Tolerances (exception triggers)

- Scope: if implementation exceeds 24 files changed or 1,200 net lines, stop
  and escalate.
- Interfaces: if `HarnessAdapter`, `ScenarioRunRequest`, or macro argument
  syntax require breaking changes, stop and escalate.
- Dependencies: if adding GPUI requires non-GPUI new dependencies in core
  crates, stop and escalate.
- Platform: if GPUI test harness APIs are unavailable on CI-supported targets
  without a feasible feature-gated fallback, stop and escalate.
- Iterations: if the same gate (`check-fmt`, `lint`, or `test`) fails three
  consecutive fix attempts, stop and escalate with logs.
- Ambiguity: if canonical GPUI test attribute naming differs across upstream
  docs and crate API, stop and record options before choosing.

## Risks

- Risk: GPUI has heavier platform/runtime requirements than Tokio and may
  require target-specific setup.
  Severity: high.
  Likelihood: medium.
  Mitigation: isolate GPUI dependency to the plugin crate and use the smallest
  viable feature set; gate behaviour tests if the upstream harness requires
  platform support unavailable in CI.

- Risk: macro attribute-policy resolution is path-based today and currently only
  recognizes default and Tokio policies.
  Severity: high.
  Likelihood: high.
  Mitigation: extend `rstest-bdd-policy` canonical path mapping and add focused
  unit tests in `rstest-bdd-macros` covering GPUI policy resolution.

- Risk: harness-context injection may compile but not remain mutable/visible
  across multiple steps.
  Severity: medium.
  Likelihood: medium.
  Mitigation: add behavioural scenario coverage asserting both immutable and
  mutable access to `TestAppContext`-like context values.

- Risk: release metadata may drift after adding a new publishable crate.
  Severity: medium.
  Likelihood: medium.
  Mitigation: update release sequencing docs/scripts if they currently enumerate
  publishable crates explicitly.

## Progress

- [x] (2026-03-04 00:00Z) Reviewed roadmap item 9.4 scope and prerequisites.
- [x] (2026-03-04 00:00Z) Reviewed ADR-005 and ADR-007 for boundary and context
      constraints.
- [x] (2026-03-04 00:00Z) Drafted this ExecPlan.
- [ ] Stage A: baseline and GPUI API reconnaissance.
- [ ] Stage B: scaffold `rstest-bdd-harness-gpui` crate.
- [ ] Stage C: implement GPUI harness adapter and context injection path.
- [ ] Stage D: implement GPUI attribute policy and macro policy resolution.
- [ ] Stage E: add unit, behavioural, and integration tests.
- [ ] Stage F: update design docs, users guide, release metadata, and roadmap.
- [ ] Stage G: run required quality gates and capture evidence.

## Surprises & Discoveries

- Observation: `docs/rust-doctest-dry-guide.md` is located under `docs/` in
  this repository, while the task prompt references `rust-doctest-dry-guide.md`
  at the root.
  Evidence: file lookup in workspace.
  Impact: this plan uses `docs/rust-doctest-dry-guide.md` as the authoritative
  reference path.

- Observation: attribute-policy codegen currently recognizes only
  `DefaultAttributePolicy` and `TokioAttributePolicy` via canonical path
  resolution in `rstest-bdd-policy`.
  Evidence: `crates/rstest-bdd-policy/src/lib.rs` and
  `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs`.
  Impact: 9.4.4 requires extending this mapping for GPUI policy support.

## Decision Log

- Decision: deliver 9.4.2, 9.4.3, and 9.4.4 in one atomic implementation
  milestone.
  Rationale: the crate scaffold, harness adapter behaviour, and policy mapping
  are tightly coupled; splitting them would create temporary half-working
  states.
  Date/Author: 2026-03-04 / Codex.

- Decision: model GPUI behavioural tests after the existing Tokio harness test
  topology (unit + behavioural + rstest-bdd integration), while adapting
  assertions to GPUI-specific context and attribute semantics.
  Rationale: this preserves consistency across first-party harness crates and
  lowers maintenance overhead.
  Date/Author: 2026-03-04 / Codex.

- Decision: treat canonical GPUI test attribute path as an explicit discovery
  checkpoint before final codegen mapping.
  Rationale: upstream GPUI test macro naming can change; hard-coding without
  verification risks brittle policy resolution.
  Date/Author: 2026-03-04 / Codex.

## Outcomes & Retrospective

Pending implementation.

Expected outcomes:

- New crate `rstest-bdd-harness-gpui` with a documented harness adapter and
  attribute policy.
- End-to-end scenario execution through the GPUI harness with harness-context
  fixture injection.
- Macro policy resolution supports canonical GPUI policy paths.
- Docs and roadmap are aligned with delivered behaviour.

Retrospective notes will be completed at implementation finish.

## Context and orientation

Primary files and modules expected to change:

- `Cargo.toml` (workspace member and workspace dependency entries).
- `crates/rstest-bdd-harness-gpui/Cargo.toml` (new crate manifest).
- `crates/rstest-bdd-harness-gpui/src/lib.rs` (public exports).
- `crates/rstest-bdd-harness-gpui/src/gpui_harness.rs` (adapter).
- `crates/rstest-bdd-harness-gpui/src/policy.rs` (attribute policy).
- `crates/rstest-bdd-harness-gpui/tests/harness_behaviour.rs` (behavioural
  adapter tests).
- `crates/rstest-bdd-harness-gpui/tests/attribute_policy_behaviour.rs`
  (behavioural policy tests).
- `crates/rstest-bdd-policy/src/lib.rs` (canonical policy path mapping).
- `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs`
  (policy-to-attribute generation).
- `crates/rstest-bdd-macros/src/codegen/scenario/tests.rs`
  (codegen regression tests for GPUI policy handling).
- `crates/rstest-bdd/Cargo.toml` (dev-dependency wiring for integration tests).
- `crates/rstest-bdd/tests/scenario_harness_gpui.rs` (new integration test).
- `crates/rstest-bdd/tests/features/gpui_harness.feature` (feature fixture).
- `docs/rstest-bdd-design.md` (architecture and implementation status).
- `docs/users-guide.md` (usage and configuration guidance).
- `docs/roadmap.md` (mark 9.4.2-9.4.4 done after gates pass).
- `docs/releasing-crates.md` and `scripts/publish_workspace_members.py`
  (publish sequence, if needed for the new crate).

Reference documents reviewed for this plan:

- `docs/roadmap.md`
- `docs/rstest-bdd-design.md`
- `docs/rstest-bdd-language-server-design.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/gherkin-syntax.md`
- `docs/adr-005-harness-adapter-crates-for-framework-specific-test-integration.md`
- `docs/adr-007-harness-context-injection.md`

## Plan of work

### Stage A: baseline and GPUI API reconnaissance

Goal: confirm upstream GPUI harness API details and baseline test health before
adding code.

Implementation details:

- Record baseline quality-gate state and current harness test state.
- Confirm canonical GPUI test harness and test attribute symbols from the GPUI
  crate version to be used.
- Capture any required crate features needed for test harness APIs.

Go/no-go validation:

- Canonical adapter and policy symbol names are verified.
- Baseline gates are known and logs captured.

### Stage B: scaffold `rstest-bdd-harness-gpui`

Goal: add a new workspace member with minimal, documented crate structure.

Implementation details:

- Add crate directory with `Cargo.toml`, `README.md`, and `src/lib.rs`.
- Wire the crate into workspace members and workspace dependency table.
- Export placeholder `GpuiHarness` and `GpuiAttributePolicy` types.

Go/no-go validation:

- `cargo test -p rstest-bdd-harness-gpui --no-run` succeeds.
- Crate-level docs and module docs compile cleanly.

### Stage C: implement GPUI harness adapter (9.4.3)

Goal: run scenario requests inside the GPUI test harness and pass GPUI context
through `HarnessAdapter::Context`.

Implementation details:

- Implement `HarnessAdapter for GpuiHarness` with a context type representing
  GPUI test context (for example `TestAppContext` or the verified equivalent).
- Execute `ScenarioRunRequest` inside GPUI harness lifecycle hooks.
- Pass context via `request.run(context)` and ensure context is available to
  step extraction through existing reserved fixture key flow.
- Add unit tests in crate modules and behavioural tests in
  `tests/harness_behaviour.rs` covering metadata visibility, single execution,
  and context handoff.

Go/no-go validation:

- Harness tests prove scenario closure execution inside GPUI harness context.
- Context can be read and mutated across step boundaries in integration tests.

### Stage D: implement GPUI attribute policy (9.4.4)

Goal: provide a GPUI test attribute policy plugin and ensure macro codegen can
emit it for canonical GPUI policy paths.

Implementation details:

- Implement `GpuiAttributePolicy: AttributePolicy` returning ordered
  attributes (`#[rstest::rstest]` plus verified GPUI test attribute).
- Extend `rstest-bdd-policy` with canonical GPUI policy path constant and hint.
- Extend `test_attrs.rs` policy resolution and rendering logic to support the
  new hint while preserving current Tokio/default behaviour.
- Add/extend macro codegen unit tests to validate:
  GPUI policy path resolution, sync-vs-async behaviour, and deduplication where
  relevant.

Go/no-go validation:

- Generated tokens include GPUI test attribute when GPUI policy is selected.
- Existing Tokio and default policy tests remain unchanged and green.

### Stage E: integration and behavioural validation

Goal: validate end-to-end scenario execution and fixture injection.

Implementation details:

- Add `rstest-bdd` integration tests similar to `scenario_harness_tokio.rs`
  using `harness = rstest_bdd_harness_gpui::GpuiHarness` and
  `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy`.
- Add feature files under `crates/rstest-bdd/tests/features/` to drive GPUI
  harness scenarios.
- Ensure at least one scenario asserts harness-context fixture usage through
  `#[from(rstest_bdd_harness_context)]`.

Go/no-go validation:

- New GPUI integration tests fail before implementation and pass after.
- Existing harness integration tests (Std/Tokio) still pass.

### Stage F: documentation and roadmap closure

Goal: keep architecture and user-facing docs synchronized with delivered
behaviour.

Implementation details:

- Update `docs/rstest-bdd-design.md` section 2.7.4 to mark GPUI crate as
  implemented and document context/policy behaviour.
- Update `docs/users-guide.md` with GPUI setup, scenario annotation examples,
  and fixture-injection examples using harness context.
- Update release-order docs/scripts if publishable crate lists require GPUI
  entry.
- Mark `docs/roadmap.md` entries 9.4.2, 9.4.3, and 9.4.4 as done after passing
  required gates.

Go/no-go validation:

- Documentation examples match actual crate symbols and usage.
- Roadmap status reflects delivered implementation.

### Stage G: required quality gates

Goal: finish with a healthy workspace and auditable logs.

Implementation details:

- Run required gates with `set -o pipefail` and `tee`.
- Review log tails and verify zero non-ignored failures.

Go/no-go validation:

- `make check-fmt`, `make lint`, and `make test` all exit 0.

## Concrete steps

Run from repository root (`/home/user/project`).

Baseline checks:

```bash
set -o pipefail
cargo test -p rstest-bdd-harness --test harness_behaviour 2>&1 | tee /tmp/9-4-2-baseline-harness.log
cargo test -p rstest-bdd-harness-tokio --test harness_behaviour 2>&1 | tee /tmp/9-4-2-baseline-tokio.log
```

Expected signal:

```plaintext
... test result: ok. <N> passed; 0 failed ...
```

Iterate on GPUI crate and focused suites:

```bash
set -o pipefail
cargo test -p rstest-bdd-harness-gpui 2>&1 | tee /tmp/9-4-2-gpui-crate-tests.log
cargo test -p rstest-bdd-macros --lib codegen::scenario::tests 2>&1 | tee /tmp/9-4-2-macro-policy-tests.log
cargo test -p rstest-bdd --test scenario_harness_gpui 2>&1 | tee /tmp/9-4-2-gpui-integration.log
```

Expected signal:

```plaintext
... test result: ok. <N> passed; 0 failed ...
```

Run required final gates:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/9-4-2-check-fmt.log
make lint 2>&1 | tee /tmp/9-4-2-lint.log
make test 2>&1 | tee /tmp/9-4-2-test.log
```

Expected signal:

```plaintext
make check-fmt  # exits 0
make lint       # exits 0
make test       # exits 0
```

## Validation and acceptance

Acceptance criteria:

- `crates/rstest-bdd-harness-gpui` exists and is wired into the workspace.
- `GpuiHarness` implements `HarnessAdapter` and executes requests inside GPUI
  test harness semantics.
- GPUI fixture context (for example `TestAppContext`) can be injected into step
  execution via `HarnessAdapter::Context`.
- `GpuiAttributePolicy` implements `AttributePolicy` and emits the canonical
  GPUI test attribute alongside `#[rstest::rstest]`.
- Macro attribute-policy resolution recognizes canonical GPUI policy paths.
- Unit tests and behavioural tests cover the adapter and policy.
- Integration tests in `rstest-bdd` verify end-to-end scenario execution using
  GPUI harness and policy.
- `docs/rstest-bdd-design.md` and `docs/users-guide.md` document delivered
  behaviour.
- `docs/roadmap.md` marks 9.4.2, 9.4.3, and 9.4.4 as done.
- Required gates pass: `make check-fmt`, `make lint`, `make test`.

## Idempotence and recovery

- All commands above are re-runnable.
- If a stage fails, keep successful prior edits and rerun only the failing
  stage plus dependent tests.
- Preserve `/tmp/9-4-2-*.log` artifacts for troubleshooting and review.
- If GPUI API discovery contradicts assumptions, update this ExecPlan first,
  then continue implementation.

## Artifacts and notes

Expected log artifacts:

```plaintext
/tmp/9-4-2-baseline-harness.log
/tmp/9-4-2-baseline-tokio.log
/tmp/9-4-2-gpui-crate-tests.log
/tmp/9-4-2-macro-policy-tests.log
/tmp/9-4-2-gpui-integration.log
/tmp/9-4-2-check-fmt.log
/tmp/9-4-2-lint.log
/tmp/9-4-2-test.log
```

Primary implementation artifacts:

- New `rstest-bdd-harness-gpui` crate.
- Policy mapping updates in `rstest-bdd-policy` and macro codegen.
- GPUI integration tests and feature fixtures in `rstest-bdd`.
- Updated design docs, users guide, release metadata (if required), and
  roadmap statuses.

## Interfaces and dependencies

Target interfaces after implementation:

```rust
pub struct GpuiHarness;

impl rstest_bdd_harness::HarnessAdapter for GpuiHarness {
    type Context = /* GPUI context type (for example TestAppContext) */;

    fn run<T>(
        &self,
        request: rstest_bdd_harness::ScenarioRunRequest<'_, Self::Context, T>,
    ) -> T {
        /* run request inside GPUI harness and call request.run(context) */
    }
}
```

```rust
pub struct GpuiAttributePolicy;

impl rstest_bdd_harness::AttributePolicy for GpuiAttributePolicy {
    fn test_attributes() -> &'static [rstest_bdd_harness::TestAttribute] {
        /* #[rstest::rstest] + canonical GPUI test attribute */
    }
}
```

Dependency direction constraints:

- `rstest-bdd-harness-gpui` depends on `rstest-bdd-harness` and GPUI.
- Core crates (`rstest-bdd`, `rstest-bdd-macros`, `rstest-bdd-harness`) must
  not gain direct GPUI dependencies.
- `rstest-bdd-policy` may add canonical GPUI policy-path constants and hints,
  but must remain dependency-light.

## Revision note

Initial draft created on 2026-03-04 for roadmap items 9.4.2, 9.4.3, and 9.4.4.
