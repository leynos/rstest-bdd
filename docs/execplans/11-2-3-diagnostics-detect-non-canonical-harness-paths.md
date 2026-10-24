# Diagnose non-canonical harness paths and missing or ambiguous attribute policies (roadmap 11.2.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (revised after community-of-experts review)

Approval: not yet approved. Implementation must not begin until the user
explicitly approves this plan.

## Purpose / big picture

`#[scenario]` and `scenarios!` let a test author choose a harness adapter
(`harness = ...`) and an attribute policy (`attributes = ...`). Procedural
macros cannot evaluate trait methods, so the macros recognize first-party
adapters by the *spelling* of the supplied path. When that spelling is not the
one the macros expect, or when two selectors disagree, the generated test
quietly changes shape. `#[gpui::test]` disappears, an explicit override
silently contradicts the harness, or the build fails with an error that
mentions neither the cause nor the fix.

After this change, a developer who writes one of those configurations sees a
targeted diagnostic, spanned at the offending argument and prefixed with a
stable code such as `rstest-bdd[W002]`. The diagnostic names the problem, the
consequence that actually occurred, and a fix that removes it: the canonical
path to write, the `attributes = ...` argument to add, or the argument to
remove. Every configuration that compiles today and is unambiguous stays
silent, and no program that compiles today stops compiling.

Observable success (each is pinned by a trybuild fixture):

1. Missing policy. `harness = reexports::rstest_bdd_harness_gpui::GpuiHarness`
   without `attributes = ...` warns `rstest-bdd[W002]`. The warning says that
   no attribute policy could be inferred, so `#[gpui::test]` was not applied.
   It suggests the canonical path `rstest_bdd_harness_gpui::GpuiHarness`, or
   adding `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy`
   explicitly.
2. Non-canonical path. A path that reaches a first-party adapter through a
   re-export, such as `a::rstest_bdd_harness_tokio::b::TokioHarness`, warns
   `rstest-bdd[W001]`. The warning names the canonical path and states only the
   consequences that really apply.
3. Ambiguous selectors. `harness = rstest_bdd_harness_tokio::TokioHarness`
   with `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy` warns
   `rstest-bdd[W003]` that the two selectors choose different frameworks. It
   suggests removing `attributes = ...` or using the matching policy.
4. Missing executor. An `async fn` scenario with
   `attributes = rstest_bdd_harness::DefaultAttributePolicy` and no harness
   fails with `rstest-bdd[E001]`, which suggests the three fixes. Previously it
   failed with the generic `rstest` message "async test requires either explicit
   `test_attr` or implicit (attribute path ends with `test`)".
5. Ambiguous runtime alias. `scenarios!` with both
   `runtime = "tokio-current-thread"` and `harness = ...` fails once with
   `rstest-bdd[E002]`, which says to remove `runtime = ...`. Today it fails
   once per generated scenario with "combining `harness` with `async fn`
   scenarios is not supported", even though the user wrote no `async fn`.

## Constraints

These are hard invariants. Violating one requires escalation, not a workaround.

- C1 (semver, v0.6.1): the change is additive. No public item, macro argument,
  or accepted syntax is removed or renamed in any published crate (`rstest-bdd`,
  `rstest-bdd-macros`, `rstest-bdd-harness`, `rstest-bdd-harness-tokio`,
  `rstest-bdd-harness-gpui`, `rstest-bdd-policy`).
- C2 (no new rejections): a program that compiles today under default lint
  levels must still compile. A new *error* may only replace an error that the
  same program already produces (lemma `LEM-ERR-REPLACES`). New *warnings* are
  limited to codes W001 to W003 as defined in `Context and orientation`.
- C3 (generated code unchanged): for every configuration that compiles today,
  the emitted test attributes, harness delegation, and trait assertions are
  token-for-token unchanged. Only diagnostic tokens (the stable deprecated-item
  blocks) are added or changed.
- C4 (resolution order): the order "explicit `attributes`, then a known
  first-party `harness`, then the deprecated `runtime` alias, then the runtime
  fallback" is preserved exactly as the code implements it today. Diagnostics
  describe the outcome of that order; they never change it. Where the code
  departs from the prose of ADR-008, the code is preserved and the departure is
  recorded (decision D14).
- C5 (ADR-017 contracts): diagnostics are computed purely and emitted at the
  owning expansion boundary. `scenarios!` emits one diagnostic per distinct
  cause, however many scenarios it generates. Canonical first-party paths and
  same-named custom types such as `custom::TokioHarness` stay warning-free. The
  nightly native channel and the stable deprecated-item channel are mutually
  exclusive. The evidence rule keeps today's conjunct that the adapter crate is
  directly resolvable.
- C6 (no warning for valid third-party usage): a non-first-party
  `harness = ...` without `attributes = ...` stays silent (decision D2).
- C7 (file size): no Rust source file may exceed 400 lines.
  `crates/rstest-bdd-macros/src/codegen/mod.rs` is at exactly 400 lines at
  `8737d88f`, so its net line count must not grow.
- C8 (no suppressions): no new `#[allow]` or `#[expect]` lint suppressions in
  production or test code. Fixtures that demonstrate a user's opt-out are the
  sole exception, because the suppression is the behaviour under test.
- C9 (policy-crate tables untouched): do not edit the private tables in
  `crates/rstest-bdd-policy/src/lib.rs`; consume only its public constants and
  resolvers. Open draft PR #783 (issue #754) is restructuring those tables.
- C10 (tooling): no new external dependency, and no Verus or Kani toolchain
  wiring. The only permitted manifest change is adding the workspace-internal
  `rstest-bdd-harness-tokio` crate as a dev-dependency of
  `rstest-bdd-harness-gpui`, and only if the EP-M0 spike proves it necessary.
- C11 (no macro-argument parser changes): spans come from the paths that are
  already parsed, so `macros/scenario/args.rs` and
  `macros/scenarios/macro_args/mod.rs` are not edited. This keeps the plan
  clear of open draft PR #784.

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
  expansion snapshot, or `harness_defaults` unit case changes. The only
  `.stderr` files permitted to change are the nine existing fallback files and
  the two existing rejection files listed in EP-M2, EP-M3, and EP-M4.
- Test cost: stop if the warm wall-clock time of either harness-crate
  `macro_compile` binary grows by more than 25% over the EP-M0 baseline.
- Iterations: stop if a gate still fails after three fix attempts in one
  milestone.
- Effort: stop if a milestone takes more than one working day of *active*
  effort. Time spent queued for a compile slot does not count.
- Ambiguity: stop if implementation evidence contradicts the taxonomy in
  `Context and orientation`, for example a further configuration class that is
  plausibly "missing" or "ambiguous" but not listed.

## Risks

- Risk R1: users who build with `-D warnings`, or `#![deny(deprecated)]`, see
  a build failure after upgrading if they have a configuration that newly
  warns. The new warnings are W001 for deeply nested re-export paths (silent
  today), W002 for non-canonical GPUI harness paths, and W003 for conflicting
  framework selectors. Severity: medium. Likelihood: low; there are zero
  in-repo instances of the W002, W003, or deep-nesting W001 shapes at
  `8737d88f`. Mitigation: each warning fires only when a consequence has
  demonstrably occurred, or when two explicit selectors contradict each other.
  Each is documented with its fix and its opt-out (decision D10) in the users'
  guide and in `docs/CHANGELOG.md`.
- Risk R2: draft PRs #783 (policy hint tables) and #784 (macro argument
  parsers) touch neighbouring files. Severity: low after revision. Likelihood:
  medium. Mitigation: C9 and C11 keep this plan out of both PRs' files. Draft
  PR #783 also edits `docs/rstest-bdd-design.md`, ADR-008, and
  `docs/developers-guide.md`, which EP-M6 rewrites. Rebase onto `origin/main`
  at the start of each milestone. If a Markdown merge is needed, verify it
  against a plain `git merge-file` (the Weave merge driver can relocate blocks).
- Risk R3: trybuild `.stderr` snapshots are toolchain-sensitive, and
  `rust-toolchain.toml` floats on `stable`. Severity: low. Likelihood: medium.
  Mitigation: follow the existing fixture idiom. Reuse the normalizers in
  `crates/rstest-bdd/tests/trybuild_macros/wrappers.rs` only if drift appears.
- Risk R4: CI has no nightly test lane. The panel reports that the matrix is
  stable Linux plus two stable Windows lanes (`.github/workflows/ci.yml`), so
  the native channel is verified only locally. Severity: low. Likelihood:
  certain. Mitigation: EP-M0 re-confirms the lane list. Every milestone that
  touches a nightly fixture records the local nightly transcript and the exact
  toolchain in `Artefacts and notes`.
- Risk R5: a future `rstest` release could relax its rule for asynchronous
  tests. The rule is currently "an explicit `#[test_attr(...)]`, or any
  attribute whose last path segment is `test`". E001 mirrors that rule; if
  `rstest` relaxed it, E001 would reject a program that `rstest` accepts,
  violating C2. Severity: high. Likelihood: low. `rstest_macros` 0.26.1 and
  0.27.0 have byte-identical `parse/rstest/test_attr.rs` files and the same
  check in `error.rs`, according to the panel's reading of the registry
  sources. Mitigation: the axiom `AX-RSTEST-ASYNC` is guarded by canary
  fixtures that compile plain `rstest` tests. One pins the rejection; the other
  pins the acceptance of the `test_attr` form. A dependency bump that changes
  either fails loudly.
- Risk R6: the conflicting-framework fixture may need both adapter crates
  resolvable in one trybuild scratch crate, and may produce extra rustc errors
  from combining the GPUI attribute with the Tokio harness. Severity: low.
  Likelihood: medium. Mitigation: the EP-M0 spike decides the host suite before
  any production code is written.
- Risk R7: harness-crate trybuild registration files and
  `crates/rstest-bdd/tests/trybuild_macros.rs` (359 lines) may approach the
  400-line cap. Severity: low. Likelihood: low; the plan adds six stable
  fixtures, two canaries, and two nightly fixtures. Mitigation: if a file would
  exceed 380 lines, split the registration into a child module first.
- Risk R8: rustc may not run the `deprecated` lint when the same expansion
  also contains a `compile_error!`. Stable warnings would then vanish beside
  errors while nightly still printed them. Severity: low. Likelihood: medium.
  Mitigation: decision D9 suppresses warnings in both channels whenever an
  error fires at the same boundary, so the outcome no longer depends on this
  rustc detail.
- Risk R9: the `deprecated` lint may not fire for spans produced by another
  crate's `macro_rules!` expansion. A facade macro would then be silent on
  stable while nightly warned. Severity: low. Likelihood: unknown. Mitigation:
  documented as a known limitation in the users' guide; no fixture is planned.

## Progress

- [x] (2026-10-15) Branch prepared. Reconnaissance (four Wyvern agents) and
  prior-art research completed. First draft written (`83a6dab2`).
- [x] (2026-10-15) Community-of-experts design review completed: three
  panels covering all six Logisphere lenses. Verdict: revise. Findings folded
  into this revision; see decisions D10 to D21.
- [ ] Plan approved by the user.
- [ ] EP-M0: baseline, fixture-host spike, and channel experiments.
- [ ] EP-M1: behaviour-preserving extraction of `policy_diagnostics`.
- [ ] EP-M2: W001 and W002 (non-canonical paths and missing policy).
- [ ] EP-M3: W003 and E002 (ambiguous selectors).
- [ ] EP-M4: E001 (missing executor).
- [ ] EP-M5: behavioural scenarios for silence and suggested fixes.
- [ ] EP-M6: documentation, ADR-024, CHANGELOG.
- [ ] EP-M7: roadmap tick, final gates, plan marked `COMPLETE`.

## Surprises & discoveries

- Observation: a warning for every third-party harness without
  `attributes = ...` would break the repository's own tests, and would offer no
  useful fix. Evidence: a scan of tracked `.rs` files at `8737d88f` found 20
  non-first-party `harness =` uses, 17 of them without `attributes =` (the
  command is recorded in `Artefacts and notes`). `make test` compiles all of
  them under `RUSTFLAGS="-D warnings"`. The panel spot-checked
  `RecordingHarness`, `CountingHarness`, and `MetadataCapturingHarness`: they
  are synchronous harnesses that need no native attribute. The only fix such a
  warning could suggest is a custom `attributes = ...`, which changes nothing
  because the macros cannot evaluate unknown policies. Impact: decision D2.
- Observation: the existing ADR-017 message offers three remedies, and none
  of the other two removes the warning. Adding `rstest-bdd-harness` as a direct
  dev-dependency makes the fallback *compile*, but `fallback_candidate` in
  `crates/rstest-bdd-macros/src/codegen/adapter_fallback.rs` never consults
  that dependency, so the warning stays. "Ensure the crate is directly
  resolvable" is always already true when the warning fires, because
  resolvability is one of the warning's own conditions. Impact: the
  fix-convergence lemma `LEM-FIX-CONVERGES` separates *fixes*, which remove a
  finding, from *workarounds*, which only make code build. Messages offer only
  fixes. The workaround moves to the users' guide (decision D5). Three
  documents repeat the false remedy: `docs/users-guide.md` line 942,
  `docs/rstest-bdd-design.md` line 1783, and `docs/v0-6-0-migration-guide.md`
  line 340. EP-M6 corrects all three.
- Observation: the Tokio consequence "`#[tokio::test]` was not applied" is
  never true in a compiling program. Evidence:
  `generate_test_attrs_with_boundary` in `codegen/scenario/test_attrs.rs` drops
  the Tokio attribute for synchronous signatures even on the canonical path. An
  asynchronous scenario with a non-canonical Tokio path already hits either
  E001 (no harness) or `reject_async_harness` (with a harness). Only
  `#[gpui::test]`, which has no synchronous filter, can be silently lost.
  Impact: the model distinguishes the *selected hint* from the *emitted
  attributes*, and consequence clauses come only from the difference in emitted
  attributes (lemma `LEM-CONSEQUENCE-TRUE`; panel: Telefono and Dinolump).
- Observation: `rstest` accepts an asynchronous test if *either* an explicit
  `#[test_attr(...)]` is present, *or* an attribute's last path segment is
  `test`. Evidence: `rstest_macros-0.26.1/src/parse/rstest/test_attr.rs`
  (`extract_test_attr` and `first_valid_test_attrs`) and `src/utils.rs`
  (`attr_ends_with`), read from the local Cargo registry on 2026-10-15. The
  first draft modelled only the second form, so it would have rejected a
  compiling program and violated C2. Impact: `has_user_test_attr` mirrors both
  forms (panel: Telefono and Doggylump).
- Observation: in `scenarios!`, combining `runtime = "tokio-current-thread"`
  with an explicit `harness` is already a hard error. Evidence:
  `resolve_effective_runtime` in `macros/scenarios/test_generation/mod.rs`
  keeps the asynchronous runtime when an explicit harness is present (unit case
  `alias_with_explicit_harness` expects `"async fn"`), so
  `reject_async_harness` in `codegen/scenario/mod.rs` fires once per generated
  scenario at `Span::call_site()`. The nightly-only "redundant" warning in
  `emit_runtime_deprecation_warning` therefore describes a fatal configuration
  as merely redundant. Impact: this combination becomes E002, an error that
  replaces errors. The first draft's model, in which `scenarios!` was never
  asynchronous, was wrong (panel: Telefono and Pandalump). EP-M0 confirms the
  behaviour with a baseline fixture.
- Observation: `rstest-bdd-macros` cannot host `rstest-bdd` behavioural tests
  without a dev-dependency cycle on the crate under test. Impact: behavioural
  scenarios extend the existing
  `crates/rstest-bdd-harness-tokio/tests/features/harness_led_defaults.feature`
  (decision D8).
- Observation: the Windows-only unit test
  `generated_scenario_metadata_uses_portable_feature_separators` in
  `macros/scenarios/test_generation/tests/mod.rs` builds
  `SharedAdapterResolutions` with a struct literal under `#[cfg(windows)]`. The
  Linux gates never compile it. Impact: EP-M1 gives the replacement type a
  constructor and updates this site, and Windows CI is part of EP-M1's evidence
  (panel: Pandalump).

## Decision log

Decisions D1 to D9 were made while drafting. D10 to D21 were added after the
community-of-experts review (Pandalump, Wafflecat, Buzzy Bee, Telefono,
Doggylump, and Dinolump, in three panels). Where a later decision revises an
earlier one, the earlier entry says so.

- Decision D1 (revised by D11, D12, and D13): interpret the roadmap's three
  cases as diagnostic classes defined by *evidence* in the token stream, each
  with a demonstrable consequence and a fix that removes it. Non-canonical
  means W001. Missing means W002 (a lost policy) or E001 (a lost executor).
  Ambiguous means W003 (conflicting frameworks) or E002 (the runtime alias plus
  a harness). Rationale: the macros can act only on what the tokens prove.
  Date/Author: 2026-10-15 / Claude.
- Decision D2: do not warn when a non-first-party harness is supplied without
  `attributes = ...`. Rationale: ADR-008 declares harness-only usage valid. The
  in-repo instances are correct synchronous harnesses, and under `-D warnings`
  a warning is a build failure for a patch upgrade (C1, C6). The only fix such
  a warning could offer is a custom policy, which the macros cannot evaluate.
  Roadmap 12.2.3 owns the policy-extension question; ADR-024 states that
  12.2.3's ADR may supersede this rule. An opt-in strict mode was considered
  and rejected: a new macro argument breaches the Interface tolerance, and a
  Cargo feature unifies across a build, so one crate enabling it would break a
  neighbour that builds with `-D warnings`. Date/Author: 2026-10-15 / Claude
  (panel: Wafflecat).
- Decision D3: E001 and E002 are hard errors. Rationale: each fires only on
  configurations that already fail to compile (`LEM-ERR-REPLACES`), so a
  clearer error rejects nothing new (C2). Date/Author: 2026-10-15 / Claude.
- Decision D4 (revised by D15): generalize `codegen::adapter_fallback` into a
  new `codegen::policy_diagnostics` module, rather than adding a parallel
  mechanism. Rationale: the abstraction sweep required by AGENTS.md found
  `adapter_fallback` to be the only existing emitter with the required purity,
  boundary ownership, and dual-channel behaviour. Date/Author: 2026-10-15 /
  Claude.
- Decision D5: messages offer only fixes that remove the finding. The
  dev-dependency workaround is documented in the users' guide, not in the
  message. The remedy "ensure the crate is directly resolvable" is dropped as
  vacuous. Rationale: `LEM-FIX-CONVERGES` (see discoveries). Date/Author:
  2026-10-15 / Claude (panel: Dinolump).
- Decision D6: compute W003 on each selector's *family*. Non-canonical
  first-party paths and the `runtime` alias contribute the family they imply.
  Rationale: otherwise applying a W001 fix (switching to the canonical path),
  or replacing the alias with `harness = TokioHarness`, can *reveal* a
  framework conflict that was not reported before, so a suggested fix would
  introduce a new diagnostic. Working `LEM-FIX-CONVERGES` through the model by
  hand found this while drafting; the executable exhaustive check must confirm
  it. Date/Author: 2026-10-15 / Claude.
- Decision D7: verification uses exhaustive enumeration of the finite
  abstract model, a token-level refinement check against the real code
  generator, `proptest` for the path classifier, and trybuild for wiring. Verus
  and Kani are not used; see `Verification plan`. Date/Author: 2026-10-15 /
  Claude.
- Decision D8 (revised by D20): behavioural scenarios use `rstest-bdd` in the
  Tokio harness crate, because the macros crate cannot depend on `rstest-bdd`
  without a cycle. Date/Author: 2026-10-15 / Claude.
- Decision D9: when any error fires at an expansion boundary, that boundary
  emits no warnings in either channel. Rationale: rustc's handling of lints
  beside `compile_error!` is uncertain (Risk R8), and both error messages
  already carry the canonical-path fix where one applies. The rule makes stable
  and nightly output agree by construction. Lemma `LEM-FIX-CONVERGES` is stated
  over *findings* (before this suppression), so it is unaffected. Date/Author:
  2026-10-15 / Claude (panel: Pandalump and Doggylump).
- Decision D10: every diagnostic text begins with a stable code:
  `rstest-bdd[W001]`, `[W002]`, `[W003]`, `[E001]`, `[E002]`. The users' guide
  has one anchor per code. Codes are for searching and for documentation links;
  they are not lint names, and on stable every warning arrives through the
  single `deprecated` lint. The documented opt-out is a module-level
  `#[allow(deprecated)]`, because the stable marker is a sibling item that a
  function-level `allow` cannot reach. Nightly native warnings cannot be
  suppressed. EP-M0 verifies the placement and EP-M2 pins it with a `pass`
  fixture. Rationale: a stable, documented escape hatch for `-D warnings` users
  (Risk R1). Date/Author: 2026-10-15 / Claude (panel: Telefono and Dinolump).
- Decision D11: add W002, the missing-policy warning. It fires when the
  harness is non-canonical first-party (`Evidenced`), `attributes` is absent,
  and the canonical harness would have emitted an attribute that the actual
  expansion lacks. In practice that means the GPUI harness, whose
  `#[gpui::test]` is unconditional. It is *rendered* instead of W001 for the
  harness slot, so each slot still produces at most one diagnostic. W001
  remains a finding underneath (see the abstract model), which keeps
  `LEM-FIX-CONVERGES` true for W002's second fix. Its fixes are the canonical
  harness path (which removes everything), or `attributes = <canonical policy>`
  (which restores the attribute). The message states that the second fix leaves
  W001 in place. Rationale: the roadmap's example fix, "adding
  `attributes = ...` explicitly", describes exactly this configuration. The
  first draft's "missing" class was the opposite shape. Date/Author: 2026-10-15
  / Claude (panel: Wafflecat).
- Decision D12: E001 mirrors `rstest`'s rule exactly: a user attribute
  satisfies it if its path is exactly `test_attr`, or its last segment is
  `test`. Rationale: discoveries, C2. Date/Author: 2026-10-15 / Claude (panel:
  Telefono and Doggylump).
- Decision D13: drop the first draft's stable warning for the `runtime`
  alias combined with `attributes = ...`, and turn the alias plus a harness
  into E002. Rationale: the alias plus attributes compiles today and is either
  consistent (a Tokio or default policy) or caught by W003 through the alias's
  implied Tokio family (D6). A new stable warning would land on the more
  explicit spelling while alias-alone users stayed silent. The alias plus a
  harness already fails with a misleading per-scenario error, so E002 replaces
  those errors with one accurate error, spanned at the harness path. Using the
  harness path's span avoids any parser change (C11). The alias-alone
  deprecation warning stays nightly-only, as it is today. Date/Author:
  2026-10-15 / Claude (panel: Wafflecat and Telefono).
- Decision D14: record without changing a departure between ADR-008's prose
  and the code. ADR-008 says that the alias does not change attribute
  selection, but in code `attribute_runtime` makes the alias select the Tokio
  hint when no first-party harness is recognized. Every configuration in which
  that departure is observable is now an E002 error, so preserving the code
  (C4) changes nothing visible. ADR-024 records the departure. Date/Author:
  2026-10-15 / Claude (panel: Telefono).
- Decision D15: the precedence ladder is implemented once. A new pure
  function, `policy_diagnostics::resolution::selected_hint`, takes classified
  slots and the attribute runtime and returns the selected `TestAttributeHint`.
  `test_attrs::resolve_attribute_policy` delegates to it, and so does the
  diagnostic decision. The abstract model's expected values come from lemmas
  and a named witness table, not from a second implementation. Rationale: two
  encodings of precedence could drift, and `diagnose` would otherwise only be a
  differential check of the same author's two definitions. The higher C3 risk
  of touching production precedence is contained by the unchanged
  `harness_defaults` cases, macrotest snapshots, and `REP-EMITTED-TOKENS`.
  Date/Author: 2026-10-15 / Claude (panel: Wafflecat, Pandalump, and Doggylump).
- Decision D16: the evidence rule keeps today's resolvability conjunct, and
  the crate resolver is injected. `classify` takes a `CrateResolver` (a small
  `pub(crate)` trait with production and test implementations), so unit tests
  can reach the unresolvable and renamed-dependency branches that `cfg(test)`
  currently hides. Suggested paths are rendered with the root that the resolver
  returns, so a crate that renames the dependency
  (`tok = { package = "rstest-bdd-harness-tokio" }`) is told to write
  `tok::TokioHarness`. Rationale: without both, a fix could fail to compile
  (E0433) for renamed-dependency and facade users while `LEM-FIX-CANONICAL`
  still passed. Date/Author: 2026-10-15 / Claude (panel: Telefono, Dinolump,
  and Doggylump).
- Decision D17: the stable marker structs get headline names, because the
  name is the first thing a user reads (after the words
  `use of deprecated unit struct`): `RstestBddUnrecognizedAdapterPath` (W001),
  `RstestBddMissingAttributePolicy` (W002), and
  `RstestBddConflictingHarnessAndPolicy` (W003). The existing name
  `RstestBddFirstPartyAdapterFallback` is retired; its `.stderr` files change
  under D5 regardless, and the struct is private generated code. Date/Author:
  2026-10-15 / Claude (panel: Dinolump).
- Decision D18: `Evidenced` carries a private `Framework { Tokio, Gpui }` with
  a conversion into `TestAttributeHint`, so `Evidenced(RstestOnly)` is
  unrepresentable. Rationale: ADR-004 governs ownership of the shared
  vocabulary; a private subset type does not compete with it. Date/Author:
  2026-10-15 / Claude (panel: Telefono).
- Decision D19: the model grows milestone by milestone. EP-M1 introduces only
  the dimensions and findings that today's fallback rule uses; each later
  milestone adds its own, together with the enumerator's asserted state count.
  Rationale: fields read only by tests would trip `dead_code` under
  `-D warnings`, and C8 forbids suppressing it. Date/Author: 2026-10-15 /
  Claude (panel: Pandalump).
- Decision D20: reduce the cost of trybuild and behavioural tests.
  Rationale and evidence: trybuild runs a separate build per fixture in a
  serial nextest group. The first draft would have taken the harness suites
  from 27 to 39 stable fixtures.
  - Fixtures are consolidated: several diagnostics share a fixture where each
    produces its own spanned block. That gives six stable fixtures and two
    canaries instead of twelve.
  - Exhaustive wording is owned by `insta` snapshots. trybuild pins one
    representative per code and channel, for spans and wiring.
  - `pass` silence checks become ordinary integration tests, which
    `make test` compiles under `-D warnings`.
  - EP-M5 adds scenarios to the existing `harness_led_defaults.feature`
    instead of creating a new test binary.

  Date/Author: 2026-10-15 / Claude (panel: Buzzy Bee and Dinolump).
- Decision D21: the shared boundary type carries the *explicit* user
  arguments for classification. In `scenarios!` the API resolution keeps using
  `effective_harness`, which includes the path that the alias generates, but
  classification uses `ScenariosArgs.harness`. Otherwise alias-alone would
  classify as alias plus a harness and raise E002. Rationale: a unit case pins
  alias-alone as diagnostic-free. Date/Author: 2026-10-15 / Claude (panel:
  Pandalump).

## Outcomes & retrospective

Not started. Complete at each milestone boundary and at completion, reconciling
every discovery against the artefacts listed in `Conformance basis`.

## Context and orientation

### Terms of art

- *Harness adapter*: a type implementing
  `rstest_bdd_harness::HarnessAdapter + Default`, supplied as `harness = path`.
  It runs the scenario body, for example inside a Tokio runtime
  (`TokioHarness`) or `gpui::run_test` (`GpuiHarness`).
- *Attribute policy*: a type implementing
  `rstest_bdd_harness::AttributePolicy`, supplied as `attributes = path`. The
  macros cannot call its methods at expansion time, so only first-party
  policies affect emitted attributes.
- *Test attribute hint*: `rstest_bdd_policy::TestAttributeHint`, one of
  `RstestOnly`, `RstestWithTokioCurrentThread`, `RstestWithGpuiTest`. ADR-004
  makes it the shared vocabulary for "which framework attributes to emit". This
  plan calls the hint a path selects that path's *family*.
- *Selected hint* versus *emitted attributes*: the selected hint is the
  outcome of the precedence ladder. The emitted attributes are what the
  generated function actually carries. They differ in one place:
  `#[tokio::test(flavor = "current_thread")]` is emitted only for an `async`
  signature, while `#[gpui::test]` is emitted for any signature.
- *Executor attribute*: an emitted or user-written attribute that satisfies
  `rstest`'s asynchronous-test rule: an explicit `#[test_attr(...)]`, or any
  attribute whose last path segment is `test` (for example `tokio::test` or
  `gpui::test`).
- *Recognized path*: a path that the macros accept as first-party today. That
  is an exact match against a `rstest-bdd-policy` constant, such as
  `rstest_bdd_harness_tokio::TokioHarness` (a leading `::` is ignored); a path
  rooted at the adapter crate or its resolved Cargo rename; or an unaliased
  single-segment import such as `TokioHarness` when the adapter crate is
  resolvable.
- *Evidence*: syntax that shows a path was meant to be first-party without
  being recognized. All three conditions must hold:
  1. the terminal segment is a first-party adapter type name
     (`TokioHarness`, `TokioAttributePolicy`, `GpuiHarness`, or
     `GpuiAttributePolicy`);
  2. the matching adapter crate identifier (`rstest_bdd_harness_tokio` or
     `rstest_bdd_harness_gpui`) appears in a non-root position;
  3. that adapter crate is directly resolvable through `proc_macro_crate`.

  `custom::TokioHarness` has no evidence and is treated as a custom type
  (ADR-017). Today's rule is the special case in which the crate identifier is
  the penultimate segment.
- *Expansion boundary*: the single place in each macro where supplied paths
  are resolved and diagnostics are emitted. For `#[scenario]` it is
  `try_scenario` in `crates/rstest-bdd-macros/src/macros/scenario/mod.rs`. For
  `scenarios!` it is `scenarios` in
  `crates/rstest-bdd-macros/src/macros/scenarios/mod.rs`.
- *Stable channel* and *nightly channel*: stable procedural macros cannot
  emit warnings. Instead the macros emit a `const _` block that uses a
  `#[deprecated(note = "...")]` unit struct, which rustc reports as a
  `deprecated` warning carrying the message. Nightly builds instead call
  `proc_macro::Diagnostic::spanned(..., Level::Warning, ...)`;
  `crates/rstest-bdd-macros/build.rs` sets `cfg(rstest_bdd_nightly)` to select
  this path. `proc_macro::Diagnostic` remains unstable (rust-lang/rust#54140,
  checked on 2026-10-15). Errors use `syn::Error::into_compile_error` on every
  toolchain.
- *Trybuild*: a test crate that compiles fixture programs. A `pass` fixture
  must compile and run. A `compile_fail` fixture must fail, and its stderr must
  match a checked-in `.stderr` file. Trybuild never inspects warnings in `pass`
  fixtures, and strips `RUSTFLAGS` (docs.rs/trybuild 1.0.122). Warning text is
  therefore pinned by adding `#![deny(deprecated)]` to a `compile_fail`
  fixture, which turns the stable warning into an error. Nightly fixtures end in
  `compile_error!("nightly warning snapshot sentinel");` so that stderr
  contains the native warning.

### The diagnostic taxonomy

Warnings use both channels and are suppressed at a boundary that also emits an
error (decision D9). Errors use `compile_error!` on every toolchain.

| Code | Roadmap class | Kind    | Fires when                                                                                    | Fixes offered                                                     |
| ---- | ------------- | ------- | --------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| W001 | non-canonical | warning | a `harness` or `attributes` path has evidence but is not recognized (and W002 does not apply) | write the canonical path (rendered with the resolved crate root)  |
| W002 | missing       | warning | the harness has evidence, `attributes` is absent, and the canonical harness would emit more   | canonical harness path; or add `attributes = <canonical policy>`  |
| W003 | ambiguous     | warning | the implied harness family and the policy family are different frameworks (Tokio versus GPUI) | remove `attributes = ...`; or use the policy matching the harness |
| E001 | missing       | error   | `#[scenario]` `async fn`, no harness, `attributes` present, and no executor attribute results | remove `attributes`; use the Tokio policy; add an executor attr   |
| E002 | ambiguous     | error   | `scenarios!` with `runtime = "tokio-current-thread"` and an explicit `harness`                | remove `runtime = ...`                                            |

*Table 1: the five diagnostics, their roadmap class, and their fixes.*

Each message names only consequences that really occurred. W001 and W002
mention a dropped attribute only when the attribute set the canonical path
would have emitted differs from the attribute set actually emitted. In practice
that is `#[gpui::test]` (see discoveries). W001 always mentions the fallback to
base harness API types, which is the consequence ADR-017 introduced. An
explicit downgrade to `rstest_bdd_harness::DefaultAttributePolicy` is a
documented override and never triggers W003.

Draft wording, refined in EP-M2 to EP-M4 and pinned by `insta`:

```plaintext
rstest-bdd[W001]: this path was not recognized as the first-party `GpuiAttributePolicy`, so
`#[gpui::test]` was not applied. Write `rstest_bdd_harness_gpui::GpuiAttributePolicy`.
```

```plaintext
rstest-bdd[W002]: no attribute policy could be inferred from this harness path, so `#[gpui::test]`
was not applied. Write `rstest_bdd_harness_gpui::GpuiHarness`, or add
`attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy` (which restores the attribute but
leaves warning W001 for the harness path).
```

```plaintext
rstest-bdd[W003]: `harness` selects Tokio but `attributes` selects GPUI; the explicit `attributes`
takes precedence. Remove `attributes = ...`, or write
`attributes = rstest_bdd_harness_tokio::TokioAttributePolicy`.
```

```plaintext
rstest-bdd[E001]: this `async fn` scenario has no executor: `attributes = ...` adds none, and no
`harness` provides one. Remove `attributes = ...` (async scenarios default to Tokio), write
`attributes = rstest_bdd_harness_tokio::TokioAttributePolicy`, or add an executor attribute such as
`#[tokio::test]`.
```

```plaintext
rstest-bdd[E002]: `runtime = "tokio-current-thread"` cannot be combined with `harness = ...`;
the harness already selects the runtime. Remove `runtime = ...`.
```

E001 appends "rstest-bdd cannot read third-party attribute policies" when the
policy is third-party (`Foreign`). W001 for a Tokio path in synchronous code
mentions only the base-API fallback.

Explicitly out of scope, recorded so that a reviewer can challenge it:

- renamed imports such as
  `use rstest_bdd_harness_tokio::TokioHarness as MyHarness`, which leave no
  evidence in the token stream (a documented limitation, already stated in
  design doc §2.7.3);
- a single-segment `TokioHarness` that is actually a local type while the
  adapter crate is resolvable (indistinguishable to the macros);
- warnings for unevaluated third-party attribute policies (roadmap 12.2.3);
- the `runtime` alias alone, whose deprecation warning stays nightly-only;
- user-written `#[tokio::test]` or `#[gpui::test]` that conflicts with the
  resolved policy (candidate follow-up; deduplication in
  `codegen/scenario/test_attrs.rs` already covers the same-framework case);
- passing a policy type as `harness =`, or the reverse (rustc's E0277 through
  the trait-bound assertions already rejects it);
- `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy` on an
  `async fn` without a harness. That emits `#[gpui::test]`, which satisfies
  `rstest`, and whether `gpui::test` accepts the shape is GPUI's concern.

### Files you will touch

- `crates/rstest-bdd-macros/src/codegen/adapter_fallback.rs` (306 lines):
  today's fallback decision (`fallback_candidate`), message, stable tokens,
  nightly emission, and `SharedAdapterResolutions`. EP-M1 moves its contents
  into the new module and deletes the file.
- `crates/rstest-bdd-macros/src/codegen/mod.rs` (400 lines, at the cap):
  `CrateSpec`, the `TOKIO_HARNESS` and `GPUI_HARNESS` specs,
  `HarnessApiResolution` (whose `fallback` field becomes derived from the
  classified slot, so there is one source of truth), `resolve_harness_api`,
  `first_party_adapter_attribute_hint`, and `path_root_matches_crate`. Child
  modules may read its private items, so new code goes into children.
- `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs` (238 lines):
  `TestAttrPolicy`, `resolve_attribute_policy` (which will delegate to
  `selected_hint`), and `generate_test_attrs_with_boundary`.
- `crates/rstest-bdd-macros/src/codegen/scenario/mod.rs` (346 lines):
  `ScenarioConfig` and `reject_async_harness`.
- `crates/rstest-bdd-macros/src/codegen/scenario/adapters.rs` (79 lines): a
  third, non-boundary `SharedAdapterResolutions::resolve` call for callers that
  pass no shared value.
- `crates/rstest-bdd-macros/src/macros/scenario/mod.rs` (260 lines): the
  `#[scenario]` boundary (`try_scenario`). It already knows `sig.asyncness` and
  `item_fn.attrs`, so it builds the full diagnostic inputs.
- `crates/rstest-bdd-macros/src/macros/scenarios/mod.rs` (368 lines): the
  `scenarios!` boundary and `emit_runtime_deprecation_warning`, whose
  alias-plus-harness branch E002 replaces.
- `crates/rstest-bdd-macros/src/macros/scenarios/test_generation/mod.rs`
  (370 lines): `ScenarioTestContext.resolutions`.
- `SharedAdapterResolutions` sites, all updated in EP-M1:
  - production: `codegen/mod.rs:23`, `codegen/scenario/adapters.rs:14,19,33`,
    `codegen/scenario/mod.rs:109`, `macros/scenario/mod.rs:175`,
    `macros/scenarios/mod.rs:75,336`, `macros/scenarios/test_generation/mod.rs:62`;
  - tests: `codegen/scenario/tests/adapter_sharing.rs`,
    `codegen/scenario/tests/runtime_split.rs`, and the `#[cfg(windows)]`
    struct literal in `macros/scenarios/test_generation/tests/mod.rs:44`.
- `crates/rstest-bdd-harness-tokio/tests/macro_compile.rs` (140 lines) and its
  `fixtures_macros/`: the trybuild suite that already hosts the stable and
  nightly fallback fixtures.
- `crates/rstest-bdd-harness-gpui/tests/macro_compile.rs` (115 lines) and its
  `fixtures_macros/`: GPUI fixtures. They run under `--all-features`
  (`native-gpui-tests`), which `make test` passes through `CARGO_FLAGS`.

The nine existing `.stderr` files that pin today's fallback message, all
regenerated in EP-M2 (decisions D5 and D17):

```plaintext
crates/rstest-bdd-harness-gpui/tests/fixtures_macros/nightly_gpui_adapter_fallback_warning.stderr
crates/rstest-bdd-harness-gpui/tests/fixtures_macros/scenario_harness_gpui_alias_fallback_warning.stderr
crates/rstest-bdd-harness-tokio/tests/fixtures_macros/nightly_adapter_fallback_warning.stderr
crates/rstest-bdd-harness-tokio/tests/fixtures_macros/nightly_multi_scenario_alias_fallback_warning.stderr
crates/rstest-bdd-harness-tokio/tests/fixtures_macros/scenario_attributes_alias_fallback_warning.stderr
crates/rstest-bdd-harness-tokio/tests/fixtures_macros/scenario_harness_alias_fallback_warning.stderr
crates/rstest-bdd-harness-tokio/tests/fixtures_macros/scenarios_attributes_alias_fallback_warning.stderr
crates/rstest-bdd-harness-tokio/tests/fixtures_macros/scenarios_combined_alias_fallback_warning.stderr
crates/rstest-bdd-harness-tokio/tests/fixtures_macros/scenarios_multi_scenario_alias_fallback_warning.stderr
```

New module layout, all under `crates/rstest-bdd-macros/src/codegen/`:

```plaintext
policy_diagnostics/
  mod.rs          module docs, re-exports, SharedPolicyDiagnostics (the boundary type)
  model.rs        pure kernel: SlotClass, Framework, PolicyInputs, PolicyFindings, diagnose()
  resolution.rs   selected_hint() and emitted_attributes(): the single precedence implementation
  classify.rs     syn::Path -> SlotClass, through an injected CrateResolver
  message.rs      finding -> code-prefixed message; suggested roots from the resolver
  emit.rs         stable deprecated-item tokens, nightly Diagnostic, error tokens
  tests/          exhaustive model checks, refinement witnesses, classifier properties, insta snapshots
```

### Skills and documentation to load

Before implementation, load `rust-router`. Then, as each facet arises, load:

- `rust-unit-testing`, for `rstest` tables, `googletest`, `pretty_assertions`,
  and `insta`;
- `proptest`;
- `rust-verification`, for method selection;
- `rust-errors`, for the two new errors;
- `rust-unused-code`, for the cfg-gated channel functions;
- `arch-decision-records`, for ADR-024;
- `en-gb-oxendict-style`, for message wording and documentation.

Use `codegraph-mcp` (`codegraph_get_callers`, `codegraph_analyze_impact`)
before moving `adapter_fallback` items, and `nextest` for focused runs.

Read:

- `docs/rstest-bdd-design.md` §§2.7.3-2.7.6.4: harness integration, ADR-008
  codegen, first-party targets, and the v0.6.1 helpers;
- `docs/adr-008-harness-led-attribute-policy-defaults.md`: precedence, and
  the validity of harness-only and attributes-only usage;
- `docs/adr-017-toolchain-aware-adapter-fallback-diagnostics.md`: channels,
  cardinality, and ownership;
- `docs/adr-004-policy-crate.md`: `TestAttributeHint` as the single source;
- `docs/developers-guide.md`: "Adapter fallback diagnostics" and "Staging
  fixtures for trybuild tests";
- `docs/users-guide.md`: "Harness adapter and attribute policy" and
  "First-party adapter fallback diagnostics";
- `docs/rust-testing-with-rstest-fixtures.md`: fixture and table idioms;
- `docs/rust-doctest-dry-guide.md`: doctest style for new rustdoc examples;
- `docs/complexity-antipatterns-and-refactoring-strategies.md`: keep
  `try_scenario` and `scenarios` from growing, and extract helpers instead;
- `docs/gherkin-syntax.md`: feature-file syntax for EP-M5;
- `docs/documentation-style-guide.md`: the ADR template and
  en-GB-oxendict spelling.

`docs/rstest-bdd-language-server-design.md` was checked: the language server
does not read `harness` or `attributes` paths, so it is unaffected.

## Conformance basis

Upstream artefacts, all at repository revision `8737d88f`:

- Roadmap item 11.2.3 in `docs/roadmap.md` §11.2, including its finish line,
  and the Phase 11 preamble on semver-compatible additive hardening.
- Design document `docs/rstest-bdd-design.md`:
  - §2.7.3: macro integration, the attribute-policy trust model, and the
    fallback diagnostic;
  - §2.7.4: ADR-008 codegen;
  - §2.7.5: first-party targets;
  - §2.7.6.3 and §2.7.6.4: the beta quick wins and the v0.6.1 helpers.
- ADR-004 (policy crate), ADR-005a (adapter crates), ADR-007 (context
  injection), ADR-008 (harness-led defaults), and ADR-017 (fallback
  diagnostics).
- Governing standards: `AGENTS.md` and `docs/documentation-style-guide.md`.
- No Terms of Reference document exists for this work, and none is invented.

Traced requirements (identifiers are local to this plan):

- `REQ-NC`: diagnose non-canonical harness and policy paths.
- `REQ-MISS`: diagnose missing attribute-policy annotations.
- `REQ-AMB`: diagnose ambiguous attribute-policy annotations.
- `REQ-FIX`: each diagnostic carries actionable fix text.
- `REQ-TRY`: trybuild tests cover the three cases and assert the fix text.
- `REQ-COMPAT`: Phase 11 semver compatibility (C1 to C3).

Trace chain:

```plaintext
11.2.3 -> REQ-NC     -> §2.7.3, ADR-017 -> EP-M1, EP-M2 -> W001 fixtures, LEM-EVIDENCE
11.2.3 -> REQ-MISS   -> §2.7.3, ADR-008 -> EP-M2, EP-M4 -> W002 + E001 fixtures, LEM-ERR-REPLACES
11.2.3 -> REQ-AMB    -> ADR-008         -> EP-M3        -> W003 + E002 fixtures, LEM-SILENCE
11.2.3 -> REQ-FIX    -> ADR-024 (new)   -> EP-M2..EP-M4 -> LEM-FIX-CONVERGES, LEM-FIX-CANONICAL, insta
11.2.3 -> REQ-TRY    -> dev guide       -> EP-M2..EP-M4 -> trybuild stable + nightly representatives
Ph. 11 -> REQ-COMPAT -> C1-C3           -> EP-M1..EP-M5 -> LEM-SILENCE, REP-EMITTED-TOKENS, BDD scenarios
```

## Verification plan

### Abstract model (specification, independent of `syn`)

Each supplied slot (`harness` or `attributes`) is classified into a finite
`SlotClass`:

```plaintext
SlotClass ::= Absent
            | Recognized(h)    h in {Base, Tokio, Gpui}   (Base = RstestOnly)
            | Evidenced(f)     f in {Tokio, Gpui}         (non-canonical first-party, with evidence)
            | Foreign                                     (anything else, including custom::TokioHarness)
```

`family(Recognized(h)) = h` and `family(Evidenced(f)) = f`. `family` is
undefined for `Absent` and `Foreign`. `canon(s)` maps `Evidenced(f)` to
`Recognized(f)` and leaves every other class unchanged.

A configuration is one of two shapes, because the two macros expose different
inputs:

```plaintext
Scenario  { harness, attributes, is_async: bool, has_user_test_attr: bool }   7 × 7 × 2 × 2 = 196 states
Scenarios { harness, attributes, runtime_alias: bool }                        7 × 7 × 2     =  98 states
```

The domain therefore has 294 states. `harness` here is always the *explicit*
user argument (decision D21).

The precedence ladder, implemented once as `selected_hint` (D15) and used by
both code generation and diagnostics, is:

```plaintext
attribute_runtime(c) = Tokio if (Scenario and is_async) or (Scenarios and runtime_alias) else Base
selected_hint(c) =
  Recognized(h)      in attributes -> h
  Evidenced or Foreign in attributes -> Base
  Recognized(h)      in harness    -> h
  otherwise                         -> attribute_runtime(c)
executes_async(c) = (Scenario and is_async) or (Scenarios and runtime_alias and harness != Absent)
emitted(c) = {tokio::test} if selected_hint(c) = Tokio and executes_async(c)
             {gpui::test}  if selected_hint(c) = Gpui
             {}            otherwise
implied_harness_family(c) = family(harness) if defined,
                            else Tokio if Scenarios and runtime_alias and harness = Absent
```

`executes_async` for `scenarios!` reproduces `resolve_effective_runtime`: the
alias forces a synchronous body unless an explicit harness is present.

The findings are a fixed-shape record, so a duplicate is unrepresentable:

```plaintext
W001(s)  iff  slot s = Evidenced(_)
W002     iff  harness = Evidenced(_) and attributes = Absent and emitted(c[harness := canon]) ⊋ emitted(c)
W003     iff  implied_harness_family(c) and family(attributes) are both defined, both in {Tokio, Gpui}, and differ
E001     iff  Scenario and is_async and harness = Absent and attributes != Absent
              and emitted(c) contains no executor attribute and not has_user_test_attr
E002     iff  Scenarios and runtime_alias and harness != Absent
EXISTING iff  Scenario and is_async and harness != Absent        (today's reject_async_harness, unchanged)
```

Emission is a rendering of the findings, separate from the findings themselves:

- if any error (E001, E002, or EXISTING) fires at a boundary, only errors
  are emitted (D9);
- otherwise each slot emits at most one warning: W002 when it holds, rendered
  in place of the harness slot's W001, and W001 otherwise; W003 is emitted once
  per boundary;
- E002 replaces the per-scenario EXISTING errors that the same `scenarios!`
  invocation produces today.

W001(harness) stays a *finding* whenever the harness has evidence, even when
the rendering shows W002 instead. Otherwise W002's second fix, adding
`attributes = ...`, would surface a W001 that was not in the original finding
set and violate `LEM-FIX-CONVERGES`. Working the lemma by hand found this
during the revision.

Each finding carries suggested *fixes*, each modelled as a transformation on
states:

- W001: `s := canon(s)`.
- W002: `harness := canon(harness)`; or
  `attributes := Recognized(family(harness))`.
- W003: `attributes := Absent`; or
  `attributes := Recognized(implied_harness_family(c))`.
- E001: `attributes := Absent`; or `attributes := Recognized(Tokio)`; or
  `has_user_test_attr := true`.
- E002: `runtime_alias := false`.

### Axioms

- `AX-RSTEST-ASYNC`: `rstest` 0.26.1 rejects an `async` test function unless
  it carries an explicit `#[test_attr(...)]` or an attribute whose last path
  segment is `test`. Evidence: `extract_test_attr` and `first_valid_test_attrs`
  in `rstest_macros-0.26.1/src/parse/rstest/test_attr.rs`, and `attr_ends_with`
  in `src/utils.rs`; 0.27.0 is reported identical. rstest-bdd places user
  attributes after `#[rstest::rstest]` (`codegen/scenario/mod.rs`), so `rstest`
  sees them. Guarded by two canary fixtures (Risk R5).
- `AX-TRYBUILD`: trybuild compares `compile_fail` stderr against `.stderr`
  files, ignores warnings in `pass` fixtures, and strips `RUSTFLAGS`.
- `AX-DEPRECATED`: rustc reports a use of a `#[deprecated(note = ...)]` item
  as the `deprecated` lint carrying the note. The lint warns by default and is
  escalated by `deny(deprecated)` or `-D warnings`. A module-level
  `#[allow(deprecated)]` suppresses it (verified in EP-M0).
- `AX-SYN-PATH`: `syn::Path::segments` excludes a leading `::`.
- `AX-PMC`: `proc_macro_crate::crate_name` reports whether a package is a
  direct dependency and under which name. The injected `CrateResolver` stands
  in for it in unit tests; one production-path trybuild case per branch
  (resolvable, renamed) checks the real interface.
- `AX-EXISTING-REJECT`: `reject_async_harness` fires for every generated
  scenario of a `scenarios!` invocation that has the alias plus an explicit
  harness. This is a code fact; it is confirmed by the EP-M0 baseline fixture
  before E002 replaces it.

### Obligations

Two kinds of evidence are distinguished. The finite model is checked
exhaustively, which is complete *for the model*. That the model matches the
real macros rests on `REP-EMITTED-TOKENS`, `LEM-EVIDENCE`, and the trybuild
fixtures. Those obligations carry the weight, and are designed to be
independently falsifiable.

- Obligation `LEM-SILENCE` (backward compatibility): for every state that
  has no `Evidenced` slot, no framework conflict, and no error, no warning is
  emitted. In particular these are silent:
  - `Absent` slots;
  - harness-only `Foreign`;
  - downgrades of the form `(Recognized(Tokio or Gpui), Recognized(Base))`;
  - the alias alone.

  Method: exhaustive enumeration of the 294 states in a `#[test]`. Rationale:
  the domain is finite, so enumeration decides the property. The property is
  phrased as a characterization of silence, written independently of the
  clauses of `diagnose`, and the named witness table (below) supplies expected
  values without a second implementation. Artefact:
  `policy_diagnostics/tests/exhaustive.rs`. Evidence:
  `cargo nextest run -p rstest-bdd-macros policy_diagnostics`.

  Non-vacuity checks:
  - the enumerator yields exactly 294 distinct states (asserted, and grown
    per milestone under D19);
  - the number of states satisfying the antecedent is recorded and asserted
    to be non-zero;
  - a guard function that matches without a wildcard fails to compile if a
    `SlotClass` or `Framework` variant is added without updating the
    enumerator.

  Seeded fault: treating `Base` as a framework family in W003 must fail on the
  `(Recognized(Tokio), Recognized(Base))` downgrade.
- Obligation `LEM-ERR-REPLACES` (C2): E001 implies that the expansion is
  already rejected today, by `AX-RSTEST-ASYNC`. E002 implies that today's
  expansion already contains `reject_async_harness` errors, by
  `AX-EXISTING-REJECT`. Method: exhaustive enumeration for the model part;
  trybuild for the link to reality. Each E001 and E002 fixture's pre-change
  stderr, recorded in EP-M0, must show the old error. Non-vacuity: E001's
  antecedent is reached by `Recognized(Base)`, `Foreign`, and
  `Evidenced(Tokio)` attributes; E002's by every non-`Absent` harness class.

  Seeded faults:
  - dropping the `test_attr` disjunct must fail on the witness that has
    `#[test_attr(tokio::test)]`;
  - dropping `harness = Absent` must break disjointness with EXISTING.
- Obligation `LEM-DISJOINT` (no double errors): E001 and EXISTING are never
  both true; E002 holds only for the `scenarios!` shape and E001 only for the
  `#[scenario]` shape. Method: exhaustive enumeration.
- Obligation `LEM-CARDINALITY` (C5): each slot *renders* at most one warning
  (W001 or W002, never both). A boundary emits at most four warnings or one
  error set. `scenarios!` emits the same set however many scenarios it
  generates. Method: the fixed-shape record makes duplication unrepresentable.
  Boundary behaviour is checked by extending
  `codegen/scenario/tests/adapter_sharing.rs` marker counts to the new marker
  structs, using a two-scenario feature. Non-vacuity: a witness that reaches
  W001(attributes), W002, and W003 together is asserted to emit exactly three
  markers.
- Obligation `LEM-FIX-CONVERGES` (`REQ-FIX`): for every state `c`, every
  finding `d` of `c`, and every fix `φ` suggested for `d`: `d` is not a finding
  of `φ(c)`, and the findings of `φ(c)` are a subset of the findings of `c`
  minus `d`. A fix never introduces a new finding. The lemma is stated over
  findings, not emitted diagnostics, so D9's suppression does not mask it.
  Method: exhaustive enumeration over states × findings × fixes. Rationale:
  working this lemma by hand already falsified four design candidates during
  drafting and review (D5, D6, and twice in D11), which shows that it
  constrains the design rather than restating it. Non-vacuity: the test counts
  the `(state, finding, fix)` triples checked for each finding kind and asserts
  every count is non-zero. Seeded fault: computing W003 on `Recognized`
  families only (the rejected D6 candidate) must fail.
- Obligation `LEM-CONSEQUENCE-TRUE` (honest messages): whenever a message
  states that an attribute "was not applied", that attribute is in the
  attributes the canonical form would emit and not in the attributes actually
  emitted. Method: exhaustive enumeration, plus `insta` snapshots keyed by
  (code, family, slot, `is_async`). Seeded fault: an unconditional Tokio
  consequence clause must fail on a synchronous W001 state.
- Obligation `LEM-FIX-CANONICAL`: every path that a message suggests parses
  as a `syn::Path` that `classify`, under the same resolver, maps to
  `Recognized(f)` for the intended family. That includes a resolver that
  reports a renamed dependency, so for example `tok::TokioHarness` must be
  recognized. Method: an `rstest` table over rendered messages for the default,
  renamed, and (for W001) unresolvable-adapter resolvers, extracting the
  backticked paths and classifying them. Non-vacuity: each message kind
  contributes at least one path, and the renamed case yields a root other than
  the default. Seeded faults: suggesting a Tokio path for a GPUI finding, and
  rendering the default root under a renamed resolver, must both fail.
- Obligation `LEM-EVIDENCE` (concretization soundness over an unbounded
  domain): for any `syn::Path` `p` and any resolver state:
  - `classify(p) = Recognized(h)` exactly when today's recognition returns
    hint `h`, so recognition is unchanged;
  - `classify(p) = Evidenced(f)` implies that the last segment is an adapter
    name of `f`, that `f`'s crate identifier occurs at a non-root position,
    that the crate is resolvable, and that `p` is not recognized;
  - `classify` is invariant under a leading `::`;
  - a path with no first-party crate identifier is never `Evidenced`;
  - an unresolvable adapter crate never yields `Evidenced`.

  Method: `proptest`. Generate the target class first, then a path for that
  class: segment sequences of length 1 to 6, drawn from first-party crate
  identifiers, adapter type names, and arbitrary identifiers, under a generated
  resolver state. `Absent` is excluded because `classify` never produces it.
  Add named `rstest` cases for:
  - `custom::TokioHarness`;
  - `alias::rstest_bdd_harness_tokio::TokioHarness`;
  - `a::rstest_bdd_harness_gpui::b::GpuiHarness`;
  - `rstest_bdd_harness_tokio::prelude::TokioHarness`, which is recognized
    because its root is the crate;
  - `$crate`-style facade paths under an unresolvable resolver.

  Rationale: the path space is unbounded, but the properties are membership
  predicates without recursion or arithmetic, so class-directed property tests
  are proportionate. Artefact: `policy_diagnostics/tests/classify_props.rs`.
  Non-vacuity: class-first generation makes each class's frequency a property
  of the strategy, not of chance. A fixed seed is recorded, and the test
  asserts that each generated class round-trips through `classify`. Seeded
  fault: matching only the penultimate segment (today's ADR-017 rule) must fail
  the deep-nesting case.
- Obligation `REP-EMITTED-TOKENS` (the representation relationship): for one
  concrete witness path per `SlotClass`, across all `#[scenario]` and
  `scenarios!` states, the attribute tokens produced by the real
  `generate_test_attrs_with_boundary` equal the model's `emitted(c)`. Method:
  an exhaustive witness table in a unit test that compares token strings.
  Rationale: this obligation links the model to generated code, and it would
  catch a refactor of `resolve_attribute_policy` that changed emitted
  attributes (C3). Non-vacuity: `classify(witness(k)) = k` for every class `k`,
  and the table covers every `(harness, attributes)` class pair. Seeded fault:
  letting `Evidenced` attributes inherit their family in `selected_hint` must
  fail.
- Obligation `LEM-CHANNEL-EXCLUSIVE` (C5): on nightly no deprecated-item
  tokens are emitted, and on stable no native diagnostic is emitted. E002
  replaces both the per-scenario `reject_async_harness` errors and the nightly
  `emit_warning` "redundant" call for the same configuration. Method:
  `runtime_split.rs`-style unit tests extended to the new markers, plus paired
  stable and nightly trybuild representatives.
- Obligation `OBS-FIX-TEXT` (`REQ-TRY`): one trybuild representative per
  code and channel pins the full text, including the fix and the span. The
  `insta` snapshot set pins the wording of every
  `(code, family, slot, is_async, resolver)` variant. Method: trybuild and
  `insta`. Rationale: the output is multivariant text whose consistency
  matters, so it needs snapshot tests (AGENTS.md).

### Methods deliberately not used

Verus is not warranted. The kernel's domain is finite (294 states), so
exhaustive enumeration decides every model lemma in the ordinary test suite. A
Verus proof of the same statements would add assurance only about the model,
which is not where the residual risk lies. The residual risk lies in the
model's correspondence to `syn` paths and generated tokens, which
`REP-EMITTED-TOKENS`, `LEM-EVIDENCE`, and trybuild address against the real
code. The only unbounded domain, path classification, consists of shallow
membership predicates that `proptest` and named cases cover proportionately.
The repository has no Verus toolchain, and introducing one would breach C10 for
no gain in assurance.

Kani is not warranted either: there is no `unsafe` code, no arithmetic, and no
bounded state machine beyond what enumeration already covers.

Revisit both if roadmap 12.2.3 introduces an open, user-extensible policy
registry, because the domain would then become unbounded.

If `cargo mutants` is available locally, run it on
`policy_diagnostics/model.rs`, `resolution.rs`, and `classify.rs` as
supplementary evidence that the suite rejects plausible defects. Record
surviving mutants in `Surprises & discoveries`. This is evidence, not a gate.
Record each hand-seeded fault as a diff plus the name of the test that rejects
it, so that it can be replayed.

## Plan of work

### Stage A: understand and propose (no production code)

This is EP-M0 below. Record baselines, run the fixture-host spike, and run the
channel experiments.

### Stage B: red tests

Each feature milestone (EP-M2 to EP-M4) begins by adding its model witness
cases, its `insta` snapshot cases, and its trybuild fixtures. Run them and
observe the expected failure before writing production code. For trybuild, the
new `compile_fail` fixture either compiles (no diagnostic yet) or produces
stderr that differs from the checked-in expectation. For the model, `diagnose`
initially reports no finding of the new kind, so the named witness cases fail.

### Stage C: green implementation

Implement the smallest change in `policy_diagnostics` that makes the red tests
pass, then wire it into the owning boundary.

### Stage D: refactor, documentation, wider validation

Tidy helpers and keep functions small (AGENTS.md). Update documentation
(EP-M6), and run the full gates.

## Milestones and plateaus

### EP-M0: baseline, fixture-host spike, and channel experiments (prototype)

Outcome: baselines are recorded and three questions are answered, with no
production code changes. Rebase onto `origin/main` first.

Record:

- the `make test` pass count;
- the warm nextest durations of the Tokio and GPUI `macro_compile` binaries
  (these feed the test-cost tolerance);
- the current stderr of `scenario_default_policy_async_no_harness_rejected`;
- the CI lane list from `.github/workflows/ci.yml`.

Experiments, each in a throwaway fixture that is deleted afterwards:

1. Fixture host for W003 (Risk R6). Combine
   `harness = rstest_bdd_harness_gpui::GpuiHarness` with
   `attributes = rstest_bdd_harness_tokio::TokioAttributePolicy` in the GPUI
   suite, and the reverse pairing in the Tokio suite. Keep the shape that
   produces only the expected output. If neither works without a manifest
   change, add `rstest-bdd-harness-tokio` as a workspace dev-dependency of
   `rstest-bdd-harness-gpui`, the single change that C10 permits.
2. `AX-EXISTING-REJECT`. Compile `scenarios!` with
   `runtime = "tokio-current-thread"` plus
   `harness = rstest_bdd_harness_tokio::TokioHarness` over a two-scenario
   feature. Record the stderr, which should contain two `reject_async_harness`
   errors. If it compiles instead, E002 is invalid: stop and revise the plan.
3. Channel behaviour (D9 and D10, Risk R8). Check whether a stable
   deprecated-item warning appears beside a `compile_error!` in the same
   expansion, and whether a module-level `#[allow(deprecated)]` placed around a
   `#[scenario]` or a `scenarios!` invocation silences the stable marker.

- Requirements: none discharged. This de-risks `REQ-AMB`, `REQ-TRY`, and C2.
- Acceptance: transcripts are recorded in `Artefacts and notes`, and each
  experiment's decision is recorded in `Decision log`.
- Conformance check:
  - no production code has changed;
  - the design is unchanged, or a deviation is recorded and the plan is set
    to `BLOCKED`;
  - Risks R4, R6, and R8 are resolved or re-rated;
  - at most the one permitted manifest change has been made;
  - there is no persisted-format or trust-boundary change;
  - trace links are current.
- Recovery: delete the throwaway fixtures.
- Remaining gaps: everything else.
- Compatibility decision: none.

### EP-M1: extract `policy_diagnostics` (behaviour-preserving refactor)

Outcome: `codegen/adapter_fallback.rs` is replaced by
`codegen/policy_diagnostics/`. The tree contains:

- the model, restricted to the W001 dimensions (D19);
- `selected_hint`, to which `resolve_attribute_policy` now delegates (D15);
- the classifier with an injected `CrateResolver` (D16), expressing today's
  penultimate-segment rule;
- the dual-channel emitter.

All existing tests, fixtures, and `.stderr` files are unchanged. This is a
separate atomic refactor commit (AGENTS.md).

Work:

1. Create the module tree. Move `AdapterFallback`, the message function, the
   stable token builder, the nightly emitter, and `SharedAdapterResolutions`.
2. Rename `SharedAdapterResolutions` to `SharedPolicyDiagnostics`, give it a
   constructor, and update every site listed in `Files you will touch`,
   including the `#[cfg(windows)]` literal.
3. Derive `HarnessApiResolution.fallback` from the classified slot, so that
   one source of truth remains.
4. Add the enumerator, its count test, the variant guard, and the
   `REP-EMITTED-TOKENS` witness table for the dimensions present.
5. Keep `codegen/mod.rs` at or below 400 lines.

- Requirements: advances `REQ-NC` (structure only) and `REQ-COMPAT`.
- Acceptance:
  - `make check-fmt`, `make lint`, and `make test` pass;
  - `git diff` shows no `.stderr`, macrotest snapshot, `pass` fixture, or
    `harness_defaults` change;
  - Windows CI passes on the pushed branch, which proves that the
    `#[cfg(windows)]` site compiles.
- Conformance check:
  - no requirement is newly claimed;
  - the ADR-017 structure is preserved (pure resolution, boundary emission);
  - assumptions are unchanged;
  - there is no public interface change (all items are `pub(crate)` or
    narrower);
  - there is no trust-boundary change;
  - trace links are updated for the module rename.
- Recovery: `git revert` the single refactor commit.
- Remaining gaps: the new findings.
- Compatibility decision: none (crate-private code).

### EP-M2: W001 and W002 (non-canonical paths and missing policy)

Outcome: W001 fires for any non-root occurrence of a resolvable adapter crate
identifier. W002 replaces W001 on the harness slot when a policy was lost.
Messages carry codes, headline marker names, true consequences, and
resolver-rooted fixes.

Red:

- Add `LEM-EVIDENCE` deep-nesting, renamed, and unresolvable cases, the
  `LEM-FIX-CANONICAL` table, the `LEM-CONSEQUENCE-TRUE` states, and the
  W001/W002 `insta` cases.
- Add trybuild fixtures, all stable fixtures under `#![deny(deprecated)]`:
  - `crates/rstest-bdd-harness-tokio/tests/fixtures_macros/policy_w001_noncanonical.rs`:
    a nested harness and a nested policy in synchronous code, giving two W001
    blocks with no attribute clause;
  - `crates/rstest-bdd-harness-gpui/tests/fixtures_macros/policy_w002_missing_policy.rs`:
    a nested GPUI harness without attributes (W002), plus a nested GPUI
    policy (W001 with the `#[gpui::test]` clause);
  - nightly twin
    `crates/rstest-bdd-harness-gpui/tests/fixtures_macros/nightly_policy_w002_warning.rs`.
- Add an integration test,
  `crates/rstest-bdd-harness-tokio/tests/policy_opt_out.rs`: a module-level
  `#[allow(deprecated)]` around a W001 configuration. It compiles under
  `make test`'s `-D warnings`, which pins the documented opt-out (D10). This is
  the C8 exception.

Green:

- Widen `classify`, add W002 to the model, and extend `message.rs`.
- Regenerate the nine existing `.stderr` files on stable and on nightly for
  both suites, and review every diff line by line.

- Requirements: `REQ-NC`, `REQ-MISS` (W002), `REQ-FIX`, and `REQ-TRY` for
  these codes.
- Acceptance:
  - the new fixtures and the opt-out test pass;
  - `scenario_custom_tokio_harness_name.rs` (a custom type under
    `deny(deprecated)`, `pass`) still passes;
  - the nightly regeneration transcript is recorded.
- Conformance check:
  - REQ-NC and the W002 part of REQ-MISS are satisfied;
  - ADR-017 cardinality and custom-type silence hold;
  - assumptions remain valid;
  - there is no public interface change;
  - there is no trust-boundary change;
  - trace links are current.
- Recovery: revert the milestone commits. EP-M1 remains a coherent plateau.
- Remaining gaps: W003, E001, and E002.
- Compatibility decision: none.

### EP-M3: W003 and E002 (ambiguous selectors)

Outcome: W003 fires through both channels. E002 replaces today's per-scenario
errors for the alias plus a harness, and also the nightly "redundant" warning
for that configuration.

Red:

- Add model witnesses, the D6 states for `LEM-FIX-CONVERGES`, the D21
  alias-alone unit case, and the `adapter_sharing.rs` three-marker case.
- Add trybuild fixtures in the suite chosen in EP-M0:
  - `policy_w003_conflicting_frameworks.rs`: a `#[scenario]` with
    `TokioHarness` plus `GpuiAttributePolicy`, and a `scenarios!` invocation
    with the alias plus `GpuiAttributePolicy` (implied Tokio). Each sits in
    its own `mod` and produces its own W003 block;
  - nightly twin `nightly_policy_w003_warning.rs`;
  - `scenarios_policy_e002_runtime_alias_with_harness.rs` in the Tokio
    suite, over a two-scenario feature, expecting exactly one E002 error.
- Add the downgrade silence case, `TokioHarness` plus
  `DefaultAttributePolicy`, to EP-M5's behavioural scenarios instead of a new
  `pass` fixture (D20).

Green:

- Add W003 and E002 to the model.
- Emit E002 at the `scenarios!` boundary, spanned at the harness path, and
  stop generating the scenario tests for that invocation, so that the
  per-scenario `reject_async_harness` errors disappear.
- Remove the alias-plus-harness branch of `emit_runtime_deprecation_warning`.
  The alias-alone branch is unchanged.
- Update `nightly_runtime_deprecation_warning.stderr` if its text moves.

- Requirements: `REQ-AMB`, `REQ-FIX`, and `REQ-TRY` for these codes.
- Acceptance:
  - the fixtures pass;
  - the existing GPUI and Tokio `*_override_default.rs` fixtures are
    unchanged;
  - the E002 fixture shows one error, not one per scenario.
- Conformance check:
  - REQ-AMB is satisfied;
  - the precedence order is unchanged (C4);
  - C2 holds (`LEM-ERR-REPLACES` for E002);
  - no macro argument or parser is changed (C11);
  - there is no trust-boundary change;
  - trace links are current.
- Recovery: revert. EP-M2 remains coherent.
- Remaining gaps: E001.
- Compatibility decision: none.

### EP-M4: E001 (missing executor)

Outcome: E001 replaces `rstest`'s generic asynchronous-test error with an
`rstest-bdd` error that carries the fixes.

Red:

- Add model witnesses for `LEM-ERR-REPLACES` and `LEM-DISJOINT`.
- Extend the existing Tokio-suite fixture
  `scenario_default_policy_async_no_harness_rejected.rs` with two more
  functions, a `Foreign` policy and an `Evidenced(Tokio)` policy, both `async`
  and without an executor. Its `.stderr` is expected to change to three E001
  blocks.
- Add canaries that use `rstest` alone:
  - `rstest_async_requires_test_attr_canary.rs` (`compile_fail`), which pins
    the rejection;
  - `rstest_async_test_attr_accepted_canary.rs` (`pass`), which pins
    acceptance of `#[test_attr(tokio::test)]`.
- Add an integration test,
  `crates/rstest-bdd-harness-tokio/tests/policy_user_executor.rs`, with two
  scenarios that must keep compiling: a `Foreign` policy with a user-written
  `#[tokio::test]`, and `DefaultAttributePolicy` with
  `#[test_attr(tokio::test)]`.

Green:

- `try_scenario` builds `PolicyInputs` from `sig.asyncness` and
  `item_fn.attrs`, so `diagnose` is the single owner of the E001 decision (no
  second check lives in `codegen/scenario`).
- On E001, the boundary returns the error spanned at the `attributes` path
  and emits no warnings (D9).
- Keep `try_scenario` from growing: extract the input construction into a
  helper in `macros/scenario/`.

- Requirements: `REQ-MISS` (E001), `REQ-FIX`, and `REQ-TRY` for this code.
- Acceptance:
  - the fixtures, canaries, and integration test pass;
  - every pre-existing `async` `pass` fixture is unchanged.
- Conformance check:
  - REQ-MISS is satisfied;
  - C2 holds;
  - `AX-RSTEST-ASYNC` has been re-checked against the pinned `rstest`
    version;
  - there is no public interface change;
  - there is no trust-boundary change;
  - trace links are current.
- Recovery: revert. EP-M3 remains coherent.
- Remaining gaps: the behavioural scenarios and documentation.
- Compatibility decision: none.

### EP-M5: behavioural scenarios for silence and suggested fixes

Outcome: `rstest-bdd` scenarios demonstrate that the configurations the
diagnostics recommend, and the valid configurations that must stay silent,
compile without diagnostics and behave as promised.

Add scenarios to the existing
`crates/rstest-bdd-harness-tokio/tests/features/harness_led_defaults.feature`
and bind them in
`crates/rstest-bdd-harness-tokio/tests/harness_led_defaults.rs` (159 lines).
`make test` compiles that file under `-D warnings`, so any stray stable marker
breaks the build. That is the end-to-end silence check for `LEM-SILENCE`.

```gherkin
  Scenario: An explicit default-policy downgrade stays silent
    Given a scenario bound with the Tokio harness and the default attribute policy
    When the scenario body checks for a Tokio runtime
    Then a Tokio runtime handle is available

  Scenario: A third-party harness without an attribute policy stays silent
    Given a scenario bound with a local harness and no attribute policy
    When the scenario body runs
    Then the local harness recorded one run

  Scenario: Removing a conflicting attribute override restores the harness default
    Given a scenario bound with the Tokio harness and no attribute policy
    When the scenario body checks for a Tokio runtime
    Then a Tokio runtime handle is available
```

Each scenario is bound with its own `#[scenario(...)]` configuration, so the
bindings are the configurations under test. The third scenario is the
configuration that W003's first fix produces; the existing "Inferred Tokio
policy provides the runtime" scenario already covers the canonical harness
path, so it is not repeated. The scenarios fail before EP-M5 because they have
no bindings ("no step definition" or unbound scenario errors), and pass
afterwards.

- Requirements: `REQ-COMPAT` and `REQ-FIX`, end to end.
- Acceptance:
  `cargo nextest run -p rstest-bdd-harness-tokio --test harness_led_defaults`
  passes all its scenarios, including the three new ones.
- Conformance check: one line per item, as for EP-M2.
- Recovery: revert. EP-M4 remains coherent.
- Remaining gaps: documentation.
- Compatibility decision: none.

### EP-M6: documentation

Outcome: documentation matches the implementation.

- Add `docs/adr-024-evidence-based-harness-and-policy-diagnostics.md`
  following the documentation style guide's ADR template. Its status becomes
  `Accepted` when the implementation lands. It records:
  - the taxonomy and codes;
  - the evidence principle, including the resolvability conjunct;
  - the error-replaces-error rule;
  - the decision not to warn on harness-only third-party usage, scoped to
    v0.6.x and supersedable by roadmap 12.2.3's ADR;
  - the fix-versus-workaround rule;
  - the rule that errors suppress warnings;
  - channel reuse and the opt-out;
  - the ADR-008 prose departure (D14).

  Add an `Amended by ADR-024 (date)` banner and an `## Amendments` entry to
  ADR-017. Index ADR-024 in `docs/contents.md`.
- `docs/rstest-bdd-design.md`:
  - §2.7.3: replace the fallback paragraphs with a "Policy diagnostics"
    subsection that summarizes the five diagnostics and references ADR-024;
  - correct the false remedy at line 1783;
  - §2.7.6.4: list 11.2.3 as delivered.
- `docs/users-guide.md`: rename "First-party adapter fallback diagnostics" to
  "Harness and attribute-policy diagnostics", and give each code an anchored
  entry with a triggering example, the message, and the fix. Also:
  - correct line 942;
  - document the dev-dependency workaround as a workaround;
  - document the module-level opt-out and the nightly limitation;
  - state the renamed-import and `macro_rules!` limitations;
  - state the `-D warnings` consequence.
- `docs/v0-6-0-migration-guide.md`: correct line 340 and update the
  fallback-warning blockquote.
- `docs/developers-guide.md`: replace "Adapter fallback diagnostics" with
  "Policy diagnostics". It must describe:
  - module ownership: `policy_diagnostics` is the only emitter of harness and
    attribute-policy diagnostics, and `selected_hint` is the only
    implementation of precedence;
  - the permitted call sites: the two expansion boundaries;
  - composition through `SharedPolicyDiagnostics`;
  - the recipe for adding a finding: model, fixes, message, marker, `insta`
    case, one stable and one nightly representative;
  - the previously undocumented `TRYBUILD=overwrite` refresh command.
- `docs/CHANGELOG.md` (Unreleased): one entry per code, flagging the
  `-D warnings` consequence and the opt-out.

- Requirements: the AGENTS.md documentation duties and ADR traceability.
- Acceptance: `make markdownlint`, `make nixie`, and `make check-fmt` pass.
- Conformance check: one line per item.
- Recovery: revert the documentation commits.
- Remaining gaps: the roadmap tick.
- Compatibility decision: none.

### EP-M7: completion

Mark roadmap 11.2.3 as done, with a completion note naming this ExecPlan and
ADR-024. Run all gates. Complete `Outcomes & retrospective`, reconcile
discoveries against the conformance basis, and set the status to `COMPLETE`.

## Concrete steps

Run every command from the repository root. Pipe long output through `tee` to
`/tmp/$ACTION-rstest-bdd-$(git branch --show-current | tr / -).out`, as the
commands below do. Run gates one after another, never in parallel, and prefer
delegating full gate runs to the `scrutineer` agent. Never run two gate runs in
one worktree at the same time (the trybuild `wip` race described in the
developers' guide).

```bash
BR=$(git branch --show-current | tr / -)
```

Focused loops:

```bash
cargo nextest run -p rstest-bdd-macros policy_diagnostics 2>&1 | tee "/tmp/unit-rstest-bdd-$BR.out"
cargo nextest run -p rstest-bdd-harness-tokio --test macro_compile 2>&1 | tee "/tmp/trybuild-tokio-rstest-bdd-$BR.out"
cargo nextest run -p rstest-bdd-harness-gpui --all-features --test macro_compile 2>&1 \
  | tee "/tmp/trybuild-gpui-rstest-bdd-$BR.out"
```

To refresh a `.stderr` snapshot after a deliberate message change, run the
following, then review the diff before staging:

```bash
TRYBUILD=overwrite cargo test -p rstest-bdd-harness-tokio --test macro_compile
TRYBUILD=overwrite cargo test -p rstest-bdd-harness-gpui --all-features --test macro_compile
git diff -- crates/rstest-bdd-harness-tokio/tests/fixtures_macros/ crates/rstest-bdd-harness-gpui/tests/fixtures_macros/
```

Nightly representatives are local only (Risk R4). Record the toolchain actually
used:

```bash
rustup run nightly-2026-08-16 rustc --version
TRYBUILD=overwrite cargo +nightly-2026-08-16 test -p rstest-bdd-harness-tokio --test macro_compile 2>&1 \
  | tee "/tmp/trybuild-nightly-tokio-rstest-bdd-$BR.out"
TRYBUILD=overwrite cargo +nightly-2026-08-16 test -p rstest-bdd-harness-gpui --all-features --test macro_compile 2>&1 \
  | tee "/tmp/trybuild-nightly-gpui-rstest-bdd-$BR.out"
```

Milestone gates:

```bash
make check-fmt 2>&1 | tee "/tmp/check-fmt-rstest-bdd-$BR.out"
make lint 2>&1 | tee "/tmp/lint-rstest-bdd-$BR.out"
make test 2>&1 | tee "/tmp/test-rstest-bdd-$BR.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-rstest-bdd-$BR.out"   # documentation milestones
make nixie 2>&1 | tee "/tmp/nixie-rstest-bdd-$BR.out"                 # documentation milestones
```

Expected: each command exits 0. `make test` reports the EP-M0 baseline count
plus the new tests, and zero failures. A gate log certifies only the revision
it ran at, so record `git rev-parse HEAD` before and after each run.

## Validation and acceptance

Red-Green-Refactor evidence is recorded per milestone in `Artefacts and notes`:
the red command and its failing output (a trybuild mismatch or a failing
witness case), the green run, and the gate run after refactoring.

Acceptance criteria, phrased as behaviour:

1. On stable, `policy_w002_missing_policy.rs` produces an error beginning
   with
   ``use of deprecated unit struct `_::RstestBddMissingAttributePolicy` ``. Its
   note starts with `rstest-bdd[W002]`, names
   `rstest_bdd_harness_gpui::GpuiHarness`, and offers
   `attributes = rstest_bdd_harness_gpui::GpuiAttributePolicy`.
2. On stable, `policy_w001_noncanonical.rs` produces two
   `RstestBddUnrecognizedAdapterPath` blocks, one per slot. Each note names the
   canonical Tokio path and does not claim that `#[tokio::test]` was dropped.
3. `policy_w003_conflicting_frameworks.rs` produces one W003 block per
   invocation, spanned at the `attributes` path.
4. `scenarios_policy_e002_runtime_alias_with_harness.rs` produces exactly
   one `rstest-bdd[E002]` error, spanned at the harness path, and no "combining
   `harness` with `async fn`" error.
5. `scenario_default_policy_async_no_harness_rejected.rs` produces three
   `rstest-bdd[E001]` errors and no `rstest` "async test requires" error.
6. `policy_user_executor.rs` and `policy_opt_out.rs` compile and pass under
   `make test`.
7. Every pre-existing `pass` fixture, macrotest snapshot, and
   `harness_defaults` case is unchanged.
8. The three new behavioural scenarios pass.
9. `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
   `make nixie` pass.

Quality criteria:

- Tests: every gate above is green, and the new tests fail before the change
  and pass after it.
- Verification: every obligation in `Verification plan` is discharged, with
  its non-vacuity checks and seeded faults recorded.
- Lint: `make lint` (Clippy `-D warnings`, rustdoc, Whitaker, and the
  file-length check) is clean, with no new suppressions outside the opt-out
  fixture.
- Cost: both `macro_compile` binaries stay within 25% of the EP-M0 warm
  baseline.
- Security: not applicable (no new trust boundary).

## Idempotence and recovery

Every step can be re-run. `TRYBUILD=overwrite` rewrites only `.stderr` files,
which `git diff` reviews and `git checkout -- <file>` restores. Each milestone
ends in a committed, gated plateau, so reverting a milestone's commits returns
the tree to the previous plateau. Stage files by path rather than with
`git add -A`, because `make fmt` can reformat unrelated documents.

## Artefacts and notes

The in-repo scan behind the "17 of 20" figure (read-only, run at `8737d88f`).
It treats a `harness =` path whose last segment is `TokioHarness`,
`GpuiHarness`, or `StdHarness` as first-party:

```bash
python3 - <<'EOF'
import re, subprocess, pathlib
files = subprocess.run(["git", "ls-files", "*.rs"], capture_output=True, text=True).stdout.split()
pat = re.compile(r"#\[scenario\s*\((.*?)\)\s*\]|scenarios!\s*\((.*?)\)\s*;", re.S)
rows = []
for f in files:
    for m in pat.finditer(pathlib.Path(f).read_text(errors="ignore")):
        body = m.group(1) or m.group(2)
        h = re.search(r"\bharness\s*=\s*([\w:]+)", body)
        if h and h.group(1).split("::")[-1] not in ("TokioHarness", "GpuiHarness", "StdHarness"):
            rows.append(bool(re.search(r"\battributes\s*=", body)))
print(len(rows), "non-first-party harness uses;", rows.count(False), "without attributes")
EOF
```

To be filled during implementation:

- the EP-M0 baselines and experiment transcripts;
- red and green transcripts per milestone;
- nightly transcripts with the toolchain version;
- seeded-fault diffs and the tests that reject them;
- generator seeds;
- warm suite durations.

## Interfaces and dependencies

No public interface changes. All new items are `pub(crate)` or narrower in
`rstest-bdd-macros`. The shapes below are the EP-M4 end state; EP-M1 to EP-M3
introduce subsets (D19).

In `crates/rstest-bdd-macros/src/codegen/policy_diagnostics/model.rs`:

```rust
use rstest_bdd_policy::TestAttributeHint;

/// First-party frameworks that can be named non-canonically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Framework { Tokio, Gpui }

/// Classification of one supplied `harness` or `attributes` slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlotClass { Absent, Recognized(TestAttributeHint), Evidenced(Framework), Foreign }

/// Abstract inputs for one expansion boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PolicyInputs {
    Scenario { harness: SlotClass, attributes: SlotClass, is_async: bool, has_user_test_attr: bool },
    Scenarios { harness: SlotClass, attributes: SlotClass, runtime_alias: bool },
}

/// At most one of each finding; duplication is unrepresentable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PolicyFindings {
    pub(crate) harness_slot: Option<SlotWarning>,     // W001 or W002
    pub(crate) attributes_slot: Option<SlotWarning>,  // W001
    pub(crate) conflicting_frameworks: Option<(Framework, Framework)>, // W003
    pub(crate) missing_executor: bool,                // E001
    pub(crate) runtime_alias_with_harness: bool,      // E002
}

/// Pure decision: no `syn`, no emission.
pub(crate) fn diagnose(inputs: PolicyInputs) -> PolicyFindings;
```

In `resolution.rs`, the single precedence implementation (D15):

```rust
pub(crate) fn selected_hint(harness: SlotClass, attributes: SlotClass, attribute_runtime: RuntimeMode)
    -> TestAttributeHint;
```

In `classify.rs`, with an injected resolver (D16):

```rust
pub(crate) trait CrateResolver {
    /// The root identifier under which the package is a direct dependency, if it is one.
    fn resolved_root(&self, package_name: &str) -> Option<proc_macro2::Ident>;
}
pub(crate) fn classify_harness_path(path: &syn::Path, resolver: &dyn CrateResolver) -> SlotClass;
pub(crate) fn classify_policy_path(path: &syn::Path, resolver: &dyn CrateResolver) -> SlotClass;
```

In `mod.rs`, `SharedPolicyDiagnostics` replaces `SharedAdapterResolutions` and
has a constructor. It keeps the split between `resolve`, which is pure, and
`emit_diagnostics`, which only the boundary calls. It also gains:

- the classified *explicit* slots (D21);
- the spans of both paths;
- the `PolicyFindings`;
- the per-slot `HarnessApiResolution`, derived from the classification.

Field and function names may be refined during EP-M1 if the refactor exposes a
better fit. Any change must keep `diagnose` and `selected_hint` pure and free of
`syn` types, and must be recorded in `Decision log`.

Dependencies: `rstest-bdd-policy` (existing; public constants only), `syn`,
`quote`, `proc-macro2`, and `proc-macro-crate` (all existing). Test-only:
`rstest`, `proptest`, `googletest`, `pretty_assertions`, and `insta` (all
existing dev-dependencies of `rstest-bdd-macros`), plus `trybuild` and
`rustversion` (existing in the harness crates).

## Revision notes

- 2026-10-15: initial draft, written after four-agent reconnaissance and
  prior-art research.
- 2026-10-15: revised after the community-of-experts review (three panels,
  all six lenses; verdict: revise). Changes:
  - "Missing" now includes W002, the shape the roadmap's fix text describes.
  - E001 mirrors `rstest`'s `test_attr` form.
  - Consequence clauses derive from emitted attributes, not selected hints.
  - The `scenarios!` alias plus a harness is recognized as an existing error
    and becomes E002; the stable alias-plus-attributes warning is dropped.
  - Precedence is implemented once; the crate resolver is injected and
    suggested roots are rename-aware.
  - Diagnostics carry codes and headline marker names.
  - Fixtures are consolidated, and EP-M5 extends an existing feature.
  - Three false documentation statements are scheduled for correction.

  Effect on remaining work: EP-M0 gains three experiments; EP-M2 gains W002;
  EP-M3 swaps the alias warning for E002; C11 now keeps the plan out of the
  parser files.
