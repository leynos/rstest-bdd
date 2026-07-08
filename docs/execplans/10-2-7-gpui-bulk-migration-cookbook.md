# Bulk-migration cookbook: share one durable-handle step library across many scenarios (10.2.7)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

Approved 2026-07-06: the maintainer accepted both Decision 1 (harness-agnostic
validation vehicle) and Decision 2 (published-`gpui 0.2.2` bridge as a hard
requirement). Implemented and delivered the same day; all gates and CodeRabbit
green. See "Outcomes & retrospective".

Roadmap item: 10.2.7 (phase 10, "First-cut beta feedback: v0.6.0-beta3 quick
wins"). Design reference: `docs/rstest-bdd-design.md` §2.7.6.2.

This draft has been through a six-lens Logisphere design review (structural,
alternatives, failure-modes, contracts, scaling, developer-experience); the
review changed the validation approach substantially. See the Decision Log for
what changed and why.

## Purpose / big picture

A team migrating a large stateful GPUI test suite to `rstest-bdd` 0.6.x today
faces a copy-paste tax. The durable-handle interim pattern — a resettable
scenario-state container, a reset protocol, a cleanup fixture, and the
`#[given]`/`#[when]`/`#[then]` steps that store and rebuild handles — is
documented as a *single-scenario* worked example. When a crate has twenty
scenarios, a naive reader copies both the scaffolding *and* the step definitions
into every test file.

After this change, a reader can follow one cookbook subsection that shows how to
place the durable-handle **step library** (the steps *and* the state
scaffolding) in a single shared module inside a consuming crate, and reuse it
across many scenarios and many feature files without duplication. The reader can
also observe the mechanism working: a small executable reference suite binds two
scenarios in two feature files to one shared step library and passes under
`make test`.

You can see success four ways:

1. `docs/users-guide.md` contains an expanded "Bulk-migration cookbook"
   subsection that documents sharing the *step library* (not only the state
   scaffolding), is framed as the v0.6.0 shape that v0.6.1 shrinks, bridges the
   GPUI specifics to published `gpui 0.2.2`, and points at the executable
   reference.
2. A new executable reference suite proves one shared step library is reused by
   two scenarios across two feature files, with **zero** step definitions in the
   binding files.
3. That suite fails for the expected reason before the shared *steps* exist
   (Red), then passes after they are added (Green), proving the reuse is real
   rather than incidental.
4. `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
   `make nixie` all pass, and the two named scenario tests appear in the
   `make test` output (not merely a green exit code).
5. **Every code example in the cookbook is backed by a test.** No fenced Rust
   example in the new or expanded cookbook subsection is illustrative-only:
   each maps to a named runtime test or a trybuild compile-pass fixture (or, for
   a GPUI-specific snippet that cannot run in the harness-agnostic crate, to a
   named item in the existing `stateful_window.rs` reference), and the prose
   names that backing so a reader can find it.

### Why this is not "already done"

A subsection titled "Bulk-migration cookbook" already exists in
`docs/users-guide.md` (roughly lines 1404-1452, introduced by PR #519 as prose
only). It shares only the *state scaffolding* — the `ScenarioState` struct,
`thread_local!`, reset helpers, `Drop` guard, and `scenario_state_cleanup`
fixture — and says the shared module contains "exactly the boilerplate from the
worked example above". It does **not** show the `#[given]`/`#[when]`/`#[then]`
**step definitions** living in the shared module, which is the substance of
"share one durable-handle **step library** … so teams … do not copy the
**helper code** per-scenario". It also has **no executable mirror**, unlike the
sibling "Third-party harness adapter cookbook" (mirrored by a trybuild fixture
`crates/rstest-bdd/tests/fixtures_macros/scenario_third_party_harness_cookbook.rs`
plus a runtime test `crates/rstest-bdd/tests/third_party_harness_cookbook.rs`).
This plan closes both gaps.

## Approach and its two live decisions

The work is documentation plus one executable reference suite. Two decisions
below were shaped by the design review and should be confirmed at the approval
gate.

### Decision 1 — validation vehicle: harness-agnostic (recommended) versus GPUI-specific

The property the cookbook actually teaches is **structural and
harness-agnostic**: steps defined once in a `#[path]`-included module register
(via the `inventory` crate, at binary link time) into every binary that
includes them, so N scenarios across M feature files resolve against ONE step
library with no per-scenario copy. GPUI's `TestAppContext`, durable handles, and
`VisualTestContext` are incidental to that property.

The recommended vehicle is therefore a **harness-agnostic** runtime suite in the
`rstest-bdd` crate, modelled on the existing shared-step precedent
`crates/rstest-bdd/tests/common/noop_steps.rs` (already `#[path]`-shared across
four test binaries) and on `crates/rstest-bdd/tests/scenario_state.rs` (which
uses the `#[derive(ScenarioState)]` + `Slot<T>` + `#[fixture]` durable-state
primitives without gpui and without thread-locals). This is:

- cheaper and lint-clean (no dead-code from per-feature-unused helpers);
- not obsolescence-bound (it uses the `Slot<T>`/`ScenarioState` primitives that
  roadmap 11.1.3's `ScenarioStore<T>` builds on, rather than the thread-local
  boilerplate 11.1.3/11.1.4 will delete);
- free of the vendored-vs-published gpui mismatch (see Decision 2); and
- free of the cross-binary GPUI concerns the review examined and dismissed.

The GPUI-specific half of the claim (durable handles, `VisualTestContext`
reconstruction, the two-sided reset) is **already** executably proven by
`crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`. The cookbook prose
composes the two: it teaches the GPUI durable-handle library and cross-links to
`stateful_window.rs` for the executable GPUI reference and to the new suite for
the executable *sharing* reference.

The rejected alternative — a second feature-gated GPUI suite with two
window-opening binaries — was declined because it re-proves the harness-agnostic
sharing mechanism at high cost, hand-builds boilerplate scheduled for deletion
in v0.6.1, trips the dead-code lint, and would be built by copying
`stateful_window.rs` wholesale (Constraint 2 forbids refactoring it), i.e. the
anti-duplication exemplar would itself be duplication.

Confirm at approval: harness-agnostic executable proof plus GPUI prose
cross-linked to `stateful_window.rs` (recommended), versus a new GPUI-specific
executable suite.

### Decision 2 — audience bridge: the cookbook is for published `gpui 0.2.2`

The cookbook's audience is teams migrating real suites, who depend on the
**published** `gpui 0.2.2`, not the vendored fork the repository tests against.
Published gpui differs from vendored in four documented ways (see the mapping
table in `docs/users-guide.md` under "Durable handles versus visual context"
and design §2.7.6.2). The cookbook prose must therefore carry a one-line pointer
to that existing mapping table rather than duplicating it, and any GPUI snippet
must state it is written against vendored gpui. This is a hard requirement
(Constraint 8), not a nicety.

## Constraints

1. **No public API changes.** Documentation plus test-only code. No new
   exported types, traits, functions, or macro parameters on `rstest-bdd`,
   `rstest-bdd-macros`, `rstest-bdd-harness`, `rstest-bdd-harness-gpui`, or
   `rstest-bdd-harness-tokio`.
2. **Do not modify the existing reference suites.**
   `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs` and
   `crates/rstest-bdd/tests/scenario_state.rs` and their feature files keep
   passing unchanged. New files are added alongside.
3. **Binding files contain zero step definitions.** No new test file that binds
   a `#[scenario]` may itself define a `#[given]`/`#[when]`/`#[then]`. Every step
   comes from the shared module. This is both the reuse proof and a guard
   against duplicate-step registration within a binary.
4. **Lint-clean under the pedantic profile.** New Rust code passes
   `clippy::expect_used`, `clippy::unwrap_used`, `clippy::shadow_reuse`,
   Whitaker `no_unwrap_or_else_panic`, and — specifically — `dead_code`: every
   step and helper in the shared module must be exercised by at least one
   including binary, or carry a tightly scoped `#[expect(dead_code, reason = …)]`.
   Prefer designing the two feature files so their union uses every shared item.
5. **Documentation style.** Markdown obeys `docs/documentation-style-guide.md`:
   prose and bullets wrapped at 80 columns, code blocks at 120, dashes for
   bullets (never `+` or `*`), no inline HTML (keep angle-bracket placeholders
   inside code spans), en-GB Oxford spelling in prose. Validate with
   `make markdownlint` and `make nixie`.
6. **Use only existing dependencies.** No new crate and no new feature flag. The
   `rstest-bdd` crate's `[dev-dependencies]` already provide `rstest`,
   `rstest-bdd-macros`, `rstest-bdd-harness`, `serial_test`, `insta`,
   `proptest`, `trybuild`. Do **not** reference `googletest` or
   `pretty_assertions`; they are not dependencies. Use plain `assert_eq!`/
   `assert!` as `scenario_state.rs` does.
7. **Fixture provenance uses the qualified `#[from]` form.** Bind the shared
   cleanup fixture as `#[from(<module>::scenario_state_cleanup)]` (module-
   qualified), matching the existing cookbook subsection, so provenance is
   visible at the `#[scenario]` site. The shared fixture and any items the
   binding files reference must be `pub`. Do not switch to an un-qualified
   `use` import (it disagrees with the existing subsection and hides provenance).
8. **GPUI prose bridges to published gpui.** The cookbook must point at the
   existing vendored-vs-published mapping table and mark any GPUI snippet as
   vendored-API. Do not hand adopters vendored-only step bodies without the
   bridge.
9. **Every example is test-backed (no unbacked snippets).** Each fenced Rust
   code example introduced or expanded by this change must be validated by a
   test, using exactly one of these backings, and the nearby prose must name it:
   - a runtime integration test that executes the example (the new shared-library
     suite, for the sharing snippets); or
   - a trybuild compile-pass fixture that compiles the example (for structural
     snippets that are not worth executing); or
   - for a GPUI-specific snippet that cannot compile or run in the harness-
     agnostic `rstest-bdd` crate, an explicit cross-reference to the concrete
     item in `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs` that
     exercises the same shape.

   A snippet that is genuinely non-compilable by nature (for example a file-tree
   `text` block, a `toml` fragment, or a `gherkin` feature body) is exempt from
   *compilation* but its Rust counterpart must still be backed; feature bodies
   used by the suite are inherently covered because the suite binds to them.
   The trybuild compile-pass fixture is therefore **required**, not optional
   (see Stage C), because the cookbook's structural Rust snippet is not otherwise
   compiled — user-guide Markdown code blocks are not compiled as doctests in
   this repository.

## Tolerances (exception triggers)

Stop and escalate (document in Decision Log, await direction) when:

1. **Scope:** more than ~10 files touched, or more than ~400 net lines of
   non-generated code (excluding prose and feature files). Intended footprint:
   1 shared module, 2 binding files, 2 feature files, 1 required trybuild
   compile-pass fixture + 1 fixture feature file, plus edits to `users-guide.md`,
   design §2.7.6.2, and `developers-guide.md`.
2. **Interface:** any change touching a public API signature (Constraint 1) —
   stop immediately.
3. **Dependencies:** any new crate or feature flag required — stop.
4. **Red ambiguity:** if the Red stage cannot be made to fail for a *step-
   resolution* reason (see "Plan of work"), stop rather than accept a vacuous
   missing-module compile error as the Red.
5. **Iterations:** if the suite still fails after 3 focused fix attempts for a
   reason not understood, stop.
6. **Decision 1/2 unresolved:** do not begin Stage C until the validation
   vehicle (Decision 1) and the published-gpui bridge (Decision 2) are
   confirmed at approval.

## Risks

1. Risk: The Red stage is written as a missing-module compile error, which
   proves nothing about step reuse. Severity: high (invalidates the proof).
   Likelihood: medium. Mitigation: at Red, commit the shared module *present*
   with the state scaffolding and the `pub` fixture but **without** the
   `#[given]`/`#[when]`/`#[then]` functions, so the binary links and the
   scenario fails at run time with `StepNotFound` on a specific step line.
   Adding the steps turns it green. (If a future build enables
   `rstest-bdd-macros/strict-compile-time-validation`, the same shape fails at
   macro-expansion instead; either is an acceptable Red because both are caused
   by the missing *steps*, not a missing module. Confirm which occurs in
   Stage A.)
2. Risk: The feature-file rebuild foot-gun (design §2.7.6.6) fakes the Red→Green
   delta. `#[scenario(path = …)]` reads `.feature` files via `std::fs` at macro-
   expansion time with no `include_str!`, so Cargo does not track them; editing
   only a `.feature` may not trigger a rebuild, producing a stale-cache false
   pass or fail. Severity: high (the plan's evidence is a build-state
   transition). Likelihood: high. Mitigation: run `cargo clean -p rstest-bdd`
   immediately before the Red run and before the Green run, and cite §2.7.6.6 in
   the cookbook so adopters learn the trap.
3. Risk: A feature-gated or mis-discovered binary compiles to zero tests and
   exits 0, so "make test is green" hides "the suite did not run". Severity:
   medium. Likelihood: medium (the recommended suite is not feature-gated, which
   lowers this, but auto-discovery still applies). Mitigation: acceptance asserts
   the two *named* scenario tests appear in the nextest output; the binding files
   are auto-discovered like `scenario_state.rs` — do not add `[[test]]` entries
   and do not set `autotests = false`.
4. Risk: Dead-code lint on shared steps/helpers not used by every including
   binary. Severity: medium. Likelihood: medium. Mitigation: Constraint 4 —
   design the two feature files so their union exercises every shared item, or
   scope an `#[expect(dead_code, reason = …)]`.
5. Risk: `make lint` runs `check_users_guide_links.py`,
   `check_gpui_mapping_table.py`, and `check_serial_nextest_matrix.py`
   unconditionally. New cross-links must use the reference-style form the link
   checker enforces, and doc edits near the mapping table or serial matrix must
   keep those scripts green. Severity: low. Likelihood: medium. Mitigation: run
   `make lint` as part of the gate; add links in the enforced form.
6. Risk: `make fmt` (mdformat) reintroduces Markdown lint violations
   (MD013/MD039). Severity: low. Likelihood: medium. Mitigation: run
   `make markdownlint` after `make fmt`.

## Progress

- [x] (2026-07-06) Stage A: Decision 1 (harness-agnostic vehicle) and Decision 2
      (published-gpui bridge) approved by the maintainer; design review complete.
      Remaining sub-item, to do at the start of implementation: observe whether
      `--all-features` activates `strict-compile-time-validation` (affects only
      how the Red is described, not its shape).
- [x] (2026-07-06) Stage B-red: added two feature files, the shared module, and
      two binding files; disabling the shared `Given` step made both bindings
      fail at run time with `Step not found at index 0: Given a fresh ledger`;
      restoring it turned them green. Red evidence captured.
- [x] (2026-07-06) Stage B-docs: expanded the cookbook subsection, extended
      design §2.7.6.2, and added the developer-guide note. Docs pass
      markdownlint, `check_users_guide_links.py`, `check_gpui_mapping_table.py`,
      and `check_serial_nextest_matrix.py`.
- [x] (2026-07-06) Stage C-green: shared step library complete; both
      `bulk_migration_cookbook_a`/`_b` scenarios pass; the binding files carry
      zero step macros (reuse proof). Also added the required trybuild
      compile-pass mirror `scenario_bulk_migration_cookbook.rs`, registered in
      `run_passing_macro_tests`, green under plain `cargo test`.
- [x] (2026-07-06) Stage C-unit: added the `ledger_reset_clears_accumulated_balance`
      rstest unit test in binding A.
- [x] (2026-07-06) Stage D-refactor: full gates green via `scrutineer` —
      `make check-fmt`, `make lint`, `make test` (1496 Rust + 53 Python tests),
      `make markdownlint` (98 files), `make nixie`, and the plain-`cargo test`
      trybuild pass (56 fixtures). `coderabbit review --agent` completed with
      zero findings.
- [x] (2026-07-06) Marked roadmap 10.2.7 done with a delivery note; PR #571
      updated to reflect the completed implementation and marked ready.
- [x] (2026-07-06) Post-review round: addressed two review findings — (1)
      strengthened the reference scenarios so the first posts two entries and
      asserts their sum (10 + 5 = 15, catching a set-instead-of-add regression)
      and the second resets mid-scenario before re-posting (100 → reset → 25 =
      25, making a no-op reset observable), adding a "the running total is reset"
      step; (2) gave the trybuild fixture's inline `mod shared` its own `//!`
      docstring. Re-ran the full gates and CodeRabbit: all green, zero findings.

## Surprises & discoveries

- Observation: a "Bulk-migration cookbook" subsection already exists (PR #519,
  commit `bb95cb0`), sharing only scaffolding. Impact: the deliverable is
  expansion + executable backing, not first authoring.
- Observation: the repository already shares a *step library* across binaries —
  `crates/rstest-bdd/tests/common/noop_steps.rs` is `#[path]`-included by
  `async_registry.rs`, `dump_registry.rs`, `diagnostic_duplicates.rs`, and
  `diagnostic_unused.rs`. Impact: strong in-repo precedent for the recommended
  harness-agnostic vehicle.
- Observation: `crates/rstest-bdd/tests/scenario_state.rs` demonstrates durable
  scenario state via `#[derive(ScenarioState)]` + `Slot<T>` + `#[fixture]`, with
  no gpui and no thread-local. Impact: the recommended suite builds on shipped
  primitives and is not obsoleted by 11.1.3.
- Observation: `make test` uses `--workspace --all-targets --all-features`
  (Makefile line 13). Impact: the recommended (non-gated) suite runs in the
  standard gate; whether `--all-features` also flips
  `rstest-bdd-macros/strict-compile-time-validation` (turning missing steps into
  a compile error) is to be confirmed in Stage A — the Red shape in Risk 1 is
  robust to either answer.
- Observation: the design review verified no process-global GPUI init (each
  scenario builds its own `TestAppContext`), so the earlier cross-binary GPUI
  worry was unfounded; this reinforces that a GPUI-specific suite would buy
  little.
- Observation (implementation): rstest's `#[from]` **does** accept a module-
  qualified path — `#[from(bulk_migration_steps::ledger_state)]` compiles and
  produces no unused-import warning, whereas a `use` + bare-ident form leaves the
  imports flagged as unused once the `#[scenario]` macro rewrites the signature.
  Evidence: the trybuild fixture compiled cleanly with the qualified form.
  Impact: Constraint 7 (qualified `#[from]`) is correct and the existing
  cookbook prose's `#[from(common::…)]` shape is valid; an earlier commit message
  that claimed `#[from]` needs a bare identifier was wrong and is corrected in a
  later commit.
- Observation (implementation): `--all-features` did not turn missing steps into
  a compile error for these suites — the Green suite built and passed under
  `cargo clippy --all-features`, and the Red failure surfaced at run time as
  `StepNotFound`. So step resolution here is runtime, and the Red shape is the
  stepless/step-disabled runtime failure.

## Decision log

- Decision: Target Interpretation B — documentation plus an executable
  reference suite — over prose-only. Rationale: the roadmap emphasizes sharing a
  *step library* and not copying *helper code*, and the sibling cookbook has an
  executable mirror; a prose-only change would leave this the only unbacked
  cookbook. Date/Author: 2026-07-06. Status: confirm at approval.
- Decision (changed by design review): Use a **harness-agnostic** executable
  proof in `rstest-bdd`, not a new feature-gated GPUI suite. Rationale: the
  reuse property is harness-agnostic and already precedented by `noop_steps.rs`;
  a GPUI suite re-proves it at high cost, hand-builds thread-local boilerplate
  that 11.1.3/11.1.4 delete in v0.6.1, trips the dead-code lint, targets vendored
  gpui for a published-gpui audience, and would be built by copying
  `stateful_window.rs` (which Constraint 2 forbids refactoring), making the
  anti-duplication exemplar out of duplication. The GPUI half is already proven
  by `stateful_window.rs`; the cookbook prose cross-links to it. Date/Author:
  2026-07-06 (post-review). Status: confirm at approval (Decision 1).
- Decision: The cookbook prose bridges to published `gpui 0.2.2` via the existing
  mapping table and marks GPUI snippets vendored-API. Rationale: the audience
  runs published gpui; vendored-only snippets would not compile for them.
  Date/Author: 2026-07-06 (post-review). (Decision 2, Constraint 8.)
- Decision: No trybuild GPUI fixture; a harness-agnostic trybuild compile-pass
  fixture guards the cookbook's structural snippet, mirroring
  `scenario_third_party_harness_cookbook.rs`. Rationale: that sibling mirrors
  against a minimal stand-in, not a real framework, so a compile guard needs no
  gpui. Originally kept optional for scope; **made required** by the maintainer
  instruction that tests are required for all examples (Constraint 9) — user-
  guide Markdown is not compiled as doctests here, so the structural Rust snippet
  needs this fixture to be test-backed. Date/Author: 2026-07-06 (post-review;
  updated per "tests required for all examples").
- Decision: Every cookbook example must be test-backed (Constraint 9), by a
  runtime test, a trybuild compile-pass fixture, or a cross-reference to a
  concrete `stateful_window.rs` item; non-compilable blocks (`text`/`toml`/
  `gherkin`) are exempt from compilation but their Rust counterparts are not.
  Rationale: maintainer instruction; also matches the repository convention that
  every playbook snippet has an executable mirror. Date/Author: 2026-07-06.
- Decision: No new ADR. Rationale: an additive, reversible, test-only
  documentation-and-testing convention resting on existing ADR-007/ADR-011 and
  design §2.7.6.2; AGENTS.md routes such conventions to the design doc and
  developer guide. Date/Author: 2026-07-06. (Panel concurred.)
- Decision: Rigour level — behavioural tests (rstest-bdd) are the core
  deliverable; one rstest unit test guards the reset helper;
  `insta`/`proptest`/`kani`/`verus` are not added. Rationale: no new formatted
  multivariant output (no snapshot need) and no new invariant over input ranges
  beyond what `stateful_window.rs`'s proptest already covers. Date/Author:
  2026-07-06.
- Decision: Fixture binding uses the qualified `#[from(<module>::…)]` form and
  `pub` items, not a `use` import. Rationale: matches the existing subsection and
  the only working precedents; the `use` form's `#[from]` resolution is
  unproven. Date/Author: 2026-07-06 (post-review). (Constraint 7.)

## Outcomes & retrospective

Delivered against all four success criteria in "Purpose":

1. `docs/users-guide.md` "Bulk-migration cookbook" now documents sharing the
   step library (not only scaffolding), framed v0.6.0→v0.6.1, bridged to
   published `gpui 0.2.2`, and pointing at the executable references.
2. The harness-agnostic reference suite proves one shared step library serves
   two scenarios across two feature files, with zero step definitions in the
   binding files.
3. Red→Green was demonstrated: disabling the shared `Given` step produced
   `Step not found at index 0: Given a fresh ledger`; restoring it turned both
   bindings green.
4. `make check-fmt`, `make lint`, `make test`, `make markdownlint`, `make nixie`,
   and the plain-`cargo test` trybuild pass all succeeded; the two named
   scenarios appear in the test output; CodeRabbit found zero issues.

What went well: the six-lens design review caught, before any code was written,
that the originally planned feature-gated GPUI suite was the wrong vehicle
(obsolescence against 11.1.3/11.1.4, dead-code lint, vendored-vs-published
mismatch, and duplication of `stateful_window.rs`). Switching to a
harness-agnostic proof built on the shipped `Slot<T>`/`ScenarioState` primitives
made the suite lint-clean, cheap, and durable.

Lessons: (1) rstest's `#[from]` accepts a module-qualified path, so the existing
cookbook prose was valid — verify such assumptions with a compile before
"correcting" docs. (2) User-guide Markdown is not compiled as doctests here, so
a trybuild fixture is the right compile-backing for a snippet; the runtime suite
is
the execution-backing. (3) The stepless-module Red is the honest shape — a
missing-`mod` compile error would have proved nothing about step resolution.

Would do differently: draft the plan against the harness-agnostic vehicle from
the start rather than reaching it via review, saving one revision cycle.

## Context and orientation

You are working in the `rstest-bdd` Cargo workspace. `rstest-bdd` is a
behaviour-driven-development (BDD) layer over the `rstest` fixture framework:
you write Gherkin `.feature` files and bind Rust step functions to their
`Given`/`When`/`Then` lines with `#[given]`/`#[when]`/`#[then]`, and bind a
scenario to a feature with `#[scenario(path = …, name = …)]`.

Key terms:

- **Step library:** a set of `#[given]`/`#[when]`/`#[then]` functions. Steps
  register globally at binary link time via the `inventory` crate
  (`inventory::submit!` inside the macro expansion; `inventory::collect!(Step)`
  in `crates/rstest-bdd/src/registry/mod.rs`). Every step compiled into a test
  binary is discoverable by every scenario in that binary, regardless of which
  module defined it. Each integration-test file is its own binary with its own
  registry, so identical step text in two binaries never collides.
- **`#[path]`-shared module:** a file under a `tests/` *subdirectory* (for
  example `tests/common/<file>.rs`) pulled into a binary with
  `#[path = "common/<file>.rs"] mod <name>;`. Cargo does not treat files under a
  `tests/` subdirectory as their own integration-test crate — only files
  directly in `tests/` become test binaries — so a shared, non-binary module
  lives in the subdirectory form. (Confirmed by the Rust Book, "Test
  Organization", and by `tests/common/noop_steps.rs`.)
- **`Slot<T>` / `ScenarioState`:** shipped primitives in
  `crates/rstest-bdd/src/state.rs`. `Slot<T>` is an interior-mutable cell for a
  durable value that persists across steps within a scenario; deriving
  `ScenarioState` gives a `reset()` that clears every slot. A regular
  `#[fixture]` returning a `ScenarioState`-derived struct is the clean, non-
  thread-local way to share durable state across steps when steps do not also
  need mutable harness context (design §2.7.6.2, first bullet).
- **Durable handle (GPUI):** a value valid across steps such as
  `gpui::Entity<T>` or `gpui::AnyWindowHandle`. `VisualTestContext` is not
  durable and is rebuilt on demand. This is the GPUI-specific specialization of
  the shared-state idea and is covered in prose only, cross-linked to the
  existing executable reference.

### Files to read first

1. `docs/roadmap.md` around lines 866-870 — the 10.2.7 item and finish line;
   lines 907-933 — items 11.1.3/11.1.4 (`ScenarioStore<T>` and the cleanup-guard
   macro) that shrink this boilerplate in v0.6.1.
2. `docs/rstest-bdd-design.md` §2.7.6.2 (lines ~1947-2058) — the interim GPUI
   state pattern and the vendored-vs-published mapping table.
3. `docs/users-guide.md`: "Stateful GPUI scenarios with durable handles"
   (~1060) through the "Worked example" (~1178-1344); "Bulk-migration cookbook"
   (~1404-1452) — the subsection to expand; "Third-party harness adapter
   cookbook" (~824-987) — the sibling with a trybuild + runtime mirror and the
   "compile-checked mirror lives at …" maintenance note to imitate.
4. Executable precedents:
   `crates/rstest-bdd/tests/common/noop_steps.rs` and one includer
   (`crates/rstest-bdd/tests/diagnostic_unused.rs`) — shared step library across
   binaries; `crates/rstest-bdd/tests/scenario_state.rs` — `Slot<T>` + fixture
   durable state; `crates/rstest-bdd/tests/third_party_harness_cookbook.rs` and
   `crates/rstest-bdd/tests/fixtures_macros/scenario_third_party_harness_cookbook.rs`
   — the runtime + trybuild mirror pair; and, for the GPUI-specific reference the
   prose cross-links to, `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`
   with `tests/features/stateful_window.feature`.
5. `crates/rstest-bdd/tests/trybuild_macros.rs` (function `step_macros_compile`,
   the `run_passing_macro_tests` list, and the `NEXTEST_RUN_ID` skip guard) —
   only if the optional trybuild fixture is added.

### Repository conventions that constrain the design

- `make test` runs cargo-nextest with `--all-features`; each test binary runs in
  its own process. `#[serial]` is only needed when a binary shares mutable state
  across scenarios (the recommended suite uses per-scenario `#[fixture]` state,
  so it does not need `#[serial]`; add it only if a single binary hosts multiple
  scenarios sharing a process-global). Design §2.7.6.7 has the full matrix.
- Every user-guide playbook snippet has an executable mirror and the prose
  states the suite is authoritative on drift (doc↔suite parity is prose-guarded,
  matching the third-party cookbook; no snippet-extraction checker is required).
- `make lint` also runs `check_users_guide_links.py`,
  `check_gpui_mapping_table.py`, and `check_serial_nextest_matrix.py`.

### Skills and documentation to signpost (for the implementer)

- Skills: `execplans` (this format); `rust-router` → `rust-unit-testing`
  (rstest fixtures, table tests, assertions) and `rust-unused-code` (dead-code
  discipline); `leta` (semantic navigation instead of grep/read); `nextest`
  (runner behaviour, filtersets to assert the named tests ran); `firecrawl`
  (only to confirm an external convention); `logisphere-experts` /
  `logisphere-design-review` (used in planning); `changelog` if a release note
  is expected.
- Documents: `docs/rstest-bdd-design.md` (§2.7.6.1-§2.7.6.7),
  `docs/rstest-bdd-language-server-design.md`,
  `docs/rust-testing-with-rstest-fixtures.md`, `rust-doctest-dry-guide.md`,
  `docs/complexity-antipatterns-and-refactoring-strategies.md`,
  `docs/gherkin-syntax.md`, `docs/documentation-style-guide.md`,
  `docs/developers-guide.md`, `docs/users-guide.md`, `AGENTS.md`.

## Plan of work

Red-Green-Refactor. The suite is written so that the one variable under test is
the presence of the shared *steps*.

### Stage A — orientation and go/no-go (no code changes)

1. Re-read the files above.
2. Confirm Decision 1 (harness-agnostic vehicle) and Decision 2 (published-gpui
   bridge) at the approval gate.
3. Empirically confirm whether `make test`'s `--all-features` activates
   `rstest-bdd-macros/strict-compile-time-validation` (build the crate under
   `--all-features` and check whether a scenario with a missing step is a compile
   error or a runtime failure). Record the answer; it only affects how the Red is
   described, not the Red shape.
4. Choose the two example domains and step text so their union exercises every
   shared item (Constraint 4). A ledger/counter world with, for example,
   `Given a fresh ledger`, `When an entry of {int} is posted`,
   `Then the balance is {int}` used by both features, differing only in the
   posted amounts, keeps every step in use by both binaries.

### Stage B-red — failing behavioural specification

1. Add two feature files under
   `crates/rstest-bdd/tests/features/bulk_migration/`:
   `first.feature` and `second.feature`, each one scenario built from the shared
   step text.
2. Add the shared module
   `crates/rstest-bdd/tests/common/bulk_migration_steps.rs` containing the
   `ScenarioState`-derived world, a `pub #[fixture]` cleanup/state fixture, and
   the reset helper — **but not** the `#[given]`/`#[when]`/`#[then]` functions.
3. Add two binding files directly in `tests/`:
   `bulk_migration_cookbook_a.rs` and `bulk_migration_cookbook_b.rs`. Each
   `#[path]`-includes the shared module and binds one `#[scenario]`, using the
   qualified `#[from(bulk_migration_steps::scenario_state_cleanup)]` form. No
   step macros here (Constraint 3).
4. Force a clean build and run the focused suite (Red):

   ```bash
   cargo clean -p rstest-bdd
   cargo test -p rstest-bdd --test bulk_migration_cookbook_a 2>&1 | tee \
     /tmp/red-rstest-bdd-$(git branch --show-current).out
   ```

   Expected: the binary links (the module and fixture exist), the scenario runs,
   and it fails with `StepNotFound` naming the first unresolved step. Record the
   exact message in `Concrete steps`.

### Stage B-docs — documentation

1. Expand `docs/users-guide.md` "Bulk-migration cookbook":
   - Open with the frame: this is the v0.6.0 shape; v0.6.1 (roadmap 11.1.3/11.1.4,
     `ScenarioStore<T>` + cleanup-guard macro) collapses the shared block to an
     import and generates the cleanup parameter — adopt now, expect to shrink.
   - State that the shared module holds the **step library** (the given/when/then
     functions) *and* the state scaffolding, not only the scaffolding.
   - Explain inventory-per-binary registration and the `tests/common/<file>.rs`
     subdirectory form (and why it avoids an accidental empty test binary).
   - Call out the `pub`-visibility requirement — the one thing that differs from
     copy-pasting the single-file worked example.
   - Standardize on the qualified `#[from(<module>::scenario_state_cleanup)]`
     form; update the existing file-tree and binding snippet to match the shipped
     module name and include form.
   - For the GPUI specialization: keep durable-handle guidance as prose/`no_run`
     snippets marked vendored-API, add the one-line pointer to the existing
     published-vs-vendored mapping table (Constraint 8), and cross-link to
     `stateful_window.rs` as the executable GPUI reference.
   - Add the pointer to the new executable reference suite with the "if a snippet
     drifts, the suite wins" note.
   - Cite the feature-file rebuild foot-gun (§2.7.6.6) so adopters know to touch
     a `.rs` after editing only a `.feature`.
2. Extend `docs/rstest-bdd-design.md` §2.7.6.2 with a short paragraph naming the
   shared-step-library convention as the recommended bulk-migration shape and
   pointing at the executable suite, completing the design↔guide↔suite triangle.
3. Add a `docs/developers-guide.md` maintainer note: the new suite's location,
   that doc↔suite parity is deliberately prose-guarded (no checker), and that
   edits near the mapping table or serial matrix must keep
   `check_gpui_mapping_table.py` / `check_serial_nextest_matrix.py` green.
4. `make fmt` then `make markdownlint`; `python3 scripts/check_users_guide_links.py`
   if links or anchors changed; `make nixie` if any diagram is added (none
   expected).

### Stage C — implementation (green)

1. Add the `#[given]`/`#[when]`/`#[then]` functions to
   `bulk_migration_steps.rs`, using the shared `ScenarioState`/`Slot<T>` fixture
   for durable state. Make every step and helper `pub` where a binding file needs
   it. Ensure the two feature files' union uses every step (Constraint 4).
2. Force a clean build and run both binaries (Green):

   ```bash
   cargo clean -p rstest-bdd
   cargo test -p rstest-bdd --test bulk_migration_cookbook_a \
     --test bulk_migration_cookbook_b 2>&1 | tee \
     /tmp/green-rstest-bdd-$(git branch --show-current).out
   ```

   Assert the two named scenario tests appear and pass — grep the output for the
   scenario test names, not just exit code 0 (Risk 3).
3. Stage C-unit: add one rstest unit test (in the shared module under
   `#[cfg(test)]`) asserting the reset helper returns the world to its default;
   use plain `assert_eq!`.
4. Required (Constraint 9): add the trybuild compile-pass fixture
   `crates/rstest-bdd/tests/fixtures_macros/scenario_bulk_migration_cookbook.rs`
   plus its feature file and register it in `run_passing_macro_tests`
   (`trybuild_macros.rs`), so the cookbook's structural Rust snippet is
   compile-checked. It runs under plain `cargo test`, not nextest (the
   `NEXTEST_RUN_ID` skip guard), so validate it with
   `cargo test -p rstest-bdd step_macros_compile` (repository memory: nextest
   skips trybuild fixtures). This fixture is the compile-backing for the
   cookbook's binding/shared-module snippet; the runtime suite is the execution-
   backing. Every other Rust snippet in the subsection must reuse one of these
   two backings or cross-reference `stateful_window.rs`.

### Stage D — refactor, gates, review

1. Keep binding files thin (attribute + `#[scenario]` + the `#[path]` include +
   the qualified `#[from]`), and verify Constraint 3 by grepping the binding
   files for step macros (expect none).
2. Run the full gate sequence via the `scrutineer` subagent, sequentially:
   `make check-fmt`, `make lint`, `make test`, then `make markdownlint` and
   `make nixie`. On failure, read the cited `/tmp` log and fix before re-running.
   Never silence a lint except as a scoped last resort with a reason.
3. Only once all deterministic gates pass, request `coderabbit review --agent`;
   clear all findings before proceeding.
4. Update `Progress`, `Surprises`, `Decision Log`, `Outcomes`.
5. Mark roadmap 10.2.7 `[x]` with a delivery note citing this ExecPlan and the
   new suite path.

## Concrete steps

Run from the worktree root. Prefer Makefile targets; capture long output with
`tee` to `/tmp/$ACTION-rstest-bdd-$(git branch --show-current).out`.

1. Branch once, before implementation: rename the session branch to
   `10-2-7-gpui-bulk-migration-cookbook` tracking
   `origin/10-2-7-gpui-bulk-migration-cookbook`, push, and open a draft PR for
   this ExecPlan (see "Validation and acceptance").
2. Red: create the two feature files, the stepless shared module, and the two
   binding files; `cargo clean -p rstest-bdd`; run the focused Red command;
   paste the `StepNotFound` transcript here.
3. Docs: edit the three documents; `make fmt && make markdownlint`.
4. Green: add the shared steps; `cargo clean -p rstest-bdd`; run the focused
   Green command; paste the transcript and the grep proving both named tests
   ran.
5. Gates: delegate the full sequential gate run to `scrutineer`; fix from the
   cited logs.
6. Review: `coderabbit review --agent`; clear findings.
7. Commit atomically after each green step (feature files + stepless module +
   red bindings; docs; shared steps + green; refactor).

Illustrative Green transcript shape:

```plaintext
    Running tests/bulk_migration_cookbook_a.rs
running 1 test
test scenario_first_reuses_shared_steps ... ok
    Running tests/bulk_migration_cookbook_b.rs
running 1 test
test scenario_second_reuses_shared_steps ... ok
```

## Validation and acceptance

Acceptance is behavioural and observable:

1. **Red proof.** With the shared module present but step-free,
   `cargo test -p rstest-bdd --test bulk_migration_cookbook_a` fails at run time
   with `StepNotFound` naming a specific step line (or, if strict compile-time
   validation is active, a macro-expansion error citing the missing step) —
   caused by the missing *steps*, not a missing module. Record the message.
2. **Green proof.** After adding the steps,
   `cargo test -p rstest-bdd --test bulk_migration_cookbook_a
   --test bulk_migration_cookbook_b` passes, and both scenarios exercise the same
   step functions defined once in `tests/common/bulk_migration_steps.rs`. The two
   named tests appear in the output.
3. **Reuse proof.** The two binding files contain no
   `#[given]`/`#[when]`/`#[then]` — a reviewer confirms by grepping and finding
   none (Constraint 3).
4. **Every example is test-backed (Constraint 9).** Each Rust snippet in the
   cookbook subsection is traceable to a named runtime test, the trybuild
   compile-pass fixture, or a named `stateful_window.rs` item, and the prose
   names it. `cargo test -p rstest-bdd step_macros_compile` passes with the new
   fixture registered. A reviewer can enumerate the subsection's Rust code
   blocks and point at a backing test for each.
5. **Full gates.** `make check-fmt`, `make lint`, `make test`,
   `make markdownlint`, `make nixie` all pass; the two named scenarios appear in
   `make test` output.
6. **Docs finish line.** `docs/users-guide.md` carries the expanded
   "Bulk-migration cookbook" covering the shared step library, framed as the
   v0.6.0 shape, bridged to published gpui, and pointing at the executable
   suite; §2.7.6.2 and the developer guide reference it.
7. **CodeRabbit.** `coderabbit review --agent` reports no outstanding concerns.

Quality criteria ("done"):

- Tests: the two new behavioural scenarios pass; the reset unit test passes; the
  pre-existing `stateful_window` and `scenario_state` suites still pass.
- Lint/typecheck: `make lint` clean under the pedantic profile, including
  `dead_code` on shared items and the three doc-parity scripts.
- Docs: `make markdownlint` and the link check clean.

Quality method: sequential gate run through `scrutineer`; focused `cargo test`
for Red/Green evidence with `cargo clean -p rstest-bdd` before each; manual grep
of binding files for the reuse proof and of `make test` output for the named
tests.

## Idempotence and recovery

- All new files are additive; re-running any step is safe. Before any focused
  Red/Green run, `cargo clean -p rstest-bdd` guarantees the `.feature` edits are
  seen (design §2.7.6.6), so no stale-cache false result.
- Fixture-based `Slot<T>` state is per-scenario, so scenario order does not
  matter and re-runs are deterministic.
- Documentation edits are text-only and reversible via git; commit atomically so
  any stage can be rolled back independently.

## Interfaces and dependencies

No production interfaces change. Test-only shapes to create (names illustrative;
finalize the domain in Stage A):

Shared module `crates/rstest-bdd/tests/common/bulk_migration_steps.rs`:

```rust
//! Shared durable-state step library for the bulk-migration cookbook.
//! One copy of the state scaffolding and the given/when/then steps, reused by
//! every `bulk_migration_cookbook_*` binding via `#[path]` inclusion. Steps
//! register once per including binary through `inventory`.

use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{given, then, when, ScenarioState};

#[derive(Default, ScenarioState)]
pub struct LedgerState {
    pub balance: Slot<i64>,
}

#[fixture]
pub fn scenario_state_cleanup() -> LedgerState { LedgerState::default() }

// pub #[given(...)] / #[when(...)] / #[then(...)] functions operating on
// `scenario_state_cleanup: &LedgerState`, added in Stage C (absent at Red).
```

Binding file `crates/rstest-bdd/tests/bulk_migration_cookbook_a.rs`:

```rust
#[path = "common/bulk_migration_steps.rs"]
mod bulk_migration_steps;

use rstest_bdd_macros::scenario;

#[scenario(
    path = "tests/features/bulk_migration/first.feature",
    name = "First scenario reuses the shared step library",
)]
fn scenario_first_reuses_shared_steps(
    #[from(bulk_migration_steps::scenario_state_cleanup)] _state: bulk_migration_steps::LedgerState,
) {
}
```

Binding file `crates/rstest-bdd/tests/bulk_migration_cookbook_b.rs` has the same
shape, binding a second scenario in `second.feature`, including the *same* shared
module, with no steps of its own.

Feature file `crates/rstest-bdd/tests/features/bulk_migration/first.feature`:

```gherkin
Feature: Bulk-migration cookbook — first scenario

  Scenario: First scenario reuses the shared step library
    Given a fresh ledger
    When an entry of 10 is posted
    Then the balance is 10
```

The second feature reuses the same step text with different amounts so both
binaries exercise every shared step (Constraint 4).

Dependencies: only crates already in `crates/rstest-bdd/Cargo.toml`
`[dev-dependencies]`. No new dependency, no new feature flag, no
`googletest`/`pretty_assertions`.

## Revision note

Revision 2 (2026-07-06), after a six-lens Logisphere design review. Changed the
validation vehicle from a feature-gated GPUI executable suite to a harness-
agnostic runtime shared-step-library proof in `rstest-bdd` (built on shipped
`Slot<T>`/`ScenarioState` primitives), because the reuse property is harness-
agnostic, the GPUI half is already proven by `stateful_window.rs`, and a GPUI
suite would hand-build v0.6.1-doomed boilerplate, trip the dead-code lint,
target vendored gpui for a published-gpui audience, and duplicate
`stateful_window.rs`. Corrected the Red stage to a stepless-module runtime
`StepNotFound` (was an incoherent missing-module compile error). Added a
`cargo clean -p rstest-bdd` cache guard for the feature-file rebuild foot-gun,
a "named tests must appear" acceptance guard, the qualified `#[from]` form and
`pub` requirement, the published-gpui bridge, the "v0.6.0 shape, shrinks in
v0.6.1" framing, and dead-code/doc-parity-script constraints.

Revision 3 (2026-07-06). Added Constraint 9 — every cookbook example must be
test-backed (a runtime test, the now-required trybuild compile-pass fixture, or
a named cross-reference to `stateful_window.rs`); non-compilable `text`/`toml`/
`gherkin` blocks are exempt from compilation but their Rust counterparts are
not. Promoted the trybuild fixture from optional to required and threaded the
requirement through the success criteria, tolerances footprint, validation
acceptance, and decision log, per the maintainer instruction that tests are
required for all examples. Marked the plan APPROVED after the maintainer
accepted Decisions 1 and 2; implementation may now proceed within tolerances.
