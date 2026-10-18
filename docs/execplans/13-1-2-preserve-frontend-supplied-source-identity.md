# Preserve frontend-supplied source identity in failed and skipped outcomes (13.1.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT — revised after the expert-panel review (D12), awaiting approval.
No implementation may begin until the plan is explicitly approved.

## Purpose / big picture

`rstest_bdd::runner` (shipped by roadmap item 13.1.1) lets a caller that is not
the Gherkin macro frontend build a `ScenarioPlan`, execute it with
`run_scenario` or `run_scenario_async`, and receive a structured
`ScenarioOutcome` instead of a panic. A frontend that parses Markdown, TOML, or
anything else supplies its own source path, line, and optional column for each
step, so that when something goes wrong it can point its user at the heading or
fence that produced the failing step.

Today that promise holds only part of the way. The per-step records keep the
supplied location, but the value a caller is told to use as its success test,
`ScenarioOutcome::into_harness_result`, returns a `ScenarioFailure` that has
thrown the location away for an ordinary step failure: what survives is a step
index and an `ExecutionError` whose only notion of place is a flattened
`feature_path` string, with no line and no column. A folded empty-plan failure
carries no identity at all, neither the outcome nor the failure records which
document the scenario came from, and `ScenarioFailure`'s `Display` reads its
path back out of the runtime error. Nothing executes a plan whose steps carry a
column and checks that the column comes back.

After this change, a frontend can execute a plan sourced from `notes/demo.md` or
`spec/cases.toml`, fold the outcome, and read from the failure the exact path,
one-based line, and one-based column it supplied for the step that failed or
skipped, plus the scenario's own document and line, without parsing any string
and without the runtime's error types learning anything about the frontend. The
observable proof is a set of runtime tests that execute such plans through both
runners, across every failure and skip path, and assert the supplied
coordinates.

## Definitions

- **Frontend**: code that turns some authored document into a `ScenarioPlan`.
  The Gherkin macros are one frontend; Trymark (roadmap 13.3.1) is intended to
  be another.
- **Step-level identity**: the `SourceLocation` a frontend attached to one
  `StepInvocation` — an opaque path, a one-based line, and an optional
  one-based column measured in Unicode scalar values. It is optional per step.
- **Scenario identity**: the plan's own `SourcePath` and optional one-based
  scenario line. The plan accepts no scenario column.
- **Terminal step**: the step whose result stopped the run — a skip or a
  failure. Every later step is recorded as `Bypassed`.
- **Fold**: `ScenarioOutcome::into_harness_result`, which consumes an outcome
  and returns `Ok(())` or a `ScenarioFailure`. It is the one place skip policy
  (`fail_on_skipped`) and the empty-plan rule are applied. It drops the
  per-step records.
- **Runtime error**: `rstest_bdd::execution::ExecutionError`, `StepError`,
  `MissingFixturesDetails`, and `StepExecutionRequest`. They were released in
  v0.6.0 and are not runner types.
- **Opaque type**: a public type whose fields and variants are private, read
  only through accessor methods, so its representation can change without
  breaking callers.
- **Failed or skipped outcome surface**: one of the places, enumerated in D1,
  where a caller can observe a failure or a skip. "Every failed or skipped
  outcome" in the roadmap means every member of that closed set.

## Signposted documentation and skills

Read these before starting:

- `docs/adr-018-parser-neutral-scenario-execution.md`, especially "Structured
  outcome" (the binding source semantics), "Source-neutral diagnostics" (the
  section this item discharges), "Known risks and limitations" ("prefer opaque
  accessors where future change is plausible"), and verification item 7.
- `docs/rstest-bdd-design.md` §2.6.4 "The parser-neutral scenario runner" and
  §3.11.1 "The parser-neutral runner modules".
- `docs/developers-guide.md` section "The parser-neutral runner
  (rstest_bdd::runner)" for the engine split, the integration-test rule (D21),
  skip-policy isolation with `#[serial]`, and the runner gate commands; and
  "Assertion vocabulary: googletest, pretty_assertions, and insta".
- `docs/users-guide.md` section "Running a plan without the Gherkin frontend".
- `docs/rust-testing-with-rstest-fixtures.md` for fixture and `#[case]` shape.
- `docs/rust-doctest-dry-guide.md` for doctest style on new accessors.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`; the
  cognitive-complexity threshold is 12 and the drivers must stay decision-free.
- `docs/gherkin-syntax.md` for the behavioural feature file added here.
- `docs/rstest-bdd-language-server-design.md` lines 868-884, only to note that
  the language server uses zero-based positions; this plan does not touch it.
- `docs/execplans/13-1-1-add-parser-neutral-types.md`, by search rather than
  linear reading: INV-5, INV-7, INV-11, D3, D14, D15, D21, D27, D46, and the
  lessons on the 400-line cap and on gate logs.

Skills to load: `rust-router`, then `rust-types-and-apis` (opaque types and
accessor naming), `rust-unit-testing` (rstest tables, googletest,
pretty_assertions, insta), `proptest` (the source-fidelity property),
`rust-verification` (method selection), `nextest`,
`addressing-whitaker-findings` (test-helper lints), `execplans`, and
`en-gb-oxendict-style` for prose. Use `codegraph-mcp` for callers of
`ScenarioFailure`, `assemble`, and `Terminal`.

## Constraints

1. `ExecutionError`, `StepError`, `MissingFixturesDetails`,
   `StepExecutionRequest`, and every other type released in v0.6.0 keep their
   exact variants, field sets, and signatures. No line, column,
   `SourceLocation`, or other frontend coordinate may be added to them. This is
   the roadmap's "without extending runtime errors with frontend metadata".
2. No source identity is ever read back out of an `ExecutionError` or derived
   by parsing any string, in accessors or in `Display` (ADR-018, "Structured
   outcome"; 13.1.1 INV-7 clause three). After this change the rule holds
   without exception (D5).
3. The runner treats paths as opaque: no normalization, extension check, case
   folding, or separator rewriting. A `.feature` path is as opaque as a `.md`
   one. The runner stores columns as given and never recomputes them.
4. No edits under `crates/rstest-bdd-macros/`. Macro migration is 13.2.1.
5. No edits to any Fluent `.ftl` file and no new message identifiers. The
   source-neutral message wording is carried by 13.3.1 (13.1.1 D15).
6. No new dependencies. `rstest`, `rstest-bdd`, `googletest`,
   `pretty_assertions`, `insta`, `proptest`, `serial_test`, `temp-env`, and
   `tokio` are already available to `rstest-bdd`'s tests.
7. Runner public types stay frontend-free: `crates/rstest-bdd/src/runner/`
   must keep passing the INV-11 token scan in
   `crates/rstest-bdd/src/runner/tests/surface.rs`, which forbids frontend
   words such as `markdown` and `reporting` in non-comment text, including
   `#[case::…]` names.
8. Every Rust file stays at or below 400 lines; the shrink-only allowlist
   `scripts/rs-length-allowlist.txt` gains no entry.
9. The drivers (`engine/drive_sync.rs`, `engine/drive_async.rs`) gain no
   decision; anything that decides belongs in `engine/policy.rs` or in
   `engine/drive.rs`'s shared helpers.
10. Any test that calls `run_scenario*` and resolves a step lives under
    `crates/rstest-bdd/tests/` (13.1.1 D21).
11. Every commit passes `make check-fmt`, `make lint`, and `make test`; a
    commit touching Markdown also passes `make markdownlint` and `make nixie`.
    No commit contains a failing or `#[ignore]`d test. The red stage of
    Red-Green-Refactor is run and recorded, never committed (D10).

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, and escalate.

## Tolerances (exception triggers)

- Scope: the committed deliverable (every changed file except this ExecPlan)
  exceeds 40 files or 3,000 net added lines, where net means insertions minus
  deletions. The planned file list in `Interfaces and dependencies` counts 34
  files. Re-measure at every milestone boundary on committed state, against the
  post-rebase merge-base, with the command in `Concrete steps`, and record the
  figure and revision in `Progress`. (13.1.1 breached its tolerance silently
  because nothing re-measured; D27 there.)
- Interface: any change to a public item released in v0.6.0, or any new
  public runner item beyond those listed in `Interfaces and dependencies`,
  stops work for escalation.
- Dependencies: any new crate, feature, or tool stops work.
- Iterations: a gate still failing after three focused fix attempts stops
  work.
- Time: any single milestone exceeding six hours of active work stops work.
- Ambiguity: if ADR-018 text admits two readings that lead to different
  public shapes, stop and present both.
- Evidence: if a red run fails for any reason other than a missing or wrong
  location (for example a compile error or an unrelated panic), stop and
  diagnose before writing implementation.

## Risks

- Risk: `crates/rstest-bdd/src/runner/outcome/mod.rs` is 367 lines and
  `outcome/failure.rs` is 254; the opaque `ScenarioFailure` and the scenario
  identity may push either past the 400-line cap. Severity: medium. Likelihood:
  high. Mitigation: before the behavioural change, commit a pure refactor moving
  `SkipPolicyRecord`, `SkipRecord`, and `ScenarioSkip` to a new
  `crates/rstest-bdd/src/runner/outcome/skip.rs`, re-exported unchanged, with
  all gates green. Check with `python3 scripts/check_rs_file_lengths.py`.
- Risk: making `ScenarioFailure` opaque breaks the four doctests that build
  `ScenarioFailure::EmptyPlan` as a value (`outcome/failure.rs` lines 124, 193,
  and 211, and `outcome/mod.rs` line 304 at `3f3752a6`), because doctests
  compile as external crates. Severity: low. Likelihood: certain. Mitigation:
  rewrite each to fold a real run of an empty plan with an explicit
  `ScenarioScope::with_skip_policy(false)`, or to show `site()` only; list them
  in EP-M2's checklist.
- Risk: about 60 in-crate call sites construct `ScenarioFailure` or call
  `ScenarioOutcome::new` (`reporting/conversion/tests.rs`,
  `runner/engine/policy_tests/assemble.rs`,
  `runner/tests/outcome/{mod,rendering}.rs`, and
  `runner/tests/source/{mod,rendering,support}.rs`). A mechanical update that
  writes `None` for the new step location silently weakens the INV-7 tests.
  Severity: medium. Likelihood: medium. Mitigation: every updated step failure
  carries its record's own location (SRC-3); crate-private constructors take
  the location as a required argument, so omission is a visible `None` in
  review; add one unit row where the two copies disagree to pin which copy
  `terminal_source()` reads.
- Risk: Whitaker lints (`no_expect_outside_tests`,
  `no_unwrap_or_else_panic`) apply to integration-test helpers, and a by-value
  non-`Copy` fixture parameter trips pedantic `needless_pass_by_value`.
  Severity: low. Likelihood: medium. Mitigation: helpers use
  `let … else { panic!(…) }`; the missing-fixture step takes `&T`.
- Risk: `make test`'s `--no-default-features` leg (`Makefile` lines 121-141)
  runs every new binary again with diagnostics off. Severity: low. Likelihood:
  medium. Mitigation: run the focused `--no-default-features` loop in
  `Concrete steps` before each gate.
- Risk: tests that exercise `fail_on_skipped` race the process-wide override.
  Severity: medium. Likelihood: low. Mitigation: pass the policy explicitly with
  `ScenarioScope::with_skip_policy` (`runner/scope.rs` line 160), as
  `tests/skip_parity.rs` does; no new test reads the environment.
- Risk: the `Display` snapshot
  `crates/rstest-bdd/src/runner/tests/source/snapshots/rstest_bdd__runner__tests__source__rendering__the_display_projection_renders_every_variant.snap`
  churns. Severity: low. Likelihood: low. Mitigation: the snapshot is built
  from hand-made outcomes whose failing row uses an error recording the same
  path as the plan (`honest_error(0, SPEC_PATH)`), and the forced-skip and
  empty-plan rows render text that names no path, so the substitution D5 makes
  cannot change it, provided `failed_step_case` gains the location
  `at(SPEC_PATH, 7)`. A snapshot change is a signal to stop and explain, not to
  accept.
- Risk: 13.2.1 promotes `reporting/conversion.rs` to production and could
  read scenario identity from two places that diverge. Severity: medium.
  Likelihood: low. Mitigation: D4 names the plan as the conversion's input and
  adds an equality assertion between the plan's and the outcome's identity to
  the conversion tests; the 13.2.1 roadmap note is updated.
- Risk: a downstream frontend passes a byte offset where a scalar-value
  column is required, so non-ASCII lines report the wrong column. The runner
  cannot detect this (Constraint 3). Severity: medium. Likelihood: medium.
  Mitigation: the users' guide gains a conversion example, and the 13.3.1
  roadmap note records the obligation.

## Progress

- [x] (2026-10-18) Branch renamed to
  `13-1-2-preserve-frontend-supplied-source-identity`; base `3f3752a6` (equal to
  `origin/main` at drafting time).
- [x] (2026-10-18) Reconnaissance by a Wyvern team: runner source flow,
  runtime-error metadata, 13.1.1 decisions, and documentation map.
- [x] (2026-10-18) Prior-art check (Cucumber messages, SARIF 2.1.0, Language
  Server Protocol) recorded in `Artefacts and notes`.
- [x] (2026-10-18) Draft committed (`b68c5d15`).
- [x] (2026-10-18) Expert-panel review (three reviewers covering six lenses);
  verdict "revise"; findings adopted as recorded in D12, and the plan rewritten.
- [ ] Approval of the plan by the user.
- [ ] EP-M1: characterization tests and the error-shape tripwire (green).
- [ ] EP-M2: opaque failure, step and scenario identity (red recorded, then
  green).
- [ ] EP-M3: property evidence, negative controls, and mutation sweep.
- [ ] EP-M4: documentation, roadmap, and close.

## Surprises & discoveries

- Observation: most of the per-step plumbing already exists. Every
  `StepOutcome` copies its invocation's `SourceLocation` through a single
  private constructor, `StepOutcome::recorded` in
  `crates/rstest-bdd/src/runner/outcome/step.rs`, and `ScenarioSkip` already
  owns its own copy of the skipping step's location. Evidence: Wyvern
  reconnaissance and direct reading. Impact: 13.1.2 closes specific gaps and
  adds executed-run evidence; it is not a new subsystem.
- Observation: `engine::policy::Terminal::Skip` carries `source`, but
  `Terminal::Fail` does not, so the asymmetry between skip and failure starts
  in the engine, not in the fold. Evidence:
  `crates/rstest-bdd/src/runner/engine/drive.rs` (`record_step`) and
  `engine/policy.rs` (`assemble`). Impact: the fix belongs at the terminal,
  where the invocation is in hand.
- Observation: the runner was merged after the `v0.6.0` tag
  (`git merge-base --is-ancestor 1dfe6725 v0.6.0` is false, and
  `git ls-tree v0.6.0 -- crates/rstest-bdd/src/runner` is empty). Impact: the
  runner's public types carry no compatibility obligation, so `ScenarioFailure`
  can become opaque now, which will not be true after the next release.
- Observation: an inherent `ScenarioFailure::source()` would shadow
  `std::error::Error::source()`, which `ScenarioFailure` implements to return
  its `ExecutionError` cause, and which existing tests call in fully qualified
  form (`runner/tests/outcome/rendering.rs` lines 90-123). Evidence: panel
  finding. Impact: the step-level accessor is named `step_source()` (D3).
- Observation: `reporting/conversion.rs` lines 33-37 document that the
  outcome deliberately carries no scenario identity, which is why the
  conversion takes the plan as an argument. Evidence: panel finding. Impact: D4
  must update that comment and name the canonical input.
- Observation: the property suite
  `crates/rstest-bdd/tests/runner_sequence_props` already generates plans with
  per-step lines over nine step kinds (including undefined and missing-fixture
  steps), runs both runners, and has a negative-control pattern over test-owned
  projections. Evidence: panel finding. Impact: the source properties extend
  that suite instead of creating a second property binary (D8).

## Decision log

- D1. The closed set of failed or skipped outcome surfaces.
  Decision: "every failed or skipped outcome" means these surfaces, and each
  exposes the frontend-supplied identity that applies to it.

  1. S1, a `StepOutcome` whose status is `Failed`, `Skipped`, or `Bypassed`:
     `StepOutcome::source()` (exists).
  2. S2, the terminal skip on an outcome: `ScenarioSkip::source()` (exists).
  3. S3, the terminal failure on an outcome:
     `ScenarioOutcome::failure().and_then(ScenarioFailure::step_source)`
     (new).
  4. S4, `ScenarioOutcome::terminal_source()` (exists; still step-level only,
     reimplemented over S2 and S3 so there is one route).
  5. S5, a folded step failure: `ScenarioFailure::step_source()` (new).
  6. S6, a folded forced skip: `ScenarioFailure::step_source()` and
     `ScenarioFailure::skip()` (new accessors over existing data).
  7. S7, a folded empty-plan failure: `ScenarioFailure::scenario_path()` and
     `scenario_line()` (new); `step_source()` is `None` because no step
     exists.
  8. S8, scenario identity on every outcome and every folded failure:
     `scenario_path()` and `scenario_line()` on both types (new).

  Rationale: naming the set makes "every" checkable and gives each member a
  test row. Rendered text is not a surface for coordinates (D5).
- D2. `ScenarioFailure` becomes an opaque struct now, while it is unreleased.
  Decision: replace the public enum with
  `pub struct ScenarioFailure { scenario: ScenarioIdentity, repr: FailureRepr }`,
  where the private `FailureRepr` enum holds `Step { index, error, source }`,
  `ForcedSkip(ScenarioSkip)`, and `EmptyPlan`. The public surface is accessors
  only. `FailureSite` remains the matchable projection, so a caller that
  branches on the kind of failure matches `failure.site()`. Rationale:
  ADR-018's "prefer opaque accessors where future change is plausible"; change
  is plausible (the struck lifecycle-hook failures are expected arrivals,
  `failure.rs` lines 162-168); the `outcome` module already promises that every
  payload sits behind an accessor; and no code outside the crate destructures
  the enum today (integration tests call only `site()`). Marking individual
  variants `#[non_exhaustive]` was rejected as half-done: it would leave
  `ForcedSkip` exhaustive and still expose the representation. Compatibility:
  none required; the runner is unreleased.
- D3. Step-level identity on a failure is `step_source()`.
  Decision: `ScenarioFailure::step_source() -> Option<&SourceLocation>` returns
  the failing invocation's location for a step failure and the skipping
  invocation's location for a forced skip, and `None` for an empty plan.
  Rationale: `source()` would shadow `std::error::Error::source()` (see
  `Surprises & discoveries`); the `step_` prefix keeps ADR-018's `source`
  vocabulary while saying which level of identity it is. A scenario line is
  never returned from a step-level accessor, so a reporter cannot mistake one
  for the other; `terminal_source()` likewise stays step-level.
- D4. Scenario identity is carried on every outcome and every failure.
  Decision: a crate-private
  `ScenarioIdentity { path: SourcePath, line: Option<u32> }`, built once per
  run by a shared helper `ScenarioIdentity::of(&ScenarioPlan)`, is passed to
  `assemble`, stored on `ScenarioOutcome`, and moved onto any `ScenarioFailure`
  the fold produces. Both types expose `scenario_path() -> &str` and
  `scenario_line() -> Option<u32>`, matching `SourceLocation::path()` and
  avoiding a second meaning of "source". The scenario name is not part of the
  identity: it is a label, not a source coordinate, and it stays on the plan.
  No scenario column is added: the plan accepts none, and adding one is an API
  extension 13.1.2 does not need. The plan remains the canonical input of
  `reporting/conversion.rs`; its doc comment is corrected and its tests assert
  that the plan's and the outcome's identity agree. Rationale: without this, an
  aggregating frontend that folds and collects failures loses the document for
  an empty plan and for any step failure whose step had no location — exactly
  the malformed-input cases a frontend most needs to point at — and `Display`
  has nowhere plan-side to fall back to. Carrying the path even when no
  scenario line was supplied resolves the draft's open question in favour of
  preservation. The cost is one copy (`Static`) or one atomic increment
  (`Shared`) per run.
- D5. Rendering keeps its shape and never reads the error.
  Decision: `ScenarioFailure`'s `Display` renders the step location's path when
  one exists and otherwise the scenario path; `ScenarioOutcome`'s `Display`
  delegates to it. Both pass that path to the crate-private
  `ExecutionError::format_with_loader_at`, so message identifiers and shape are
  unchanged. Line and column are not added to rendered text; that is a
  temporary state, ending when 13.3.1 introduces the source-neutral Fluent
  message (13.1.1 D15). Rationale: Constraint 2 then holds without the fallback
  13.1.1 documented; coordinates in messages are a user-visible wording change
  owned by 13.3.1.
- D6. "Runtime errors are not extended" is enforced, not merely observed.
  Decision: add `crates/rstest-bdd/src/execution/error/shape_tests.rs`, an
  in-crate test module containing one wildcard-free `match` over every
  `ExecutionError` variant and every `StepError` variant, each destructured
  without `..`, plus `..`-free destructuring of `MissingFixturesDetails` and
  `StepExecutionRequest`. `#[non_exhaustive]` does not apply inside the
  defining crate, so adding any field or variant stops the build. The module
  lives under `execution/` because that module owns the types. Rationale: a
  compile-time tripwire is the cheapest artefact that fails when Constraint 1
  is broken; `FailureKind::of`'s wildcard arm would not.
- D7. No Verus and no Kani for this item. Rationale: every obligation is a
  value-preservation property of data plumbing — a location cloned from an
  invocation into a record, moved through a terminal, an assembly, and a fold.
  The shape space (failure kind × location present or absent × runner) is
  finite and covered exhaustively by `rstest` tables; the sequence dimension is
  covered by `proptest`. No contractual business logic or lemma needing
  unbounded reasoning is introduced, and the repository has no Verus or Kani
  setup; introducing one to restate `clone() == original` would be the
  artificial obligation the task warns against. The argument's structure is
  kept explicit as "structural claims" in `Verification plan`, mapped to tests,
  so that a later reader can see what was shown and how.
- D8. Test layout: one new integration binary and one new property module.
  Decision: `crates/rstest-bdd/tests/runner_sources.rs` hosts both the
  executed-run `rstest` tables and the behavioural scenarios bound to
  `crates/rstest-bdd/tests/features/runner_sources.feature`, with support in
  `crates/rstest-bdd/tests/runner_sources/`. The behavioural steps reuse
  `crates/rstest-bdd/tests/parser_neutral_runner/support.rs` through `#[path]`,
  after giving its `Bench` a `fail_on_skipped` field (default `false`) and an
  asynchronous execution step. The source properties extend
  `crates/rstest-bdd/tests/runner_sequence_props` with new files only:
  `runner_sequence_props/sources.rs` (properties),
  `runner_sequence_props/source_controls.rs` (negative controls), and
  `runner_sequence_props/sequence/located.rs` (a location-shape dimension and
  `plan_for_located`, beside the untouched `plan_for`). Rationale: each
  integration binary costs a link of the whole crate; the property suite
  already owns the generator, the step registry, both runners, and the
  60-second case budget; and the near-cap files (`skip_parity.rs` at 389 lines,
  `runner_panics.rs` at 370, `sequence/steps/mod.rs` at 386) are not touched.
- D9. No ADR. Rationale: the change implements ADR-018's existing binding
  semantics ("Terminal `Skipped` and `Failed` outcomes carry `source`";
  "Locations are never added to `ExecutionError`"; "prefer opaque accessors")
  rather than altering them. D2-D5 are recorded in `docs/rstest-bdd-design.md`
  §2.6.4 in a new "Source-neutral diagnostics" paragraph that cites ADR-018.
- D10. Red-Green-Refactor without red commits.
  Decision: EP-M1 is a green plateau (characterization tests that pin what
  already works, plus the tripwire). In EP-M2 the red rows are written first
  and made to compile with local, uncommitted accessor stubs; the red run is
  executed and its transcript recorded in `Artefacts and notes`; then the
  implementation replaces the stubs and the tests and implementation are
  committed together. Rationale: Constraint 11 forbids a red or `#[ignore]`d
  commit, and stub accessors that return wrong answers must never be published,
  even transiently. Alternative rejected: committing the red stage behind
  `#[ignore]` (breaks the gate rule and 13.1.1's gate-clean plateau convention).
- D11. The step index is the frontend's join key, and the frontend location
  is the primary one. Decision: document and test (SRC-9) that a `FailureSite`
  or `ScenarioSkip` index `i` identifies `plan.steps()[i]`, so a frontend
  holding a richer source map (for example Markdown byte ranges) can key it by
  index rather than forcing that map into `SourceLocation`. The
  frontend-supplied location is the primary location of a failure;
  definition-side locations (`MissingFixturesDetails::step_location`, a panic's
  Rust location) remain inside the error, unchanged. Rationale: ADR-018
  technical requirement 6 (frontends retain richer source maps); this is also
  how Cucumber messages join results to sources. `SourceLocation`'s private
  fields leave room to add an end position or span later without a breaking
  change.
- D12. Expert-panel record. Three reviewers covered six lenses (contracts and
  structure; failure modes and long-term viability; alternatives and cost).
  Verdict on the draft: "revise". Adopted: opaque failure (D2), `step_source`
  naming (D3), step-level `terminal_source` and scenario identity on every
  failure (D4), no error read-back in `Display` (D5), full-shape tripwire (D6),
  structural claims instead of lemmas (D7), consolidated tests and the existing
  generator (D8), no red commits (D10), the index join key (D11), realistic
  scope tolerance, projection-based negative controls, a fixed proptest seed, a
  skip-policy dimension, distinct per-index locations, a non-interference
  property, and the pre-mortem mitigations in `Risks`. Declined: deferring
  scenario identity to 13.3.1 (the alternatives reviewer's strongest
  alternative), because the runner types are cheapest to change before their
  first release and two reviewers showed that folding without it loses the
  document in the malformed-input cases; and a test-formatted source-report
  snapshot, because it would repeat the table's assertions without covering a
  second output format.
- D13. Prior art supports the shape, not a different one. Cucumber messages
  give `Location` a required one-based line and an optional one-based column,
  matching `SourceLocation`, and join results to locations through identifiers
  rather than embedding locations in results or errors; that is D11's index
  join key, and D2-D4 are justified by ADR-018's binding text, not by Cucumber.
  SARIF and the Language Server Protocol inform the column-unit note in the
  users' guide.

## Outcomes & retrospective

Not started. To be completed at each milestone and at close.

## Context and orientation

The runtime crate is `crates/rstest-bdd`. Its parser-neutral runner is the
module `crates/rstest-bdd/src/runner/`:

- `source.rs` defines `SourcePath` (an opaque path held as
  `Static(&'static str)` or `Shared(Arc<str>)`, compared by text) and
  `SourceLocation` (path, one-based `line: u32`, optional one-based
  `column: Option<u32>`; both constructors panic on a zero coordinate in every
  profile). The crate-private `ScenarioIdentity` will live here.
- `plan.rs` and `plan/builder.rs` define `ScenarioPlan` (name, tags,
  `source: SourcePath`, `source_line: Option<u32>`, steps, `allow_skipped`) and
  `StepInvocation` (keyword, text, optional doc string and table,
  `source: Option<SourceLocation>`). `ScenarioPlan::source()` returns `&str`,
  so building `ScenarioIdentity` needs a crate-private accessor for the
  `SourcePath`. A step column can only be supplied with
  `StepInvocation::new(..).at(SourceLocation::new(path, line, Some(column)))`;
  `ScenarioPlanBuilder::step_at` takes a line only.
- `outcome/mod.rs` defines `ScenarioStatus`, `SkipPolicyRecord`,
  `SkipRecord`, `ScenarioSkip`, and `ScenarioOutcome` (status, steps, optional
  skip, optional failure), with `terminal_source()`, `into_harness_result()`,
  and a `Display`. `outcome/step.rs` defines `StepOutcome`;
  `outcome/failure.rs` defines `FailureKind`, `FailureSite`, and today's
  `ScenarioFailure` enum (`Step { index, error }`, `ForcedSkip(ScenarioSkip)`,
  `EmptyPlan`).
- `engine/drive.rs` holds helpers shared by both drivers: `request` (builds
  the `StepExecutionRequest`, copying the plan's path into the request's
  `feature_path` field), `record_step` (classifies a result and builds the
  terminal), and `bypassed`. `engine/policy.rs` holds `absorb`, `classify`,
  `Terminal`, `SkipPolicy`, and `assemble`. `engine/drive_sync.rs` and
  `engine/drive_async.rs` are the two loops.
- `tests/` holds in-crate unit tests, including INV-7's synthetic source
  tests in `tests/source/` and the INV-11 surface scan in `tests/surface.rs`.

Runtime errors live in `crates/rstest-bdd/src/execution/error/`.
`ExecutionError::StepNotFound` and `HandlerFailed` carry `feature_path` and
`scenario_name` strings; `MissingFixturesDetails` carries the same two plus the
step definition's own `step_location`. None carries a line or column.

The one-way, test-only conversion from runner outcome to the legacy reporting
record is `crates/rstest-bdd/src/reporting/conversion.rs`; its tests construct
`ScenarioFailure` and `ScenarioOutcome` values and must be updated with D2.

Integration tests that execute plans are in `crates/rstest-bdd/tests/`:
`parser_neutral_runner.rs` (behavioural, with
`tests/features/parser_neutral_runner.feature` and support in
`parser_neutral_runner/support.rs`), `completeness.rs`, `skip_parity.rs`,
`runner_panics.rs`, `runner_instrumentation.rs`, `runner_cancel.rs`,
`runner_wire.rs`, and the property suite `runner_sequence_props.rs` with its
`runner_sequence_props/` directory (generator, step registry, run harness, and
controls under `sequence/`). Steps are registered in each binary with the
`#[given]`, `#[when]`, and `#[then]` attribute macros.

## Conformance basis

- Roadmap: `docs/roadmap.md` item 13.1.2 at `3f3752a6` (RM-13.1.2). Success
  criterion RM-13.1.2-S1: "runtime tests execute plans whose sources are not
  `.feature` files and assert that supplied path, line, and column information
  remains available to the caller".
- ADR: `docs/adr-018-parser-neutral-scenario-execution.md` (Accepted,
  2026-07-13) at `3f3752a6`:
  - ADR18-OUT-SRC: "Every such outcome exposes
    `source() -> Option<&SourceLocation>`. Terminal `Skipped` and `Failed`
    outcomes carry `source` … alongside the terminal step index." The names
    are explicitly illustrative; the semantics are binding.
  - ADR18-LOC: "`SourceLocation` consists of a path, a one-based line, and an
    optional one-based column measured in Unicode scalar values. Locations
    are never added to `ExecutionError` and are never inferred by parsing
    error strings."
  - ADR18-DIAG ("Source-neutral diagnostics"): the runner pairs an
    `ExecutionError` with the invocation's source location through the
    structured outcome.
  - ADR18-TR6: the API permits frontends to retain richer source maps without
    forcing those maps into rstest-bdd's core types.
  - ADR18-OPAQUE ("Known risks and limitations"): prefer opaque accessors
    where future change is plausible.
  - ADR18-VER7: "Execute plans whose source paths are not `.feature` files
    and prove outcomes preserve the supplied locations without Gherkin
    parsing."
- Design: `docs/rstest-bdd-design.md` §2.6.4 at `3f3752a6` (DES-2.6.4).
- Predecessor: `docs/execplans/13-1-1-add-parser-neutral-types.md`, INV-5
  (sync/async equality), INV-7 (source fidelity), INV-11 (surface purity), D15
  (Fluent wording deferred), D21 (integration-test rule).
- No Terms of Reference document exists for this phase.

Trace links:

```plaintext
RM-13.1.2 -> ADR18-OUT-SRC -> D2,D3 -> EP-M2 -> runner_sources::a_folded_failure_keeps_its_step_location
RM-13.1.2 -> ADR18-OUT-SRC -> D4 -> EP-M2 -> runner_sources::a_folded_empty_plan_names_its_document
RM-13.1.2 -> ADR18-OPAQUE -> D2 -> EP-M2 -> runner::tests::outcome::fold_source
RM-13.1.2 -> ADR18-LOC -> D6 -> EP-M1 -> execution::error::shape_tests (compile-time tripwire)
RM-13.1.2 -> ADR18-LOC -> D5 -> EP-M2 -> runner::tests::source::rendering::a_folded_failure_renders_a_plan_side_path
RM-13.1.2-S1 -> ADR18-VER7 -> D8 -> EP-M1,EP-M2 -> runner_sources (tables over failure kinds x runners x location shapes)
RM-13.1.2-S1 -> ADR18-VER7 -> EP-M3 -> runner_sequence_props::sources (SRC-1..SRC-5, SRC-9, SRC-10)
RM-13.1.2-S1 -> ADR18-DIAG -> EP-M1,EP-M2 -> runner_sources.feature
ADR18-TR6 -> D11 -> EP-M2 -> runner_sources::the_step_index_joins_a_frontend_side_table
ADR18-TR6 -> Constraint 3 -> EP-M1 -> runner_sources::paths_and_columns_are_preserved_verbatim
```

## Verification plan

### Abstract model

Model a plan as `P = (scn, inv[0..n))` where `scn = (path, line?)` and each
invocation `inv[i]` has `src(inv[i]) ∈ Option<Loc>`, with
`Loc = (path, line ≥ 1, column? ≥ 1)`. A run maps `P`, a behaviour per step
(pass, return, skip, fail by error, fail by panic, undefined, missing fixture),
and a resolved skip policy to an outcome
`O = (status, rec[0..n), skip?, failure?, scn')`. The fold maps `O` to `Ok(())`
or a failure `f = (scn'', kind, index?, loc?, skip?)`. The model never inspects
`path`: it is an opaque value compared by equality.

Preconditions: every `Loc` was built by `SourceLocation::new` or `new_static`,
so its coordinates are one-based (enforced by construction); the plan was built
by `ScenarioPlanBuilder`, which rejects a zero scenario line.

### Invariants

- SRC-1 (record fidelity): for every `i < n`, `src(O.rec[i]) = src(inv[i])`
  — path, line, and column — whatever `O.rec[i]`'s status.
- SRC-2 (terminal fidelity): if `O.status ≠ Passed`, exactly one of
  `O.skip()` and `O.failure()` is present; if the run stopped at step `t`, then
  `O.terminal_source() = src(inv[t])`, and the present one's step-level
  location equals `src(inv[t])`.
- SRC-3 (representation agreement): the outcome holds each terminal location
  twice — once in `rec[t]` and once in the skip or failure. The two copies are
  equal. (Safe because neither copy can be constructed outside the crate.)
- SRC-4 (fold preservation): if `fold(O) = Err(f)` then
  `f.step_source() = O.terminal_source()` for a step failure or forced skip,
  `f.step_source() = None` for an empty plan, and in every case
  `f.scenario_path() = scn.path` and `f.scenario_line() = scn.line`.
- SRC-5 (scenario fidelity): `O.scenario_path() = scn.path` and
  `O.scenario_line() = scn.line`, for every status.
- SRC-6 (runner agreement): the synchronous and asynchronous runners produce
  equal outcomes, sources and scenario identity included (13.1.1 INV-5,
  re-asserted with columns).
- SRC-7 (no error extension, no read-back): the runtime error types' variants
  and field sets are unchanged, and no accessor or `Display` takes a path from
  an `ExecutionError`.
- SRC-8 (opacity): paths come back byte-identical, including paths ending in
  `.feature`, paths with `./` prefixes, backslashes, spaces, and non-ASCII
  text; lines and columns `1` and `u32::MAX` come back unchanged; columns on
  lines containing multi-byte characters come back as supplied.
- SRC-9 (join key): for a terminal at index `t`, `f.site()` and
  `O.skip().at()` name `t`, and `plan.steps()[t]` is the invocation whose
  location the failure reports.
- SRC-10 (non-interference): removing every step location and the scenario
  line from a plan changes the outcome only in those fields — status, indices,
  records, skip messages, and failure kinds are equal.

### Structural claims (the argument's shape)

These are claims about four small functions, discharged by inspection plus the
tests named against each invariant. They are not lemmas in the formal sense,
and implementation may show that SC-B and SC-C collapse into one.

- SC-A (single constructor): every `StepOutcome` is built by the private
  `StepOutcome::recorded`, which clones `invocation.source()`. Field privacy
  makes this the only route. With INV-2 (one record per invocation, in order)
  this gives SRC-1.
- SC-B (terminal carries its invocation's location): both arms of
  `record_step` that produce a `Terminal` set its `source` from the same
  `invocation` whose record they return. With SC-A this gives SRC-3.
- SC-C (assembly moves, never builds): `assemble` moves the terminal's
  `source` into `SkipRecord` or the failure's `Step` representation unchanged,
  and moves `ScenarioIdentity` onto the outcome unchanged. With SC-B and
  INV-1/INV-2 (the terminal index is the stopping step) this gives SRC-2,
  SRC-5, and SRC-9.
- SC-D (fold moves, never builds a location): `into_harness_result` moves
  the stored failure, wraps the stored skip, or builds an empty-plan failure,
  and in every case moves the outcome's `ScenarioIdentity` onto it. With SC-C
  this gives SRC-4.

### Axioms

- AXIOM-1: derived `Clone` on `SourceLocation` and the handwritten text
  equality on `SourcePath` are value-preserving (the latter tested by 13.1.1's
  `one_path_has_one_identity_whatever_its_storage`).
- AXIOM-2: `execute_step` and `execute_step_async` return the error variants
  13.1.1's INV-16 classifies (undefined, missing fixture, assertion, panic),
  and the runner catches step panics, so a panicking step does not abort a
  proptest case.
- AXIOM-3: Rust's exhaustive-pattern checking rejects a `..`-free struct
  pattern, and a wildcard-free `match`, when the struct or enum gains a field
  or variant, inside the defining crate.
- AXIOM-4: `proptest` 1.11 (the locked version) generates from the declared
  strategies and honours `Config::rng_seed = RngSeed::Fixed(seed)`; acceptance
  is checked by classification counters, not assumed.
- AXIOM-5: a current-thread `tokio` runtime drives `run_scenario_async` to
  completion in tests (the future is not `Send`; roadmap 13.3.1 note).

### Obligations, methods, and evidence

- Obligation SRC-1, SRC-2, SRC-3, SRC-5, SRC-6, SRC-9 on executed runs.
  Method: parameterized `rstest` table. Rationale: the relevant shape space is
  finite — terminal kind {none, skip permitted, skip forced, undefined step,
  missing fixture, assertion error, panic} × runner {sync, async} × terminal
  location {line and column, line only, absent} — and every cell is enumerable
  (42 rows). Artefact: `crates/rstest-bdd/tests/runner_sources.rs` with
  `runner_sources/{steps,cases}.rs`. Evidence:
  `cargo nextest run -p rstest-bdd -E 'binary(runner_sources)'`. Rows that read
  only existing accessors pass at EP-M1; rows that read `step_source()`,
  `skip()`, or scenario identity are red at EP-M2's recorded red run and green
  after. Non-vacuity: each row asserts the terminal status and `FailureKind`
  before reading any location, so a row cannot pass without reaching its
  terminal; lines are `100 + 10 * i` and columns `3 + i`, so a record carrying
  a neighbour's location fails; the sources are `notes/demo.md`,
  `spec/cases.toml`, and `urn:example:doc-7`, none of which a Gherkin path
  could produce; the SRC-9 row keys a test-owned side table of byte ranges by
  the failure's index and asserts it selects the failing step's range.
- Obligation SRC-4. Method: `rstest` tables. Artefacts: `runner_sources.rs`
  (executed: the three failure kinds × terminal location present or absent ×
  scenario line present or absent) and
  `crates/rstest-bdd/src/runner/tests/outcome/fold_source.rs` (unit, new, built
  from crate-private constructors). Evidence: the command above plus
  `cargo nextest run -p rstest-bdd -E 'test(/runner::tests::outcome/)'`.
  Non-vacuity: the forced-skip rows assert `forced_failure()` before reading
  the location; the empty-plan rows assert `FailureSite::EmptyPlan`; a unit row
  in which the record's copy and the failure's copy disagree pins which copy
  `terminal_source()` and `step_source()` read.
- Obligation SRC-1..SRC-5, SRC-9, SRC-10 over sequences. Method: `proptest`,
  extending the existing suite. Rationale: where the terminal falls, how many
  steps are bypassed, and which steps lack a location are open-ended. Domain:
  the existing `case()` strategy's plans over its nine step kinds, each step
  additionally given a location shape drawn uniformly from {absent, line only,
  line and column} with distinct per-index values (as above), a scenario line
  drawn from {absent, present}, and `fail_on_skipped` and `allow_skipped` drawn
  from `{false, true}` so forced skips are generated. Boundary values (`1`,
  `u32::MAX`) are covered by the SRC-8 table rather than the generator, so
  neighbouring steps never collide. Artefact:
  `crates/rstest-bdd/tests/runner_sequence_props/sources.rs` and
  `runner_sequence_props/sequence/located.rs`, using the existing `CASES`
  budget and a fixed seed (`RngSeed::Fixed`). Evidence: the property-suite
  command in `Concrete steps`, printing classification counts. Non-vacuity:
  under the fixed seed the counts are deterministic, so the floors are exact:
  every terminal kind, every location shape at the terminal, forced and
  permitted skips, empty plans, and terminal positions first, middle, and last
  must each occur at least three times. Before writing the floors, EP-M3 runs
  the strategy with counting only and records the observed distribution in
  `Artefacts and notes` with the arithmetic (for example: each step's shape is
  independent with probability one third, so among the `T` cases with a
  terminal step about `T / 3` have each terminal shape); a class below its
  floor is a generator defect to fix, not a floor to lower. Negative controls,
  in `runner_sequence_props/source_controls.rs`, follow the existing
  `controls.rs` pattern: the property checker runs on a test-owned projection
  (per-record locations, terminal index, step-level failure location, scenario
  identity), and each control corrupts one field of a projection taken from a
  real run and must be rejected: (a) the failure's step location set to `None`
  while the terminal record's is `Some`; (b) one record given its neighbour's
  location; (c) a column dropped; (d) a folded failure given the wrong scenario
  line.
- Obligation SRC-7. Method: compile-time tripwire plus decoy tests.
  Artefacts: `crates/rstest-bdd/src/execution/error/shape_tests.rs` (new) and
  the existing decoy tests in `crates/rstest-bdd/src/runner/tests/source/`
  extended with a folded-failure row and a no-step-location row. Evidence:
  `make lint` and `make test` compile the tripwire; the decoy rows fail if
  either `Display` reads the error's path. Non-vacuity: during EP-M1, add a
  dummy field to `ExecutionError::StepNotFound` and a dummy variant to
  `StepError` in a scratch commit, observe both tripwire compile failures,
  record the error text in `Artefacts and notes`, then drop the scratch commit.
- Obligation SRC-8. Method: `rstest` table on executed runs. Artefact:
  `runner_sources.rs` row family `paths_and_columns_are_preserved_verbatim`.
  Non-vacuity: the table includes a path ending `.feature`, which a normalizing
  implementation would be most tempted to treat specially, and a line containing
  `é` and an emoji before the step text, with a column the test computes by
  `chars().count()`, so a byte-counting implementation would disagree.
- Snapshot. Method: `insta`. The existing `Display` snapshot is the
  multi-variant output-format evidence; it must pass unchanged (see `Risks` for
  why). No new snapshot is added (D12).
- Mutation evidence. Method: `cargo-mutants`, out-of-tree, scoped with
  `--file` to `crates/rstest-bdd/src/runner/outcome/**` and
  `crates/rstest-bdd/src/runner/engine/{drive,policy}.rs`, using nextest with a
  test filter limited to `runner_sources`, `runner_sequence_props`, and the
  `runner::` unit tests, so no mutant runs the slow trybuild or nested-cargo
  binaries. Confirm the exact flags against the installed `cargo-mutants`
  version before running. Evidence: no surviving mutant in any function this
  plan touches, or each survivor explained in `Decision log`.

Formal verification: not used (D7). Residual gap: SC-A..SC-D are argued by
inspection and evidenced by tests, not checked by a prover; the property suite,
the non-interference property, and the negative controls are the evidence that
the composition holds.

## Plan of work

### Stage A: understand and propose (this document)

No code changes. Ends with user approval.

### Stage B: characterization and tripwire (EP-M1, green)

Pin what already works on executed runs, so the refactor cannot regress it
silently. In `crates/rstest-bdd/tests/runner_sources/steps.rs`, register steps
for each behaviour: a passing step, a value-returning step, a step that calls
`rstest_bdd::skip!`, a step that returns `Err`, a step that panics, and a step
that requires a fixture (taken by reference) the context does not provide.
Leave one step text unregistered for the undefined-step row. In
`runner_sources/cases.rs`, build plans with
`ScenarioPlanBuilder::new(name, path)` and
`.step(StepInvocation::new(keyword, text).at(SourceLocation::new(path, line, column)))`,
using the three non-`.feature` sources. In `runner_sources.rs`, write the
tables that run each plan through `run_scenario` and, on a current-thread
`tokio` runtime, through `run_scenario_async`, passing the skip policy with
`ScenarioScope::with_skip_policy`, and asserting SRC-1, SRC-2 (via
`terminal_source()` and `skip().source()`), SRC-6, and SRC-8. Use `googletest`
matchers and `pretty_assertions` per the developers' guide house style.

Add the tripwire `crates/rstest-bdd/src/execution/error/shape_tests.rs`, wired
with `#[cfg(test)] mod shape_tests;`, and run the scratch non-vacuity check
described in `Verification plan`.

Validation: the new tests pass; `make check-fmt`, `make lint`, and `make test`
pass; commit.

### Stage C: implementation (EP-M2, red recorded, then green)

If either `outcome/mod.rs` or `outcome/failure.rs` would exceed 400 lines,
first commit the pure refactor that moves `SkipPolicyRecord`, `SkipRecord`, and
`ScenarioSkip` to `crates/rstest-bdd/src/runner/outcome/skip.rs`, re-exported
unchanged, with all gates green.

Red: add the rows that read the new surface — folded step failure keeps its
step location (S5), folded forced skip keeps its location and skip (S6), folded
empty plan names its document (S7), `failure().step_source()` on an unfolded
outcome (S3), scenario identity on outcomes and failures (S8), the SRC-9
join-key row — plus the behavioural feature and its bindings, and the unit file
`runner/tests/outcome/fold_source.rs`. Make them compile with local accessor
stubs that return `None` or an empty path. Run
`cargo nextest run -p rstest-bdd -E 'binary(runner_sources)'` and record the
failure transcript; every failure must be an assertion about a missing or wrong
location (Evidence tolerance). Do not commit this state.

Green, in one commit with the red tests:

1. `source.rs`: add the crate-private `ScenarioIdentity` with `path()` and
   `line()`; place its `of(&ScenarioPlan)` constructor in `engine/drive.rs` if
   that keeps `source.rs` free of a plan dependency. `plan.rs`: add a
   crate-private accessor returning the plan's `&SourcePath`.
2. `engine/policy.rs`: add `source: Option<SourceLocation>` to
   `Terminal::Fail`; give `assemble` a `ScenarioIdentity` parameter; build the
   outcome with it and the step failure with its location.
3. `engine/drive.rs`: set the new `source` in `record_step`'s failure arm
   from `invocation.source().cloned()`, mirroring the skip arm.
4. `engine/drive_sync.rs` and `engine/drive_async.rs`: build the identity
   once with the shared helper and pass it to `assemble` (no decision added).
5. `outcome/failure.rs`: replace the enum with the opaque struct and private
   `FailureRepr`; add crate-private constructors taking every field explicitly;
   implement `site()`, `error()`, `skip()`, `step_source()`, `scenario_path()`,
   `scenario_line()`, `render_with`, `Display`, and
   `std::error::Error::source()` (unchanged semantics: the `ExecutionError` for
   a step failure, otherwise `None`).
6. `outcome/mod.rs`: store the identity; add `scenario_path()` and
   `scenario_line()`; move the identity onto the failure in
   `into_harness_result`; reimplement `terminal_source()` as the skip's source
   or else the failure's `step_source()`.
7. Update every constructor and pattern found by
   `rg -n 'ScenarioFailure::|ScenarioOutcome::new|assemble\(' crates docs`,
   including `reporting/conversion.rs` (doc comment per D4) and its tests (add
   the identity-equality assertion), the unit tests under `runner/tests/` and
   `runner/engine/policy_tests/`, and the four doctests named in `Risks`. Every
   updated step failure carries its record's own location.
8. Remove the stubs (they were never committed).

Validation: the red rows and the behavioural scenarios pass; the `Display`
snapshot is unchanged; the focused `--no-default-features` loop passes;
`make check-fmt`, `make lint`, and `make test` pass; re-measure scope; commit.

### Stage D: evidence, documentation, and close (EP-M3, EP-M4)

EP-M3 adds the location dimension, the source properties, the non-interference
property, and the negative controls to the property suite; records the
classification distribution and floors; and runs the scoped mutation sweep.
EP-M4 updates documentation and the roadmap (see `Milestones and plateaus`).

## Behavioural specification

File: `crates/rstest-bdd/tests/features/runner_sources.feature`, bound in
`crates/rstest-bdd/tests/runner_sources.rs`. The feature file is Gherkin
because the repository's behavioural tests are; the plans its steps build are
sourced from non-`.feature` documents.

```gherkin
Feature: Frontend-supplied source identity survives every terminal outcome

  Scenario: A failing step keeps its line and column after folding
    Given a plan named "toml failure" sourced from "spec/cases.toml"
    And the plan has a passing step at line 8 column 3
    And the plan has a failing step at line 9 column 5
    And the plan has a passing step at line 10 column 3
    When the plan is executed synchronously
    And the outcome is folded for the harness
    Then the failure reports "spec/cases.toml" at line 9 column 5
    And step 2 is recorded as bypassed at line 10 column 3

  Scenario: A forced skip keeps its location after folding
    Given a plan named "forced skip" sourced from "notes/demo.md"
    And the plan has a skipping step at line 21 column 1
    And skipped steps fail the suite
    When the plan is executed asynchronously
    And the outcome is folded for the harness
    Then the failure reports "notes/demo.md" at line 21 column 1

  Scenario: An undefined step reports where it was written
    Given a plan named "undefined" sourced from "urn:example:doc-7"
    And the plan has an undefined step at line 4 column 2
    When the plan is executed through both runners
    Then both outcomes report "urn:example:doc-7" at line 4 column 2

  Scenario: An empty plan names its own document
    Given a plan named "empty" sourced from "notes/empty.md" at line 30
    When the plan is executed synchronously
    And the outcome is folded for the harness
    Then the failure names "notes/empty.md" at scenario line 30
    And the failure reports no step location

  Scenario: A step without a location still names its document
    Given a plan named "unlocated" sourced from "notes/unlocated.md"
    And the plan has a failing step without a location
    When the plan is executed synchronously
    And the outcome is folded for the harness
    Then the failure names "notes/unlocated.md" with no scenario line
    And the failure reports no step location
```

Red evidence (recorded, not committed): before EP-M2's implementation,
scenarios 1, 2, 4, and 5 fail at their `Then` steps with assertions naming the
missing location or identity. Scenario 3 reads the unfolded outcome, which
already works, and is a characterization scenario. Green evidence: all five
pass after EP-M2 under
`cargo nextest run -p rstest-bdd -E 'binary(runner_sources)'`.

## Milestones and plateaus

### EP-M1: characterization tests and the error-shape tripwire

- Outcome: executed-run tests pin today's per-step and skip source behaviour
  with columns and non-`.feature` paths through both runners, and a
  compile-time tripwire guards the runtime error shape. No production code
  changes.
- Requirements: RM-13.1.2-S1 (partly), ADR18-VER7, ADR18-LOC (tripwire).
- Acceptance: `binary(runner_sources)` passes; the tripwire's scratch
  non-vacuity failures are recorded; gates pass.
- Conformance check:
  - S1, S2, S4 evidenced on executed runs.
  - Design unchanged; no production behaviour changed.
  - ADR-018 assumptions hold.
  - No public interface change.
  - No trust-boundary or persisted-format change.
  - Trace links current.
- Recovery: additive tests; revert the commit to retry.
- Remaining gaps: S3, S5-S8, SRC-4, SRC-9.
- Compatibility decision: none.

### EP-M2: opaque failure carrying step and scenario identity

- Outcome: every member of D1's set exposes its supplied identity;
  `ScenarioFailure` is opaque; neither `Display` reads the runtime error's path.
- Requirements: RM-13.1.2, ADR18-OUT-SRC, ADR18-DIAG, ADR18-LOC,
  ADR18-OPAQUE, ADR18-TR6.
- Acceptance: the recorded red transcript shows location assertions
  failing; all `runner_sources` rows and all five behavioural scenarios pass;
  the `Display` snapshot is unchanged; gates pass; the scope figure is recorded.
- Conformance check:
  - S1-S8 and SRC-1..SRC-9 satisfied on executed runs.
  - Design: drivers still decision-free; policy owns assembly.
  - ADR-018 assumptions hold; no location enters a runtime error.
  - Public change limited to `Interfaces and dependencies`.
  - No trust-boundary or persisted-format change.
  - Trace links current.
- Recovery: the structural commit is self-contained; revert it to return to
  EP-M1's plateau.
- Remaining gaps: generated-sequence evidence, mutation evidence,
  documentation.
- Compatibility decision: none — the runner is unreleased.

### EP-M3: property evidence, controls, and mutation sweep

- Outcome: the property suite discharges SRC-1..SRC-5, SRC-9, and SRC-10
  over generated plans with recorded deterministic floors and four rejected
  negative controls; the scoped mutation run shows no unexplained survivor.
- Requirements: RM-13.1.2-S1, ADR18-VER7.
- Acceptance: the property-suite command in `Concrete steps` passes and
  prints counts at or above the floors; the controls are rejected for the
  intended reason; gates pass; scope re-measured.
- Conformance check: as EP-M2, plus verification evidence recorded.
- Recovery: additive; revert to retry.
- Remaining gaps: documentation.
- Compatibility decision: none.

### EP-M4: documentation, roadmap, and close

- Outcome: documentation describes the contract and conventions; roadmap
  13.1.2 is marked done; this plan is marked `COMPLETE` with a retrospective.
- Documentation edits:
  - `docs/users-guide.md`, "Running a plan without the Gherkin frontend":
    supplying a column with `StepInvocation::at`; reading `step_source()`,
    `skip()`, `scenario_path()`, and `scenario_line()` after folding, and
    `terminal_source()` before; branching on `site()` now that
    `ScenarioFailure` is opaque; the one-based, scalar-value convention with
    a short example converting a byte offset into a column
    (`line_text[..byte].chars().count() + 1`); using the step index to reach
    a frontend's own richer source map; and the fact that rendered messages
    carry only a path until 13.3.1. Rewrite the existing paragraph at
    `users-guide.md` lines 2106-2111 that describes the old fallback.
  - `docs/rstest-bdd-design.md` §2.6.4: a "Source-neutral diagnostics"
    paragraph recording D2-D5 and D11 and citing ADR-018; §2.6's Stage 1
    paragraph (near line 1392) gains 13.1.2.
  - `docs/developers-guide.md`, runner section: runner outcome types carry
    source identity and runtime errors never do; the tripwire module and what
    to do when it fires; `ScenarioFailure` is opaque and `FailureSite` is the
    matchable projection; the `runner_sources` and property-suite commands.
  - `docs/roadmap.md`: tick 13.1.2 with a short "Shipped:" note; add to
    13.2.1 that the macro path should populate step sources, and that the
    promoted conversion must forward the failure's step location and treat
    the plan as its canonical scenario identity; add to 13.3.1 the
    byte-offset-to-column obligation and that rendered `line:column` arrives
    with the source-neutral Fluent message.
  - `docs/contents.md`: index this ExecPlan if execution plans are listed
    individually.
- Acceptance: `make fmt` (staging by path only), `make check-fmt`,
  `make lint`, `make test`, `make markdownlint`, `make nixie`, and
  `make -B spelling` pass.
- Conformance check: reconcile every discovery with ADR-018 and the design
  document; record any mechanical difference in `Decision log`.
- Recovery: documentation-only; revert to retry.
- Remaining gaps: none expected.
- Compatibility decision: none.

## Concrete steps

All commands run from the repository root,
`/home/leynos/.lody/repos/github---leynos---rstest-bdd/worktrees/91f5bf1d-cb2d-4ea1-b5e8-937782634136`.
Capture every gate through `tee`, for example:

```sh
make check-fmt 2>&1 | tee /tmp/check-fmt-rstest-bdd-13-1-2-preserve-frontend-supplied-source-identity.out
make lint 2>&1 | tee /tmp/lint-rstest-bdd-13-1-2-preserve-frontend-supplied-source-identity.out
make test 2>&1 | tee /tmp/test-rstest-bdd-13-1-2-preserve-frontend-supplied-source-identity.out
```

Run gates sequentially, never in parallel, and prefer delegating full gate runs
to the `scrutineer` agent. A gate log is evidence only for the revision
recorded at its start; record `git rev-parse HEAD` before and after each run,
and never edit a gate input while a gate runs. `make lint` aborts at its first
failing step, so a red run leaves later steps unverified.

Focused loops during development:

```sh
cargo nextest run -p rstest-bdd -E 'binary(runner_sources)'
cargo nextest run -p rstest-bdd -E 'binary(runner_sequence_props)' --no-capture
cargo nextest run -p rstest-bdd -E 'test(/runner::/)'
cargo nextest run -p rstest-bdd --no-default-features \
  -E 'binary(runner_sources) | binary(runner_sequence_props) | test(/runner::/)'
cargo test --doc -p rstest-bdd
python3 scripts/check_rs_file_lengths.py
```

Expected red transcript shape at EP-M2 (illustrative):

```plaintext
FAIL rstest-bdd::runner_sources a_folded_failure_keeps_its_step_location::case_3
  Expected: has value equal to SourceLocation { path: "spec/cases.toml", line: 110, column: Some(4) }
  Actual: None
```

Expected green at EP-M2: the same filter reports every case passed.

Scope re-measure at each milestone boundary, on committed state:

```sh
git fetch origin main
base="$(git merge-base HEAD origin/main)"
git diff --shortstat "$base" HEAD -- . \
  ':!docs/execplans/13-1-2-preserve-frontend-supplied-source-identity.md'
```

Net added lines are the reported insertions minus deletions.

## Validation and acceptance

The change is accepted when:

- `cargo nextest run -p rstest-bdd -E 'binary(runner_sources)'` passes all
  table rows and all five behavioural scenarios, and the recorded EP-M2 red
  transcript shows the new-surface rows failing with location assertions.
- The property-suite command in `Concrete steps` passes, prints
  classification counts at or above the recorded floors, and each negative
  control is rejected.
- The `Display` snapshot under
  `crates/rstest-bdd/src/runner/tests/source/snapshots/` is byte-for-byte
  unchanged.
- The tripwire's non-vacuity failures are recorded in
  `Artefacts and notes`.
- `make check-fmt`, `make lint`, and `make test` pass on every commit;
  `make markdownlint` and `make nixie` pass on every commit that touches
  Markdown.

Quality criteria:

- Tests: as above, plus the full workspace suite, including the
  `--no-default-features` leg.
- Verification: SRC-1..SRC-10 discharged as stated; no formal proof (D7).
- Lint: `make lint` clean, including Whitaker and the file-length check.
- Performance: no measurable change is expected; the added work is one
  `Option<SourceLocation>` clone on the failure path, which already clones the
  whole `ExecutionError`, and one `SourcePath` clone per run.
- Security: no new input surface; paths are not opened or interpreted.

## Idempotence and recovery

Every step is re-runnable. Tests and documentation are additive. The structural
change in EP-M2 is one commit that can be reverted cleanly. The tripwire
non-vacuity check is done in a scratch commit that is dropped with
`git reset --hard HEAD~1` only after confirming `HEAD` is that scratch commit
(`git log -1 --format=%s`). Never use a bare `git stash`.

## Artefacts and notes

Prior art, checked with Firecrawl on 2026-10-18:

- Cucumber messages define `Location` as a required one-based `line` and an
  optional one-based `column`, and test-step results refer to their source
  through identifiers (`testStepId` to `pickleStepId` to `astNodeIds` to
  `Location`) rather than embedding a location in the result or error
  (`https://github.com/cucumber/messages/blob/main/jsonschema/messages.md`).
  This matches `SourceLocation` and supports D11's index join key.
- SARIF 2.1.0 regions use one-based `startLine` and `startColumn` and make
  the column unit explicit through `run.columnKind` (`unicodeCodePoints` or
  `utf16CodeUnits`)
  (`https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html`, §3.14.27
  and §3.30). The runner's documented unit, Unicode scalar values, corresponds
  to `unicodeCodePoints`.
- The Language Server Protocol uses zero-based lines and characters with a
  negotiated position encoding (UTF-16 by default). A frontend that also serves
  an editor must convert; the runner does not, and must not, choose an editor
  convention.

Records to be added during implementation: the tripwire non-vacuity transcripts
(EP-M1), the red transcript (EP-M2), the classification distribution, floors,
and arithmetic (EP-M3), and the mutation summary (EP-M3).

## Interfaces and dependencies

No new dependencies. At the end of EP-M2 the runner's public surface changes
only as follows:

```rust
// crates/rstest-bdd/src/runner/outcome/failure.rs
/// Opaque: read through accessors; branch on `site()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioFailure {
    scenario: ScenarioIdentity,
    repr: FailureRepr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FailureRepr {
    Step { index: usize, error: ExecutionError, source: Option<SourceLocation> },
    ForcedSkip(ScenarioSkip),
    EmptyPlan,
}

impl ScenarioFailure {
    pub fn site(&self) -> FailureSite;                      // unchanged meaning
    pub const fn error(&self) -> Option<&ExecutionError>;   // unchanged meaning
    pub const fn skip(&self) -> Option<&ScenarioSkip>;      // new
    pub fn step_source(&self) -> Option<&SourceLocation>;   // new
    pub fn scenario_path(&self) -> &str;                    // new
    pub const fn scenario_line(&self) -> Option<u32>;       // new
}
// `Display` and `std::error::Error` stay implemented; `Error::source()` still
// returns the `ExecutionError` of a step failure and `None` otherwise.

// crates/rstest-bdd/src/runner/outcome/mod.rs
impl ScenarioOutcome {
    pub fn terminal_source(&self) -> Option<&SourceLocation>; // unchanged meaning
    pub fn scenario_path(&self) -> &str;                      // new
    pub const fn scenario_line(&self) -> Option<u32>;         // new
}

// crate-private
pub(crate) struct ScenarioIdentity { path: SourcePath, line: Option<u32> }
pub(crate) enum Terminal {
    Skip { index: usize, message: Option<String>, source: Option<SourceLocation> },
    Fail { index: usize, error: ExecutionError, source: Option<SourceLocation> }, // source new
}
```

The public variants `ScenarioFailure::Step`, `ForcedSkip`, and `EmptyPlan`
cease to exist as public names. `ExecutionError`, `StepError`,
`MissingFixturesDetails`, `StepExecutionRequest`, `SourcePath`,
`SourceLocation`, `ScenarioPlan`'s public methods, `StepInvocation`,
`ScenarioPlanBuilder`, `StepOutcome`, `ScenarioSkip`, `FailureSite`,
`FailureKind`, `run_scenario`, and `run_scenario_async` are unchanged.

Planned files (34), counted for the Scope tolerance:

- Production (11): `runner/source.rs`, `runner/plan.rs`,
  `runner/outcome/mod.rs`, `runner/outcome/failure.rs`,
  `runner/outcome/skip.rs` (new, only if the split is needed),
  `runner/engine/policy.rs`, `runner/engine/drive.rs`,
  `runner/engine/drive_sync.rs`, `runner/engine/drive_async.rs`,
  `reporting/conversion.rs`, and `execution/error/mod.rs` (tripwire wiring).
- In-crate tests (9): `execution/error/shape_tests.rs` (new),
  `reporting/conversion/tests.rs`, `runner/engine/policy_tests/assemble.rs`,
  `runner/tests/outcome/mod.rs`, `runner/tests/outcome/rendering.rs`,
  `runner/tests/outcome/fold_source.rs` (new), `runner/tests/source/mod.rs`,
  `runner/tests/source/rendering.rs`, and `runner/tests/source/support.rs`.
- Integration tests (9): `tests/runner_sources.rs`,
  `tests/runner_sources/steps.rs`, `tests/runner_sources/cases.rs`,
  `tests/features/runner_sources.feature`,
  `tests/parser_neutral_runner/support.rs`, `tests/runner_sequence_props.rs`
  (module wiring), `tests/runner_sequence_props/sources.rs`,
  `tests/runner_sequence_props/source_controls.rs`, and
  `tests/runner_sequence_props/sequence/located.rs`.
- Documentation (5): `docs/users-guide.md`, `docs/rstest-bdd-design.md`,
  `docs/developers-guide.md`, `docs/roadmap.md`, and `docs/contents.md`.

All production and in-crate test paths are relative to
`crates/rstest-bdd/src/`; integration paths to `crates/rstest-bdd/`.

## Revision note

2026-10-18, revision 2: rewritten after the expert-panel review. Changed:
`ScenarioFailure` becomes opaque (D2); the step-level accessor is
`step_source()` to avoid shadowing `std::error::Error::source()` (D3); scenario
identity is carried on every outcome and failure and `terminal_source()` stays
step-level (D4); `Display` never reads the error (D5); the tripwire covers
every variant (D6); tests are consolidated into one new binary plus new modules
of the existing property suite (D8); red stages are recorded, never committed
(D10); the step index is documented as the join key (D11); the scope tolerance
is re-based on a counted file list. Effect on remaining work: EP-M1 is now
green-only, and EP-M2 carries the red run; no milestone has started.
