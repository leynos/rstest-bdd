# ExecPlan 9.6.2: integration tests covering GPUI attribute policy resolution

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

`PLANS.md` is not present in this repository at the time of writing, so this
ExecPlan is the governing plan for roadmap item 9.6.2.

## Purpose / big picture

Roadmap item 9.6.2 closes the last validation gap for the GPUI attribute policy
integration delivered in phases 9.4.2 through 9.4.4. The repository already
contains:

- unit tests for `GpuiAttributePolicy` in
  `crates/rstest-bdd-harness-gpui/src/policy.rs` (three tests verifying emitted
  attributes, render output, and ordering);
- unit tests for GPUI policy path resolution in
  `crates/rstest-bdd-policy/src/lib.rs` (verifying that
  `resolve_test_attribute_hint_for_policy_path` maps
  `["rstest_bdd_harness_gpui", "GpuiAttributePolicy"]` to
  `TestAttributeHint::RstestWithGpuiTest`);
- macro codegen unit tests in
  `crates/rstest-bdd-macros/src/codegen/scenario/tests/gpui_policy.rs`
  (verifying that `generate_test_attrs` respects GPUI policy paths, emits
  `#[gpui::test]` for both sync and async functions, and deduplicates when a
  user-supplied `#[gpui::test]` attribute is already present);
- feature-gated integration tests in
  `crates/rstest-bdd/tests/scenario_harness_gpui.rs` (three scenarios
  exercising `GpuiHarness` with and without `GpuiAttributePolicy`, plus
  `GpuiAttributePolicy` without a harness);
- a user-facing example under `examples/gpui-counter` with two BDD scenarios
  using both `GpuiHarness` and `GpuiAttributePolicy`.

What is missing is a dedicated integration test file that validates the
end-to-end attribute policy resolution path for GPUI scenarios at the
`rstest-bdd` integration level, focusing specifically on **policy resolution
behaviour** rather than harness execution. The existing
`scenario_harness_gpui.rs` tests exercise harness context injection and
mutation but do not specifically validate that:

1. The `GpuiAttributePolicy` path is correctly resolved during macro expansion
   and the generated test function actually receives the `#[gpui::test]`
   attribute (proven by observing GPUI-specific behaviour that only occurs
   under `#[gpui::test]`).
2. A `scenarios!` macro invocation with
   `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy` discovers and
   runs feature scenarios with the correct policy-resolved attributes.
3. Policy resolution works for `#[scenario]` when `attributes` is specified
   without `harness` (the "attribute-only" path).
4. Policy resolution works for `#[scenario]` when both `harness` and
   `attributes` are specified together.

After this work:

- A new integration test file exists under `crates/rstest-bdd/tests/`
  exercising GPUI attribute policy resolution end-to-end across the
  `#[scenario]` and `scenarios!` code paths.
- Each test scenario validates that the generated test function's attributes
  are correct by observing policy-specific runtime behaviour.
- The existing GPUI unit, macro, and harness tests continue to pass unchanged.
- `docs/users-guide.md` records the new coverage under the GPUI harness
  section.
- `docs/rstest-bdd-design.md` §2.7.4 and §3.12 reflect the expanded
  validation surface.
- `docs/roadmap.md` marks 9.6.2 as done after all quality gates pass.

Success is observable when `make test` runs the new integration tests
successfully, the full gate set passes (`make check-fmt`, `make lint`,
`make test`), and the roadmap item is checked off.

## Constraints

- Implement only roadmap item 9.6.2 from `docs/roadmap.md`.
- Preserve Architecture Decision Record (ADR) 005 boundaries: GPUI
  dependencies remain outside core crates. New tests may depend on `gpui` and
  `rstest-bdd-harness-gpui`, but `rstest-bdd`, `rstest-bdd-macros`, and
  `rstest-bdd-harness` must not gain new GPUI-specific responsibilities.
- The new integration test file must be feature-gated behind
  `gpui-harness-tests` (same gate as the existing `scenario_harness_gpui.rs`),
  since it requires the workspace GPUI shim.
- Do not change the public API of `GpuiHarness`, `GpuiAttributePolicy`,
  `HarnessAdapter`, `AttributePolicy`, `#[scenario]`, or `scenarios!`.
- Do not introduce any new third-party dependencies.
- Keep files under 400 lines.
- Every new Rust module must begin with a `//!` module-level comment.
- Use en-GB-oxendict spelling in documentation and comments.
- Record design decisions in `docs/rstest-bdd-design.md`.
- Record user-facing usage in `docs/users-guide.md`.
- Required gates before completion: `make check-fmt`, `make lint`, and
  `make test`.
- Run long-lived commands with `set -o pipefail` and `tee`.

## Tolerances (exception triggers)

- Scope: if implementation grows beyond 8 files changed or 400 net lines,
  stop and escalate.
- Interfaces: if delivering the tests requires changing any public signatures
  in `GpuiHarness`, `GpuiAttributePolicy`, `HarnessAdapter`, `AttributePolicy`,
  `#[scenario]`, or `scenarios!`, stop and escalate.
- Dependencies: if the tests need a new external crate that is not already in
  the workspace, stop and escalate.
- Iterations: if `make check-fmt`, `make lint`, or `make test` each fail
  three consecutive times after attempted fixes, stop and escalate with logs.
- Ambiguity: if there is more than one reasonable interpretation of what
  "attribute policy resolution" should validate at integration level that
  materially changes the test surface, stop and resolve before implementation.

## Risks

- Risk: the `scenarios!` macro may not currently support
  `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy` in a
  feature-gated integration test because the compile-pass trybuild fixture
  (`scenarios_harness_params.rs`) only exercises `StdHarness` /
  `DefaultAttributePolicy`. Severity: medium. Likelihood: low. Mitigation:
  first confirm that `scenarios!` with `GpuiAttributePolicy` compiles
  successfully in a GPUI-enabled test target before writing assertions.

- Risk: new integration tests using `#[gpui::test]` or `gpui::run_test` may
  conflict with existing GPUI harness tests that use `serial_test::serial` for
  process-global state. Severity: medium. Likelihood: medium. Mitigation: keep
  all GPUI integration tests in one gated test binary or use `#[serial]` to
  prevent parallel execution of tests that use global GPUI state.

- Risk: verifying that `#[gpui::test]` was actually applied to the generated
  function is not directly observable at the Rust source level (it happens
  during macro expansion). Severity: medium. Likelihood: high. Mitigation:
  validate policy resolution indirectly by observing GPUI-specific runtime
  behaviour (such as `TestAppContext` availability via `#[gpui::test]`) that
  only occurs when the policy attribute is correctly applied. The existing
  `scenario_harness_gpui.rs` pattern of asserting on `TestAppContext` access
  serves as precedent.

- Risk: adding `scenarios!` integration tests with GPUI feature gates may
  require a new `[[test]]` entry in `crates/rstest-bdd/Cargo.toml` if the tests
  live in a separate file. Severity: low. Likelihood: high. Mitigation: add the
  `[[test]]` entry with `required-features = ["gpui-harness-tests"]` following
  the pattern of the existing `scenario_harness_gpui` entry.

## Progress

- [ ] Reviewed roadmap item 9.6.2 and prerequisite 9.4.3.
- [ ] Reviewed existing GPUI test coverage across all layers.
- [ ] Drafted this ExecPlan.
- [ ] Stage A: confirm baseline command health and identify gaps.
- [ ] Stage B: scaffold the new integration test file and feature file with
      red tests.
- [ ] Stage C: implement step definitions and make tests pass.
- [ ] Stage D: update documentation (users' guide, design doc, roadmap).
- [ ] Stage E: run full quality gates and capture logs.

## Surprises & Discoveries

(None yet — to be updated during implementation.)

## Decision Log

(To be updated during implementation.)

## Outcomes & Retrospective

(To be updated on completion.)

## Context and orientation

The repository is a Cargo workspace for `rstest-bdd`, a Behaviour-Driven
Development framework for Rust. The relevant crates and files for this task are
described below.

### Key crates

- `crates/rstest-bdd-policy/src/lib.rs`: centralizes `RuntimeMode`,
  `TestAttributeHint`, and `resolve_test_attribute_hint_for_policy_path`. This
  function performs exact segment matching against a table of three canonical
  policy paths: `DefaultAttributePolicy`, `TokioAttributePolicy`, and
  `GpuiAttributePolicy`. Unknown paths return `None`.

- `crates/rstest-bdd-harness/src/policy.rs`: defines `TestAttribute`,
  `AttributePolicy` (trait), and `DefaultAttributePolicy` (implementation
  emitting only `#[rstest::rstest]`).

- `crates/rstest-bdd-harness-gpui/src/policy.rs`: defines
  `GpuiAttributePolicy` (implementation emitting `#[rstest::rstest]` and
  `#[gpui::test]`). Unit-tested locally.

- `crates/rstest-bdd-harness-gpui/src/gpui_harness.rs`: defines
  `GpuiHarness` implementing `HarnessAdapter` with
  `type Context = TestAppContext`. Wraps scenario execution in
  `gpui::run_test`, builds a `TestAppContext`, and passes it through
  `request.run(context)`.

- `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs`: the macro
  codegen module that resolves attribute policies. `generate_test_attrs`
  converts an `attributes = ...` path into emitted test attributes via
  `resolve_attribute_policy` → `resolve_attribute_hint_from_policy_path` →
  `resolve_test_attribute_hint_for_policy_path`. For GPUI, this produces
  `#[rstest::rstest]` and `#[gpui::test]`.

- `crates/rstest-bdd-macros/src/codegen/scenario/tests/gpui_policy.rs`:
  three unit tests verifying GPUI policy paths, sync function emission, and
  deduplication.

### Existing integration test files

- `crates/rstest-bdd/tests/scenario_harness_gpui.rs`: gated behind
  `gpui-harness-tests`. Contains three scenarios:
  1. `GpuiHarness` alone — tests `TestAppContext` injection and mutation
     across steps using atomics.
  2. `GpuiHarness` + `GpuiAttributePolicy` — same steps as (1).
  3. `GpuiAttributePolicy` alone (no harness) — tests that steps execute
     under the GPUI policy without harness delegation.
  Also includes a direct `#[gpui::test]` function testing function-name
  preservation.

- `crates/rstest-bdd/tests/scenario_harness_tokio.rs`: (not feature-gated)
  analogous Tokio integration tests. Contains three scenarios: Tokio runtime
  active, Tokio harness + policy, and async step definitions.

- `crates/rstest-bdd/tests/scenario_harness.rs`: (not feature-gated)
  general harness integration tests with custom harness types, metadata
  capture, outline delegation, context injection via
  `rstest_bdd_harness_context`, and `StdHarness`/`DefaultAttributePolicy`.

### Feature file conventions

Integration test feature files live in `crates/rstest-bdd/tests/features/`.
Each scenario-binding test references its feature file relative to the crate
root (e.g., `path = "tests/features/gpui_harness.feature"`).

### Existing GPUI feature file

`crates/rstest-bdd/tests/features/gpui_harness.feature` contains three
scenarios used by `scenario_harness_gpui.rs`:

1. "GPUI harness injects TestAppContext"
2. "GPUI harness with GPUI attribute policy"
3. "GPUI attribute policy runs without harness"

### How attribute policy resolution works

When a user writes `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy`
on `#[scenario]` or `scenarios!`, the macro:

1. Extracts the path segments `["rstest_bdd_harness_gpui",
   "GpuiAttributePolicy"]`.
2. Calls `resolve_test_attribute_hint_for_policy_path` which returns
   `Some(TestAttributeHint::RstestWithGpuiTest)`.
3. Maps that hint to `ResolvedAttributePolicy::Gpui`.
4. Emits `#[rstest::rstest]` and `#[gpui::test]` on the generated test
   function (unless the user already supplied `#[gpui::test]`, in which case
   deduplication skips the second emission).

The `#[gpui::test]` attribute, provided by the workspace GPUI shim in
`vendor/gpui-macros`, wraps the test body in `gpui::run_test`, making
`gpui::TestAppContext` available to functions annotated with it. This is the
same mechanism `GpuiHarness` uses, but applied through the macro attribute
system rather than explicit harness delegation.

### Terms used in this plan

- **Attribute policy**: a type implementing `AttributePolicy` that decides
  which test attributes get emitted on generated scenario test functions.
- **Policy resolution**: the macro-time process of mapping a canonical path
  (e.g., `rstest_bdd_harness_gpui::GpuiAttributePolicy`) to a set of emitted
  test attributes.
- **Harness adapter**: a type implementing `HarnessAdapter` that decides how
  a scenario request executes.
- **Harness context**: the framework-specific object a harness injects into
  step execution. For GPUI, that object is `gpui::TestAppContext`.
- **BDD suite**: the combination of `.feature` files plus Rust step bindings.
- **Feature gate**: a Cargo feature flag that controls whether a test target
  is compiled. Here, `gpui-harness-tests` gates GPUI integration tests.

### Reference documents reviewed while drafting this plan

- `docs/roadmap.md` (item 9.6.2)
- `docs/rstest-bdd-design.md` (§2.7.4 and §3.12)
- `docs/users-guide.md` (GPUI harness section)
- `docs/adr-005-harness-adapter-crates-for-framework-specific-test-integration.md`
- `docs/adr-007-harness-context-injection.md`
- `docs/rstest-bdd-design.md`
- `docs/rstest-bdd-language-server-design.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/gherkin-syntax.md`
- `docs/execplans/9-4-5-gpui-demonstration-application.md`
- `docs/execplans/9-6-1-update-the-harness-adapter-chapter-in-the-users-guide.md`

## Plan of work

### Stage A: confirm baseline and identify test gaps

Goal: verify that existing GPUI tests pass and identify exactly which
integration-level policy resolution paths are not yet covered.

Implementation details:

- Run the existing GPUI-focused test suites to confirm a green baseline:
  `cargo test -p rstest-bdd-harness-gpui` and
  `cargo test -p rstest-bdd --test scenario_harness_gpui --features gpui-harness-tests`.
- Enumerate the specific integration-level gaps by comparing the existing
  `scenario_harness_gpui.rs` coverage against the full policy resolution
  surface:
  - `#[scenario]` with `attributes = GpuiAttributePolicy` but **no**
    `harness` (partially covered by "GPUI attribute policy runs without
    harness", but that scenario only checks step execution, not
    policy-specific observable behaviour).
  - `#[scenario]` with **both** `harness = GpuiHarness` and
    `attributes = GpuiAttributePolicy` (covered at harness level but not
    with a dedicated policy-resolution assertion).
  - `scenarios!` macro with
    `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy` (not
    covered at all — no integration test uses `scenarios!` with GPUI
    policy).
  - `#[scenario]` using `GpuiAttributePolicy` where the `#[gpui::test]`
    attribute provides observable context (such as `TestAppContext`
    availability without explicit harness delegation).
- Record the gap list for Stage B.

Go/no-go validation:

- Existing GPUI test suites pass.
- Gap list is concrete and scoped.

### Stage B: scaffold integration test file and feature file

Goal: add a new integration test file with red tests encoding the desired
policy-resolution behaviour.

Implementation details:

- Create a new feature file at
  `crates/rstest-bdd/tests/features/gpui_attribute_policy.feature` with
  scenarios targeting each identified gap. The scenarios should read as
  observable policy-resolution behaviour, not framework internals. Proposed
  scenarios:

  1. "GPUI attribute policy provides test context without explicit harness" —
     validates that `#[scenario]` with
     `attributes = GpuiAttributePolicy` (no `harness`) still receives a
     functioning GPUI test environment from the `#[gpui::test]` attribute
     emitted by the policy. Steps assert that `gpui::TestAppContext`
     methods work (via `#[gpui::test]`'s injection, not harness injection).
  2. "GPUI attribute policy with harness provides consistent context" —
     validates that `#[scenario]` with both `harness = GpuiHarness` and
     `attributes = GpuiAttributePolicy` provides harness-injected context
     that is consistent with the policy-resolved `#[gpui::test]` attribute.
  3. "Scenarios macro discovers features under GPUI attribute policy" —
     validates that `scenarios!` with
     `attributes = GpuiAttributePolicy` and `harness = GpuiHarness`
     discovers and runs feature scenarios with the correct policy attributes.

- Create a new integration test file at
  `crates/rstest-bdd/tests/gpui_attribute_policy_resolution.rs`, gated behind
  `#![cfg(feature = "gpui-harness-tests")]`. Add step definitions and scenario
  bindings for each scenario listed above. Start with assertions that will fail
  until Stage C completes the implementation (red phase).

- Register the new test target in `crates/rstest-bdd/Cargo.toml` with a
  `[[test]]` entry:

  ```toml
  [[test]]
  name = "gpui_attribute_policy_resolution"
  path = "tests/gpui_attribute_policy_resolution.rs"
  required-features = ["gpui-harness-tests"]
  ```

- Add a `scenarios!`-based feature directory if needed (or reuse the
  existing feature file with named scenario bindings and a `scenarios!`
  invocation for directory discovery).

Go/no-go validation:

- The new test file compiles as a workspace member.
- `cargo test -p rstest-bdd --test gpui_attribute_policy_resolution --features gpui-harness-tests`
  fails with expected assertion failures (red phase).

### Stage C: implement step definitions and make tests green

Goal: fill in step definitions and bindings so that all new scenarios pass.

Implementation details:

- For scenario 1 ("attribute policy without explicit harness"): the
  `GpuiAttributePolicy` causes the macro to emit `#[gpui::test]` on the
  generated test function. The workspace GPUI shim's `#[gpui::test]` attribute
  wraps the test body in `gpui::run_test`, which means the test runs inside a
  GPUI test context. Step definitions should observe a GPUI-specific effect
  that proves the policy attribute was applied. The observable effect is that
  `gpui::run_test` provides a dispatcher and executor environment. Steps can
  verify this by calling `gpui::TestDispatcher::new()` or similar lightweight
  GPUI API calls. However, without explicit harness delegation, the
  `TestAppContext` is **not** injected via `rstest_bdd_harness_context`. This
  scenario validates that the policy attribute alone creates the GPUI test
  environment.

- For scenario 2 ("policy with harness"): when both `harness = GpuiHarness`
  and `attributes = GpuiAttributePolicy` are specified, the macro emits
  `#[gpui::test]` (from the policy) and delegates execution through
  `GpuiHarness` (which calls `gpui::run_test` and injects `TestAppContext`).
  Steps should assert that `TestAppContext` is available via
  `#[from(rstest_bdd_harness_context)]` and that the dispatcher seed is a valid
  value, proving both the harness and policy paths cooperated.

- For scenario 3 ("scenarios! with GPUI policy"): use the `scenarios!` macro
  with `harness = GpuiHarness` and
  `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy` to discover
  feature files in a subdirectory. The step definitions can reuse existing step
  functions from the same file. The test validates that `scenarios!` correctly
  passes the `attributes` parameter through to each generated scenario test.

- Keep step definitions simple and focused on proving that the correct
  attributes were generated. Avoid duplicating the harness-injection testing
  already covered by `scenario_harness_gpui.rs`.

- Use `#[serial]` from `serial_test` where tests share process-global GPUI
  state.

Go/no-go validation:

- `cargo test -p rstest-bdd --test gpui_attribute_policy_resolution --features gpui-harness-tests`
  passes (green phase).
- Existing GPUI tests remain green:
  `cargo test -p rstest-bdd --test scenario_harness_gpui --features gpui-harness-tests`.

### Stage D: documentation and roadmap

Goal: make the new test coverage discoverable and update the roadmap.

Implementation details:

- Update `docs/rstest-bdd-design.md` §2.7.4 to mention the expanded
  integration test surface for GPUI attribute policy resolution, noting that
  coverage now includes `#[scenario]` attribute-only, combined harness+policy,
  and `scenarios!`-based discovery.

- Update `docs/rstest-bdd-design.md` §3.12 to reflect the expanded
  validation layer for GPUI policy resolution (attribute-only path,
  harness+policy path, and `scenarios!` macro path).

- Update `docs/users-guide.md` in the "Using the GPUI harness" section to
  note that integration-level coverage exists for all three GPUI policy
  resolution paths and link to the test file as an additional reference.

- Mark 9.6.2 done in `docs/roadmap.md` only after all stage validations
  and full gates pass.

Go/no-go validation:

- The design doc, users' guide, and roadmap all describe the delivered test
  coverage.

### Stage E: full validation and cleanup

Goal: prove the repository passes all required quality gates.

Implementation details:

- Run documentation formatting: `make fmt`.
- Run documentation validation: `make markdownlint`, `make nixie`.
- Run Rust quality gates: `make check-fmt`, `make lint`, `make test`.
- If any command fails, fix the underlying issue and rerun until clean or
  until a tolerance threshold is hit.

Go/no-go validation:

- All commands pass.
- The new integration test crate is included in workspace-wide testing.

## Concrete steps

Run all commands from the workspace root: `/home/user/project`.

1. Baseline GPUI plugin checks:

   ```bash
   set -o pipefail
   cargo test -p rstest-bdd-harness-gpui \
     2>&1 | tee /tmp/9-6-2-gpui-plugin-baseline.log
   ```

   Expected signal:

   ```plaintext
   test result: ok
   ```

2. Baseline GPUI integration checks:

   ```bash
   set -o pipefail
   cargo test -p rstest-bdd --test scenario_harness_gpui \
     --features gpui-harness-tests \
     2>&1 | tee /tmp/9-6-2-gpui-integration-baseline.log
   ```

   Expected signal:

   ```plaintext
   test result: ok
   ```

3. During Stage B and Stage C, iterate on the new integration test:

   ```bash
   set -o pipefail
   cargo test -p rstest-bdd \
     --test gpui_attribute_policy_resolution \
     --features gpui-harness-tests \
     2>&1 | tee /tmp/9-6-2-gpui-policy-resolution.log
   ```

   Expected red/green progression:

   ```plaintext
   before implementation: one or more assertions fail
   after implementation: test result: ok
   ```

4. After tests pass, run documentation formatting:

   ```bash
   set -o pipefail
   make fmt 2>&1 | tee /tmp/9-6-2-make-fmt.log
   ```

5. Run documentation validation:

   ```bash
   set -o pipefail
   make markdownlint 2>&1 | tee /tmp/9-6-2-markdownlint.log
   ```

   ```bash
   set -o pipefail
   make nixie 2>&1 | tee /tmp/9-6-2-nixie.log
   ```

6. Run the required Rust quality gates:

   ```bash
   set -o pipefail
   make check-fmt 2>&1 | tee /tmp/9-6-2-check-fmt.log
   ```

   ```bash
   set -o pipefail
   make lint 2>&1 | tee /tmp/9-6-2-lint.log
   ```

   ```bash
   set -o pipefail
   make test 2>&1 | tee /tmp/9-6-2-test.log
   ```

7. If every command succeeds, update `docs/roadmap.md` to mark 9.6.2 done.

## Validation and acceptance

Acceptance is behavioural, not structural.

Tests:

- `cargo test -p rstest-bdd --test gpui_attribute_policy_resolution --features gpui-harness-tests`
  passes and includes at least three scenarios covering:
  1. `#[scenario]` with `attributes = GpuiAttributePolicy` alone (no
     harness).
  2. `#[scenario]` with both `harness = GpuiHarness` and
     `attributes = GpuiAttributePolicy`.
  3. `scenarios!` with `harness = GpuiHarness` and
     `attributes = GpuiAttributePolicy`.
- Each scenario validates policy-specific observable behaviour (not just
  step execution).
- Existing GPUI suites remain green:
  - `cargo test -p rstest-bdd-harness-gpui`
  - `cargo test -p rstest-bdd --test scenario_harness_gpui --features gpui-harness-tests`

Lint and formatting:

- `make fmt` completes cleanly.
- `make markdownlint` passes.
- `make nixie` passes.
- `make check-fmt` passes.
- `make lint` passes.

Workspace regression gate:

- `make test` passes from the repository root and runs the new integration
  test as part of the workspace.

Documentation:

- `docs/users-guide.md` notes the expanded integration test coverage for
  GPUI attribute policy resolution.
- `docs/rstest-bdd-design.md` §2.7.4 and §3.12 reflect the expanded
  validation surface.
- `docs/roadmap.md` marks 9.6.2 done only after the gates above succeed.

## Idempotence and recovery

This plan is intentionally additive. Re-running the steps is safe.

- The integration test file creation is idempotent once files exist; reruns
  should only update content.
- If a step definition conflicts with existing step patterns in other test
  files (unlikely because integration tests compile as separate binaries), use
  distinct step text to avoid pattern collisions.
- If a late-stage gate fails, keep the roadmap checkbox unchecked and rerun
  the failing command after fixes.

## Artifacts and notes

Expected final artefacts:

- `crates/rstest-bdd/tests/gpui_attribute_policy_resolution.rs` (new)
- `crates/rstest-bdd/tests/features/gpui_attribute_policy.feature` (new)
- `crates/rstest-bdd/tests/features/gpui_attribute_policy_scenarios/` (new
  directory for `scenarios!` discovery, if needed)
- `crates/rstest-bdd/Cargo.toml` (updated `[[test]]` entry)
- `docs/rstest-bdd-design.md` (updated §2.7.4 and §3.12)
- `docs/users-guide.md` (updated GPUI harness section)
- `docs/roadmap.md` (mark 9.6.2 done)

Expected evidence to keep in logs:

- `/tmp/9-6-2-gpui-plugin-baseline.log`
- `/tmp/9-6-2-gpui-integration-baseline.log`
- `/tmp/9-6-2-gpui-policy-resolution.log`
- `/tmp/9-6-2-make-fmt.log`
- `/tmp/9-6-2-markdownlint.log`
- `/tmp/9-6-2-nixie.log`
- `/tmp/9-6-2-check-fmt.log`
- `/tmp/9-6-2-lint.log`
- `/tmp/9-6-2-test.log`

## Interfaces and dependencies

The new integration tests depend on the already-delivered GPUI integration
surface. No new interfaces are introduced.

Required crate dev-dependencies (already present in
`crates/rstest-bdd/Cargo.toml`):

- `rstest-bdd-macros` (for `#[scenario]`, `scenarios!`, `#[given]`,
  `#[when]`, `#[then]`)
- `rstest-bdd-harness-gpui` (for `GpuiHarness` and `GpuiAttributePolicy`)
- `rstest` (for `#[fixture]`)
- `gpui` (for `TestAppContext`, `TestDispatcher`)
- `serial_test` (for `#[serial]`)

Expected Rust-facing test structure:

```rust
#![cfg(feature = "gpui-harness-tests")]

use rstest_bdd_macros::{given, scenario, scenarios, then, when};
use serial_test::serial;

// Step definitions observing GPUI policy-specific runtime behaviour

#[scenario(
    path = "tests/features/gpui_attribute_policy.feature",
    name = "...",
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
)]
#[serial]
fn scenario_gpui_policy_without_harness() {}

#[scenario(
    path = "tests/features/gpui_attribute_policy.feature",
    name = "...",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
)]
#[serial]
fn scenario_gpui_policy_with_harness() {}

scenarios!(
    "tests/features/gpui_attribute_policy_scenarios",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
    attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy,
);
```

No changes to any existing trait, struct, or function signature are expected or
permitted.

## Revision note

Initial draft created on 2026-03-24 for roadmap item 9.6.2 after reviewing the
existing GPUI plugin crate, GPUI integration tests, macro codegen tests, policy
resolution code, example crates, design document, and users' guide.
