# Diagnose non-canonical harness paths and missing or ambiguous attribute policies (roadmap 11.2.3)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: DRAFT

Approval: not yet approved. Implementation must not begin until the user
explicitly approves this plan.

## Purpose / big picture

`#[scenario]` and `scenarios!` let a test author choose a harness adapter
(`harness = ...`) and an attribute policy (`attributes = ...`). Procedural
macros cannot evaluate trait methods, so the macros recognize first-party
adapters by the *spelling* of the supplied path. When the spelling is not the
one the macros expect, or when two selectors disagree, the generated test
quietly changes shape: `#[tokio::test]` or `#[gpui::test]` disappears, an
explicit override silently contradicts the harness, or an `async fn` scenario
fails with a cryptic `rstest` error that never mentions `attributes = ...`.

After this change, a developer who writes one of those configurations sees a
targeted diagnostic, spanned at the offending argument, naming the problem,
its consequence, and the exact fix: the canonical path to use, the
`attributes = ...` argument to add, or the argument to remove. Every
configuration that is valid and unambiguous today stays silent.

Observable success:

1. A `#[scenario]` whose `attributes =` path reaches the Tokio adapter through
   a nested re-export, such as
   `reexports::rstest_bdd_harness_tokio::inner::TokioAttributePolicy`, now
   warns. The warning names the canonical path
   `rstest_bdd_harness_tokio::TokioAttributePolicy` and states that
   `#[tokio::test(flavor = "current_thread")]` was not applied.
2. An `async fn` scenario with
   `attributes = rstest_bdd_harness::DefaultAttributePolicy` and no harness
   fails with an `rstest-bdd` error that suggests removing `attributes = ...`,
   using `attributes = rstest_bdd_harness_tokio::TokioAttributePolicy`, or
   writing an executor attribute such as `#[tokio::test]`. Previously it failed
   with `rstest`'s generic "async test requires either explicit `test_attr`"
   message.
3. `harness = rstest_bdd_harness_tokio::TokioHarness` combined with
   `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy` warns that the
   two selectors choose different frameworks and suggests removing
   `attributes = ...` or using the matching Tokio policy.
4. `scenarios!` with both `runtime = "tokio-current-thread"` and
   `harness = ...` or `attributes = ...` warns on stable as well as on nightly,
   and suggests removing the deprecated `runtime` argument.
5. Trybuild tests pin each diagnostic's full text, including the suggested
   fix, on stable; nightly twins pin the native warning form.

## Constraints

These are hard invariants. Violating one requires escalation, not a
workaround.

- C1 (semver, v0.6.1): the change is additive. No public item, macro argument,
  or accepted syntax is removed or renamed in any published crate
  (`rstest-bdd`, `rstest-bdd-macros`, `rstest-bdd-harness`,
  `rstest-bdd-harness-tokio`, `rstest-bdd-harness-gpui`, `rstest-bdd-policy`).
- C2 (no new rejections): a program that compiles today under default lint
  levels must still compile. A new *error* may only replace an error that the
  same program already produces (lemma `LEM-ERR-REPLACES`). New *warnings* are
  limited to the three evidence-backed classes defined in `Context and
  orientation`.
- C3 (generated code unchanged): for every configuration that compiles today,
  the emitted test attributes, harness delegation, and trait assertions are
  token-for-token unchanged. Only diagnostic tokens (the stable
  deprecated-item blocks) are added.
- C4 (ADR-008 precedence): the resolution order "explicit `attributes`, then
  known first-party `harness`, then the deprecated `runtime` alias, then the
  runtime fallback" is preserved. Diagnostics describe the outcome of that
  order; they never change it.
- C5 (ADR-017 contracts): diagnostics are computed purely and emitted once at
  the owning expansion boundary; `scenarios!` emits one diagnostic per
  distinct cause regardless of scenario count; canonical first-party paths and
  same-named custom types such as `custom::TokioHarness` stay warning-free; the
  nightly native channel and the stable deprecated-item channel are mutually
  exclusive.
- C6 (no warning for valid third-party usage): a non-first-party
  `harness = ...` without `attributes = ...` stays silent. Seventeen in-repo
  call sites use that shape (measured at `8737d88f`, see `Surprises &
  discoveries`) and ADR-008 declares it valid.
- C7 (file size): no Rust source file may exceed 400 lines.
  `crates/rstest-bdd-macros/src/codegen/mod.rs` is at exactly 400 lines at
  `8737d88f`, so its net line count must not grow.
- C8 (no suppressions): no new `#[allow]`/`#[expect]` lint suppressions.
- C9 (policy-crate tables untouched): do not edit the private tables in
  `crates/rstest-bdd-policy/src/lib.rs`; consume only its public constants and
  resolvers. Open draft PR #783 (issue #754) is restructuring those tables.
- C10 (tooling): no new external dependency, no Verus or Kani toolchain
  wiring. The only permitted manifest change is adding the workspace-internal
  `rstest-bdd-harness-tokio` crate as a dev-dependency of
  `rstest-bdd-harness-gpui`, and only if the Milestone 0 spike proves it
  necessary.

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, and escalate.

## Tolerances (exception triggers)

- Scope: stop and escalate if production code (excluding tests, fixtures, and
  documentation) grows by more than 700 net lines, or if more than 40
  non-fixture files change.
- Interface: stop if any `pub` item of a published crate must change
  signature, or if a macro argument must be added or removed.
- Dependencies: stop if any external crate must be added, or if a manifest
  change beyond C10's single permitted dev-dependency is needed.
- Behaviour: stop if any existing trybuild `pass` fixture, macrotest
  expansion snapshot, or `harness_defaults` unit case changes, except for
  diagnostic text in `compile_fail` `.stderr` files listed in this plan.
- Iterations: stop if a gate still fails after three fix attempts in one
  milestone.
- Time: stop if any single milestone exceeds one working day.
- Ambiguity: stop if implementation evidence contradicts the taxonomy in
  `Context and orientation`, for example a further configuration class that
  is plausibly "ambiguous" but not listed.

## Risks

- Risk R1: users who build with `-D warnings` (or `#![deny(deprecated)]`)
  and already have a conflicting or alias-plus-selector configuration will see
  a build failure after upgrading. Severity: medium. Likelihood: low (zero
  in-repo instances of either shape at `8737d88f`). Mitigation: restrict new
  warnings to configurations where explicit arguments contradict each other
  or a path demonstrably fails recognition; document each in the users' guide
  and `docs/CHANGELOG.md`; never warn on valid third-party usage (C6).
- Risk R2: draft PRs #783 (policy hint tables) and #784 (macro argument
  parsers) touch `rstest-bdd-policy/src/lib.rs`,
  `macros/scenario/args.rs`, and `macros/scenarios/macro_args/mod.rs`.
  Severity: medium. Likelihood: medium. Mitigation: C9 keeps this plan out of
  the policy tables; argument-parser edits are confined to capturing one span.
  Rebase onto `origin/main` at the start of each milestone and re-run the
  focused tests if either PR has merged.
- Risk R3: trybuild `.stderr` snapshots are toolchain-sensitive, and
  `rust-toolchain.toml` floats on `stable`. Severity: low. Likelihood: medium.
  Mitigation: follow the existing fixture idiom; reuse the normalizers in
  `crates/rstest-bdd/tests/trybuild_macros/wrappers.rs` only if drift
  appears.
- Risk R4: nightly fixtures may not run in CI, leaving the native channel
  verified only locally. Severity: low. Likelihood: medium. Mitigation:
  Milestone 0 confirms whether CI runs a nightly lane; if not, record the
  local nightly command and its transcript in `Artefacts and notes` for every
  nightly fixture.
- Risk R5: the `rstest` dependency bump (open PRs #775 and #810, 0.26 to
  0.27) may change `rstest`'s rule that an `async` test needs an attribute
  whose path ends in `test`. The missing-policy error mirrors that rule; if
  `rstest` relaxes it, the new error would reject a program `rstest` accepts,
  violating C2. Severity: high. Likelihood: low. Mitigation: the axiom
  `AX-RSTEST-ASYNC` is guarded by a canary trybuild fixture that compiles a
  plain `rstest` async test with no executor attribute and pins `rstest`'s own
  error. A dependency bump that changes the rule fails the canary loudly.
- Risk R6: the ambiguous-framework fixture (`TokioHarness` plus
  `GpuiAttributePolicy`) may produce extra rustc errors because the GPUI
  attribute and the Tokio harness are combined, or because one crate is not
  resolvable in the trybuild scratch crate. Severity: low. Likelihood:
  medium. Mitigation: the Milestone 0 spike decides the fixture's host suite
  before any production code is written.
- Risk R7: `crates/rstest-bdd/tests/trybuild_macros.rs` (359 lines) and the
  harness `macro_compile.rs` files may approach the 400-line cap as fixtures
  are registered. Severity: low. Likelihood: medium. Mitigation: register new
  fixture groups in the harness-crate suites, and split registration into a
  child module first if a file would exceed 380 lines.

## Progress

- [x] (2026-10-15) Branch prepared; reconnaissance (four Wyvern agents) and
  prior-art research completed; draft written.
- [ ] Community-of-experts design review completed and findings folded in.
- [ ] Plan approved by the user.
- [ ] EP-M0: baseline and fixture-host spike.
- [ ] EP-M1: behaviour-preserving extraction of `policy_diagnostics`.
- [ ] EP-M2: non-canonical path diagnostic.
- [ ] EP-M3: ambiguous selector diagnostics.
- [ ] EP-M4: missing async test attribute diagnostic.
- [ ] EP-M5: behavioural silence and fix scenarios.
- [ ] EP-M6: documentation, ADR-024, CHANGELOG.
- [ ] EP-M7: roadmap tick, final gates, plan marked `COMPLETE`.

## Surprises & discoveries

- Observation: a "missing attribute policy" warning for any third-party
  harness without `attributes = ...` would break the repository's own tests.
  Evidence: a scan of tracked `.rs` files at `8737d88f` found 20 non-first-party
  `harness =` uses, 17 without `attributes =`, all compiled by `make test`
  under `RUSTFLAGS="-D warnings"`. Impact: "missing" is redefined in terms of
  an observable failure (an async scenario with no executor attribute), not
  the mere absence of an argument (decision D2).
- Observation: the existing ADR-017 fallback message offers three remedies,
  but only one of them, the canonical crate-root path, removes the warning.
  Adding `rstest-bdd-harness` as a direct dev-dependency makes the fallback
  *compile* but leaves the warning in place, because `fallback_candidate` in
  `crates/rstest-bdd-macros/src/codegen/adapter_fallback.rs` never consults
  that dependency. Evidence: `fallback_candidate` checks only the terminal
  adapter name, the penultimate crate identifier, and adapter-crate
  resolvability. Impact: the fix-convergence lemma `LEM-FIX-CONVERGES`
  separates *fixes* (which silence the diagnostic) from *workarounds* (which
  make the fallback build), and the rewritten message labels them (decision
  D5).
- Observation: an aliased `attributes = alias::rstest_bdd_harness_tokio::
  TokioAttributePolicy` already warns, but the warning never says that
  `#[tokio::test]` was dropped. Evidence: `first_party_adapter_attribute_hint`
  in `codegen/mod.rs` fails on the `alias` root, so `resolve_attribute_policy`
  in `codegen/scenario/test_attrs.rs` falls to `RstestOnly`. Impact: the
  non-canonical message gains a consequence clause.
- Observation: on stable, the deprecated `runtime = "tokio-current-thread"`
  warnings are no-ops. Evidence: `utils::warnings::emit_warning` is native
  only under `rstest_bdd_nightly`; the stable fixture
  `scenarios_runtime_alias_deprecated.stderr` pins only a forced error.
  Impact: the alias-plus-selector ambiguity is promoted to the dual channel;
  the alias-alone deprecation stays as it is (out of scope).
- Observation: `rstest-bdd-macros` cannot host `rstest-bdd` behavioural tests
  without a dev-dependency cycle on the crate under test. Impact: the
  behavioural suite lives in `crates/rstest-bdd-harness-tokio/tests/`
  (decision D8).

## Decision log

- Decision D1: interpret the roadmap's three cases as three diagnostic
  classes defined by *evidence*, not by syntax alone. "Non-canonical" means
  that a supplied path carries a first-party crate identifier and adapter
  type name but is not recognized. "Missing" means that an `async fn`
  scenario's resolved policy supplies no executor attribute. "Ambiguous" means
  that two explicit selectors choose different frameworks, or that the
  deprecated `runtime` alias is combined with an explicit selector. Rationale:
  the macros can act only on what the token stream proves; each class has a
  demonstrable consequence and a fix that removes it. Date/Author:
  2026-10-15 / Claude (planning).
- Decision D2: do not warn when a non-first-party harness is supplied without
  `attributes = ...`. Rationale: ADR-008 declares harness-only valid; 17
  in-repo sites use it; under `-D warnings` a warning is a build failure, so
  the warning would effectively break a v0.6.1 patch upgrade (C1, C6).
  Third-party policies that cannot be evaluated remain roadmap 12.2.3's
  concern. Date/Author: 2026-10-15 / Claude.
- Decision D3: make the missing-policy diagnostic a hard error, emitted
  instead of the test. Rationale: every configuration it fires on already
  fails to compile with `rstest`'s generic message (axiom `AX-RSTEST-ASYNC`),
  so a clearer error rejects nothing new (C2). Date/Author: 2026-10-15 /
  Claude.
- Decision D4: generalize `codegen::adapter_fallback` into a new
  `codegen::policy_diagnostics` module rather than adding a parallel
  mechanism. Rationale: the abstraction sweep (AGENTS.md policy) found
  `adapter_fallback` to be the only existing emitter with the required purity,
  boundary ownership, and dual-channel behaviour; a second emitter would
  duplicate all three. The stable marker struct for the existing class keeps
  its name, `RstestBddFirstPartyAdapterFallback`, so stderr continuity is
  preserved. Date/Author: 2026-10-15 / Claude.
- Decision D5: rewrite the non-canonical message so that it names the
  canonical path explicitly, states the consequence, and labels the
  dev-dependency advice as a workaround that does not silence the warning.
  Rationale: `LEM-FIX-CONVERGES` (see discoveries). The existing `.stderr`
  files for the alias fixtures change as a result; that change is expected
  and reviewed. Date/Author: 2026-10-15 / Claude.
- Decision D6: compute the ambiguity check on each selector's *family*, so
  that non-canonical first-party paths and the `runtime` alias contribute
  their implied family. Rationale: without this, applying the non-canonical
  fix (switching to the canonical path) or the alias fix (switching to
  `harness = TokioHarness`) can *reveal* a framework conflict that was
  previously unreported, so a suggested fix would introduce a new diagnostic.
  Working `LEM-FIX-CONVERGES` through the abstract model by hand found this
  while the plan was being drafted; the executable exhaustive check in EP-M3
  must confirm it. Date/Author: 2026-10-15 / Claude.
- Decision D7: verification uses exhaustive enumeration of the finite
  abstract model plus `proptest` for the path classifier; Verus and Kani are
  not used (see `Verification plan` for the full justification). Date/Author:
  2026-10-15 / Claude.
- Decision D8: host the `rstest-bdd` behavioural scenarios in
  `crates/rstest-bdd-harness-tokio/tests/`, compiled under
  `#![deny(deprecated)]` so that compiling the suite is itself the
  end-to-end silence check. Rationale: see the discoveries entry on
  dev-dependency cycles. Date/Author: 2026-10-15 / Claude.
- Decision D9: record the taxonomy, the evidence principle, and the
  error-replaces-error rule in a new ADR-024 that amends ADR-017, and
  reference it from design doc §2.7.3. Rationale: the decision fixes
  user-visible diagnostic policy and constrains future roadmap items (12.2.3);
  that is substantive under the documentation style guide. Date/Author:
  2026-10-15 / Claude.

## Outcomes & retrospective

Not started. Complete at each milestone boundary and at completion,
reconciling every discovery against the artefacts listed in `Conformance
basis`.

## Context and orientation

### Terms of art

- *Harness adapter*: a type implementing
  `rstest_bdd_harness::HarnessAdapter + Default`, supplied as
  `harness = path`. It runs the scenario body, for example inside a Tokio
  runtime (`TokioHarness`) or `gpui::run_test` (`GpuiHarness`).
- *Attribute policy*: a type implementing
  `rstest_bdd_harness::AttributePolicy`, supplied as `attributes = path`. It
  documents which test attributes the generated function needs. The macros
  cannot call its methods at expansion time, so only first-party policies have
  any effect on emitted attributes.
- *Test attribute hint*: `rstest_bdd_policy::TestAttributeHint`, one of
  `RstestOnly`, `RstestWithTokioCurrentThread`, `RstestWithGpuiTest`. It is the
  shared vocabulary for "which framework attributes to emit" (ADR-004 makes it
  the single source of truth). This plan calls the hint selected by a path that
  path's *family*.
- *Canonical path*: a path that the macros recognize as first-party:
  an exact match against the `rstest-bdd-policy` constants, such as
  `rstest_bdd_harness_tokio::TokioHarness` (a leading `::` is ignored); a path
  rooted at the adapter crate or its resolved Cargo rename; or an unaliased
  single-segment import such as `TokioHarness` when the adapter crate is
  resolvable. In this plan, *recognized* is used for all three.
- *Evidence*: syntax that proves a path was meant to be first-party without
  being recognized: the terminal segment is a first-party adapter type name
  (`TokioHarness`, `TokioAttributePolicy`, `GpuiHarness`,
  `GpuiAttributePolicy`) *and* the matching adapter crate identifier
  (`rstest_bdd_harness_tokio` or `rstest_bdd_harness_gpui`) appears in a
  non-root position. `custom::TokioHarness` has no evidence and is treated as
  a custom type (ADR-017).
- *Expansion boundary*: the single place in each macro where supplied paths
  are resolved and diagnostics are emitted: `try_scenario` in
  `crates/rstest-bdd-macros/src/macros/scenario/mod.rs` for `#[scenario]`, and
  `scenarios` in `crates/rstest-bdd-macros/src/macros/scenarios/mod.rs` for
  `scenarios!`.
- *Stable channel* / *nightly channel*: stable procedural macros cannot emit
  warnings, so the macros emit a `const _` block that uses a
  `#[deprecated(note = "...")]` unit struct, which rustc reports as a
  `deprecated` warning carrying the message. Nightly builds (detected by
  `crates/rstest-bdd-macros/build.rs`, which sets `cfg(rstest_bdd_nightly)`)
  call `proc_macro::Diagnostic::spanned(..., Level::Warning, ...)` instead.
  `proc_macro::Diagnostic` remains unstable (rust-lang/rust#54140, checked
  2026-10-15).
- *Trybuild*: a test crate that compiles fixture programs. A `pass` fixture
  must compile and run; a `compile_fail` fixture must fail, and its stderr
  must match a checked-in `.stderr` file. Trybuild never inspects warnings in
  `pass` fixtures (docs.rs/trybuild 1.0.122), so warning text is pinned by
  adding `#![deny(deprecated)]` to a `compile_fail` fixture, which turns the
  stable warning into an error. Nightly fixtures end in
  `compile_error!("nightly warning snapshot sentinel");` so that stderr
  contains the native warning.

### The diagnostic taxonomy

The plan introduces or extends exactly four diagnostics, grouped into the
roadmap's three classes.

1. `NON-CANONICAL` (warning, both channels). It fires when a supplied
   `harness` or `attributes` path has evidence but is not recognized. It
   extends today's ADR-017 rule, which only accepts the crate identifier as
   the *penultimate* segment, to any non-root position, so
   `a::rstest_bdd_harness_tokio::b::TokioHarness` now qualifies. It fires at
   most once per slot. The message names the canonical path for the adapter
   type, states the consequences (base-API fallback, plus the framework
   attribute that was not applied when the slot determines attributes), and
   offers the dev-dependency workaround, labelled as not silencing the warning.
2. `MISSING-EXECUTOR` (error, toolchain-independent). For `#[scenario]` only,
   it fires when the function is `async`, no `harness` is supplied, an
   `attributes` path is supplied, the resolved policy emits no executor
   attribute, and the function carries no user attribute whose path ends in
   `test`. Each of those configurations already fails today with
   `rstest`'s message. The new error is spanned at the `attributes` path and
   suggests: remove `attributes = ...` (the async default is Tokio); use
   `attributes = rstest_bdd_harness_tokio::TokioAttributePolicy`; add an
   executor attribute such as `#[tokio::test]`; or, for a third-party policy,
   note that the macros cannot evaluate it.
3. `AMBIGUOUS-FRAMEWORK` (warning, both channels). It fires when the harness
   family and the attribute-policy family are two *different framework*
   families (Tokio versus GPUI). An explicit downgrade to
   `rstest_bdd_harness::DefaultAttributePolicy` is a documented override and
   stays silent. It is spanned at the `attributes` path, which wins under
   ADR-008, and suggests removing `attributes = ...` or using the policy that
   matches the harness.
4. `AMBIGUOUS-RUNTIME-ALIAS` (warning, both channels). For `scenarios!`
   only, it fires when `runtime = "tokio-current-thread"` is combined with
   `harness = ...` and/or `attributes = ...`. It is spanned at the `runtime`
   string literal and suggests removing `runtime = ...`. On nightly it
   replaces today's native-only redundancy warning, so exactly one warning is
   emitted. The alias-alone deprecation warning is unchanged.

Explicitly out of scope (recorded so the reviewer can challenge it):

- renamed imports such as `use rstest_bdd_harness_tokio::TokioHarness as
  MyHarness`, which leave no evidence in the token stream (documented
  limitation, already stated in design doc §2.7.3);
- a single-segment `TokioHarness` that is actually a local type while the
  adapter crate is resolvable (the macros cannot distinguish the two);
- warnings for unevaluated third-party attribute policies (roadmap 12.2.3);
- user-written `#[tokio::test]` or `#[gpui::test]` that conflicts with the
  resolved policy (candidate follow-up; deduplication rules in
  `codegen/scenario/test_attrs.rs` already cover the same-framework case);
- passing a policy type as `harness =` or vice versa (already rejected by
  rustc's E0277 through the trait-bound assertions).

### Files you will touch

- `crates/rstest-bdd-macros/src/codegen/adapter_fallback.rs` (306 lines):
  today's fallback decision (`fallback_candidate`), message, stable tokens,
  nightly emission, and `SharedAdapterResolutions`. Milestone 1 moves its
  contents into the new module and deletes the file.
- `crates/rstest-bdd-macros/src/codegen/mod.rs` (400 lines, at the cap):
  `CrateSpec`, the `TOKIO_HARNESS`/`GPUI_HARNESS` specs, `resolve_harness_api`,
  `first_party_adapter_attribute_hint`, `path_root_matches_crate`. Child
  modules may read its private items, so new code goes into children.
- `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs` (238 lines):
  `TestAttrPolicy`, `resolve_attribute_policy` (ADR-008 precedence), and
  `generate_test_attrs_with_boundary`.
- `crates/rstest-bdd-macros/src/codegen/scenario/mod.rs` (346 lines):
  `ScenarioConfig`, `reject_async_harness`.
- `crates/rstest-bdd-macros/src/macros/scenario/mod.rs` (260 lines): the
  `#[scenario]` boundary (`try_scenario`), which already calls
  `SharedAdapterResolutions::resolve` and `emit_diagnostics`.
- `crates/rstest-bdd-macros/src/macros/scenarios/mod.rs` (368 lines): the
  `scenarios!` boundary and `emit_runtime_deprecation_warning`.
- `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/mod.rs`
  (219 lines): `ScenariosArgs`; capture the `runtime` literal's span.
- `crates/rstest-bdd-harness-tokio/tests/macro_compile.rs` and
  `crates/rstest-bdd-harness-tokio/tests/fixtures_macros/`: the trybuild
  suite that already hosts the stable and nightly fallback fixtures.
- `crates/rstest-bdd-harness-gpui/tests/macro_compile.rs` and its
  `fixtures_macros/`: GPUI fixtures. They run under `--all-features`
  (`native-gpui-tests`), which `make test` passes through `CARGO_FLAGS`.

New module layout (all under `crates/rstest-bdd-macros/src/codegen/`):

```plaintext
policy_diagnostics/
  mod.rs        module docs, re-exports, SharedPolicyDiagnostics (boundary type)
  model.rs      pure kernel: SlotClass, PolicyInputs, Finding, diagnose()
  classify.rs   syn::Path -> SlotClass (concretization)
  message.rs    finding -> message text, fix text built from rstest-bdd-policy constants
  emit.rs       stable deprecated-item tokens and nightly Diagnostic emission
  tests/        exhaustive model checks, classifier properties, message snapshots
```

### Skills and documentation to load

Before implementation, load `rust-router` and then, as each facet arises,
`rust-unit-testing` (rstest tables, `googletest`, `pretty_assertions`,
`insta`), `proptest`, `rust-verification` (method selection), and
`rust-unused-code` (cfg-gated channel functions). Use `codegraph-mcp`
(`codegraph_get_callers`, `codegraph_analyze_impact`) before moving
`adapter_fallback` items. Use `nextest` for focused runs. Read:

- `docs/rstest-bdd-design.md` §§2.7.3-2.7.6.4 (harness integration, ADR-008
  codegen, first-party targets, v0.6.1 helpers);
- `docs/adr-008-harness-led-attribute-policy-defaults.md` (precedence,
  validity of harness-only and attributes-only);
- `docs/adr-017-toolchain-aware-adapter-fallback-diagnostics.md` (channels,
  cardinality, ownership);
- `docs/adr-004-policy-crate.md` (`TestAttributeHint` as single source);
- `docs/developers-guide.md` "Adapter fallback diagnostics" and "Staging
  fixtures for trybuild tests";
- `docs/users-guide.md` "Harness adapter and attribute policy" and
  "First-party adapter fallback diagnostics";
- `docs/rust-testing-with-rstest-fixtures.md` (fixture and table idioms);
- `docs/rust-doctest-dry-guide.md` (doctest style for new rustdoc examples);
- `docs/complexity-antipatterns-and-refactoring-strategies.md` (keep
  `try_scenario` and `scenarios` from growing; extract helpers instead);
- `docs/gherkin-syntax.md` (feature-file syntax for Milestone 5);
- `docs/documentation-style-guide.md` (ADR template, en-GB-oxendict).

`docs/rstest-bdd-language-server-design.md` was checked: the language server
does not read `harness` or `attributes` paths, so it is unaffected.

## Conformance basis

Upstream artefacts, all at repository revision `8737d88f`:

- Roadmap item 11.2.3 (`docs/roadmap.md`, §11.2), including its finish line.
  Phase 11 preamble: semver-compatible additive hardening.
- Design document `docs/rstest-bdd-design.md` §2.7.3 (macro integration,
  attribute-policy trust model, fallback diagnostic), §2.7.4 (ADR-008 codegen),
  §2.7.5 (first-party targets), §2.7.6.3-§2.7.6.4 (beta quick wins, v0.6.1
  helpers).
- ADR-004 (policy crate), ADR-005a (adapter crates), ADR-007 (context
  injection), ADR-008 (harness-led defaults), ADR-017 (fallback diagnostics).
- Governing standards: `AGENTS.md`, `docs/documentation-style-guide.md`.
- No Terms of Reference document exists for this work; none is invented.

Traced requirements (identifiers are local to this plan):

- `REQ-NC`: diagnose non-canonical harness and policy paths.
- `REQ-MISS`: diagnose missing attribute-policy annotations.
- `REQ-AMB`: diagnose ambiguous attribute-policy annotations.
- `REQ-FIX`: each diagnostic carries actionable fix text.
- `REQ-TRY`: trybuild tests cover the three cases and assert the fix text.
- `REQ-COMPAT`: Phase 11 semver compatibility (C1-C3).

Trace chain:

```plaintext
roadmap 11.2.3 -> REQ-NC   -> design §2.7.3 fallback, ADR-017 -> EP-M1, EP-M2 -> fixtures *_noncanonical_*, LEM-EVIDENCE
roadmap 11.2.3 -> REQ-MISS -> design §2.7.3 async rules        -> EP-M4        -> fixtures *_missing_executor_*, LEM-ERR-REPLACES
roadmap 11.2.3 -> REQ-AMB  -> ADR-008 precedence                -> EP-M3        -> fixtures *_ambiguous_*, LEM-SILENCE
roadmap 11.2.3 -> REQ-FIX  -> ADR-024 (new)                     -> EP-M2..EP-M4 -> LEM-FIX-CONVERGES, LEM-FIX-CANONICAL, insta snapshots
roadmap 11.2.3 -> REQ-TRY  -> developers' guide trybuild idiom  -> EP-M2..EP-M4 -> trybuild stable + nightly pairs
Phase 11       -> REQ-COMPAT -> C1-C3                           -> EP-M1..EP-M5 -> LEM-SILENCE, unchanged pass fixtures, BDD suite
```

## Verification plan

### Abstract model (specification, independent of `syn`)

Each supplied slot (`harness`, `attributes`) is classified into a finite
`SlotClass`:

```plaintext
SlotClass ::= Absent
            | Recognized(f)   f in {Base, Tokio, Gpui}   (Base = rstest_bdd_harness::{StdHarness, DefaultAttributePolicy})
            | Evidenced(f)    f in {Tokio, Gpui}         (non-canonical, with evidence)
            | Foreign                                    (anything else, including custom::TokioHarness)
```

`family(Recognized(f)) = family(Evidenced(f)) = f`, and `family` is undefined
for `Absent` and `Foreign`. Families are represented by
`TestAttributeHint` (`Base = RstestOnly`), not by a new enum (ADR-004).

A configuration is
`PolicyInputs { harness: SlotClass, attributes: SlotClass, runtime_alias:
bool, is_async: bool, has_user_test_attr: bool }`. `#[scenario]` always has
`runtime_alias = false`; `scenarios!` always has `is_async = false` and
`has_user_test_attr = false`. The model domain is therefore all
`7 × 7 × 2 × 2 × 2 = 392` combinations, restricted per macro by those two
rules.

The specification of today's resolution (ADR-008) is:

```plaintext
effective_hint(c) =
  if c.attributes = Recognized(f)          then f
  if c.attributes in {Evidenced(_), Foreign} then Base
  if c.harness    = Recognized(f)          then f
  if c.runtime_alias or c.is_async         then Tokio
  otherwise                                     Base
```

`implied_harness_family(c)` is `family(c.harness)` if defined, otherwise
`Tokio` when `c.runtime_alias` holds and `c.harness = Absent`.

The diagnostics are a set `diagnose(c) ⊆ {NC(harness), NC(attributes),
MISSING, AMB-FRAMEWORK, AMB-ALIAS}`:

```plaintext
NC(s)          in diagnose(c)  iff  c.s = Evidenced(_)
MISSING        in diagnose(c)  iff  c.is_async and c.harness = Absent and c.attributes != Absent
                                     and effective_hint(c) = Base and not c.has_user_test_attr
AMB-FRAMEWORK  in diagnose(c)  iff  implied_harness_family(c) and family(c.attributes) are both defined,
                                     both in {Tokio, Gpui}, and differ
AMB-ALIAS      in diagnose(c)  iff  c.runtime_alias and (c.harness != Absent or c.attributes != Absent)
```

Each finding carries a list of suggested *fixes*, each modelled as a
transformation `PolicyInputs -> PolicyInputs` (for example "replace
`attributes` with `Recognized(Tokio)`", "set `attributes = Absent`", "set
`has_user_test_attr = true`", "set `runtime_alias = false` and, if
`harness = Absent`, `harness = Recognized(Tokio)`"), and a list of
*workarounds* that are not claimed to silence anything.

### Axioms

- `AX-RSTEST-ASYNC`: `rstest` 0.26 rejects an `async` test function that
  carries no attribute whose path ends in `test` (message: "async test
  requires either explicit `test_attr` or implicit (attribute path ends with
  `test`)"). Pinned today by
  `crates/rstest-bdd-harness-tokio/tests/fixtures_macros/
  scenario_default_policy_async_no_harness_rejected.stderr`, and guarded in
  this plan by a canary fixture that uses `rstest` directly (Risk R5).
- `AX-TRYBUILD`: trybuild compares `compile_fail` stderr against `.stderr`
  files and ignores warnings in `pass` fixtures (docs.rs/trybuild 1.0.122).
- `AX-DEPRECATED`: rustc reports use of a `#[deprecated(note = ...)]` item as
  the `deprecated` lint carrying the note, warn-by-default and escalated by
  `deny(deprecated)` or `-D warnings`.
- `AX-SYN-PATH`: `syn::Path::segments` excludes a leading `::`.
- `AX-CFG-TEST`: under `cfg(test)`, adapter crates are not resolvable through
  `proc_macro_crate`, and `fallback_crate_is_resolvable` is forced to `true`
  (existing code). Unit-test witnesses therefore use multi-segment paths.

### Obligations

- Obligation `LEM-SILENCE` (backward compatibility): for every `c` in the
  domain with no `Evidenced` slot, no framework conflict, and no alias
  combination, `diagnose(c)` contains no warning. In particular, `Absent`
  slots, harness-only `Foreign`, and `(Recognized(Tokio|Gpui),
  Recognized(Base))` downgrades are silent.
  Method: exhaustive enumeration of all 392 states in a `#[test]`,
  checking the property (not recomputing `diagnose`).
  Rationale: the domain is finite, so enumeration is a complete decision
  procedure; Verus would prove the same statement at far higher cost.
  Artefact: `policy_diagnostics/tests/exhaustive.rs`.
  Evidence: `cargo nextest run -p rstest-bdd-macros policy_diagnostics`.
  Non-vacuity: the test asserts the enumerator yields exactly 392 distinct
  states, and records (and asserts non-zero) the number that satisfy the
  antecedent; a
  match-without-wildcard guard function fails to compile if a `SlotClass`
  variant is added without updating the enumerator. Seeded fault: treating
  `Base` as a framework family in `AMB-FRAMEWORK` must fail on the
  `(Recognized(Tokio), Recognized(Base))` state.
- Obligation `LEM-ERR-REPLACES` (C2): `MISSING ∈ diagnose(c)` implies
  `effective_hint(c) = Base ∧ c.is_async ∧ ¬c.has_user_test_attr`, which by
  `AX-RSTEST-ASYNC` means today's expansion is already rejected.
  Method: exhaustive enumeration, plus a trybuild check that each
  missing-executor fixture's pre-change stderr (recorded in Milestone 0) is
  the `rstest` error. Rationale: the model part is finite; the link to real
  rejection rests on the axiom, which is checked against the real `rstest`.
  Artefact: `exhaustive.rs`; fixtures listed under EP-M4; canary fixture.
  Non-vacuity: at least three states (`Recognized(Base)`, `Foreign`,
  `Evidenced(Tokio)` attributes) satisfy the antecedent. Seeded fault:
  dropping the `¬has_user_test_attr` conjunct must fail the state with a user
  `#[tokio::test]`.
- Obligation `LEM-DISJOINT-HARNESS-ASYNC`: `MISSING` never fires together
  with the existing `reject_async_harness` error (which requires
  `harness ≠ Absent`). Method: exhaustive enumeration. Seeded fault: dropping
  the `harness = Absent` conjunct.
- Obligation `LEM-CARDINALITY` (C5): `diagnose(c)` contains each finding at
  most once, so a boundary emits at most five diagnostics, and `scenarios!`
  emits the same set however many scenarios it generates. Method: the model
  returns a fixed-shape struct (one `Option` per finding), so duplication is
  unrepresentable; boundary behaviour is checked by extending
  `codegen/scenario/tests/adapter_sharing.rs` marker counts to the new marker
  structs with a two-scenario feature. Non-vacuity: a state that reaches four
  findings at once (`NC(harness)`, `NC(attributes)`, `AMB-FRAMEWORK`,
  `AMB-ALIAS` for `scenarios!`) is asserted to emit exactly four markers.
- Obligation `LEM-FIX-CONVERGES` (`REQ-FIX`): for every state `c`, every
  finding `d ∈ diagnose(c)`, and every *fix* `φ` suggested for `d`:
  `d ∉ diagnose(φ(c))` and `diagnose(φ(c)) ⊆ diagnose(c) \ {d}`. Fixes never
  introduce a new diagnostic. Method: exhaustive enumeration over states ×
  findings × fixes. Rationale: working this lemma by hand already falsified
  two design candidates during drafting (D5, D6), which is evidence that it
  constrains the design rather than restating it. Non-vacuity: every finding kind has at
  least one state with at least one fix; the test counts the
  `(state, finding, fix)` triples checked and asserts the count is greater
  than zero per finding kind. Seeded fault: computing `AMB-FRAMEWORK` on
  `Recognized` families only (the rejected D6 candidate) must fail.
- Obligation `LEM-FIX-CANONICAL`: every path that a message suggests parses
  as a `syn::Path` that `classify` maps to `Recognized(f)` for the intended
  family. Method: a `rstest` table over every rendered message, extracting
  the backticked paths and classifying them. Non-vacuity: each message kind
  contributes at least one path. Seeded fault: suggesting
  `rstest_bdd_harness_tokio::TokioHarness` for a GPUI finding.
- Obligation `LEM-EVIDENCE` (concretization soundness, unbounded domain):
  for any `syn::Path` `p` in a slot, `classify(p) = Recognized(f)` exactly
  when today's recognition (`resolve_test_attribute_hint_for_*_path` or
  `first_party_adapter_attribute_hint`) returns hint `f`, so recognition is
  unchanged; `classify(p) = Evidenced(f)` implies that the last segment is an
  adapter name of `f`, that `f`'s crate identifier occurs at a non-root
  position, and that `p` is not recognized; `classify` is invariant under a
  leading `::`; a path containing no first-party crate identifier is never
  `Evidenced`. Method: `proptest` over generated segment sequences (length
  1-6, drawn from first-party crate identifiers, adapter type names, and
  arbitrary identifiers), plus named `rstest` cases for `custom::TokioHarness`,
  `alias::rstest_bdd_harness_tokio::TokioHarness`,
  `a::rstest_bdd_harness_gpui::b::GpuiHarness`, and
  `rstest_bdd_harness_tokio::prelude::TokioHarness` (recognized, because its
  root is the crate). Rationale: the path space is unbounded but the
  properties are membership predicates without recursion or arithmetic;
  property testing with targeted generators is proportionate. Artefact:
  `policy_diagnostics/tests/classify_props.rs`. Non-vacuity: `prop_assume!`
  is not used; generator classification counters assert that each
  `SlotClass` constructor is produced in at least 5% of 256 cases. Seeded
  fault: matching only the penultimate segment (today's ADR-017 rule) must
  fail the deep-nesting case.
- Obligation `REP-SAME-RESOLVER` (representation relationship): the model's
  `effective_hint` agrees with the real `resolve_attribute_policy` for one
  concrete witness path per `SlotClass` across all `(harness, attributes,
  is_async)` combinations, and the `MISSING` decision in code consumes the
  result of `resolve_attribute_policy` rather than recomputing precedence.
  Method: an exhaustive witness table in a unit test, plus code review of the
  call site. Non-vacuity: `classify(witness(k)) = k` for every class `k`.
  Seeded fault: changing `effective_hint` so that `Evidenced` attributes
  inherit their family must fail.
- Obligation `LEM-CHANNEL-EXCLUSIVE` (C5): on nightly, no deprecated-item
  tokens are emitted; on stable, no native diagnostic is emitted; the
  alias-plus-selector case no longer also calls `emit_warning`. Method:
  existing `runtime_split.rs`-style unit tests extended to the new markers,
  and paired stable/nightly trybuild fixtures.
- Obligation `OBS-FIX-TEXT` (`REQ-TRY`): each diagnostic's full text,
  including fix text, is pinned in stable trybuild `.stderr` files, with
  nightly twins for warnings, and the rendered messages for every
  `(finding, family, slot)` variant are pinned in one `insta` snapshot set.
  Method: trybuild and `insta`. Rationale: the output is multivariant text
  whose consistency matters (AGENTS.md: snapshot tests).

### Methods deliberately not used

Verus is not warranted. The decision kernel's domain is finite (392 states),
so exhaustive enumeration against stated lemmas is already a complete proof
of every kernel property, and it runs in the ordinary test suite. The only
unbounded domain, path classification, consists of shallow membership
predicates that `proptest` and named cases cover proportionately. The
repository has no Verus toolchain, and introducing one would breach C10 for no
gain in assurance. Kani is not warranted either: there is no `unsafe` code, no
arithmetic, and no bounded state machine beyond what enumeration covers.
Revisit both if roadmap 12.2.3 introduces an open, user-extensible policy
registry, because the domain would then become unbounded.

If `cargo mutants` is available locally, run it on
`policy_diagnostics/model.rs` and `classify.rs` as supplementary evidence that
the suite rejects plausible defects; record surviving mutants in `Surprises &
discoveries`. This is evidence, not a gate.

## Plan of work

### Stage A: understand and propose (no production code)

Milestone 0 below. Record baselines, run the fixture-host spike, and confirm
the nightly CI question (Risk R4).

### Stage B: red tests

Each feature milestone (EP-M2 to EP-M4) begins by adding the trybuild
fixtures and unit tests for its finding. Run them and observe the expected
failure before writing production code. For trybuild this means the new
`compile_fail` fixture either compiles (no diagnostic yet) or produces stderr
that differs from the checked-in expectation. For model tests, `diagnose`
initially returns no finding of the new kind, so the named witness cases fail.

### Stage C: green implementation

Implement the smallest change in `policy_diagnostics` that makes the red
tests pass, then wire it into the owning boundary.

### Stage D: refactor, documentation, wider validation

Tidy helpers, keep functions small (AGENTS.md), update documentation
(EP-M6), and run the full gates.

## Milestones and plateaus

### EP-M0: baseline and fixture-host spike (prototype)

Outcome: the baseline is recorded and fixture placement is decided; no
production code changes. Rebase onto `origin/main`. Record the `make test`
pass counts and the current stderr of
`scenario_default_policy_async_no_harness_rejected`. Check
`.github/workflows/ci.yml` for a nightly lane.

Prototype: in a scratch branch, add one throwaway `compile_fail` fixture to
the GPUI suite combining `harness = rstest_bdd_harness_gpui::GpuiHarness` and
`attributes = rstest_bdd_harness_tokio::TokioAttributePolicy`, and one to the
Tokio suite with the reverse pairing. Observe which suite can resolve both
crates and which extra rustc errors appear. Keep the winner's shape; delete
the prototype. If neither suite works without a manifest change, add
`rstest-bdd-harness-tokio` as a workspace dev-dependency of
`rstest-bdd-harness-gpui` (the single change C10 permits).

- Requirements: none discharged; de-risks `REQ-AMB` and `REQ-TRY`.
- Acceptance: baseline transcripts are recorded in `Artefacts and notes`; the
  host decision is recorded in `Decision log`.
- Conformance check:
  - no production code changed;
  - design unchanged;
  - Risks R4 and R6 are resolved or re-rated;
  - at most one permitted manifest change;
  - no persisted-format or trust-boundary change;
  - trace links are current.
- Recovery: delete the scratch branch.
- Remaining gaps: everything else.
- Compatibility decision: none.

### EP-M1: extract `policy_diagnostics` (behaviour-preserving refactor)

Outcome: `codegen/adapter_fallback.rs` is replaced by
`codegen/policy_diagnostics/` with the pure model, the classifier, and the
dual-channel emitter. The only finding implemented is today's fallback rule,
with today's message. All existing tests, fixtures, and `.stderr` files are
unchanged.

Work: create the module tree. Move `AdapterFallback`, the message function,
the stable token builder, the nightly emitter, and `SharedAdapterResolutions`
into it. Rename `SharedAdapterResolutions` to `SharedPolicyDiagnostics` and
update its two boundary call sites. Express today's rule as
`classify(...) = Evidenced(f)` restricted to the penultimate position, so the
classifier exists with its proptest scaffold. Add the exhaustive enumerator,
its count test, and the variant guard. Move any `codegen/mod.rs` helper that
the module needs only if this keeps `codegen/mod.rs` at or below 400 lines.
This is a separate atomic refactor commit (AGENTS.md).

- Requirements: advances `REQ-NC` (structure only).
- Acceptance: `make check-fmt`, `make lint`, and `make test` pass; `git diff`
  shows no `.stderr`, macrotest snapshot, or `pass` fixture change.
- Conformance check:
  - no requirement newly claimed;
  - ADR-017 structure preserved (pure resolve, boundary emit);
  - assumptions unchanged;
  - no public interface change (all items `pub(crate)` or narrower);
  - no trust-boundary change;
  - trace links updated for the module rename.
- Recovery: `git revert` the single refactor commit.
- Remaining gaps: new findings.
- Compatibility decision: none (crate-private code).

### EP-M2: non-canonical path diagnostic

Outcome: `NON-CANONICAL` fires for any non-root occurrence of the adapter
crate identifier, and its message names the canonical path, states the
consequence, and labels the workaround.

Red: add the `LEM-EVIDENCE` deep-nesting cases and the `LEM-FIX-CANONICAL`
table; add trybuild fixtures in
`crates/rstest-bdd-harness-tokio/tests/fixtures_macros/`:
`scenario_harness_noncanonical_nested.rs`,
`scenario_attributes_noncanonical_nested.rs` (both stable, under
`#![deny(deprecated)]`), and `nightly_noncanonical_nested_warning.rs`, plus a
GPUI twin `scenario_attributes_gpui_noncanonical_nested.rs` in the GPUI suite.
Register them in `macro_compile.rs` beside the existing `rustversion` helpers.

Green: widen `classify`; extend `message.rs` with the canonical-path and
consequence clauses; regenerate the existing `*_alias_fallback_warning.stderr`
files with `TRYBUILD=overwrite` and review each diff line by line (decision
D5).

- Requirements: `REQ-NC`, `REQ-FIX`, `REQ-TRY` (non-canonical part).
- Acceptance: the new fixtures pass; `scenario_custom_tokio_harness_name.rs`
  (custom type, `deny(deprecated)`, `pass`) still passes; the classifier
  properties hold.
- Conformance check:
  - REQ-NC and the non-canonical part of REQ-TRY are satisfied;
  - ADR-017 cardinality and custom-type silence hold;
  - assumptions valid;
  - no public interface change;
  - no trust-boundary change;
  - trace links current.
- Recovery: revert the milestone commits; EP-M1 remains a coherent plateau.
- Remaining gaps: ambiguous and missing classes.
- Compatibility decision: none.

### EP-M3: ambiguous selector diagnostics

Outcome: `AMB-FRAMEWORK` and `AMB-ALIAS` fire through both channels; the
nightly alias-plus-harness warning is emitted exactly once.

Red: model witness cases; `LEM-FIX-CONVERGES` covering D6;
`adapter_sharing.rs` four-marker case; trybuild fixtures (host from EP-M0):
`scenario_ambiguous_framework.rs`, `scenarios_ambiguous_framework.rs`,
`scenarios_ambiguous_runtime_alias_harness.rs`,
`scenarios_ambiguous_runtime_alias_attributes.rs`, and nightly twins
`nightly_ambiguous_framework_warning.rs` and
`nightly_ambiguous_runtime_alias_warning.rs`. Also add a `pass` fixture under
`#![deny(deprecated)]`, `scenario_override_default_policy_silent.rs`, that pins
silence for the `TokioHarness` plus `DefaultAttributePolicy` downgrade.

Green: capture the `runtime` literal span in `ScenariosArgs` (one field,
populated in `parse_runtime_mode`'s caller); compute the two findings in the
model; route `emit_runtime_deprecation_warning`'s redundancy branch through
the new finding so it no longer calls `emit_warning` in that case. Update the
nightly fixture `nightly_runtime_deprecation_warning.stderr` if its text
moves.

- Requirements: `REQ-AMB`, `REQ-FIX`, `REQ-TRY` (ambiguous part).
- Acceptance: the fixtures pass; existing GPUI and Tokio
  `*_override_default.rs` fixtures are unchanged; nightly emits one alias
  warning, checked locally with the nightly command in `Concrete steps`.
- Conformance check:
  - REQ-AMB satisfied;
  - ADR-008 precedence unchanged (explicit `attributes` still wins);
  - assumptions valid;
  - no macro argument added (a span is captured, not a new key);
  - no trust-boundary change;
  - trace links current.
- Recovery: revert; EP-M2 remains coherent.
- Remaining gaps: missing class.
- Compatibility decision: none.

### EP-M4: missing executor attribute diagnostic

Outcome: `MISSING-EXECUTOR` replaces `rstest`'s generic async error with an
`rstest-bdd` error that carries the fix.

Red: model witnesses; `LEM-ERR-REPLACES`; trybuild fixtures in the Tokio
suite: the existing `scenario_default_policy_async_no_harness_rejected.rs`
(its `.stderr` is expected to change), new
`scenario_foreign_policy_async_missing_executor.rs`,
`scenario_noncanonical_policy_async_missing_executor.rs`, and a `pass` fixture
`scenario_foreign_policy_async_user_tokio_test.rs` (user-written
`#[tokio::test]` stays accepted). Add the canary
`rstest_async_requires_test_attr_canary.rs`, which uses `rstest` alone.

Green: add `reject_async_without_executor` beside `reject_async_harness` in
`codegen/scenario/mod.rs`. It takes the `ResolvedAttributePolicy` produced by
`resolve_attribute_policy` (which becomes `pub(super)` if needed) and the
function's attributes; on a finding it returns a `syn::Error` spanned at the
`attributes` path. If `codegen/scenario/mod.rs` would exceed 380 lines, put the
helper in a sibling `codegen/scenario/async_guards.rs` with the existing
`reject_async_harness`.

- Requirements: `REQ-MISS`, `REQ-FIX`, `REQ-TRY` (missing part).
- Acceptance: the fixtures pass; the canary passes; every pre-existing
  `async` `pass` fixture is unchanged.
- Conformance check:
  - REQ-MISS satisfied;
  - C2 holds (`LEM-ERR-REPLACES`);
  - `AX-RSTEST-ASYNC` re-checked against the pinned `rstest` version;
  - no public interface change;
  - no trust-boundary change;
  - trace links current.
- Recovery: revert; EP-M3 remains coherent.
- Remaining gaps: behavioural suite and documentation.
- Compatibility decision: none.

### EP-M5: behavioural silence and fix scenarios

Outcome: an `rstest-bdd` feature demonstrates that the configurations the
diagnostics recommend compile silently and behave as promised.

Add `crates/rstest-bdd-harness-tokio/tests/features/policy_diagnostics.feature`
and `crates/rstest-bdd-harness-tokio/tests/policy_diagnostics_behaviour.rs`.
The test file starts with `#![deny(deprecated)]`, so a stray diagnostic
breaks compilation (the end-to-end silence check for `LEM-SILENCE`).

```gherkin
Feature: Attribute-policy diagnostics recommend working configurations

  Scenario: The canonical Tokio harness path provides a Tokio runtime
    Given a scenario bound with the canonical Tokio harness path
    When the scenario body checks for a Tokio runtime
    Then a Tokio runtime handle is available

  Scenario: Removing a conflicting attributes override restores the harness default
    Given a scenario bound with the Tokio harness and no attribute policy
    When the scenario body checks for a Tokio runtime
    Then a Tokio runtime handle is available

  Scenario: An explicit default-policy downgrade stays silent
    Given a scenario bound with the Tokio harness and the default attribute policy
    When the scenario body checks for a Tokio runtime
    Then a Tokio runtime handle is available

  Scenario: A third-party harness without an attribute policy stays silent
    Given a scenario bound with a local harness and no attribute policy
    When the scenario body runs
    Then the local harness recorded one run
```

Each scenario is bound with its own `#[scenario(..., harness = ...,
attributes = ...)]` configuration, so the bindings are the configurations
under test. The scenarios fail before EP-M5 because the feature file and
bindings do not exist, and pass afterwards.

- Requirements: `REQ-COMPAT`, `REQ-FIX` (end to end).
- Acceptance: `cargo nextest run -p rstest-bdd-harness-tokio
  policy_diagnostics_behaviour` passes four scenarios.
- Conformance check: one line each, as for EP-M2.
- Recovery: revert; EP-M4 remains coherent.
- Remaining gaps: documentation.
- Compatibility decision: none.

### EP-M6: documentation

Outcome: documentation matches the implementation.

- Add `docs/adr-024-evidence-based-harness-and-policy-diagnostics.md`
  (status Accepted on approval of the implementation), covering the taxonomy,
  the evidence principle, the error-replaces-error rule, the decision not to
  warn on harness-only third-party usage, channel reuse, and the fix versus
  workaround distinction. Add an `Amended by ADR-024 (date)` banner and an
  `## Amendments` entry to ADR-017. Index ADR-024 in `docs/contents.md`.
- `docs/rstest-bdd-design.md` §2.7.3: replace the fallback paragraphs with a
  "Policy diagnostics" subsection summarizing the four diagnostics and
  referencing ADR-024; §2.7.6.4: list 11.2.3 as delivered.
- `docs/users-guide.md`: rename "First-party adapter fallback diagnostics" to
  "Harness and attribute-policy diagnostics"; document each diagnostic with a
  triggering example, the message, and the fix; state the renamed-import
  limitation and the `-D warnings` consequence.
- `docs/v0-6-0-migration-guide.md`: update the fallback-warning blockquote.
- `docs/developers-guide.md`: replace "Adapter fallback diagnostics" with
  "Policy diagnostics" describing module ownership (`policy_diagnostics` is
  the only emitter of harness and attribute-policy diagnostics; permitted
  call sites are the two expansion boundaries; findings compose through
  `SharedPolicyDiagnostics`), the recipe for adding a finding (model, fix
  transformations, message, emit marker, stable and nightly fixtures), and the
  previously undocumented `TRYBUILD=overwrite` refresh command.
- `docs/CHANGELOG.md` (Unreleased): one entry per diagnostic, flagging the
  `-D warnings` consequence.

- Requirements: AGENTS.md documentation duties; ADR traceability.
- Acceptance: `make markdownlint`, `make nixie`, and `make check-fmt` pass.
- Conformance check: one line each.
- Recovery: revert documentation commits.
- Remaining gaps: roadmap tick.
- Compatibility decision: none.

### EP-M7: completion

Mark roadmap 11.2.3 as done with a completion note naming this ExecPlan and
ADR-024. Run all gates. Complete `Outcomes & retrospective`, reconcile
discoveries against the conformance basis, and set the status to `COMPLETE`.

## Concrete steps

Run every command from the repository root. Pipe long output through `tee`
to `/tmp/$ACTION-rstest-bdd-$(git branch --show-current | tr / -).out`. Run
gates sequentially, never in parallel, and prefer delegating full gate runs to
the `scrutineer` agent.

Focused loops:

```bash
cargo nextest run -p rstest-bdd-macros policy_diagnostics 2>&1 | tee /tmp/unit-rstest-bdd-policy-diag.out
cargo nextest run -p rstest-bdd-harness-tokio --test macro_compile 2>&1 | tee /tmp/trybuild-rstest-bdd-tokio.out
cargo nextest run -p rstest-bdd-harness-gpui --all-features --test macro_compile 2>&1 | tee /tmp/trybuild-rstest-bdd-gpui.out
```

Refresh a `.stderr` snapshot after a deliberate message change, then review
the diff before staging:

```bash
TRYBUILD=overwrite cargo test -p rstest-bdd-harness-tokio --test macro_compile
git diff -- crates/rstest-bdd-harness-tokio/tests/fixtures_macros/
```

Nightly twins (local, Risk R4):

```bash
cargo +nightly-2026-08-16 test -p rstest-bdd-harness-tokio --test macro_compile 2>&1 | tee /tmp/trybuild-nightly-rstest-bdd-tokio.out
```

Milestone gates:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-rstest-bdd-11-2-3.out
make lint 2>&1 | tee /tmp/lint-rstest-bdd-11-2-3.out
make test 2>&1 | tee /tmp/test-rstest-bdd-11-2-3.out
make markdownlint 2>&1 | tee /tmp/markdownlint-rstest-bdd-11-2-3.out   # documentation milestones
make nixie 2>&1 | tee /tmp/nixie-rstest-bdd-11-2-3.out                 # documentation milestones
```

Expected: each command exits 0. `make test` reports the EP-M0 baseline count
plus the new tests, and zero failures.

## Validation and acceptance

Red-Green-Refactor evidence is recorded per milestone in `Artefacts and
notes`: the red command and its failing output (a trybuild mismatch or a
failing witness case), the green run, and the post-refactor gate run.

Acceptance criteria, phrased as behaviour:

1. Compiling `scenario_attributes_noncanonical_nested.rs` on stable produces
   an error starting
   ``use of deprecated unit struct `_::RstestBddFirstPartyAdapterFallback` ``
   whose note names `rstest_bdd_harness_tokio::TokioAttributePolicy` and
   states that `#[tokio::test(flavor = "current_thread")]` was not applied.
2. Compiling `scenario_default_policy_async_no_harness_rejected.rs` produces
   an `rstest-bdd` error spanned at the `attributes` path, suggesting removal
   of `attributes = ...` or `attributes =
   rstest_bdd_harness_tokio::TokioAttributePolicy`, and no `rstest` "async
   test requires" error.
3. Compiling `scenario_ambiguous_framework.rs` on stable produces one
   deprecated-item error carrying the ambiguous-framework message, spanned at
   the `attributes` path.
4. Compiling `scenarios_ambiguous_runtime_alias_harness.rs` on stable
   produces one error spanned at the `runtime` literal suggesting removal of
   `runtime = ...`; its nightly twin produces exactly one `warning:`.
5. Every pre-existing `pass` fixture, macrotest snapshot, and
   `harness_defaults` case is unchanged.
6. The behavioural feature passes its four scenarios under
   `#![deny(deprecated)]`.
7. `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
   `make nixie` pass.

Quality criteria:

- Tests: every gate above is green; new tests fail before and pass after.
- Verification: all obligations in `Verification plan` are discharged with
  their non-vacuity checks and seeded faults recorded.
- Lint: `make lint` (Clippy `-D warnings`, rustdoc, Whitaker, file-length
  check) is clean, with no new suppressions.
- Performance: not applicable (classification is constant-time per path).
- Security: not applicable (no new trust boundary).

## Idempotence and recovery

Every step is re-runnable. `TRYBUILD=overwrite` rewrites only `.stderr`
files, which `git diff` reviews and `git checkout -- <file>` restores. Each
milestone ends in a committed, gated plateau, so `git revert` of a
milestone's commits returns the tree to the previous plateau. Do not run two
gate runs in one worktree at the same time (developers' guide: trybuild `wip`
race). Stage files by path, not `git add -A`, because `make fmt` can reformat
unrelated documents.

## Artefacts and notes

To be filled during implementation: baseline counts (EP-M0), the pre-change
stderr of the async fixture, red and green transcripts per milestone, nightly
transcripts, seeded-fault results, and generator classification counts.

## Interfaces and dependencies

No public interface changes. All new items are `pub(crate)` or narrower in
`rstest-bdd-macros`.

In `crates/rstest-bdd-macros/src/codegen/policy_diagnostics/model.rs`:

```rust
use rstest_bdd_policy::TestAttributeHint;

/// Classification of one supplied `harness` or `attributes` slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlotClass {
    Absent,
    Recognized(TestAttributeHint),
    /// Non-canonical first-party path; never `RstestOnly`.
    Evidenced(TestAttributeHint),
    Foreign,
}

/// Abstract inputs to the diagnostic decision for one expansion boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PolicyInputs {
    pub(crate) harness: SlotClass,
    pub(crate) attributes: SlotClass,
    pub(crate) runtime_alias: bool,
    pub(crate) is_async: bool,
    pub(crate) has_user_test_attr: bool,
}

/// At most one of each finding; duplication is unrepresentable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PolicyFindings {
    pub(crate) non_canonical_harness: Option<TestAttributeHint>,
    pub(crate) non_canonical_attributes: Option<TestAttributeHint>,
    pub(crate) missing_executor: bool,
    pub(crate) ambiguous_framework: Option<(TestAttributeHint, TestAttributeHint)>,
    pub(crate) ambiguous_runtime_alias: bool,
}

/// Pure decision; no `syn`, no emission.
pub(crate) fn diagnose(inputs: PolicyInputs) -> PolicyFindings;
```

In `classify.rs`:

```rust
pub(crate) fn classify_harness_path(path: &syn::Path) -> SlotClass;
pub(crate) fn classify_policy_path(path: &syn::Path) -> SlotClass;
```

In `mod.rs`, `SharedPolicyDiagnostics` replaces `SharedAdapterResolutions`.
It keeps the `resolve` (pure) and `emit_diagnostics` (boundary-only) split,
and gains the classified slots, the alias flag, spans for each slot and for
the `runtime` literal, and the `PolicyFindings`.

The final field and function names may be refined during EP-M1 if the
refactor exposes a better fit; any change must keep `diagnose` pure and free
of `syn` types, and must be recorded in `Decision log`.

Dependencies: `rstest-bdd-policy` (existing, public constants only),
`syn`, `quote`, `proc-macro2` (existing). Test-only: `rstest`, `proptest`,
`googletest`, `pretty_assertions`, `insta` (all existing dev-dependencies of
`rstest-bdd-macros`), and `trybuild`, `rustversion` (existing in the harness
crates).

## Revision notes

- 2026-10-15: initial draft, written after four-agent reconnaissance and
  prior-art research. Awaiting community-of-experts review and user approval.
