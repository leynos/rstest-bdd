# ExecPlan 9.4.5: add a GPUI demonstration application

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT (2026-03-12)

`PLANS.md` is not present in this repository at the time of writing, so this
ExecPlan is the governing plan for roadmap item 9.4.5.

## Purpose / big picture

Roadmap item 9.4.5 closes the last missing GPUI deliverable from phase 9.4. The
repository already contains the GPUI plugin crate
`crates/rstest-bdd-harness-gpui`, behavioural coverage for `GpuiHarness` and
`GpuiAttributePolicy`, and integration coverage in
`crates/rstest-bdd/tests/scenario_harness_gpui.rs`. What is still missing is a
user-facing example under `examples/` that shows how an application crate
adopts that plugin in the same style as the existing `todo-cli` and
`japanese-ledger` examples.

After this work:

- A new workspace example crate exists under `examples/` with the same
  maintenance posture as the existing demonstration crates.
- The example's BDD suite exercises `harness = GpuiHarness` and
  `attributes = GpuiAttributePolicy` in generated `#[scenario]` bindings.
- Step definitions demonstrate direct access to injected
  `gpui::TestAppContext` through
  `#[from(rstest_bdd_harness_context)] context: &gpui::TestAppContext` and
  `&mut gpui::TestAppContext`.
- The example also contains unit tests for its own domain model so the feature
  is validated with both unit tests and behavioural tests.
- `docs/rstest-bdd-design.md` records the design decisions taken while shaping
  the example.
- `docs/users-guide.md` points to the example as the canonical GPUI harness
  walkthrough and clearly states any native-library setup that is or is not
  required.
- `docs/roadmap.md` marks 9.4.5 as done only after every gate passes.

Success is observable when `make test` runs the new example crate successfully,
the example's BDD scenarios pass, and the full gate set passes: `make fmt`,
`make markdownlint`, `make nixie`, `make check-fmt`, `make lint`, and
`make test`.

## Constraints

- Implement only roadmap item 9.4.5 from `docs/roadmap.md`.
- Preserve ADR-005 boundaries: GPUI dependencies remain outside core crates.
  The example may depend on `gpui` and `rstest-bdd-harness-gpui`, but
  `rstest-bdd`, `rstest-bdd-macros`, and `rstest-bdd-harness` must not gain new
  GPUI-specific responsibilities.
- Preserve ADR-007 usage: step access to harness context must continue to flow
  through `rstest_bdd_harness_context`.
- Keep the example aligned with the current workspace GPUI shim in
  `vendor/gpui`; do not design the example around APIs that the repository does
  not currently expose.
- Do not introduce a new third-party dependency unless the repository already
  carries it in the workspace or the user explicitly approves a new one.
- Keep files under 400 lines.
- Every new Rust module must begin with a `//!` module-level comment.
- Public APIs in the example crate must include Rustdoc, with examples where
  appropriate.
- Record final design choices in `docs/rstest-bdd-design.md` §2.7.4.
- Record user-facing usage and setup in `docs/users-guide.md`.
- Mark roadmap item 9.4.5 done only after all validation passes.
- Required gates before completion:
  `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`,
  and `make test`.
- Run long-lived commands with `set -o pipefail` and `tee`.

## Tolerances (exception triggers)

- Scope: if implementation grows beyond 14 files changed or 750 net lines,
  stop and escalate.
- Interfaces: if delivering the example requires changing public signatures in
  `GpuiHarness`, `GpuiAttributePolicy`, `HarnessAdapter`, or `#[scenario]`,
  stop and escalate.
- Dependencies: if the example needs a new external crate that is not already
  in the workspace, stop and escalate.
- Native setup: if the example uncovers native-library requirements that are
  not satisfied by the workspace GPUI shim, stop and document the exact linker
  or runtime failure before proceeding.
- Iterations: if `make check-fmt`, `make lint`, or `make test` each fail three
  consecutive times after attempted fixes, stop and escalate with logs.
- Ambiguity: if there is more than one reasonable interpretation of
  "demonstration application" that materially changes the crate shape
  (library-only example vs. binary app vs. mixed app), stop and resolve that
  choice before implementation.

## Risks

- Risk: the new example could duplicate the existing GPUI integration test
  almost verbatim, which would add maintenance cost without improving user
  guidance. Severity: medium. Likelihood: high. Mitigation: give the example a
  small but real domain model and README, and keep the integration test focused
  on framework semantics.

- Risk: proving `TestAppContext` injection via process-global statics would
  force serial execution and teach poor example hygiene. Severity: medium.
  Likelihood: medium. Mitigation: keep observation state inside the example's
  fixture-owned domain model and compare context-derived values there.

- Risk: roadmap wording mentions native-library setup, but this workspace uses
  a local GPUI test shim rather than the full upstream native stack. Severity:
  medium. Likelihood: high. Mitigation: explicitly verify the actual runtime
  requirements during implementation and document the result precisely, even if
  the answer is "no extra native libraries are required in this repository".

- Risk: adding the example to the workspace may expose latent formatting,
  Clippy, or doctest issues in example code that are easy to miss if only the
  focused crate test is run. Severity: medium. Likelihood: medium. Mitigation:
  run focused example tests first, then the full repository gate set.

- Risk: the minimal GPUI shim may not support a full visual application model.
  Severity: low. Likelihood: medium. Mitigation: keep the example scoped to
  harness and context behaviour, not rendering or window-management features
  outside the shim's surface.

## Progress

- [x] (2026-03-12 00:00Z) Reviewed roadmap item 9.4.5 and prerequisite 9.4.4.
- [x] (2026-03-12 00:00Z) Reviewed `docs/rstest-bdd-design.md` §2.7.4 and the
      current GPUI users-guide section.
- [x] (2026-03-12 00:00Z) Reviewed existing examples under `examples/`.
- [x] (2026-03-12 00:00Z) Reviewed current GPUI harness integration tests and
      the workspace GPUI shim.
- [x] (2026-03-12 00:00Z) Drafted this ExecPlan.
- [ ] Stage A: confirm the final example topology and baseline command health.
- [ ] Stage B: scaffold the new example crate and red tests.
- [ ] Stage C: implement the example domain model and BDD steps.
- [ ] Stage D: harden docs, native-setup guidance, and roadmap state.
- [ ] Stage E: run full quality gates and capture logs.

## Surprises & Discoveries

- Observation: the task prompt references `rust-doctest-dry-guide.md` at the
  repository root, but the actual file in this repository is
  `docs/rust-doctest-dry-guide.md`. Evidence: repository file lookup on
  2026-03-12. Impact: this ExecPlan uses the path under `docs/` as the source
  of truth.

- Observation: the only current example crates are `examples/todo-cli` and
  `examples/japanese-ledger`; both are intentionally small and keep their BDD
  assets under `tests/features/`. Evidence: workspace tree inspection on
  2026-03-12. Impact: the GPUI example should follow that pattern instead of
  inventing a new structure.

- Observation: the repository uses a workspace-local GPUI shim under
  `vendor/gpui` and `vendor/gpui-macros`. The shim exposes only the surface
  needed by `rstest-bdd`: `#[gpui::test]`, `run_test`, `TestDispatcher`,
  `BackgroundExecutor`, and `TestAppContext`. Evidence:
  `vendor/gpui/src/lib.rs` and `vendor/gpui-macros/src/lib.rs`. Impact: the
  example should demonstrate harness and context usage, not upstream GPUI view
  APIs that the shim does not provide.

## Decision Log

- Decision: the example crate should be a small library-style application under
  `examples/gpui-counter` rather than a larger binary application. Rationale:
  the existing demonstration crates are intentionally compact, and a
  library-style example is sufficient to prove `GpuiHarness`,
  `GpuiAttributePolicy`, and `TestAppContext` injection end to end.
  Date/Author: 2026-03-12 / Codex.

- Decision: the primary BDD scenario binding should specify both
  `harness = rstest_bdd_harness_gpui::GpuiHarness` and
  `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy`. Rationale: this
  gives one canonical code path that exercises both GPUI knobs together,
  instead of forcing users to assemble the pieces mentally from separate tests.
  Date/Author: 2026-03-12 / Codex.

- Decision: context observations should be stored in the example fixture-owned
  model instead of process-global statics. Rationale: examples should model
  parallel-safe, reusable patterns rather than serial-test-only techniques.
  Date/Author: 2026-03-12 / Codex.

- Decision: the users' guide should document the exact native setup result from
  implementation, even if that result is "none required in this workspace".
  Rationale: roadmap item 9.4.5 explicitly calls for clear native-library
  guidance, and silence would leave the deliverable incomplete. Date/Author:
  2026-03-12 / Codex.

## Outcomes & Retrospective

This work has not been implemented yet. Expected delivered outcomes are:

- A new example crate under `examples/` that can be run by workspace test
  commands.
- Unit coverage for the example's own domain logic.
- Behavioural coverage through `#[scenario]` bindings that demonstrate GPUI
  harness context injection.
- Users-guide and design-doc updates that explain the example and the native
  setup story.
- Roadmap item 9.4.5 marked complete after validation.

Retrospective placeholder:

- Revisit whether the chosen example remained the smallest useful expression of
  GPUI harness integration.
- Record any friction encountered when moving from the repository's GPUI shim
  to the user-facing example.

## Context and orientation

The repository is a Cargo workspace. Relevant current files are:

- `Cargo.toml`: workspace members currently include `examples/todo-cli` and
  `examples/japanese-ledger`, but no GPUI example yet.
- `examples/todo-cli/` and `examples/japanese-ledger/`: existing reference
  examples for scope, file layout, and README expectations.
- `crates/rstest-bdd-harness-gpui/src/gpui_harness.rs`: the delivered GPUI
  harness adapter.
- `crates/rstest-bdd-harness-gpui/src/policy.rs`: the delivered GPUI attribute
  policy plugin.
- `crates/rstest-bdd/tests/scenario_harness_gpui.rs` and
  `crates/rstest-bdd/tests/features/gpui_harness.feature`: the current GPUI
  integration tests that validate framework semantics but are not a user-facing
  example.
- `docs/rstest-bdd-design.md` §2.7.4: design document section describing
  first-party plugin targets.
- `docs/users-guide.md`: current GPUI harness documentation, which explains the
  API but does not yet point to a canonical example crate.
- `docs/roadmap.md`: roadmap section 9.4.5, currently unchecked.
- `vendor/gpui/src/lib.rs`: workspace-local GPUI shim that defines
  `TestAppContext`, `run_test`, and related test support.

Terms used in this plan:

- **Harness adapter**: a type implementing `HarnessAdapter` that decides how a
  scenario request executes. Here that is `GpuiHarness`.
- **Attribute policy**: a type implementing `AttributePolicy` that decides
  which test attributes get emitted on generated scenario tests. Here that is
  `GpuiAttributePolicy`.
- **Harness context**: the framework-specific object a harness injects into
  step execution. For GPUI, that object is `gpui::TestAppContext`.
- **BDD suite**: the combination of `.feature` files plus Rust step bindings in
  `tests/`.

Reference documents reviewed while drafting this plan:

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

### Stage A: confirm topology and baseline

Goal: lock down the crate shape and confirm there is no hidden infrastructure
constraint before writing code.

Implementation details:

- Re-read the existing example crates and confirm the new GPUI example should
  be a library-style example with `src/lib.rs`, `tests/*.rs`, and
  `tests/features/*.feature`.
- Verify whether the current workspace GPUI shim requires any native-library
  setup to run tests. If no extra setup is required, document that result
  explicitly for later propagation to `docs/users-guide.md`.
- Record baseline command health for the focused GPUI suites:
  `cargo test -p rstest-bdd-harness-gpui --features native-gpui-tests` and
  `cargo test -p rstest-bdd --test scenario_harness_gpui --features gpui-harness-tests`.

Go/no-go validation:

- The final crate name and layout are chosen.
- The native-setup story is known well enough to document accurately.
- Baseline GPUI-focused suites pass before the example is added.

### Stage B: scaffold the example crate and write red tests

Goal: add a new workspace member and encode desired behaviour before filling in
all implementation details.

Implementation details:

- Add `examples/gpui-counter` to the workspace members in `Cargo.toml`.
- Create:
  - `examples/gpui-counter/Cargo.toml`
  - `examples/gpui-counter/README.md`
  - `examples/gpui-counter/src/lib.rs`
  - `examples/gpui-counter/tests/counter.rs`
  - `examples/gpui-counter/tests/features/counter.feature`
- Mirror the dependency posture of the other examples, plus GPUI-specific
  dev-dependencies: `gpui`, `rstest-bdd-harness-gpui`, `rstest`, `rstest-bdd`,
  `rstest-bdd-macros`.
- Define the intended behaviour first in tests:
  - Unit tests for the example's domain model in `src/lib.rs`.
  - BDD scenarios in `tests/counter.rs` that bind to the feature file and
    assert the desired counter behaviour plus `TestAppContext` access.
- Choose step text that reads like a tiny application, not a framework test.
  Example theme: a counter application records user increments while also
  capturing GPUI harness context details.

Go/no-go validation:

- `cargo test -p gpui-counter` fails only for the expected red-phase reasons
  before the implementation is completed.
- The example compiles as a workspace member.

### Stage C: implement the example domain model and step bindings

Goal: make the red tests pass with the smallest clear implementation.

Implementation details:

- In `examples/gpui-counter/src/lib.rs`, implement a small domain model such as
  `CounterApp` plus an observation record for GPUI context facts. Keep the
  public surface tiny and documented.
- Keep example logic self-contained and deterministic. The model should be easy
  to exercise from both unit tests and BDD steps.
- In `examples/gpui-counter/tests/counter.rs`:
  - add a fixture returning the example model;
  - add `#[given]`, `#[when]`, and `#[then]` step definitions;
  - demonstrate immutable and mutable step access to
    `gpui::TestAppContext` via `#[from(rstest_bdd_harness_context)]`;
  - bind at least one scenario with both
    `harness = rstest_bdd_harness_gpui::GpuiHarness` and
    `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy`.
- The step assertions should show a real user benefit and a real harness fact.
  A good target is:
  - the counter value changes as requested;
  - the same injected `TestAppContext` is visible across steps;
  - a GPUI-specific method such as `on_quit`, `dispatcher().seed()`, or
    `test_function_name()` is observed and recorded in the model.
- If a second scenario improves clarity, add one. Keep the feature file small
  enough to remain a teaching example.

Go/no-go validation:

- `cargo test -p gpui-counter` passes.
- The BDD suite clearly demonstrates harness-context injection.
- No globals or serial-only scaffolding are required.

### Stage D: documentation and roadmap hardening

Goal: make the new example discoverable and keep docs consistent.

Implementation details:

- Update `docs/rstest-bdd-design.md` §2.7.4 to record the design decisions
  taken for the example, including the chosen scope and any native-setup
  conclusion.
- Update `docs/users-guide.md` in the GPUI harness section to:
  - link to `examples/gpui-counter`;
  - show the canonical `#[scenario(...)]` binding using both
    `GpuiHarness` and `GpuiAttributePolicy`;
  - explain how steps request `TestAppContext`;
  - state exactly what native-library setup is required in this repository.
- Update `examples/gpui-counter/README.md` with focused instructions for
  running just the example crate.
- Mark 9.4.5 done in `docs/roadmap.md` only after all stage validations and
  full gates pass.

Go/no-go validation:

- The design doc, users' guide, README, and roadmap all describe the same
  delivered example.
- The users' guide contains an explicit statement about native setup.

### Stage E: full validation and cleanup

Goal: prove the repository still passes all required quality gates.

Implementation details:

- Run focused crate checks first for faster feedback.
- Run the documentation gates because this task changes Markdown.
- Run the required Rust gates from the workspace root.
- If any command fails, fix the underlying issue and rerun until clean or until
  a tolerance threshold is hit.

Go/no-go validation:

- All commands in `Validation and acceptance` pass.
- The example crate is included in workspace-wide testing.

## Concrete steps

Run all commands from the workspace root: `/home/user/project`.

1. Baseline GPUI-focused checks:

```bash
set -o pipefail
cargo test -p rstest-bdd-harness-gpui --features native-gpui-tests \
  2>&1 | tee /tmp/9-4-5-gpui-plugin-baseline.log
```

Expected signal:

```plaintext
test result: ok
```

1. Baseline GPUI integration check:

```bash
set -o pipefail
cargo test -p rstest-bdd --test scenario_harness_gpui \
  --features gpui-harness-tests \
  2>&1 | tee /tmp/9-4-5-gpui-integration-baseline.log
```

Expected signal:

```plaintext
test result: ok
```

1. During Stage B and Stage C, iterate on the new example crate:

```bash
set -o pipefail
cargo test -p gpui-counter 2>&1 | tee /tmp/9-4-5-gpui-counter.log
```

Expected red/green progression:

```plaintext
before implementation: one or more unit/BDD assertions fail
after implementation: test result: ok
```

1. Once the example passes locally, run the documentation formatting pass:

```bash
set -o pipefail
make fmt 2>&1 | tee /tmp/9-4-5-make-fmt.log
```

1. Run documentation validation:

```bash
set -o pipefail
PATH=/root/.bun/bin:$PATH make markdownlint \
  2>&1 | tee /tmp/9-4-5-markdownlint.log
```

```bash
set -o pipefail
make nixie 2>&1 | tee /tmp/9-4-5-nixie.log
```

1. Run the required Rust quality gates:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/9-4-5-check-fmt.log
```

```bash
set -o pipefail
make lint 2>&1 | tee /tmp/9-4-5-lint.log
```

```bash
set -o pipefail
make test 2>&1 | tee /tmp/9-4-5-test.log
```

1. If every command succeeds, update `docs/roadmap.md` to mark 9.4.5 done.

## Validation and acceptance

Acceptance is behavioural, not structural.

Tests:

- `cargo test -p gpui-counter` passes and includes both unit coverage for the
  example model and the BDD scenario suite.
- The BDD suite demonstrates step access to injected `gpui::TestAppContext`.
- Existing GPUI-focused suites still pass:
  - `cargo test -p rstest-bdd-harness-gpui --features native-gpui-tests`
  - `cargo test -p rstest-bdd --test scenario_harness_gpui --features gpui-harness-tests`

Lint and formatting:

- `make fmt` completes cleanly.
- `make markdownlint` passes.
- `make nixie` passes.
- `make check-fmt` passes.
- `make lint` passes.

Workspace regression gate:

- `make test` passes from the repository root and runs the new example crate as
  part of the workspace.

Documentation:

- `docs/users-guide.md` links to the new example and explicitly states the
  native setup story.
- `docs/rstest-bdd-design.md` records the design choices taken for the example.
- `docs/roadmap.md` marks 9.4.5 done only after the gates above succeed.

## Idempotence and recovery

This plan is intentionally additive. Re-running the steps is safe.

- The example crate creation is idempotent once files exist; reruns should only
  update content.
- If Stage B chooses the wrong example name or layout, fix that before Stage C
  rather than carrying both structures forward.
- If the example proves too ambitious for the shim's surface, simplify the
  domain model instead of broadening the shim unless the user approves that
  wider scope.
- If a late-stage gate fails, keep the roadmap checkbox unchecked and rerun the
  failing command after fixes.

## Artifacts and notes

Expected final artefacts:

- `examples/gpui-counter/Cargo.toml`
- `examples/gpui-counter/README.md`
- `examples/gpui-counter/src/lib.rs`
- `examples/gpui-counter/tests/counter.rs`
- `examples/gpui-counter/tests/features/counter.feature`
- `docs/rstest-bdd-design.md` updates
- `docs/users-guide.md` updates
- `docs/roadmap.md` update

Expected evidence to keep in logs:

- `/tmp/9-4-5-gpui-plugin-baseline.log`
- `/tmp/9-4-5-gpui-integration-baseline.log`
- `/tmp/9-4-5-gpui-counter.log`
- `/tmp/9-4-5-make-fmt.log`
- `/tmp/9-4-5-markdownlint.log`
- `/tmp/9-4-5-nixie.log`
- `/tmp/9-4-5-check-fmt.log`
- `/tmp/9-4-5-lint.log`
- `/tmp/9-4-5-test.log`

## Interfaces and dependencies

The example crate should depend on the already-delivered GPUI integration
surface, not invent a new one.

Required crate dependencies:

- `gpui` as a dev-dependency, matching the workspace dependency.
- `rstest-bdd-harness-gpui` as a dev-dependency.
- `rstest`, `rstest-bdd`, and `rstest-bdd-macros` as dev-dependencies.

Expected Rust-facing interfaces:

```rust
pub struct CounterApp {
    /* application state plus GPUI context observations */
}
```

```rust
#[scenario(
    path = "tests/features/counter.feature",
    name = "...",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
)]
fn counter_scenario(/* example fixture(s) */) {}
```

```rust
#[when("...")]
fn step_name(
    app: &mut CounterApp,
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    /* record both application and harness effects */
}
```

The example should not require any new extension points in
`rstest-bdd-harness-gpui`; the entire point of 9.4.5 is to prove that the
existing plugin interfaces are sufficient for a real application example.

## Revision note

Initial draft created on 2026-03-12 for roadmap item 9.4.5 after reviewing the
existing GPUI plugin crate, GPUI integration tests, example crates, and the
workspace GPUI shim.
