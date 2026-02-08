# ExecPlan 9.1.1: Add `rstest-bdd-harness` and default attribute policy

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

`PLANS.md` is not present in the repository at the time of writing, so this
ExecPlan is the governing plan for this task.

## Purpose / big picture

Phase 9.1 introduces the framework-agnostic harness foundation required by
ADR-005. After this work, the codebase will have a dedicated
`rstest-bdd-harness` crate that defines:

- a harness adapter trait for executing scenario runners,
- shared runner types used by adapter crates and macro integration,
- a default synchronous `StdHarness`, and
- an attribute policy plugin interface with a default policy that emits only
  `#[rstest::rstest]`.

Success is observable when unit tests and behavioural tests pass for the new
harness core, documentation explains how to use and extend the APIs, and the
roadmap entries `9.1.1`, `9.1.2`, and `9.1.3` are marked complete.

## Constraints

- Implement only Phase 9.1 scope from `docs/roadmap.md`; do not implement
  `9.2+` harness selection in `#[scenario]`/`scenarios!` in this change.
- Keep Tokio and GPUI dependencies out of core crates (`rstest-bdd`,
  `rstest-bdd-macros`, `rstest-bdd-harness`) to preserve ADR-005 goals.
- Preserve existing public behaviour for current users (including
  `runtime = "tokio-current-thread"` compatibility paths) unless a change is
  explicitly documented as preparatory and non-breaking.
- Every new Rust module must begin with a `//!` module-level comment, and all
  public APIs must have Rustdoc comments with usage examples.
- Keep files under 400 lines by splitting modules when needed.
- Record design decisions in `docs/rstest-bdd-design.md`.
- Record user-facing usage in `docs/users-guide.md`.
- On completion, mark `9.1.1`, `9.1.2`, and `9.1.3` as done in
  `docs/roadmap.md`.
- Quality gates must pass before completion:
  `make check-fmt`, `make lint`, and `make test`.

## Tolerances (exception triggers)

- Scope: if delivery requires changing more than 18 files or more than 900 net
  LOC, stop and escalate.
- Interfaces: if any existing public API in `rstest-bdd` or
  `rstest-bdd-macros` must be removed or made incompatible, stop and escalate.
- Dependencies: if a new external dependency is needed in core crates, stop and
  escalate.
- Behaviour: if existing async scenario behaviour regresses in tests, stop and
  escalate instead of weakening tests.
- Iterations: if the same failing gate (`check-fmt`, `lint`, or `test`) fails
  three times after attempted fixes, stop and escalate with logs.
- Ambiguity: if ADR-005 and current roadmap text conflict on interface shape,
  stop and request direction before coding further.

## Risks

- Risk: premature coupling between 9.1 interfaces and 9.2 macro integration can
  force rework. Severity: medium Likelihood: medium Mitigation: keep 9.1
  interfaces small, explicit, and tested in isolation.

- Risk: duplicated policy concepts between `rstest-bdd-policy` and
  `rstest-bdd-harness` can create unclear ownership. Severity: medium
  Likelihood: medium Mitigation: document boundary clearly in design docs and
  keep legacy policy enums only as compatibility for existing runtime paths.

- Risk: behavioural coverage may miss adapter edge cases (runner panics,
  fixture hand-off, error propagation). Severity: high Likelihood: medium
  Mitigation: add targeted behavioural tests in the new crate plus integration
  checks around generated attribute policy output.

- Risk: docs drift between ADR, design doc, roadmap, and user guide.
  Severity: medium Likelihood: medium Mitigation: update all four in the same
  change and cross-link sections.

## Progress

- [x] (2026-02-07 00:00Z) Collected roadmap and ADR-005 requirements and
      drafted this ExecPlan.
- [x] (2026-02-08 01:24Z) Ran baseline validation to confirm pre-change
      status (`make test` logged in `/tmp/9-1-1-baseline-test.log`).
- [x] (2026-02-08 01:39Z) Added `crates/rstest-bdd-harness` and wired
      workspace membership and dependency metadata.
- [x] (2026-02-08 01:53Z) Implemented harness adapter trait, shared runner
      types, and synchronous `StdHarness`.
- [x] (2026-02-08 01:58Z) Implemented attribute policy plug-in interface and
      default rstest-only policy.
- [x] (2026-02-08 02:04Z) Added unit tests and behavioural tests for harness
      execution and policy output semantics.
- [x] (2026-02-08 02:16Z) Updated design and user docs for the new interfaces.
- [x] (2026-02-08 02:17Z) Marked roadmap entries `9.1.1`, `9.1.2`, and
      `9.1.3` as done.
- [x] (2026-02-08 02:30Z) Completed final quality gates:
      `make check-fmt`, `make lint`, and `make test` all passed and were
      captured in `/tmp/9-1-1-*.log`.

## Surprises & Discoveries

- Observation: project-memory helper command `qdrant-find` is not available in
  this environment. Evidence: shell reported `qdrant-find: command not found`.
  Impact: planning relied on repository docs and ADR files directly.

- Observation: `cargo test -p rstest-bdd` failed when forcing
  `CARGO_TARGET_DIR=/tmp/9-1-1-target` because trybuild fixtures resolve test
  files relative to the default `target` layout. Evidence:
  `/tmp/9-1-1-runtime-tests.log` showed
  `feature file not found: /tmp/9-1-1-target/tests/trybuild/...`. Impact: used
  default target dir for authoritative gate runs.

- Observation: Clippy raised `must_use_candidate` for runner methods in
  `crates/rstest-bdd-harness/src/runner.rs`. Impact: added `#[must_use]`
  annotations to `run` helpers to keep lint strict.

## Decision Log

- Decision: keep this plan scoped strictly to roadmap phase 9.1 and avoid
  implementing phase 9.2 harness selection syntax in macros. Rationale: the
  roadmap separates core interfaces from macro integration; mixing both would
  raise delivery risk and hide regressions. Date/Author: 2026-02-07 / Codex

- Decision: treat unit tests and behavioural tests as separate layers.
  Rationale: unit tests validate type contracts and defaults, while behavioural
  tests validate runner execution semantics and policy emission outputs.
  Date/Author: 2026-02-07 / Codex

- Decision: model attribute output as `TestAttribute { path, arguments }` with
  a small renderer helper rather than using proc-macro token types. Rationale:
  keeps `rstest-bdd-harness` free from proc-macro dependencies while still
  allowing deterministic policy assertions in behavioural tests. Date/Author:
  2026-02-08 / Codex

- Decision: keep phase 9.1 integration additive by introducing the harness
  crate without rewiring existing macro argument handling yet. Rationale:
  satisfies roadmap scope and preserves current
  `runtime = "tokio-current-thread"` behaviour for existing users. Date/Author:
  2026-02-08 / Codex

## Outcomes & Retrospective

Shipped in this phase:

- New crate `crates/rstest-bdd-harness` with `HarnessAdapter`,
  `ScenarioMetadata`, `ScenarioRunner<T>`, `ScenarioRunRequest<T>`,
  `StdHarness`, `AttributePolicy`, `TestAttribute`, and
  `DefaultAttributePolicy`.
- Unit and behavioural tests validating runner execution semantics and
  rstest-only default attribute emission.
- Documentation updates in `docs/rstest-bdd-design.md`,
  `docs/users-guide.md`, `README.md`, and roadmap completion in
  `docs/roadmap.md`.

Deferred to phase 9.2+:

- Macro-level harness/policy selection wiring in `#[scenario]` and
  `scenarios!`.
- Tokio/GPUI adapter crates and framework-specific policy crates.

Risks encountered and outcomes:

- Trybuild path coupling was triggered when using a custom `CARGO_TARGET_DIR`;
  this was contained by keeping canonical gate runs on the default target dir.
- No regressions were observed in workspace-wide `make test` gate results.

Follow-up feeding phase 9.2:

- Add macro argument parsing and codegen bridge to select harness and attribute
  policies by path.
- Introduce first-party Tokio/GPUI adapter crates as optional workspace
  members.

## Context and orientation

Current runtime and macro policy logic is split across:

- `crates/rstest-bdd-policy/src/lib.rs` (runtime mode and attribute hint enums),
- `crates/rstest-bdd-macros/src/codegen/scenario.rs`
  (`generate_test_attrs`), and
- `crates/rstest-bdd-macros/src/macros/scenarios/macro_args.rs`
  (`runtime = "tokio-current-thread"` parsing).

`docs/adr-005-harness-adapter-crates-for-framework-specific-test-integration.md`
 defines the architectural direction: framework integrations move to opt-in
adapter crates, while a small core harness crate owns shared contracts.

Phase 9.1 is the foundational layer for that architecture. It should introduce
harness and policy contracts now, with macro argument integration to follow in
phase 9.2.

## Plan of work

Stage A: Baseline and contract freeze (no functional change)

Confirm baseline with current tests and identify exact insertion points for the
new crate and docs. Freeze interface names for this phase so tests and docs are
written against stable names.

Go/no-go validation for Stage A:

- Baseline `make test` succeeds before edits.
- Proposed public type and trait names are recorded in this plan and in
  `docs/rstest-bdd-design.md` before implementation proceeds.

Stage B: Introduce `rstest-bdd-harness` core crate

Create `crates/rstest-bdd-harness` and add it to workspace members and
workspace dependencies in `Cargo.toml`. Implement module layout that keeps file
sizes bounded, for example:

- `crates/rstest-bdd-harness/src/lib.rs`
- `crates/rstest-bdd-harness/src/adapter.rs`
- `crates/rstest-bdd-harness/src/runner.rs`
- `crates/rstest-bdd-harness/src/policy.rs`
- `crates/rstest-bdd-harness/src/std_harness.rs`

Define:

- the harness adapter trait,
- shared runner input/output types used by adapters,
- `StdHarness` default synchronous implementation.

Keep this crate framework-agnostic and dependency-light.

Go/no-go validation for Stage B:

- `cargo test -p rstest-bdd-harness` passes.
- Public API docs compile and doctests pass for the new crate.

Stage C: Attribute policy plugin interface and default policy

In `rstest-bdd-harness`, define a minimal attribute policy interface suitable
for macro-time attribute generation, and add a default policy that emits only
`#[rstest::rstest]`.

Add tests that assert policy output content and ordering. Add behavioural tests
that validate the default policy remains framework-agnostic and does not emit
Tokio or GPUI attributes.

Where existing code still relies on `RuntimeMode`/`TestAttributeHint`, keep it
intact for compatibility, but document that policy plugins are the new
extension point introduced by ADR-005.

Go/no-go validation for Stage C:

- Unit tests for policy types pass.
- Behavioural tests prove default policy output is exactly rstest-only.

Stage D: Documentation and roadmap completion

Update docs to reflect final interfaces and user workflows:

- `docs/rstest-bdd-design.md`: document concrete harness trait and policy
  interfaces in ADR-005 section(s), including rationale for default behaviour.
- `docs/users-guide.md`: add usage guidance for the harness core and attribute
  policy extension points, including minimal examples.
- `docs/roadmap.md`: mark `9.1.1`, `9.1.2`, and `9.1.3` as done once all
  validation passes.

Run formatting, linting, and test gates. Complete outcomes and decision log.

Go/no-go validation for Stage D:

- docs build/lint checks pass,
- required make targets succeed,
- roadmap status is updated only after passing gates.

## Concrete steps

All commands run from repository root: `/home/user/project`.

1. Baseline checks before edits.

    set -o pipefail
    make test 2>&1 | tee /tmp/9-1-1-baseline-test.log

2. Scaffold harness crate and workspace wiring.

    cargo new crates/rstest-bdd-harness --lib --vcs none

3. Implement core harness modules and tests.

    cargo test -p rstest-bdd-harness 2>&1 | tee /tmp/9-1-1-harness-tests.log

4. Add/adjust integration or behavioural tests in existing crates as required.

    cargo test -p rstest-bdd-macros 2>&1 | tee /tmp/9-1-1-macros-tests.log
    cargo test -p rstest-bdd 2>&1 | tee /tmp/9-1-1-runtime-tests.log

5. Update docs and roadmap.

    set -o pipefail
    make fmt 2>&1 | tee /tmp/9-1-1-fmt.log
    set -o pipefail
    make markdownlint 2>&1 | tee /tmp/9-1-1-markdownlint.log

6. Final required quality gates.

    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/9-1-1-check-fmt.log
    set -o pipefail
    make lint 2>&1 | tee /tmp/9-1-1-lint.log
    set -o pipefail
    make test 2>&1 | tee /tmp/9-1-1-test.log

Expected success indicators:

- each command exits with status 0,
- logs contain no `error:` lines from Rust compiler or Clippy,
- roadmap checkboxes for 9.1.x are `[x]` only after all gates succeed.

## Validation and acceptance

Acceptance criteria for phase 9.1 implementation:

- `crates/rstest-bdd-harness` exists and is part of the workspace.
- The crate exposes a harness adapter trait, shared runner types,
  `StdHarness`, and an attribute policy plugin interface.
- The default attribute policy emits only `#[rstest::rstest]`.
- Unit tests cover trait contract defaults, runner type behaviour, and policy
  output semantics.
- Behavioural tests exercise `StdHarness` runner execution semantics and policy
  behaviour from a consumer perspective.
- `docs/rstest-bdd-design.md` records design decisions taken.
- `docs/users-guide.md` explains usage and extension points.
- `docs/roadmap.md` marks `9.1.1`, `9.1.2`, and `9.1.3` as done.
- `make check-fmt`, `make lint`, and `make test` all succeed.

## Idempotence and recovery

Most steps are repeatable. If a gate fails:

- inspect the corresponding `/tmp/9-1-1-*.log`,
- fix the smallest local cause,
- re-run only the failed command,
- re-run the full required gates at the end.

If interface design needs to change after Stage B, record the change in
`Decision Log`, update this plan and design docs first, then continue.

## Artifacts and notes

Expected evidence files:

- `/tmp/9-1-1-baseline-test.log`
- `/tmp/9-1-1-harness-tests.log`
- `/tmp/9-1-1-macros-tests.log`
- `/tmp/9-1-1-runtime-tests.log`
- `/tmp/9-1-1-fmt.log`
- `/tmp/9-1-1-markdownlint.log`
- `/tmp/9-1-1-check-fmt.log`
- `/tmp/9-1-1-lint.log`
- `/tmp/9-1-1-test.log`

## Interfaces and dependencies

Target interface surface for `rstest-bdd-harness` at the end of this phase:

- A harness adapter trait that executes a scenario runner closure in a
  harness-owned environment.
- Shared runner types that describe runner inputs and outputs without binding
  to Tokio, GPUI, or other frameworks.
- `StdHarness` implementing the adapter trait with synchronous execution.
- An attribute policy plugin trait that returns test attributes for macro
  generation.
- A default policy implementation whose emitted attributes are exactly
  rstest-only.

Dependency constraints:

- `rstest-bdd-harness` may depend on workspace-core crates only as needed for
  shared types, but must not add Tokio or GPUI dependencies.
- `rstest-bdd` and `rstest-bdd-macros` remain free of new framework
  dependencies in this phase.

## Revision note

Initial draft created from roadmap phase 9.1, ADR-005 harness decision, and
current macro/runtime implementation state. Revised on 2026-02-08 after
implementation to mark completion, record test evidence, and capture
integration caveats for phase 9.2.
