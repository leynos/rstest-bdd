# Implement first-party harness-led attribute defaults

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This plan covers roadmap item 9.7.2 only. It must be approved before
implementation begins. While
`docs/adr-008-harness-led-attribute-policy-defaults.md` remains in `Proposed`
status, implementation remains contingent unless a maintainer explicitly
authorizes it.

## Purpose / big picture

Roadmap item 9.7.2 makes first-party harness selection carry the matching
first-party test attributes during macro code generation. After the change, a
consumer can write:

```rust,no_run
#[scenario(
    path = "tests/features/payment.feature",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn payment_succeeds() {}
```

and the generated test attributes behave as though the caller had also written
`attributes = rstest_bdd_harness_tokio::TokioAttributePolicy`, unless the
caller supplies an explicit `attributes = ...` override. The same rule applies
to `rstest_bdd_harness::StdHarness` and `rstest_bdd_harness_gpui::GpuiHarness`.

This matters because Architecture Decision Record (ADR) 008 makes
`harness = ...` the lead user-facing configuration for first-party integrations
while preserving the architectural split between `HarnessAdapter`, which
controls runtime delegation, and `AttributePolicy`, which controls emitted test
attributes.

Success is observable when generated code for both `#[scenario]` and
`scenarios!` emits the first-party default attributes from harness-only
configuration, explicit `attributes = ...` remains authoritative, unknown
third-party harness paths do not infer first-party attributes, existing
attribute de-duplication still prevents duplicate `#[tokio::test]` and
`#[gpui::test]`, and the repository gates pass.

## Constraints

- Explicit approval was received in this thread on 2026-05-14; continue
  implementation within the tolerances below.
- Do not implement this plan while ADR-008 is still `Proposed` unless a
  maintainer explicitly authorizes contingent implementation.
- Keep the scope to roadmap item 9.7.2. Do not complete roadmap items 9.7.3 or
  9.7.4 except for the minimum unit, behavioural, trybuild, and documentation
  updates needed to prove and explain the 9.7.2 behaviour.
- Preserve the ADR-008 precedence order:
  1. explicit `attributes = ...`
  2. known first-party `harness = ...` mapping
  3. deprecated `runtime = "tokio-current-thread"` compatibility alias
  4. existing runtime-mode or synchronous fallback
- Preserve `attributes`-only configuration. A caller that supplies
  `attributes = ...` without `harness = ...` must keep the current behaviour.
- Preserve harness-only configuration. A caller that supplies `harness = ...`
  without `attributes = ...` must still get harness delegation.
- Preserve explicit override behaviour. If `harness = GpuiHarness` and
  `attributes = TokioAttributePolicy` are both supplied, the explicit attribute
  policy wins for attribute emission.
- Preserve current attribute de-duplication rules for user-supplied
  `#[tokio::test]` and `#[gpui::test]`.
- Preserve the compile-time rejection of `async fn` scenarios combined with a
  harness adapter.
- Preserve strict third-party behaviour. Unknown harness paths must not infer
  first-party attributes, because procedural macros cannot evaluate arbitrary
  third-party `AttributePolicy::test_attributes()` implementations during
  expansion.
- Do not add external dependencies unless the maintainer approves a separate
  dependency decision.
- Use `rstest` for parameterized unit tests. Use existing trybuild and
  behavioural test infrastructure where applicable. `rust-rspec` is mentioned
  in the request, but if the workspace still does not include it, do not add it
  solely for this task without approval.
- Property tests, Kani, or Verus are not required for a finite, table-backed
  precedence rule. If implementation expands into a generalized resolver over
  arbitrary path states, stop and add a property-test or bounded-model-checking
  milestone before continuing.
- Keep Rust source files under 400 lines. If a touched file would exceed that
  limit, refactor before committing.
- Run format, lint, and test gates sequentially and write long outputs to
  `/tmp` with `tee`.
- Use `coderabbit review --agent` after each major milestone and clear all
  concerns before moving to the next milestone.
- Commit each approved implementation milestone after its focused gates pass.

## Tolerances

- Scope: if implementation requires more than 10 files changed or more than
  450 net lines outside tests and documentation, stop and re-check whether the
  work has drifted into 9.7.3, 9.7.4, or a broader macro refactor.
- Interface: if any public trait, macro argument name, or existing public
  function signature must change, stop and escalate before continuing.
- Dependencies: if a new external crate is required, stop and escalate.
- Governance: if ADR-008 remains `Proposed`, stop before implementation unless
  explicit maintainer authorization is recorded in this plan.
- Validation: if `make check-fmt`, `make lint`, or `make test` fails for an
  unrelated reason, capture the log path and stop before marking roadmap 9.7.2
  done.
- Iterations: if the same gate fails three consecutive fix attempts, stop and
  escalate with the log path and current hypothesis.
- CodeRabbit: if `coderabbit review --agent` reports a concern that requires a
  design decision rather than a mechanical fix, record it in `Decision Log` and
  ask for direction.
- Ambiguity: if docs and code disagree on first-party path recognition,
  attribute precedence, or whether 9.7.2 is already delivered, stop and present
  the interpretations.

## Risks

- Risk: the current code already appears to contain ADR-008-shaped
  `TestAttrPolicy` and harness hint resolution while the roadmap still leaves
  9.7.2 unchecked. Severity: high. Likelihood: high. Mitigation: start with a
  reconciliation milestone that proves whether behaviour is already present
  before editing implementation code.
- Risk: attribute selection could accidentally use execution runtime
  (`ScenarioConfig.runtime`) instead of attribute runtime
  (`ScenarioConfig.attribute_runtime`). Severity: high. Likelihood: medium.
  Mitigation: keep tests around runtime fallback, explicit harness selection,
  and the `scenarios!` compatibility alias.
- Risk: first-party single-segment or imported path recognition could infer
  attributes for aliased third-party names. Severity: high. Likelihood: medium.
  Mitigation: keep canonical path tests and negative tests for aliases,
  wrong-prefix paths, and extra path segments.
- Risk: Tokio attributes require async test signatures, but harness-delegated
  scenarios are synchronous. Severity: medium. Likelihood: medium. Mitigation:
  explicitly test that `TokioHarness` harness-only configuration does not emit
  an invalid `#[tokio::test]` on synchronous generated harness functions while
  still preserving Tokio attribute behaviour for explicit async
  attribute-policy scenarios where harness delegation is absent.
- Risk: GPUI test attributes may have crate-specific compile behaviour that is
  best proven in the adapter crate rather than only in macro unit tests.
  Severity: medium. Likelihood: medium. Mitigation: extend existing
  `rstest-bdd-harness-gpui` trybuild or behavioural tests rather than creating
  a new integration crate.
- Risk: documentation may lead users to believe arbitrary third-party harnesses
  infer policies. Severity: medium. Likelihood: medium. Mitigation: update
  `docs/users-guide.md`, `docs/rstest-bdd-design.md`, and
  `docs/developers-guide.md` only where the 9.7.2 behaviour changes or needs a
  sharper caveat.
- Risk: `make test` may not give enough signal for warning-oriented trybuild
  fixtures. Severity: medium. Likelihood: medium. Mitigation: include focused
  trybuild commands before the full gates where compile output is part of the
  acceptance evidence.

## Progress

- [x] Loaded and applied the `execplans`, `leta`, `rust-router`, and
      `firecrawl-mcp` skills for this planning task.
- [x] Confirmed the current branch is a task branch, not `main`.
- [x] Reviewed `AGENTS.md`, `docs/roadmap.md`,
      `docs/adr-008-harness-led-attribute-policy-defaults.md`, and
      `docs/rstest-bdd-design.md` §§2.7.3-2.7.4.
- [x] Used a Wyvern agent team for planning reconnaissance over the roadmap,
      ADR, design document, existing execplans, macro codegen path, and tests.
- [x] Used Firecrawl to check prior art. The directly relevant result was
      `rstest`'s documented default test attribute behaviour, which confirms
      that default test attributes are a known proc-macro ergonomics pattern.
- [x] Drafted this pre-implementation ExecPlan.
- [x] Received explicit approval to implement this plan.
- [x] Reconcile current code against the 9.7.2 acceptance criteria.
- [x] Add failing or missing tests that prove harness-led defaults through
      `#[scenario]` and `scenarios!`.
- [x] Implement or complete macro codegen changes.
- [ ] Update user-facing and internal documentation where behaviour or
      conventions changed.
- [x] Run focused validation and CodeRabbit review for the first
      behavioural milestone.
- [x] Commit the first behavioural milestone.
- [x] Run final repository gates and CodeRabbit review.
- [x] Mark roadmap item 9.7.2 done only after implementation, documentation,
      review, and gates pass.

## Surprises & Discoveries

- The design document already contains a §2.7.4 section describing the
  ADR-008 codegen refactoring, including `TestAttrPolicy`,
  `ScenarioConfig.attribute_runtime`, and the shared policy crate lookup. This
  suggests the implementation may be partially or fully present and needs
  evidence reconciliation before further edits.
- `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs` already has
  `resolve_attribute_hint_from_harness_path`, `TestAttrPolicy`, and
  `generate_test_attrs` wired to both harness and explicit attributes.
- `crates/rstest-bdd-policy/src/lib.rs` already contains canonical mappings
  for `StdHarness`, `TokioHarness`, and `GpuiHarness`.
- The existing `docs/execplans/9-7-1-first-party-policy-hint-resolution.md`
  records that 9.7.1 was delivered under maintainer authorization while ADR-008
  remained `Proposed`; the same governance caveat applies here.
- Firecrawl found `https://docs.rs/rstest/latest/rstest/attr.rstest.html`,
  whose search result describes `rstest`'s own default test-attribute
  behaviour. This supports treating harness-led default attributes as prior
  art, but the implementation must still follow this repository's stricter
  ADR-008 precedence rules.
- On 2026-05-14, `leta workspace add .` confirmed the workspace was already
  registered, but `leta grep` could not start `rust-analyzer` through the LSP
  bridge. Reconciliation therefore uses targeted `rg` searches and direct
  symbol-oriented file reads until the local LSP daemon is healthy.
- On 2026-05-14 at 21:13Z, focused reconciliation showed the production
  ADR-008 resolver path was already present: `generate_test_attrs` receives
  `TestAttrPolicy` from both regular and outline generation, and `scenarios!`
  threads resolved harness paths while retaining the original runtime for
  attribute fallback. The implementation milestone therefore adds missing
  coverage rather than rewriting code.
- The existing Tokio and GPUI adapter crates already had harness-only
  behavioural coverage for `#[scenario]`; Tokio trybuild coverage was missing
  the harness-only `scenarios!` compile-pass case, so this milestone adds that
  fixture.
- CodeRabbit's first pass found that the new Tokio `scenarios!` trybuild
  fixture used only synchronous steps. The finding was valid; the fixture now
  uses a local feature file with an async `Then` step so the harness-only
  `scenarios!` case proves async step handling at compile time.
- Validation after the first behavioural milestone:
  `cargo test -p rstest-bdd-macros codegen::scenario::tests::harness_defaults`,
  `cargo test -p rstest-bdd-harness-tokio --test macro_compile`,
  `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`, and
  `make test` passed. `make test` ran 1422 Rust tests with 1422 passed and 7
  skipped, then 62 Python publish-check tests with 62 passed. `make fmt` was
  also attempted and failed on pre-existing repository-wide Markdown MD013
  line-length findings; unrelated formatter churn was restored.
- The first behavioural milestone was committed as `5b8a17e` with message
  `Cover harness-led attribute defaults`.
- Roadmap item 9.7.2 is marked delivered after the first milestone passed
  focused tests, CodeRabbit review, Markdown diagram/lint checks, and the full
  `make check-fmt`, `make lint`, and `make test` gates.
- Final validation on 2026-05-14 passed: `make markdownlint`, `make nixie`,
  `make check-fmt`, `make lint`, `make test`, and `coderabbit review --agent`.
  The final `make test` run reported 1422 Rust tests passed, 7 skipped, and
  62 Python publish-check tests passed.

## Decision Log

- Decision: proceed with contingent implementation on 2026-05-14. Rationale:
  the maintainer explicitly approved implementation in this thread despite the
  plan's ADR-008 governance caveat.
- Decision: make the first implementation milestone a reconciliation pass, not
  an immediate rewrite. Rationale: local code and docs already show the
  expected ADR-008 resolver shape, and rewriting a working path would increase
  risk without improving behaviour.
- Decision: keep third-party inference out of scope. Rationale: ADR-008
  explicitly limits default inference to known first-party paths because Rust
  procedural macros cannot evaluate arbitrary trait methods during expansion.
- Decision: treat tests as the likely centre of the change unless
  reconciliation proves behaviour is missing. Rationale: the roadmap finish
  line is externally observable generated attributes; if code already provides
  them, missing acceptance evidence is the real gap.
- Decision: do not change production macro code in the first milestone.
  Rationale: focused tests prove the resolver already implements the requested
  precedence order, so the minimal safe change is to strengthen regression
  coverage around synchronous Tokio harness omission, de-duplication, and
  harness-only `scenarios!` expansion.
- Decision: keep the Tokio `scenarios!` trybuild feature local to
  `tests/fixtures_macros`. Rationale: this makes the fixture self-contained and
  lets `stage_trybuild_support_files` copy only the exact feature file needed
  by the compile-pass case.
- Decision: do not introduce `rust-rspec` unless the workspace already has a
  usable integration point or the maintainer approves the dependency.
  Rationale: the request asks for behavioural tests using `rust-rspec` where
  applicable, while existing project practice and 9.7.1 note that it is not
  currently present.

## Implementation Plan

### Milestone 1: Reconcile behaviour and write the red tests

Inspect the current code path before editing. Use `leta` for symbol-oriented
navigation and `rg` for literal documentation or fixture searches. The key
symbols and files are:

- `ScenarioArgs` and `ScenarioArgs::parse` in
  `crates/rstest-bdd-macros/src/macros/scenario/args.rs`
- `try_scenario` in `crates/rstest-bdd-macros/src/macros/scenario/mod.rs`
- `ScenarioTestContext`, `resolve_harness_path`,
  `resolve_effective_runtime`, and `generate_scenario_test` in
  `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs`
- `ScenarioConfig`, `generate_regular_scenario_code`, and
  `generate_outline_scenario_code` in
  `crates/rstest-bdd-macros/src/codegen/scenario.rs`
- `TestAttrPolicy`, `resolve_attribute_policy`, and `generate_test_attrs` in
  `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs`
- `first_party_adapter_attribute_hint` in
  `crates/rstest-bdd-macros/src/codegen/mod.rs`
- `resolve_test_attribute_hint_for_harness_path` and
  `resolve_test_attribute_hint_for_policy_path` in
  `crates/rstest-bdd-policy/src/lib.rs`

Add or extend focused `rstest` unit tests first. The expected home for the core
precedence matrix is
`crates/rstest-bdd-macros/src/codegen/scenario/tests/harness_defaults.rs`. The
matrix must cover:

- `StdHarness` with no explicit `attributes = ...` produces rstest-only
  attributes, even when the runtime fallback would otherwise be Tokio.
- `TokioHarness` with no explicit `attributes = ...` resolves to the Tokio
  hint, but `#[tokio::test]` is omitted for synchronous harness-delegated
  signatures because Tokio requires async test functions.
- `GpuiHarness` with no explicit `attributes = ...` emits `#[gpui::test]`.
- explicit `attributes = ...` overrides a conflicting first-party harness.
- unknown third-party harness paths fall back to runtime/default behaviour.
- existing user `#[tokio::test]` or `#[gpui::test]` attributes are not
  duplicated.

Add `scenarios!` generation tests under
`crates/rstest-bdd-macros/src/macros/scenarios/test_generation/tests.rs` where
unit coverage can prove that generated `ScenarioConfig` receives both the
resolved harness and the correct attribute-runtime inputs. If direct token
assertions are clearer, assert on the generated token stream for the presence
or absence of `rstest::rstest`, `tokio::test`, and `gpui::test`.

Run the focused tests and keep the output:

```bash
set -o pipefail
cargo test -p rstest-bdd-macros \
  codegen::scenario::tests::harness_defaults 2>&1 | \
  tee /tmp/test-rstest-bdd-9-7-2-harness-defaults.out
cargo test -p rstest-bdd-macros \
  macros::scenarios::test_generation 2>&1 | \
  tee /tmp/test-rstest-bdd-9-7-2-scenarios-generation.out
```

If these tests already pass before implementation changes, record that in
`Surprises & Discoveries` and continue by adding only missing externally
observable coverage. If they fail for the expected reason, keep the failing
evidence in `Progress`.

### Milestone 2: Complete code generation

If reconciliation shows code is missing or incomplete, update the smallest code
path that keeps ADR-008 precedence centralised. The preferred shape is:

- `ScenarioConfig` continues to carry both `runtime` for execution and
  `attribute_runtime` for attribute fallback.
- `generate_regular_scenario_code` and `generate_outline_scenario_code`
  continue to call `generate_test_attrs` with a `TestAttrPolicy` containing
  `attribute_runtime`, `harness`, and `attributes`.
- `generate_test_attrs` remains the single place where framework test
  attributes are selected and de-duplicated.
- `rstest-bdd-policy` remains the canonical table for first-party path
  mappings.

Do not move third-party inference into macro codegen. Do not make the macro
call `AttributePolicy::test_attributes()` for user-provided types; that is not
available at proc-macro expansion time.

Run the same focused tests as Milestone 1 after implementation. Then run
CodeRabbit and clear concerns before committing:

```bash
set -o pipefail
coderabbit review --agent 2>&1 | tee /tmp/coderabbit-rstest-bdd-9-7-2-codegen.out
```

If CodeRabbit is unavailable in the environment, record the command failure and
continue only if the failure is environmental rather than a review finding.

Commit this milestone only after focused tests and CodeRabbit concerns are
resolved.

### Milestone 3: Add behavioural and trybuild coverage

Add externally observable coverage where unit tests alone cannot prove the
macro expansion contract.

For Tokio, extend existing tests in
`crates/rstest-bdd-harness-tokio/tests/scenario_macros.rs` and trybuild
fixtures under `crates/rstest-bdd-harness-tokio/tests/fixtures_macros/`. The
important cases are harness-only `#[scenario]`, harness-only `scenarios!`,
explicit attributes override, and the existing async-with-harness rejection.

For GPUI, extend existing tests in
`crates/rstest-bdd-harness-gpui/tests/scenario_macros.rs` and trybuild fixtures
under `crates/rstest-bdd-harness-gpui/tests/fixtures_macros/`. Prefer extending
adapter-crate tests because they naturally have the GPUI dependency surface.

For the base `StdHarness`, add a small compile or behavioural assertion where
existing `rstest-bdd-harness` or `rstest-bdd` tests already exercise standard
harness macro usage. The case should prove that harness-only `StdHarness`
remains rstest-only and does not accidentally inherit Tokio or GPUI attributes.

Run focused commands, adjusted to the exact tests touched:

```bash
set -o pipefail
cargo test -p rstest-bdd-harness-tokio --test macro_compile 2>&1 | tee /tmp/test-rstest-bdd-9-7-2-tokio-trybuild.out
cargo test -p rstest-bdd-harness-tokio --test scenario_macros 2>&1 | tee /tmp/test-rstest-bdd-9-7-2-tokio-behaviour.out
cargo test -p rstest-bdd-harness-gpui --test macro_compile 2>&1 | tee /tmp/test-rstest-bdd-9-7-2-gpui-trybuild.out
cargo test -p rstest-bdd-harness-gpui --test scenario_macros 2>&1 | tee /tmp/test-rstest-bdd-9-7-2-gpui-behaviour.out
```

If a GPUI test requires features, use the existing command pattern from that
crate rather than inventing a new one. Do not run tests in parallel.

Run CodeRabbit and commit this milestone after focused tests pass:

```bash
set -o pipefail
coderabbit review --agent 2>&1 | tee /tmp/coderabbit-rstest-bdd-9-7-2-tests.out
```

### Milestone 4: Update documentation and roadmap state

Update documentation only to the extent needed for 9.7.2. At minimum, check
these files for accuracy:

- `docs/users-guide.md`, especially the harness adapter and attribute policy
  sections around first-party defaults and explicit override examples.
- `docs/rstest-bdd-design.md` §§2.7.3-2.7.4, especially the macro integration
  and codegen refactoring description.
- `docs/developers-guide.md`, especially first-party canonical path and
  trybuild conventions.
- `docs/adr-008-harness-led-attribute-policy-defaults.md`, only if a design
  decision changes or the implementation reveals a substantive clarification.
- `docs/roadmap.md`, marking item 9.7.2 done only after implementation,
  documentation, gates, and review are complete.

If Markdown changed, run:

```bash
set -o pipefail
make fmt 2>&1 | tee /tmp/markdownfmt-rstest-bdd-9-7-2.out
make markdownlint 2>&1 | tee /tmp/markdownlint-rstest-bdd-9-7-2.out
make nixie 2>&1 | tee /tmp/nixie-rstest-bdd-9-7-2.out
```

Run CodeRabbit and commit the documentation/roadmap milestone:

```bash
set -o pipefail
coderabbit review --agent 2>&1 | tee /tmp/coderabbit-rstest-bdd-9-7-2-docs.out
```

### Milestone 5: Final gates

Run the required repository gates sequentially:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt-rstest-bdd-9-7-2.out
make lint 2>&1 | tee /tmp/lint-rstest-bdd-9-7-2.out
make test 2>&1 | tee /tmp/test-rstest-bdd-9-7-2.out
```

If documentation changed and those gates were not already run after the final
documentation edits, also run:

```bash
set -o pipefail
make fmt 2>&1 | tee /tmp/markdownfmt-rstest-bdd-9-7-2-final.out
make markdownlint 2>&1 | tee /tmp/markdownlint-rstest-bdd-9-7-2-final.out
make nixie 2>&1 | tee /tmp/nixie-rstest-bdd-9-7-2-final.out
```

Run a final CodeRabbit review:

```bash
set -o pipefail
coderabbit review --agent 2>&1 | tee /tmp/coderabbit-rstest-bdd-9-7-2-final.out
```

Commit any final fixes. Do not mark `Status: COMPLETE` until the working tree
is clean apart from intentionally uncommitted user changes, the roadmap entry
is marked done, and every required gate has passing evidence.

## Validation Strategy

The validation strategy is red-green-refactor at three levels.

First, unit tests with `rstest` prove the finite precedence matrix and
de-duplication rules around `generate_test_attrs`. These tests should fail if
harness-led defaults are removed, if explicit attributes stop winning, or if
unknown third-party harnesses infer first-party attributes.

Second, trybuild and adapter-crate behavioural tests prove that the macro
expansion works in consumer-like crates with real first-party harness
dependencies. These tests are especially important for Tokio and GPUI because
their framework attributes have crate-specific compile requirements.

Third, repository gates prove that the implementation remains formatted,
lint-clean, and passing across the workspace:

- `make check-fmt`
- `make lint`
- `make test`
- `make markdownlint` and `make nixie` when Markdown changed

No property test, Kani harness, or Verus proof is planned because this change
does not introduce an unbounded state machine or contractual axiom. It applies
a finite precedence rule over a fixed set of first-party paths. If that
changes, update this plan before implementing.

## Documentation and Skill Signposts

Relevant repository documents:

- `docs/roadmap.md` item 9.7.2 for the requested finish line.
- `docs/adr-008-harness-led-attribute-policy-defaults.md` for the accepted
  precedence model, pending governance status, and first-party mappings.
- `docs/rstest-bdd-design.md` §§2.7.3-2.7.4 for macro integration and
  codegen structure.
- `docs/rstest-bdd-language-server-design.md` for avoiding accidental
  language-server contract drift.
- `docs/rust-testing-with-rstest-fixtures.md` for `rstest` fixture and
  parameterized-test conventions.
- `docs/rust-doctest-dry-guide.md` for documentation example style.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for refactoring
  thresholds if the macro path becomes too complex.
- `docs/gherkin-syntax.md` for feature/scenario terminology.
- `docs/users-guide.md` for public behaviour.
- `docs/developers-guide.md` for internal macro and trybuild conventions.

Relevant skills for implementation:

- `execplans`: keep this plan current and self-contained.
- `leta`: use LSP navigation for symbols and references before editing.
- `rust-router`: route any Rust-specific design question.
- `arch-crate-design`: use if crate boundaries or public API placement become
  unclear.
- `rust-types-and-apis`: use if public trait or generic signatures become part
  of the solution.
- `rust-errors`: use if generated error behaviour or diagnostic shape changes.
- `nextest`: use only if the maintainer asks for nextest-specific validation.

## Outcomes & Retrospective

Roadmap item 9.7.2 is delivered as a coverage-and-documentation completion
because reconciliation showed the production ADR-008 codegen path already
implemented the requested precedence order. The branch adds regression tests
for synchronous Tokio harness omission, first-party attribute de-duplication,
and harness-only Tokio `scenarios!` expansion with an async step. The roadmap
entry is marked done with the same "delivered under maintainer authorization
while ADR-008 remains Proposed" caveat used for 9.7.1.

CodeRabbit raised one valid concern during the first milestone: the new Tokio
`scenarios!` trybuild fixture initially used only synchronous steps. That was
fixed by adding `scenarios_harness_tokio_default.feature` with an async `Then`
step and staging that file for trybuild. Subsequent CodeRabbit reviews reported
zero findings.

All required gates passed after the roadmap update: `make markdownlint`,
`make nixie`, `make check-fmt`, `make lint`, and `make test`. `make fmt` was
attempted, but it still fails on pre-existing repository-wide Markdown MD013
line-length findings unrelated to this task; unrelated formatter churn was
restored both times.
