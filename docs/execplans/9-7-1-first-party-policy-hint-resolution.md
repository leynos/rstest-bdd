# Extend first-party policy hint resolution

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This plan covers roadmap item 9.7.1 only. It must be approved before
implementation begins, and implementation must not begin while
`docs/adr-008-harness-led-attribute-policy-defaults.md` remains in `Proposed`
status unless a maintainer explicitly authorizes work against the proposed
decision.

## Purpose / big picture

Roadmap item 9.7.1 extends the shared test-attribute hint resolver so known
first-party harness paths imply the same default test-attribute hints as their
matching first-party attribute-policy paths. After the change, shared helper
code can resolve these two equivalent user choices to the same
`TestAttributeHint`:

```rust,no_run
// Attribute-policy path.
rstest_bdd_harness_tokio::TokioAttributePolicy

// Harness path.
rstest_bdd_harness_tokio::TokioHarness
```

This matters because ADR-008 makes `harness = ...` the lead configuration for
first-party integrations. Users should be able to select `StdHarness`,
`TokioHarness`, or `GpuiHarness` without also repeating the matching
`attributes = ...` value, while still preserving explicit attribute-policy
overrides and third-party escape hatches.

Success is observable when unit tests prove that the shared resolver returns
the same hint for each first-party harness path and its matching
attribute-policy path, unknown third-party harnesses do not infer a policy, and
the ADR-008 precedence edge cases are covered. The roadmap item must not be
marked done until the accepted ADR, code, tests, documentation, and repository
gates all agree.

## Constraints

- Do not implement this plan before explicit approval.
- Do not implement this plan while ADR-008 is still `Proposed` unless a
  maintainer explicitly authorizes contingent implementation.
- Keep the scope to roadmap item 9.7.1. Do not fold in roadmap items 9.7.2,
  9.7.3, or 9.7.4 except where a small test is necessary to prove the shared
  resolver contract.
- Preserve the ADR-005 boundary: `HarnessAdapter` remains the runtime
  delegation boundary and `AttributePolicy` remains the emitted test-attribute
  boundary.
- Preserve the ADR-008 precedence order:
  1. explicit `attributes = ...`
  2. known first-party `harness = ...` mapping
  3. deprecated `runtime = "tokio-current-thread"` compatibility alias
  4. existing runtime-mode or synchronous fallback
- Keep `attributes`-only configuration valid.
- Unknown third-party harness paths must not infer attribute-policy hints.
- Do not add external dependencies unless the maintainer approves a separate
  dependency decision.
- Use `rstest` for parameterized unit tests. `rust-rspec` is not currently
  present in this workspace; do not introduce it just for this finite lookup
  task unless a maintainer explicitly asks for that dependency.
- Property tests, Kani, or Verus are not required for a fixed canonical lookup
  table. If implementation changes into a generalized parser or stateful
  resolver with invariants over arbitrary inputs, stop and add the appropriate
  proof or property-test strategy before continuing.
- Keep code files under the repository's 400-line limit and keep new helpers
  documented with examples where public.
- Run gates sequentially. Do not run format, lint, or test commands in
  parallel.

## Tolerances

- Scope: if the implementation requires more than 8 files changed or more than
  350 net lines, stop and re-check whether the work has drifted into 9.7.2 or a
  broader macro-codegen refactor.
- Interface: if any public trait, macro argument, or existing public function
  signature must change, stop and escalate before continuing.
- Dependencies: if a new external crate is required, stop and escalate.
- Governance: if ADR-008 remains `Proposed`, stop before code changes unless
  explicit maintainer approval is recorded in this plan.
- Existing-work reconciliation: if the current code already satisfies 9.7.1,
  do not rewrite it. Verify the behaviour, update missing documentation or
  roadmap state only after gates pass, and record the finding in the Decision
  Log.
- Validation: if `make check-fmt`, `make lint`, or `make test` fails for an
  unrelated reason, capture the log path and stop before marking the roadmap
  item done.
- Iterations: if the same gate fails three consecutive fix attempts, stop and
  escalate with the log path and current hypothesis.
- Ambiguity: if docs and code disagree on canonical paths, precedence, or
  whether 9.7.1 is already delivered, stop and present the interpretations.

## Risks

- Risk: the working tree already contains ADR-008-shaped helpers while
  `docs/roadmap.md` still leaves 9.7.1 unchecked and ADR-008 remains
  `Proposed`. Severity: high. Likelihood: high. Mitigation: start with a
  reconciliation milestone that inspects code, tests, ADR status, and roadmap
  state before editing implementation code.
- Risk: implementing harness-led defaults in macro codegen would accidentally
  consume scope from 9.7.2. Severity: medium. Likelihood: medium. Mitigation:
  keep 9.7.1 centred on shared hint helpers and unit-level precedence proof;
  defer broad public macro behaviour to the 9.7.2 plan unless a narrow helper
  test is needed.
- Risk: a single-segment path such as `TokioHarness` could be mistaken for the
  canonical first-party path. Severity: medium. Likelihood: medium. Mitigation:
  require exact segment matching against canonical paths.
- Risk: unknown third-party harnesses could silently receive first-party
  attributes, making extension behaviour surprising. Severity: high.
  Likelihood: low. Mitigation: add explicit `rstest` cases for unknown
  harnesses, similarly named harnesses, and wrong crate prefixes.
- Risk: `make test` may not exercise every compile-time fixture when nextest is
  active. Severity: medium. Likelihood: medium. Mitigation: include focused
  crate tests for `rstest-bdd-policy` and `rstest-bdd-macros`, then run the
  repository-wide gates required by this task.
- Risk: documentation could overstate third-party inference. Severity: medium.
  Likelihood: medium. Mitigation: update design and user-facing docs to state
  that only known first-party paths infer defaults and third-party policies
  remain explicit.

## Progress

- [x] (2026-05-08T11:22:32Z) Loaded and applied the `execplans`, `leta`,
      `rust-router`, and `arch-crate-design` skills for this planning task.
- [x] (2026-05-08T11:22:32Z) Created context-pack artefact
      `pk_4pujtp56` for the Wyvern planning team.
- [x] (2026-05-08T11:22:32Z) Asked Wyvern agents to inspect ADR/design
      requirements, implementation/test touchpoints, and PR workflow
      constraints.
- [x] (2026-05-08T11:22:32Z) Confirmed the branch was not `main` and renamed
      it to `9-7-1-first-party-policy-hint-resolution`.
- [x] (2026-05-08T11:22:32Z) Reviewed `docs/roadmap.md`,
      `docs/adr-008-harness-led-attribute-policy-defaults.md`, and
      `docs/rstest-bdd-design.md` for the requested scope.
- [x] (2026-05-08T11:22:32Z) Inspected current resolver and macro-codegen
      touchpoints enough to identify existing ADR-008-shaped helpers.
- [x] (2026-05-08T11:22:32Z) Drafted this pre-implementation ExecPlan.
- [ ] Await explicit approval before implementation.
- [ ] After approval and ADR acceptance, reconcile existing implementation
      against 9.7.1 before making code edits.
- [ ] Implement or complete missing shared resolver behaviour.
- [ ] Add or complete unit tests for canonical mappings, unknown paths, and
      precedence edge cases.
- [ ] Add behavioural or compile-time coverage only where applicable to prove
      externally observable resolver use without absorbing 9.7.2.
- [ ] Update design, user, and component documentation where behaviour or
      internal contracts change.
- [ ] Run all required gates.
- [ ] Mark roadmap item 9.7.1 done after successful validation.
- [ ] Commit the completed implementation as a focused change.

## Surprises & Discoveries

- Observation: `docs/adr-008-harness-led-attribute-policy-defaults.md` is still
  in `Proposed` status. Evidence: its `Status` section says `Proposed`. Impact:
  this plan must remain contingent until approval and ADR acceptance.
- Observation: roadmap prerequisites 9.3.4 and 9.4.4 are already marked done.
  Evidence: `docs/roadmap.md` marks both items with `[x]`. Impact: the
  remaining governance gate is ADR-008 acceptance.
- Observation: the current worktree already contains
  `resolve_test_attribute_hint_for_harness_path` and a `KNOWN_HARNESS_HINTS`
  table in `crates/rstest-bdd-policy/src/lib.rs`. Evidence: targeted source
  inspection found canonical entries for `StdHarness`, `TokioHarness`, and
  `GpuiHarness`. Impact: implementation must begin by verifying whether 9.7.1
  is already functionally delivered and only then decide whether code changes
  are needed.
- Observation: `docs/rstest-bdd-design.md` and `docs/users-guide.md` already
  describe harness-led defaults in several places. Evidence: both documents
  contain sections describing known first-party harness inference. Impact:
  documentation work may be reconciliation and correction rather than
  first-time drafting.
- Observation: `leta workspace add` succeeded, but rust-analyzer failed to
  start for semantic queries in this worktree. Evidence: `leta grep` returned a
  rust-analyzer connection-closed error. Impact: this planning pass used
  targeted `rg` and file reads after recording the tool limitation.

## Decision Log

- Decision: keep this ExecPlan in `DRAFT` status and explicitly gate
  implementation on both plan approval and ADR-008 acceptance. Rationale: the
  roadmap says 9.7 items are contingent while ADR-008 remains `Proposed`, and
  the user reminded that the plan must be approved before implementation.
  Date/Author: 2026-05-08 / Codex.
- Decision: make Stage A a reconciliation inventory rather than assuming no
  prior implementation exists. Rationale: current code and docs already contain
  several of the names and behaviours requested by 9.7.1, while the roadmap
  remains unchecked. Date/Author: 2026-05-08 / Codex.
- Decision: keep canonical mappings in `rstest-bdd-policy` unless the
  reconciliation step finds a dependency or API problem. Rationale: policy
  hints are shared execution-policy data, and keeping the lookup outside
  `rstest-bdd-macros` avoids reintroducing macro-local mapping tables.
  Date/Author: 2026-05-08 / Codex.
- Decision: do not require property testing, Kani, or Verus for the fixed
  first-party lookup table. Rationale: the invariant is finite and better
  covered by exhaustive parameterized `rstest` cases over the canonical and
  negative path matrix. Date/Author: 2026-05-08 / Codex.

## Outcomes & Retrospective

This section is intentionally empty while the plan is in `DRAFT`. Fill it in
after implementation or after a reconciliation-only delivery proves that the
feature was already present and only roadmap or documentation state needed to
be corrected.

## Context and orientation

The relevant workspace crates are:

- `crates/rstest-bdd-policy`: shared runtime and test-attribute policy enums.
  This is the preferred home for canonical path-to-`TestAttributeHint`
  resolution because both runtime and macro crates can depend on it without a
  procedural macro dependency cycle.
- `crates/rstest-bdd-macros`: procedural macro code generation for
  `#[scenario]` and `scenarios!`. It consumes `TestAttributeHint` to decide
  whether to emit `#[rstest::rstest]`,
  `#[tokio::test(flavor = "current_thread")]`, or `#[gpui::test]`.
- `crates/rstest-bdd-harness`: first-party shared harness traits and
  `StdHarness`.
- `crates/rstest-bdd-harness-tokio`: first-party Tokio harness and
  `TokioAttributePolicy`.
- `crates/rstest-bdd-harness-gpui`: first-party GPUI harness and
  `GpuiAttributePolicy`.

The important terms are:

- A harness path is the Rust type path supplied as `harness = ...`, such as
  `rstest_bdd_harness_tokio::TokioHarness`.
- An attribute-policy path is the Rust type path supplied as
  `attributes = ...`, such as `rstest_bdd_harness_tokio::TokioAttributePolicy`.
- A `TestAttributeHint` is the shared enum that describes which framework test
  attributes the macro layer should generate.
- A first-party path is an exact canonical path owned by this workspace. A
  third-party path is any user or external crate path.

The canonical mappings required by 9.7.1 are:

- `rstest_bdd_harness::StdHarness` maps to the same hint as
  `rstest_bdd_harness::DefaultAttributePolicy`.
- `rstest_bdd_harness_tokio::TokioHarness` maps to the same hint as
  `rstest_bdd_harness_tokio::TokioAttributePolicy`.
- `rstest_bdd_harness_gpui::GpuiHarness` maps to the same hint as
  `rstest_bdd_harness_gpui::GpuiAttributePolicy`.

The relevant source and documentation entrypoints are:

- `docs/roadmap.md`, section 9.7.1, for acceptance criteria and prerequisite
  status.
- `docs/adr-008-harness-led-attribute-policy-defaults.md`, especially the
  decision outcome, precedence rules, migration plan, and known risks.
- `docs/rstest-bdd-design.md`, section 2.7.3 and adjacent ADR-008 notes, for
  component architecture and macro integration.
- `docs/users-guide.md`, harness adapter and attribute policy sections, for
  user-visible behaviour and third-party caveats.
- `docs/rstest-bdd-language-server-design.md`, to confirm no language-server
  contract changes are needed.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md`,
  `docs/complexity-antipatterns-and-refactoring-strategies.md`, and
  `docs/gherkin-syntax.md`, for testing and documentation style constraints.
- `crates/rstest-bdd-policy/src/lib.rs`, for `RuntimeMode`,
  `TestAttributeHint`, existing policy-path resolution, and any harness-path
  resolution.
- `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs`, for
  precedence between explicit attribute paths, harness paths, runtime aliases,
  and fallback hints.
- `crates/rstest-bdd-macros/src/codegen/scenario/tests.rs` and sibling test
  modules, for unit tests around generated attributes.

Relevant skills to load when implementing this plan are `leta` for code
navigation, `rust-router` followed by `arch-crate-design` for crate-boundary
decisions, and `rust-types-and-apis` only if a public resolver signature must
change.

## Plan of work

Stage A is reconciliation and must make no functional edits. Confirm ADR-008
status first. If the ADR is still `Proposed`, record the blocker in this plan
and stop unless maintainer approval explicitly authorizes implementation. After
that, compare roadmap 9.7.1 against the current implementation in
`crates/rstest-bdd-policy/src/lib.rs` and
`crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs`. Confirm whether
the shared resolver already exposes policy-path and harness-path lookup
helpers, whether the helper tests already cover unknown third-party paths, and
whether macro-level precedence tests already exist. Record findings in
`Surprises & Discoveries` and decide whether code changes are needed.

Stage B adds or completes the shared resolver. In
`crates/rstest-bdd-policy/src/lib.rs`, keep or add canonical path constants for
`DEFAULT_ATTRIBUTE_POLICY_PATH`, `STD_HARNESS_PATH`,
`TOKIO_ATTRIBUTE_POLICY_PATH`, `TOKIO_HARNESS_PATH`,
`GPUI_ATTRIBUTE_POLICY_PATH`, and `GPUI_HARNESS_PATH`. Keep or add a
`resolve_test_attribute_hint_for_harness_path(path_segments: &[&str]) -> Option<TestAttributeHint>`
 helper beside `resolve_test_attribute_hint_for_policy_path`. The harness
resolver must match only exact canonical path segments and return `None` for
single-segment names, wrong crate prefixes, similarly named third-party types,
and arbitrary unknown paths.

Stage C adds or completes unit tests. Use `rstest` parameterized cases in
`crates/rstest-bdd-policy/src/lib.rs` or a focused sibling test module to prove
each first-party harness path returns the same `TestAttributeHint` as the
matching first-party attribute-policy path. Add negative cases for
`["TokioHarness"]`, `["my", "TokioHarness"]`, `["my", "Harness"]`, and any
other similarly named path needed to prove exact matching. If macro-level
precedence is not already covered, add focused tests in
`crates/rstest-bdd-macros/src/codegen/scenario/tests/harness_defaults.rs` or
the local equivalent to prove explicit `attributes = ...` beats known
`harness = ...`, known harness beats runtime fallback, unknown harness falls
back to runtime, and explicit unknown attributes intentionally resolve to the
rstest-only fallback.

Stage D adds only applicable behavioural or compile-time coverage. Because
9.7.1 is a shared helper change, public-macro end-to-end expansion belongs
primarily to 9.7.2 and 9.7.3. Add a behavioural, trybuild, or integration test
in this stage only if the reconciliation step shows there is no existing test
that exercises the helper through macro codegen. If such a test is needed,
prefer existing suites such as `crates/rstest-bdd/tests/trybuild_macros.rs`,
`crates/rstest-bdd-harness-tokio/tests/macro_compile.rs`, or
`crates/rstest-bdd-harness-gpui/tests/macro_compile.rs`. Keep the test narrow
and record why it belongs in 9.7.1 rather than 9.7.2.

Stage E updates documentation. If behaviour or implementation details change,
update `docs/rstest-bdd-design.md` section 2.7.3 or the adjacent ADR-008
codegen notes to describe the shared helper, exact canonical mappings, and
precedence order. Update `docs/users-guide.md` only if user-visible behaviour
or caveats differ from the current guide. If no docs change is needed because
the current docs already describe the final behaviour, record that in the
Decision Log. Do not mark `docs/roadmap.md` item 9.7.1 done until all
validation has passed.

Stage F validates and closes. Run focused tests first, then repository gates.
If all gates pass, update `docs/roadmap.md` to mark 9.7.1 done and add a short
delivery note with the date. Review the diff for accidental 9.7.2 scope. Make
one focused commit for the approved implementation and roadmap update. If the
only required change is roadmap reconciliation after validation, the commit
message must say that explicitly.

## Concrete steps

Start in the repository root:

```bash
cd /home/leynos/.lody/repos/github---leynos---rstest-bdd/worktrees/740747bd-ef6a-438d-a91e-d81f8120dbe2
git branch --show-current
```

Expect:

```plaintext
9-7-1-first-party-policy-hint-resolution
```

Confirm the governance gate:

```bash
rg -n \
  "^## Status|^Proposed$|^Accepted$" \
  docs/adr-008-harness-led-attribute-policy-defaults.md
```

If the ADR is still `Proposed`, stop before implementation unless approval is
explicitly recorded.

Inspect the current implementation state:

```bash
rg -n \
  "resolve_test_attribute_hint_for_harness_path|KNOWN_HARNESS_HINTS" \
  crates/rstest-bdd-policy/src/lib.rs
rg -n \
  "STD_HARNESS_PATH|TOKIO_HARNESS_PATH|GPUI_HARNESS_PATH" \
  crates/rstest-bdd-policy/src/lib.rs
rg -n \
  "resolve_attribute_policy|TestAttrPolicy" \
  crates/rstest-bdd-macros/src/codegen/scenario
rg -n \
  "resolve_test_attribute_hint_for_harness_path" \
  crates/rstest-bdd-macros/src/codegen/scenario
```

Run focused tests before changing code so red/green evidence is meaningful:

```bash
set -o pipefail && \
  cargo test -p rstest-bdd-policy 2>&1 | \
  tee /tmp/test-rstest-bdd-policy-9-7-1.out
set -o pipefail && \
  cargo test -p rstest-bdd-macros codegen::scenario 2>&1 | \
  tee /tmp/test-rstest-bdd-macros-scenario-9-7-1.out
```

After implementation edits, repeat the focused tests. If macro compile-time
fixtures were touched, also run:

```bash
set -o pipefail && \
  cargo test -p rstest-bdd --test trybuild_macros 2>&1 | \
  tee /tmp/test-trybuild-9-7-1.out
```

Run the required gates sequentially:

```bash
set -o pipefail && \
  make fmt 2>&1 | tee /tmp/fmt-9-7-1.out
set -o pipefail && \
  make check-fmt 2>&1 | tee /tmp/check-fmt-9-7-1.out
set -o pipefail && \
  make lint 2>&1 | tee /tmp/lint-9-7-1.out
set -o pipefail && \
  make test 2>&1 | tee /tmp/test-9-7-1.out
```

If Markdown files changed, run the documentation gates:

```bash
set -o pipefail && \
  make markdownlint 2>&1 | tee /tmp/markdownlint-9-7-1.out
set -o pipefail && \
  make nixie 2>&1 | tee /tmp/nixie-9-7-1.out
```

Before committing, inspect the complete diff:

```bash
git status --short
git diff -- \
  docs/roadmap.md \
  docs/rstest-bdd-design.md \
  docs/users-guide.md \
  crates/rstest-bdd-policy/src/lib.rs \
  crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs
```

Commit with a file-based commit message, not `git commit -m`.

## Validation and acceptance

The implementation is accepted only when all of the following are true:

- ADR-008 is accepted or maintainer approval for contingent implementation is
  recorded in this plan.
- `resolve_test_attribute_hint_for_policy_path` and
  `resolve_test_attribute_hint_for_harness_path` return matching hints for the
  first-party policy and harness pairs.
- Unknown third-party harness paths return `None` from the harness resolver.
- Unit tests use `rstest` to cover happy paths, unknown paths, and precedence
  edge cases.
- Any applicable behavioural or compile-time coverage passes without absorbing
  roadmap item 9.7.2.
- `docs/rstest-bdd-design.md` documents any new internal contract or records
  that no update was required.
- `docs/users-guide.md` documents any user-visible behaviour or records that
  no update was required.
- `docs/roadmap.md` marks 9.7.1 done only after validation succeeds.
- `make check-fmt`, `make lint`, and `make test` pass.
- If Markdown changed, `make markdownlint` and `make nixie` pass.

No end-to-end test is required for the fixed shared lookup table unless the
implementation changes externally observable macro workflows. If macro workflow
changes are needed, add focused trybuild or behavioural coverage in the
existing macro or harness suites.

## Idempotence and recovery

All inspection and test commands are safe to repeat. Formatting commands may
rewrite Markdown or Rust files; review `git diff` after running them. If a
validation command fails, use its `/tmp/...9-7-1...out` log as the evidence
source, fix the smallest relevant issue, and rerun the same command before
moving on.

If reconciliation shows the feature is already implemented, do not delete or
rewrite working code. Run the focused tests and gates, update only the missing
documentation or roadmap state, and explain the reconciliation in the Decision
Log and final commit message.

If ADR-008 remains `Proposed`, stop cleanly with this plan in `DRAFT` and do
not mark the roadmap item done.

## Artifacts and notes

The planning context pack used for Wyvern coordination is `pk_4pujtp56`
(`9-7-1-policy-hint-planning`). The first Wyvern planning brief confirmed:

```plaintext
ADR-008 is still Proposed.
Precedence is explicit attributes, known first-party harness mapping,
deprecated Tokio runtime alias, then fallback.
Canonical mappings are StdHarness -> DefaultAttributePolicy,
TokioHarness -> TokioAttributePolicy, and GpuiHarness -> GpuiAttributePolicy.
Unknown third-party harnesses must not infer defaults.
```

The current planning pass observed these existing identifiers:

```plaintext
crates/rstest-bdd-policy/src/lib.rs:
  resolve_test_attribute_hint_for_policy_path
  resolve_test_attribute_hint_for_harness_path
  KNOWN_HARNESS_HINTS

crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs:
  TestAttrPolicy
  resolve_attribute_policy
```

Treat these as starting points for reconciliation, not proof that the roadmap
item can be closed without validation.

## Interfaces and dependencies

The intended shared helper surface in `crates/rstest-bdd-policy/src/lib.rs` is:

```rust
#[must_use]
pub fn resolve_test_attribute_hint_for_policy_path(
    path_segments: &[&str],
) -> Option<TestAttributeHint>;

#[must_use]
pub fn resolve_test_attribute_hint_for_harness_path(
    path_segments: &[&str],
) -> Option<TestAttributeHint>;
```

Both helpers must remain independent of `syn`, `quote`, `proc_macro2`, Tokio,
and GPUI. Macro code may convert `syn::Path` into segment names locally before
calling these helpers, but the shared policy crate should stay a small,
dependency-light resolver crate.

The canonical mapping table must be exact:

```rust
const KNOWN_HARNESS_HINTS: [(&[&str], TestAttributeHint); 3] = [
    (STD_HARNESS_PATH, TestAttributeHint::RstestOnly),
    (
        TOKIO_HARNESS_PATH,
        TestAttributeHint::RstestWithTokioCurrentThread,
    ),
    (GPUI_HARNESS_PATH, TestAttributeHint::RstestWithGpuiTest),
];
```

Do not add crate-name-only autodiscovery, trait-method evaluation during macro
expansion, registration macros, or third-party inference in 9.7.1.

## Revision note

- 2026-05-08: Initial draft created from roadmap item 9.7.1, ADR-008,
  current design/user documentation, targeted source inspection, and Wyvern
  planning input. The plan is explicitly contingent because ADR-008 remains
  `Proposed` and implementation requires approval.
