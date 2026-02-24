# ExecPlan 9.3.6: Add `StdHarness` behavioural tests for parity

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

`PLANS.md` is not present in this repository at the time of writing, so this
ExecPlan is the governing plan for roadmap item 9.3.6.

## Purpose / big picture

Roadmap item 9.3.6 requires dedicated behavioural coverage for `StdHarness`
with parity to the behavioural themes already covered for `TokioHarness`.
`StdHarness` is the default harness when users omit `harness = ...`, so its
behaviour must be explicit and protected by tests instead of being only an
implicit default path.

After this change:

- `StdHarness` has dedicated behavioural tests for metadata forwarding,
  closure execution, and panic propagation.
- Unit tests and behavioural tests both validate the delivered behaviour.
- `docs/rstest-bdd-design.md` records the final semantics and rationale.
- `docs/users-guide.md` records user-facing usage and guarantees.
- `docs/roadmap.md` marks 9.3.6 as done.
- `make check-fmt`, `make lint`, and `make test` all pass.

Success is observable when at least three behavioural tests for `StdHarness`
pass in `make test`, including one panic-propagation case.

## Constraints

- Implement only roadmap item 9.3.6 from `docs/roadmap.md`.
- Keep the `HarnessAdapter` public API unchanged.
- Keep scope focused on `StdHarness` behavioural parity and related
  documentation updates.
- Validate with both unit tests and behavioural tests.
- Record design decisions in `docs/rstest-bdd-design.md` (Design Doc §2.7.1).
- Record user-facing usage/guarantees in `docs/users-guide.md`.
- Mark roadmap entry 9.3.6 done only after all quality gates pass.
- Required quality gates: `make check-fmt`, `make lint`, `make test`.
- Capture gate output with `set -o pipefail` and `tee` to log files.

## Tolerances (exception triggers)

- Scope: if the change exceeds 8 files or 350 net lines, stop and escalate.
- Interfaces: if public trait or type signatures in
  `crates/rstest-bdd-harness` must change, stop and escalate.
- Behaviour: if `TokioHarness` behavioural coverage regresses while adding
  `StdHarness` parity, stop and escalate.
- Iterations: if the same gate (`check-fmt`, `lint`, or `test`) fails three
  times after attempted fixes, stop and escalate with logs.
- Ambiguity: if roadmap wording conflicts with current harness semantics,
  stop and request direction before proceeding.

## Risks

- Risk: tests may accidentally prove `ScenarioRunRequest` behaviour instead of
  `StdHarness` behaviour. Severity: medium. Likelihood: medium. Mitigation:
  keep assertions centred on `StdHarness::run` outcomes and panic forwarding.

- Risk: panic-propagation assertions can be brittle if they over-specify panic
  payload formatting. Severity: low. Likelihood: medium. Mitigation: assert a
  stable panic message substring or use `#[should_panic(expected = "...")]`.

- Risk: duplication between unit and behavioural tests could create noisy
  maintenance overhead. Severity: low. Likelihood: medium. Mitigation: keep
  unit tests minimal and reserve end-to-end semantics for behavioural tests.

- Risk: documentation drift between roadmap, design doc, and user guide.
  Severity: medium. Likelihood: medium. Mitigation: update all three in one
  delivery stage and cross-check wording.

## Progress

- [x] (2026-02-24) Reviewed roadmap item 9.3.6 and Design Doc §2.7.1.
- [x] (2026-02-24) Reviewed current `StdHarness` and `TokioHarness` tests.
- [x] (2026-02-24) Drafted this ExecPlan.
- [x] (2026-02-24) Stage A: baseline test run and gap confirmation.
- [x] (2026-02-24) Stage B: add/adjust `StdHarness` unit tests.
- [x] (2026-02-24) Stage C: add/adjust `StdHarness` behavioural tests (>=3
      cases).
- [x] (2026-02-24) Stage D: update design doc and user guide.
- [x] (2026-02-24) Stage E: mark roadmap item 9.3.6 done.
- [x] (2026-02-24) Stage F: run full quality gates and capture logs.

## Surprises & Discoveries

- Observation: this repository does not include additional planning metadata
  files beyond the documented roadmap and design docs. Impact: this plan is
  based on repository documents and source inspection only.

- Observation: `crates/rstest-bdd-harness/tests/harness_behaviour.rs` already
  includes `StdHarness` closure-execution tests, but there is no dedicated
  panic-propagation behavioural test for `StdHarness`, and metadata-forwarding
  coverage is currently expressed through a custom harness type rather than a
  `StdHarness`-named behavioural case. Impact: 9.3.6 will make parity explicit
  with dedicated `StdHarness`-focused behavioural cases.

## Decision Log

- Decision: satisfy roadmap parity by adding explicit `StdHarness` behavioural
  tests for three themes: metadata forwarding, closure execution, and panic
  propagation. Rationale: this maps directly to 9.3.6 acceptance criteria and
  avoids ambiguous interpretation of "implicit default" coverage. Date/Author:
  2026-02-24 / Codex.

- Decision: keep one targeted unit-test addition in `std_harness.rs` and place
  richer semantics in behavioural tests under
  `crates/rstest-bdd-harness/tests/harness_behaviour.rs`. Rationale: unit tests
  should stay small and behavioural tests should own user-observable semantics.
  Date/Author: 2026-02-24 / Codex.

- Decision: document `StdHarness` guarantees explicitly in design and user
  docs (runs closure directly, forwards request metadata unchanged to harness
  boundary, and propagates runner panics). Rationale: aligns ADR-005 intent
  with concrete, test-backed behaviour. Date/Author: 2026-02-24 / Codex.

## Outcomes & Retrospective

Delivered in 9.3.6:

- Added `StdHarness` panic-propagation unit coverage in
  `crates/rstest-bdd-harness/src/std_harness.rs`.
- Updated `StdHarness` behavioural coverage in
  `crates/rstest-bdd-harness/tests/harness_behaviour.rs` to include explicit
  cases for:
  - closure execution,
  - metadata forwarding at the harness boundary,
  - panic propagation.
- Updated `docs/rstest-bdd-design.md` (§2.7.1) with explicit behavioural
  guarantees for `StdHarness`.
- Updated `docs/users-guide.md` harness section with the same test-backed
  guarantees.
- Marked roadmap item 9.3.6 complete in `docs/roadmap.md`.

Validation summary:

- `make check-fmt` passed (`/tmp/9-3-6-impl-check-fmt.log`).
- `make lint` passed (`/tmp/9-3-6-impl-lint.log`).
- `make test` passed (`/tmp/9-3-6-impl-test.log`), including
  `rstest-bdd-harness` unit and behavioural tests.
- Documentation gates also passed:
  - `make markdownlint` (`/tmp/9-3-6-impl-markdownlint.log`)
  - `make nixie` (`/tmp/9-3-6-impl-nixie.log`)

Result against finish line:

- At least three behavioural tests for `StdHarness` pass in `make test`
  (four pass).
- Unit and behavioural test coverage now explicitly exercise panic
  propagation.

## Context and orientation

Primary implementation files:

- `crates/rstest-bdd-harness/src/std_harness.rs`
- `crates/rstest-bdd-harness/tests/harness_behaviour.rs`

Documentation files to update:

- `docs/rstest-bdd-design.md` (section 2.7.1)
- `docs/users-guide.md` (harness adapter section)
- `docs/roadmap.md` (mark 9.3.6 done)

Related existing coverage used as parity reference:

- `crates/rstest-bdd-harness-tokio/tests/harness_behaviour.rs`
- `crates/rstest-bdd-harness-tokio/src/tokio_harness.rs` unit tests

Reference documents reviewed while drafting:

- `docs/roadmap.md`
- `docs/rstest-bdd-design.md`
- `docs/rstest-bdd-language-server-design.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/gherkin-syntax.md`

## Plan of work

### Stage A: baseline and gap confirmation

Goal: confirm current behaviour, test names, and missing parity items before
editing.

Implementation details:

- Run targeted baseline tests:
  `cargo test -p rstest-bdd-harness --test harness_behaviour`.
- Confirm whether the three 9.3.6 themes are all covered by explicit
  `StdHarness` behavioural test cases.
- Record any naming/coverage gaps before editing.

Go/no-go validation:

- A clear delta list exists for metadata forwarding, closure execution, and
  panic propagation test coverage.

### Stage B: add or adjust unit tests for `StdHarness`

Goal: ensure the new behaviour is validated at unit-test level as well.

Implementation details:

- Extend `crates/rstest-bdd-harness/src/std_harness.rs` unit tests with a
  panic-propagation case for `StdHarness::run`.
- Keep existing direct-execution unit test intact.
- Keep unit tests minimal and local to module behaviour.

Go/no-go validation:

- `cargo test -p rstest-bdd-harness std_harness` passes.

### Stage C: add or adjust behavioural tests for `StdHarness`

Goal: deliver explicit behavioural parity with `TokioHarness` themes.

Implementation details:

- Update `crates/rstest-bdd-harness/tests/harness_behaviour.rs` so there are
  explicit `StdHarness` behavioural tests for:
  - closure execution (runner executes once and returns value),
  - metadata forwarding (request metadata reaches harness boundary unchanged),
  - panic propagation (runner panic is not swallowed).
- Keep existing non-static borrow coverage where useful, but ensure the three
  finish-line themes are represented by clear, dedicated test names.

Go/no-go validation:

- At least three `StdHarness` behavioural tests pass locally and are easy to
  map to roadmap 9.3.6 acceptance language.

### Stage D: documentation updates

Goal: keep design and user docs aligned with delivered semantics.

Implementation details:

- Update `docs/rstest-bdd-design.md` §2.7.1 with any final decision text about
  `StdHarness` behavioural guarantees.
- Update `docs/users-guide.md` harness section to describe the guarantees and
  test-backed expectations for `StdHarness`.

Go/no-go validation:

- Both docs describe the same guarantees with no contradictions.

### Stage E: roadmap update

Goal: close the roadmap item only after verified delivery.

Implementation details:

- Change roadmap checkbox 9.3.6 to done in `docs/roadmap.md` once all tests
  and quality gates pass.

Go/no-go validation:

- Roadmap reflects completion status accurately.

### Stage F: full quality gates with captured logs

Goal: prove release-quality completion.

Implementation details:

- Run each gate with log capture:
  - `set -o pipefail; make check-fmt 2>&1 | tee /tmp/9-3-6-check-fmt.log`
  - `set -o pipefail; make lint 2>&1 | tee /tmp/9-3-6-lint.log`
  - `set -o pipefail; make test 2>&1 | tee /tmp/9-3-6-test.log`
- Inspect logs for pass/fail and unexpected warnings.

Go/no-go validation:

- All three commands exit `0`.
