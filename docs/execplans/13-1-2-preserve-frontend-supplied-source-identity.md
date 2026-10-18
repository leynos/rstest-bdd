# Preserve frontend-supplied source identity in failed and skipped outcomes (13.1.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT — awaiting approval. No implementation may begin until the plan
is explicitly approved.

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
`feature_path` string, with no line and no column. An empty plan's failure
carries no identity at all, and the outcome does not record which scenario it
belongs to. Nothing executes a plan whose steps carry a column and checks that
the column comes back.

After this change, a frontend can execute a plan sourced from `notes/demo.md` or
`spec/cases.toml`, fold the outcome, and read from the failure the exact path,
one-based line, and one-based column it supplied for the step that failed or
skipped, without parsing any string and without the runtime's error types
learning anything about the frontend. The observable proof is a set of runtime
tests that execute such plans through both runners, across every failure and
skip path, and assert the supplied coordinates.

## Definitions

- **Frontend**: code that turns some authored document into a `ScenarioPlan`.
  The Gherkin macros are one frontend; Trymark (roadmap 13.3.1) is intended to
  be another.
- **Source identity**: the location a frontend supplied. At step level it is a
  `SourceLocation` (opaque path, one-based line, optional one-based column
  measured in Unicode scalar values). At scenario level it is the plan's
  `SourcePath` plus an optional one-based line.
- **Terminal step**: the step whose result stopped the run — a skip or a
  failure. Every later step is recorded as `Bypassed`.
- **Fold**: `ScenarioOutcome::into_harness_result`, which consumes an outcome
  and returns `Ok(())` or a `ScenarioFailure`. It is the one place skip policy
  (`fail_on_skipped`) and the empty-plan rule are applied.
- **Runtime error**: `rstest_bdd::execution::ExecutionError`, `StepError`, and
  `MissingFixturesDetails`. They were released in v0.6.0 and are not runner
  types.
- **Failed or skipped outcome surface**: one of the seven places, enumerated
  in D1, where a caller can observe a failure or a skip. "Every failed or
  skipped outcome" in the roadmap means every member of that closed set.

## Signposted documentation and skills

Read these before starting:

- `docs/adr-018-parser-neutral-scenario-execution.md`, especially "Structured
  outcome" (the binding source semantics) and "Source-neutral diagnostics" (the
  section this item discharges), plus verification item 7.
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
  linear reading: INV-7, D3, D14, D15, D21, D27, D46.

Skills to load: `rust-router`, then `rust-types-and-apis` (enum variant shape
and `#[non_exhaustive]`), `rust-unit-testing` (rstest tables, googletest,
pretty_assertions, insta), `proptest` (the source-fidelity property),
`rust-verification` (method selection), `nextest`, `execplans`, and
`en-gb-oxendict-style` for prose. Use `codegraph-mcp` for callers of
`ScenarioFailure`, `assemble`, and `Terminal`.

## Constraints

1. `ExecutionError`, `StepError`, `MissingFixturesDetails`,
   `StepExecutionRequest`, and every other type released in v0.6.0 keep their
   exact field sets and signatures. No line, column, `SourceLocation`, or other
   frontend coordinate may be added to them. This is the roadmap's "without
   extending runtime errors with frontend metadata".
2. No source identity is ever read back out of an `ExecutionError` or derived
   by parsing any string, in accessors or in `Display` (ADR-018, "Structured
   outcome"; 13.1.1 INV-7 clause three).
3. The runner treats paths as opaque: no normalization, extension check,
   case folding, or separator rewriting. A `.feature` path is as opaque as a
   `.md` one.
4. No edits under `crates/rstest-bdd-macros/`. Macro migration is 13.2.1.
5. No edits to any Fluent `.ftl` file and no new message identifiers. The
   source-neutral message wording is carried by 13.3.1 (13.1.1 D15).
6. No new dependencies. `rstest`, `googletest`, `pretty_assertions`, `insta`,
   `proptest`, `serial_test`, `temp-env`, and `tokio` are already available to
   `rstest-bdd`'s tests.
7. Runner public types stay frontend-free: `crates/rstest-bdd/src/runner/`
   must keep passing the INV-11 token scan in
   `crates/rstest-bdd/src/runner/tests/surface.rs`.
8. Every Rust file stays at or below 400 lines; the shrink-only allowlist
   `scripts/rs-length-allowlist.txt` gains no entry.
9. The drivers (`engine/drive_sync.rs`, `engine/drive_async.rs`) gain no
   decision; anything that decides belongs in `engine/policy.rs` or
   `engine/drive.rs`'s shared helpers.
10. Any test that calls `run_scenario*` and resolves a step lives under
    `crates/rstest-bdd/tests/` (13.1.1 D21).

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, and escalate.

## Tolerances (exception triggers)

- Scope: the deliverable (all changed files except this ExecPlan) exceeds 25
  files or 2,000 net added lines. Re-measure at every milestone boundary
  against the post-rebase merge-base with
  `git diff --shortstat $(git merge-base HEAD origin/main) -- . ':!docs/execplans/13-1-2-preserve-frontend-supplied-source-identity.md'`,
  and record the figure and revision in `Progress`. (13.1.1 breached its
  tolerance silently because nothing re-measured; D27 there.)
- Interface: any change to a public item released in v0.6.0, or any new
  public runner item beyond those listed in `Interfaces and dependencies`,
  stops work for escalation.
- Representation: if preserving identity in a folded empty-plan failure
  cannot be done with the shape chosen in D4, stop and present options rather
  than inventing a new public type.
- Dependencies: any new crate, feature, or tool stops work.
- Iterations: a gate still failing after three focused fix attempts stops
  work.
- Time: any single milestone exceeding six hours of active work stops work.
- Ambiguity: if ADR-018 text admits two readings that lead to different
  public shapes, stop and present both.

## Risks

- Risk: `crates/rstest-bdd/src/runner/outcome/mod.rs` is 367 lines; adding
  scenario identity and its accessors may breach the 400-line cap. Severity:
  medium. Likelihood: high. Mitigation: split at the existing seam first — move
  `SkipPolicyRecord`, `SkipRecord`, and `ScenarioSkip` to a new
  `crates/rstest-bdd/src/runner/outcome/skip.rs` in a separate refactor commit
  before EP-M2's behavioural change. Check with
  `python3 scripts/check_rs_file_lengths.py`.
- Risk: marking `ScenarioFailure::Step` and `EmptyPlan` `#[non_exhaustive]`
  breaks doctests and integration tests that construct or exhaustively match
  them, because both compile as external crates. Severity: low. Likelihood:
  high. Mitigation: grep before the change
  (`rg -n 'ScenarioFailure::(Step|EmptyPlan)' crates docs`); constructors in
  in-crate unit tests are unaffected; rewrite external patterns with `..`.
- Risk: the INV-11 token scan rejects identifiers or string literals that name
  a frontend inside `src/runner/`, including its test tree. Severity: medium.
  Likelihood: medium. Mitigation: keep new runner-side code and tests free of
  frontend words; the compile-time error-shape tripwire (D6) lives under
  `crates/rstest-bdd/src/execution/`, outside the scanned tree, because it must
  name `feature_path`.
- Risk: tests that exercise `fail_on_skipped` race the process-wide override.
  Severity: medium. Likelihood: medium. Mitigation: pass the policy explicitly
  through `ScenarioScope`, as `crates/rstest-bdd/tests/skip_parity.rs` already
  does, and reserve `#[serial]` plus `temp-env` for any test that must observe
  environment resolution.
- Risk: the `Display` snapshot
  `crates/rstest-bdd/src/runner/tests/source/snapshots/rstest_bdd__runner__tests__source__rendering__the_display_projection_renders_every_variant.snap`
  churns. Severity: low. Likelihood: low. Mitigation: D5 keeps the rendered
  shape; in production the plan path and the error's flattened path are equal,
  so the snapshot must pass unmodified. A snapshot change is a signal to stop
  and explain, not to accept.
- Risk: the scope figure grows through test-file splitting forced by the line
  cap, as it did in 13.1.1. Severity: medium. Likelihood: medium. Mitigation:
  new test concerns go in new files from the start; re-measure at each
  milestone.
- Risk: 13.2.1 needs the macro path to populate step sources and the
  reporting conversion to learn failures; a shape chosen here could make that
  awkward. Severity: medium. Likelihood: low. Mitigation: the chosen shape only
  adds optional data to runner types; `reporting/conversion.rs` keeps working,
  and its tests are updated in the same commit.

## Progress

- [x] (2026-10-18) Branch renamed to
  `13-1-2-preserve-frontend-supplied-source-identity`; base `3f3752a6` (equal to
  `origin/main` at drafting time).
- [x] (2026-10-18) Reconnaissance by a Wyvern team: runner source flow,
  runtime-error metadata, 13.1.1 decisions, and documentation map.
- [x] (2026-10-18) Prior-art check (Cucumber messages, SARIF 2.1.0, Language
  Server Protocol) recorded in `Artefacts and notes`.
- [ ] Expert-panel review of this draft and revision.
- [ ] Approval of the plan by the user.
- [ ] EP-M1: characterization and red tests.
- [ ] EP-M2: structural change, green.
- [ ] EP-M3: property evidence and negative controls.
- [ ] EP-M4: documentation, roadmap, and close.

## Surprises & discoveries

- Observation: most of the per-step plumbing already exists. Every
  `StepOutcome` copies its invocation's `SourceLocation` through a single
  constructor, `StepOutcome::recorded` in
  `crates/rstest-bdd/src/runner/outcome/step.rs`, and `ScenarioSkip` already
  owns its own copy of the skipping step's location. Evidence: Wyvern
  reconnaissance and direct reading. Impact: 13.1.2 is a targeted closure of
  three gaps plus executed-run evidence, not a new subsystem.
- Observation: `engine::policy::Terminal::Skip` carries `source`, but
  `Terminal::Fail` does not, so the asymmetry between skip and failure starts
  in the engine, not in the fold. Evidence:
  `crates/rstest-bdd/src/runner/engine/drive.rs` (`record_step`) and
  `engine/policy.rs` (`assemble`). Impact: the fix belongs at the terminal,
  where the invocation is in hand.
- Observation: the runner was merged after the `v0.6.0` tag
  (`git merge-base --is-ancestor 1dfe6725 v0.6.0` is false). Impact: the
  runner's public types carry no compatibility obligation, so changing
  `ScenarioFailure`'s variants needs no shim (D2).

## Decision log

- D1. The closed set of failed or skipped outcome surfaces.
  Decision: "every failed or skipped outcome" means these seven surfaces, and
  each must expose the frontend-supplied identity that applies to it.

  1. S1, a `StepOutcome` whose status is `Failed`, `Skipped`, or `Bypassed`:
     `StepOutcome::source()` (exists).
  2. S2, the terminal skip on an outcome: `ScenarioSkip::source()` (exists).
  3. S3, the terminal failure on an outcome:
     `ScenarioOutcome::failure().and_then(ScenarioFailure::source)` (new).
  4. S4, `ScenarioOutcome::terminal_source()` (exists; reimplemented over S2
     and S3 so there is one route).
  5. S5, a folded `ScenarioFailure::Step`: `ScenarioFailure::source()` (new).
  6. S6, a folded `ScenarioFailure::ForcedSkip`: `ScenarioFailure::source()`,
     delegating to the carried `ScenarioSkip` (new accessor, existing data).
  7. S7, a folded `ScenarioFailure::EmptyPlan`: `ScenarioFailure::source()`
     returns the scenario's own location (new; D4).

  The outcome additionally records the scenario's identity
  (`ScenarioOutcome::scenario_source()` and `scenario_line()`), which applies
  to every outcome, including skipped ones. Rationale: naming the set makes
  "every" checkable and gives each member a test row. Rendered text is not a
  surface for coordinates (D5).
- D2. `ScenarioFailure::Step` carries the failing invocation's location.
  Decision: add `source: Option<SourceLocation>` to the `Step` variant, add
  `source` to `engine::policy::Terminal::Fail`, populate it in
  `engine::drive::record_step` from `invocation.source().cloned()` exactly as
  the skip arm already does, and move it through `assemble` unchanged. Mark the
  `Step` variant `#[non_exhaustive]`. Rationale: the location must survive the
  fold, which drops `steps`; the terminal is where the invocation is in hand.
  `#[non_exhaustive]` on the variant means downstream code must write `..` and
  cannot construct it, so a future field is not a breaking change — the very
  hazard the `outcome` module's own documentation warns about. Compatibility:
  none required; the runner is unreleased (see `Surprises & discoveries`).
  Alternative rejected: deriving the location at fold time from
  `steps[index]` — it would leave `failure()` on an unfolded outcome without a
  location and make S3 and S5 differ.
- D3. One accessor, `ScenarioFailure::source() -> Option<&SourceLocation>`,
  answers "where should this failure be reported?" for every variant: `Step`
  returns its own location, `ForcedSkip` returns the skip's, and `EmptyPlan`
  returns the scenario's (D4). Rationale: a reporter needs one call, not a
  `match`. `FailureSite` stays an index-only projection.
- D4. Scenario identity is recorded on the outcome and on the empty-plan
  failure. Decision: `assemble` receives the plan's `SourcePath` (cloned; a
  copy for `Static`, a reference-count increment for `Shared`) and its optional
  scenario line, stores them on `ScenarioOutcome`, and exposes them as
  `scenario_source() -> &str` and `scenario_line() -> Option<u32>`.
  `ScenarioFailure::EmptyPlan` becomes
  `#[non_exhaustive] EmptyPlan { source: Option<SourceLocation> }`, populated
  by the fold as the scenario path at the scenario line with no column when a
  line was supplied, and `None` otherwise. Rationale: an empty plan has no
  step, so the only identity a frontend supplied is the scenario's; without
  this an aggregate frontend that folds and collects failures loses the
  document entirely. The plan accepts no scenario column, so none can be
  preserved; adding one is an API extension 13.1.2 does not need. Open question
  for the review panel: whether a path-only empty-plan failure (scenario line
  absent) should still carry its path; the recommendation is no, because
  `SourceLocation` requires a line and the path remains on the unfolded outcome.
- D5. Rendering keeps its shape and stops reading the error. Decision:
  `ScenarioFailure`'s `Display` passes
  `self.source().map(SourceLocation::path)` to `render_with`, so a folded
  failure renders the plan-side path rather than the error's flattened copy;
  `ScenarioOutcome`'s `Display` delegates through the same accessor. Line and
  column are not added to rendered text. Rationale: Constraint 2 now holds for
  the folded failure too; rendering coordinates is a user-visible wording
  change that belongs with 13.3.1's source-neutral Fluent message (13.1.1 D15).
  The existing fallback to the error's own path remains only when no plan-side
  location exists.
- D6. "Runtime errors are not extended" is enforced, not merely observed.
  Decision: add an in-crate test module under
  `crates/rstest-bdd/src/execution/error/` that destructures
  `ExecutionError::StepNotFound`, `ExecutionError::HandlerFailed`, and
  `MissingFixturesDetails` exhaustively, without `..`. `#[non_exhaustive]` does
  not apply inside the defining crate, so adding any field there stops the
  build with a pointer to this plan. Rationale: a compile-time tripwire is the
  cheapest artefact that can fail when the constraint is broken.
- D7. No Verus and no Kani for this item. Rationale: every obligation is a
  value-preservation property of data plumbing — a location cloned from an
  invocation into a record, moved through a terminal and a fold. The state
  space that matters is finite at the level of shapes (variant × source present
  or absent × runner) and is covered exhaustively by `rstest` tables; the
  sequence dimension is covered by `proptest`. There is no introduced lemma of
  business logic whose guarantee needs unbounded reasoning, and the repository
  has no Verus or Kani setup; introducing one to restate `clone() == original`
  would be the artificial obligation the task warns against. The proof sketch in
  `Verification plan` is kept so that the structure of the argument is
  explicit and its lemmas map to tests.
- D8. Executed-run evidence lives in new integration targets rather than in
  near-cap files: `crates/rstest-bdd/tests/runner_sources.rs` (with a `#[path]`
  support directory), `crates/rstest-bdd/tests/runner_source_props.rs`, and the
  behavioural pair `crates/rstest-bdd/tests/parser_neutral_sources.rs` with
  `crates/rstest-bdd/tests/features/parser_neutral_sources.feature`. Rationale:
  `skip_parity.rs` (389 lines) and `runner_panics.rs` (370) have no room, and
  D21 requires integration tests for anything that resolves a step.
- D9. No ADR. Rationale: the change implements ADR-018's existing binding
  semantics ("Terminal `Skipped` and `Failed` outcomes carry `source`";
  "Locations are never added to `ExecutionError`") rather than altering them.
  The decisions above are recorded in `docs/rstest-bdd-design.md` §2.6.4 in a
  new "Source-neutral diagnostics" paragraph that cites ADR-018.

## Outcomes & retrospective

Not started. To be completed at each milestone and at close.

## Context and orientation

The runtime crate is `crates/rstest-bdd`. Its parser-neutral runner is the
module `crates/rstest-bdd/src/runner/`:

- `source.rs` defines `SourcePath` (an opaque path held as
  `Static(&'static str)` or `Shared(Arc<str>)`, compared by text) and
  `SourceLocation` (path, one-based `line: u32`, optional one-based
  `column: Option<u32>`; both constructors panic on a zero coordinate in every
  profile).
- `plan.rs` and `plan/builder.rs` define `ScenarioPlan` (name, tags,
  `source: SourcePath`, `source_line: Option<u32>`, steps, `allow_skipped`) and
  `StepInvocation` (keyword, text, optional doc string and table,
  `source: Option<SourceLocation>`). A step column can only be supplied with
  `StepInvocation::new(..).at(SourceLocation::new(path, line, Some(column)))`;
  `ScenarioPlanBuilder::step_at` takes a line only.
- `outcome/mod.rs` defines `ScenarioStatus`, `SkipPolicyRecord`, `SkipRecord`,
  `ScenarioSkip`, and `ScenarioOutcome` (status, steps, optional skip, optional
  failure), with `terminal_source()`, `into_harness_result()`, and a `Display`.
  `outcome/step.rs` defines `StepOutcome`; `outcome/failure.rs` defines
  `FailureKind`, `FailureSite`, and `ScenarioFailure` (`Step { index, error }`,
  `ForcedSkip(ScenarioSkip)`, `EmptyPlan`).
- `engine/drive.rs` holds helpers shared by both drivers: `request` (builds
  the `StepExecutionRequest`, copying the plan's path into the request's
  `feature_path` field), `record_step` (classifies a result and builds the
  terminal), and `bypassed`. `engine/policy.rs` holds `absorb`, `classify`,
  `Terminal`, `SkipPolicy`, and `assemble`. `engine/drive_sync.rs` and
  `engine/drive_async.rs` are the two loops.
- `tests/` holds in-crate unit tests, including INV-7's synthetic source tests
  in `tests/source/` and the INV-11 surface scan in `tests/surface.rs`.

Runtime errors live in `crates/rstest-bdd/src/execution/error/`.
`ExecutionError::StepNotFound` and `HandlerFailed` carry `feature_path` and
`scenario_name` strings; `MissingFixturesDetails` carries the same two plus the
step definition's own `step_location`. None carries a line or column.

The one-way, test-only conversion from runner outcome to the legacy reporting
record is `crates/rstest-bdd/src/reporting/conversion.rs`; its tests construct
`ScenarioFailure::Step` and must be updated with D2.

Integration tests that execute plans are in `crates/rstest-bdd/tests/`:
`parser_neutral_runner.rs` (behavioural, with
`tests/features/parser_neutral_runner.feature`), `completeness.rs`,
`skip_parity.rs`, `runner_panics.rs`, `runner_instrumentation.rs`,
`runner_cancel.rs`, `runner_wire.rs`, and the property suite
`runner_sequence_props.rs` with its `runner_sequence_props/` directory. Steps
are registered in each binary with the `#[given]`, `#[when]`, and `#[then]`
attribute macros.

## Conformance basis

- Roadmap: `docs/roadmap.md` item 13.1.2 at `3f3752a6` (RM-13.1.2). Success
  criterion RM-13.1.2-S1: "runtime tests execute plans whose sources are not
  `.feature` files and assert that supplied path, line, and column information
  remains available to the caller".
- ADR: `docs/adr-018-parser-neutral-scenario-execution.md` (Accepted,
  2026-07-13) at `3f3752a6`:
  - ADR18-OUT-SRC: "Every such outcome exposes
    `source() -> Option<&SourceLocation>`.
    Terminal `Skipped` and `Failed` outcomes carry `source` … alongside the
    terminal step index."
  - ADR18-LOC: "`SourceLocation` consists of a path, a one-based line, and an
    optional one-based column measured in Unicode scalar values. Locations
    are never added to `ExecutionError` and are never inferred by parsing
    error strings."
  - ADR18-DIAG ("Source-neutral diagnostics"): the runner pairs an
    `ExecutionError` with the invocation's source location through the
    structured outcome.
  - ADR18-TR6: the API permits frontends to retain richer source maps without
    forcing those maps into rstest-bdd's core types.
  - ADR18-VER7: "Execute plans whose source paths are not `.feature` files
    and prove outcomes preserve the supplied locations without Gherkin
    parsing."
- Design: `docs/rstest-bdd-design.md` §2.6.4 at `3f3752a6` (DES-2.6.4).
- Predecessor: `docs/execplans/13-1-1-add-parser-neutral-types.md`, INV-7
  (source fidelity), INV-5 (sync/async equality), INV-11 (surface purity), D15
  (Fluent wording deferred), D21 (integration-test rule).
- No Terms of Reference document exists for this phase.

Trace links:

```plaintext
RM-13.1.2 -> ADR18-OUT-SRC -> D2,D3 -> EP-M2 -> runner_sources::folded_step_failure_keeps_its_location
RM-13.1.2 -> ADR18-OUT-SRC -> D4 -> EP-M2 -> runner_sources::folded_empty_plan_reports_the_scenario_location
RM-13.1.2 -> ADR18-LOC -> D6 -> EP-M1 -> execution::error::shape_tests (compile-time tripwire)
RM-13.1.2 -> ADR18-LOC -> D5 -> EP-M2 -> runner::tests::source::rendering::the_folded_failure_renders_the_plan_path
RM-13.1.2-S1 -> ADR18-VER7 -> D8 -> EP-M1,EP-M2 -> runner_sources (table over failure kinds x runners)
RM-13.1.2-S1 -> ADR18-VER7 -> EP-M3 -> runner_source_props (SRC-1..SRC-5 over generated plans)
RM-13.1.2-S1 -> ADR18-DIAG -> EP-M1,EP-M2 -> parser_neutral_sources.feature
ADR18-TR6 -> Constraint 3 -> EP-M1 -> runner_sources::paths_are_preserved_verbatim
```

## Verification plan

### Abstract model

Model a plan as `P = (scn, inv[0..n))` where `scn = (path, line?)` and each
invocation `inv[i]` has `src(inv[i]) ∈ Option<Loc>`, with
`Loc = (path, line ≥ 1, column? ≥ 1)`. A run maps `P`, a handler behaviour per
step (pass, return, skip, fail by error, fail by panic, undefined, missing
fixture), and a resolved skip policy to an outcome
`O = (status, rec[0..n), skip?, failure?, scn')`. The fold maps `O` to `Ok(())`
or a failure `f`. The model never inspects `path`: it is an opaque value
compared by equality.

Preconditions: every `Loc` was built by `SourceLocation::new` or `new_static`,
so its coordinates are one-based (enforced by construction); the plan was built
by `ScenarioPlanBuilder`.

### Invariants

- SRC-1 (record fidelity): for every `i < n`,
  `src(O.rec[i]) = src(inv[i])` — path, line, and column — whatever
  `O.rec[i]`'s status.
- SRC-2 (terminal fidelity): if the run stopped at step `t`, then
  `O.terminal_source() = src(inv[t])`, and whichever of `O.skip().source()` or
  `O.failure().source()` is present equals `src(inv[t])`.
- SRC-3 (representation agreement): the outcome holds each terminal location
  twice — once in `rec[t]` and once in the skip or failure. The two copies are
  equal.
- SRC-4 (fold preservation): if `fold(O) = Err(f)` then: for a step failure or
  forced skip, `f.source() = O.terminal_source()`; for an empty plan,
  `f.source() = Some(Loc(scn.path, scn.line, None))` when `scn.line` is present
  and `None` otherwise.
- SRC-5 (scenario fidelity): `O.scenario_source() = scn.path` and
  `O.scenario_line() = scn.line`, for every status.
- SRC-6 (runner agreement): the synchronous and asynchronous runners produce
  equal outcomes, sources included (13.1.1 INV-5, re-asserted with columns).
- SRC-7 (no error extension, no read-back): the runtime error types' field
  sets are unchanged, and no accessor or `Display` takes a location from an
  `ExecutionError` when a plan-side location exists.
- SRC-8 (opacity): paths come back byte-identical, including paths ending in
  `.feature`, paths with `./` prefixes, backslashes, spaces, and non-ASCII
  text, and columns come back unchanged on lines containing multi-byte
  characters (the runner stores the frontend's count and never recomputes it).

### Lemmas (hypotheses about the proof structure)

- LEM-A (single constructor): every `StepOutcome` is built by
  `StepOutcome::recorded`, which clones `invocation.source()`. With INV-2 (one
  record per invocation, in order) this gives SRC-1.
- LEM-B (terminal carries its invocation's location): both arms of
  `record_step` that produce a `Terminal` set its `source` from the same
  `invocation` whose record they return. With LEM-A this gives SRC-3.
- LEM-C (assembly moves, never builds): `assemble` moves the terminal's
  `source` into `SkipRecord` or `ScenarioFailure::Step` unchanged and moves the
  scenario identity into the outcome unchanged. With LEM-B and INV-1/INV-2 (the
  terminal index is the stopping step) this gives SRC-2 and SRC-5.
- LEM-D (fold moves, or builds only from the outcome's own scenario
  identity): `into_harness_result` returns the stored failure as is, wraps the
  stored skip as is, or builds `EmptyPlan` from `scenario_source` and
  `scenario_line`. With LEM-C this gives SRC-4.

The lemmas are structural facts about four small functions; implementation may
show that LEM-B and LEM-C collapse into one statement.

### Axioms

- AXIOM-1: derived `Clone` on `SourceLocation` and the handwritten text
  equality on `SourcePath` are value-preserving (the latter tested by 13.1.1's
  `one_path_has_one_identity_whatever_its_storage`).
- AXIOM-2: `execute_step` and `execute_step_async` return the error variants
  13.1.1's INV-16 classifies (undefined, missing fixture, assertion, panic).
- AXIOM-3: Rust's exhaustive-pattern checking rejects a struct pattern without
  `..` when the struct or variant gains a field, inside the defining crate.
- AXIOM-4: `proptest` generates from the declared strategies; acceptance is
  checked by classification counters, not assumed.
- AXIOM-5: a current-thread `tokio` runtime drives `run_scenario_async` to
  completion in tests (the future is not `Send`; roadmap 13.3.1 note).

### Obligations, methods, and evidence

- Obligation SRC-1, SRC-2, SRC-3, SRC-5, SRC-6 on executed runs.
  Method: parameterized `rstest` table. Rationale: the relevant shape space is
  finite — terminal kind {pass-through, skip permitted, skip forced, undefined
  step, missing fixture, assertion error, panic} × runner {sync, async} ×
  location {present with column, present without column, absent} — and every
  cell is enumerable. Artefact: `crates/rstest-bdd/tests/runner_sources.rs` and
  `crates/rstest-bdd/tests/runner_sources/{steps,cases}.rs`. Evidence:
  `cargo nextest run -p rstest-bdd -E 'binary(runner_sources)'`; red before
  EP-M2 for the rows that read `ScenarioFailure::source()` or scenario
  identity, green after. Non-vacuity: each row asserts the terminal status and
  `FailureKind` first, so a row cannot pass by never reaching its terminal;
  lines are offset per step (`100 + i`) and columns differ per step so a record
  that copied a neighbour's location fails; the sources are `notes/demo.md`,
  `spec/cases.toml`, and `urn:example:doc-7`, none of which a Gherkin path
  could produce.
- Obligation SRC-4. Method: `rstest` table over the three failure variants ×
  location present or absent, on executed runs, plus a unit row for the empty
  plan with and without a scenario line. Artefact: `runner_sources.rs`
  (executed) and `crates/rstest-bdd/src/runner/tests/outcome/fold_source.rs`
  (unit, new). Evidence: the same commands plus
  `cargo nextest run -p rstest-bdd -E 'test(/runner::tests::outcome/)'`.
  Non-vacuity: the forced-skip row asserts `forced_failure()` is true before
  reading the location; the empty-plan row asserts `FailureSite::EmptyPlan`.
- Obligation SRC-1..SRC-5 over sequences. Method: `proptest`. Rationale: the
  sequence dimension (where the terminal falls, how many steps are bypassed,
  which steps lack a location) is open-ended. Domain: plans of 0..=8 steps,
  each with a behaviour drawn from {pass, return, skip, fail, panic} and a
  location drawn from {absent, line only, line and column}, with lines and
  columns spanning `1..=u32::MAX` (boundaries `1` and `u32::MAX` included by
  `prop_oneof!` weighting), and a scenario line drawn from {absent,
  `1..=u32::MAX`}. Artefact: `crates/rstest-bdd/tests/runner_source_props.rs`
  with `crates/rstest-bdd/tests/runner_source_props/`. Evidence:
  `cargo nextest run -p rstest-bdd -E 'binary(runner_source_props)' --no-capture`
  printing classification counts. Non-vacuity: classification counters must
  show every terminal kind, every location shape, empty plans, and terminal
  positions first, middle, and last, each above a stated floor (for example 5 %
  of cases); a counter below its floor fails the test. Negative controls, each
  a hand-built outcome fed to the same property checker, which must reject it:
  (a) a step failure whose `source` is `None` while `rec[t].source()` is
  `Some`; (b) a record carrying its neighbour's location; (c) a location whose
  column was dropped; (d) an empty-plan failure with the wrong line.
- Obligation SRC-7. Method: compile-time tripwire plus decoy tests. Artefact:
  `crates/rstest-bdd/src/execution/error/shape_tests.rs` (new) and the existing
  decoy tests in `crates/rstest-bdd/src/runner/tests/source/` extended with a
  folded-failure row. Evidence: `make lint` and `make test` compile the
  tripwire; the decoy row fails if `ScenarioFailure`'s `Display` reads the
  error's path. Non-vacuity: during EP-M1, temporarily add a dummy field to
  `ExecutionError::StepNotFound` in a scratch commit, observe the tripwire fail
  to compile, and record the error text in `Artefacts and notes`; then drop the
  scratch commit.
- Obligation SRC-8. Method: `rstest` table on executed runs. Artefact:
  `runner_sources.rs` row family `paths_are_preserved_verbatim`. Evidence: as
  above. Non-vacuity: the table includes a path ending `.feature`, which a
  normalizing implementation would be most tempted to treat specially, and a
  line containing `é` and an emoji before the step text with a column the test
  computed by `chars().count()`, so a byte-counting implementation would
  disagree.
- Snapshot. Method: `insta`. Artefact: a new snapshot
  `crates/rstest-bdd/tests/snapshots/runner_sources__source_report.snap`
  rendering, for each row of the SRC-1/2 table, a one-line
  `kind runner -> path:line:column` report that the test itself formats from
  the accessors. Rationale: it shows in one reviewable artefact that every
  failure kind reports the same shape of identity through both runners; it is
  formatted by the test, so it freezes no runtime wording. The existing
  `Display` snapshot must pass unchanged (D5).
- Mutation evidence. Method: `cargo-mutants`, scoped and out-of-tree, over
  `crates/rstest-bdd/src/runner/outcome/**` and
  `crates/rstest-bdd/src/runner/engine/{drive,policy}.rs`, run once at EP-M3.
  Evidence: no surviving mutant in any function this plan touches, or each
  survivor explained in `Decision log`.

Formal verification: not used (D7). Residual gap: the proof of LEM-A..LEM-D is
by inspection plus tests, not by a checker; the property suite and the negative
controls are the evidence that the composition holds.

## Plan of work

### Stage A: understand and propose (this document)

No code changes. Ends with expert-panel revision and user approval.

### Stage B: characterization and red tests (EP-M1)

First, pin what already works on executed runs, so the refactor cannot regress
it silently. In `crates/rstest-bdd/tests/runner_sources/steps.rs`, register
steps for each behaviour: a passing step, a value-returning step, a step that
calls `rstest_bdd::skip!`, a step that returns `Err`, a step that panics, and a
step that requires a fixture the context does not provide. Leave one step text
deliberately unregistered for the undefined-step row. In
`runner_sources/cases.rs`, build plans with
`ScenarioPlanBuilder::new(name, path)` and
`.step(StepInvocation::new(keyword, text).at(SourceLocation::new(path, line, column)))`,
using the three non-`.feature` sources. In `runner_sources.rs`, write the
table that runs each plan through `run_scenario` and, on a current-thread
`tokio` runtime, through `run_scenario_async`, and asserts SRC-1, SRC-2 (via
`terminal_source()` and `skip().source()`), SRC-6, and SRC-8. These rows should
pass on the current code; commit them as the green characterization baseline.

Add the compile-time tripwire
`crates/rstest-bdd/src/execution/error/shape_tests.rs` (wired with
`#[cfg(test)] mod shape_tests;`), and run the scratch non-vacuity check
described in the Verification plan.

Then write the red rows: folded step failure keeps its location (S5), folded
forced skip keeps its location (S6), folded empty plan reports the scenario
location (S7), `failure().source()` on an unfolded outcome (S3), and
`scenario_source()`/`scenario_line()` (SRC-5). To make them compile and fail
for the intended reason, add the accessor signatures with bodies that return
`None` or a placeholder in the same commit; the red run must show assertion
failures naming the missing location, not compile errors. Write the behavioural
feature (below) and its step bindings; its folded-failure scenarios are red for
the same reason.

Validation: the characterization rows pass; the red rows fail with assertion
messages; `make check-fmt` and `make lint` pass (the red rows are committed
with the red stage noted in the commit message and the accessor stubs
documented as such; `make test` is expected to fail only on those rows).

Go/no-go: proceed only if every red failure is an assertion about a missing or
wrong location.

### Stage C: implementation (EP-M2)

If `outcome/mod.rs` would exceed 400 lines, first commit the pure refactor that
moves `SkipPolicyRecord`, `SkipRecord`, and `ScenarioSkip` to
`crates/rstest-bdd/src/runner/outcome/skip.rs`, re-exported unchanged, with all
gates green.

Then, in one commit:

1. In `engine/policy.rs`, add `source: Option<SourceLocation>` to
   `Terminal::Fail`; give `assemble` the scenario identity as a parameter (a
   small crate-private struct holding the `SourcePath` and optional line, to
   keep the parameter list short); store it on the outcome; build
   `ScenarioFailure::Step { index, error, source }`.
2. In `engine/drive.rs`, set the new `source` in `record_step`'s failure arm
   from `invocation.source().cloned()`, mirroring the skip arm.
3. In `engine/drive_sync.rs` and `engine/drive_async.rs`, pass the plan's
   scenario identity to `assemble` (no decision added).
4. In `outcome/failure.rs`, add the `source` field and `#[non_exhaustive]` to
   `Step`, change `EmptyPlan` to
   `#[non_exhaustive] EmptyPlan { source: Option<SourceLocation> }`, implement
   `ScenarioFailure::source()`, update `site()`, `error()`, `render_with`, and
   `Display` per D5.
5. In `outcome/mod.rs`, add the scenario identity fields and the
   `scenario_source()` and `scenario_line()` accessors, build `EmptyPlan`'s
   location in `into_harness_result`, and reimplement `terminal_source()` as
   the skip's source or else the failure's source.
6. Update every constructor and pattern found by
   `rg -n 'ScenarioFailure::(Step|EmptyPlan)|ScenarioOutcome::new|assemble\(' crates`,
   including `reporting/conversion.rs` and its tests, the unit tests under
   `runner/tests/` and `runner/engine/policy_tests/`, and doctests.
7. Remove the red-stage stubs.

Validation: the red rows and the behavioural scenarios now pass; the existing
`Display` snapshot passes unmodified; `make check-fmt`, `make lint`, and
`make test` pass.

### Stage D: evidence, documentation, and close (EP-M3, EP-M4)

EP-M3 adds the property suite and its negative controls, the source-report
snapshot, and the scoped mutation run. EP-M4 updates documentation and the
roadmap (see `Milestones and plateaus`).

## Behavioural specification

File: `crates/rstest-bdd/tests/features/parser_neutral_sources.feature`, bound
by `crates/rstest-bdd/tests/parser_neutral_sources.rs`. The feature file is
Gherkin because the repository's behavioural tests are; the plans its steps
build are sourced from non-`.feature` documents.

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

  Scenario: An empty plan reports the scenario's own location
    Given a plan named "empty" sourced from "notes/empty.md" at line 30
    When the plan is executed synchronously
    And the outcome is folded for the harness
    Then the failure reports "notes/empty.md" at line 30 with no column
```

Red evidence: before EP-M2, scenarios 1, 2, and 4 fail at their `Then` step
with an assertion naming the missing location; scenario 3 passes (it reads the
unfolded outcome, which already works) and is kept as a characterization
scenario. Green evidence: all four pass after EP-M2 with
`cargo nextest run -p rstest-bdd -E 'binary(parser_neutral_sources)'`.

## Milestones and plateaus

### EP-M1: characterization, tripwire, and red tests

- Outcome: the repository contains executed-run tests that pin today's
  per-step and skip source behaviour with columns and non-`.feature` paths, a
  compile-time tripwire on the runtime error shape, and red tests for the three
  gaps, made compilable by documented stubs.
- Requirements: RM-13.1.2-S1 (partly), ADR18-VER7, ADR18-LOC (tripwire).
- Acceptance: `binary(runner_sources)` characterization rows pass; red rows
  fail with location assertions; the tripwire's scratch non-vacuity failure is
  recorded.
- Conformance check:
  - S1, S2, S4 evidenced on executed runs.
  - Design unchanged; no production behaviour changed apart from stubs.
  - ADR-018 assumptions hold.
  - No public interface change beyond the stubbed accessor signatures listed
    in `Interfaces and dependencies`.
  - No trust-boundary or persisted-format change.
  - Trace links current.
- Recovery: tests are additive; revert the commit to retry.
- Remaining gaps: S3, S5, S6, S7, SRC-5.
- Compatibility decision: none.

Note on the plateau: a red stage is not a coherent resting state. If work must
stop after EP-M1, the red rows are marked
`#[ignore = "13.1.2 red stage: see execplan EP-M1"]` in a follow-up commit so
`make test` is green; the marker is removed in EP-M2's first commit.

### EP-M2: failures, empty plans, and outcomes carry identity

- Outcome: every member of D1's set exposes its supplied identity; the folded
  failure renders the plan-side path; the stubs are gone.
- Requirements: RM-13.1.2, ADR18-OUT-SRC, ADR18-DIAG, ADR18-LOC.
- Acceptance: all `runner_sources` rows and all four behavioural scenarios
  pass; the `Display` snapshot is unchanged; `make check-fmt`, `make lint`,
  `make test` pass; the scope figure is re-measured and recorded.
- Conformance check:
  - S1-S7 and SRC-1..SRC-7 satisfied on executed runs.
  - Design: drivers still decision-free; policy owns assembly.
  - ADR-018 assumptions hold; no location enters `ExecutionError`.
  - Public change limited to `Interfaces and dependencies`.
  - No trust-boundary or persisted-format change.
  - Trace links current.
- Recovery: the structural commit is self-contained; revert it to return to
  EP-M1 (with the ignore markers reinstated).
- Remaining gaps: generated-sequence evidence, mutation evidence,
  documentation.
- Compatibility decision: none — the runner is unreleased.

### EP-M3: property evidence, snapshot, and mutation sweep

- Outcome: `runner_source_props` discharges SRC-1..SRC-5 over generated
  plans with recorded classification floors and four rejected negative
  controls; the source-report snapshot is committed; the scoped mutation run
  shows no unexplained survivor.
- Requirements: RM-13.1.2-S1, ADR18-VER7.
- Acceptance: the property-suite command in `Concrete steps` passes and
  prints counts above the floors; the four controls fail for the intended
  reason; gates pass.
- Conformance check: as EP-M2, plus verification evidence recorded.
- Recovery: additive; revert to retry.
- Remaining gaps: documentation.
- Compatibility decision: none.

### EP-M4: documentation, roadmap, and close

- Outcome: documentation describes the contract and conventions; roadmap
  13.1.2 is marked done; this plan is marked `COMPLETE` with retrospective.
- Documentation edits:
  - `docs/users-guide.md`, "Running a plan without the Gherkin frontend":
    show supplying a column with `StepInvocation::at`, reading
    `ScenarioFailure::source()` after folding, `terminal_source()`, and
    `scenario_source()`/`scenario_line()`; state the one-based,
    scalar-value convention and that rendered messages carry only the path.
  - `docs/rstest-bdd-design.md` §2.6.4: a "Source-neutral diagnostics"
    paragraph recording D1-D5 and citing ADR-018; §2.6 line 1392 paragraph
    gains 13.1.2.
  - `docs/developers-guide.md`, runner section: the convention that runner
    outcome types carry source identity and runtime errors never do, the
    tripwire module and what to do when it fires, and the
    `runner_sources`/`runner_source_props` gate commands.
  - `docs/roadmap.md`: tick 13.1.2 with a short "Shipped:" note; add to
    13.2.1 that the macro path should populate step sources so generated
    tests benefit.
  - `docs/contents.md`: index this ExecPlan if execution plans are listed
    individually.
- Acceptance: `make fmt`, `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie` pass; `make -B spelling` passes.
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
recorded at its start; record `git rev-parse HEAD` before and after each run.

Focused loops during development:

```sh
cargo nextest run -p rstest-bdd -E 'binary(runner_sources)'
cargo nextest run -p rstest-bdd -E 'binary(parser_neutral_sources)'
cargo nextest run -p rstest-bdd -E 'binary(runner_source_props)' --no-capture
cargo nextest run -p rstest-bdd -E 'test(/runner::/)'
cargo nextest run -p rstest-bdd --no-default-features -E 'test(/runner::/)'
cargo test --doc -p rstest-bdd
python3 scripts/check_rs_file_lengths.py
```

Expected red transcript shape at EP-M1 (illustrative):

```plaintext
FAIL rstest-bdd::runner_sources folded_step_failure_keeps_its_location::case_1
  Expected: has value equal to SourceLocation { path: "spec/cases.toml", line: 109, column: Some(5) }
  Actual: None
```

Expected green at EP-M2: the same filter reports every case passed.

Scope re-measure at each milestone boundary:

```sh
git fetch origin main
git diff --shortstat "$(git merge-base HEAD origin/main)" -- . \
  ':!docs/execplans/13-1-2-preserve-frontend-supplied-source-identity.md'
```

## Validation and acceptance

The change is accepted when:

- `cargo nextest run -p rstest-bdd -E 'binary(runner_sources)'` passes, and
  its folded-failure rows failed before EP-M2 with location assertions.
- `cargo nextest run -p rstest-bdd -E 'binary(parser_neutral_sources)'`
  passes all four scenarios; scenarios 1, 2, and 4 failed before EP-M2.
- `cargo nextest run -p rstest-bdd -E 'binary(runner_source_props)' --no-capture`
  passes with classification counts above their floors, and each negative
  control is rejected.
- The `Display` snapshot under
  `crates/rstest-bdd/src/runner/tests/source/snapshots/` is byte-for-byte
  unchanged.
- The tripwire's non-vacuity failure is recorded in `Artefacts and notes`.
- `make check-fmt`, `make lint`, and `make test` pass at the close of every
  milestone; `make markdownlint` and `make nixie` pass at EP-M2 and EP-M4.

Quality criteria:

- Tests: as above, plus the full workspace suite.
- Verification: SRC-1..SRC-8 discharged as stated; no formal proof (D7).
- Lint: `make lint` clean, including Whitaker and the file-length check.
- Performance: no measurable change is expected; the added work is one
  `Option<SourceLocation>` clone on the failure path and one `SourcePath` clone
  per run.
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
  optional one-based `column`, and test-step results refer to their source by
  identifier rather than embedding a location in the result or error
  (`https://github.com/cucumber/messages/blob/main/jsonschema/messages.md`).
  This matches `SourceLocation` exactly and supports pairing an error with a
  location through the outcome rather than inside the error.
- SARIF 2.1.0 regions use one-based `startLine` and `startColumn` and make
  the column unit explicit through `run.columnKind` (`unicodeCodePoints` or
  `utf16CodeUnits`)
  (`https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html`, §3.14.27
  and §3.30). The runner's documented unit, Unicode scalar values, corresponds
  to `unicodeCodePoints`; a SARIF-emitting frontend can therefore map a
  `SourceLocation` without conversion.
- The Language Server Protocol uses zero-based lines and characters with a
  negotiated position encoding (UTF-16 by default). A frontend that also serves
  an editor must convert; the runner does not, and must not, pick an editor
  convention.

Tripwire non-vacuity transcript: to be recorded during EP-M1.

## Interfaces and dependencies

No new dependencies. At the end of EP-M2 the following runner items exist (new
or changed items marked):

```rust
// crates/rstest-bdd/src/runner/outcome/failure.rs
#[non_exhaustive]
pub enum ScenarioFailure {
    #[non_exhaustive] // changed: variant now non-exhaustive
    Step {
        index: usize,
        error: ExecutionError,
        source: Option<SourceLocation>, // new
    },
    ForcedSkip(ScenarioSkip),
    #[non_exhaustive] // changed: was a unit variant
    EmptyPlan { source: Option<SourceLocation> },
}

impl ScenarioFailure {
    pub fn site(&self) -> FailureSite;                         // unchanged
    pub const fn error(&self) -> Option<&ExecutionError>;      // unchanged
    pub const fn source(&self) -> Option<&SourceLocation>;     // new
}

// crates/rstest-bdd/src/runner/outcome/mod.rs
impl ScenarioOutcome {
    pub fn terminal_source(&self) -> Option<&SourceLocation>;  // unchanged signature
    pub fn scenario_source(&self) -> &str;                     // new
    pub const fn scenario_line(&self) -> Option<u32>;          // new
}

// crates/rstest-bdd/src/runner/engine/policy.rs (crate-private)
pub(crate) enum Terminal {
    Skip { index: usize, message: Option<String>, source: Option<SourceLocation> },
    Fail { index: usize, error: ExecutionError, source: Option<SourceLocation> }, // source new
}
```

`ExecutionError`, `StepError`, `MissingFixturesDetails`, `StepExecutionRequest`,
`SourcePath`, `SourceLocation`, `ScenarioPlan`, `StepInvocation`,
`ScenarioPlanBuilder`, `StepOutcome`, `ScenarioSkip`, `FailureSite`,
`FailureKind`, `run_scenario`, and `run_scenario_async` are unchanged.
