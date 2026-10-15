# Typed harness-context extractors for step parameters (roadmap 12.2.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT — awaiting approval. Implementation must not begin until the plan
is explicitly approved.

## Purpose / big picture

A step definition that wants the object a harness adapter provides (for example
GPUI's `TestAppContext`, or a Tokio test context) currently has three
spellings, all of which end at the same reserved fixture key:

```rust,no_run
// v0.6.0 legacy spelling: the reserved key is spelled by hand.
#[when("the user presses Tab")]
fn press_tab(#[from(rstest_bdd_harness_context)] cx: &mut gpui::TestAppContext) {}

// v0.6.1 marker (roadmap 11.2.1): readable, but still an attribute on a
// plain reference whose meaning is invisible in the type.
#[when("the user presses Tab")]
fn press_tab(#[harness_context] cx: &mut gpui::TestAppContext) {}
```

After this change a step can say what it wants in its type, with no attribute
and no reserved key:

```rust,no_run
use rstest_bdd::{Harness, HarnessMut};

#[when("the user presses Tab")]
fn press_tab(mut cx: HarnessMut<gpui::TestAppContext>, world: &mut UiWorld) {
    cx.simulate_keystrokes("tab"); // `HarnessMut` dereferences to the context.
    world.presses += 1;
}

#[then("the window title is shown")]
fn title_is_shown(cx: Harness<gpui::TestAppContext>) {
    assert!(cx.test_function_name().is_some());
}
```

`Harness<T>` is a shared view of the harness context and `HarnessMut<T>` an
exclusive one. Both are thin wrappers over the guard types that ADR-012
introduced, so they coexist with other fixtures, including `&mut World`
fixtures, inside one step. The `#[harness_context]` marker remains the stable
attribute spelling for steps that prefer plain references, and this plan
declares it part of the v1 surface.

Success is observable in four ways:

1. A behavioural scenario in `crates/rstest-bdd/tests/harness_extractors.rs`
   passes. Its steps read and mutate harness context through `Harness<T>`,
   `HarnessMut<T>`, and `#[harness_context]`, and mutate world state in the
   same step as `HarnessMut<T>`. None of its source spells
   `rstest_bdd_harness_context`.
2. Macro unit, property, and snapshot tests in `rstest-bdd-macros` prove that
   `Harness<T>`, `HarnessMut<T>`, and the marker are each classified as the
   harness context with the right access mode, and that the generated wrapper
   is stable.
3. `trybuild` compile-fail fixtures prove that misuse (redundant marker,
   extractor behind a reference, extractor bound to a placeholder, conflicting
   harness borrows in one step, a foreign type that happens to be called
   `Harness`) yields a targeted compiler diagnostic.
4. `examples/gpui-counter` uses `Harness<T>` and `examples/tokio-reminders`
   keeps the marker, and both compile and pass under `make test`.

Roadmap entry 12.2.1 in `docs/roadmap.md` is the requirement of record. Its
finish line is: *"macro tests cover `Harness<T>`, `HarnessMut<T>`, and the
marker path without user-visible reserved-key spelling."*

## Constraints

Hard invariants. Violating one requires escalation, not a workaround.

- C1. The reserved key `rstest_bdd_policy::HARNESS_CONTEXT_FIXTURE`
  (`"rstest_bdd_harness_context"`) stays the internal wire format between
  scenario code generation and step wrappers (ADR-007 addendum). Extractors
  bind that key; they do not introduce a second storage location.
- C2. The legacy spelling `#[from(rstest_bdd_harness_context)]`, the
  parameter-named spelling, and the `#[harness_context]` marker keep compiling
  and keep their current runtime meaning, except where this plan's approved
  harness-borrow conflict check (EP-M4) turns a step that can never succeed at
  run time into a compile error. No deprecation warnings are added in this item.
- C3. No existing public signature changes: `StepContext` methods,
  `FixtureRef`, `FixtureRefMut`, `FixtureBorrowError`, `StepError`,
  `HarnessAdapter`, and `ScenarioRunRequest` are untouched. The ADR-007 closed
  set of `*_harness_context` wrapper methods on `StepContext` is not extended.
- C4. New public items are limited to `rstest_bdd::Harness`,
  `rstest_bdd::HarnessMut`, and one sealed, documentation-hidden extraction
  trait used by generated code (named in `Interfaces and dependencies`).
- C5. No new external dependency in any crate. `rstest-bdd-macros` must not
  gain a dependency on `rstest-bdd` (workspace dependency policy,
  `docs/developers-guide.md` "Workspace dependency policy").
- C6. No source file exceeds 400 lines (`scripts/check_rs_file_lengths.py`,
  run by `make lint`). `crates/rstest-bdd/src/context/mod.rs` (397 lines),
  `crates/rstest-bdd-macros/src/codegen/wrapper/arguments/mod.rs` (395),
  `.../args/classify/mod.rs` (381), and `.../args/classify/tests.rs` (385) must
  not absorb new code; new logic goes in new sibling modules.
- C7. Runtime diagnostics (Fluent `.ftl` messages in 20+ locales) are not
  changed by this item. New diagnostics are compile-time, English, emitted by
  the macros crate, matching the 11.2.1 precedent.
- C8. The workspace stays on the stable toolchain pinned in
  `rust-toolchain.toml` with `rust-version = "1.88"`. Only stable language
  features may be used (`#[diagnostic::on_unimplemented]` is stable since 1.78
  and already used in `crates/rstest-bdd/src/step_return.rs`).
- C9. Generated code must stay clean under the workspace Clippy configuration
  (pedantic warnings denied, `allow_attributes` denied) when expanded into
  workspace crates, and must not force users to add lint suppressions for an
  ordinary extractor step (see EP-M1).

## Tolerances (exception triggers)

- Scope: if implementation needs more than 35 changed files or more than
  1,800 net lines of Rust (excluding `.stderr`, `.snap`, and Markdown), stop
  and escalate.
- Interface: if any item outside C4 must become public, or any signature named
  in C3 must change, stop and escalate.
- Dependencies: if any new crate dependency (including a dev-dependency not
  already in the workspace manifest) is needed, stop and escalate.
- Lint posture: if EP-M1 shows that ordinary extractor steps cannot be made
  lint-clean without a user-visible `#[allow]` or `#[expect]`, stop and present
  the options recorded in EP-M1 before continuing.
- Behaviour change: if the EP-M4 conflict check rejects any step that exists
  in the workspace today and can succeed at run time, stop and escalate; that
  would falsify lemma L3.
- Iterations: if a milestone's focused tests still fail after three
  fix attempts, stop and escalate with the failing output.
- Time: if any single milestone takes more than one working day, stop and
  record why before continuing.
- Ambiguity: the interpretations recorded in the Decision log (D1, D2, D6)
  were chosen by the planner. If the approver rejects one, revise the plan
  before implementation.

## Risks

- R1. Clippy's pedantic `needless_pass_by_value` lint fires on by-value guard
  parameters that are only read, as Bevy users see with `Res<T>`
  ([rust-clippy#8940](https://github.com/rust-lang/rust-clippy/issues/8940),
  open). This workspace denies pedantic warnings, so `fn step(cx: Harness<T>)`
  in a test crate may fail `make lint`, and users with pedantic Clippy will see
  the same. Severity: high. Likelihood: high. Mitigation: EP-M1 prototypes the
  lint posture before any other code, measures whether the lint fires on a step
  function whose body only dereferences the extractor, and selects a mitigation
  from the options listed there.
- R2. Type-directed recognition is syntactic: a procedural macro sees tokens,
  not resolved types. A user type that happens to be named `Harness<T>` would
  be captured; `use rstest_bdd::Harness as H;` would not be recognized.
  Severity: medium. Likelihood: low. Mitigation: generated code routes every
  recognized parameter through the sealed extraction trait, so a foreign
  `Harness` fails to compile with an `on_unimplemented` message that names the
  `#[from(...)]` opt-out; renamed imports are documented as unsupported and
  fall back to ordinary fixture handling (D4).
- R3. Roadmap 11.2.2 (public prelude, plan in DRAFT on branch
  `11-2-2-public-prelude-execplan`) and 11.2.3 (harness-path diagnostics) edit
  `crates/rstest-bdd/tests/trybuild_macros.rs`, `crates/rstest-bdd/Cargo.toml`,
  and the users' guide. Severity: medium. Likelihood: medium. Mitigation: EP-M0
  rebases onto `origin/main` and re-reads both plans' status; if the prelude
  has landed, EP-M6 adds `Harness` and `HarnessMut` to it and to its parity
  gates; if not, the hand-off is recorded in this plan and in the 11.2.2 plan's
  open questions.
- R4. The 400-line cap (C6). Severity: low. Likelihood: high. Mitigation: new
  modules `args/classify/harness_extractor/` and
  `arguments/harness_extractor.rs` in the macros crate, and
  `context/extractors.rs` in the runtime crate.
- R5. Making harness-borrow conflicts a compile error (EP-M4) also applies to
  the legacy and marker spellings. Severity: medium. Likelihood: low.
  Mitigation: lemma L3 shows every rejected signature fails on every run, so no
  working program is broken; EP-M4 greps the workspace and the published
  examples for affected signatures before enabling the check.
- R6. `trybuild` `.stderr` snapshots drift across compiler versions. Severity:
  low. Likelihood: medium. Mitigation: keep diagnostics macro-emitted where
  possible (stable text), and keep the single `on_unimplemented` fixture's
  expected output minimal.
- R7. When the scenario has no harness, or the harness context has a
  different type, the existing runtime diagnostic still names
  `rstest_bdd_harness_context` (C7). Severity: low. Likelihood: certain.
  Mitigation: recorded as a residual gap in D6 and in
  `Outcomes & retrospective`; a follow-up roadmap note is proposed rather than
  widening this item.

## Progress

- [x] (2026-10-15) Branch renamed to
  `12-2-1-typed-harness-extractor-steps-annotations`; upstream configured as
  `origin/12-2-1-typed-harness-extractor-steps-annotations`.
- [x] (2026-10-15) Reconnaissance: macro classification pipeline, runtime
  `StepContext` and guard surface, documentation and ADRs, tests, examples, and
  sibling branches surveyed by a Wyvern team.
- [x] (2026-10-15) Prior-art research: axum extractors, Bevy `Res`/`ResMut`,
  the `elided_lifetimes_in_paths` lint, and rust-clippy#8940.
- [x] (2026-10-15) First draft of this plan.
- [ ] Community-of-experts design review and revision.
- [ ] Plan approved by the user.
- [ ] EP-M0 orientation and green baseline.
- [ ] EP-M1 lint-posture prototype.
- [ ] EP-M2 runtime extractor types.
- [ ] EP-M3 macro recognition and code generation.
- [ ] EP-M4 harness-borrow conflict check.
- [ ] EP-M5 compile tests, behavioural scenario, and examples.
- [ ] EP-M6 documentation, ADR-024, and prelude hand-off.
- [ ] EP-M7 roadmap tick, full gates, and pull request.

## Surprises & discoveries

- Observation: the wrapper maps every fixture-borrow failure to
  `StepError::MissingFixture`, even when the real cause is a type mismatch or
  an outstanding borrow. Evidence:
  `crates/rstest-bdd-macros/src/codegen/wrapper/arguments/fixtures.rs` emits
  `.borrow_ref(..)`/`.borrow_mut(..)` followed by
  `.ok_or_else(|| MissingFixture { .. })?`, and `StepContext::borrow_mut` is
  `try_borrow_mut(..).ok()` (`crates/rstest-bdd/src/context/mod.rs`). Impact:
  two `&mut` harness-context parameters in one step fail at run time with a
  misleading "missing fixture" error. EP-M4 turns that case into a compile
  error.
- Observation: the language server does not model harness context at all.
  Evidence: `crates/rstest-bdd-server/src/handlers/diagnostics/placeholder.rs`
  counts a parameter as a step argument only by placeholder name. Impact:
  extractor parameters are neutral for the server; no server change is planned
  (D8).

## Decision log

- D1. Decision: interpret "a stable attribute marker" in the roadmap entry as
  the existing `#[harness_context]` marker from 11.2.1, declared part of the v1
  surface, rather than a new attribute. Rationale: 11.2.1 shipped exactly that
  marker; adding a second marker would enlarge the surface 12.x is meant to
  shrink. Date/Author: 2026-10-15, planner. Requires approval.
- D2. Decision: reject "automatic" injection by type alone, that is, treating
  an unannotated `cx: &mut TestAppContext` as the harness context when no
  fixture named `cx` exists. Rationale: the step macro cannot know which
  harness a step will run under, so a run-time fallback would silently change
  meaning when a fixture of the same type is later added. Extractor types make
  the request explicit at the same cost as writing the type. Date/Author:
  2026-10-15, planner.
- D3. Decision: recognize extractors syntactically (final path segment
  `Harness` or `HarnessMut`, exactly one generic type argument, at most one
  lifetime argument), then route generated code through a sealed trait so that
  the compiler confirms the recognized type really is
  `rstest_bdd::Harness`/`HarnessMut`. Rationale: a procedural macro cannot
  resolve types; the classification must happen at expansion time because it
  chooses the fixture key. The sealed trait turns any misrecognition into a
  compile error with guidance rather than wrong behaviour. This follows the
  ADR-019 pattern of a syntactic fast path backed by a type-checked contract.
  Alternatives considered: a fully trait-directed design in which every
  by-value fixture goes through a `FromStepContext` trait (axum style) was
  rejected because the fixture key must be chosen by the macro, and a blanket
  impl for ordinary `Clone` fixtures would collide with the extractor impls.
  Date/Author: 2026-10-15, planner.
- D4. Decision: `#[from(name)]` on an extractor-shaped parameter opts out of
  extractor recognition and classifies the parameter as an ordinary fixture
  named `name`. Renamed imports (`use rstest_bdd::Harness as H`) are not
  recognized and are documented as unsupported. Rationale: gives owners of an
  unrelated `Harness<T>` fixture type a one-attribute escape hatch, and keeps
  recognition a pure function of the parameter's own tokens. Date/Author:
  2026-10-15, planner.
- D5. Decision: combining `#[harness_context]` with an extractor type, or
  taking an extractor behind a reference (`&Harness<T>`, `&mut HarnessMut<T>`),
  is a compile error with a targeted message. Rationale: one canonical spelling
  per intent; both forms are redundant or meaningless, and the 11.2.1 precedent
  rejects every marker combination. Date/Author: 2026-10-15, planner.
- D6. Decision: extractor borrow failures map to the same
  `StepError::MissingFixture` the marker path produces, and the missing-
  fixture pre-validation uses the same reserved-key metadata, so the extractor
  and marker paths are observationally equivalent (obligation O4). The reserved
  key therefore still appears in run-time diagnostics when a scenario has no
  harness (R7). Rationale: changing runtime diagnostics touches 20+ Fluent
  locales and the public `StepError` enum (C3, C7); the roadmap finish line
  concerns the authoring surface. A follow-up roadmap note will propose
  harness-aware run-time wording. Date/Author: 2026-10-15, planner. Requires
  approval.
- D7. Decision: add a compile-time harness-borrow conflict check that covers
  every harness-context spelling, not only extractors (EP-M4). Rationale: with
  ADR-012 the wrapper acquires all guards before calling the step, so
  conflicting harness borrows can never succeed (lemma L3). Rejecting them at
  compile time replaces a misleading run-time "missing fixture" error. Limiting
  the check to extractors would leave two spellings of the same request with
  different failure modes. Date/Author: 2026-10-15, planner.
- D8. Decision: no language-server change.
  Rationale: the server treats every non-placeholder parameter as a fixture,
  which is correct for extractors; the macro already rejects extractors bound
  to placeholders. Date/Author: 2026-10-15, planner.
- D9. Decision: no Verus or Kani artefact. The proof sketch in
  `Verification plan` is kept as the design argument, and its obligations are
  discharged by exhaustive bounded enumeration against a real `RefCell` oracle,
  property tests, and compile tests. Rationale: the only non-trivial invariant
  (O3) is a counting predicate over a two-letter alphabet whose semantic
  content is an axiom about `std::cell::RefCell` (A1) plus a structural
  property of generated code (L1). A Verus proof would have to assume A1 as a
  specification of `RefCell` and would then restate the predicate, which the
  policy in `AGENTS.md` and the user's brief forbid as a vacuous proof. Kani
  would add a new tool to a repository that has none for a check that plain
  exhaustive enumeration covers at the same bound. The 11.2.1 precedent made
  the same call. Date/Author: 2026-10-15, planner.
- D10. Decision: record the design in a new ADR-024, "Typed harness-context
  extractors", and add an `Amended by ADR-024` banner to ADR-007. Rationale:
  unlike 11.2.1's spelling change, this introduces type-directed parameter
  classification, new public types, and a compile-time borrow rule that applies
  to existing spellings; that is substantive under the documentation style
  guide. Date/Author: 2026-10-15, planner.

## Outcomes & retrospective

Not started. To be completed at each milestone boundary and at completion,
including reconciliation of discoveries with ADR-007, ADR-012, ADR-024, and
design document §2.7.6.5.

## Context and orientation

### What this repository is

`rstest-bdd` is a behaviour-driven development (BDD) framework for Rust built on
`rstest`. Users write Gherkin `.feature` files and annotate Rust functions with
`#[given]`, `#[when]`, and `#[then]`. The procedural macros in
`crates/rstest-bdd-macros` turn each annotated function into a registered *step
wrapper*: generated code that receives a `&StepContext`, pulls each parameter
out of it, and calls the user's function. `#[scenario]` and `scenarios!`
generate one test per scenario that builds the `StepContext` and runs the
matching steps.

### Terms of art used in this plan

- *Fixture*: a value stored in the `StepContext` under a string key, usually
  produced by an `rstest` fixture function and keyed by parameter name.
- *Harness adapter*: a type implementing
  `rstest_bdd_harness::HarnessAdapter`
  (`crates/rstest-bdd-harness/src/adapter.rs`) that runs a scenario inside a
  framework-specific environment and hands the scenario a value of its
  associated type `Context` (which must be `Any`, and therefore `'static`).
- *Harness context*: that `Context` value. Generated scenario code stores it
  in the `StepContext` under the reserved key `rstest_bdd_harness_context` using
  `insert_owned_harness_context`
  (`crates/rstest-bdd-macros/src/codegen/scenario/runtime/harness.rs`).
- *Guard*: `FixtureRef<'a, T>` or `FixtureRefMut<'a, T>`
  (`crates/rstest-bdd/src/context/guards.rs`), the ADR-012 borrow handles
  returned by `StepContext::borrow_ref`/`borrow_mut` (which take `&self`).
  Distinct fixtures can be borrowed mutably at the same time; the same fixture
  follows `RefCell` rules.
- *Extractor*: a by-value parameter type whose construction pulls data out of
  the `StepContext`, as in axum's handler extractors. Here: `Harness<T>` (a
  shared guard over the harness context) and `HarnessMut<T>` (an exclusive
  guard).
- *Classifier*: a function in
  `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/` that decides
  what a step parameter is (fixture, step argument, data table, doc string, or
  `#[step_args]` struct) and records it as an `Arg`.
- *Harness access mode*: how a parameter borrows the harness context:
  `Shared` (`&T`, owned `T` cloned from a shared borrow, or `Harness<T>`) or
  `Exclusive` (`&mut T` or `HarnessMut<T>`).

### The parameter classification pipeline

`extract_args` in
`crates/rstest-bdd-macros/src/codegen/wrapper/args/extract.rs` walks the step
function's parameters in order. For each parameter it runs, first match wins:

1. `#[step_args]` struct classification.
2. `classify_harness_context` (marker), deliberately before the placeholder
   short-circuit so the attribute is always stripped.
3. If the name is not a placeholder: data-table and doc-string classifiers.
4. `classify_step_or_fixture`: placeholder match makes a step argument;
   anything else becomes `Arg::Fixture { pat, name, ty }`, with `name` taken
   from `#[from(name)]` or the normalized parameter name.

The new extractor classifier slots in immediately after step 2 and before the
placeholder short-circuit, for the same reason the marker does.

Code generation for fixtures is in
`crates/rstest-bdd-macros/src/codegen/wrapper/arguments/fixtures.rs`
(`gen_fixture_decls`), which branches on the parameter type: `&mut T` uses
`borrow_mut::<T>` and `value_mut()`, `&T` uses `borrow_ref::<T>` with a
`&'static T` fallback, and any other type is cloned from `borrow_ref::<T>`.
Because `Harness<T>` is not a reference it would currently be treated as an
owned fixture named after the parameter and cloned, which is wrong on both
counts. Fixture requirement metadata (`FixtureRequirement { name, ty }`) is
emitted in `.../wrapper/emit/mod.rs` (`effective_fixture_type`), and the
runtime uses it in `crates/rstest-bdd/src/execution/fixtures.rs` to report
missing fixtures before a step runs, including a harness hint when the reserved
key is missing.

### File-length pressure

Line counts measured during reconnaissance: `context/mod.rs` 397,
`arguments/mod.rs` 395, `args/classify/mod.rs` 381, `args/classify/tests.rs`
385, `args/classify/fixture_or_step.rs` 352, `args/extract.rs` 326,
`args/mod.rs` 232, `arguments/fixtures.rs` 245, `emit/mod.rs` 284,
`context/harness.rs` 86, `context/guards.rs` 145. Re-measure in EP-M0 with
`python3 scripts/check_rs_file_lengths.py`.

### Skills and documentation to load before starting

- Skills: `rust-router` (then `rust-types-and-apis` for the extractor types
  and sealed trait, `rust-unit-testing` for the rstest, googletest, and insta
  work, `proptest` for EP-M3 and EP-M4), `execplans` for maintaining this plan,
  `codegraph-mcp` for callers and impact, `en-gb-oxendict-style` and
  `arch-decision-records` for ADR-024, `commit-message` for commits,
  `pr-creation` for EP-M7.
- Documents: `docs/adr-007-harness-context-injection.md` (wire format and the
  11.2.1 addendum), `docs/adr-012-guard-based-stepcontext-borrowing.md`
  (guards), `docs/adr-019-type-directed-step-return-classification.md`
  (syntactic fast path plus type-checked contract), `docs/rstest-bdd-design.md`
  §§2.7.6.4–2.7.6.5, `docs/developers-guide.md` ("Macro implementation: fixture
  classification and normalization", "The `#[harness_context]` classifier
  stage", "Assertion vocabulary", "Staging fixtures for trybuild"),
  `docs/users-guide.md` ("Harness adapter and attribute policy", Table 2),
  `docs/rust-testing-with-rstest-fixtures.md`, `docs/rust-doctest-dry-guide.md`
  (doctests for the new public types),
  `docs/complexity-antipatterns-and-refactoring-strategies.md` (keep the new
  classifier free of nested conditionals), `docs/gherkin-syntax.md` (feature
  file), `docs/rstest-bdd-language-server-design.md` (D8),
  `docs/documentation-style-guide.md` (ADR format), and the completed precedent
  `docs/execplans/11-2-1-annotate-parameters-with-harness-context.md` (its
  surprises section lists traps that recur here).

## Conformance basis

Upstream artefacts, read at `origin/main` commit `8737d88f`:

- RM-12.2.1: `docs/roadmap.md` entry 12.2.1, with finish-line clauses
  FL-1 (`Harness<T>` covered by macro tests), FL-2 (`HarnessMut<T>` covered),
  FL-3 (marker path covered), and FL-4 (those tests do not spell the reserved
  key).
- DD-2.7.6.5: `docs/rstest-bdd-design.md` §2.7.6.5, remaining candidate
  "typed harness-context extractors that hide the reserved fixture key".
- ADR-007 (Accepted 2026-03-03) and its 11.2.1 addendum: key-as-wire-format
  (C1), closed wrapper set (C3), marker semantics.
- ADR-012 (Accepted 2026-06-11): `&self` borrows, guard types, concurrent
  distinct mutable borrows; supplies axiom A2.
- ADR-019: precedent for syntactic classification backed by a type-checked
  contract and `on_unimplemented` (D3).
- Governing standards: `AGENTS.md`, `docs/documentation-style-guide.md`,
  `docs/developers-guide.md` workspace dependency policy.
- There is no Terms of Reference document for this phase; the roadmap entry
  is the requirement of record.

Trace links:

```plaintext
RM-12.2.1/FL-1 -> DD-2.7.6.5 -> ADR-024 -> EP-M2, EP-M3 -> O1, O2, O4, O5
  -> harness_extractor::tests, harness_extractors.rs scenario
RM-12.2.1/FL-2 -> DD-2.7.6.5 -> ADR-024 -> EP-M2, EP-M3, EP-M4 -> O2, O3, O4, O5
  -> borrow_plan tests, harness_extractors.rs scenario
RM-12.2.1/FL-3 -> ADR-007 addendum -> EP-M3, EP-M5 -> O2, O4 -> marker rows in the same suites
RM-12.2.1/FL-4 -> ADR-024 -> EP-M5 -> O6 -> reserved-key absence check
ADR-012 -> A2 -> L1, L3 -> EP-M4 -> O3
```

## Verification plan

### Abstract model

Fix one step function. Its signature is a finite sequence of parameters
`p_1 .. p_n`. Classification assigns each parameter a harness access mode
`a(p_i)` in `{None, Shared, Exclusive}`, where `None` means the parameter does
not touch the harness context. Let `S = |{i : a(p_i) = Shared}|` and
`X = |{i : a(p_i) = Exclusive}|`.

Execution of the generated wrapper is modelled as: for `i = 1..n`, acquire a
borrow of mode `a(p_i)` on the harness-context cell, keeping every acquired
guard alive; then call the user function; then drop all guards. The cell is a
`RefCell` whose state is a pair `(readers, writer)` with the usual transitions:
a shared acquisition succeeds iff `writer = false`; an exclusive acquisition
succeeds iff `readers = 0` and `writer = false`.

The admissibility predicate the classifier must implement is
`Compatible(S, X) := X = 0 or (X = 1 and S = 0)`.

### Axioms

- A1. `std::cell::RefCell` implements the transition rules above
  (`try_borrow` fails iff a mutable borrow is outstanding; `try_borrow_mut`
  fails iff any borrow is outstanding). Third-party (standard library)
  interface; not verified.
- A2. ADR-012: the harness context inserted by `insert_owned_harness_context`
  lives in exactly one `RefCell` per scenario run, and every guard taken by a
  previous step has been dropped before the next step's wrapper starts.
- A3. A procedural macro sees parameter types as tokens only; it cannot
  resolve paths or aliases. Hence recognition (O1) is a syntactic contract, and
  its soundness against real types is delegated to the sealed trait (O7).

### Lemmas (hypotheses about the eventual argument)

- L1 (hold-until-call). Every guard the wrapper acquires for parameter
  `p_i` is still alive when the user function is called. Structural property of
  `gen_fixture_decls` plus the new extractor declaration: guards are `let`
  bindings in the wrapper's top-level block and are moved into, or borrowed by,
  the call.
- L2 (order independence). For a multiset of borrow requests on one
  `RefCell`, whether all can be held at once does not depend on acquisition
  order. Follows from A1 by induction on `n`.
- L3 (characterization). Under A1, A2, and L1, the wrapper's harness
  acquisitions all succeed (given the context exists with the requested type
  and was inserted owned) iff `Compatible(S, X)`.
- Theorem T (soundness and completeness of the compile-time check). The
  classifier accepts a signature iff `Compatible(S, X)`; hence accepted
  signatures never fail with a harness-cell borrow conflict, and rejected
  signatures would fail on every run.

These are hypotheses: implementation may show L1 needs refinement (for example
if an async step's guard is moved into a future), in which case this section is
revised before continuing.

### Obligations

- O1 (recognition). Obligation: a parameter without `#[from]` is classified
  as an extractor iff its type is a path type whose final segment is exactly
  `Harness` or `HarnessMut` with angle-bracketed arguments consisting of
  exactly one type argument and at most one lifetime argument; it is then bound
  to the reserved key, keeps its written type, and records the inner type.
  Method: `rstest` table of named positives and negatives plus a `proptest`
  property over generated paths. Rationale: the input space (prefix segments,
  lifetime forms, inner types) is open-ended but cheap to generate. Domain:
  prefixes `[]`, `rstest_bdd::`, `::rstest_bdd::`, `rstest_bdd::prelude::`, a
  random module path; lifetime forms none, `'_`, `'a`; inner types from a small
  type grammar; negatives `Harnesses<T>`, `MyHarness<T>`, bare `Harness`,
  `Harness<A, B>`, `Harness<'a>`, `&Harness<T>`. Artefact:
  `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_extractor/{tests,prop_tests}.rs`.
  Evidence: `cargo test -p rstest-bdd-macros harness_extractor` fails before
  EP-M3 (module missing) and passes after. Non-vacuity: the property classifies
  every generated case with `prop_assert!` on both branches and records class
  counts with `proptest`'s `prop_oneof!` weights so both positives and
  negatives are drawn; seeded mutation — change the final segment comparison to
  `ends_with("Harness")` — must be rejected by the `MyHarness<T>` row.
- O2 (access-mode mapping). Obligation: `Harness<T>` maps to `Shared`,
  `HarnessMut<T>` to `Exclusive`, marker/legacy `&T` and owned `T` to `Shared`,
  marker/legacy `&mut T` to `Exclusive`, every other parameter to `None`.
  Method: `rstest` table (finite partition). Artefact: same test module as O1
  plus `.../args/classify/harness_extractor/borrow_plan.rs` tests. Non-vacuity:
  every row of the partition appears; swapping the two extractor arms must fail
  two rows.
- O3 (conflict check, theorem T). Obligation: the classifier rejects a
  signature iff `not Compatible(S, X)`, and accepted signatures run without a
  harness-cell conflict. Method: (a) exhaustive enumeration of every signature
  of length 0..=4 over the alphabet {ordinary fixture, `Harness`, `HarnessMut`,
  marker `&T`, marker `&mut T`, legacy `&T`, legacy `&mut T`, marker owned `T`}
  — 4,681 signatures — comparing the classifier verdict with an oracle that
  acquires the same borrows on a real `RefCell<u32>` and holds them
  simultaneously; (b) a `proptest` property for lengths up to 12 using the same
  oracle; (c) a behavioural test that the accepted mixed shape (`HarnessMut<T>`
  plus `&mut World`) runs. Rationale: exhaustive enumeration at bound 4 covers
  every material class (0, 1, 2+ exclusive; 0, 1, 2+ shared; interleavings),
  and L2 makes longer sequences reducible to counts; the oracle is the real
  `RefCell`, so the test checks the predicate against A1 rather than against
  itself. Artefact: `.../harness_extractor/borrow_plan.rs` (predicate) and its
  `borrow_plan/tests.rs`, `borrow_plan/prop_tests.rs`. Evidence: red before
  EP-M4 (predicate stub returns accept), green after. Non-vacuity: the
  enumeration asserts that both verdicts occur and that each of the classes
  `(X=0,S>0)`, `(X=1,S=0)`, `(X=1,S>0)`, `(X=2,S=0)` is reached; seeded mutation
  `X <= 1` (forgetting the shared conflict) must be rejected by the oracle on
  `[HarnessMut, Harness]`.
- O4 (observational equivalence of spellings). Obligation: for each context
  state in {absent; owned `T`; owned `U != T`; shared-inserted `T` via
  `insert_harness_context`; owned `T` with an outstanding exclusive guard; owned
  `T` with an outstanding shared guard} and each mode in {shared, exclusive},
  extracting through `Harness`/`HarnessMut` and through the marker wrapper code
  yields the same `Ok`/`Err` outcome and, on error, the same `StepError`
  variant. Method: `rstest` table over the 12 cells (finite partition,
  exhaustive). Artefact:
  `crates/rstest-bdd/tests/harness_extractor_equivalence.rs` driving two
  generated wrappers (one per spelling) against hand-built `StepContext`
  values. Non-vacuity: each cell carries an explicit expected outcome rather
  than only comparing the two spellings with each other. The expected column
  has four `Ok` cells (owned `T` in both modes, and shared mode for the
  shared-inserted and outstanding-shared states) and eight `Err` cells, so a
  pair of spellings that both always succeed or both always fail is rejected.
  Outstanding guards are real guards held across the call.
- O5 (transparency). Obligation: `Harness<T>` dereferences to the same
  object `borrow_harness_context::<T>()` would return (pointer equality), and
  writes through `HarnessMut<T>` are visible to the next step. Method: unit
  tests in `crates/rstest-bdd/src/context/extractors.rs` plus the behavioural
  scenario. Non-vacuity: the behavioural scenario asserts a value that only a
  successful write could produce.
- O6 (reserved key hidden from authors). Obligation: the new behavioural
  test, the new compile-pass fixture, and the migrated example steps do not
  contain the string `rstest_bdd_harness_context`. Method: a unit test that
  `include_str!`s those files and asserts absence. Non-vacuity: the same test
  asserts the string is present in a known legacy fixture
  (`tests/fixtures_macros/harness_context_coexist.rs`), proving the search
  works.
- O7 (foreign-type rejection). Obligation: a recognized parameter whose type
  is not `rstest_bdd::Harness`/`HarnessMut` fails to compile with the
  `on_unimplemented` message naming the `#[from(...)]` opt-out. Method:
  `trybuild` compile-fail fixture. Non-vacuity: the paired opt-out fixture
  (`#[from(harness)] harness: Harness<Foo>` with a local `Harness<T>` type)
  compiles and runs.
- O8 (metadata). Obligation: an extractor parameter contributes the reserved
  key to `Step::fixtures` and
  `FixtureRequirement { name: reserved key, ty: inner type }`, so
  missing-context pre-validation reports `has_suggestion = true`. Method:
  generated-wrapper unit test on the emitted tokens plus an
  `execution_error.rs`-style runtime test. Non-vacuity: the runtime test
  asserts the inner type name, which only the new metadata path produces.

### Where formal verification is and is not warranted

Verus is not warranted (D9). The obligations are either syntactic (O1, O2, O6,
O7), structural properties of generated code (L1, O8), equivalences over a
finite state partition that is enumerated exhaustively (O4), or a counting
predicate whose semantic content is the standard library's `RefCell` contract
(O3 via A1). A Verus development would have to take A1 as an assumed
specification of `RefCell` and would then prove only that `Compatible` equals
itself under that specification, which is the vacuous restatement the brief
rules out. Kani is not warranted either: the predicate is total over counts,
and exhaustive enumeration to length 4 explores the same space as a bounded
model check at that bound without adding a tool the repository does not use. If
implementation changes the design so that borrow acquisition depends on
run-time ordering or on state carried between steps, this decision is revisited
before continuing.

## Plan of work

Stages map to milestones. Each milestone ends green on `make check-fmt`,
`make lint`, and `make test` (run sequentially, delegated to the `scrutineer`
agent), and with a commit.

### EP-M0 — orientation and green baseline (no code changes)

Rebase onto `origin/main`. Re-read the 11.2.2 and 11.2.3 plans' status lines
(R3). Run the three gates and record the baseline. Re-measure file lengths.
Confirm A2 by reading `StepContext::try_borrow_mut` and the step-override
precedence (`values` map lookups in `crates/rstest-bdd/src/context/mod.rs`):
verify that no step-return override can be stored under the reserved key; if
one can, O3's oracle must model it and the plan is revised. Confirm that no
workspace step today has conflicting harness borrows
(`rg -n "harness_context" crates examples --type rust` and inspect every step
with two harness parameters).

Acceptance: baseline gate logs exist under `/tmp`, and this plan's
`Surprises & discoveries` records any drift.

### EP-M1 — prototype: lint posture of by-value extractor parameters

This is a prototyping milestone. Create a scratch crate outside the workspace
build graph (under `/tmp`, depending on the workspace crates by path) or a
temporary `#[cfg(test)]` module, defining a minimal `Harness<'a, T>` newtype
over `FixtureRef<'a, T>` and a function
`fn step(cx: Harness<'_, Ctx>) -> usize { cx.len() }`. Run `cargo clippy` with
the workspace's lint table. Record whether `needless_pass_by_value`,
`elided_lifetimes_in_paths` (when `rust_2018_idioms` is enabled), and
`allow_attributes` fire, both for a plain function and for a function annotated
with `#[given]`.

Candidate mitigations, to be chosen by evidence:

1. Clippy does not fire (for example because the wrapper type has a `Drop`
   implementation or because the macro re-spans the item). Keep the plain
   design.
2. The step attribute macro emits a narrowly scoped
   `#[expect(clippy::needless_pass_by_value, reason = "...")]` on the user
   function only when an extractor parameter is present and only on the
   parameters' function, if Clippy honours it on macro-emitted attributes
   without tripping `allow_attributes` or `unfulfilled_lint_expectations`.
3. Users are told to write `cx: &Harness<'_, T>`; rejected by D5 unless 1 and
   2 both fail.

Acceptance: a short transcript of the Clippy runs is pasted into
`Artefacts and notes`, the chosen option is recorded in the Decision log, and
the scratch code is deleted. If only option 3 remains, stop and escalate
(Tolerances).

### EP-M2 — runtime extractor types (red, green, refactor)

Red: add `crates/rstest-bdd/src/context/extractors.rs` with the O5 unit tests
and the O4 equivalence table in
`crates/rstest-bdd/tests/harness_extractor_equivalence.rs` (the macro-driven
half of O4 is completed in EP-M3; in this milestone the table drives
`Harness::try_from_context` against `StepContext::borrow_harness_context`). Run
`cargo test -p rstest-bdd harness_extractor` and observe compile failure
because the types do not exist.

Green: define `Harness<'a, T>` and `HarnessMut<'a, T>` as private-field
newtypes over `FixtureRef<'a, T>` and `FixtureRefMut<'a, T>`, with `Deref`
(both), `DerefMut` (mutable only), `AsRef`/`AsMut` as appropriate, and `Debug`;
constructors `try_from_context` returning `Result<Self, FixtureBorrowError>`
built on `StepContext::try_borrow` and `try_borrow_mut` with the reserved key
(not new `StepContext` methods, C3). Add the sealed extraction trait in a
`#[doc(hidden)]` module with `#[diagnostic::on_unimplemented]`. Re-export
`Harness` and `HarnessMut` from the crate root next to `FixtureRef`. Write
rustdoc with runnable examples following `docs/rust-doctest-dry-guide.md`.

Refactor: keep `context/mod.rs` untouched except for `mod extractors;` and the
re-export line; if that breaches 400 lines, move the re-export into
`context/harness.rs` instead.

Acceptance: `cargo test -p rstest-bdd extractors` passes; the O4 runtime table
passes for all 12 cells; gates green; commit.

### EP-M3 — macro recognition and code generation

Red: add
`crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_extractor/mod.rs`
(classifier, empty body returning `Ok(false)`), `tests.rs` (O1, O2 tables and
diagnostics for D5 and placeholder binding), `prop_tests.rs` (O1 property). Add
an insta snapshot test in
`crates/rstest-bdd-macros/src/codegen/wrapper/equivalence_tests.rs` (or a new
sibling `extractor_snapshot_tests.rs` if that file would exceed the cap)
pinning the generated wrapper for two steps: one that takes `HarnessMut<Ctx>`
and `&mut World`, and one that takes `Harness<Ctx>` and the marker `&Ctx`. Run
`cargo test -p rstest-bdd-macros harness_extractor` and observe failures.

Green:

1. Represent the classification without touching every `Arg::Fixture`
   construction site: add a `harness: Option<HarnessExtractor>` field to
   `Arg::Fixture` and `FixtureArg` (where `HarnessExtractor` holds the access
   mode and the inner type), set to `None` by existing classifiers. Expected to
   be the smallest diff because fixture-name collection and requirement
   metadata already iterate fixtures. If the field approach pushes
   `arguments/mod.rs` over 400 lines, move `bind_arguments` helpers into a
   sibling module first, as a separate refactor commit.
2. Implement `classify_harness_extractor` in the new module, called from
   `extract_args` immediately after `classify_harness_context`. It returns
   `Ok(false)` when `#[from]` is present (D4) or the type is not extractor
   shaped; errors per D5 and on placeholder binding; otherwise pushes
   `Arg::Fixture { name: HARNESS_CONTEXT_FIXTURE, ty: written type, harness:
   Some(..) }`.
3. Extend `classify_harness_context` to reject an extractor-shaped type
   under the marker (D5), sharing the shape predicate.
4. Add `arguments/harness_extractor.rs` with `gen_harness_extractor_decl`,
   emitting the declaration below, and dispatch to it from `gen_fixture_decls`
   when `harness` is `Some`:

   ```rust,ignore
   let binding: #ty = <#ty as #path::__macro_support::HarnessExtract<'_>>::extract(#ctx)
       .ok_or_else(|| #missing_err)?;
   ```

5. Make `effective_fixture_type` return the inner type for extractor
   fixtures (O8).

Acceptance: O1 and O2 tables and the O1 property pass; the snapshot is reviewed
and accepted with `cargo insta review`; the macro-driven half of the O4 table
passes; gates green; commit.

### EP-M4 — harness-borrow conflict check

Red: add `.../harness_extractor/borrow_plan.rs` exposing
`fn check_harness_borrows(args: &[Arg]) -> syn::Result<()>` with a stub that
accepts everything, plus `borrow_plan/tests.rs` (O3 exhaustive enumeration and
class-coverage assertions) and `borrow_plan/prop_tests.rs`. Run and observe
failures on conflicting signatures.

Green: implement the access-mode function (O2) and `Compatible(S, X)`; call
`check_harness_borrows` once from `extract_args` after the loop. The error
spans the second conflicting parameter and names both parameters and their
modes, for example:
`` conflicting harness-context borrows: `cx` takes it
mutably (HarnessMut<..>) and `view` takes it shared (Harness<..>); a step may
hold one mutable borrow or any number of shared borrows ``.

Run the seeded mutation (`X <= 1`) locally and confirm the oracle test fails;
revert the mutation. Record the transcript.

Acceptance: O3 enumeration and property pass; the mutation transcript is in
`Artefacts and notes`; gates green; commit.

### EP-M5 — compile tests, behavioural scenario, and examples

Compile-pass fixture
`crates/rstest-bdd/tests/fixtures_macros/harness_extractors_pass.rs`,
registered in `crates/rstest-bdd/tests/trybuild_macros.rs`, covering
`Harness<T>`, `Harness<'_, T>`, `rstest_bdd::HarnessMut<T>`, coexistence with
`&mut World`, the marker path, an `async fn` step, and the D4 opt-out.

Compile-fail fixtures in `crates/rstest-bdd/tests/ui_macros/`, each with a
`.stderr` file and registered as `UiFixtureCase` entries:
`harness_extractor_with_marker.rs`, `harness_extractor_by_reference.rs`,
`harness_extractor_on_placeholder.rs`, `harness_extractor_with_datatable.rs`,
`harness_borrow_conflict_mut_mut.rs`, `harness_borrow_conflict_mut_shared.rs`
(using one extractor and one marker parameter, proving the check spans
spellings), and `harness_extractor_foreign_type.rs` (O7).

Behavioural test `crates/rstest-bdd/tests/harness_extractors.rs` with
`crates/rstest-bdd/tests/features/harness_extractors.feature`:

```gherkin
Feature: Typed harness-context extractors
  Steps receive the harness context through Harness, HarnessMut, or the
  harness_context marker without naming the reserved fixture key.

  Scenario: Read and mutate the harness context through extractors
    Given the harness context starts at zero
    When a step increments the harness context through HarnessMut
    Then a step reading through Harness sees one increment
    And a step reading through the harness_context marker sees one increment

  Scenario: Mutate the harness context and world state in one step
    Given the extractor world starts empty
    When a step mutates the harness context and the world together
    Then the harness context and the world reflect the mutation
```

The harness is a local `CountingHarness` modelled on
`crates/rstest-bdd/tests/concurrent_mut_fixtures.rs`.

Examples: migrate `examples/gpui-counter/tests/counter.rs` to
`Harness<gpui::TestAppContext>`; keep `examples/tokio-reminders` on the marker
so both supported spellings stay demonstrated end to end. Add the O6 absence
test in `crates/rstest-bdd/tests/harness_extractors.rs`.

Acceptance: `cargo test -p rstest-bdd --test trybuild_macros` and
`cargo test -p rstest-bdd --test harness_extractors` pass;
`cargo test -p gpui-counter` and `cargo test -p tokio-reminders` pass; gates
green; commit.

### EP-M6 — documentation, ADR-024, and prelude hand-off

- Write `docs/adr-024-typed-harness-context-extractors.md` (Status Proposed
  until the PR merges, then Accepted) recording D3–D7 and D9, with options
  considered and the migration impact. Add an `Amended by ADR-024` banner and
  `## Amendments` entry to ADR-007.
- `docs/rstest-bdd-design.md` §2.7.6.5: mark the typed-extractor candidate
  delivered, summarize the recognition rule, the sealed trait, and the
  borrow-conflict rule, and reference ADR-024. Correct §2.7.5's GPUI prose that
  still says steps access context via `rstest_bdd_harness_context`.
- `docs/users-guide.md`: extend "Harness adapter and attribute policy" Table 2
  with `Harness<T>` and `HarnessMut<T>` rows, lead examples with extractors,
  migrate GPUI worked examples from `#[from(rstest_bdd_harness_context)]`,
  document D4 (opt-out and renamed imports), D5, and the borrow-conflict rule,
  and the EP-M1 lint outcome. Update "Fixture key versus parameter name".
- `docs/developers-guide.md`: new subsection "The harness-extractor
  classifier stage" after "The `#[harness_context]` classifier stage",
  documenting `HarnessExtractor` on `Arg::Fixture`, the sealed trait,
  `check_harness_borrows`, and the reuse policy for the shape predicate (owned
  by `harness_extractor/`; callers: the marker classifier and the extractor
  classifier only).
- `docs/v0-6-0-migration-guide.md`: add a v0.7.0 subsection mapping the three
  existing spellings to extractors, and noting the new compile error for
  conflicting harness borrows.
- `CHANGELOG.md` files for `rstest-bdd` and `rstest-bdd-macros`; and
  `docs/contents.md` for ADR-024.
- Prelude (R3): if `rstest_bdd::prelude` exists, add `Harness` and
  `HarnessMut` and update its parity gates; otherwise add an open-question note
  to this plan for 11.2.2.
- Record a follow-up roadmap note for harness-aware run-time diagnostics
  (D6, R7) under phase 12.2 as a proposed item, not a commitment.

Acceptance: `make fmt`, `make markdownlint`, `make nixie` pass; gates green;
commit.

### EP-M7 — roadmap tick, full gates, and pull request

Mark 12.2.1 done in `docs/roadmap.md` with a completion note naming this plan
and ADR-024. Run all gates via `scrutineer`. Update this plan's
`Outcomes & retrospective` and set Status to COMPLETE. Push and open the
implementation PR.

## Milestones and plateaus

- EP-M0. Outcome: verified baseline, no code change. Requirements: none.
  Evidence: gate logs. Conformance: A2 confirmed or plan revised. Recovery:
  nothing to revert. Remaining gaps: all. Compatibility decision: none.
- EP-M1. Outcome: lint posture decided, scratch code deleted. Requirements:
  C9. Evidence: Clippy transcripts. Conformance: no public change. Recovery:
  delete scratch crate. Remaining gaps: all implementation. Compatibility
  decision: none.
- EP-M2. Outcome: `Harness`/`HarnessMut` usable from handwritten code.
  Requirements: FL-1, FL-2 (runtime half), O4 (runtime half), O5. Evidence:
  unit tests and equivalence table. Conformance: C3, C4 checked against
  `cargo doc` output for new public items. Recovery: revert the commit.
  Remaining gaps: macros do not yet recognize the types. Compatibility
  decision: none (additive, pre-1.0).
- EP-M3. Outcome: steps can take extractors. Requirements: FL-1, FL-2,
  FL-3, O1, O2, O4, O8. Evidence: unit, property, snapshot tests. Conformance:
  C1 (key unchanged), C6 (lengths). Recovery: revert; EP-M2 stands alone.
  Remaining gaps: conflict check, compile tests, docs. Compatibility decision:
  none.
- EP-M4. Outcome: conflicting harness borrows are compile errors for all
  spellings. Requirements: O3, D7. Evidence: enumeration, property, mutation
  transcript. Conformance: C2 exception verified per R5. Recovery: revert;
  EP-M3 stands alone. Remaining gaps: compile tests, docs. Compatibility
  decision: none (pre-1.0; no working program rejected).
- EP-M5. Outcome: user-observable behaviour proven end to end.
  Requirements: FL-1..FL-4, O6, O7. Evidence: trybuild, behavioural scenario,
  examples. Conformance: examples use public API only. Recovery: revert fixture
  additions. Remaining gaps: docs. Compatibility decision: none.
- EP-M6. Outcome: documentation and ADR consistent with code. Requirements:
  DD-2.7.6.5, ADR-007 amendment. Evidence: Markdown gates. Conformance: every
  decision in this plan appears in ADR-024 or the design document. Recovery:
  revert docs commit. Remaining gaps: roadmap tick. Compatibility decision:
  none.
- EP-M7. Outcome: roadmap item done. Evidence: all gates green on the final
  commit. Recovery: none needed. Compatibility decision: none.

## Concrete steps

All commands run from the repository root. Long outputs are teed to `/tmp`:

```bash
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee /tmp/check-fmt-rstest-bdd-$BRANCH.out
make lint 2>&1 | tee /tmp/lint-rstest-bdd-$BRANCH.out
make test 2>&1 | tee /tmp/test-rstest-bdd-$BRANCH.out
```

Focused loops used during red-green-refactor:

```bash
cargo test -p rstest-bdd-macros harness_extractor
cargo test -p rstest-bdd extractors
cargo test -p rstest-bdd --test harness_extractor_equivalence
cargo test -p rstest-bdd --test harness_extractors
cargo test -p rstest-bdd --test trybuild_macros
cargo insta review
python3 scripts/check_rs_file_lengths.py
```

Expected red transcript at the start of EP-M3 (abridged):

```plaintext
error[E0432]: unresolved import `super::harness_extractor`
test result: FAILED
```

Expected green transcript at the end of EP-M4 (abridged):

```plaintext
test ...::borrow_plan::tests::classifier_matches_refcell_oracle_up_to_length_four ... ok
test ...::borrow_plan::prop_tests::classifier_matches_refcell_oracle ... ok
```

## Validation and acceptance

Red-green-refactor evidence is recorded per milestone in `Artefacts and notes`:
the red command and its expected failure, the green command and its pass, and
the post-refactor gate runs.

Acceptance criteria (behavioural):

- Running `cargo test -p rstest-bdd --test harness_extractors` passes both
  scenarios; the first fails before EP-M3 because `HarnessMut<..>` is treated
  as an owned fixture named after the parameter.
- `trybuild` reports every new compile-fail fixture's expected diagnostic,
  and the compile-pass fixture builds.
- `examples/gpui-counter` and `examples/tokio-reminders` pass under
  `make test`.
- Searching the new behavioural test, compile-pass fixture, and migrated
  example steps for `rstest_bdd_harness_context` returns nothing (O6).

Quality criteria:

- Tests: `make test` passes.
- Verification: O1–O8 discharged as described; the EP-M4 mutation is
  rejected.
- Lint and format: `make check-fmt`, `make lint`, `make markdownlint`, and
  `make nixie` pass.
- No Verus or Kani command applies (D9).

## Idempotence and recovery

Every milestone is a commit on this branch; rolling back is `git revert` of
that milestone's commits. Gate runs are repeatable. `cargo insta review` only
rewrites snapshot files under `snapshots/`. Trybuild `.stderr` files are
regenerated with `TRYBUILD=overwrite` only for fixtures added by this plan, and
diffs are inspected before committing. The EP-M1 scratch crate lives under
`/tmp` and is deleted at the end of the milestone.

## Artefacts and notes

To be filled during implementation: EP-M1 Clippy transcripts, EP-M4 seeded
mutation transcript, and red/green transcripts for each milestone.

## Interfaces and dependencies

No new dependencies. Test crates already available: `rstest`, `googletest`
(with the `proptest` feature), `pretty_assertions`, `insta`, `proptest`,
`trybuild` (workspace manifest).

In `crates/rstest-bdd/src/context/extractors.rs`:

```rust
/// Shared access to the harness context provided by the scenario's harness adapter.
pub struct Harness<'a, T: Any>(FixtureRef<'a, T>);

/// Exclusive access to the harness context provided by the scenario's harness adapter.
pub struct HarnessMut<'a, T: Any>(FixtureRefMut<'a, T>);

impl<'a, T: Any> Harness<'a, T> {
    pub fn try_from_context<'c: 'a>(ctx: &'a StepContext<'c>) -> Result<Self, FixtureBorrowError>;
}

impl<'a, T: Any> HarnessMut<'a, T> {
    pub fn try_from_context<'c: 'a>(ctx: &'a StepContext<'c>) -> Result<Self, FixtureBorrowError>;
}

// Deref<Target = T> for both; DerefMut for HarnessMut; AsRef<T>; AsMut<T> for HarnessMut; Debug.
```

In a `#[doc(hidden)] pub mod __macro_support` of `rstest-bdd`:

```rust
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an rstest-bdd harness extractor",
    note = "parameters typed `Harness<T>` or `HarnessMut<T>` request the harness context; \
            to inject an ordinary fixture of this type, add `#[from(fixture_name)]`"
)]
pub trait HarnessExtract<'a>: Sized + sealed::Sealed {
    fn extract(ctx: &'a StepContext<'_>) -> Option<Self>;
}
```

The exact lifetime plumbing is settled in EP-M2; the requirement is that the
generated `let` binding infers `'a` from the wrapper's `&StepContext`.

In
`crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_extractor/`:

```rust
pub(crate) enum HarnessAccess { Shared, Exclusive }
pub(crate) struct HarnessExtractor { pub access: HarnessAccess, pub inner: syn::Type }
pub(crate) fn extractor_shape(ty: &syn::Type) -> Option<HarnessExtractor>;
pub(crate) fn classify_harness_extractor(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    placeholders: &HashSet<String>,
) -> syn::Result<bool>;
pub(crate) fn check_harness_borrows(args: &[Arg]) -> syn::Result<()>;
```

## Revision notes

- 2026-10-15: initial draft written from Wyvern reconnaissance and prior-art
  research. Awaiting community-of-experts review.
