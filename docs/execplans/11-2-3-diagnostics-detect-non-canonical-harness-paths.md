# Diagnose non-canonical harness paths and missing or ambiguous attribute policies (roadmap 11.2.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS (EP-M0). Approved 2026-11-26; implementation started the
same day. Revised after community-of-experts review; observations re-verified
against `origin/main` at `0df7fbec` on 2026-11-24; restacked on pull request
986 at `5dbc2fb5`, over `main` at `6308b9af`, on 2026-11-26.

Approval: approved by the user on 2026-11-26, who instructed implementation to
begin and to carry the pull request through babysitting to equilibrium.

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

- C1 (semver): the change is additive, as the Phase 11 preamble requires, even
  though `main` is now at 0.7.0 (D26). No public item, macro argument, or
  accepted syntax is removed or renamed in any published crate (`rstest-bdd`,
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
  departs from the prose of ADR-0008, the code is preserved and the departure
  is recorded (decision D14).
- C5 (ADR-0017 contracts): diagnostics are computed purely and emitted at the
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
  `8737d88f` and still at `0df7fbec`, so its net line count must not grow. The
  boundary files have grown on `main` and have little headroom at `0df7fbec`:
  `macros/scenario/mod.rs` (356 lines), `macros/scenarios/mod.rs` (367), and
  `macros/scenarios/test_generation/mod.rs` (374). Boundaries therefore gain
  call sites only; logic goes into `policy_diagnostics`.
- C8 (no suppressions): no new `#[allow]` or `#[expect]` lint suppressions in
  production or test code. The trybuild fixture that demonstrates a user's
  opt-out is the sole exception, because the suppression is the behaviour under
  test. It must be a trybuild fixture: the workspace denies Clippy's
  `allow_attributes`, so the same `#[allow]` in a linted test target would fail
  `make lint` (D25).
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

- C12 (test conventions, AGENTS.md at `0df7fbec`): every `assert!` family
  invocation carries a diagnostic message; `insta` snapshots are paired with
  semantic assertions; tests that use fallible fixtures return `Result`; no new
  test relies on `#[serial]` or another process-wide lock, or changes the
  process environment. New trybuild cases therefore join the existing timed
  test functions (D27).

- C13 (cognitive complexity, `dylint.toml` at `6308b9af`): `make lint` gates
  cognitive complexity with Whitaker's `macro_aware_cognitive_complexity` at a
  maximum of 9, replacing Clippy's former threshold of 12. Every new or edited
  function in the macro crate must score 9 or less; extract helpers rather than
  suppress the lint (D28).

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
  framework selectors. Severity: medium. Likelihood: low. A scan at `0df7fbec`
  found no W003, E002, or deep-nesting shape in the repository. The only
  `Evidenced` paths are in the nine fallback fixtures, two of which (both GPUI)
  are W002 shapes that already warn today. Mitigation: each warning fires only
  when a consequence has demonstrably occurred, or when two explicit selectors
  contradict each other. Each is documented with its fix and its opt-out
  (decision D10) in the users' guide and in `docs/CHANGELOG.md`.
- Risk R2: open pull requests touch neighbouring files. All 22 open pull
  requests were scanned on 2026-11-24. Those that overlap are:
  - drafts #783 (policy hint tables) and #784 (macro argument parsers), both
    unchanged since 2026-09-15. C9 and C11 keep this plan out of their code
    files. #783 also edits ADR-0008 at its old path, the design document, and
    `docs/developers-guide.md`, which EP-M6 rewrites;
  - #930 (empty `scenarios!` results) rewrites part of
    `macros/scenarios/mod.rs`, the `scenarios!` boundary that EP-M3 edits. E002
    must return before any empty-result check that #930 adds;
  - #927 (outline case indexes) edits `codegen/scenario/mod.rs`, home of
    `reject_async_harness`;
  - #925 (outline data tables) edits `codegen/scenario/helpers.rs` and its
    tests;
  - #936 edits `macros/scenarios/tests/mod.rs`;
  - draft #777 (poison recovery) edits both harness crates' manifests, which
    matters only if C10's permitted dev-dependency is needed;
  - #810 and #775 bump `rstest` in fixture and workspace lockfiles; see Risk R5;
  - #926 to #931 each edit `docs/CHANGELOG.md`, and several edit
    `docs/users-guide.md`.

  #942 and #773, listed here previously, have merged. Severity: low.
  Likelihood: high. Mitigation: rebase onto `origin/main` at the start of each
  milestone and re-run the `SharedAdapterResolutions` call site inventory. If a
  Markdown merge is needed, verify it against a plain `git merge-file` (the
  Weave merge driver can relocate blocks).
- Risk R3: trybuild `.stderr` snapshots are toolchain-sensitive, and
  `rust-toolchain.toml` floats on `stable`. Severity: low. Likelihood: medium.
  Mitigation: follow the existing fixture idiom. Reuse the normalizers in
  `crates/rstest-bdd/tests/trybuild_macros/wrappers.rs` only if drift appears.
- Risk R4: CI has no nightly test lane. The panel reports that the matrix is
  stable Linux plus two stable Windows lanes (`.github/workflows/ci.yml`), so
  the native channel is verified only locally. Severity: low. Likelihood:
  certain. Mitigation: EP-M0 re-confirms the lane list. Every milestone that
  touches a nightly fixture records the local nightly transcript and the exact
  toolchain in `Artefacts and notes`. Re-confirmed at `0df7fbec`: CI installs a
  nightly only for `rustfmt` and the published-GPUI end-to-end fixture.
- Risk R5: a future `rstest` release could relax its rule for asynchronous
  tests. The rule is currently "an explicit `#[test_attr(...)]`, or any
  attribute whose last path segment is `test`". E001 mirrors that rule; if
  `rstest` relaxed it, E001 would reject a program that `rstest` accepts,
  violating C2. Severity: high. Likelihood: low. `rstest_macros` 0.26.1 and
  0.27.0 have byte-identical `parse/rstest/test_attr.rs` files. 0.28.0 differs
  there only in the layout of its unit tests. All three keep the same error
  text in `error.rs` (registry sources compared on 2026-11-24; 0.26.1 is
  locked). Mitigation: the axiom `AX-RSTEST-ASYNC` is guarded by canary
  fixtures that compile plain `rstest` tests. One pins the rejection; the other
  pins the acceptance of the `test_attr` form. A dependency bump that changes
  either fails loudly.
- Risk R6: the conflicting-framework fixture may need both adapter crates
  resolvable in one trybuild scratch crate, and may produce extra rustc errors
  from combining the GPUI attribute with the Tokio harness. Severity: low.
  Likelihood: medium. Resolved by EP-M0 experiment 1 (D29): the GPUI suite with
  the Tokio harness as a dev-dependency produced only the fixture's own
  sentinel.
- Risk R7: harness-crate trybuild registration files and
  `crates/rstest-bdd/tests/trybuild_macros.rs` (373 lines at `0df7fbec`) may
  approach the 400-line cap. Severity: low. Likelihood: low; the plan adds five
  stable fixtures (four `compile_fail`, one `pass`), two canaries, and at most
  two nightly fixtures, all in the harness crates' suites. Mitigation: if a
  file would exceed 380 lines, split the registration into a child module first.
- Risk R8: rustc may not run the `deprecated` lint when the same expansion
  also contains a `compile_error!`. Stable warnings would then vanish beside
  errors while nightly still printed them. Severity: low. Likelihood: medium.
  Resolved by EP-M0 experiment 3: on stable, both the `compile_error!` and the
  generated marker were reported. Decision D9's suppression is therefore a
  deliberate message-quality choice, not a workaround for uncertain rustc
  behaviour.
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
- [x] (2026-11-23) Rebased onto `origin/main` at `4c21753d` (25 commits, no
  conflicts) and revised (D22, D23). That revision claimed that every
  revision-anchored observation had been re-measured. It had not: see the
  2026-11-24 entry under `Surprises & discoveries`.
- [x] (2026-11-24) Rebased onto `origin/main` at `0df7fbec` (11 further
  commits, no conflicts). The code anchors, documentation anchors, open pull
  requests, gate commands, and convention files listed under the 2026-11-24
  discoveries were re-measured; see D24 to D27.
- [x] (2026-11-26) Restacked onto #986 (`raise-nested-preparation-limits` at
  `5dbc2fb5`), which sits on `origin/main` at `6308b9af` (four further `main`
  commits, #980 to #984; no conflicts). The plan's commits are unchanged apart
  from this revision. Only #980 bears on the plan, through C13 and D28. The
  user confirmed D26 and accepted CI as the gate evidence for this plan-only
  pull request.
- [x] (2026-11-26) Plan approved by the user, who instructed implementation to
  begin and to carry the pull request through babysitting to equilibrium.
- [x] (2026-11-27) EP-M0 complete: baselines recorded at `b55bd2c1`, and the
  three experiments answered (see `Surprises & discoveries` and D29 to D33).
  The throwaway fixtures and the one manifest change were reverted; the
  dev-dependency returns with the EP-M3 fixture that needs it.
- [x] (2026-11-27) EP-M1 implemented: `codegen::adapter_fallback` is replaced by
  `codegen::policy_diagnostics/`; `test_attrs::resolve_attribute_policy`
  delegates to `selected_hint`; the classifier takes an injected
  `CrateResolver`. Every fixture, `.stderr`, macrotest snapshot and
  `harness_defaults` case is unchanged, and both harness `macro_compile` suites
  pass against the untouched snapshots (D34 to D36).
- [x] (2026-11-27) EP-M1 gates green at `e0f8cfec`: `make check-fmt` (exit 0),
  `make lint` (all 11 steps, exit 0, including Clippy, rustdoc, Whitaker and
  the seven script checks), `make typecheck` (exit 0) and `make test` (exit 0:
  2598 and 838 nextest cases passed, doctests 180 passed, 7 fixture lockfiles
  current, pytest 398 passed). Windows CI is left to the pull request's hosted
  lanes.
- [x] (2026-11-27) EP-M2 implemented: W001 and W002. The classifier's
  evidence rule widens to any non-root crate identifier, the model gains the
  two shape variants and W002, and both channels render the same code-prefixed
  text. Fixtures: `policy_w001_noncanonical` (two W001 blocks, no attribute
  claim), `policy_w002_missing_policy` (W002 plus the W001 attribute
  consequence), `policy_opt_out` (`pass`, the documented opt-out). The nine
  existing `.stderr` files were regenerated on stable and on
  `nightly-2026-08-16` (D37 to D39).
- [x] (2026-11-27) EP-M2 gates green at `61f1eacc`: `make check-fmt`,
  `make lint` (all 11 steps), `make typecheck` and `make test` (2627 and 838
  nextest cases, 180 doctests, 7 lockfiles, 398 pytest) all exit 0. Two
  Whitaker rounds were needed: the first flagged cognitive complexity in
  `attribute_runtime` and the convergence tests plus an `unwrap_or_else` panic,
  all fixed by extraction rather than suppression (D40).
- [x] (2026-11-27) EP-M3 implemented: W003 and E002. The model gained the
  implied-family rule (including the alias), the conflict finding and the alias
  error; the renderer moved to `policy_diagnostics/render.rs` when the module
  passed Whitaker's 400-line gate. `policy_w003_conflicting_frameworks` pins
  one W003 block spanned at the `attributes` path and
  `scenarios_policy_e002_runtime_alias_with_harness` pins exactly one E002
  error spanned at the harness path; a nightly twin pins the native W003
  warning. The alias-plus-harness branch of the deprecation warning is gone
  (D41, D42).
- [x] (2026-11-27) EP-M4 implemented: E001 replaces `rstest`'s generic
  "async test requires" error with a code-prefixed error spanned at the
  `attributes` path, offering its three remedies and naming the third-party
  limitation when the policy is foreign. The existing rejection fixture now
  carries three blocks (default, foreign, non-canonical policy); two
  `rstest`-only canaries pin `AX-RSTEST-ASYNC`, and `policy_user_executor.rs`
  keeps a user-written `#[tokio::test]` and `#[test_attr(...)]` compiling. The
  model split `model.rs` into `model.rs` plus `inputs.rs` when Whitaker
  measured the module at 412 lines (D43).
- [x] (2026-11-27) EP-M5 implemented: the three drafted scenarios are in
  `harness_led_defaults.feature` and bound in `harness_led_defaults.rs` — the
  explicit default-policy downgrade, a local third-party harness with no
  policy, and the harness default restored after removing an override. That
  binary reports 9 passed under `-D warnings`, so any stray marker would
  break it.
- [ ] EP-M6: documentation (ADR-0029 written; the guides are being updated).
- [ ] EP-M4 and EP-M3 full gates: focused evidence is recorded (565 unit
  tests, both `macro_compile` suites on stable and on `nightly-2026-08-16`, the
  new `policy_user_executor` integration test, Clippy, and Whitaker run to
  completion on the full workspace). `make lint`, `make check-fmt` and
  `make markdownlint` were run at the EP-M3 tree; run all four full gates at
  the pushed head `222e1355` before calling the milestone complete.
- [ ] EP-M6: documentation, ADR-0029, CHANGELOG.
- [ ] EP-M7: roadmap tick, final gates, plan marked `COMPLETE`.

## Surprises & discoveries

- Observation: a warning for every third-party harness without
  `attributes = ...` would break the repository's own tests, and would offer no
  useful fix. Evidence: a scan of tracked `.rs` files at `8737d88f`, re-run
  with the same result at `0df7fbec`, found 20 non-first-party `harness =`
  uses, 17 of them without `attributes =` (the command is recorded in
  `Artefacts and notes`). `make test` compiles all of them under
  `RUSTFLAGS="-D warnings"`. The panel spot-checked `RecordingHarness`,
  `CountingHarness`, and `MetadataCapturingHarness`: they are synchronous
  harnesses that need no native attribute. The only fix such a warning could
  suggest is a custom `attributes = ...`, which changes nothing because the
  macros cannot evaluate unknown policies. Impact: decision D2.
- Observation: the existing ADR-0017 message offers three remedies, and none
  of the other two removes the warning. Adding `rstest-bdd-harness` as a direct
  dev-dependency makes the fallback *compile*, but `fallback_candidate` in
  `crates/rstest-bdd-macros/src/codegen/adapter_fallback.rs` never consults
  that dependency, so the warning stays. "Ensure the crate is directly
  resolvable" is always already true when the warning fires, because
  resolvability is one of the warning's own conditions. Impact: the
  fix-convergence lemma `LEM-FIX-CONVERGES` separates *fixes*, which remove a
  finding, from *workarounds*, which only make code build. Messages offer only
  fixes. The workaround moves to the users' guide (decision D5). Three
  documents repeat the false remedy (line numbers at `0df7fbec`):
  `docs/users-guide.md` lines 1172-1173 ("to get the same attribute
  recognition"), `docs/rstest-bdd-design.md` lines 1794-1795 ("for the macro to
  resolve it"), and `docs/v0-6-0-migration-guide.md` lines 339-341. EP-M6
  corrects all three. Two nearby passages are true and stay, because they
  describe the workaround rather than a cure: `docs/users-guide.md` lines
  1196-1200 and `docs/v0-6-0-migration-guide.md` lines 334-335.
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
- Observation: the unit test
  `generated_scenario_metadata_uses_portable_feature_separators` in
  `macros/scenarios/test_generation/tests/mod.rs` builds
  `SharedAdapterResolutions` with a struct literal. At `8737d88f` it was under
  `#[cfg(windows)]`, so the Linux gates never compiled it (panel: Pandalump).
  Commit `5a1bd0d6` (#686) removed that gate, so at `0df7fbec` the literal
  (line 41) compiles on every platform. Impact: EP-M1 still gives the
  replacement type a constructor and updates this site, but the Linux
  `make test` now proves it compiles; Windows CI is no longer needed as special
  evidence.
- Observation (2026-11-23 rebase): `main` restructured scenario emission
  without changing policy behaviour. `codegen/scenario/mod.rs` now calls
  `emit::emit_scenario_test` (new `codegen/scenario/emit.rs`), which in turn
  calls `boundary::finalize_scenario_signature`. `ScenarioConfig` gained a
  `scope` field for step-library scopes (#686). The fallback message text in
  `adapter_fallback.rs` changed only in its `concat!` layout, and the nine
  pinning `.stderr` files are the same nine. Impact: the plan's seams
  (`resolve_attribute_policy`, `generate_test_attrs_with_boundary`,
  `reject_async_harness`, `emit_runtime_deprecation_warning`) are unchanged.
  The planned `policy_diagnostics/emit.rs` is a different module from
  `codegen/scenario/emit.rs`; module docs must say so.
- Observation (2026-11-23 rebase): the Tokio and GPUI `macro_compile`
  suites now gate every timed test on a nested-fixture preparation stamp
  (`macro_compile/preparation.rs`, issue #910). Under nextest an unprepared or
  differently fingerprinted tree fails fast, and the fingerprint includes
  `RUSTFLAGS`, so a hand-typed `cargo nextest run` filter is refused. Impact:
  focused loops use `make prepare-nested-fixtures` and
  `make nextest-run EXPR=...` (D23). Any new timed `#[test]` added to these
  binaries, including nightly representatives, must begin with
  `preparation::tree().ensure_prepared(preparation::prepare)?`, as the existing
  ones do. New fixtures need no preparation change, because the prepared
  dependency graph comes from the manifest rather than from the fixture list.
- Observation (2026-11-24 rebase): the 2026-11-23 revision overstated its
  checks. It said that the W002, W003, and deep-nesting scan had been re-run,
  but only the "17 of 20" scan had been. It did not compare `AGENTS.md`,
  `clippy.toml`, or the workspace version. It left two hand-typed
  `cargo nextest run` commands in place, and it deferred ADR-0008 and ADR-0017
  path changes. Evidence: this revision's re-measurement. Impact: Risk R1, C8,
  C12, and D24 to D27.
- Observation (2026-11-24 rebase): the existing fixtures
  `scenario_harness_gpui_alias_fallback_warning.rs` and
  `nightly_gpui_adapter_fallback_warning.rs` are W002 shapes (an `Evidenced`
  GPUI harness without `attributes`). Impact: EP-M2 regenerates both as W002,
  so the stable and nightly W002 texts are already pinned. EP-M2 adds the
  separate nightly twin only if it pins something the existing fixture does not.
- Observation (2026-11-24 rebase): the workspace has denied Clippy's
  `allow_attributes` since before this plan was drafted, so the planned
  integration test with a module-level `#[allow(deprecated)]` could never have
  passed `make lint`. Impact: D25.
- Observation (2026-11-24 rebase): the workspace version is 0.7.0, not 0.6.x,
  and `docs/v0-7-0-migration-guide.md` covers the unreleased line. Roadmap
  11.2.2 (#773) shipped there. Impact: D26.
- Observation (2026-11-24 rebase): `make test` now runs
  `prepare-nested-fixtures` before each of its two nextest passes (#979), with
  that pass's own `PREPARE_FLAGS`. Impact: milestone gates need no separate
  preparation step. Filtered loops still do (D23), and the default
  `PREPARE_FLAGS` covers both harness suites, because the second pass builds
  only `rstest-bdd`. The macro crate's policy seams are unchanged: between
  `4c21753d` and `0df7fbec` the files under `codegen/scenario/` changed only in
  documentation comments and blank lines.
- Observation (2026-11-24 rebase): the `scenarios!` boundary body now lives in
  `expand_scenarios_tokens`, which is testable without a compiler session
  (`macros/scenarios/tests.rs`). It calls `emit_runtime_deprecation_warning`
  before any feature file is read. Impact: EP-M3 emits E002 there, and can unit
  test it directly.
- Observation (2026-11-24 rebase): `rstest_bdd::prelude` (#773) re-exports no
  harness adapter or attribute policy, so it adds no new non-canonical path.
  Impact: none.
- Observation (EP-M0 experiment 1, 2026-11-27): the GPUI suite hosts the W003
  fixture. A throwaway `compile_fail` fixture combining
  `harness = rstest_bdd_harness_gpui::GpuiHarness` with
  `attributes = rstest_bdd_harness_tokio::TokioAttributePolicy` compiled with
  only its own `compile_error!` sentinel in stderr — no resolution errors — once
  `rstest-bdd-harness-tokio` was a dev-dependency of
  `rstest-bdd-harness-gpui`. The suite reported 3 tests passed. The Tokio suite
  was not tried with GPUI as a dev-dependency, because that would put the
  vendored GPUI stack in a second trybuild tree for no extra evidence. Impact:
  D29; the manifest change is C10's single permitted one.
- Observation (EP-M0 experiment 2, 2026-11-27): the alias-plus-harness
  rejection is *reported once*, not once per scenario. The fixture's token
  stream carries one `reject_async_harness` error per generated scenario, but
  every one is `Span::call_site()` on the same `scenarios!` invocation with
  identical text, so rustc collapses them into one diagnostic. Impact: D33 and
  the E002 acceptance criterion, which must be phrased as "one accurate error
  spanned at the harness path", not as a reduction in error count.
- Observation (EP-M0 experiment 3, 2026-11-27): the stable channel and errors
  are independent. A fixture whose expansion emitted both a `compile_error!`
  and the generated `#[deprecated]` marker, under `#![deny(deprecated)]`,
  reported both errors. Separately, a `#[allow(deprecated)]` on the enclosing
  `mod` silenced the marker under a crate-level `#![deny(deprecated)]`, and the
  `pass` fixture succeeded. Impact: Risk R8 is resolved for stable — rustc does
  run the lint beside an error — so decision D9's suppression is a deliberate
  message-quality choice rather than a workaround for uncertain rustc
  behaviour. Decision D10's documented opt-out works as promised.
- Observation (EP-M0 operations, 2026-11-27): the nested-fixture fingerprint
  hashes `Cargo.toml`, `Cargo.lock`, `crates/`, and `vendor/`, so *any* edit
  under `crates/` — including writing a new `trybuild` `.stderr` file — makes
  every harness family's stamp stale. A `TRYBUILD=overwrite` run for one suite
  therefore invalidates the other suite's stamp. Impact: batch fixture edits,
  then prepare once, then run each suite; do not interleave snapshot writes
  with a second suite's run.
- Observation (EP-M1, 2026-11-27): `quote_spanned!` does not re-span an
  interpolated `Ident`. Building the marker name with
  `Ident::new(name, Span::call_site())` and interpolating it moved every stable
  warning's span to the macro call site and added an "originates in the
  attribute macro" note, which the trybuild snapshots rejected. The span must
  be given to `Ident::new`. Impact: the emitters construct each marker ident
  from the warning's own span; the six stable fallback fixtures then matched
  byte for byte.

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
  `attributes = ...`. Rationale: ADR-0008 declares harness-only usage valid.
  The in-repo instances are correct synchronous harnesses, and under
  `-D warnings` a warning is a build failure for a patch upgrade (C1, C6). The
  only fix such a warning could offer is a custom policy, which the macros
  cannot evaluate. Roadmap 12.2.3 owns the policy-extension question; ADR-0029
  states that 12.2.3's ADR may supersede this rule. An opt-in strict mode was
  considered and rejected: a new macro argument breaches the Interface
  tolerance, and a Cargo feature unifies across a build, so one crate enabling
  it would break a neighbour that builds with `-D warnings`. Date/Author:
  2026-10-15 / Claude (panel: Wafflecat).
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
- Decision D14: record without changing a departure between ADR-0008's prose
  and the code. ADR-0008 says that the alias does not change attribute
  selection, but in code `attribute_runtime` makes the alias select the Tokio
  hint when no first-party harness is recognized. Every configuration in which
  that departure is observable is now an E002 error, so preserving the code
  (C4) changes nothing visible. ADR-0029 records the departure. Date/Author:
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
  unrepresentable. Rationale: ADR-0004 governs ownership of the shared
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
- Decision D22 (superseded by D24): the new ADR is not ADR-0024, because that
  number was taken on `main`. The provisional number 0028 was chosen to survive
  two open pull requests. Date/Author: 2026-11-23 / Claude.
- Decision D23: focused test loops go through the Makefile's prepared-fixture
  targets instead of hand-typed `cargo nextest run` filters. Rationale: on
  `main` the stamp fingerprint includes `RUSTFLAGS`, so an ad hoc filter is
  refused as unprepared (developers' guide, "The preparation Make targets and
  their roles"). `TRYBUILD=overwrite cargo test ...` remains valid because plain
  `cargo test` prepares lazily under its own lock. `make typecheck` is added
  to the milestone gate list because it is one of the repository's commit
  gates. Date/Author: 2026-11-23 / Claude.
- Decision D24: the new ADR is ADR-0029, at
  `docs/adrs/adr-0029-evidence-based-harness-and-policy-diagnostics.md`.
  Rationale: #942 moved every ADR to `docs/adrs/adr-nnnn-*.md` and made
  ADR-005a ADR-0027, and #773 took ADR-0028. On 2026-11-24 no open pull request
  and no fetched branch claimed 0029. EP-M6 confirms this before creating the
  file, because published ADRs are never renumbered. Date/Author: 2026-11-24 /
  Claude.
- Decision D25 (revises D20's third bullet for one case): the documented opt-out
  is pinned by a trybuild `pass` fixture, as D10 says, and not by an
  integration test. Rationale: the workspace denies `allow_attributes`, and
  trybuild fixtures are not Clippy targets. The fixture sets a crate-level
  `#![deny(deprecated)]`, so it fails without the module-level opt-out;
  trybuild strips `RUSTFLAGS`, so `-D warnings` alone would not prove that. The
  alternative, `#[expect(deprecated)]`, is unfulfilled on nightly, where no
  deprecated item is generated. `policy_user_executor.rs` needs no suppression
  and stays an integration test. Date/Author: 2026-11-24 / Claude.
- Decision D26: keep the roadmap's additive constraint and release on the
  current unreleased line. Rationale: `main` already carries 0.7.0 breaking
  changes, so a v0.6.1 release of this work would need a backport that the
  roadmap does not request. C1 to C3 are stricter than 0.7.0 requires, and are
  kept. User-facing guidance goes to `docs/v0-7-0-migration-guide.md`; the
  v0.6.0 guide receives only the correction of its false statement.
  Date/Author: 2026-11-24 / Claude. Confirmed by the user on 2026-11-25: target
  v0.7.0, with no v0.6.1 backport.
- Decision D27: new trybuild cases join the existing timed test functions, and
  the plan adds no new `#[serial]` test. Rationale: AGENTS.md no longer accepts
  process-wide locks as a fallback. If a nightly function's name stops
  describing its contents, rename it. Date/Author: 2026-11-24 / Claude.
- Decision D28: new and edited functions are held to the macro-aware cognitive
  complexity gate of 9 (C13), and the plan's local test commands run under pull
  request #986's `local` nextest profile. Rationale: #980 moved the complexity
  gate from Clippy (12) to Whitaker (9), so a helper sized against the old
  threshold would fail `make lint`. #986 makes every Makefile nextest recipe
  select `--profile local`, whose longer limits let `make test` finish on a
  loaded development host; CI keeps the default profile. Date/Author:
  2026-11-26 / Claude.
- Decision D29 (EP-M0): the W003 fixture is hosted by the GPUI suite, and
  `rstest-bdd-harness-tokio` becomes a dev-dependency of
  `rstest-bdd-harness-gpui` — C10's single permitted manifest change.
  Rationale: a fixture that names both adapter crates needs both in the
  generated `trybuild` crate's dependency graph, which `trybuild` builds from
  the source manifest's dependencies and dev-dependencies. The GPUI suite
  already pays for GPUI; adding the Tokio harness is the cheap direction, and
  experiment 1 confirmed it produces no unrelated errors. Date/Author:
  2026-11-27 / Claude.
- Decision D30 (EP-M0): W003's "remove `attributes`" fix is modelled as the
  composite `attributes := Absent; harness := canon(harness)`, and its message
  says so when the harness path is non-canonical. Rationale: with a
  non-canonical harness (`Evidenced`), removing `attributes` alone lands
  squarely in W002's antecedent, so the fix would *introduce* a finding and
  falsify `LEM-FIX-CONVERGES`. Working the lemma over the model found this
  while implementing the obligation; the composite is the union of the two
  fixes the user is already being told to make. For a recognized harness or the
  alias, `canon` is the identity and the wording is the drafted "Remove
  `attributes = ...`". Date/Author: 2026-11-27 / Claude.
- Decision D31 (EP-M0, refines D11): `has_user_test_attr` arrives in EP-M2, and
  every "was not applied" claim — W001's attribute clause and W002 in its
  entirety — additionally requires `!has_user_test_attr`. Rationale: the model
  computes emitted attributes from the resolved policy, while the real code
  generator drops an attribute the user already wrote. Without this conjunct, a
  user who writes `#[gpui::test]` by hand beside a non-canonical GPUI harness
  would be told the attribute "was not applied" when it is present, violating
  `LEM-CONSEQUENCE-TRUE`. One flag is deliberately conservative: it silences
  the claim whenever *any* executor attribute is user-written, rather than
  growing a per-framework pair the plan does not need. Date/Author: 2026-11-27
  / Claude.
- Decision D32 (EP-M0, corrects `LEM-CARDINALITY`'s non-vacuity witness): the
  three-warning witness is `W001(harness) + W001(attributes) + W003`, not the
  plan's `W001(attributes) + W002 + W003`. Rationale: W002 requires
  `attributes = Absent` and W003 requires `family(attributes)` to be defined,
  so no state can carry both; the drafted witness is unrepresentable. The
  replacement exists (non-canonical harness and non-canonical, conflicting
  policy) and pins the same three-marker count. Date/Author: 2026-11-27 /
  Claude.
- Decision D33 (EP-M0): E002's acceptance is "one accurate error spanned at the
  harness path, replacing today's message", not "fewer errors". Rationale:
  experiment 2 showed rustc already collapses the identical per-scenario
  `compile_error!` diagnostics into one, so the observable defect is the
  message and span, not the count. `LEM-ERR-REPLACES` is unaffected: the
  program still fails to compile today. Date/Author: 2026-11-27 / Claude.
- Decision D34 (EP-M1): `codegen/mod.rs`'s unit-test module moves to
  `codegen/tests/mod.rs` (`#[path = "tests/mod.rs"]`), matching the
  `codegen/scenario/tests` layout. Rationale: threading the resolver and adding
  the derived-diagnostic case took the file past 400 lines (C7). No test case
  or assertion changed; only its file and the resolver argument sites did.
  Date/Author: 2026-11-27 / Claude.
- Decision D35 (EP-M1): `CrateResolver` answers two questions, and they are not
  interchangeable. `resolved_root` decides recognition of renamed roots and
  bare imports; `is_direct_dependency` decides whether syntactic evidence is
  admissible. The unit-test resolver reports `None` for the first and `true`
  for the second, which is exactly the split the pre-refactor `cfg(test)` stubs
  had (`try_resolve_crate_path` was real Cargo state, while
  `fallback_crate_is_resolvable` was forced true). Collapsing them into one
  method made five single-segment-import tests and two `harness_defaults` cases
  recognize imports that the real macros cannot, which the unit suite caught.
  Date/Author: 2026-11-27 / Claude.
- Decision D43 (EP-M4): `PolicyInputs` moves to `inputs.rs` when the model
  module reached 412 lines. Rationale: C7's module-length gate again, split
  along the seam between "what the boundary knows" and "what the decision
  concludes". Date/Author: 2026-11-27 / Claude.
- Decision D41 (EP-M3): E002's fix is composite for the same reason D30's is.
  `LEM-FIX-CONVERGES` found that clearing the alias from
  `(Evidenced(GPUI), Absent, alias)` reveals W002. Because errors suppress
  warnings (D9), the harness slot's own fix is never shown beside E002, so the
  E002 message names the canonical harness path itself when the harness is
  non-canonical. Date/Author: 2026-11-27 / Claude.
- Decision D42 (EP-M3): the renderer moves from `policy_diagnostics/mod.rs` to
  `render.rs` when Whitaker's module-length lint measured the module at 407
  lines. Rationale: C7's 400-line cap applies to the module tree, not only to
  one file, so the split is the required remedy rather than an optional tidy.
  Date/Author: 2026-11-27 / Claude.
- Decision D40 (EP-M2): the Whitaker findings were answered by extraction,
  not by suppression: `attribute_runtime` asks one named question
  (`implies_tokio_runtime`), and the convergence checks share a per-state
  helper with an explicit `Tally`. Rationale: C8 forbids new suppressions, and
  the clustered-conditional lint was pointing at real nesting. Date/Author:
  2026-11-27 / Claude.
- Decision D37 (EP-M2): a suggested canonical path renders the *written*
  terminal type under the resolver's root, rather than a fixed first-party type
  name. Rationale: it keeps the suggestion exact for a path that named, say,
  `TokioAttributePolicy` in the `harness` slot, and `LEM-FIX-CANONICAL` is
  pinned by extracting every backticked path from the rendered messages and
  classifying it under the default and renamed resolvers. The plan's "(for
  W001) unresolvable-adapter resolver" row is unreachable: evidence requires
  the adapter crate to be resolvable, so W001 cannot fire without one. The
  unresolvable case is covered instead by `classify_props`. Date/Author:
  2026-11-27 / Claude.
- Decision D38 (EP-M2): the exhaustive wording matrix is an `insta` file
  snapshot (`wording_matrix`), not one assertion per message. Rationale:
  diagnostics are user-visible text with a stable contract, which is what
  snapshots are for (AGENTS.md), and the matrix covers code, slot, framework,
  signature shape, user-attribute suppression and a renamed resolver in one
  reviewable artefact. `trybuild` still pins one representative per channel and
  the spans. Date/Author: 2026-11-27 / Claude.
- Decision D39 (EP-M2): W002 keeps the plan's draft text and does not repeat
  W001's base-harness-API clause; W001 always states it. Rationale: W002's
  finding is the lost policy, and its own second fix already has to explain
  that W001 survives, so repeating the fallback would bury the remedies.
  Date/Author: 2026-11-27 / Claude.
- Decision D36 (EP-M1): the boundary type is constructed by `for_scenario`,
  `for_scenarios`, or `resolve_api`, rather than by a public `resolve`. The
  first two record a diagnosed boundary; `resolve_api` exists only for direct
  code generation and carries no findings, so a value that never diagnoses
  cannot pretend it did. Field and function names otherwise follow the plan's
  `Interfaces` section. Date/Author: 2026-11-27 / Claude.

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
  `RstestOnly`, `RstestWithTokioCurrentThread`, `RstestWithGpuiTest`. ADR-0004
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
  (ADR-0017). Today's rule is the special case in which the crate identifier is
  the penultimate segment.
- *Expansion boundary*: the single place in each macro where supplied paths
  are resolved and diagnostics are emitted. For `#[scenario]` it is
  `try_scenario` in `crates/rstest-bdd-macros/src/macros/scenario/mod.rs`. For
  `scenarios!` it is `expand_scenarios_tokens` in
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
base harness API types, which is the consequence ADR-0017 introduced. An
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

- `crates/rstest-bdd-macros/src/codegen/adapter_fallback.rs` (308 lines):
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
- `crates/rstest-bdd-macros/src/codegen/scenario/mod.rs` (338 lines):
  `ScenarioConfig` (which now also carries the step-library `scope`) and
  `reject_async_harness`. Test emission goes through
  `codegen/scenario/emit.rs::emit_scenario_test` (88 lines), which calls
  `boundary.rs::finalize_scenario_signature` (87 lines).
- `crates/rstest-bdd-macros/src/codegen/scenario/adapters.rs` (79 lines): a
  third, non-boundary `SharedAdapterResolutions::resolve` call for callers that
  pass no shared value.
- `crates/rstest-bdd-macros/src/macros/scenario/mod.rs` (356 lines): the
  `#[scenario]` boundary (`try_scenario`). It already knows `sig.asyncness` and
  `item_fn.attrs`, so it builds the full diagnostic inputs.
- `crates/rstest-bdd-macros/src/macros/scenarios/mod.rs` (367 lines): the
  `scenarios!` boundary and `emit_runtime_deprecation_warning`, whose
  alias-plus-harness branch E002 replaces.
- `crates/rstest-bdd-macros/src/macros/scenarios/test_generation/mod.rs`
  (374 lines): `ScenarioTestContext.resolutions`.
- `SharedAdapterResolutions` sites at `0df7fbec`, all updated in EP-M1:
  - production: `codegen/mod.rs:20`, `codegen/scenario/adapters.rs:14,19,33`,
    `codegen/scenario/mod.rs:115`, `macros/scenario/mod.rs:231`,
    `macros/scenarios/mod.rs:89,328`, `macros/scenarios/test_generation/mod.rs:66`;
  - tests: `codegen/scenario/tests/adapter_sharing.rs`,
    `codegen/scenario/tests/runtime_split.rs`, `macros/scenarios/tests.rs`
    (new on `main`; lines 36, 70, 216, 245, 281), and the struct literal in
    `macros/scenarios/test_generation/tests/mod.rs:41`, which is no longer
    Windows-only.
- `crates/rstest-bdd-harness-tokio/tests/macro_compile.rs` (152 lines) and its
  `fixtures_macros/`: the trybuild suite that already hosts the stable and
  nightly fallback fixtures. Its `macro_compile/preparation.rs` child owns the
  nested-fixture preparation stamp that every timed test checks first.
- `crates/rstest-bdd-harness-gpui/tests/macro_compile.rs` (127 lines) and its
  `fixtures_macros/`: GPUI fixtures, with the same preparation child. They run
  under `--all-features` (`native-gpui-tests`), which `make test` passes through
  `CARGO_FLAGS`.

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
- `arch-decision-records`, for ADR-0029;
- `en-gb-oxendict-style`, for message wording and documentation.

Use `codegraph-mcp` (`codegraph_get_callers`, `codegraph_analyze_impact`)
before moving `adapter_fallback` items, and `nextest` for focused runs.

Read:

- `docs/rstest-bdd-design.md` §§2.7.3-2.7.6.4: harness integration, ADR-0008
  codegen, first-party targets, and the v0.6.1 helpers;
- `docs/v0-7-0-migration-guide.md`: the unreleased line's guidance (D26);
- `docs/adrs/adr-0008-harness-led-attribute-policy-defaults.md`: precedence, and
  the validity of harness-only and attributes-only usage;
- `docs/adrs/adr-0017-toolchain-aware-adapter-fallback-diagnostics.md`:
  channels, cardinality, and ownership;
- `docs/adrs/adr-0004-policy-crate.md`: `TestAttributeHint` as the single
  source;
- `docs/developers-guide.md`: "Adapter fallback diagnostics" and "Staging
  fixtures for trybuild tests";
- `docs/users-guide.md`: "Harness adapter and attribute policy" and
  "First-party adapter fallback diagnostics";
- `docs/rust-testing-with-rstest-fixtures.md`: fixture and table idioms;
- `docs/rust-doctest-dry-guide.md`: doctest style for new rustdoc examples;
- `docs/complexity-antipatterns-and-refactoring-strategies.md`: keep
  `try_scenario` and `scenarios` from growing, and extract helpers instead,
  within the cognitive complexity limit of 9 (C13);
- `docs/gherkin-syntax.md`: feature-file syntax for EP-M5;
- `docs/documentation-style-guide.md`: the ADR template and
  en-GB-oxendict spelling.

`docs/rstest-bdd-language-server-design.md` was checked: the language server
does not read `harness` or `attributes` paths, so it is unaffected.

## Conformance basis

Upstream artefacts, read at repository revision `8737d88f` and checked again at
`0df7fbec` (2026-11-24):

- Roadmap item 11.2.3 in `docs/roadmap.md` §11.2, including its finish line,
  and the Phase 11 preamble on semver-compatible additive hardening.
- Design document `docs/rstest-bdd-design.md`:
  - §2.7.3: macro integration, the attribute-policy trust model, and the
    fallback diagnostic;
  - §2.7.4: ADR-0008 codegen;
  - §2.7.5: first-party targets;
  - §2.7.6.3 and §2.7.6.4: the beta quick wins and the v0.6.1 helpers.
- ADR-0004 (policy crate), ADR-0027 (adapter crates; formerly ADR-005a),
  ADR-0007 (context injection), ADR-0008 (harness-led defaults), and ADR-0017
  (fallback diagnostics).
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
11.2.3 -> REQ-NC     -> §2.7.3, ADR-0017 -> EP-M1, EP-M2 -> W001 fixtures, LEM-EVIDENCE
11.2.3 -> REQ-MISS   -> §2.7.3, ADR-0008 -> EP-M2, EP-M4 -> W002 + E001 fixtures, LEM-ERR-REPLACES
11.2.3 -> REQ-AMB    -> ADR-0008         -> EP-M3        -> W003 + E002 fixtures, LEM-SILENCE
11.2.3 -> REQ-FIX    -> ADR-0029 (new)   -> EP-M2..EP-M4 -> LEM-FIX-CONVERGES, LEM-FIX-CANONICAL, insta
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
  `policy_diagnostics/tests/exhaustive.rs`. Evidence: the `policy_diagnostics`
  focused loop in `Concrete steps`.

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
  structs, using a two-scenario feature. Non-vacuity: three markers come from
  the witness that carries a non-canonical, conflicting policy *and* a
  non-canonical harness — `W001(harness) + W001(attributes) + W003` — because
  W002 requires `attributes = Absent` and so can never co-occur with W003
  (decision D32).
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
  fault: matching only the penultimate segment (today's ADR-0017 rule) must
  fail the deep-nesting case.
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
   including the struct literal in `test_generation/tests/mod.rs`. Re-run
   `git grep -n SharedAdapterResolutions -- crates` first, because `main` keeps
   adding call sites.
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
  - Windows CI passes on the pushed branch.
- Conformance check:
  - no requirement is newly claimed;
  - the ADR-0017 structure is preserved (pure resolution, boundary emission);
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
- Register every fixture in the existing timed tests (D27). Those functions
  already call `preparation::tree().ensure_prepared(preparation::prepare)?`,
  which any new timed test would also have to do.
- Add trybuild fixtures, all stable fixtures under `#![deny(deprecated)]`:
  - `crates/rstest-bdd-harness-tokio/tests/fixtures_macros/policy_w001_noncanonical.rs`:
    a nested harness and a nested policy in synchronous code, giving two W001
    blocks with no attribute clause;
  - `crates/rstest-bdd-harness-gpui/tests/fixtures_macros/policy_w002_missing_policy.rs`:
    a nested GPUI harness without attributes (W002), plus a nested GPUI
    policy (W001 with the `#[gpui::test]` clause);
  - nightly twin
    `crates/rstest-bdd-harness-gpui/tests/fixtures_macros/nightly_policy_w002_warning.rs`,
    only if the existing nightly GPUI fixture does not already pin W002.
- Add a trybuild `pass` fixture,
  `crates/rstest-bdd-harness-tokio/tests/fixtures_macros/policy_opt_out.rs`: a
  module-level `#[allow(deprecated)]` around a W001 configuration, under a
  crate-level `#![deny(deprecated)]`. Removing the opt-out must make it fail.
  This pins the documented opt-out (D10, D25) and is the C8 exception.

Green:

- Widen `classify`, add W002 to the model, and extend `message.rs`.
- Regenerate the nine existing `.stderr` files on stable and on nightly for
  both suites, and review every diff line by line.

- Requirements: `REQ-NC`, `REQ-MISS` (W002), `REQ-FIX`, and `REQ-TRY` for
  these codes.
- Acceptance:
  - the new fixtures, including the opt-out, behave as expected;
  - `scenario_custom_tokio_harness_name.rs` (a custom type under
    `deny(deprecated)`, `pass`) still passes;
  - the nightly regeneration transcript is recorded.
- Conformance check:
  - REQ-NC and the W002 part of REQ-MISS are satisfied;
  - ADR-0017 cardinality and custom-type silence hold;
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
  - the E002 fixture shows exactly one `rstest-bdd[E002]` error spanned at the
    harness path, and no "combining `harness` with `async fn`" text (D33).
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
  `make nextest-run EXPR='binary_id(rstest-bdd-harness-tokio::harness_led_defaults)'`
  passes all its scenarios, including the three new ones.
- Conformance check: one line per item, as for EP-M2.
- Recovery: revert. EP-M4 remains coherent.
- Remaining gaps: documentation.
- Compatibility decision: none.

### EP-M6: documentation

Outcome: documentation matches the implementation.

- Add `docs/adrs/adr-0029-evidence-based-harness-and-policy-diagnostics.md`
  following the documentation style guide's ADR template. Its status becomes
  `Accepted` when the implementation lands. It records:
  - the taxonomy and codes;
  - the evidence principle, including the resolvability conjunct;
  - the error-replaces-error rule;
  - the decision not to warn on harness-only third-party usage, scoped to
    the additive Phase 11 line and supersedable by roadmap 12.2.3's ADR;
  - the fix-versus-workaround rule;
  - the rule that errors suppress warnings;
  - channel reuse and the opt-out;
  - the ADR-0008 prose departure (D14).

  Confirm the next free ADR number first (D24). Add an
  `Amended by ADR-0029 (date)` banner to the Status section, and an
  `## Amendments` section, in ADR-0017. Index ADR-0029 in `docs/contents.md`.
- `docs/rstest-bdd-design.md`:
  - §2.7.3: replace the fallback paragraphs with a "Policy diagnostics"
    subsection that summarizes the five diagnostics and references ADR-0029;
  - correct the false remedy at lines 1794-1795 (at `0df7fbec`), and the
    description of the warning text in the paragraph that follows;
  - §2.7.6.4: list 11.2.3 as delivered.
- `docs/users-guide.md`: rename "First-party adapter fallback diagnostics" to
  "Harness and attribute-policy diagnostics", and give each code an anchored
  entry with a triggering example, the message, and the fix. Also:
  - correct lines 1172-1173 (at `0df7fbec`);
  - document the dev-dependency workaround as a workaround;
  - document the module-level opt-out and the nightly limitation;
  - state the renamed-import and `macro_rules!` limitations;
  - state the `-D warnings` consequence.
- `docs/v0-6-0-migration-guide.md`: correct lines 339-341 (at `0df7fbec`) in
  the fallback-warning blockquote.
- `docs/v0-7-0-migration-guide.md`: add guidance for the new warnings, their
  `-D warnings` consequence, and the opt-out (D26).
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
ADR-0029. Run all gates. Complete `Outcomes & retrospective`, reconcile
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

`make test` prepares its own fixtures. Focused loops use the prepared-fixture
targets directly (D23). Run `prepare-nested-fixtures` after any `RUSTFLAGS`,
toolchain, or dependency change, and after a plain or nightly `cargo test`,
then filter. It is idempotent on a warm tree:

```bash
make prepare-nested-fixtures 2>&1 | tee "/tmp/prepare-rstest-bdd-$BR.out"
make nextest-run EXPR='package(rstest-bdd-macros) & test(/policy_diagnostics/)' 2>&1 \
  | tee "/tmp/unit-rstest-bdd-$BR.out"
make nextest-run EXPR='binary_id(rstest-bdd-harness-tokio::macro_compile)' 2>&1 \
  | tee "/tmp/trybuild-tokio-rstest-bdd-$BR.out"
make nextest-run EXPR='binary_id(rstest-bdd-harness-gpui::macro_compile)' 2>&1 \
  | tee "/tmp/trybuild-gpui-rstest-bdd-$BR.out"
make nextest-run EXPR='binary_id(rstest-bdd-harness-tokio::harness_led_defaults)' 2>&1 \
  | tee "/tmp/bdd-tokio-rstest-bdd-$BR.out"
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
make typecheck 2>&1 | tee "/tmp/typecheck-rstest-bdd-$BR.out"
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
6. `policy_user_executor.rs` compiles and passes, and the `policy_opt_out.rs`
   fixture passes, under `make test`.
7. Every pre-existing `pass` fixture, macrotest snapshot, and
   `harness_defaults` case is unchanged.
8. The three new behavioural scenarios pass.
9. `make check-fmt`, `make lint`, `make typecheck`, `make test`,
   `make markdownlint`, and `make nixie` pass.

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

The in-repo scan behind the "17 of 20" figure (read-only, run at `8737d88f` and
re-run with the same result at `0df7fbec`). It treats a `harness =` path whose
last segment is `TokioHarness`, `GpuiHarness`, or `StdHarness` as first-party:

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

### EP-M0 baselines (recorded 2026-11-27 at `b55bd2c1`)

All figures below were measured on this development host with the shared
compile-admission pool roughly 120 to 200 requests deep, so they are
*contended-host* figures. Time spent waiting for a compile slot counts against
none of the plan's effort tolerances.

- `make test` (three processes: attempt 2's first pass and doctests, then a
  resumed second pass and trailing steps; every step exited 0, and the working
  tree was clean at every checkpoint with `HEAD` unmoved):
  - preparation lane: 7 tests run, 7 passed, 2592 skipped;
  - first nextest pass (`--workspace --all-targets --all-features`): 2592
    tests run, 2592 passed, 7 skipped, 3782.750 s;
  - doctests: 180 passed, 0 failed;
  - second-leg preparation: 3 run, 3 passed, 842 skipped;
  - second nextest pass (`-p rstest-bdd --all-targets
    --no-default-features`): 838 run, 838 passed, 7 skipped, 3487.356 s;
  - `make check-fixture-lockfiles`: 7 fixture lockfiles up to date;
  - `pytest scripts/tests`: 398 passed.
- warm (contended) `macro_compile` wall-clock, which is the test-cost
  tolerance's baseline:
  - Tokio: 408.080 s total; fixture compilation 400.384 s, snapshot assertions
    0.017 s;
  - GPUI: 610.290 s total; fixture compilation 607.355 s, snapshot assertions
    0.017 s.
- current stderr of `scenario_default_policy_async_no_harness_rejected`: one
  `rstest` error whose text is "async test requires either explicit `test_attr`
  or implicit (attribute path ends with `test`)", spanned at the generated
  function name.
- CI lane list (`.github/workflows/ci.yml`): one stable Linux lane and two
  stable Windows lanes. A nightly toolchain is installed only for `rustfmt` and
  the published-GPUI end-to-end fixture, so Risk R4 stands: the native warning
  channel is verified locally only.

### EP-M0 experiment transcripts (2026-11-27)

Each experiment was a throwaway fixture registered in the existing timed test,
run with `TRYBUILD=overwrite` under `make nextest-run`, then deleted together
with its `.stderr`. The logs are `/tmp/epm0-exp-tokio6.out`,
`/tmp/epm0-exp-gpui2.out`, and `/tmp/epm0-exp-prepare*.out`.

1. Fixture host: `zz_epm0_w003_host.rs` in the GPUI suite, with
   `rstest-bdd-harness-tokio` added to that crate's dev-dependencies. Its
   stderr was the sentinel alone:

   ```plaintext
   error: epm0 w003 sentinel
     --> tests/fixtures_macros/zz_epm0_w003_host.rs:27:1
   ```

   The suite reported `Summary [...] 3 tests run: 3 passed, 1 skipped`.

2. `AX-EXISTING-REJECT`: `scenarios!` with
   `runtime = "tokio-current-thread"` plus
   `harness = rstest_bdd_harness_tokio::TokioHarness` over the two-scenario
   `scenarios_multi_fallback.feature` produced exactly one error:

   ```plaintext
   error: combining `harness` with `async fn` scenarios is not supported; use a
   synchronous scenario function with `TokioHarness` instead (the harness
   provides the Tokio runtime for step functions)
     --> tests/fixtures_macros/zz_epm0_alias_with_harness.rs:18:1
   ```

   (The compiler prints that message on one line; it is wrapped here only to
   satisfy the documentation line limit.)

   The token stream holds one such error per generated scenario; all share the
   call-site span and text, so rustc collapses them (D33).

3. Channel behaviour: the expansion carrying both
   `compile_error!("epm0 sentinel")` and the generated marker, under
   `#![deny(deprecated)]`, reported both. The module-level opt-out fixture
   compiled and passed under the same crate-level deny.

To be filled during implementation:

- red and green transcripts per milestone;
- nightly transcripts with the toolchain version;
- EP-M2 snapshot regeneration: stable runs repaired the six stable markers and
  created `policy_w001_noncanonical.stderr` and
  `policy_w002_missing_policy.stderr`; `nightly-2026-08-16` runs repaired the
  three nightly `.stderr` files, and the native warnings carry the same codes
  and text as the stable markers;
- seeded-fault diffs and the tests that reject them;
- generator seeds.

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
- 2026-11-23: rebased onto `origin/main` at `4c21753d`. The ADR number, focused
  test commands, line references, and open pull requests were revised (D22,
  D23). The revision's claim to have re-verified everything was wrong.

  Effect on remaining work: none on the design. EP-M1 step 2, EP-M2's
  registration, and EP-M6's ADR step carry the new instructions.
- 2026-11-24: rebased onto `origin/main` at `0df7fbec`. Changes:
  - the ADR becomes ADR-0029 under `docs/adrs/`, and all ADR references use
    four digits (D24);
  - the opt-out is pinned by a trybuild fixture, because the integration test
    would have failed `make lint` (D25);
  - the work targets the unreleased 0.7.0 line (D26);
  - C12 and D27 record the current AGENTS.md test conventions;
  - Risk R1's in-repo claim, Risk R2's pull requests, and all anchors are
    corrected;
  - `make test` prepares its own nested fixtures (#979).

  Effect on remaining work: EP-M2 changes where the opt-out is tested and may
  drop one nightly fixture; EP-M6 gains the v0.7.0 migration guidance. The
  taxonomy, model, and obligations are unchanged.
- 2026-11-26: restacked onto #986 at `5dbc2fb5`, over `origin/main` at
  `6308b9af`. Changes:
  - C13 and D28 record #980's macro-aware cognitive complexity gate at 9;
  - D28 records that local test commands run under #986's `local` profile;
  - D26 is confirmed by the user: target v0.7.0, with no v0.6.1 backport.

  Effect on remaining work: helpers extracted in EP-M1 to EP-M4 must score 9 or
  less. The design, taxonomy, and obligations are unchanged.
