# ExecPlan 9.2.3: Treat `runtime = "tokio-current-thread"` as a compatibility alias

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

`PLANS.md` is not present in the repository at the time of writing, so this
ExecPlan is the governing plan for this task.

## Purpose / big picture

Phase 8 introduced async scenario execution via
`runtime = "tokio-current-thread"`, while phase 9 introduces harness adapters
and attribute policy plug-ins per ADR-005. This task bridges those models:
`runtime = "tokio-current-thread"` should be treated as a compatibility alias
for the Tokio harness-adapter path, so existing users keep working while the
new harness-oriented architecture becomes the canonical model.

After this change:

- Existing `scenarios!(..., runtime = "tokio-current-thread")` usage continues
  to compile and run unchanged.
- Macro internals represent that runtime choice as a compatibility alias for
  Tokio harness selection, rather than as an independent long-term execution
  model.
- Unit tests validate the alias-resolution logic and guard against regression.
- Behavioural tests validate end-user behaviour (async scenarios and macro
  fixtures) remains correct.
- Design and user documentation explicitly state the compatibility relationship
  between `runtime` and harness adapters.
- Roadmap entry 9.2.3 is marked done.

## Constraints

- Scope this implementation to roadmap item 9.2.3 only. Do not implement phase
  9.3 adapter crate delivery in this change.
- Preserve user-visible behaviour for existing
  `runtime = "tokio-current-thread"` scenarios.
- Keep Tokio and Graphical Processing User Interface (GPUI) dependencies out
  of core crates (`rstest-bdd`,
  `rstest-bdd-macros`, `rstest-bdd-harness`) per ADR-005.
- Avoid public API breakage in `rstest-bdd`, `rstest-bdd-macros`, and
  `rstest-bdd-harness`.
- Keep files under 400 lines; split modules/tests when required.
- Record design decisions in `docs/rstest-bdd-design.md`.
- Record user-facing usage in `docs/users-guide.md`.
- Mark roadmap entry 9.2.3 done in `docs/roadmap.md` only when implementation
  is complete and validated.
- Run quality gates and require success:
  `make check-fmt`, `make lint`, and `make test`.

## Tolerances (exception triggers)

- Scope: if implementation exceeds 15 changed files or 900 net lines, stop and
  escalate.
- Interfaces: if any existing public API must be removed or changed
  incompatibly, stop and escalate.
- Dependencies: if a new external dependency is required in core crates, stop
  and escalate.
- Behaviour: if existing async scenario behaviour regresses, stop and escalate
  rather than weakening tests.
- Iterations: if the same gate (`check-fmt`, `lint`, or `test`) fails three
  times after fixes, stop and escalate with logs.
- Ambiguity: if alias semantics conflict with ADR-005 or existing docs, stop
  and request direction before coding further.

## Risks

- Risk: alias semantics are underspecified relative to phase 9.3 (full Tokio
  adapter crate), causing accidental scope creep. Severity: high. Likelihood: medium.
  Mitigation: confine this task to compatibility normalization and
  preserve current runtime behaviour; defer full adapter extraction to 9.3.

- Risk: changing argument-resolution flow in `scenarios!` could accidentally
  break interactions with `harness` and `attributes` parameters. Severity:
  medium. Likelihood: medium. Mitigation: add focused unit tests covering all
  parameter combinations and existing parser diagnostics.

- Risk: trybuild snapshots may change due to diagnostic wording updates.
  Severity: low. Likelihood: medium. Mitigation: update only impacted fixtures
  and verify no unrelated snapshot churn.

- Risk: documentation may drift between design doc, user guide, and roadmap.
  Severity: medium. Likelihood: medium. Mitigation: update all three in one
  milestone and verify links/wording.

## Progress

- [x] (2026-02-17 16:47Z) Retrieved repository context and reviewed roadmap,
      ADR-005, prior phase ExecPlans, and current macro/runtime code paths.
- [x] (2026-02-17 16:53Z) Drafted this ExecPlan for phase 9.2.3.
- [x] (2026-02-17) Stage A: confirmed baseline behaviour and added targeted
      parser/codegen tests for runtime alias resolution.
- [x] (2026-02-17) Stage B: implemented macro-level compatibility alias
      canonicalization in runtime-derived macro/test-generation paths.
- [x] (2026-02-17) Stage C: expanded unit coverage for parser/runtime alias
      semantics and harness-resolution interaction.
- [x] (2026-02-17) Stage D: added behavioural coverage in
      `crates/rstest-bdd/tests/runtime_compat_alias.rs` with feature fixture
      `crates/rstest-bdd/tests/features/runtime_compat_alias.feature`.
- [x] (2026-02-17) Stage E: updated `docs/rstest-bdd-design.md`,
      `docs/users-guide.md`, and marked roadmap entry 9.2.3 done in
      `docs/roadmap.md`.
- [x] (2026-02-17) Stage F: final quality gates passed with log capture:
      `make check-fmt`, `make lint`, `make test`; documentation gates
      `make markdownlint` and `make nixie` also passed.

## Surprises & discoveries

- Observation: project-memory Model Context Protocol (MCP) resources
  (including qdrant-backed notes) are unavailable in this environment.
  Evidence: `list_mcp_resources` and
  `list_mcp_resource_templates` returned no entries. Impact: planning relied on
  repository documents and code inspection only.

- Observation: current macro code deliberately rejects `harness` combined with
  async scenario generation, which is directly adjacent to 9.2.3 alias work.
  Evidence: `generate_regular_scenario_code()` and
  `generate_outline_scenario_code()` emit a compile error when
  `config.harness.is_some() && config.runtime.is_async()`. Impact: the alias
  implementation must avoid accidental activation of the async+harness
  rejection path for existing runtime compatibility users.

## Decision log

- Decision: implement 9.2.3 as a compatibility-normalization change in macro
  argument resolution, preserving existing runtime behaviour. Rationale: this
  satisfies roadmap compatibility intent without pulling phase 9.3 adapter
  extraction into the same change. Date/Author: 2026-02-17 / Codex.

- Decision: treat alias semantics as an internal canonical form in
  `scenarios!` generation, then keep generated observable behaviour unchanged
  for legacy runtime users. Rationale: users get stability; internals move
  toward ADR-005 terminology and architecture. Date/Author: 2026-02-17 / Codex.

- Decision: require both unit and behavioural coverage for alias changes.
  Rationale: parser-only tests are insufficient; end-user async
  scenario execution is unaffected. Date/Author: 2026-02-17 / Codex.

## Outcomes & retrospective

Implemented runtime compatibility alias canonicalization for
`runtime = "tokio-current-thread"` without changing current runtime execution
behaviour.

What changed:

- Added `RuntimeCompatibilityAlias::TokioHarnessAdapter` and
  `RuntimeMode::compatibility_alias()` for runtime-derived alias resolution.
- Added `resolve_harness_path()` in scenario test generation so explicit
  `harness` remains authoritative while runtime alias remains
  compatibility-only until phase 9.3.
- Added unit coverage for alias parsing and harness-resolution behaviour.
- Added behavioural coverage for async scenario execution under runtime alias.
- Updated design and user documentation, and marked roadmap item 9.2.3 done.

Validation summary:

- `make check-fmt` passed.
- `make lint` passed.
- `make test` passed (1171 Rust tests, 47 Python tests).
- `make markdownlint` passed.
- `make nixie` passed.

Success criteria at completion:

- [x] Alias-resolution logic is implemented and tested.
- [x] Existing runtime-tokio behaviour is preserved.
- [x] Documentation and roadmap updates are complete.
- [x] All requested quality gates pass.

## Context and orientation

The relevant implementation surface is concentrated in macro parsing and
scenario test generation:

- `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/mod.rs`
  parses `scenarios!` arguments, including `runtime`, `harness`, and
  `attributes`.
- `crates/rstest-bdd-macros/src/macros/scenarios/mod.rs`
  threads parsed arguments into `ScenarioTestContext` for generated tests.
- `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs`
  uses `RuntimeMode` to select async/sync test signatures.
- `crates/rstest-bdd-macros/src/codegen/scenario.rs`
  controls generated test attributes and harness/attribute assertions.
- Behavioural coverage currently lives in:
  - `crates/rstest-bdd/tests/async_scenario.rs`
  - `crates/rstest-bdd/tests/trybuild_macros.rs`
  - `crates/rstest-bdd/tests/fixtures_macros/`

Documentation to update:

- `docs/rstest-bdd-design.md`
- `docs/users-guide.md`
- `docs/roadmap.md`

Reference documents reviewed for this plan:

- `docs/rstest-bdd-design.md`
- `docs/rstest-bdd-language-server-design.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/gherkin-syntax.md`

## Plan of work

### Stage A: Baseline and test-first guardrails

Capture baseline behaviour before edits and add focused tests that express the
compatibility alias requirement.

Implementation details:

- Run targeted tests proving current async runtime behaviour remains green.
- Add or adjust unit tests in
  `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/tests.rs` to define
  expected alias-resolution semantics.
- Where useful, add codegen-level unit checks in
  `crates/rstest-bdd-macros/src/codegen/scenario/tests.rs` to ensure attribute
  generation remains compatible for runtime-based async scenarios.

Go/no-go validation:

- New/updated unit tests fail before implementation (or clearly assert pending
  behaviour), then pass after stage B.

### Stage B: Implement compatibility alias resolution

Introduce a small internal resolution layer for `scenarios!` arguments that
canonicalizes `runtime = "tokio-current-thread"` as the Tokio compatibility
alias path while preserving current generated behaviour.

Implementation details:

- Update
  `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/mod.rs` with a
  clearly named internal representation/helper for compatibility alias
  resolution.
- Thread resolved values through
  `crates/rstest-bdd-macros/src/macros/scenarios/mod.rs` and
  `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs` with
  minimal behavioural delta.
- Keep parser diagnostics clear and stable; if diagnostics change, update
  snapshots only where necessary.

Go/no-go validation:

- Existing async runtime tests still pass.
- No new dependency is introduced.

### Stage C: Unit-test hardening

Expand unit coverage around alias resolution and parameter interplay.

Implementation details:

- Add table-driven `rstest` cases in
  `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/tests.rs` for:
  - runtime-only alias use,
  - runtime plus other non-extension args,
  - extension-parameter interplay (`harness`, `attributes`),
  - duplicate/invalid runtime diagnostics.
- Add any required codegen unit assertions in
  `crates/rstest-bdd-macros/src/codegen/scenario/tests.rs` so the resulting
  async attribute behaviour stays explicit.

Go/no-go validation:

- `cargo test -p rstest-bdd-macros` passes.

### Stage D: Behavioural validation

Prove end-user behaviour from generated macros remains correct.

Implementation details:

- Reuse/update `crates/rstest-bdd/tests/async_scenario.rs` as the primary
  behavioural test for runtime compatibility.
- Add or extend trybuild fixtures in `crates/rstest-bdd/tests/fixtures_macros/`
  only when needed to verify compile-time compatibility/diagnostics.
- Register any new fixture files in
  `crates/rstest-bdd/tests/trybuild_macros.rs`.

Go/no-go validation:

- `cargo test -p rstest-bdd --test async_scenario` passes.
- `cargo test -p rstest-bdd --test trybuild_macros` passes.

### Stage E: Documentation and roadmap

Record the design choice and user guidance, then mark roadmap completion.

Implementation details:

- Update `docs/rstest-bdd-design.md` section 2.7.x to document the implemented
  compatibility-alias semantics and boundaries relative to phase 9.3.
- Update `docs/users-guide.md` runtime/harness sections to describe
  `runtime = "tokio-current-thread"` as compatibility syntax and indicate
  preferred harness terminology.
- Mark `docs/roadmap.md` entry `9.2.3` as done.

Go/no-go validation:

- Documentation reflects the implemented behaviour exactly and does not claim
  unshipped phase 9.3 functionality.

### Stage F: Final quality gates

Run required project gates and capture logs.

Implementation details:

- Run `make check-fmt`, `make lint`, and `make test` from repo root with
  `set -o pipefail` and `tee` logs.
- Review logs for warnings/errors before finalizing.

Go/no-go validation:

- All three commands exit 0.

## Concrete steps

All commands run from repository root (`/home/user/project`).

1. Baseline and targeted tests.

   ```bash
   set -o pipefail && cargo test -p rstest-bdd-macros \
     2>&1 | tee /tmp/9-2-3-baseline-macros-test.log
   set -o pipefail && cargo test -p rstest-bdd --test async_scenario \
     2>&1 | tee /tmp/9-2-3-baseline-async-scenario.log
   ```

2. Implement stages B-E edits in the files listed above.

3. Behavioural fixture verification.

   ```bash
   set -o pipefail && cargo test -p rstest-bdd --test trybuild_macros \
     2>&1 | tee /tmp/9-2-3-trybuild.log
   ```

4. Final required quality gates.

   ```bash
   set -o pipefail && make check-fmt \
     2>&1 | tee /tmp/9-2-3-check-fmt.log
   set -o pipefail && make lint \
     2>&1 | tee /tmp/9-2-3-lint.log
   set -o pipefail && make test \
     2>&1 | tee /tmp/9-2-3-test.log
   ```

## Validation and acceptance

Acceptance is behaviour-focused:

- `scenarios!(..., runtime = "tokio-current-thread")` remains fully usable for
  async scenario execution.
- Macro internals contain explicit compatibility-alias handling for this
  runtime value.
- Unit tests validate alias parsing/resolution and guard diagnostics.
- Behavioural tests validate runtime compatibility in generated tests.
- `docs/rstest-bdd-design.md` and `docs/users-guide.md` document the alias
  relationship and current boundaries.
- `docs/roadmap.md` marks 9.2.3 done.
- `make check-fmt`, `make lint`, and `make test` all pass.

## Idempotence and recovery

- All steps are safe to re-run.
- If trybuild snapshots change unexpectedly, inspect the diff and keep only
  changes directly attributable to alias semantics.
- If a gate fails, fix the issue and rerun only the failed command first, then
  rerun the full gate set.

## Artifacts and notes

Expected logs:

- `/tmp/9-2-3-baseline-macros-test.log`
- `/tmp/9-2-3-baseline-async-scenario.log`
- `/tmp/9-2-3-trybuild.log`
- `/tmp/9-2-3-check-fmt.log`
- `/tmp/9-2-3-lint.log`
- `/tmp/9-2-3-test.log`

## Interfaces and dependencies

No new external dependencies are expected.

Likely touched interfaces are internal to macro crates:

- `ScenariosArgs` parsing and internal resolution helpers in
  `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/mod.rs`.
- Scenario test generation flow in
  `crates/rstest-bdd-macros/src/macros/scenarios/mod.rs` and
  `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs`.

Public API expectations:

- No breaking changes to exported APIs in `rstest-bdd`, `rstest-bdd-macros`,
  or `rstest-bdd-harness`.
- Compatibility syntax `runtime = "tokio-current-thread"` remains supported.

## Revision note

Initial draft created for roadmap item 9.2.3. This draft resolves scope by
implementing runtime compatibility alias semantics without pulling phase 9.3
adapter crate delivery into the same change.
