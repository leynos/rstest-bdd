# Typed harness-context extractors for step parameters (roadmap 12.2.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT — awaiting approval. Implementation must not begin until the plan
is explicitly approved. Decisions marked "Requires approval" in the Decision
log (D1, D6, D7, D11, D14) are part of that approval.

## Purpose / big picture

A step definition that wants the object a harness adapter provides (for example
GPUI's `TestAppContext`, or a Tokio test context) has three spellings today,
all of which end at the same reserved fixture key:

```rust,no_run
// v0.6.0 legacy spelling: the reserved key is spelled by hand.
#[when("the user presses Tab")]
fn press_tab(#[from(rstest_bdd_harness_context)] cx: &mut gpui::TestAppContext) {}

// v0.6.0 parameter-named spelling: the parameter itself is named after the key.
#[when("the user presses Tab")]
fn press_tab(rstest_bdd_harness_context: &mut gpui::TestAppContext) {}

// v0.6.1 marker (roadmap 11.2.1): readable, but the request is an attribute on
// a plain reference rather than something the type says.
#[when("the user presses Tab")]
fn press_tab(#[harness_context] cx: &mut gpui::TestAppContext) {}
```

After this change a step can state what it wants in its parameter type, with no
attribute and no reserved key:

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

`Harness<T>` is a shared, `Copy` view of the harness context and
`HarnessMut<T>` an exclusive view. Each is a thin wrapper around a reference;
the generated wrapper owns the underlying ADR-012 borrow guard exactly as it
does for `&T` and `&mut T` today. Extractors therefore coexist with other
fixtures, including `&mut World`, inside one step, and a step that takes
`Harness<T>` can be called directly from a unit test with `Harness::new(&ctx)`.
The `#[harness_context]` marker remains the stable attribute spelling for steps
that prefer plain references, and this plan names the v1 spelling set as
`Harness<T>`, `HarnessMut<T>`, and `#[harness_context]` (D1).

Success is observable in four ways:

1. A behavioural suite in `crates/rstest-bdd/tests/harness_extractors.rs`
   passes. Its steps read and mutate harness context through `Harness<T>`,
   `HarnessMut<T>`, and `#[harness_context]`, mutate world state in the same
   step as `HarnessMut<T>`, and include an unhappy path in which a step that
   needs the harness context runs in a scenario without a harness. None of its
   source spells `rstest_bdd_harness_context`.
2. Macro unit, table, and snapshot tests in `rstest-bdd-macros` prove that
   `Harness<T>`, `HarnessMut<T>`, and the marker are each classified as the
   harness context with the right access mode, and that the generated wrappers
   are stable.
3. `trybuild` compile-fail fixtures prove that misuse yields a targeted
   compiler diagnostic, and a permanent Clippy lane proves that ordinary
   extractor steps are lint-clean under `clippy::pedantic` and
   `rust_2018_idioms`.
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
  and keep their current run-time meaning, with one approved exception: the
  harness-borrow conflict check (EP-M4, D7) rejects steps that would fail on
  every run. No deprecation warnings are added in this item.
- C3. No existing public signature changes: `StepContext` methods,
  `FixtureRef`, `FixtureRefMut`, `FixtureBorrowError`, `StepError`,
  `HarnessAdapter`, and `ScenarioRunRequest` are untouched. The ADR-007 closed
  set of `*_harness_context` wrapper methods on `StepContext` is not extended.
- C4. New public items are limited to `rstest_bdd::Harness`,
  `rstest_bdd::HarnessMut`, their `new` constructors and standard trait
  implementations, one sealed trait in a `#[doc(hidden)]` module used by
  generated code, and two path constants in `rstest-bdd-policy` (all named in
  `Interfaces and dependencies`).
- C5. No new external dependency in any crate. `rstest-bdd-macros` must not
  gain a dependency on `rstest-bdd` (`docs/developers-guide.md`, "Workspace
  dependency policy").
- C6. No source file exceeds 400 lines (`scripts/check_rs_file_lengths.py`,
  run by `make lint`). Files at or near the cap
  (`crates/rstest-bdd/src/context/mod.rs` 397 lines,
  `crates/rstest-bdd-macros/src/codegen/wrapper/arguments/mod.rs` 395,
  `.../args/classify/mod.rs` 381, `.../args/classify/tests.rs` 385,
  `crates/rstest-bdd/tests/trybuild_macros.rs` 359) must not absorb new logic;
  new logic goes in new sibling modules.
- C7. Run-time diagnostics (Fluent `.ftl` messages in every locale under
  `crates/rstest-bdd/i18n/`) are not changed by this item. New diagnostics are
  compile-time, English, and emitted by the macros crate, matching the 11.2.1
  precedent.
- C8. Only stable Rust features usable at the workspace `rust-version`
  (1.88) are allowed. The toolchain is *not* pinned to a version:
  `rust-toolchain.toml` selects the moving `stable` channel, so compiler-
  rendered diagnostic text can change between releases.
  `#[diagnostic::on_unimplemented]` (stable since 1.78) is already used in
  `crates/rstest-bdd/src/step_return.rs`.
- C9. Ordinary extractor steps must be lint-clean, without user-written
  suppressions, under `-D warnings -D clippy::pedantic -D rust_2018_idioms`.
  The step macro must not emit `#[allow]` or `#[expect]` attributes onto the
  user's function.

## Tolerances (exception triggers)

- Scope: if implementation needs more than 40 changed files or more than
  1,800 net lines of Rust (excluding `.stderr`, `.snap`, and Markdown), stop
  and escalate.
- Interface: if any item outside C4 must become public, or any signature named
  in C3 must change, stop and escalate.
- Dependencies: if any new crate dependency (including a dev-dependency not
  already in the workspace manifest) is needed, stop and escalate.
- Lint posture: if EP-M1 shows that `Harness<T>` or `mut cx: HarnessMut<T>`
  steps cannot be made lint-clean under C9, stop and present options.
- Behaviour change: if the EP-M4 conflict check rejects any step in the
  workspace, the examples, or `tests/fixtures/published-gpui-*`, stop and
  escalate; lemma L3 says only always-failing steps are rejected.
- Iterations: if a milestone's focused tests still fail after three fix
  attempts, stop and escalate with the failing output.
- Time: if any single milestone takes more than one working day, stop and
  record why before continuing.
- Ambiguity: if the approver rejects any decision marked "Requires approval",
  revise the plan before implementation.

## Risks

- R1. Clippy's pedantic `needless_pass_by_value` lint fires on by-value step
  parameters that are only read. The workspace has direct evidence: commit
  `31fd3a7f` (#351) added `&str` step parameters to avoid suppressing it, and
  `crates/rstest-bdd/tests/overlapping_patterns.rs` still uses `drop(..)` to
  silence it. The same lint fires on Bevy's `Res<T>` and suggests `&Res<T>`
  ([rust-clippy#8940](https://github.com/rust-lang/rust-clippy/issues/8940)).
  Severity: high. Likelihood: high for a guard-holding design. Mitigation:
  `Harness<'a, T>` is a `Copy` view over `&'a T` (the lint ignores `Copy`
  types), `HarnessMut<'a, T>` is used with a `mut` binding when it is written
  through, and EP-M1 proves both in a permanent Clippy lane (D3, D14).
- R2. Type-directed recognition is syntactic: a procedural macro sees tokens,
  not resolved types. A user type that happens to be named `Harness<T>` would
  be captured, and `type Cx<'a> = HarnessMut<'a, Ctx>;` or
  `use rstest_bdd::Harness as H;` would not be recognized. Severity: medium.
  Likelihood: low. Mitigation: generated code routes every recognized parameter
  through the sealed `HarnessView` trait, so a foreign `Harness` fails to
  compile with an `on_unimplemented` message naming the `#[from(..)]` opt-out;
  aliases and renamed imports are documented as unsupported (D4, D12).
- R3. Roadmap 11.2.2 (public prelude; plan in DRAFT on branch
  `11-2-2-public-prelude-execplan`) and 11.2.3 (harness-path diagnostics) touch
  `crates/rstest-bdd/tests/trybuild_macros.rs`, `crates/rstest-bdd/Cargo.toml`,
  and the users' guide. The 11.2.2 plan adds `scripts/check_prelude_exports.py`
  to `make lint`, which fails unless every `pub use` in `prelude.rs` matches a
  users' guide table and doctest. Severity: medium. Likelihood: medium.
  Mitigation: EP-M0 rebases and re-reads both plans; EP-M6 updates
  `prelude.rs`, its table, and its doctest together if the prelude has landed,
  and otherwise records the admission decision for 11.2.2 in this plan's
  `Outcomes & retrospective` and in the PR description.
- R4. The 400-line cap (C6). Severity: low. Likelihood: high. Mitigation: new
  modules `args/classify/harness_extractor/` and
  `arguments/harness_extractor.rs` in the macros crate, and
  `context/extractors.rs` in the runtime crate; any helper move needed to make
  room is a separate refactor commit.
- R5. The conflict check (D7) also applies to the legacy and marker
  spellings. A crate that defines a conflicting step but never runs it compiles
  today and will not compile after EP-M4. Severity: low. Likelihood: low.
  Mitigation: lemma L3 shows such a step fails on every run; EP-M0 confirms
  none exists in the repository; the CHANGELOG and migration guide name the new
  error.
- R6. `trybuild` `.stderr` snapshots drift across stable releases (C8), and
  the trybuild binary is already slow (186–387 s cold, `.config/nextest.toml`).
  Severity: medium. Likelihood: medium. Mitigation: macro-emitted diagnostics
  have stable text; the single `on_unimplemented` fixture goes through the
  existing normalizer in `crates/rstest-bdd/tests/trybuild_macros/wrappers.rs`
  (`normalize_conditional_trait_help`) or asserts substrings; fixtures are
  consolidated to one pass and four compile-fail files.
- R7. When a step that needs the harness context runs in a scenario with no
  harness, or under a harness whose context is a different type, the run-time
  diagnostic still names `rstest_bdd_harness_context`. With no harness, the
  `en-GB` message is "Select a harness-backed scenario so
  rstest_bdd_harness_context is inserted." With a wrong type, the wrapper
  reports a missing fixture of that name. Shared step libraries make this
  likely in practice. Severity: medium. Likelihood: medium. Mitigation: the
  missing-fixture `ty` field reports the written extractor type, which points
  back to the user's code; the users' guide gains a troubleshooting entry
  quoting the exact messages; a test pins today's text; and EP-M6 adds a
  roadmap item for harness-aware run-time wording (D6).
- R8. The 11.2.1 precedent recorded that Tokio-harness async steps are polled
  once. Severity: low. Likelihood: medium. Mitigation: the async extractor
  scenario (O12) awaits only immediately-ready futures.

## Progress

- [x] (2026-10-15) Branch renamed to
  `12-2-1-typed-harness-extractor-steps-annotations`; upstream configured as
  `origin/12-2-1-typed-harness-extractor-steps-annotations`.
- [x] (2026-10-15) Reconnaissance of the macro classification pipeline, the
  run-time `StepContext` and guard surface, documentation and ADRs, tests,
  examples, and sibling branches.
- [x] (2026-10-15) Prior-art research: axum extractors, Bevy `Res`/`ResMut`,
  the `elided_lifetimes_in_paths` lint, and rust-clippy#8940.
- [x] (2026-10-15) First draft of this plan.
- [x] (2026-10-15) Design review by a panel of four reviewers (structure and
  contracts; alternatives; failure modes and cost; verification and
  executability). Verdict: proceed with conditions. Every finding is either
  folded into this revision or recorded as a rejected option in the Decision
  log.
- [ ] Plan approved by the user.
- [ ] EP-M0 orientation and green baseline.
- [ ] EP-M1 permanent lint-posture gate.
- [ ] EP-M2 run-time extractor types.
- [ ] EP-M3 macro recognition and code generation.
- [ ] EP-M4 harness-borrow conflict check.
- [ ] EP-M5 compile tests, behavioural suite, and examples.
- [ ] EP-M6 documentation, ADR-024, and prelude hand-off.
- [ ] EP-M7 roadmap tick, full gates, and pull request.

## Surprises & discoveries

- Observation: the wrapper maps every fixture-borrow failure to
  `StepError::MissingFixture`, even when the cause is a type mismatch, a
  non-mutable entry, or an outstanding borrow. Evidence:
  `crates/rstest-bdd-macros/src/codegen/wrapper/arguments/fixtures.rs` emits
  `.borrow_ref(..)`/`.borrow_mut(..)` then
  `.ok_or_else(|| MissingFixture { .. })?`, and `StepContext::borrow_mut` is
  `try_borrow_mut(..).ok()` (`crates/rstest-bdd/src/context/mod.rs`). Impact:
  two `&mut` harness-context parameters in one step fail at run time with a
  misleading "missing fixture" error. EP-M4 makes that a compile error.
- Observation: a value returned by an earlier step can override the harness
  context. Evidence: `StepContext::insert_value` matches any fixture entry by
  `TypeId`, including the reserved key, and `try_borrow`/`try_borrow_mut`
  consult the `values` map first. Impact: the first draft's axiom A2 was false.
  Within one wrapper invocation every borrow of the key still reaches one cell,
  because `values` only changes through `&mut self`. A2 is restated below and
  O4 gains an "override present" state.
- Observation: the generated wrapper takes `&mut StepContext<'_>`, not
  `&StepContext<'_>`. Evidence:
  `crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly/mod.rs`
  (synchronous and asynchronous wrapper signatures). Impact: a test cannot hold
  a guard taken through the context while calling a wrapper; O4 holds
  outstanding guards on the backing `RefCell` instead.
- Observation: the language server does not model harness context. Evidence:
  `crates/rstest-bdd-server/src/handlers/diagnostics/placeholder.rs` counts a
  parameter as a step argument by placeholder name only. Impact: extractor
  parameters are neutral for the server; no server change (D8).

## Decision log

- D1. Decision: read "a stable attribute marker" in the roadmap entry as the
  existing `#[harness_context]` marker from 11.2.1, and name the v1 spelling
  set as `Harness<T>`, `HarnessMut<T>`, and `#[harness_context]`. The
  parameter-named and `#[from(rstest_bdd_harness_context)]` spellings stay
  supported without warnings in this item; EP-M6 adds a roadmap item proposing
  their deprecation before 1.0. Rationale: 11.2.1 shipped exactly that marker;
  a second marker would enlarge a surface phase 12 is meant to shrink, and five
  undifferentiated spellings would contradict that goal. Date/Author:
  2026-10-15, planner. Requires approval.
- D2. Decision: reject "automatic" injection by type alone (treating an
  unannotated `cx: &mut TestAppContext` as the harness context when no fixture
  named `cx` exists). Rationale: the step macro cannot know which harness a
  step will run under, so a run-time fallback would silently change meaning
  when a fixture of the same type is added later. Extractor types make the
  request explicit at the cost of writing the type. Date/Author: 2026-10-15,
  planner.
- D3. Decision: `Harness<'a, T>` is a `Copy` view over `&'a T` and
  `HarnessMut<'a, T>` a view over `&'a mut T`. The generated wrapper acquires
  the borrow guard with exactly the code it already emits for a marker `&T` or
  `&mut T` parameter (including the `&'static T` fallback), then wraps the
  resulting reference through a sealed trait. Rationale: (1) a `Copy` view is
  not linted by `needless_pass_by_value`, whereas a guard newtype is (R1); (2)
  reusing the marker's guard code makes the two spellings equivalent by
  construction (O4) and keeps the `&'static T` fallback that a fresh
  `try_borrow`-based path would lose; (3) the guard's lifetime is the wrapper's
  block, which makes lemma L1 a property of existing, tested code generation;
  (4) views over references are safe to construct, so steps stay unit-testable.
  Alternatives considered and rejected: guard newtypes over
  `FixtureRef`/`FixtureRefMut` (the first draft; fails R1, and Clippy's
  suggested `&Harness<T>` would hit D5's error); a type alias of `FixtureRef`
  (leaks the guard type into errors and cannot be told apart from other guards
  by the sealed trait); an owned clone or handle (needs `Context: Clone` or
  `Rc`, pre-empting roadmap 12.2.4); and a fully trait-directed design (axum
  style) in which every by-value fixture goes through a trait (the macro must
  choose the fixture key and the `const` requirement metadata at expansion
  time, which method-call dispatch cannot do, and it would change every
  wrapper's code). Date/Author: 2026-10-15, planner, after design review.
- D4. Decision: a parameter is an extractor when its type is a path type with
  no qualified self, whose final segment is exactly `Harness` or `HarnessMut`,
  and whose angle-bracketed arguments are one type argument and at most one
  lifetime argument. `#[from(name)]` with any name other than the reserved key
  opts out and makes the parameter an ordinary fixture. Aliases and renamed
  imports are not recognized and are documented as unsupported.
  `Option<Harness<T>>` is out of scope. Rationale: recognition must happen at
  expansion time because it chooses the fixture key; keeping it a pure function
  of the parameter's own tokens keeps it predictable, and the sealed trait (O7)
  catches misrecognition at compile time. This mirrors ADR-019's syntactic fast
  path backed by a type-checked contract. Date/Author: 2026-10-15, planner.
- D5. Decision: the following are compile errors with targeted messages:
  `#[harness_context]` on an extractor type;
  `#[from(rstest_bdd_harness_context)]` on an extractor type; an extractor
  behind a reference (`&Harness<T>`, `&mut HarnessMut<T>`), whose message
  explains that Clippy may suggest this form and that `Harness<T>` is already
  `Copy`; an extractor bound to a step placeholder; and an extractor combined
  with `#[datatable]` or `#[step_args]`. Rationale: one canonical spelling per
  intent, matching the 11.2.1 precedent that rejects every marker combination.
  Date/Author: 2026-10-15, planner.
- D6. Decision: extractor borrow failures produce the same
  `StepError::MissingFixture` variant, with the same fixture name, as the
  marker path; the `ty` field carries the written extractor type with lifetimes
  normalized (for example `HarnessMut<'_, gpui::TestAppContext>`). Run-time
  `.ftl` messages are unchanged (C7). EP-M6 adds a roadmap item, with its own
  finish line, for harness-aware run-time diagnostics that do not name the
  reserved key. Rationale: the finish line concerns the authoring surface;
  changing run-time wording touches every locale and the public `StepError`
  enum, which belong to a separately reviewed item. Reporting the written type
  is a cheap, in-scope improvement that points users back to their code.
  Date/Author: 2026-10-15, planner. Requires approval.
- D7. Decision: add a compile-time harness-borrow conflict check that covers
  every harness-context spelling but only the reserved key. Its rule: a step
  may hold at most one exclusive harness borrow, or any number of shared ones.
  Generalizing the same check to every fixture key is recorded as a proposed
  follow-up, not done here. Rationale: the wrapper acquires every guard before
  calling the step and holds them until the call returns, so a conflicting step
  can never succeed (lemma L3); a compile error replaces a misleading run-time
  "missing fixture" error. Limiting the check to extractors would give two
  spellings of the same request different failure modes, while widening it to
  every key would change behaviour for steps outside this item's scope.
  Date/Author: 2026-10-15, planner. Requires approval.
- D8. Decision: no language-server change.
  Rationale: the server treats every non-placeholder parameter as a fixture,
  which is correct for extractors; the macro already rejects extractors bound
  to placeholders. Date/Author: 2026-10-15, planner.
- D9. Decision: no Verus or Kani artefact. The written argument for lemma L2′
  in this plan (and in ADR-024) is the proof that bounded enumeration is
  complete; the enumeration is the executable check. Rationale: proving that a
  `RefCell` state-machine fold succeeds exactly when the counting predicate
  holds would be a genuine induction, not a restatement. Verus is still not
  warranted, for four reasons. (1) Its trusted base would be a handwritten
  model of `RefCell` (axiom A1), as large as the proof itself. (2) The parts
  most likely to contain defects (classification over `syn` tokens, O1 and O2,
  and code-generation structure, L1) are out of Verus's reach. (3) Lemma L2′
  makes exhaustive enumeration to length 3 complete for every length. (4) The
  repository has no prover toolchain, and adding one would breach the
  dependency tolerance. Kani over a fixed-length array of modes would repeat
  the enumeration. The 11.2.1 precedent made the same call. Date/Author:
  2026-10-15, planner, after design review.
- D10. Decision: record the design in a new ADR-024, "Typed harness-context
  extractors", as the single canonical home for how step authors request
  harness context, and add an `Amended by ADR-024` banner and `## Amendments`
  entry to ADR-007. ADR-012 is not amended: borrow semantics are unchanged and
  the conflict check is a compile-time projection of them. Rationale: this item
  introduces type-directed parameter classification, new public types, a
  compile-time borrow rule that applies to existing spellings, and the v1
  spelling set; that is substantive under the documentation style guide, and
  spreading it across a second ADR-007 addendum would scatter the authoring
  story further. Date/Author: 2026-10-15, planner.
- D11. Decision: keep the roadmap's names, `Harness<T>` and `HarnessMut<T>`.
  Rationale: the finish line names them, and they are short where they are
  written most. Two reviewers argued that `Harness` collides with the adapter
  vocabulary (`HarnessAdapter`, `GpuiHarness`): a user who writes
  `Harness<GpuiHarness>` gets a real `rstest_bdd::Harness`, so the sealed trait
  accepts it and the mistake surfaces only at run time. Their alternative,
  `HarnessContext<T>`/`HarnessContextMut<T>`, matches `#[harness_context]` and
  `borrow_harness_context`, but deviates from the roadmap text. The plan keeps
  the roadmap names and mitigates with rustdoc, the users' guide
  troubleshooting entry, and the written-type diagnostic (D6). Date/Author:
  2026-10-15, planner. Requires approval; choosing the alternative changes only
  names, the finish-line wording, and the policy constants.
- D12. Decision: do not add an autoref probe to the ordinary owned-fixture
  path to diagnose aliased extractors. Rationale: it would change the generated
  code, and the snapshots, of every wrapper with an owned fixture, to improve
  one documented-unsupported case. Date/Author: 2026-10-15, planner.
- D13. Decision: represent every harness-context request with one field on
  `Arg::Fixture`, `harness: Option<HarnessRequest>`, set by the marker, legacy,
  parameter-named, and extractor classifiers alike. `check_harness_borrows`
  reads only that field. `ty` keeps meaning the type as written (after lifetime
  normalization). Rationale: one source of truth for the borrow check, and a
  field rather than a new variant keeps fixture-name collection and requirement
  metadata, which iterate `fixtures()`, correct without new arms. The field
  must still be added at every construction and destructuring site, listed in
  EP-M3. Date/Author: 2026-10-15, planner.
- D14. Decision: the step macro rewrites lifetime arguments in a recognized
  extractor type: an elided lifetime becomes `'_` in the re-emitted user
  signature, and every lifetime becomes `'_` in the wrapper. Rationale: the
  re-emitted user function is linted as user code, so `Harness<T>` would trip
  `elided_lifetimes_in_paths` under `rust_2018_idioms` (C9); and the wrapper
  does not declare the user's named lifetimes, so copying `Harness<'a, T>`
  verbatim would fail with E0261. Inserting `'_` does not change meaning.
  Date/Author: 2026-10-15, planner. Requires approval, because the macro edits
  the user's written signature.

## Outcomes & retrospective

Not started. To be completed at each milestone boundary and at completion,
including reconciliation of discoveries with ADR-007, ADR-012, ADR-024, and
design document §2.7.6.5.

## Context and orientation

### What this repository is

`rstest-bdd` is a behaviour-driven development (BDD) framework for Rust built on
`rstest`. Users write Gherkin `.feature` files and annotate Rust functions with
`#[given]`, `#[when]`, and `#[then]`. The procedural macros in
`crates/rstest-bdd-macros` re-emit each annotated function unchanged (apart
from stripping helper attributes) and generate a registered *step wrapper*:
code that receives a `&mut StepContext`, pulls each parameter out of it, and
calls the user's function. `#[scenario]` and `scenarios!` generate one test per
scenario that builds the `StepContext` and runs the matching steps.

### Terms of art used in this plan

- *Fixture*: a value stored in the `StepContext` under a string key, usually
  produced by an `rstest` fixture function and keyed by parameter name.
- *Harness adapter*: a type implementing
  `rstest_bdd_harness::HarnessAdapter`
  (`crates/rstest-bdd-harness/src/adapter.rs`) that runs a scenario inside a
  framework-specific environment and hands it a value of its associated type
  `Context` (which must be `Any`, and therefore `'static`).
- *Harness context*: that `Context` value. Generated scenario code stores it
  under the reserved key `rstest_bdd_harness_context` with
  `insert_owned_harness_context`
  (`crates/rstest-bdd-macros/src/codegen/scenario/runtime/harness.rs`).
- *Guard*: `FixtureRef<'a, T>` or `FixtureRefMut<'a, T>`
  (`crates/rstest-bdd/src/context/guards.rs`), the ADR-012 borrow handles
  returned by `StepContext::borrow_ref`/`borrow_mut`, which take `&self`.
  Distinct fixtures can be borrowed mutably at once; one fixture follows
  `RefCell` rules.
- *Extractor*: a by-value parameter type whose construction pulls data out of
  the context, as in axum's handler extractors. Here `Harness<T>` (shared view)
  and `HarnessMut<T>` (exclusive view).
- *Classifier*: a function in
  `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/` that decides
  what a step parameter is and records it as an `Arg`.
- *Harness access mode*: how a parameter borrows the harness context.
  `Shared`: `&T`, owned `T` (cloned from a shared borrow), or `Harness<T>`.
  `Exclusive`: `&mut T` or `HarnessMut<T>`.
- *Commit gates*: `make check-fmt`, `make lint`, and `make test`, run one
  after another (never in parallel) with output teed to `/tmp` as shown in
  `Concrete steps`.

### The parameter classification pipeline

`extract_args` in
`crates/rstest-bdd-macros/src/codegen/wrapper/args/extract.rs` walks the step
function's parameters in order. For each parameter it runs, first match wins:

1. `#[step_args]` struct classification.
2. `classify_harness_context` (marker), deliberately before the placeholder
   short-circuit so the attribute is always stripped.
3. If the name is not a placeholder: data-table and doc-string classifiers.
4. `classify_step_or_fixture`: a placeholder match makes a step argument;
   anything else becomes `Arg::Fixture { pat, name, ty }`, with `name` taken
   from `#[from(name)]` or the normalized parameter name.

The new extractor classifier slots in immediately after step 2, before the
placeholder short-circuit, for the same reason the marker does.

Code generation for fixtures is in
`crates/rstest-bdd-macros/src/codegen/wrapper/arguments/fixtures.rs`
(`gen_fixture_decls`). It branches on the parameter type: `&mut T` uses
`borrow_mut::<T>` and `value_mut()`; `&T` uses `borrow_ref::<T>` with a
`&'static T` fallback; any other type is cloned from `borrow_ref::<T>`. A
`Harness<T>` parameter today would be treated as an owned fixture named after
the parameter and cloned, which fails to compile. Fixture requirement metadata
(`FixtureRequirement { name, ty }`) is emitted in `.../wrapper/emit/mod.rs`
(`effective_fixture_type`). The runtime uses it in
`crates/rstest-bdd/src/execution/fixtures.rs` to report missing fixtures before
a step runs, with a harness hint when the reserved key is missing.

Wrapper-level lint expectations live in
`crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly/lint_config.rs`;
they apply to the generated wrapper only, never to the user's function.

### File-length pressure

Line counts measured during reconnaissance: `context/mod.rs` 397,
`arguments/mod.rs` 395, `args/classify/mod.rs` 381, `args/classify/tests.rs`
385, `args/classify/fixture_or_step.rs` 352, `args/extract.rs` 326,
`args/mod.rs` 232, `arguments/fixtures.rs` 245, `emit/mod.rs` 284,
`context/harness.rs` 86, `context/guards.rs` 145, `tests/trybuild_macros.rs`
359. Re-measure in EP-M0 with `python3 scripts/check_rs_file_lengths.py`.

### Skills and documentation to load before starting

- Skills: `rust-router`, then `rust-types-and-apis` (view types and sealed
  trait), `rust-unit-testing` (rstest, googletest, pretty_assertions, insta),
  and `nextest` if a test needs scheduling attention; `execplans` for
  maintaining this plan; `codegraph-mcp` for callers and impact;
  `arch-decision-records` and `en-gb-oxendict-style` for ADR-024;
  `commit-message` for commits; `pr-creation` for EP-M7.
- Documents: `docs/adr-007-harness-context-injection.md` (wire format and the
  11.2.1 addendum), `docs/adr-012-guard-based-stepcontext-borrowing.md`
  (guards), `docs/adr-019-type-directed-step-return-classification.md`
  (syntactic fast path plus type-checked contract), `docs/rstest-bdd-design.md`
  §§2.7.5–2.7.6.5, `docs/developers-guide.md` ("Macro implementation: fixture
  classification and normalization", "The `#[harness_context]` classifier
  stage", "Assertion vocabulary", "Staging fixtures for trybuild"),
  `docs/users-guide.md` ("Harness adapter and attribute policy", Table 2),
  `docs/testing-strategy.md`, `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md` (doctests for the new public types),
  `docs/complexity-antipatterns-and-refactoring-strategies.md` (keep the
  classifier free of nested conditionals), `docs/gherkin-syntax.md` (feature
  files), `docs/rstest-bdd-language-server-design.md` (D8),
  `docs/documentation-style-guide.md` (ADR format), and the completed precedent
  `docs/execplans/11-2-1-annotate-parameters-with-harness-context.md`, whose
  `Surprises & discoveries` section lists traps that recur here.

## Conformance basis

Upstream artefacts, read at `origin/main` commit `8737d88f`:

- RM-12.2.1: `docs/roadmap.md` entry 12.2.1, with finish-line clauses FL-1
  (`Harness<T>` covered by macro tests), FL-2 (`HarnessMut<T>` covered), FL-3
  (marker path covered), and FL-4 (those tests do not spell the reserved key).
- DD-2.7.6.5: `docs/rstest-bdd-design.md` §2.7.6.5, remaining candidate
  "typed harness-context extractors that hide the reserved fixture key".
- ADR-007 (Accepted 2026-03-03) and its 11.2.1 addendum: key-as-wire-format
  (C1), closed wrapper set (C3), marker semantics.
- ADR-012 (Accepted 2026-06-11): `&self` borrows, guard types, concurrent
  distinct mutable borrows; source of axiom A2.
- ADR-019: precedent for syntactic classification backed by a type-checked
  contract and `on_unimplemented` (D4).
- Governing standards: `AGENTS.md`, `docs/documentation-style-guide.md`,
  `docs/developers-guide.md` workspace dependency policy.
- There is no Terms of Reference document for this phase; the roadmap entry is
  the requirement of record.

Trace links:

```plaintext
RM-12.2.1/FL-1 -> DD-2.7.6.5 -> ADR-024 -> EP-M2, EP-M3 -> O1, O2, O4, O5
  -> harness_extractor tests, harness_extractors.rs suite
RM-12.2.1/FL-2 -> DD-2.7.6.5 -> ADR-024 -> EP-M2, EP-M3, EP-M4 -> O2, O3, O4, O5, O11
  -> borrow_plan tests, harness_extractors.rs suite
RM-12.2.1/FL-3 -> ADR-007 addendum -> EP-M3, EP-M5 -> O2, O4, O10
  -> marker rows in the same suites
RM-12.2.1/FL-4 -> ADR-024 -> EP-M5 -> O6 -> reserved-key absence check
ADR-012 -> A2 -> L1, L3 -> EP-M4 -> O3
C9 -> D3, D14 -> EP-M1 -> O13 -> ui_lints harness_extractor_lint_clean
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
`RefCell` with state `(readers, writer)`: a shared acquisition succeeds iff
`writer = false`; an exclusive acquisition succeeds iff `readers = 0` and
`writer = false`. A failed acquisition aborts the wrapper.

The admissibility predicate the classifier must implement is
`Compatible(S, X) := X = 0 or (X = 1 and S = 0)`.

### Preconditions and postconditions

- Precondition of the wrapper: the `StepContext` holds an entry (fixture or
  step-returned override) under the reserved key whose type is the requested
  inner type `T`, and the entry is mutable when any access is `Exclusive`.
- Postcondition: if the precondition holds and `Compatible(S, X)`, every
  harness-context parameter receives a reference to that one entry and the user
  function is called; otherwise the wrapper returns `StepError::MissingFixture`
  without calling the user function.

### Axioms

- A1. `std::cell::RefCell` implements the transition rules above
  (`try_borrow` fails iff a mutable borrow is outstanding; `try_borrow_mut`
  fails iff any borrow is outstanding). Standard-library interface; not
  verified.
- A2. During one wrapper invocation, every lookup of the reserved key
  resolves to the same storage: the step-returned override cell if one exists,
  otherwise the fixture entry. The `values` map changes only through
  `StepContext::insert_value(&mut self, ..)`, which the borrow checker forbids
  while guards borrow the context. Guards from earlier steps have been dropped
  before the next wrapper starts (ADR-012 lifecycle). The harness does not
  borrow the context while a step wrapper runs.
- A3. A procedural macro sees parameter types as tokens only; it cannot
  resolve paths or aliases. Recognition (O1) is therefore a syntactic contract,
  and its soundness against real types is delegated to the sealed trait (O7,
  O9).

### Lemmas (hypotheses about the eventual argument)

- L1 (hold until call). The lexical scope of every guard binding the wrapper
  creates for parameter `p_i` encloses the call to the user function: the
  synchronous call inside `catch_unwind(AssertUnwindSafe(|| ..))`, or the
  `async move` block of an asynchronous wrapper. This is a structural property
  of `gen_fixture_decls` and of the new extractor declaration, which reuses it;
  it is checked by the snapshot tests (O8) for both wrapper kinds.
- L2′ (finite abstraction). The outcome of acquiring a sequence of borrows on
  one cell depends only on the abstract state
  `{Empty, Readers, Writer, Failed}`, where `Failed` absorbs. Each mode has
  exactly one transition from each state: `Shared` maps `Empty` and `Readers` to
  `Readers` and `Writer` to `Failed`; `Exclusive` maps `Empty` to `Writer` and
  `Readers` and `Writer` to `Failed`. Every state is reachable within two
  acquisitions (`Failed` via `Exclusive, Exclusive` or `Exclusive, Shared`), so
  every transition from every reachable state is exercised by sequences of
  length at most three. An exhaustive check over all sequences of length at
  most three is therefore complete for sequences of every length.
- L3 (characterization). Under A1, A2, and L1, and given the wrapper
  precondition, the wrapper's harness acquisitions all succeed iff
  `Compatible(S, X)`. Proof sketch: by L2′ the fold over a sequence reaches
  `Failed` iff some prefix ends with an `Exclusive` acquisition after any
  earlier acquisition, or with any acquisition after an `Exclusive` one; that
  happens iff `X >= 2`, or `X = 1` and `S >= 1`.
- Theorem T (soundness and completeness of the compile-time check). The
  classifier accepts a signature iff `Compatible(S, X)`. Hence accepted
  signatures never fail with a harness-cell borrow conflict, and rejected
  signatures fail on every run.

These are hypotheses. If implementation shows a guard can outlive or precede
the call differently (for example a future that moves a guard), L1 and this
section are revised before continuing.

### Obligations

Each obligation names its method, artefact, evidence, and non-vacuity check.
Unit and table tests use `#[rstest]` with `#[googletest::test]`, matching with
`matches_pattern!` (with a trailing `..`) on `Arg::Fixture` values and with
`err(displays_as(contains_substring(..)))` on diagnostics; structural
comparisons of tokens and `StepError` values use `pretty_assertions`.

- O1 (recognition). Obligation: a parameter is classified as an extractor iff
  D4's rule holds; it is then bound to the reserved key, records its access
  mode and inner type, and has its lifetimes normalized per D14. Method:
  `rstest` table of named cases (finite partition). Domain: prefixes none,
  `rstest_bdd::`, `::rstest_bdd::`, `rstest_bdd::prelude::`, and an arbitrary
  module path; lifetime forms none, `'_`, and `'a`; negatives `Harnesses<T>`,
  `MyHarness<T>`, bare `Harness`, `Harness<A, B>`, `Harness<'a>`,
  `<X as Tr>::Harness<T>`, `&Harness<T>` (D5 error), `Option<Harness<T>>`
  (ordinary fixture), and `#[from(x)] x: Harness<T>` (ordinary fixture).
  Artefact:
  `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_extractor/tests.rs`.
  Evidence: `cargo test -p rstest-bdd-macros harness_extractor` fails to
  compile before EP-M3 and passes after. Non-vacuity: each row carries its
  expected class, fixed when the row is written; a seeded mutation that
  compares the final segment with `ends_with("Harness")` must fail the
  `MyHarness<T>` row, and one that skips the `qself` check must fail the
  qualified-path row.
- O2 (access-mode mapping). Obligation: `Harness<T>` maps to `Shared` and
  `HarnessMut<T>` to `Exclusive`; marker, legacy, and parameter-named `&T` and
  owned `T` map to `Shared`; their `&mut T` maps to `Exclusive`; every other
  parameter maps to `None` (no `harness` field). Method: `rstest` table (finite
  partition). Artefact: the same module. Non-vacuity: every spelling-by-shape
  cell appears; swapping the two extractor arms must fail two rows.
- O3 (conflict check, theorem T). Obligation: `check_harness_borrows` rejects
  a signature iff `not Compatible(S, X)`. Method: exhaustive enumeration of
  every signature of length 0 to 3 over a twelve-letter alphabet: ordinary
  fixture `&mut World`; `Harness<T>`; `Harness<'_, T>`; `HarnessMut<T>`; marker
  `&T`, `&mut T`, and owned `T`; legacy `#[from(..)]` `&T`, `&mut T`, and owned
  `T`; the parameter-named `&mut T` (at most once per signature, because
  parameter names must be unique); and the D4 opt-out
  `#[from(other)] other: Harness<Foo>`, which must map to `None`. Each
  signature is rendered as real tokens and passed through `extract_args`. The
  oracle acquires the same borrows on a real `RefCell<u32>`, holding every
  guard until the end, using a letter-to-mode table written out in the test
  rather than taken from the O2 code. By L2′, length 3 is complete; the run
  covers 1,885 signatures before deduplicating the parameter-named letter.
  Artefact: `.../harness_extractor/borrow_plan.rs` and
  `.../harness_extractor/borrow_plan/tests.rs`. Evidence: red before EP-M4 (the
  stub accepts every signature, so the oracle disagrees), green after.
  Non-vacuity: the test asserts that both verdicts occur and that each of the
  classes `(X=0,S=0)`, `(X=0,S>0)`, `(X=1,S=0)`, `(X=1,S>0)`, `(X=2,S=0)`, and
  `(X>=2,S>=1)` is reached. A seeded mutation `X <= 1` must be rejected on
  `[HarnessMut, Harness]`; a mutation that keys the check on
  `harness.is_some()` of extractors only must be rejected on
  `[HarnessMut, marker &T]`; a mutation that treats any `Harness`-typed
  parameter as harness access must be rejected on the opt-out letter.
- O4 (observational equivalence of spellings). Obligation: for each context
  state in {absent; owned `T`; owned `U != T`; shared-inserted `T` via
  `insert_harness_context`; owned `T` with an outstanding exclusive guard; owned
  `T` with an outstanding shared guard; step-returned override of type `T`
  present} and each mode in {shared, exclusive}, a step taking
  `Harness`/`HarnessMut` and the same step taking the marker reference produce
  the same outcome: both call the step, or both return
  `StepError::MissingFixture` with the same `name` and `step`, differing only in
  `ty` (written extractor type versus `T`, per D6). Method: `rstest` table
  over the 14 cells (finite partition, exhaustive), invoking the registered
  wrappers through the step registry. Artefact: the `equivalence` module of
  `crates/rstest-bdd/tests/harness_extractors.rs`. Non-vacuity: each cell
  carries an explicit expected outcome rather than only comparing the two
  spellings with each other, and the full `StepError` values are compared with
  `pretty_assertions::assert_eq!`. The expected column has six cells that call
  the step (owned `T` and override in both modes, and shared mode for the
  shared-inserted and outstanding-shared states) and eight errors. Outstanding
  guards are taken directly on the backing `RefCell<Box<dyn Any>>` passed to
  `insert_owned_harness_context`, never through the `StepContext`, because the
  wrapper borrows the context mutably. A token-level companion test asserts
  that the extractor's guard declaration contains the marker's declaration for
  `&T`/`&mut T` verbatim.
- O5 (transparency). Obligation: `Harness<T>` dereferences to the same object
  the marker's `&T` would receive (pointer equality), and writes through
  `HarnessMut<T>` are visible to the next step. Method: unit tests in
  `crates/rstest-bdd/src/context/extractors.rs` plus the behavioural suite.
  Non-vacuity: the behavioural scenario asserts a value only a successful write
  could produce.
- O6 (reserved key hidden from authors). Obligation: the new behavioural
  suite, the new compile-pass fixture, and the migrated example steps do not
  contain `rstest_bdd_harness_context`. Method: an `rg` command in EP-M5's
  acceptance step, run alongside the same command on a known legacy fixture
  (`crates/rstest-bdd/tests/fixtures_macros/harness_context_coexist.rs`), which
  must match, proving the search works.
- O7 (foreign-type rejection). Obligation: a recognized parameter whose type
  is not `rstest_bdd::Harness`/`HarnessMut` fails to compile with the
  `on_unimplemented` message that names the `#[from(..)]` opt-out, spanned at
  the parameter. Method: `trybuild` compile-fail fixture checked through the
  existing normalizer. Non-vacuity: the compile-pass fixture includes the
  opt-out `#[from(harness)] harness: Harness<Foo>` with a local `Harness<T>`
  type, which compiles.
- O8 (metadata and generated shape). Obligation: an extractor contributes the
  reserved key to `Step::fixtures` and a `FixtureRequirement` whose `ty` is the
  written, lifetime-normalized type; the synchronous and asynchronous wrappers
  declare the guard before the call. Method: `insta` snapshots of generated
  wrappers for sync and async steps taking `Harness`, `HarnessMut` with
  `&mut World`, and the marker (multivariant output whose consistency matters),
  plus a run-time test that missing-context pre-validation reports the harness
  hint. Non-vacuity: the run-time test asserts the written type, which only the
  new metadata path produces.
- O9 (sealing). Obligation: downstream code cannot implement `HarnessView`.
  Method: `trybuild` compile-fail fixture implementing it for a local type.
- O10 (shared shape predicate). Obligation: `classify_harness_context` rejects
  a type under the marker exactly when `extractor_shape` returns `Some`. Method:
  `rstest` rows in the marker's test module. Non-vacuity: one mutation to
  `extractor_shape` must break a row in both modules.
- O11 (patterns). Obligation: `mut cx: HarnessMut<T>`, `_cx: Harness<T>`, and
  `_: Harness<T>` are accepted, keep the user's pattern in the re-emitted
  signature, and run. Method: compile-pass fixture plus one behavioural step
  using `mut cx`. Non-vacuity: the `mut cx` step writes through `DerefMut`,
  which fails to compile if `mut` is lost.
- O12 (asynchronous steps at run time). Obligation: an `async fn` step taking
  `Harness<TokioTestContext>` runs under `TokioHarness`. Method: a scenario
  added to `crates/rstest-bdd-harness-tokio/tests/scenario_macros.rs`, whose
  step awaits only an immediately-ready future (R8). Non-vacuity: the step
  asserts a property of the context that only a real Tokio context has.
- O13 (lint posture, C9). Obligation: steps taking `Harness<T>`,
  `Harness<'_, T>`, and `mut cx: HarnessMut<T>` that write through it compile
  without warnings under `-D warnings -D clippy::pedantic -D rust_2018_idioms`.
  Method: permanent `ui_lints` binary. Non-vacuity: EP-M1 records that the same
  binary fails under those flags when the extractor is replaced by a guard
  newtype (or a non-`Copy` `Harness`), proving the lane detects the lint.

### Where formal verification is and is not warranted

Verus is not warranted (D9). Lemma L2′ and the characterization L3 are recorded
here and in ADR-024 as the written argument; the O3 enumeration is complete by
L2′, so no generated-length property test is planned. Tests continue to cover
the executable integration: O1, O2, O10, and O11 cover token classification,
O4, O5, and O12 cover run-time behaviour, O7 and O9 cover the type-level
contract, and O8 and O13 cover generated code. If implementation makes borrow
acquisition depend on run-time ordering or on state carried between steps, D9
is revisited before continuing.

## Plan of work

Each milestone ends with the commit gates green and a commit. Gates run one
after another, never in parallel, with output teed to `/tmp`.

### EP-M0 — orientation and green baseline (no code changes)

Rebase onto `origin/main`. Re-read the 11.2.2 and 11.2.3 plans' status lines
(R3). Run the commit gates and record the baseline. Re-measure file lengths.
Read `StepContext::insert_value`, `try_borrow`, and `try_borrow_mut` and
confirm A2 as restated. Search for steps with more than one harness-context
parameter in `crates/`, `examples/`, and `tests/fixtures/` and confirm none
conflicts (R5):

```bash
rg -n "harness_context" crates examples tests/fixtures --type rust
```

Acceptance: baseline gate logs exist, and `Surprises & discoveries` records any
drift.

### EP-M1 — permanent lint-posture gate (prototype first)

Add a binary `harness_extractor_lint_clean` under
`crates/rstest-bdd/tests/ui_lints/src/bin/` and register it in
`run_lint_ui_tests` in `crates/rstest-bdd/tests/trybuild_macros.rs` with
`-D warnings -D clippy::pedantic -D rust_2018_idioms`. In this milestone it
uses provisional local definitions of the D3 view types (a `Copy`
`Harness<'a, T>(&'a T)` and `HarnessMut<'a, T>(&'a mut T)` with `Deref`) and
plain functions shaped like steps. Run it and record whether
`needless_pass_by_value` and `elided_lifetimes_in_paths` fire for
`fn step(cx: Harness<'_, Ctx>)`, `fn step(cx: Harness<Ctx>)`, and
`fn step(mut cx: HarnessMut<'_, Ctx>)` that writes through `DerefMut`. Then
temporarily replace `Harness` with a non-`Copy` guard newtype and record that
the lane fails (O13's negative control).

Go/no-go: if `Copy` views and `mut` bindings are clean, continue. If
`elided_lifetimes_in_paths` fires on `Harness<Ctx>`, D14 covers it in EP-M3. If
`HarnessMut` with `mut` is still linted, stop and escalate (Tolerances).

Acceptance: transcripts in `Artefacts and notes`; the binary stays in the
repository and is switched to the real `rstest_bdd` types and `#[given]` steps
in EP-M5.

### EP-M2 — run-time extractor types (red, green, refactor)

Red: add `crates/rstest-bdd/src/context/extractors.rs` with O5 unit tests and
doctests, and a test in `crates/rstest-bdd-policy` that checks the new path
constants against the real type names (written to fail until both exist). Run
`cargo test -p rstest-bdd extractors`; expect `cannot find type Harness`.

Green: define `Harness<'a, T: ?Sized>(&'a T)` and
`HarnessMut<'a, T: ?Sized>(&'a mut T)` with no bounds on the struct definitions;
`new` constructors; `Clone`/`Copy` for `Harness` (manual impls, so `T` need
not be `Clone`); `Deref` and `AsRef<T>` for both; `DerefMut` and `AsMut<T>` for
`HarnessMut`; `Debug` where `T: Debug`. Add the sealed `HarnessView` trait in a
`#[doc(hidden)] pub mod __macro_support` with
`#[diagnostic::on_unimplemented]`. Re-export `Harness` and `HarnessMut` from
the crate root beside `FixtureRef`. Add `HARNESS_VIEW_PATH` and
`HARNESS_VIEW_MUT_PATH` to `crates/rstest-bdd-policy/src/lib.rs`, following the
`STD_HARNESS_PATH` precedent. Write rustdoc with runnable examples per
`docs/rust-doctest-dry-guide.md`, including the unit-testing pattern
`title_is_shown(Harness::new(&ctx))`.

Refactor: `context/mod.rs` gains only `mod extractors;`; if the re-export
pushes it past 400 lines, re-export from `context/harness.rs` instead.

Acceptance: `cargo test -p rstest-bdd extractors` and
`cargo test -p rstest-bdd-policy` pass; commit gates green; commit.

### EP-M3 — macro recognition and code generation

Red:

1. Add
   `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_extractor/mod.rs`
   with `extractor_shape` and `classify_harness_extractor` stubs that return
   `None` and `Ok(false)`, and `tests.rs` with the O1, O2, O10, and D5 tables.
2. Add insta snapshot tests (O8) in a new
   `crates/rstest-bdd-macros/src/codegen/wrapper/extractor_snapshot_tests.rs`.
3. Add `crates/rstest-bdd/tests/features/harness_extractors.feature` and the
   steps and scenarios of `crates/rstest-bdd/tests/harness_extractors.rs`
   (EP-M5 extends it).

Run `cargo test -p rstest-bdd-macros harness_extractor`, expecting table
failures, and `cargo test -p rstest-bdd --test harness_extractors`, expecting a
compile error because `HarnessMut<'_, CountingContext>` is treated as an owned
fixture named `cx` and cloned (`FixtureRef` and the view are not `'static`).

Green:

1. Add `harness: Option<HarnessRequest>` to `Arg::Fixture` and to the
   borrowed `FixtureArg` (D13). Update every construction and destructuring
   site: `args/classify/fixture_or_step.rs` (set the field when the resolved
   name is the reserved key), `args/classify/harness_context/mod.rs`, the
   exhaustive `Debug` impl in `args/mod.rs`, `bind_arguments` in
   `arguments/mod.rs`, and the tests in `arguments/tests/mod.rs`,
   `arguments/tests/helpers.rs`, and `arguments/tests/prop_tests.rs`. If
   `arguments/mod.rs` would pass 400 lines, first move `bind_arguments` helpers
   into a sibling module as a separate refactor commit.
2. Implement `extractor_shape` and `classify_harness_extractor` (D4, D5,
   D14), called from `extract_args` right after `classify_harness_context`. The
   classifier rewrites the parameter's type in place (`'_` insertion) so the
   re-emitted user signature carries it, and records the wrapper type with
   every lifetime replaced by `'_`.
3. Extend `classify_harness_context` to reject an extractor-shaped type
   (D5, O10), and `parse_from_attribute` handling to reject
   `#[from(rstest_bdd_harness_context)]` on one.
4. Add `arguments/harness_extractor.rs` with `gen_harness_extractor_decl`.
   It calls the existing `&T`/`&mut T` declaration generators for a hidden
   binding of type `&Inner`/`&mut Inner`, then emits, spanned at the parameter
   type with `quote_spanned!`:

   ```rust,ignore
   let binding: #wrapper_ty =
       <#wrapper_ty as #path::__macro_support::HarnessView>::from_borrow(#hidden_binding);
   ```

   `gen_fixture_decls` dispatches to it when `harness` holds an extractor.
5. Make `effective_fixture_type` take the whole `FixtureArg` and return the
   written wrapper type for extractors (D6, O8).

Acceptance: O1, O2, O10 tables pass; snapshots reviewed and accepted with
`cargo insta review`; the behavioural suite compiles and its happy-path
scenarios pass; commit gates green; commit.

### EP-M4 — harness-borrow conflict check

Red: first record, in `Artefacts and notes`, a run of a temporary test step
taking `HarnessMut<T>` and `Harness<T>` that compiles and fails at run time with
`MissingFixture` (evidence for L3 and R5); delete the temporary step. Then add
`.../harness_extractor/borrow_plan.rs` exposing
`check_harness_borrows(args: &[Arg]) -> syn::Result<()>` with a stub that
accepts everything, and `borrow_plan/tests.rs` with the O3 enumeration. Run it
and observe the oracle disagreement.

Green: implement the check from the `harness` field and `Compatible(S, X)`;
call it once from `extract_args` after the parameter loop. The error spans the
first parameter that makes the set incompatible and names both parameters and
their modes, for example:

```plaintext
conflicting harness-context borrows: `cx` borrows it mutably (HarnessMut<..>)
and `view` borrows it shared (Harness<..>); a step may hold one mutable borrow
or any number of shared borrows
```

Mutation check: run

```bash
cargo mutants -p rstest-bdd-macros \
  --file 'crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_extractor/**'
```

and record surviving mutants; any survivor in `borrow_plan.rs` or
`extractor_shape` gets a new table row. Also apply the three O3 seeded
mutations by hand and record that each is caught. (The nightly mutation
workflow in `.github/workflows/mutation-testing.yml` will also mutate these
files.)

Acceptance: O3 passes; mutation transcripts recorded; commit gates green;
commit.

### EP-M5 — compile tests, behavioural suite, and examples

Compile-pass fixture
`crates/rstest-bdd/tests/fixtures_macros/harness_extractors_pass.rs`,
registered in `crates/rstest-bdd/tests/trybuild_macros.rs`, covering
`Harness<T>`, `Harness<'_, T>`, `Harness<'a, T>` in a step with a named
lifetime, `rstest_bdd::HarnessMut<T>`, coexistence with `&mut World`, two
`Harness<T>` in one step, the marker path, an `async fn` step, the O11
patterns, and the D4 opt-out with a local `Harness<T>` type.

Compile-fail fixtures in `crates/rstest-bdd/tests/ui_macros/`, each with a
`.stderr` file and a `UiFixtureCase` entry:

- `harness_extractor_misuse.rs`: one step per D5 case.
- `harness_borrow_conflicts.rs`: `HarnessMut` with `HarnessMut`, and
  `HarnessMut` with a marker `&T` (proving the check spans spellings).
- `harness_extractor_foreign_type.rs` (O7), through the existing normalizer.
- `harness_extractor_sealed.rs` (O9).

Behavioural suite: `crates/rstest-bdd/tests/harness_extractors.rs` with
`crates/rstest-bdd/tests/features/harness_extractors.feature`, using a local
`CountingHarness` modelled on
`crates/rstest-bdd/tests/concurrent_mut_fixtures.rs`, plus its `equivalence`
module (O4):

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

  Scenario: Read the harness context twice in one step
    Given the harness context starts at zero
    Then a step reading through two Harness parameters sees the same context

  Scenario: Request the harness context without a harness
    Given a step that reads the harness context through Harness
```

The last scenario is bound by a `#[scenario]` without `harness = ..` and is
expected to fail. It follows the pattern in
`crates/rstest-bdd/tests/execution_error.rs`: the test asserts that the failure
is a missing-fixture error carrying the harness hint and the written type
`Harness<'_, CountingContext>`, and pins the current message text (R7) without
spelling the reserved key in this file (it compares against
`rstest_bdd::RSTEST_BDD_HARNESS_CONTEXT_FIXTURE`).

Add the O12 asynchronous scenario to
`crates/rstest-bdd-harness-tokio/tests/scenario_macros.rs` and its feature file.

Switch the EP-M1 lint binary to real `#[given]` steps using
`rstest_bdd::Harness` and `HarnessMut`.

Examples: migrate `examples/gpui-counter/tests/counter.rs` to
`Harness<gpui::TestAppContext>`; keep `examples/tokio-reminders` on the marker
so both v1 spellings stay demonstrated end to end.

Acceptance: `cargo test -p rstest-bdd --test trybuild_macros`,
`cargo test -p rstest-bdd --test harness_extractors`,
`cargo test -p rstest-bdd-harness-tokio`, `cargo test -p gpui-counter`, and
`cargo test -p tokio-reminders` pass; the O6 search returns nothing for the new
files and a match for the legacy fixture; commit gates green; commit.

### EP-M6 — documentation, ADR-024, and prelude hand-off

- Write `docs/adr-024-typed-harness-context-extractors.md` (Status Proposed
  until the implementation PR merges, then Accepted), recording D1, D3–D7, D9,
  D11, D13, and D14 with options considered, the v1 spelling set, the L2′/L3
  argument, migration impact, and the inputs it leaves for roadmap 12.2.4 (no
  `'static` bound on the view types) and 12.2.6 (views are held for a whole
  step, across `.await`). Add the `Amended by ADR-024` banner and
  `## Amendments` entry to ADR-007. Index ADR-024 in `docs/contents.md`.
- `docs/rstest-bdd-design.md` §2.7.6.5: mark the typed-extractor candidate
  delivered and summarize the recognition rule, the view types, and the
  borrow-conflict rule, referencing ADR-024. Correct §2.7.5's GPUI prose that
  still says steps reach context through `rstest_bdd_harness_context`.
- `docs/users-guide.md`: extend Table 2 in "Harness adapter and attribute
  policy" with `Harness<T>` and `HarnessMut<T>` rows, lead with extractors,
  migrate GPUI worked examples from `#[from(rstest_bdd_harness_context)]`,
  document D4 (opt-out, aliases), D5, the borrow-conflict rule, unit-testing
  steps with `Harness::new`, and a troubleshooting entry quoting the R7
  messages; update "Fixture key versus parameter name".
- `docs/developers-guide.md`: a new subsection "The harness-extractor
  classifier stage" after "The `#[harness_context]` classifier stage",
  documenting `HarnessRequest` on `Arg::Fixture`, `extractor_shape` and its
  reuse policy (owned by `harness_extractor/`; callers limited to the marker
  and extractor classifiers), `HarnessView`, `check_harness_borrows`, the
  lifetime rewriting, and the `ui_lints` lane flags.
- `docs/testing-strategy.md`: the real-`RefCell` oracle enumeration pattern
  and why L2′ bounds it.
- `docs/v0-6-0-migration-guide.md`: update "Adopt harness context", add a
  v0.7.0 subsection mapping the existing spellings to extractors, and add the
  new conflict error to "Common errors and fixes" and the migration checklist.
- `docs/known-issues.md`: the R7 residual.
- `crates/rstest-bdd/CHANGELOG.md` and `docs/CHANGELOG.md` entries.
- `docs/roadmap.md`: add two proposed items under 12.2 with finish lines —
  harness-aware run-time diagnostics (D6, R7), and deprecating the
  parameter-named and `#[from(rstest_bdd_harness_context)]` spellings (D1).
- Prelude (R3): if `rstest_bdd::prelude` exists, add `Harness` and
  `HarnessMut` and update its table, doctest, and parity gate together;
  otherwise record the admission decision for 11.2.2 in
  `Outcomes & retrospective` and the PR description.

Acceptance: `make fmt`, `make markdownlint`, and `make nixie` pass (stage
documentation by path, because `make fmt` may reformat unrelated files); commit
gates green; commit.

### EP-M7 — roadmap tick, full gates, and pull request

Mark 12.2.1 done in `docs/roadmap.md` with a completion note naming this plan
and ADR-024. Run the commit gates plus `make markdownlint` and `make nixie` on
the final commit. Complete `Outcomes & retrospective`, reconcile every
discovery with ADR-007, ADR-012, ADR-024, and §2.7.6.5, set Status to COMPLETE,
then push and open the implementation PR.

## Milestones and plateaus

- EP-M0. Outcome: verified baseline, no code change. Requirements: none.
  Evidence: gate logs. Conformance check: A2 confirmed or plan revised; no
  conflicting harness steps found. Recovery: nothing to revert. Remaining gaps:
  all. Compatibility decision: none.
- EP-M1. Outcome: a permanent Clippy lane encodes C9 against provisional
  view types. Requirements: C9, O13. Evidence: lint transcripts, including the
  negative control. Conformance check: no public interface change. Recovery:
  revert the commit. Remaining gaps: real types and steps. Compatibility
  decision: none.
- EP-M2. Outcome: `Harness`/`HarnessMut` usable from handwritten code and
  unit tests. Requirements: FL-1 and FL-2 (run-time half), O5. Evidence: unit
  tests and doctests. Conformance check: C3 and C4 against `cargo doc` output
  for new public items. Recovery: revert the commit. Remaining gaps: macros do
  not yet recognize the types. Compatibility decision: none (additive, pre-1.0).
- EP-M3. Outcome: steps can take extractors. Requirements: FL-1, FL-2, FL-3,
  O1, O2, O8, O10, O11 (classification half). Evidence: tables, snapshots,
  happy-path scenarios. Conformance check: C1 (key unchanged), C6 (lengths),
  D13 field set by every spelling. Recovery: revert; EP-M2 stands alone.
  Remaining gaps: conflict check, compile tests, documentation. Compatibility
  decision: none.
- EP-M4. Outcome: conflicting harness borrows are compile errors for every
  spelling. Requirements: O3, D7. Evidence: enumeration, mutation transcripts,
  pre-change run-time failure. Conformance check: the C2 exception holds only
  for always-failing steps (R5). Recovery: revert; EP-M3 stands alone.
  Remaining gaps: compile tests, documentation. Compatibility decision: none
  (pre-1.0).
- EP-M5. Outcome: user-observable behaviour proven end to end.
  Requirements: FL-1 to FL-4, O4, O6, O7, O9, O11, O12, O13. Evidence:
  trybuild, behavioural suite, Tokio scenario, examples, lint lane. Conformance
  check: examples use public API only. Recovery: revert fixture additions.
  Remaining gaps: documentation. Compatibility decision: none.
- EP-M6. Outcome: documentation and ADRs consistent with the code.
  Requirements: DD-2.7.6.5, ADR-007 amendment, ADR-024. Evidence: Markdown
  gates. Conformance check: every decision in this plan appears in ADR-024 or
  the design document. Recovery: revert the documentation commit. Remaining
  gaps: roadmap tick. Compatibility decision: none.
- EP-M7. Outcome: roadmap item done. Requirements: RM-12.2.1 in full.
  Evidence: all gates green on the final commit; trace links checked against
  the evidence named in `Conformance basis`. Conformance check: no unrecorded
  deviation remains. Recovery: none needed. Remaining gaps: the two proposed
  follow-up roadmap items. Compatibility decision: none.

## Concrete steps

All commands run from the repository root. Long outputs are teed to `/tmp`:

```bash
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-rstest-bdd-$BRANCH.out"
make lint 2>&1 | tee "/tmp/lint-rstest-bdd-$BRANCH.out"
make test 2>&1 | tee "/tmp/test-rstest-bdd-$BRANCH.out"
```

Record the revision a gate ran against (`git rev-parse HEAD` before and after)
in the log name or the commit message; do not edit files while a gate runs.

Focused loops used during red-green-refactor:

```bash
cargo test -p rstest-bdd-macros harness_extractor
cargo test -p rstest-bdd extractors
cargo test -p rstest-bdd --test harness_extractors
cargo test -p rstest-bdd --test trybuild_macros
cargo test -p rstest-bdd-harness-tokio
cargo insta review
python3 scripts/check_rs_file_lengths.py
```

Expected red transcript at the start of EP-M3 for the behavioural suite
(abridged; exact wording depends on the compiler release):

```plaintext
error[E0277]: the trait bound `HarnessMut<'_, CountingContext>: Clone` is not satisfied
```

or an error that the view type is not `'static`. Either confirms the parameter
is still treated as an ordinary owned fixture.

Expected green transcript at the end of EP-M4 (abridged):

```plaintext
test ...::borrow_plan::tests::classifier_matches_refcell_oracle_up_to_length_three ... ok
test ...::borrow_plan::tests::enumeration_reaches_every_borrow_class ... ok
```

## Validation and acceptance

Red-green-refactor evidence is recorded per milestone in `Artefacts and notes`:
the red command and its expected failure, the green command and its pass, and
the post-refactor gate runs.

Acceptance criteria (behavioural):

- `cargo test -p rstest-bdd --test harness_extractors` passes all four
  scenarios and the O4 table.
- `trybuild` reports every new compile-fail fixture's expected diagnostic,
  and the compile-pass fixture builds.
- The `ui_lints` lane passes `harness_extractor_lint_clean` under
  `-D warnings -D clippy::pedantic -D rust_2018_idioms`.
- `examples/gpui-counter` and `examples/tokio-reminders` pass under
  `make test`.
- The O6 search finds no `rstest_bdd_harness_context` in the new behavioural
  suite, compile-pass fixture, or migrated example steps.

Quality criteria:

- Tests: `make test` passes.
- Verification: O1–O13 discharged as described; the EP-M4 mutations are
  caught.
- Lint and format: `make check-fmt`, `make lint`, `make markdownlint`, and
  `make nixie` pass.
- No Verus or Kani command applies (D9).

## Idempotence and recovery

Every milestone is a commit on this branch; rolling back is `git revert` of
that milestone's commits. Gate runs are repeatable. `cargo insta review` only
rewrites files under `snapshots/`. Trybuild `.stderr` files are regenerated with
`TRYBUILD=overwrite` only for fixtures added by this plan, and their diffs are
inspected before committing. The EP-M4 temporary test step is deleted in the
same milestone.

## Artefacts and notes

To be filled during implementation: EP-M1 Clippy transcripts and negative
control, the EP-M4 pre-change run-time failure, mutation transcripts, and
red/green transcripts for each milestone.

## Interfaces and dependencies

No new dependencies. Test crates already in the workspace: `rstest`,
`googletest` (with the `proptest` feature), `pretty_assertions`, `insta`,
`trybuild`, and `cargo-mutants` (installed locally and used by CI).

In `crates/rstest-bdd/src/context/extractors.rs`:

```rust
/// Shared view of the context supplied by the scenario's harness adapter.
pub struct Harness<'a, T: ?Sized>(&'a T);

/// Exclusive view of the context supplied by the scenario's harness adapter.
pub struct HarnessMut<'a, T: ?Sized>(&'a mut T);

impl<'a, T: ?Sized> Harness<'a, T> {
    pub const fn new(context: &'a T) -> Self { Self(context) }
}

impl<'a, T: ?Sized> HarnessMut<'a, T> {
    pub const fn new(context: &'a mut T) -> Self { Self(context) }
}

// Clone and Copy for Harness; Deref<Target = T> for both; DerefMut for
// HarnessMut; AsRef<T> for both; AsMut<T> for HarnessMut; Debug where T: Debug.
```

In `#[doc(hidden)] pub mod __macro_support` of `rstest-bdd`:

```rust
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an rstest-bdd harness extractor",
    note = "parameters typed `Harness<T>` or `HarnessMut<T>` request the harness context; \
            to inject an ordinary fixture of this type, add `#[from(fixture_name)]`"
)]
pub trait HarnessView: Sized + sealed::Sealed {
    /// `&'a T` for `Harness<'a, T>`, `&'a mut T` for `HarnessMut<'a, T>`.
    type Borrow;
    fn from_borrow(borrow: Self::Borrow) -> Self;
}
```

In `crates/rstest-bdd-policy/src/lib.rs`:

```rust
pub const HARNESS_VIEW_PATH: &[&str] = &["rstest_bdd", "Harness"];
pub const HARNESS_VIEW_MUT_PATH: &[&str] = &["rstest_bdd", "HarnessMut"];
```

In
`crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_extractor/`:

```rust
pub(crate) enum HarnessAccess { Shared, Exclusive }
pub(crate) enum HarnessForm { Extractor { inner: syn::Type }, Reference, MutReference, Owned }
pub(crate) struct HarnessRequest { pub access: HarnessAccess, pub form: HarnessForm }
pub(crate) fn extractor_shape(ty: &syn::Type) -> Option<HarnessRequest>;
pub(crate) fn classify_harness_extractor(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    placeholders: &HashSet<String>,
) -> syn::Result<bool>;
pub(crate) fn check_harness_borrows(args: &[Arg]) -> syn::Result<()>;
```

## Revision notes

- 2026-10-15: initial draft written from reconnaissance and prior-art
  research.
- 2026-10-15: revised after the four-reviewer design review. The extractor
  types changed from guard newtypes to `Copy`/exclusive views over references
  held by the wrapper's existing guard code (D3), because Clippy's
  `needless_pass_by_value` would otherwise fire on every read-only step (R1)
  and the marker's `&'static T` fallback would be lost. Lifetimes are
  normalized (D14) to avoid E0261 and `elided_lifetimes_in_paths`. A2 was
  corrected for step-returned overrides; L2 was replaced by the saturation
  lemma L2′, which makes the length-3 enumeration complete and removes the
  property test; O9–O13 were added; the lint prototype became a permanent
  `ui_lints` gate inside the repository rather than a `/tmp` crate; trybuild
  fixtures were consolidated (R6); the run-time diagnostic now reports the
  written type and gains follow-up roadmap items (D6); the v1 spelling set was
  named (D1); and the naming question was recorded for approval (D11).
  Remaining work is unchanged in shape: approval, then EP-M0 to EP-M7.
