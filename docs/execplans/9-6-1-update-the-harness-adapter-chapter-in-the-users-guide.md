# ExecPlan 9.6.1: update the harness adapter chapter in the user guide

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE (2026-03-22)

`PLANS.md` is not present in this repository at the time of writing, so this
ExecPlan is the governing plan for roadmap item 9.6.1.

## Purpose / big picture

Roadmap item 9.6.1 closes the documentation gap left after phases 9.3, 9.4,
and 9.5 delivered the harness adapter core, Tokio and Graphical Processing
User Interface (GPUI) opt-in crates, path-based attribute-policy wiring, and
`HarnessAdapter::Context` injection.
The current harness chapter in the user guide and the corresponding design-doc
sections already mention parts of that work, but the narrative is split across
multiple passages and does not yet present the delivered model as one coherent
story.

After this work:

- `docs/users-guide.md` presents a complete harness-adapter chapter covering
  harness selection, attribute-policy selection, path-based policy resolution,
  compatibility-alias behaviour, and `Context`-based fixture injection.
- `docs/rstest-bdd-design.md` records the delivered architecture and any
  design decisions taken while tightening the documentation around
  Architectural Decision Record (ADR) 005, ADR-007, and the 9.3.4 codegen
  trust model.
- Unit tests and behavioural tests validate any examples, emitted attributes,
  and end-to-end documentation claims introduced while updating the docs.
- `docs/roadmap.md` marks 9.6.1 done only after the documentation, tests, and
  required quality gates all pass.

Success is observable when a reader can follow one authoritative chapter in
the user guide from `harness = ...` and `attributes = ...` configuration
through to context injection and framework-specific examples, and when the
quality gates pass: `make check-fmt`, `make lint`, and `make test`.

## Constraints

- Implement roadmap item 9.6.1 only. Do not fold 9.6.2 or 9.6.3 into this
  change, except to note future work where the docs need to point forward.
- Treat 9.5.3 as delivered and document the current `HarnessAdapter::Context`
  contract, not speculative alternatives.
- Keep ADR-005 boundaries intact: framework-specific details belong in opt-in
  crates and documentation must not imply that Tokio or GPUI are built into
  the core runtime.
- Preserve the documented 9.3.4 trust model: attribute-policy resolution is
  path-based during macro expansion, not arbitrary trait-method execution on
  user types.
- Update `docs/users-guide.md` with concrete usage guidance for both first-
  party harnesses and third-party harness authors.
- Update `docs/rstest-bdd-design.md` with any design decisions taken while
  reconciling ADR-005, ADR-007, and the delivered implementation.
- Mark roadmap entry 9.6.1 done only after documentation, tests, and gates all
  pass.
- Because this change edits Markdown, run the documentation gates as well:
  `make fmt`, `make markdownlint`, and `make nixie`.
- Run validation commands with `set -o pipefail` and `tee` so failures are not
  hidden by truncated output.

## Tolerances (exception triggers)

- Scope: if delivering 9.6.1 requires more than 10 files changed or more than
  500 net lines, stop and re-check whether the work is drifting into 9.6.2 or
  9.6.3.
- Behaviour: if a documentation-backed behaviour claim cannot be proven with
  existing or modest new tests, stop and tighten the claim instead of
  documenting an unverified contract.
- Interfaces: if clarifying the docs reveals an API inconsistency that requires
  public Rust API changes, stop and split that into a separate implementation
  task.
- Validation: if `make check-fmt`, `make lint`, or `make test` fails for
  unrelated reasons, capture logs and stop before marking the roadmap item
  done.
- Iterations: if the same gate fails three consecutive fix attempts, stop and
  escalate with the recorded log path.
- Ambiguity: if the user guide, design doc, ADRs, and implementation disagree
  on canonical harness usage, stop and record the conflicting sources before
  editing narrative text.

## Risks

- Risk: the user guide may over-promise support for arbitrary third-party
  `AttributePolicy` evaluation even though macro expansion still relies on
  canonical path mapping. Severity: high. Likelihood: medium. Mitigation:
  document the trust model explicitly and ensure examples use supported
  canonical paths or clearly labelled custom-policy guidance.

- Risk: the design doc and user guide may duplicate the same low-level details
  and drift again. Severity: medium. Likelihood: medium. Mitigation: define a
  sharper split, with the user guide focused on usage and the design doc on
  architecture and rationale.

- Risk: examples for harness context injection may compile in docs but miss the
  reserved fixture-key convention (`rstest_bdd_harness_context`). Severity:
  medium. Likelihood: medium. Mitigation: align examples with the delivered
  `#[from(rstest_bdd_harness_context)]` pattern and add test coverage if any
  new example is non-trivial.

- Risk: roadmap item 9.6.1 may be marked done even if the docs still do not
  mention GPUI or the compatibility alias. Severity: medium. Likelihood:
  medium. Mitigation: use an explicit acceptance checklist tied to the roadmap
  wording before updating the checkbox.

## Progress

- [x] (2026-03-18) Reviewed roadmap item 9.6.1, its 9.3.4 and 9.5.3
      prerequisites, and the current harness documentation surface.
- [x] (2026-03-18) Reviewed existing execplans for 9.3.4, 9.4.1, and 9.4.2 to
      match repository planning conventions.
- [x] (2026-03-18) Drafted this ExecPlan.
- [x] (2026-03-22) Stage A: documentation inventory and gap analysis.
- [x] (2026-03-22) Stage B: tighten design-document architecture and decision
      record.
- [x] (2026-03-22) Stage C: rewrite the user-guide harness chapter and
      examples.
- [x] (2026-03-22) Stage D: confirm documented behaviour against the existing
      unit and behavioural test surface.
- [x] (2026-03-22) Stage E: run required Rust quality gates.
- [x] (2026-03-22) Stage F: mark roadmap item 9.6.1 done and capture
      outcomes.

## Surprises & Discoveries

- Observation: the harness-related material already exists in both
  `docs/users-guide.md` and `docs/rstest-bdd-design.md`, but it is spread
  across multiple non-adjacent sections rather than one clearly delivered
  chapter. Impact: 9.6.1 should primarily restructure and tighten existing
  content, with only limited new prose.

- Observation: project-memory Model Context Protocol (MCP) resources are
  unavailable in this environment (`list_mcp_resources` and
  `list_mcp_resource_templates` returned no entries). Impact: this plan relies
  on repository documents and source inspection only.

- Observation: the current user guide already mentions path-based resolution
  and `Context`, but it does not yet tell a reader which behaviour is
  compatibility legacy (`runtime = "tokio-current-thread"`) versus canonical
  usage (`harness = ...`, `attributes = ...`). Impact: the rewrite needs a
  clearer progression from preferred configuration to compatibility notes.

## Decision Log

- Decision: treat 9.6.1 as documentation plus validation, not prose-only.
  Rationale: the task explicitly requires unit tests and behavioural tests to
  validate the feature, and the roadmap item should not be checked off on
  narrative updates alone. Date/Author: 2026-03-18 / Codex.

- Decision: keep the main narrative centred in `docs/users-guide.md` and use
  `docs/rstest-bdd-design.md` for architecture, trade-offs, and trust-model
  rationale. Rationale: this minimizes future drift by giving each document a
  distinct purpose. Date/Author: 2026-03-18 / Codex.

- Decision: preserve explicit mention of the canonical-path trust model for
  attribute policies until the implementation changes. Rationale: hiding that
  limitation would make the docs inaccurate and would mislead third-party
  harness authors. Date/Author: 2026-03-18 / Codex.

- Decision: use existing harness and policy test coverage as the validation
  surface for 9.6.1 instead of adding new documentation-only tests. Rationale:
  the delivered behaviour is already covered by focused unit, behavioural, and
  integration suites; this milestone is a documentation alignment pass rather
  than a new runtime feature. Date/Author: 2026-03-22 / Codex.

## Outcomes & Retrospective

Delivered outcomes for 9.6.1:

- Reworked the harness-adapter guidance in `docs/users-guide.md` so it now
  presents explicit `harness = ...` and `attributes = ...` configuration as
  the canonical path, documents the Tokio compatibility alias as deprecated
  legacy syntax, and calls out the current first-party policy-resolution trust
  model for Tokio and GPUI.
- Updated `docs/users-guide.md` custom-harness material to explain
  `HarnessAdapter::Context`, `rstest_bdd_harness_context`, and the current
  limitation for third-party attribute-policy paths during macro expansion.
- Updated `docs/rstest-bdd-design.md` §2.7 and §3.12 so the design document
  reflects the delivered architecture, validation layers, and the distinction
  between canonical configuration and legacy compatibility syntax.
- Marked `docs/roadmap.md` item 9.6.1 complete.
- Validated the documented behaviour against the existing unit, behavioural,
  and integration suites that already cover:
  - policy-path resolution,
  - `StdHarness`, `TokioHarness`, and `GpuiHarness` contracts,
  - `rstest_bdd_harness_context` injection,
  - Tokio compatibility alias behaviour.

Validation summary:

- `make check-fmt` passed.
- `make lint` passed.
- `make test` passed.

Retrospective:

- The main gap was not missing content but fragmented content. Consolidating
  the user-facing guidance and explicitly distinguishing first-party canonical
  policy mapping from legacy compatibility behaviour removed the ambiguity.
- Existing tests already covered the documented runtime semantics, so no new
  test code was required for this documentation-alignment milestone.

## Context and orientation

Primary documentation targets:

- `docs/users-guide.md`
  - harness adapter and attribute policy chapter
  - custom harness author guidance
  - first-party Tokio and GPUI usage examples
- `docs/rstest-bdd-design.md`
  - §2.7 Harness adapters and attribute policy plugins
  - any status appendix entries that still summarize phase 9 behaviour too
    loosely
- `docs/roadmap.md`
  - mark 9.6.1 done after validation passes

Primary implementation and test references to verify against while editing:

- `docs/adr-005-harness-adapter-crates-for-framework-specific-test-integration.md`
- `docs/adr-007-harness-context-injection.md`
- `crates/rstest-bdd-harness/src/adapter.rs`
- `crates/rstest-bdd-harness/src/policy.rs`
- `crates/rstest-bdd-harness/src/runner.rs`
- `crates/rstest-bdd-harness/src/std_harness.rs`
- `crates/rstest-bdd-harness-tokio/src/tokio_harness.rs`
- `crates/rstest-bdd-harness-tokio/src/policy.rs`
- `crates/rstest-bdd-harness-gpui/src/gpui_harness.rs`
- `crates/rstest-bdd-harness-gpui/src/policy.rs`
- `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs`
- `crates/rstest-bdd/tests/scenario_harness_tokio.rs`
- `crates/rstest-bdd/tests/scenario_harness_gpui.rs`
- any trybuild fixtures covering harness and attribute-policy diagnostics

Reference documents reviewed while drafting this plan:

- `docs/roadmap.md`
- `docs/rstest-bdd-design.md`
- `docs/rstest-bdd-language-server-design.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/gherkin-syntax.md`
- `docs/adr-007-harness-context-injection.md`
- `docs/execplans/9-3-4-wire-test-attributes-into-codegen.md`
- `docs/execplans/9.4.1-fixture-injection-mechanism.md`
- `docs/execplans/9-4-2-create-rstest-bdd-harness-gpui.md`

## Plan of work

### Stage A: documentation inventory and gap analysis

Goal: identify exactly which statements in the current docs are stale,
duplicated, or incomplete relative to delivered 9.3.4, 9.4.x, and 9.5.x work.

Implementation details:

- Re-read the harness sections in `docs/users-guide.md` and
  `docs/rstest-bdd-design.md` side by side.
- Build a checklist from the roadmap wording:
  - delivered 9.3 outcomes,
  - attribute-policy wiring from 9.3.4,
  - context injection from 9.5.
- Cross-check each claim against the current code and behavioural tests so the
  rewrite stays implementation-first.
- Decide which material belongs in the user guide versus the design doc.

Go/no-go validation:

- A concrete edit list exists, tied to file sections and verified behaviour.

### Stage B: tighten design-document architecture and decision record

Goal: make the design doc accurately describe the delivered harness/plugin
architecture and the rationale behind its current limitations.

Implementation details:

- Update `docs/rstest-bdd-design.md` §2.7 so it describes:
  - `HarnessAdapter::Context`,
  - `ScenarioRunRequest<'_, C, T>` and runner context handoff,
  - path-based first-party attribute-policy resolution,
  - first-party Tokio and GPUI plugin crates as opt-in extensions.
- Remove or rewrite stale speculative wording that predates 9.3.4 or ADR-007.
- Record any new documentation-level design decision needed to keep the doc
  stable, such as the split between usage guidance and architectural detail.

Go/no-go validation:

- The design doc can be read as an accurate architectural reference without
  contradicting ADR-005, ADR-007, or the current implementation.

### Stage C: rewrite the user-guide harness chapter and examples

Goal: give users one coherent guide for selecting a harness, selecting an
attribute policy, and consuming harness context inside steps.

Implementation details:

- Consolidate the harness adapter chapter in `docs/users-guide.md` so it walks
  from the default `StdHarness` model to Tokio and GPUI opt-in crates.
- Show the preferred explicit configuration using `harness = ...` and
  `attributes = ...`, then explain the deprecated
  `runtime = "tokio-current-thread"` compatibility alias separately.
- Document the reserved harness context fixture key and the
  `#[from(rstest_bdd_harness_context)]` pattern for step injection.
- Clarify what custom harness authors must implement:
  - `Default`,
  - `HarnessAdapter`,
  - `type Context`,
  - request execution via `request.run(context)`,
  - optional attribute-policy crate and `Cargo.toml` wiring.
- Ensure examples use real crate paths and compile-friendly signatures.

Go/no-go validation:

- A reader can configure first-party harnesses and understand how to build a
  third-party harness without reading the design doc first.

### Stage D: add or adjust unit and behavioural validation

Goal: validate every non-trivial documented claim introduced by the rewrite.

Implementation details:

- Review existing unit, integration, and behavioural tests for coverage of:
  - Tokio policy attribute emission,
  - GPUI policy attribute emission,
  - `StdHarness` baseline guarantees,
  - harness context injection through the reserved fixture key,
  - compatibility alias behaviour.
- Add only the missing tests required to support the rewritten docs. Likely
  targets include doc-adjacent integration tests in `crates/rstest-bdd/tests`
  or focused macro/codegen unit tests if examples expose an uncovered edge.
- Prefer behavioural tests for user-facing guarantees and unit tests for
  path-resolution details.

Go/no-go validation:

- Each new or materially strengthened documentation claim is backed by an
  automated test, or the claim is reduced to match existing coverage.

### Stage E: run documentation and Rust quality gates

Goal: confirm the plan’s documentation and any supporting test changes leave
the repository green.

Implementation details:

- Run `make fmt` after Markdown edits.
- Run `make markdownlint` and `make nixie`.
- Run the required Rust gates with logging:
  - `make check-fmt`
  - `make lint`
  - `make test`
- Use `set -o pipefail` and `tee` for every gate, writing logs under `/tmp/`.

Go/no-go validation:

- All applicable gates exit successfully and their logs are available for
  review.

### Stage F: mark roadmap item done and capture outcomes

Goal: close the roadmap item only after the documentation and validation work
is actually complete.

Implementation details:

- Update `docs/roadmap.md` to mark 9.6.1 done.
- Record the final outcome, validation summary, and any follow-on work that
  remains for 9.6.2 or 9.6.3.
- If the work uncovers a documentation or API gap outside 9.6.1 scope, record
  it explicitly rather than silently folding it into this milestone.

Go/no-go validation:

- The roadmap checkbox changes only in the same change set as the completed
  docs and passing gates.
