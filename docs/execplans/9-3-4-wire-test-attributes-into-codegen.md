# ExecPlan 9.3.4: Wire `AttributePolicy::test_attributes()` Into Codegen

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

`PLANS.md` is not present in this repository at the time of writing, so this
ExecPlan is the governing plan for this task.

## Purpose / big picture

Roadmap item 9.3.4 targets a concrete behaviour gap: macro codegen currently
ignores the selected attribute policy and always emits only
`#[rstest::rstest]`. This blocks the ADR-005 plug-in model from fully owning
framework attributes.

After this change:

- Macro codegen resolves an attribute policy for both harness and non-harness
  paths and uses policy-defined test attributes when generating scenario tests.
- A scenario using `attributes = rstest_bdd_harness_tokio::TokioAttributePolicy`
  emits `#[tokio::test(flavor = "current_thread")]` in expanded output.
- Existing scenarios continue to pass unchanged.
- Unit tests and behavioural tests cover policy-driven attribute emission.
- `docs/rstest-bdd-design.md` and `docs/users-guide.md` reflect the delivered
  behaviour.
- `docs/roadmap.md` marks 9.3.4 as done once all quality gates pass.

## Constraints

- Implement roadmap item 9.3.4 only; do not pull in 9.3.5+ scope.
- Preserve ADR-005 separation: Tokio/GPUI integrations remain opt-in and must
  not be hard-wired into core runtime behaviour.
- Keep backward compatibility for existing macro call sites and diagnostics.
- Keep `#[scenario]`/`scenarios!` trait-bound assertions for
  `AttributePolicy` intact.
- Validate with both unit and behavioural tests before completion.
- Update `docs/rstest-bdd-design.md` with any decisions taken.
- Update `docs/users-guide.md` with user-facing usage and semantics.
- Mark roadmap entry 9.3.4 done only after all gates pass.
- Required gates before completion: `make check-fmt`, `make lint`, and
  `make test`.

## Tolerances (exception triggers)

- Scope: if implementation exceeds 14 files or 700 net lines, stop and
  escalate.
- Interface: if public API changes are required in `rstest-bdd-harness` or
  macro input syntax, stop and escalate.
- Dependency: if satisfying 9.3.4 requires adding Tokio as a direct dependency
  of `rstest-bdd-macros`, stop and escalate.
- Behaviour: if async scenario behaviour without explicit policies regresses,
  stop and escalate.
- Iterations: if the same gate (`check-fmt`, `lint`, or `test`) fails three
  times after fixes, stop and escalate with logs.
- Ambiguity: if roadmap wording conflicts with proc-macro limitations around
  calling user trait methods at expansion time, stop and request direction with
  concrete options.

## Risks

- Risk: Rust proc macros cannot generally execute arbitrary user-supplied
  trait methods at expansion time. Severity: high. Likelihood: high.
  Mitigation: add an explicit de-risk stage first; document and agree the
  resolution model before broad edits.

- Risk: policy resolution diverges between harness and non-harness paths,
  causing inconsistent attributes. Severity: medium. Likelihood: medium.
  Mitigation: centralize policy resolution in one helper and test both paths.

- Risk: policy-driven attributes accidentally duplicate or conflict with
  existing runtime-derived `tokio::test` emission. Severity: medium.
  Likelihood: medium. Mitigation: add unit tests for de-duplication and
  precedence rules.

- Risk: documentation drift (design doc and user guide still describe old
  behaviour). Severity: medium. Likelihood: medium. Mitigation: update docs in
  the same milestone as code and tests.

## Progress

- [x] (2026-02-23 18:34Z) Reviewed roadmap item 9.3.4, current macro code,
      and design/user docs for policy behaviour.
- [x] (2026-02-23 18:34Z) Drafted this ExecPlan.
- [x] (2026-02-23) Stage A: de-risk policy-evaluation approach and lock
      implementation strategy.
- [x] (2026-02-23) Stage B: implement policy resolution and
      attribute-token synthesis.
- [x] (2026-02-23) Stage C: wire attribute emission through harness and
      non-harness codegen paths.
- [x] (2026-02-23) Stage D: add/adjust unit tests and behavioural tests.
- [x] (2026-02-23) Stage E: update design doc, user guide, and roadmap status.
- [x] (2026-02-23) Stage F: final quality gates passed (`make check-fmt`,
      `make lint`, `make test`, `make markdownlint`, `make nixie`).

## Surprises & discoveries

- Observation: `docs/rstest-bdd-design.md` §2.7.3 currently states that macros
  cannot evaluate `AttributePolicy::test_attributes()` for arbitrary user types
  and therefore emit only `#[rstest::rstest]` when `attributes` is set. Impact:
  9.3.4 intentionally supersedes this behaviour and requires explicit
  design-doc updates.

- Observation: project-memory MCP resources are unavailable in this
  environment (`list_mcp_resources` and `list_mcp_resource_templates` returned
  no entries). Impact: planning relies on repository docs and source inspection
  only.

- Observation: Tokio's test attribute rejects synchronous function signatures
  with the compiler error "the `async` keyword is missing from the function
  declaration". Impact: codegen must omit `tokio::test` for synchronous
  scenarios (including harness-delegated functions) even when Tokio policy is
  selected.

## Decision log

- Decision: include a mandatory de-risk stage before editing production logic,
  because 9.3.4 touches a known proc-macro limitation from 9.2.1. Rationale:
  avoid partial rewrites before confirming the technically viable
  attribute-emission path. Date/Author: 2026-02-23 / Codex.

- Decision: keep unit and behavioural coverage as separate acceptance layers.
  Rationale: token-level correctness alone is insufficient; end-user macro
  behaviour must be exercised via integration/trybuild tests. Date/Author:
  2026-02-23 / Codex.

- Decision: resolve policy-backed attributes using a macro-local
  `ResolvedAttributePolicy` layer (path-based), not direct execution of
  arbitrary user policy methods. Rationale: Rust procedural macros cannot
  evaluate unknown user type methods at expansion time; path-based resolution
  allows 9.3.4 delivery without introducing framework dependencies into core
  crates. Date/Author: 2026-02-23 / Codex.

## Outcomes & retrospective

Delivered in 9.3.4:

- Added a dedicated codegen policy-resolution module
  (`crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs`) that maps
  resolved policies to emitted test attributes.
- Updated scenario codegen to use policy-backed attribute emission for both
  regular and outline paths.
- Introduced async-safety filtering so `tokio::test` is omitted for synchronous
  signatures (including harness-delegated scenarios), preventing invalid
  expansion.
- Added unit coverage for:
  - Tokio-policy emission in async mode,
  - default/unknown policy fallback,
  - synchronous omission of Tokio test attributes.
- Added behavioural coverage with a new trybuild compile-pass fixture:
  `crates/rstest-bdd/tests/fixtures_macros/scenario_attributes_tokio.rs`,
  registered in `crates/rstest-bdd/tests/trybuild_macros.rs`.
- Updated documentation:
  `docs/rstest-bdd-design.md`, `docs/users-guide.md`, and marked
  `docs/roadmap.md` item 9.3.4 done.

Validation summary:

- `make check-fmt` passed.
- `make lint` passed.
- `make test` passed (1190 Rust tests run, 1 skipped; 47 Python tests passed).
- `make markdownlint` passed.
- `make nixie` passed.

Follow-on considerations:

- Path-based resolution remains a pragmatic compromise for proc-macro limits.
  Supporting arbitrary third-party policy evaluation at expansion time remains
  constrained by Rust proc-macro type-resolution boundaries.

## Context and orientation

Primary code areas for 9.3.4:

- `crates/rstest-bdd-macros/src/codegen/scenario.rs`
  - `generate_test_attrs`
  - `generate_scenario_code`
  - `generate_regular_scenario_code`
  - `generate_outline_scenario_code`
- `crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs`
  - `assemble_test_tokens_with_context`
  - non-harness `assemble_test_tokens` path
- `crates/rstest-bdd-macros/src/codegen/scenario/runtime/harness.rs`
  - `assemble_test_tokens_with_harness`
- `crates/rstest-bdd-harness/src/policy.rs`
  - `AttributePolicy`, `DefaultAttributePolicy`, `TestAttribute`
- `crates/rstest-bdd-harness-tokio/src/policy.rs`
  - `TokioAttributePolicy`

Primary tests likely to change:

- `crates/rstest-bdd-macros/src/codegen/scenario/tests.rs`
- `crates/rstest-bdd-macros/src/codegen/scenario/runtime/tests.rs`
- `crates/rstest-bdd/tests/scenario_harness_tokio.rs`
- `crates/rstest-bdd/tests/trybuild_macros.rs`
- `crates/rstest-bdd/tests/fixtures_macros/*` (if new compile fixtures are
  needed)

Documentation to update:

- `docs/rstest-bdd-design.md` (§2.7.2, §2.7.3)
- `docs/users-guide.md` (harness/attributes section)
- `docs/roadmap.md` (9.3.4 checkbox)

Reference documents reviewed while drafting this plan:

- `docs/rstest-bdd-design.md`
- `docs/rstest-bdd-language-server-design.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/gherkin-syntax.md`

## Plan of work

### Stage A: de-risk and lock policy-emission strategy

Goal: confirm how 9.3.4 will satisfy roadmap intent without violating ADR-005
or proc-macro constraints.

Implementation details:

- Trace the current attribute path through `generate_test_attrs`,
  `assemble_test_tokens_with_context`, and `assemble_test_tokens_with_harness`.
- Prototype the minimal viable mechanism that allows policy-driven attributes
  to be emitted on generated functions in both harness and non-harness paths.
- Decide and document precedence rules:
  - explicit `attributes = ...` policy,
  - inferred/default policy,
  - `RuntimeMode` compatibility behaviour,
  - duplicate `tokio::test` prevention.

Go/no-go validation:

- A single, documented strategy exists that can satisfy the finish line and is
  testable.

### Stage B: implement policy resolution and attribute token synthesis

Goal: produce attribute token streams from the resolved policy in a shared,
reusable location.

Implementation details:

- Add a focused helper (or small module) to resolve the active policy for code
  generation.
- Add a converter from `TestAttribute` semantics to `syn::Attribute`/
  `TokenStream2` emitted on the generated test function.
- Keep the logic small and explicit to avoid new complexity anti-patterns.

Go/no-go validation:

- Unit tests prove token generation for:
  - default policy (`#[rstest::rstest]`),
  - Tokio policy (includes `#[tokio::test(flavor = "current_thread")]`),
  - precedence and de-duplication rules.

### Stage C: wire both generation paths

Goal: ensure harness and non-harness codegen paths consume the same resolved
policy output.

Implementation details:

- Wire the policy-derived attributes through the harness path
  (`assemble_test_tokens_with_harness`) and the non-harness path.
- Remove or refactor obsolete runtime-only attribute branching once policy
  wiring is authoritative.
- Preserve existing compile-time trait assertions for invalid policies.

Go/no-go validation:

- Macro unit tests show both paths emit the same expected attributes for the
  same resolved policy.

### Stage D: tests (unit + behavioural)

Goal: prove the new behaviour at both token-generation and user-facing levels.

Implementation details:

- Unit tests in macro codegen:
  - verify emitted tokens include Tokio attribute when
    `TokioAttributePolicy` is selected,
  - verify existing sync/default cases are unchanged,
  - verify no duplicate Tokio attribute when already present.
- Behavioural tests:
  - add or extend integration/trybuild fixtures to prove a scenario using
    `attributes = TokioAttributePolicy` expands and executes correctly,
  - keep existing harness tests passing to guard against regressions.

Go/no-go validation:

- New tests fail before the change and pass after the change.
- Existing behavioural suites remain green.

### Stage E: documentation and roadmap

Goal: make docs reflect delivered behaviour and close roadmap item 9.3.4.

Implementation details:

- Update `docs/rstest-bdd-design.md` §2.7.2 and §2.7.3 to record the delivered
  policy-emission behaviour and any trade-offs.
- Update `docs/users-guide.md` harness/attributes guidance with a concrete
  usage example showing Tokio policy output.
- Mark `docs/roadmap.md` item 9.3.4 as done.

Go/no-go validation:

- Docs no longer claim that `attributes` always emits only `#[rstest::rstest]`.

### Stage F: full quality gates

Goal: verify repository health before closing the task.

Implementation details:

- Run required gates with `set -o pipefail` and `tee` logs.
- Review logs for errors/warnings and rerun after fixes as needed.

Go/no-go validation:

- `make check-fmt` exits 0.
- `make lint` exits 0.
- `make test` exits 0.

## Concrete execution steps

Run from repository root (`/home/user/project`).

1. Optional focused test loop while iterating:

    set -o pipefail
    cargo test -p rstest-bdd-macros 2>&1 | tee /tmp/9-3-4-macros-test.log

2. Behavioural loop while iterating:

    set -o pipefail
    cargo test -p rstest-bdd --test scenario_harness_tokio 2>&1 | tee \
      /tmp/9-3-4-scenario-harness-tokio.log
    set -o pipefail
    cargo test -p rstest-bdd --test trybuild_macros 2>&1 | tee \
      /tmp/9-3-4-trybuild.log

3. Final required gates:

    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/9-3-4-check-fmt.log
    set -o pipefail
    make lint 2>&1 | tee /tmp/9-3-4-lint.log
    set -o pipefail
    make test 2>&1 | tee /tmp/9-3-4-test.log

## Validation and acceptance

9.3.4 is complete only when all of the following are true:

- A scenario configured with
  `attributes = rstest_bdd_harness_tokio::TokioAttributePolicy` emits
  `#[tokio::test(flavor = "current_thread")]` in expanded macro output.
- Unit tests cover policy attribute emission and precedence logic.
- Behavioural tests validate user-facing macro behaviour remains correct.
- Existing tests continue to pass.
- `docs/rstest-bdd-design.md` and `docs/users-guide.md` are updated.
- `docs/roadmap.md` marks 9.3.4 as done.
- `make check-fmt`, `make lint`, and `make test` all succeed.

## Idempotence and recovery

- All steps are safe to re-run.
- If a gate fails, fix the issue and re-run the same command until clean.
- If Stage A cannot produce a viable approach without violating a tolerance,
  stop, capture evidence, and escalate with options before proceeding.

## Revision note

- 2026-02-23: Initial draft created for roadmap item 9.3.4.
