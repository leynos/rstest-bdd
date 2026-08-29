# Prevent a step returning a `Result` type alias from silently passing on `Err`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: BLOCKED — EP-M0 disproved the collision-safety acceptance criterion.

Roadmap item: 11.3.1. Origin: `leynos/rstest-bdd#573` and the gauss
v0.6.0-beta3 validation matrix.

## Purpose / big picture

Today a `#[given]`, `#[when]`, or `#[then]` step function whose return type is
a *type alias* for `Result<T, E>` has its `Err` silently discarded. The
scenario passes. For a test framework this is the worst possible defect class:
a real assertion becomes a no-op with no compile error, no warning, and no
runtime diagnostic.

After this change, a step written like this fails its scenario, exactly as if
the author had spelled `Result<(), String>` in the signature:

```rust
type MyResult<T> = Result<T, String>;

#[then("the thing is false")]
fn the_thing_is_false() -> MyResult<()> {
    Err("this should fail the scenario".to_string())
}
```

You can observe the change directly. Before the change, running the new
behavioural test reports a pass where a failure is expected; after the change
it reports the scenario failing with the message `alias failure`. The same
holds for `anyhow::Result<T>` and `std::io::Result<T>`, which are affected by
exactly the same defect today.

Ordinary value-returning steps keep working with no annotation. A step
returning `-> Score` (where `type Score = u32;`) still stores `u32` as a
fixture override. Nothing in the 90.8% of steps that return `()` changes at
all.

## Definitions

Terms used throughout, defined once so the rest reads plainly.

- **Step function**: a function annotated `#[given]`, `#[when]`, or `#[then]`.
- **Wrapper**: the function the step macro generates around a step function. It
  extracts arguments, calls the step, catches unwinds, and converts the return
  value into the runtime's common representation.
- **Classification**: deciding whether a step's return value should be treated
  as a fallible outcome (`Result`) or as a payload value.
- **Payload**: a value returned by a step, boxed as `Box<dyn Any>` and offered
  to the scenario's fixtures as an override.
- **Return-kind hint**: the existing `result` / `value` argument on a step
  attribute, for example `#[when("pattern", value)]`.
- **Autoref specialization**: a stable-Rust technique in which two disjoint
  traits, one implemented on a type and one on a reference to that type, are
  resolved by the compiler's method-probe ordering rather than by impl
  coherence. It only works inside macro-generated code. The canonical write-up
  is dtolnay's case study, and `anyhow!` uses it in production.
- **False green**: a test that reports success when the behaviour it asserts is
  actually broken.

## Constraints

Hard invariants. Violating one requires escalation, not a workaround.

- Stable Rust only. No nightly features, no `feature(specialization)`, no
  `auto_traits`, no `negative_impls`. The workspace minimum supported Rust
  version is `1.85` (`Cargo.toml`, `rust-version`). This constraint is the
  entire reason ADR-002 exists.
- `unsafe_code = "forbid"` workspace-wide. Nothing in this change needs it.
- No `.rs` file may exceed 400 lines
  (`scripts/check_rs_file_lengths.py`, run by `make lint`). Do **not** add
  entries to `scripts/rs-length-allowlist.txt`; that file's own header calls
  its entries temporary exemptions pending refactor.
- Every module needs a `//!` header; every item needs a `///` doc comment
  (`missing_docs = "deny"`, `clippy::missing_docs_in_private_items = "deny"`).
- `clippy::allow_attributes = "deny"` and
  `clippy::allow_attributes_without_reason = "deny"` in repository-authored
  source: use `#[expect(..., reason = "...")]`, not `#[allow(...)]`. This does
  **not** necessarily extend to tokens the macro emits into a *user's* crate;
  see EP-M0.
- `clippy::expect_used = "deny"` and `clippy::unwrap_used = "deny"`. In test
  helper functions that are not inside `#[cfg(test)]` or `#[test]`, use
  let-else plus `panic!`, never `.expect()`.
- Generated wrapper code must remain lint-clean in a downstream crate building
  under `-D warnings`. An emitted `#[expect(...)]` whose lint stops firing
  produces `unfulfilled_lint_expectations`, which is a hard error for that
  adopter and which they cannot silence.
- The scenario-function classification path (`#[scenario]`, `scenarios!`) is
  **out of scope**. Roadmap 11.3.2 is already complete and an alias on a
  scenario body is a loud compile error, not a false green.
- Prose is en-GB-oxendict (`-ize`, `-yse`, `-our`), gated by `typos` via
  `make markdownlint`. `typos.toml` is generated; never hand-edit it.
- Markdown: 80-column paragraph wrap, 120-column code blocks, `-` bullets,
  no wrapping of tables or headings.

## Tolerances (exception triggers)

Stop and escalate when any of these is reached. Do not work around them.

- **Scope**: more than 26 files changed, or more than 1000 net added lines.
  (Baseline estimate is 22–26 files and +750 to +950 net.)
- **Mechanism**: if EP-M0 cannot produce an emission shape that is
  simultaneously correct for all probe cases and clean under
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, stop.
  Do not ship a shape that requires adopters to silence lints.
- **Interface**: if the change requires altering any non-`#[doc(hidden)]`
  public item of `rstest-bdd` or `rstest-bdd-macros`, stop.
- **Dependencies**: if a new external crate dependency is required, stop.
- **File-size pressure**: if `crates/rstest-bdd-macros/src/macros/mod.rs`
  cannot be brought under 400 lines by a mechanical extraction (no behaviour
  change), stop.
- **Iterations**: if a gate still fails after 3 focused fix attempts, stop and
  report the exact log path.
- **Release line**: this targets v0.6.0 (`DEC-009`). If v0.6.0 is tagged before
  EP-M3 completes, stop — the change would then be a breaking change to a
  released API and needs a fresh release-line decision.
- **Regression breadth**: if making the new behavioural tests pass requires
  editing more than 3 pre-existing test assertions, stop — that indicates the
  change is broader than analysed.

## Risks

- Risk: the emitted trait prelude falls out of one emission path during a
  future refactor, silently reinstating the original false green.
  Severity: high. Likelihood: medium.
  Mitigation: emit the whole dispatch (prelude plus call) from exactly one
  function; pin it with an `insta` snapshot of the token stream; add the
  seeded-fault negative control described in `Verification plan`.

- Risk: the generated `#[expect(clippy::unnecessary_wraps)]` becomes
  unfulfilled for dispatched steps and breaks downstream `-D warnings` builds.
  Severity: high. Likelihood: high (verified to occur).
  Mitigation: EP-M4 reworks `wrapper_expect_lint_names` and adds a
  `tests/ui_lints` fixture that builds a dispatched step under
  `#![deny(warnings)]`.

- Risk: the `Ok` payload shape changes from `Result<T, E>` to `T`, so payloads
  that were previously dropped as `InsertOutcome::NoMatch` now override
  fixtures — silent state mutation in adopter suites.
  Severity: high. Likelihood: medium.
  Mitigation: pin the new behaviour with
  `step_return_alias_ok_overrides_fixture.feature`; lead the migration-guide
  entry with it.

- Risk: version skew. `rstest-bdd-macros` does not depend on `rstest-bdd`; a
  user who updates one and not the other gets `E0433: could not find
  __rstest_bdd_step_return in rstest_bdd` at every step site.
  Severity: medium. Likelihood: medium.
  Mitigation: record the constraint in `docs/releasing-crates.md` and the
  migration guide; consider a versioned marker constant (see `DEC-008`).

- Risk: trybuild `.stderr` snapshots of cross-crate trait-bound errors are the
  most toolchain-volatile class of rustc output and will churn on upgrades.
  Severity: low. Likelihood: high.
  Mitigation: keep the compile-fail surface to one fixture; `rustversion` is
  already a dev-dependency of `rstest-bdd` if gating becomes necessary.

- Risk: a beta adopter reads a green-to-red suite as "the release broke my
  tests" rather than "these tests were never passing".
  Severity: medium. Likelihood: high.
  Mitigation: CHANGELOG and migration-guide entries must *lead* with
  "scenarios that previously passed may now correctly fail", and the
  `## Common errors and fixes` entry must be findable by the panic text.
  Landing in v0.6.0 (`DEC-009`) bounds the affected population to beta
  adopters rather than everyone upgrading off a released 0.6.x.

- Risk: a future maintainer "simplifies" the autoref pattern into breakage.
  Severity: medium. Likelihood: medium.
  Mitigation: the invariant list in the module `//!` docs (EP-M3) plus the
  type-identity guard test (EP-M2).

## Progress

- [-] EP-M0 Prototype and pin the emission shape (go/no-go): blocked.
- [ ] EP-M1 Preparatory refactors: bring `macros/mod.rs` and
      `tests/step_return.rs` under the 400-line cap. No behaviour change.
- [ ] EP-M2 Red: behavioural feature files, the tag-identity guard test, and
      the compile-fail fixture, all failing for the expected reason.
- [ ] EP-M3 Green: the runtime dispatch module and the macro emission change.
- [ ] EP-M4 Lint-expectation reconciliation, `ui_lints` fixture, and the
      seeded-fault negative control.
- [ ] EP-M5 Documentation: ADR-019, the ADR-002 amendment, design doc and
      diagram, users' guide, developers' guide, known issues, migration guide,
      CHANGELOG, contents index, ergonomics doc, and the roadmap.

## Surprises & discoveries

Recorded during planning; keep appending during implementation.

- Observation: the autoref technique works on stable for every case that
  matters, including nested aliases and aliases with defaulted type
  parameters.
  Evidence: falsification experiment in `~/scratch-11-3-1`, run on
  `rustc 1.85.0` and `rustc 1.98.0`; 18 probe cases plus 4 extras, identical
  results on both toolchains.
  Impact: the mechanism risk is retired; the residual risk is entirely in
  emission shape and lint interaction.

- Observation: the change alters the `Ok` payload *shape*, not only `Err`
  propagation. Previously an alias step's payload was the whole `Result<T, E>`,
  which `insert_value` dropped as `NoMatch` without logging
  (`crates/rstest-bdd/src/context/mod.rs:317-319`). Now the payload is `T`,
  which can match a fixture and override it.
  Evidence: Doggylump pre-mortem, scenario 3.
  Impact: a second behaviour change requiring its own regression and its own
  migration-guide entry.

- Observation: `-> !` steps compile today but fail under naive dispatch with
  `error[E0282]: type annotations needed`.
  Evidence: verified on `rustc 1.98.0`.
  Impact: the never type needs a syntactic bypass alongside `()`.

- Observation: named `use ... as _;` trait imports emit `unused_imports` on
  every step (whichever trait loses the autoref race); a glob import does not.
  Evidence: verified. This is why `anyhow` uses a glob.
  Impact: the import form is a real decision, not a detail. See EP-M0.

- Observation: the roadmap's design-doc citation is wrong. Roadmap 11.3.1 cites
  `docs/rstest-bdd-design.md` §2.1; §2.1 is "Procedural macro API design" and
  contains no return-classification prose. The material is in §3.8, lines
  2877-2944.
  Impact: correct the citation as part of EP-M5.

- Observation: `crates/rstest-bdd-macros/src/macros/mod.rs` is 399 lines
  against a hard 400-line cap, and is not allowlisted.
  Impact: any edit forces a mechanical module split first (EP-M1).

- Observation: the crate-owned `StepReturnProbe` prevents an inherent method
  on the returned value from hijacking dispatch, and the orphan rule prevents
  downstream implementation of the runtime trait for the probe. However, a
  downstream blanket trait with the same method name silently wins method
  resolution for a non-`Result` value; it does not produce the planned loud
  `E0034` ambiguity.
  Evidence: on 2026-08-29, `~/scratch-11-3-1/examples/probe_ambiguity.rs`
  compiled far enough to report `E0599: no method named name found for type
  u8`, meaning the caller's `UserTrait::__rstest_bdd_step_return_kind` was
  selected. The expected dispatch tag method was not selected or ambiguous.
  The corresponding logs are
  `/tmp/ep-m0-probe-blanket-collision-rstest-bdd.out` and
  `/tmp/ep-m0-probe-hijack-rstest-bdd.out`.
  Impact: DEC-002's safety claim and EP-M0 acceptance criterion 2 are false.
  The mechanism tolerance is reached, so implementation must not proceed
  without an approved design change.

## Decision log

- DEC-001: Classify step return types by *type*, using autoref specialization,
  rather than syntactically in the macro.
  Rationale: alias expansion happens before trait selection, so the compiler
  answers the question the macro cannot. It fixes `anyhow::Result` and
  `io::Result` for free and imposes zero annotation burden on the 90.8% of
  steps that return `()` or the value steps that are already correct.
  Alternatives are exhausted: on stable there are exactly three families —
  method-resolution ordering, nominal opt-in, and runtime `TypeId` inspection.
  Runtime inspection is infeasible (a `TypeId` is an opaque hash of a
  monomorphized type; you cannot ask whether one *is a* `Result` without
  already naming `T` and `E`, and even on detection you cannot recover the
  `Err`). Nominal opt-in is what `axum::IntoResponse` and libtest's
  `Termination` do, and it is strictly worse here because it fixes a false
  green by breaking every correct value-returning step — a headline rstest-bdd
  feature that axum and libtest simply do not offer.
  Date/Author: 2026-08-29, planning agent, pending approval.

- DEC-002: Dispatch on a crate-owned probe newtype, not on the bare return
  value.
  Rationale: dispatching on `&V` directly leaves two holes. A user type with an
  inherent method of the dispatch name hijacks classification silently
  (verified). A downstream `impl StepReturnResultKind for MyType {}` is legal
  (foreign trait, local type) and permanently reclassifies that type. Wrapping
  the value in a crate-owned `StepReturnProbe<'_, V>` closes both: nobody can
  add an inherent method to a foreign type, and
  `impl ForeignTrait for StepReturnProbe<'_, LocalType>` violates the orphan
  rule because `StepReturnProbe` is not `#[fundamental]`. This is stronger
  than a sealed supertrait and cheaper.
  Date/Author: 2026-08-29, planning agent, pending approval.

- DEC-003: Keep syntactic fast paths for `()` and `!` only.
  Rationale: `()` is provable from the syntax, costs nothing, and preserves the
  existing `clippy::unnecessary_wraps` expectation for the 90.8% majority. `!`
  must be bypassed because dispatch on a diverging expression yields
  `error[E0282]`. Every other return type goes through dispatch; adding a
  syntactic fast path for spelled `Result` would double the codegen paths and
  the test matrix to save ~25 dispatch sites in this workspace.
  Date/Author: 2026-08-29, planning agent, pending approval.

- DEC-004: Keep both return-kind hints, with re-scoped meanings. `value`
  becomes load-bearing (it is now the *only* way to store a `Result` as a
  payload) and ships to 1.0. `result` is retained as an *assertion* rather than
  an instruction: it preserves the good-span compile error already pinned by
  `crates/rstest-bdd/tests/ui_macros/return_override_result_requires_result.stderr`.
  Rationale: deprecating `result` costs churn for no correctness gain, and a
  redundant-but-harmless assertion is the cheaper contract.
  Date/Author: 2026-08-29, planning agent, pending approval.

- DEC-005: Bound the error type on a crate-owned marker trait carrying
  `#[diagnostic::on_unimplemented]`, blanket-implemented for `E: Display`, and
  annotated `#[diagnostic::do_not_recommend]` so rustc does not suggest
  implementing the hidden trait.
  Rationale: this is what turns an opaque `E0277` pointing into macro-expanded
  tokens into the actionable migration diagnostic roadmap 11.3.1 asks for. Both
  attributes are stable at or below MSRV 1.85 (`on_unimplemented` 1.78,
  `do_not_recommend` 1.85).
  Date/Author: 2026-08-29, planning agent, pending approval.

- DEC-006: The diagnostic note must recommend implementing `Display` **only**.
  It must not offer the `value` hint as a remedy.
  Rationale: `value` converts a loud compile error into a permanent silent
  `Err`-swallow — the exact defect being fixed. `value` is documented
  separately as a payload-storage tool.
  Date/Author: 2026-08-29, planning agent, pending approval.

- DEC-007: Reject, at compile time, two shapes that would otherwise be new
  false greens: a return type of `Result<Result<..>, ..>` (the inner `Err`
  would be boxed as an opaque payload) and a return type of `impl Trait` (the
  hidden type is opaque at the dispatch site, so an RPIT hiding a `Result`
  reproduces the original bug). Both are detectable syntactically and both
  accept an explicit hint as the escape hatch.
  Rationale: the roadmap item exists to eliminate a class of silent false
  green; shipping two narrower instances of the same class would be
  self-defeating.
  Date/Author: 2026-08-29, planning agent, pending approval.

- DEC-008: Record the macro/runtime version-skew constraint in
  `docs/releasing-crates.md` and the migration guide. Do **not** add a
  versioned ABI marker in this change.
  Rationale: the marker is a real improvement but it is a separate contract
  decision with its own naming and stability questions, and bundling it would
  widen an already-broad change. Revisit if skew reports appear.
  Date/Author: 2026-08-29, planning agent, pending approval.

- DEC-009: **Target v0.6.0.** Land this before the v0.6.0 release rather than
  shipping v0.6.0 with the false green and breaking adopters again at v0.7.0.
  Rationale: the workspace is at `0.6.0-beta4`, so v0.6.0 is unreleased and
  this is not a breaking change to any published API — it is a defect fix
  applied before the release that would otherwise enshrine the defect. The
  gauss adopter is already on beta3 and hit this in practice; deferring would
  ask them to change step signatures twice for one defect. It also removes the
  semver conflict entirely: roadmap §11 states the v0.6.1 line "should stay
  semver-compatible", and all three behaviour changes here break that promise,
  so landing in v0.6.0 means no promise needs amending.
  Consequences for EP-M5: the migration content goes into
  `docs/v0-6-0-migration-guide.md` under its existing `## Breaking changes`
  section, framed as a beta-to-final change, **not** under the
  `(v0.7.0)`-suffixed subsection pattern the repository uses for post-0.6 work
  (precedent at that guide's lines 466-468 and 567-569). The CHANGELOG entry
  goes under `## Unreleased` for v0.6.0. The roadmap needs an explicit note
  that 11.3.1 landed in v0.6.0, because item 11.3 sits physically under the
  `## 12. Pre-1.0.0 API consolidation: landed v0.7.0` heading despite its
  `11.` numbering, which would otherwise imply the wrong release line.
  Date/Author: 2026-08-29, maintainer decision.

- DEC-010: Write a new ADR-019 and *amend* ADR-002 rather than superseding it.
  Rationale: ADR-002's load-bearing conclusion — reject nightly
  `auto_traits`/`negative_impls`, stay on stable — remains true and has live
  dependants (`docs/adr-006-fallible-scenario-functions.md:16-18`,
  `docs/contents.md:66-67`). Only its *mechanism* bullet dies. Follow the dated
  in-Status banner precedent set by `docs/adr-005-async-step-functions.md:5-9`.
  Date/Author: 2026-08-29, planning agent, pending approval.

- DEC-011: No `proptest`, `kani`, or `verus` for this change. See
  `Verification plan` for the justification; it is recorded there rather than
  omitted, per the ExecPlan rules.
  Date/Author: 2026-08-29, planning agent, pending approval.

- DEC-012: **Proposed deviation — resolve the caller-trait collision before
  continuing.** EP-M0 falsified the assertion that a crate-owned probe makes a
  caller blanket trait with the dispatch method name loud. The current
  method-call form remains susceptible to an in-scope blanket trait, even
  though it closes the inherent-method and downstream-implementation holes.
  The approved design needs either a stable dispatch form that cannot be
  captured by caller method scope, or an explicit decision that the extremely
  unlikely collision is an accepted residual risk with revised tests,
  documentation, and migration impact. Moving imports into a runtime
  `macro_rules!` macro is not yet accepted as a remedy: it may change hygiene,
  but this must be proved rather than assumed. Affects DEC-002, EP-M0,
  INV-1, LEMMA-1, the module invariants, and ADR-019. Status: pending
  maintainer decision.

## Outcomes & retrospective

To be completed at EP-M5. Before setting this plan to `COMPLETE`, reconcile
every implementation discovery against the artefacts named in
`Conformance basis`: update ADR-002's amendment and ADR-019 if the mechanism
changed, update `docs/rstest-bdd-design.md` §3.8 if the flow changed, and
record any purely mechanical difference here rather than leaving it
unexplained.

## Context and orientation

You have only this repository and this file. Here is what you need to know.

`rstest-bdd` is a behaviour-driven-development test framework for Rust. A
`.feature` file written in Gherkin lists steps; Rust functions annotated
`#[given]`, `#[when]`, and `#[then]` implement them; a `#[scenario]` function
binds a feature scenario to a test.

The workspace lives under `crates/`:

- `crates/rstest-bdd` — the runtime library. Owns `StepContext`, the step
  registry, the execution loop, and the `#[doc(hidden)]` helpers that
  macro-generated code calls into.
- `crates/rstest-bdd-macros` — the procedural macros. Note that it does **not**
  depend on `rstest-bdd`; it emits `::rstest_bdd::…` paths as bare tokens
  (`crates/rstest-bdd-macros/src/codegen/mod.rs:3-4`).
- `crates/rstest-bdd-harness`, `-tokio`, `-gpui` — adapters that wrap the
  *scenario body*. They contain zero references to `StepFn`, `StepExecution`,
  or any `__rstest_bdd_*` symbol, and are unaffected by this change.
- `crates/rstest-bdd-server`, `crates/cargo-bdd` — a language server and a CLI.
  Neither classifies step return types; both are unaffected. (Verified.)

### How a step return value is handled today

1. `crates/rstest-bdd-macros/src/return_classifier/mod.rs` classifies the
   declared return type into `ReturnKind::{Unit, Value, ResultUnit,
   ResultValue}`. The decision is purely syntactic. `classify_result_like`
   matches only `Type::Path` whose last segment is `Result` (bare, or under
   `std::result` / `core::result`) or `StepResult` (bare, or under
   `rstest_bdd` / `crate` / `self` / `super`).
2. Line 82 is the defect:

   ```rust
   None => Ok(classify_result_like(ty).unwrap_or(ReturnKind::Value)),
   ```

   Anything unrecognized — a local alias, `anyhow::Result`, `io::Result`, any
   two-segment path — becomes `Value`.
3. `crates/rstest-bdd-macros/src/codegen/wrapper/emit/call_expr.rs` turns the
   `ReturnKind` into the call expression. The `Value` arm is
   `Ok(#path::__rstest_bdd_payload_from_value(#call))`, which boxes the entire
   return value — `Err` included — as an opaque payload.
4. `__rstest_bdd_payload_from_value<T: Any>`
   (`crates/rstest-bdd/src/lib.rs:162-170`) returns `None` when
   `TypeId::of::<T>() == TypeId::of::<()>()`, otherwise `Some(Box::new(value))`.
   Note it already resolves *unit* aliases correctly by `TypeId`.
5. The call expression is spliced into
   `catch_unwind(AssertUnwindSafe(|| { #call_expr }))` for sync steps
   (`crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly/mod.rs`,
   `generate_sync_unwind_handling`) or into `async move { #call_expr }` for
   async steps
   (`crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly/async_wrapper.rs`).
6. The scenario loop
   (`crates/rstest-bdd-macros/src/codegen/scenario/runtime/generators/step_loop.rs:61-86`)
   receives `Ok(Some(payload))` and calls `let _ = ctx.insert_value(payload)`.
   `insert_value` returns `InsertOutcome::{Inserted, NoMatch, AmbiguousIgnored}`
   (ADR-015). `NoMatch` is dropped silently and, unlike `AmbiguousIgnored`, is
   not logged (`crates/rstest-bdd/src/context/mod.rs:317-319`).

So an alias-returning step that fails does this: the `Err` is boxed, no fixture
has type `Result<T, E>`, `insert_value` returns `NoMatch`, the payload is
dropped, and the step reports success.

### Why the macro cannot fix this syntactically

A procedural macro runs before name resolution and type checking. It sees
tokens. `MyResult<()>` and `Score` are indistinguishable to it: both are a path
with an identifier. Requiring a hint whenever the macro cannot prove the kind
would mean requiring a hint on *every* named return type, which is a large
migration for no benefit in the common case.

Trait resolution, by contrast, happens after alias expansion. If the generated
code asks the *compiler* which arm applies, aliases resolve for free. The
obstacle ADR-002 identified is real — you cannot write "for all `T` except
`Result<_, _>`" as a blanket impl without an overlap conflict — but autoref
specialization sidesteps it. Two *disjoint* traits never overlap; the choice
between them is made by the method probe's ordering, not by coherence.

## Conformance basis

Upstream artefacts and their revisions at the time of writing.

- `docs/roadmap.md` item **11.3.1** (unchecked), located at lines 1057-1075.
  Its design-doc citation (§2.1) is wrong; the material is at §3.8.
- `docs/adr-002-stable-step-return-classification.md`, Status `Accepted`,
  dated 2025-12-19. To be amended, not superseded (`DEC-010`).
- `docs/adr-006-fallible-scenario-functions.md` — depends on ADR-002 for the
  shared `ReturnKind`; needs a one-line note.
- `docs/adr-015-insert-outcome-for-step-return-overrides.md` — defines
  `InsertOutcome`. Unchanged by this work, but it is the immediate downstream
  consumer of a classified payload.
- `docs/rstest-bdd-design.md` §3.8, lines 2828-2944, including the Mermaid
  sequence diagram at 2906-2934 and its caption at 2936-2937.
- `docs/users-guide.md` §"Step return values", lines 389-429. The heading text
  must not change: `docs/adr-006-fallible-scenario-functions.md:17` deep-links
  `users-guide.md#step-return-values`.
- `docs/ergonomics-and-developer-experience.md` §3.1 bullet 3, lines 236-243.
- `AGENTS.md` — the repository's binding conventions.
- `docs/testing-strategy.md` — the structural-versus-semantic test split.
- `docs/documentation-style-guide.md` — ADR format and required sections.
- Issue `leynos/rstest-bdd#573` (closed as documented; the code defect remains).

Trace links:

```plaintext
issue#573 -> ROADMAP-11.3.1 -> ADR-019 -> EP-M3 -> tests::step_return::scenario_alias_no_hint_failure
ROADMAP-11.3.1 -> ADR-002-amendment -> EP-M5 -> docs/users-guide.md#step-return-values
ADR-002-L27 (stable overlap constraint) -> ADR-019-rationale -> EP-M3 -> crates/rstest-bdd/src/step_return.rs
gauss-beta3 -> DESIGN-3.8 -> EP-M5 -> docs/rstest-bdd-design.md sequence diagram
ADR-015 (InsertOutcome) -> EP-M2 -> tests::step_return::scenario_alias_ok_overrides_fixture
```

## Verification plan

### Axioms

These are assumptions about third-party behaviour. They are not verified here;
they are relied upon, and each is exercised at a contract-level boundary.

- **AXIOM-1**: rustc's method probe, for a receiver expression of type `U`,
  builds candidate types by repeatedly dereferencing `U`, and for each candidate
  type tries the by-value receiver before the autoref (`&`) receiver. This is the
  ordering the whole mechanism rests on. Documented in dtolnay's
  autoref-specialization case study and relied upon in production by `anyhow`.
- **AXIOM-2**: type aliases are expanded before trait selection, so
  `MyResult<()>` and `Result<(), MyError>` select the same impl.
- **AXIOM-3**: NLL ends the borrow created for the probe before the subsequent
  move of the same value, because the tag returned is a zero-sized type with no
  lifetime parameter.
- **AXIOM-4**: the orphan rule forbids
  `impl ForeignTrait for ForeignType<Local>` when `ForeignType` is not
  `#[fundamental]`.
- **AXIOM-5**: `#[diagnostic::on_unimplemented]` (stable 1.78) and
  `#[diagnostic::do_not_recommend]` (stable 1.85) behave as documented at MSRV.
- **AXIOM-6**: clippy suppresses most style lints for code originating in an
  external macro expansion. This one is *load-bearing and least certain*, which
  is precisely why EP-M0 verifies it empirically rather than assuming it.

AXIOM-1 through AXIOM-4 were exercised directly during planning against
`rustc 1.85.0` and `rustc 1.98.0` in a scratch crate; the 18-case probe matrix
is reproduced in `Artefacts and notes`. AXIOM-6 is re-verified at EP-M0 inside
this workspace, since the workspace's own lint configuration differs from the
scratch crate's.

### Obligations

**INV-1 — Classification soundness.** For every concrete step return type `V`,
the generated wrapper selects the `Result` arm if and only if `V` expands to
`core::result::Result<T, E>`, and selects the value arm otherwise.

- Method: exhaustive parameterized test over an enumerated partition of type
  shapes, plus behavioural scenarios for the shapes that carry runtime meaning.
- Rationale: the domain is a partition over *types*, which Rust cannot quantify
  over at runtime, so a generated-input property test is not expressible. The
  partition is small, closed, and enumerable: `()`, `!`, a plain value, a value
  alias, a spelled `Result`, `StepResult`, a local alias, a nested alias, an
  alias with a defaulted parameter, a two-segment path alias, a reference to a
  `Result`, a `Box<Result<..>>`, an `Option<Result<..>>`, a newtype wrapping a
  `Result`, and a `Deref<Target = Result<..>>`. An enumerated partition with a
  witness per class is *stronger* evidence here than sampled generation.
- Domain: the fifteen classes above.
- Artefact: `crates/rstest-bdd/tests/step_return_dispatch.rs` — a runtime-only
  table test using `rstest` `#[case]` parameterization, asserting the selected
  tag by `std::any::type_name_of_val` (stable 1.76), with `pretty_assertions`.
- Evidence: the test fails to compile before `crates/rstest-bdd/src/step_return.rs`
  exists; after EP-M3 it passes with every class exercised.
- Non-vacuity: each of the fifteen classes is a distinct witness, and the test
  asserts a *specific* tag name per class rather than a boolean. Merging the
  two traits into one, renaming one trait's method, or implementing
  `StepReturnValueKind` for `T` instead of `&T` each break at least one row.
  The negative control below seeds exactly such a fault.

**INV-2 — No `Err` becomes an opaque payload.** For any step whose return type
expands to `Result<T, E>`, and which carries no `value` hint, an `Err` fails
the scenario and never reaches `__rstest_bdd_payload_from_value`.

- Method: behavioural (BDD) scenarios through the real runtime path.
- Rationale: `docs/testing-strategy.md` is explicit that semantic behaviour
  tests, not token inspection, are the backstop for this contract.
- Artefact: feature files and step definitions listed under EP-M2.
- Evidence: `scenario_alias_no_hint_failure` fails before EP-M3 (the scenario
  passes when it should panic) and passes after (the scenario panics with
  `alias failure`).
- Non-vacuity: the assertion is `#[should_panic(expected = "alias failure")]`.
  If dispatch degrades to the value arm, no panic occurs and the test fails.
  This is the property the seeded-fault control exploits.

**INV-3 — Payload shape.** For a `Result`-classified step, the payload offered
to fixtures is the `Ok` value `T`, not `Result<T, E>`; for a value-classified
step it is `V`; for `()` and for any alias of `()` it is `None`.

- Method: behavioural scenario plus a unit assertion on
  `__rstest_bdd_payload_from_value`.
- Rationale: this is the behaviour change nobody expects (see
  `Surprises & discoveries`) and it mutates scenario state.
- Artefact: `step_return_alias_ok_overrides_fixture.feature`; plus a test
  pinning that the `Unit` fast path and the dispatch path agree for `-> ()`.
- Evidence: a `#[then]` asserts the fixture holds the unwrapped value.
- Non-vacuity: before EP-M3 the payload is a `Result`, no fixture matches, and
  the `#[then]` sees the original fixture value — so the test fails for the
  right reason.

**INV-4 — Generated code is lint-clean downstream.** A crate that builds under
`#![deny(warnings)]` and `cargo clippy -- -D warnings`, and which defines steps
of every return kind, compiles with zero diagnostics.

- Method: compile-pass fixture plus a dedicated `ui_lints` binary.
- Rationale: an emitted `#[expect(...)]` that goes unfulfilled is a hard error
  in the adopter's crate which they cannot silence. This is the highest-blast-
  radius failure mode identified.
- Artefact: `crates/rstest-bdd/tests/fixtures_macros/step_return_dispatch_lint_clean.rs`
  (registered in `run_passing_macro_tests`) and a new bin under
  `crates/rstest-bdd/tests/ui_lints/src/bin/`.
- Evidence: `make lint` and `make test` pass; the `ui_lints` case exits zero.
- Non-vacuity: the fixture must contain at least one step of each kind
  (`()`, value, spelled `Result`, alias, `StepResult`, `anyhow::Result`,
  `io::Result`). Removing the lint-list rework of EP-M4 must make it fail with
  `unfulfilled_lint_expectations`; confirm that it does before applying the fix.

**INV-5 — Error-message parity.** The message produced by the dispatch path for
an `Err` is byte-identical to the message produced by the existing
`result`-hint path, namely `error.to_string()`.

- Method: parameterized unit test comparing both paths for the same error
  value.
- Rationale: `crates/rstest-bdd/tests/execution_error.rs` and the panic-message
  assertions in the behavioural suite depend on this text.
- Artefact: the tag-dispatch test file.
- Non-vacuity: use an error type whose `Display` differs from its `Debug`, so a
  `{:?}`-for-`{}` slip is caught.

**LEMMA-1 — The dispatch is total.** Every step return type selects exactly one
tag; no return type selects both (ambiguity) or neither (no-method).

- Method: the compile-pass fixture (a no-method failure is a compile error) plus
  the INV-1 table (an ambiguity is `E0034`, also a compile error).
- Rationale: both failure directions are compile-time and therefore fully
  decided by the fixture set; no runtime reasoning is required.
- Residual gap: totality is established only over the enumerated partition, not
  over all types. Stated explicitly rather than claimed away.

### Negative control (seeded fault)

Run once before merge and record the transcript in `Artefacts and notes`.

In `crates/rstest-bdd-macros/src/codegen/wrapper/emit/call_expr.rs`, change the
emitted prelude so that only the value-kind trait is in scope. The workspace
must still compile cleanly with zero warnings, and exactly these tests must
fail:

- `step_return::scenario_alias_no_hint_failure`
- `step_return::scenario_anyhow_failure`
- `step_return::scenario_alias_async_failure`
- the `insta` snapshot of the emitted dispatch token stream

If the workspace compiles and every test still passes, the suite does not
detect classification degradation and the plan's mitigation for the
missing-prelude risk is unproven. Revert the seeded fault afterwards.

A second, cheaper control comes free: the repository's existing cargo-mutants
workflow (`tests/workflow_contracts/mutation_testing_test.py`) covers
`crates/`, so `crates/rstest-bdd/src/step_return.rs` is in scope. Run
`cargo mutants -f crates/rstest-bdd/src/step_return.rs` once and confirm zero
survivors. Note the limitation honestly: cargo-mutants treats a `quote!` body
as an opaque token literal and will **not** delete an import from it, so it
cannot reach the highest-risk element. That is why the hand-seeded control
above is required and why the `insta` snapshot exists.

### Methods deliberately not used

- **`proptest`**: the invariant quantifies over *types*, not values. Rust
  cannot generate types at runtime, so a property test would degenerate into
  the same enumerated table with worse failure messages. The table test is
  strictly stronger evidence.
- **`kani`**: there is no `unsafe` code, no arithmetic, and no bounded state
  machine. A bounded model check would have nothing to explore.
- **`verus`**: no lemma is introduced. The one non-trivial proof obligation
  (AXIOM-1, the method-probe ordering) is a property of the *compiler*, not of
  repository-owned logic, and is treated as an axiom exercised at a
  contract-level boundary — which is exactly what the ExecPlan rules require
  for third-party interfaces.

This is recorded as a conclusion with its rationale, not omitted.

## Interfaces and dependencies

No new external dependencies. Everything below lives in existing crates.

### New: `crates/rstest-bdd/src/step_return.rs`

A `#[doc(hidden)]` module in the runtime crate, re-exported from
`crates/rstest-bdd/src/lib.rs`. It is the *only* permitted call-site owner:
generated step wrappers in `rstest-bdd-macros` are its sole consumers. Although
`#[doc(hidden)]`, changing or removing it is a breaking change for existing
macro expansions — state this in the module docs, mirroring the wording already
used for the Tokio bridge in `docs/developers-guide.md`.

At the end of EP-M3 these items must exist:

```rust
/// Borrowing probe used to select a step return classification.
pub struct StepReturnProbe<'a, T: ?Sized>(pub &'a T);

/// Tag selected when the probed type is a `Result`.
pub struct StepReturnResultTag;

/// Tag selected when the probed type is not a `Result`.
pub struct StepReturnValueTag;

/// Higher-priority arm: matches without an autoref step.
pub trait StepReturnResultKind {
    fn __rstest_bdd_step_return_kind(&self) -> StepReturnResultTag { StepReturnResultTag }
}
impl<T, E> StepReturnResultKind for StepReturnProbe<'_, ::core::result::Result<T, E>> {}

/// Lower-priority arm: requires one autoref step, so it loses to the above.
pub trait StepReturnValueKind {
    fn __rstest_bdd_step_return_kind(&self) -> StepReturnValueTag { StepReturnValueTag }
}
impl<T: ?Sized> StepReturnValueKind for &StepReturnProbe<'_, T> {}

/// Marker carrying the migration diagnostic for a step's error type.
#[diagnostic::on_unimplemented(
    message = "a step's error type `{Self}` must implement `std::fmt::Display`",
    label = "this step returns `Result<_, {Self}>`",
    note = "rstest-bdd renders a step's `Err` through `Display` to build the scenario \
            failure message; implement `std::fmt::Display` for `{Self}`"
)]
pub trait StepErrorDisplay: ::core::fmt::Display {}

#[diagnostic::do_not_recommend]
impl<E: ::core::fmt::Display + ?Sized> StepErrorDisplay for E {}

impl StepReturnResultTag {
    pub fn normalize<T: ::core::any::Any, E: StepErrorDisplay>(
        self,
        value: ::core::result::Result<T, E>,
    ) -> ::core::result::Result<Option<Box<dyn ::core::any::Any>>, String>;
}

impl StepReturnValueTag {
    pub fn normalize<T: ::core::any::Any>(
        self,
        value: T,
    ) -> ::core::result::Result<Option<Box<dyn ::core::any::Any>>, String>;
}
```

Both `normalize` implementations **must** delegate boxing to the existing
`crate::__rstest_bdd_payload_from_value`, not reimplement it. Two copies of the
"unit becomes `None`" rule will drift. `AGENTS.md` requires sweeping for an
existing equivalent helper before adding an abstraction; this is that helper.

`StepReturnResultTag::normalize` must produce the error string as
`value.to_string()` so it is byte-identical to the existing
`call_expr.rs` `Err(error.to_string())` (INV-5).

### Changed: `crates/rstest-bdd-macros/src/return_classifier/mod.rs`

The step pipeline gains its own strategy enum, distinct from the `ReturnKind`
that the scenario pipeline keeps using:

```rust
pub(crate) enum StepReturnStrategy {
    /// `-> ()` or no return type: emit `Ok(None)` directly.
    Unit,
    /// `-> !`: emit the call and an unreachable tail.
    Never,
    /// Ask the compiler via autoref dispatch.
    Dispatch,
    /// `value` hint: box the whole return value as a payload.
    ForcedValue,
    /// `result` hint: assert `Result` shape and take the fallible arm.
    ForcedResult,
}
```

Keep `is_result_like_path`, `first_type_argument`, `second_type_argument`, and
the `is_definitely_non_result_type` family. They are **not** dead: the scenario
classifier, the `scenarios!` generator, and `crates/rstest-bdd-macros/src/utils/result_type.rs`
(`Result`-typed fixture parameters) all still depend on them. Only the
`unwrap_or(ReturnKind::Value)` fallback for steps goes away.

Add the two `DEC-007` rejections here: a return type of `Result<Result<..>, ..>`
and a return type of `Type::ImplTrait` are compile errors without an explicit
hint.

### Changed: `crates/rstest-bdd-macros/src/codegen/wrapper/emit/call_expr.rs`

`generate_call_expression` takes `StepReturnStrategy` instead of `ReturnKind`
and gains a single private function that emits the entire dispatch block —
prelude import and call together. **No second construction site may exist.**
That single function is what the `insta` snapshot pins.

### Changed: `crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly/lint_config.rs`

`WrapperLintConfig.return_kind` becomes `strategy: StepReturnStrategy`, and
`wrapper_expect_lint_names` emits `clippy::unnecessary_wraps` only for the
strategies whose generated body provably always returns `Ok`: `Unit` and
`ForcedValue`. It must **not** be emitted for `Dispatch`, `ForcedResult`, or
`Never`.

## Plan of work

Five stages, each ending in validation. Do not start a stage while the previous
stage's validation is failing.

### Stage A — prototype and pin the emission shape (EP-M0)

No repository code changes. Work in a scratch crate outside the repository and
outside `/tmp` (use `~/scratch-11-3-1`, which already exists from planning).

Re-run the probe matrix with the **`StepReturnProbe` newtype** variant, because
planning verified the *bare* `&V` variant, not the newtype one. Confirm each of
the fifteen INV-1 classes selects the expected tag, and additionally confirm:

1. The borrow ends before the move (no `E0505`) for a non-`Copy` return value.
2. A user blanket trait with a colliding method name now produces a loud
   `E0034` ambiguity rather than a silent hijack.
3. `impl StepReturnResultKind for StepReturnProbe<'_, MyLocalType> {}` is
   rejected by the orphan rule.
4. `-> !` still needs the syntactic bypass.
5. `async fn` steps dispatch correctly after `.await`.

Then settle the import form. Two candidates are known to work; pick by
measurement, not preference:

- **Glob** — `use ::rstest_bdd::step_return::*;`. Verified to produce no
  `unused_imports`. Risk: `clippy::wildcard_imports` is pedantic-warn and the
  workspace denies warnings.
- **Named anonymous** — an `#[allow(unused_imports)]`-annotated
  `use ::rstest_bdd::step_return::{StepReturnResultKind as _,
  StepReturnValueKind as _};`. Verified clean when emitted by an *external*
  macro. Risk: `clippy::allow_attributes` is denied in this workspace.

The deciding experiment must run **inside this workspace**, not the scratch
crate, because AXIOM-6 (clippy's external-macro suppression) is the uncertain
axiom and the workspace's lint configuration differs. Build a throwaway step
using each shape and run
`cargo clippy --workspace --all-targets --all-features -- -D warnings`.

Fallback ladder if both shapes fail: move the dispatch into a
`#[doc(hidden)] #[macro_export] macro_rules!` in `rstest-bdd`, so the import
lives inside a `macro_rules!` body owned by the runtime crate and the proc
macro emits a single invocation. This is exactly what `anyhow!` does.

**Go/no-go.** If no shape is simultaneously correct and lint-clean, stop and
escalate (see `Tolerances`). Record the chosen shape verbatim in
`Artefacts and notes` before proceeding; every later stage depends on it.

### Stage B — preparatory refactors (EP-M1)

Mechanical only. No behaviour change, no new tests, gates green at the end.

1. Split `crates/rstest-bdd-macros/src/macros/mod.rs` (399 of 400 lines). The
   attribute-parsing helpers (`StepAttrArgs`, its `Parse` impl,
   `parse_return_override`, `try_parse_expr_syntax`) are the natural extraction
   into `crates/rstest-bdd-macros/src/macros/step_attr_args.rs`.
2. Split `crates/rstest-bdd/tests/step_return.rs` (229 lines). It will exceed
   400 once the new scenarios land. Extract the shared fixtures and newtypes
   into `crates/rstest-bdd/tests/common/step_return_support.rs`, keeping the
   existing scenarios in place.

Do not add to `scripts/rs-length-allowlist.txt`.

### Stage C — red tests (EP-M2)

Write the tests before the implementation and observe each failing for the
expected reason. Where a test cannot fail because the item under test does not
exist yet, that compile failure *is* the red state — record the exact error.

Feature files under `crates/rstest-bdd/tests/features/`, each 3-5 lines,
following the existing `step_return_*.feature` shape:

| Feature file | Step return | Expected |
| --- | --- | --- |
| `step_return_alias_no_hint_failure.feature` | `-> MyResult<Number>` returning `Err` | scenario panics with `alias failure` |
| `step_return_anyhow_failure.feature` | `-> anyhow::Result<()>` returning `Err` | scenario panics |
| `step_return_io_result_failure.feature` | `-> std::io::Result<()>` returning `Err` | scenario panics |
| `step_return_alias_ok_overrides_fixture.feature` | `-> MyResult<Number>` returning `Ok(Number(2))` | fixture is 2 |
| `step_return_alias_async_failure.feature` | `async fn -> MyResult<()>` returning `Err` | scenario panics |
| `step_return_genuine_value_alias.feature` | `type Score = u32; -> Score` | passes, fixture overridden |
| `step_return_boxed_result.feature` | `-> Box<Result<..>>` returning `Err` | scenario **passes** |

The last row is deliberate: it pins `Box<Result<..>>` as a documented value
classification, so a future "improvement" that starts dereferencing becomes a
red test rather than a silent semantic change.

Also in Stage C:

- `crates/rstest-bdd/tests/step_return_dispatch.rs` — the INV-1 tag-identity
  table and the INV-5 message-parity check. Use `rstest` `#[case]`
  parameterization and `pretty_assertions`. Add a
  `// Guards invariant N of step_return.rs` comment beside each row that guards
  a named module invariant.
- `crates/rstest-bdd/tests/ui_macros/step_return_alias_error_not_display.rs`
  plus a checked-in `.stderr`, registered in `run_failing_ui_tests`
  (`crates/rstest-bdd/tests/trybuild_macros.rs:143`). Put both `E: !Display`
  variants in the **one** fixture — trybuild emits all diagnostics from a
  fixture into one `.stderr`, so one file is one compilation instead of two,
  and the trybuild binary is already on the serialized critical path.
- `crates/rstest-bdd/tests/ui_macros/step_return_nested_result.rs` and
  `step_return_impl_trait.rs` with `.stderr`, for the `DEC-007` rejections.
- `crates/rstest-bdd/tests/fixtures_macros/step_return_dispatch_lint_clean.rs`
  with `#![deny(warnings)]`, registered in `run_passing_macro_tests`
  (`trybuild_macros.rs:85`), covering one step of every kind.

Validation for this stage: `make test` fails, and the failures are the ones
listed above and no others.

### Stage D — implementation (EP-M3)

1. Add `crates/rstest-bdd/src/step_return.rs` with the interface above and the
   invariant list in its `//!` header (see `Artefacts and notes`).
2. Wire it into `crates/rstest-bdd/src/lib.rs` as a `#[doc(hidden)]` module.
3. Introduce `StepReturnStrategy` in the return classifier and route the step
   macros to it, leaving the scenario paths on `ReturnKind`.
4. Rewrite `generate_call_expression` to emit the Stage-A-pinned shape from one
   function, using `quote_spanned!` at `func.sig.output.span()` so a bound
   failure underlines the user's `-> MyResult<()>` rather than macro internals.
5. Add the `DEC-007` compile errors.

Validation: the Stage C tests go green; `make check-fmt`, `make lint`, and
`make test` all pass.

### Stage E — lint reconciliation and controls (EP-M4)

1. Rework `wrapper_expect_lint_names` per the interface section, and update
   `crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly/tests.rs`
   (which asserts the lint list at lines 169 and 184) to cover every strategy.
2. Add the `ui_lints` binary and refresh its lockfile with
   `make update-ui-lints-lock` — that target is needed *only* because the
   `ui_lints` crate is built with `cargo clippy --locked`; it is **not** needed
   for `ui_macros` fixtures.
3. Add the `insta` snapshot of the emitted dispatch token stream to
   `crates/rstest-bdd-macros/src/codegen/wrapper/emit/tests.rs`.
4. Run the seeded-fault negative control and the cargo-mutants check, and paste
   both transcripts into `Artefacts and notes`.

### Stage F — documentation (EP-M5)

See `Concrete steps` for the exact file list.

## Milestones and plateaus

Each milestone ends in a coherent, validated repository state.

**EP-M0 — emission shape decided.**
Requirements: de-risks ROADMAP-11.3.1's mechanism.
Acceptance: the probe matrix passes with the newtype variant on the MSRV
toolchain, and one import shape is verified lint-clean inside this workspace.
Conformance check: no repository files changed; no interface committed yet.
Recovery: discard the scratch crate and re-run.
Remaining gaps: everything.
Compatibility decision: none required.

**EP-M1 — file-size headroom, no behaviour change.**
Acceptance: `make check-fmt`, `make lint`, `make test` all pass; `git diff`
shows only moves and re-exports; `scripts/check_rs_file_lengths.py` reports no
violations and the allowlist is unchanged.
Conformance check: no public interface moved; no upstream assumption touched.
Recovery: `git revert` the single refactor commit.
Remaining gaps: the defect is still present.
Compatibility decision: none — the extracted items are crate-private.

**EP-M2 — red suite.**
Acceptance: the named tests fail for the stated reasons and nothing else
regresses.
Conformance check: the feature files match roadmap 11.3.1's finish-line list —
an alias returning `Err`, spelled `Result`, `StepResult`, an alias marked
`result`, and a genuine value alias — with `anyhow::Result`, `io::Result`, the
async path, and the payload-shape change added on top.
Recovery: the tests are additive; delete them to return to EP-M1.
Remaining gaps: no implementation.
Compatibility decision: none — test-only surface.

**EP-M3 — the defect is fixed.**
Requirements: discharges ROADMAP-11.3.1's core requirement and INV-1, INV-2,
INV-3, INV-5.
Acceptance: `step_return::scenario_alias_no_hint_failure` panics with
`alias failure`; the full gate set passes.
Conformance check: ADR-002's constraint "make unresolved classification
explicit without treating every named type as fallible" is satisfied — nothing
is *assumed*, the compiler decides. No non-hidden public interface changed. No
new dependency. No persisted or wire format touched.
Recovery: revert the EP-M3 commits; EP-M1 and EP-M2 remain valid.
Remaining gaps: generated-code lint expectations not yet reconciled; docs stale.
Compatibility decision: none. `rstest-bdd` is pre-1.0 and v0.6.0 is unreleased
(`DEC-009`), so no compatibility layer is warranted and none is prescribed.

**EP-M4 — generated code is lint-clean and the fix is guarded.**
Requirements: discharges INV-4 and LEMMA-1.
Acceptance: the `ui_lints` case exits zero; the seeded-fault control fails
exactly the four named artefacts; `cargo mutants -f crates/rstest-bdd/src/step_return.rs`
reports zero survivors.
Conformance check: the emitted `#[expect]` list matches the emitted body for
every strategy.
Recovery: the lint list change is one function; revert it alone.
Remaining gaps: docs stale.
Compatibility decision: none.

**EP-M5 — documentation truthful.**
Acceptance: `make markdownlint` and `make nixie` pass;
`scripts/check_users_guide_links.py` passes; no document still asserts that an
unresolved alias is classified as a value.
Conformance check: every passage listed in `Concrete steps` has been updated or
consciously retained; the roadmap entry is ticked and its design-doc citation
corrected.
Recovery: documentation-only; revert freely.
Remaining gaps: none.
Compatibility decision: none.

## Concrete steps

Run everything from the repository root:
`/home/leynos/.lody/repos/github---leynos---rstest-bdd/worktrees/7e774aa3-6d3d-4d34-8bf8-ad2f1350d2b6`.

### Gates

Run these sequentially, never in parallel — the environment uses build caching
and sequential runs benefit from it. Tee each to a log so truncated output can
be reviewed:

```bash
make check-fmt 2>&1 | tee "/tmp/check-fmt-rstest-bdd-$(git branch --show-current).out"
make lint      2>&1 | tee "/tmp/lint-rstest-bdd-$(git branch --show-current).out"
make test      2>&1 | tee "/tmp/test-rstest-bdd-$(git branch --show-current).out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-rstest-bdd-$(git branch --show-current).out"
make nixie     2>&1 | tee "/tmp/nixie-rstest-bdd-$(git branch --show-current).out"
```

Prefer delegating full gate runs to the `scrutineer` sub-agent, which runs them
sequentially, captures each log, and returns a bounded report. When it reports a
failure, read the cited log rather than re-running the gate.

Expected on success, at the tail of the test log:

```plaintext
Summary [  ...s] ... tests run: ... passed, 0 failed, ... skipped
```

### Focused commands

Run one behavioural scenario:

```bash
cargo nextest run -p rstest-bdd --test step_return scenario_alias_no_hint_failure
```

Expected before EP-M3 (the red state — the scenario passes when it must panic):

```plaintext
FAIL [ 0.0xxs] rstest-bdd::step_return step_return::scenario_alias_no_hint_failure
note: test did not panic as expected
```

Expected after EP-M3:

```plaintext
PASS [ 0.0xxs] rstest-bdd::step_return step_return::scenario_alias_no_hint_failure
```

Run the classification table:

```bash
cargo nextest run -p rstest-bdd --test step_return_dispatch
```

Regenerate a trybuild `.stderr` after a deliberate diagnostic change (review the
diff before committing; never accept it blind):

```bash
TRYBUILD=overwrite cargo test -p rstest-bdd --test trybuild_macros
```

Refresh the `ui_lints` lockfile (only needed if that crate's bins or
dependencies changed):

```bash
make update-ui-lints-lock
```

### Documentation edits (EP-M5)

Each item names the file and the passage. Prefer delegating the mechanical
prose edits to the `scribe` sub-agent, with minimal, evidence-based changes that
preserve project terminology.

1. **New** `docs/adr-019-type-directed-step-return-classification.md`. Status
   `Accepted`. Must state, in the rationale, *why* autoref sidesteps ADR-002's
   line 27-28 constraint: the choice is made by **method-resolution ordering
   across autoref steps, not by impl coherence; the two impls live on two
   disjoint traits and never overlap**. Without that sentence a future reader
   re-derives the 2025 constraint and reverts the change. Also record that the
   design satisfies `docs/ergonomics-and-developer-experience.md`'s first
   guiding principle ("reduce ceremony … where intent can be clearly
   inferred"), since it removes the need for `result` hints entirely.
2. `docs/adr-002-stable-step-return-classification.md`: keep Status
   `Accepted.`, add a dated amendment banner in the Status section following
   the precedent at `docs/adr-005-async-step-functions.md:5-9`, and add an
   `## Amendments` section. Correct or past-tense lines 32-37 (the "removing
   the need for trait trickery" claim), 39-43, 57-63 (the inverted
   alias paragraph — the most dangerous stale passage in the repository),
   69-70, and 78-82. Add a fourth entry to `## Alternatives considered`
   recording that autoref specialization was not on the table in 2025-12, so
   ADR-019 reads as filling a blind spot rather than reversing a considered
   decision.
3. `docs/rstest-bdd-design.md` §3.8: rewrite lines 2884-2889 and 2891-2901.
   The second passage currently *mandates* "require or diagnose an explicit
   `result`/`value` choice", which the delivered design deliberately does not
   do — it resolves instead. Keep the gauss beta3 narrative at 2892-2896; it is
   the only place the empirical failure is recorded. Redraw the Mermaid
   sequence diagram at 2906-2934 (see below) and update the caption at
   2936-2937.
4. `docs/users-guide.md` lines 389-429: rewrite 401-410 and 417-422. **Do not
   change the `### Step return values` heading text** —
   `docs/adr-006-fallible-scenario-functions.md:17` deep-links its anchor.
   Re-scope the hint-validation paragraph at 412-415 to `result` only. Lines
   424-429 survive unchanged. Note that `make lint` runs
   `scripts/check_users_guide_links.py`, so any new links here are gated.
5. `docs/developers-guide.md`: add a new subsection. There is currently **no**
   coverage of return classification in this file at all — verified by grep —
   and `AGENTS.md`'s abstraction policy requires documenting a new
   abstraction's ownership boundaries, permitted call-sites, and composition
   rules. Copy the structure of the existing "Generated-wrapper Tokio bridge"
   subsection verbatim, including its breaking-change sentence.
6. `docs/known-issues.md`: add one entry for the residual limitations, in the
   file's existing Status / Affected usage / Symptom / Reproduction /
   Workaround / Next steps schema. Cover the shapes that classify as values and
   would swallow an `Err`: `Box<Result<..>>`, `Option<Result<..>>`,
   `&Result<..>`, and a type with `Deref<Target = Result<..>>`. Each entry must
   name the regression test that pins it, so the list is executable rather than
   prose — ADR-002's own conclusion is that prose guidance alone is not an
   adequate guard against a false green. (`impl Trait` and nested `Result` are
   *not* listed here: `DEC-007` makes them compile errors.)
7. `docs/v0-6-0-migration-guide.md`: per `DEC-009` this is a beta-to-final
   change, so the content goes in the guide's existing `## Breaking changes`
   list rather than a `(v0.7.0)`-suffixed subsection. Four insertions: the
   breaking-changes bullet, a body subsection, an item in
   `## Migration checklist` for auditing steps with named-alias return types,
   and an entry in `## Common errors and fixes` keyed on the **literal rustc
   text** so a stuck adopter can paste-and-search. Lead with "scenarios that
   previously passed may now correctly fail". Cover all three changes: `Err`
   now fails the scenario; the `Ok` payload shape changes from `Result<T, E>`
   to `T` so previously dropped payloads now override fixtures; and `E` must
   implement `Display`, with `Display` as the **only** recommended remedy
   (`DEC-006`).
8. `docs/v0-5-0-migration-guide.md` lines 64-66: add a parenthetical qualifier.
   The statement stays true for `#[scenario]` but a reader will over-generalize
   it to steps.
9. `docs/ergonomics-and-developer-experience.md` §3.1 bullet 3, lines 236-243:
   rewrite. It currently states the wrapper "will inspect its return type",
   which is no longer how the value/result split is decided.
10. `docs/adr-006-fallible-scenario-functions.md` lines 16-18, 48, and 126: add
    a one-line note that scenarios retain `ReturnKind` while steps use
    type-directed dispatch. Do not reopen ADR-006.
11. `docs/adr-015-insert-outcome-for-step-return-overrides.md`: no change
    needed, but confirm the payload-shape change does not invalidate its
    `NoMatch`/`AmbiguousIgnored` policy statement.
12. `docs/contents.md`: note ADR-002 is amended by ADR-019 at lines 66-67;
    insert the ADR-019 bullet after ADR-018 at lines 104-105; add the
    `[adr-019]:` link reference to the **alphabetically sorted** block at lines
    111-128, after `[adr-018]`.
13. `docs/CHANGELOG.md` under `## Unreleased`: a behaviour-change entry leading
    with the green-to-red warning.
14. `docs/releasing-crates.md`: record the macro/runtime version-skew
    constraint (`DEC-008`).
15. `docs/roadmap.md`: tick 11.3.1; correct its design-doc citation from §2.1
    to §3.8; add a note that it landed in v0.6.0, since item 11.3 sits
    physically under the `## 12 … v0.7.0` heading despite its `11.` numbering.
16. `docs/documentation-style-guide.md`: optional one-line addition recording
    `Amended by ADR-NNN (YYYY-MM-DD)` as the sanctioned amendment form. There
    is no such convention today and `docs/adr-005` invented one ad hoc.

### Mermaid redraw (`docs/rstest-bdd-design.md` lines 2906-2934)

Two lines are specifically wrong: line 2921
(`Wrap->>Wrap: normalize return value` — normalization is no longer
wrapper-local, and this self-message hides the entire mechanism) and line 2920
(`Step-->>Wrap: returns (any type or Result<T,E>)` — the discrimination point
is now the interesting event and is invisible). Add a participant for the
runtime dispatch and split the self-message into the two-phase tag-then-
normalize sequence, with an `alt` separating the still-macro-side `Unit` path
from type-directed dispatch — otherwise the diagram asserts that dispatch always
runs, which is false.

Two `make nixie` constraints apply. Participant labels containing spaces or
parentheses after `as` must be quoted. Raw newlines in node labels are
rejected; use `<br/>`, following the precedent in the adjacent diagram at line
2928. Add a "For screen readers:" preamble while redrawing — the second diagram
in this section has one at line 2946 and this one does not, and the
documentation style guide requires it for complex diagrams.

### Commit discipline

Commit after each stage so every step is revertible and reviewable:

1. `Split step attribute parsing out of macros/mod.rs` (EP-M1)
2. `Extract shared step-return test support` (EP-M1)
3. `Add failing regressions for aliased step return types` (EP-M2)
4. `Classify step return types by type, not by syntax` (EP-M3)
5. `Reconcile generated-wrapper lint expectations` (EP-M4)
6. `Record type-directed step return classification` (EP-M5, docs)

Follow the repository's commit-message conventions: imperative mood, subject
line 50 characters or fewer, body wrapped at 72. Run the full gate set before
each commit; do not commit changes that fail a gate.

## Validation and acceptance

### Roadmap finish-line mapping

Roadmap 11.3.1 requires that "compile and runtime regressions cover an alias
that returns `Err`, spelled `Result`, `StepResult`, an alias explicitly marked
`result`, and a genuine value alias; no `Err` case is boxed and discarded as an
opaque payload."

| Finish-line item | Evidence |
| --- | --- |
| alias returning `Err` | `step_return::scenario_alias_no_hint_failure` |
| spelled `Result` | existing `step_return_fallible_result_failure.feature` |
| `StepResult` | existing `step_return_stepresult_failure.feature` |
| alias marked `result` | existing `step_return_alias_override.feature` |
| genuine value alias | `step_return::scenario_genuine_value_alias` |
| no `Err` boxed and discarded | INV-2, plus the seeded-fault negative control |
| compile regression | `ui_macros/step_return_alias_error_not_display.rs` |

### Red-green-refactor evidence

- **Red**: `cargo nextest run -p rstest-bdd --test step_return scenario_alias_no_hint_failure`
  reports `note: test did not panic as expected`. That is the false green
  reproduced inside the suite, and it is the precise failure mode issue #573
  reports.
- **Green**: after EP-M3 the same command passes, and the scenario's panic
  message contains `alias failure`.
- **Refactor**: EP-M4's lint reconciliation changes no behaviour; re-run the
  focused test plus the full gate set.

Do not leave any expected-failure marker in the final tree.

### Quality criteria

- Tests: `make test` passes, including `cargo test --doc --workspace
  --all-features` and `uv run pytest scripts/tests`.
- Verification: INV-1 through INV-5 and LEMMA-1 discharged as described, with
  the seeded-fault transcript recorded. Residual gap stated: totality is
  established over an enumerated partition, not over all types.
- Lint: `make lint` passes, which includes clippy `-D warnings`, `cargo doc`
  under `RUSTDOCFLAGS`, the full Whitaker Dylint suite, the Python linters, the
  400-line file check, the users' guide link check, the GPUI mapping-table
  check, and the serial/nextest matrix check.
- Format: `make check-fmt` passes. `make markdownlint` and `make nixie` pass.
- Performance: no benchmark gate. If a compile-time check is wanted, the cheap
  recipe is a generated 500-step crate measured with `hyperfine` before and
  after; escalate only if the per-step cost exceeds ~2 ms or the `rstest-bdd`
  test-target compile time regresses more than 5%. Expected impact is
  unmeasurable: only ~40 of 436 step sites in this workspace dispatch at all.
- Security: none applicable; `unsafe_code` remains forbidden.

## Idempotence and recovery

Every step is re-runnable. The gates are pure checks. The scratch crate at
`~/scratch-11-3-1` is outside the repository and outside `/tmp`, so re-running
Stage A neither dirties the tree nor consumes `/tmp`.

The one destructive-feeling command is `TRYBUILD=overwrite`, which rewrites
checked-in `.stderr` files. Always review its diff before committing; a blind
accept would silently ratify a degraded diagnostic, which is exactly the
contract those snapshots exist to protect.

The seeded-fault negative control must be reverted immediately after its
transcript is captured. It deliberately reintroduces the defect.

Two environment cautions carried from prior work in this repository:

- `make fmt` can *introduce* MD039/MD013 violations, so always run
  `make markdownlint` **after** `make fmt`, never only before.
- Disable the `weave` merge driver before any rebase (`*.md merge=text` in
  `info/attributes`), or Markdown blocks get relocated.

Use the shared default Cargo cache. If another Cargo job holds the
package-cache lock, wait for it rather than working around it with a separate
cache.

## Artefacts and notes

### Probe matrix from the planning falsification experiment

Run on `rustc 1.85.0` (MSRV) and `rustc 1.98.0`; identical on both. This
validated the **bare `&V`** variant; Stage A must re-run it for the
`StepReturnProbe` newtype variant chosen in `DEC-002`.

| Return shape | Tag |
| --- | --- |
| `()` | Value |
| `i32`, `String` | Value |
| `Result<(), String>` spelled | Result |
| `Result<i32, MyError>` | Result |
| `MyResult<()>` local alias (issue #573) | Result |
| nested alias `type Outer = Inner<i32>` | Result |
| alias with defaulted parameter `R2<T, E = MyError>` | Result |
| `std::io::Result<()>` | Result |
| newtype `Wrapper(Result<i32, MyError>)` | Value |
| `&'static Result<i32, MyError>` | Value |
| `Box<Result<..>>`, `Option<i32>`, `Deref<Target = Result<..>>` | Value |
| `Result<i32, E>` where `E: !Display` | Result, then compile error |
| `&'a str` borrowed from an argument | Value, then lifetime error |
| `-> impl Debug` hiding a `Result` | Value (hence `DEC-007`) |
| `Result<(), MyError>` after `.await` | Result |

Two observations from that run that shaped the design. The naive
`match expr { out => (&out).kind().normalize(out) }` shape trips
`clippy::match_single_binding` **and** `clippy::needless_borrow`; a `let`-bound
shape is clean. And `#[expect(clippy::needless_borrow)]` would be actively
wrong, because the borrow is load-bearing only on the value arm, so the
expectation would go unfulfilled on every `Result` step.

### Invariants to write into the module `//!` header

Order matters: the silent one goes first.

1. The two traits **must** declare a method with the *identical* name.
   Renaming one does not fail to compile — the generated call resolves to
   whichever trait still matches, and every non-matching return type silently
   takes the wrong arm. This is the only silent-breakage path.
2. `StepReturnValueKind` **must** be implemented for `&StepReturnProbe<..>`,
   never for `StepReturnProbe<..>` directly. Implementing it without the
   reference makes both methods applicable at the same probe step, producing
   `E0034` for every step. Loud, but it is the obvious "simplification".
3. The two impls **must** live on two disjoint traits. Merging them into one
   trait with two impls is `E0119` — this is exactly the wall ADR-002 hit; cite
   ADR-002 lines 27-28.
4. The tags **must** be zero-sized with no lifetime parameter, and `normalize`
   **must** take `self` by value. The mechanism depends on NLL ending the probe
   borrow before `normalize` moves the value; a tag that borrows reintroduces
   `E0505`.
5. The generated `&` in the probe is **not** a needless borrow. Clippy may
   propose stripping it; whether the stripped form still dispatches correctly is
   non-obvious and version-sensitive.
6. Reference: dtolnay's autoref-specialization case study, and the sibling
   `(&error).anyhow_kind().new(error)` in `anyhow!`. Cite it — a maintainer who
   recognizes the pattern will not "fix" it.

Add `// GUARD:` pointers from the module docs to
`crates/rstest-bdd/tests/step_return_dispatch.rs`, and reciprocal
`// Guards invariant N of step_return.rs` comments in that test.

### Signposted documentation

Read in this order before starting.

| When | Document | Why |
| --- | --- | --- |
| Before anything | `AGENTS.md` | 400-line cap, `//!` docs, en-GB, `.expect()` policy, abstraction-documentation policy |
| Before anything | `docs/contents.md` | The index `AGENTS.md` mandates for choosing where documentation lands |
| Before anything | `docs/repository-layout.md` | Orientation across crates |
| Design | `docs/adr-002-stable-step-return-classification.md` | The decision being amended |
| Design | `docs/rstest-bdd-design.md` §3.8, lines 2828-2944 | The classification prose and diagram (**not** §2.1, which the roadmap wrongly cites) |
| Design | `docs/documentation-style-guide.md` §ADRs | ADR-019's required sections |
| Implementation | `docs/developers-guide.md`, "Generated-wrapper Tokio bridge" | The exact template for documenting a `#[doc(hidden)]` runtime-to-macro bridge |
| Implementation | `docs/complexity-antipatterns-and-refactoring-strategies.md` | Guidance for the EP-M1 extractions |
| Tests | `docs/testing-strategy.md` | The structural-versus-semantic split; the tag table is *structural* and must be justified as such |
| Tests | `docs/rust-testing-with-rstest-fixtures.md` | Fixture and parameterization conventions |
| Tests | `docs/rust-doctest-dry-guide.md` | Doctests run under `make test` |
| Tests | `docs/gherkin-syntax.md` | Feature-file syntax for the new scenarios |
| Tests | `crates/rstest-bdd/tests/ui_lints/` and `Makefile` target `update-ui-lints-lock` | The generated-code lint-cleanliness gate |
| Docs | `docs/v0-6-0-migration-guide.md` | Where the migration content lands (`DEC-009`) |

### Signposted skills

| Stage | Skill | Why |
| --- | --- | --- |
| Planning | `execplans` | This document's format and living-section obligations |
| A | `rust-router` | Load first; routes to the smallest useful follow-on skill |
| A, D | `rust-types-and-apis` | The change is entirely trait shape, method resolution, and a `#[doc(hidden)]` surface with breaking-change semantics |
| D | `rust-errors` | The `E: Display` boundary, the sealed marker trait, `#[diagnostic::on_unimplemented]` |
| A, D | `codegraph-mcp` | Enumerating every `ReturnKind` call site before narrowing it |
| C | `rust-unit-testing` | `rstest` `#[case]` tables, `pretty_assertions`, `googletest`, `insta` |
| C, E | `nextest` | `make test` prefers `cargo nextest`; the trybuild binary is in a serialized test group |
| B | `rust-unused-code` | The EP-M1 extractions will surface `dead_code` questions |
| F | `arch-decision-records` | ADR-019 and the amendment-versus-supersession call |
| F | `en-gb-oxendict` | Hard-gated by `make markdownlint` via `typos` |
| E | `addressing-whitaker-findings` | `make lint` runs the full Whitaker Dylint suite over new public API |
| Landing | `commit-message`, `pr-creation`, `comenq-coderabbit` | Repository-standard commit, pull-request, and review loop |

Deliberately **not** recommended, recorded here to pre-empt the question:
`proptest`, `kani`, and `verus` (see `Verification plan`); `arch-crate-design`
(no crate boundary moves — the module lands in the existing `rstest-bdd`
crate).
