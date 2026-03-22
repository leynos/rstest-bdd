# ExecPlan 9.3.8: add a Tokio async demonstration application

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

`PLANS.md` is not present in this repository at the time of writing, so this
ExecPlan is the governing plan for roadmap item 9.3.8.

## Purpose / big picture

Roadmap item 9.3.8 is the missing user-facing Tokio deliverable in phase 9.3.
The workspace already contains the Tokio plugin crate
`crates/rstest-bdd-harness-tokio`, behavioural coverage for `TokioHarness` and
`TokioAttributePolicy`, and macro support for policy-backed test attributes.
What is still missing is the same kind of end-to-end example that now exists
for GPUI under `examples/gpui-counter`.

After this work:

- A new example crate exists under `examples/` and is added to the Cargo
  workspace.
- The example demonstrates a small application with real asynchronous
  behaviour, not just a harness smoke test.
- Its BDD suite binds scenarios with both
  `harness = rstest_bdd_harness_tokio::TokioHarness` and
  `attributes = rstest_bdd_harness_tokio::TokioAttributePolicy`.
- The example also carries focused unit tests for its own domain model so the
  feature is validated at both unit and behavioural levels.
- `docs/rstest-bdd-design.md` §2.7.4 records the chosen example shape and any
  Tokio-specific guidance discovered while implementing it.
- `docs/users-guide.md` links to the example as the canonical Tokio harness
  walkthrough.
- `docs/roadmap.md` marks 9.3.8 done only after all quality gates pass.

Success is observable when:

1. `cargo test -p <new-example-crate>` passes and runs both the new unit tests
   and the new BDD scenarios.
2. `make test` passes with the new example crate included in the workspace.
3. `make check-fmt` and `make lint` pass after the example, docs, and roadmap
   updates land.
4. The users' guide contains a direct pointer to the example as the canonical
   Tokio harness example.

## Constraints

- Implement only roadmap item 9.3.8 from `docs/roadmap.md`.
- Preserve ADR-005 boundaries: Tokio integration must remain in
  `rstest-bdd-harness-tokio` and other opt-in crates. The example may depend on
  Tokio and the Tokio harness crate, but core crates must not gain new
  Tokio-specific responsibilities.
- Treat this as a consumer-example task, not a framework redesign. If the
  example can only be delivered by changing public `HarnessAdapter`,
  `AttributePolicy`, `#[scenario]`, or `scenarios!` interfaces, stop and
  escalate.
- Validate with both unit tests and behavioural tests.
- Record any design decisions taken in `docs/rstest-bdd-design.md`, especially
  in §2.7.4.
- Record user-facing usage in `docs/users-guide.md`.
- Mark roadmap entry 9.3.8 done only after all validation passes.
- Keep files under 400 lines.
- Every new Rust module must begin with a `//!` module-level comment.
- Public APIs in the example crate must have Rustdoc comments and examples
  where appropriate.
- Required gates before completion:
  `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`,
  and `make test`.
- Run long-lived commands with `set -o pipefail` and `tee`.

## Tolerances (exception triggers)

- Scope: if implementation grows beyond 16 files changed or 850 net lines,
  stop and escalate.
- Interfaces: if the example requires a public API change in
  `rstest-bdd-harness-tokio`, `rstest-bdd-harness`, `rstest-bdd`, or
  `rstest-bdd-macros`, stop and escalate.
- Dependencies: if the example needs a new external crate not already present
  in `[workspace.dependencies]`, stop and escalate.
- Semantics: if current repository behaviour and current documentation disagree
  about how Tokio async steps are meant to work, resolve that discrepancy
  explicitly before coding past the example scaffold. Do not quietly encode one
  interpretation.
- Reliability: if the only viable example depends on TokioHarness's
  single-tick `yield_now()` drain to make assertions pass, stop and redesign
  the example. The example must teach a stable coordination pattern rather than
  relying on incidental scheduler timing.
- Iterations: if `make check-fmt`, `make lint`, or `make test` each fail three
  consecutive times after attempted fixes, stop and escalate with logs.

## Risks

- Risk: the repository currently contains mixed signals about Tokio async
  behaviour. `crates/rstest-bdd/tests/scenario_harness_tokio.rs` exercises
  `async fn` step definitions under `TokioHarness`, while
  `docs/rstest-bdd-design.md` §2.7.4 still says async step definitions are not
  supported inside `TokioHarness`. Severity: high. Likelihood: high.
  Mitigation: make semantic reconciliation the first implementation stage and
  document the outcome before finalizing the example.

- Risk: a minimal example could collapse into a duplicate of
  `crates/rstest-bdd/tests/scenario_harness_tokio.rs`, which would prove little
  beyond what the framework test already covers. Severity: medium. Likelihood:
  high. Mitigation: give the example a small domain model with observable async
  behaviour and a README that explains the pattern.

- Risk: an example that uses background tasks without explicit coordination
  will be flaky or will teach poor Tokio practice. Severity: high. Likelihood:
  medium. Mitigation: design the example around explicit `await`, channels,
  `JoinHandle`, or other deterministic completion signals.

- Risk: the example may need `rstest-bdd-harness` as a dev-dependency even if
  it does not import the crate directly, because the `#[scenario]` macro uses
  Cargo manifest lookup. Severity: medium. Likelihood: medium. Mitigation:
  follow the `examples/gpui-counter` dependency pattern and record the outcome
  in the plan if it is required.

- Risk: documentation drift. The users' guide already documents Tokio harness
  usage, but it does not yet point to a canonical example crate. Severity:
  medium. Likelihood: high. Mitigation: update design doc, users' guide, and
  roadmap in the same milestone as the example crate.

## Progress

- [x] (2026-03-22 00:00Z) Reviewed roadmap item 9.3.8 and prerequisite 9.3.4.
- [x] (2026-03-22 00:00Z) Reviewed existing Tokio harness tests, current design
      doc wording, users-guide Tokio sections, and the GPUI demonstration
      example.
- [x] (2026-03-22 00:00Z) Drafted this ExecPlan.
- [ ] Stage A: reconcile Tokio async semantics and lock the example shape.
- [ ] Stage B: scaffold the new example crate and establish red tests.
- [ ] Stage C: implement the example domain model and BDD steps.
- [ ] Stage D: update docs and roadmap state.
- [ ] Stage E: run full quality gates and capture logs.

## Surprises & Discoveries

- Observation: `examples/gpui-counter` already provides the expected precedent
  for a first-party harness example crate: small library crate, unit tests in
  `src/lib.rs`, BDD tests under `tests/`, feature files under
  `tests/features/`, README, users-guide link, design-doc note, and roadmap
  update. Impact: 9.3.8 should follow the same structure rather than inventing
  a new example layout.

- Observation: the users' guide already documents the Tokio harness and the
  Tokio attribute policy, but it currently stops at API-level explanation and
  does not link to a canonical example crate. Impact: the example must become
  the primary user-facing reference, not just an extra workspace member.

- Observation: the prompt refers to `rust-doctest-dry-guide.md` without a
  `docs/` prefix, but the actual repository path is
  `docs/rust-doctest-dry-guide.md`. Impact: doctest guidance in this plan uses
  the real path under `docs/`.

## Decision Log

- Decision: this plan treats the current Tokio semantic ambiguity as in scope
  for 9.3.8 because a canonical example cannot responsibly teach one behaviour
  while the design doc describes another. Rationale: example crates are user
  documentation with executable enforcement, so they must align with the
  documented model. Date/Author: 2026-03-22 / Codex.

- Decision: the example should be a compact library-style crate rather than a
  larger CLI or service binary unless implementation discovery proves a binary
  is necessary to make the async behaviour observable. Rationale: the existing
  demonstration crates (`japanese-ledger`, `gpui-counter`) are intentionally
  compact and easier to maintain. Date/Author: 2026-03-22 / Codex.

- Decision: the example must demonstrate deterministic async coordination.
  Rationale: teaching users to rely on one scheduler tick after `run()` would
  bake the known `TokioHarness::run` limitation into the canonical example.
  Date/Author: 2026-03-22 / Codex.

## Outcomes & Retrospective

This section is intentionally incomplete until implementation finishes. It must
be updated with the delivered crate name, final semantic decisions, changed
files, validation commands, and any lessons worth carrying forward.

## Context and orientation

The workspace already contains the following relevant material:

- `crates/rstest-bdd-harness-tokio/src/tokio_harness.rs`
  implements `TokioHarness` using a current-thread runtime plus `LocalSet`.
- `crates/rstest-bdd-harness-tokio/src/policy.rs`
  implements `TokioAttributePolicy`.
- `crates/rstest-bdd/tests/scenario_harness_tokio.rs`
  is the current end-to-end framework integration test for the Tokio harness
  and Tokio attribute policy.
- `crates/rstest-bdd/tests/features/tokio_harness.feature`
  contains the current Tokio behavioural scenarios.
- `examples/gpui-counter/`
  is the current reference shape for a harness-specific example crate.
- `examples/japanese-ledger/`
  is the current reference shape for a compact, library-style BDD example.
- `docs/rstest-bdd-design.md` §2.7.4
  documents first-party plugin targets and must record any 9.3.8 design
  decisions.
- `docs/users-guide.md`
  already contains "Using the Tokio harness" and related async sections, which
  must be updated to point to the new example.
- `docs/roadmap.md`
  contains the unchecked 9.3.8 item that must be marked done after validation.

Terms used in this plan:

- **Harness adapter**: a type implementing `HarnessAdapter` that owns scenario
  execution. Here that is `TokioHarness`.
- **Attribute policy**: a type implementing `AttributePolicy` that decides
  which test attributes are emitted on generated scenario tests. Here that is
  `TokioAttributePolicy`.
- **BDD suite**: the combination of `.feature` files plus Rust step bindings in
  `tests/`.
- **Async application behaviour**: asynchronous work that belongs to the
  example's own domain flow, such as spawned tasks, channels, or awaited
  operations, not just "a Tokio runtime exists".

Reference documents reviewed while drafting this plan:

- `docs/roadmap.md`
- `docs/rstest-bdd-design.md`
- `docs/rstest-bdd-language-server-design.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/gherkin-syntax.md`
- `docs/execplans/9-2-4-activate-tokio-current-thread-compatibility-alias.md`
- `docs/execplans/9-3-4-wire-test-attributes-into-codegen.md`
- `docs/execplans/9-4-5-gpui-demonstration-application.md`

## Plan of work

### Stage A: reconcile Tokio async semantics and lock the example shape

Goal: decide exactly what the canonical example is teaching before any new
crate files are written.

Implementation details:

- Review the current Tokio harness implementation, the behavioural test
  `crates/rstest-bdd/tests/scenario_harness_tokio.rs`, and the design-doc/user
  prose that describes Tokio behaviour.
- Determine whether the canonical example should demonstrate:
  - synchronous scenario functions plus `async fn` step definitions, or
  - synchronous scenario functions plus synchronous steps that coordinate async
    work explicitly inside Tokio.
- If the code and docs disagree, choose the behaviour that matches the shipped
  implementation and update the docs accordingly in later stages.
- Choose the example crate name and domain. The domain must be:
  - small enough to fit the existing example style,
  - obviously asynchronous,
  - deterministic under test,
  - understandable to a new user without framework internals.

Go/no-go validation:

- A single example topology is chosen and documented in `Decision Log`.
- The chosen topology does not require framework API changes.

### Stage B: scaffold the new example crate and establish red tests

Goal: create the crate skeleton and failing tests that define the desired
behaviour before implementation turns them green.

Implementation details:

- Add a new workspace member under `examples/` with:
  - `Cargo.toml`
  - `README.md`
  - `src/lib.rs`
  - `tests/<example>.rs`
  - `tests/features/<example>.feature`
- Mirror the dependency pattern from `examples/gpui-counter` and only add
  Tokio-related dependencies that already exist in the workspace.
- Write unit tests in `src/lib.rs` for the example's domain model first.
- Write BDD scenarios that exercise both `TokioHarness` and
  `TokioAttributePolicy`.
- Keep Gherkin steps observable and user-facing. The feature file should
  describe application behaviour, not internal runtime mechanics.

Expected red-state checks:

```plaintext
set -o pipefail; cargo test -p <new-example-crate> 2>&1 | tee /tmp/9-3-8-example-red.log
```

The first failing run should show missing implementation or behaviour
assertions, not unexplained macro wiring failures.

### Stage C: implement the example domain model and BDD steps

Goal: make the new unit tests and BDD tests pass with a clear, maintainable
example.

Implementation details:

- Implement the example library with small, focused functions and a
  fixture-friendly state container.
- Use `rstest` fixtures to share example state across steps.
- Bind scenarios with both:
  - `harness = rstest_bdd_harness_tokio::TokioHarness`
  - `attributes = rstest_bdd_harness_tokio::TokioAttributePolicy`
- Implement step definitions that demonstrate real async behaviour in the
  example domain according to the Stage A decision.
- Add at least one doctest on the example's public API if the crate surface
  warrants it, keeping the doctest focused on public usage rather than test
  internals.
- Keep helper logic extracted and small so the example teaches readable code,
  not a monolithic step-definition file.

Go/no-go validation:

```plaintext
set -o pipefail; cargo test -p <new-example-crate> 2>&1 | tee /tmp/9-3-8-example-green.log
```

Success means the example crate's unit tests and BDD scenarios pass in
isolation before the full workspace gates are run.

### Stage D: update docs and roadmap state

Goal: make the example discoverable and keep project documentation consistent.

Implementation details:

- Update `docs/rstest-bdd-design.md` §2.7.4 to describe the delivered Tokio
  example and any semantic clarification uncovered in Stage A.
- Update `docs/users-guide.md` to point to the example as the canonical Tokio
  harness walkthrough and to summarise the pattern it demonstrates.
- Update `docs/roadmap.md` to mark 9.3.8 done only after all gates in Stage E
  pass.
- Ensure the example `README.md` explains how to run the focused crate tests
  and what the scenarios demonstrate.

Go/no-go validation:

- A new user reading the users' guide can discover the example directly and can
  tell why it is the recommended Tokio harness reference.

### Stage E: run full quality gates and capture logs

Goal: prove the example integrates cleanly into the workspace and the docs
remain healthy.

Run the full gate set with logged output:

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/9-3-8-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/9-3-8-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/9-3-8-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/9-3-8-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/9-3-8-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/9-3-8-make-test.log
```

Expected success indicators:

- `make check-fmt` exits 0 with no formatting diffs.
- `make lint` exits 0 with no Clippy or repository-lint warnings.
- `make test` exits 0 and includes the new example crate in the workspace test
  run.
- `make markdownlint` and `make nixie` exit 0 after the documentation changes.

If any gate fails, fix the underlying issue and rerun the affected command
until all gates pass or a tolerance threshold is reached.

## Approval gate

This document is the draft-phase ExecPlan required by the `execplans` skill.
Implementation must not begin until the user explicitly approves the plan or
requests revisions.
