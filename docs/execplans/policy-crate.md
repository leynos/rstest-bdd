# Add shared policy crate and ADR

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises and Discoveries`,
`Decision Log`, and `Outcomes and Retrospective` must be kept up to date as
work proceeds.

Status: COMPLETE

## Purpose / big picture

Create a small `rstest-bdd-policy` crate that centralizes runtime policy types
for both the runtime crate and the proc-macro crate, and document the decision
in a minimal architectural decision record (ADR). Success is observable when
the duplicated `RuntimeMode` and `TestAttributeHint` enums are removed from the
macro crate, the runtime crate re-exports or depends on the shared types, and
all tests/lints pass.

## Constraints

- Do not break the public API surface of `rstest_bdd::execution::RuntimeMode`
  and `rstest_bdd::execution::TestAttributeHint`; keep them available to users.
- Proc-macro crates may not depend on the runtime crate; any shared types must
  live in a non-proc-macro crate.
- Every module must begin with a crate/module-level `//!` doc comment.
- All public items require Rustdoc comments to satisfy `missing_docs` lint.
- Markdown must use en-GB spelling and be wrapped at 80 columns.
- Run required Makefile quality gates using `tee` logs before committing.
- Commit only after tests, lint, and formatting checks pass.

## Tolerances (exception triggers)

- Scope: if implementation needs more than 20 files or 800 net lines of code
  (LOC) changes, stop and escalate.
- Interfaces: if a public API signature must change (beyond re-exporting
  existing names), stop and escalate.
- Dependencies: if a new external dependency is required, stop and escalate.
- Tests: if the test suite still fails after two fix attempts, stop and
  escalate with logs.
- Ambiguity: if the ADR conflicts with existing documentation or roadmap,
  stop and ask for direction.

## Risks

- Risk: proc-macro dependency constraints could still prevent reuse.
  Severity: medium Likelihood: low Mitigation: keep `rstest-bdd-policy`
  dependency-free and non-proc-macro.

- Risk: Markdown format rules or linting could fail on ADR updates.
  Severity: low Likelihood: medium Mitigation: run `make fmt`,
  `make markdownlint`, and `make nixie` before committing.

- Risk: public API regressions if re-exports are incomplete.
  Severity: medium Likelihood: low Mitigation: keep `rstest_bdd::execution`
  re-exports and adjust tests to confirm behaviour.

## Progress

- [x] (2026-01-17 01:16Z) Draft ExecPlan and obtain approval.
- [x] (2026-01-17 01:20Z) Run baseline test suite before edits.
- [x] (2026-01-17 01:26Z) Add minimal ADR for the policy crate and update
      ADR-001 if needed.
- [x] (2026-01-17 01:26Z) Add `crates/rstest-bdd-policy` with shared enums and
      docs.
- [x] (2026-01-17 01:26Z) Update runtime and macro crates to use shared policy
      types.
- [x] (2026-01-17 01:26Z) Update tests to validate policy mapping and remove
      duplication.
- [x] (2026-01-17 01:30Z) Run format, lint, and test quality gates.

## Surprises and discoveries

- Observation: `make markdownlint` failed because `markdownlint` was missing.
  Evidence: `xargs: markdownlint: No such file or directory`. Impact: reran the
  gate with `MDLINT=markdownlint-cli2` to complete the markdown lint step.

## Decision log

- Decision: proceed with implementation after plan approval.
  Rationale: explicit user approval granted for Stages A-D. Date/Author:
  2026-01-17 01:20Z / Codex.
- Decision: rerun markdown lint with `MDLINT=markdownlint-cli2`.
  Rationale: the default `markdownlint` binary is unavailable in this
  environment, and the Makefile supports overriding `MDLINT`. Date/Author:
  2026-01-17 01:30Z / Codex.

## Outcomes and retrospective

The policy enums live in `rstest-bdd-policy`, eliminating macro/runtime
duplication while preserving the public `rstest_bdd::execution` API via
re-exports. Documentation now includes ADR-004, and ADR-001 references the new
new policy crate. Running the full quality gate suite confirms the changes
across all outcomes: formatting, lint, and tests.

## Context and orientation

The runtime crate currently defines `RuntimeMode` and `TestAttributeHint` in
`crates/rstest-bdd/src/execution.rs` and the macro crate mirrors them in
`crates/rstest-bdd-macros/src/macros/scenarios/macro_args.rs`. The duplication
was necessary because proc-macro crates cannot depend on runtime crates. The
new policy crate will house those shared types, so both crates can depend on it
without duplication.

Documentation lives in `docs/`. ADRs include `docs/adr-004-policy-crate.md`,
which records moving `RuntimeMode` and `TestAttributeHint` from
`crates/rstest-bdd/src/execution.rs` and the mirrored definitions in
`crates/rstest-bdd-macros/src/macros/scenarios/macro_args.rs` into
`crates/rstest-bdd-policy`. Workspace layout documentation appears in the root
`README.md`, the crate READMEs, and `docs/rstest-bdd-design.md`, and should
list the policy crate alongside the runtime and macro crates.

## Plan of work

Stage A: Baseline and doc alignment. Run the full test suite before any code
changes to establish a baseline. Draft a minimal ADR describing the policy
crate decision and update `docs/adr-001-async-fixtures-and-test.md` if its
runtime policy references to the new shared crate need to be updated.

Stage B: Policy crate implementation. Create a new crate at
`crates/rstest-bdd-policy` with a `Cargo.toml` and `src/lib.rs` that defines
`RuntimeMode` and `TestAttributeHint` with the existing behaviour. Include
module-level documentation and Rustdoc examples. Add unit tests in the new
crate to preserve the behaviour currently asserted in
`crates/rstest-bdd/src/execution/tests.rs`.

Stage C: Integrate runtime and macros. Replace the runtime and macro crate
local enum definitions with imports from `rstest-bdd-policy`. Re-export the
policy types from `rstest_bdd::execution` to keep the public API stable. Update
any tests that previously referenced the local enums.

Stage D: Validation and commit. Run formatting and lint/test gates using the
project Makefile. Commit the changes with a descriptive message once all
quality gates pass.

## Concrete steps

1) Baseline tests (before edits) from the repo root:

   - `make test 2>&1 | tee /tmp/test-$(get-project)-$(git branch --show).out`

2) Documentation updates:

   - Add `docs/adr-004-policy-crate.md` with a minimal ADR.
   - Update `docs/adr-001-async-fixtures-and-test.md` if it mentions the policy
     duplication or trait abstraction in a way that now changes.

3) Policy crate scaffolding:

   - Add `crates/rstest-bdd-policy/Cargo.toml` with workspace metadata and no
     extra dependencies.
   - Add `crates/rstest-bdd-policy/src/lib.rs` with `RuntimeMode` and
     `TestAttributeHint` plus their methods and tests.
   - Add the crate to workspace members and dependencies in the root
     `Cargo.toml`.

4) Integration changes:

   - Update `crates/rstest-bdd/src/execution.rs` to re-export the policy types
     and remove local enum definitions and duplication notes.
   - Update macro crate code and tests to import the shared enums from
     `rstest-bdd-policy`.

5) Format and quality gates (after edits):

   - `make fmt 2>&1 | tee /tmp/fmt-$(get-project)-$(git branch --show).out`
   - `make check-fmt 2>&1 | tee /tmp/check-fmt-$(get-project)-$(git branch --show).out`
   - `make markdownlint 2>&1 | tee /tmp/markdownlint-$(get-project)-$(git branch
     --show).out`
   - `make nixie 2>&1 | tee /tmp/nixie-$(get-project)-$(git branch --show).out`
   - `make lint 2>&1 | tee /tmp/lint-$(get-project)-$(git branch --show).out`
   - `make test 2>&1 | tee /tmp/test-$(get-project)-$(git branch --show).out`

6) Commit once all gates pass. Use an imperative subject and a wrapped body.

## Validation and acceptance

Acceptance means:

- The macro crate no longer defines its own `RuntimeMode` or
  `TestAttributeHint` enums.
- `rstest_bdd::execution::RuntimeMode` and
  `rstest_bdd::execution::TestAttributeHint` still compile for downstream users
  via re-exports.
- `make check-fmt`, `make markdownlint`, `make nixie`, `make lint`, and
  `make test` succeed.

## Idempotence and recovery

All steps are repeatable. If a command fails, review the log in `/tmp/` and
re-run only the failed command after fixing the issue. Avoid partial commits;
use `git status` to confirm a clean staging area before committing.

## Artifacts and notes

Expected log files after validation:

    /tmp/fmt-$(get-project)-$(git branch --show).out
    /tmp/check-fmt-$(get-project)-$(git branch --show).out
    /tmp/markdownlint-$(get-project)-$(git branch --show).out
    /tmp/nixie-$(get-project)-$(git branch --show).out
    /tmp/lint-$(get-project)-$(git branch --show).out
    /tmp/test-$(get-project)-$(git branch --show).out

## Interfaces and dependencies

- New crate: `rstest-bdd-policy` with public enums:
  - `pub enum RuntimeMode { Sync, TokioCurrentThread }`
  - `impl RuntimeMode { pub const fn is_async(self) -> bool; pub const fn
    test_attribute_hint(self) -> TestAttributeHint; }`
  - `pub enum TestAttributeHint { RstestOnly, RstestWithTokioCurrentThread }`
- Runtime crate: re-export the policy types from
  `crates/rstest-bdd/src/execution.rs`.
- Macro crate: import policy types from `rstest-bdd-policy` instead of defining
  local copies.

## Revision note

Status updated to COMPLETE after implementation and validation. Outcomes and
decision log updated to reflect the markdown lint override and the final
delivery state.
