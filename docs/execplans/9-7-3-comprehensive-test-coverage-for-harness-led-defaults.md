# Cover harness-led attribute-policy defaults

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

This plan covers roadmap item 9.7.3 only. It must be approved before
implementation begins. While
`docs/adr-008-harness-led-attribute-policy-defaults.md` remains in `Proposed`
status, implementation remains contingent unless a maintainer explicitly
authorizes it.

## Purpose / big picture

Roadmap item 9.7.3 adds comprehensive test coverage for the harness-led
attribute-policy defaults delivered by the earlier 9.7.1 and 9.7.2 work. The
behaviour under test is the Architecture Decision Record (ADR) 008 rule that
first-party harness choices can imply their matching first-party test
attributes when `attributes = ...` is omitted.

After this work is implemented, maintainers can change the macro code with
confidence because the test suite will prove four observable behaviours:

- harness-only Tokio and GPUI scenarios receive first-party test attributes
  when the generated function signature permits those attributes;
- explicit `attributes = ...` overrides beat any harness-led default;
- `attributes`-only scenarios keep their existing behaviour; and
- unknown third-party harness paths do not infer first-party defaults.

The work is deliberately coverage-focused. The current documentation already
describes harness-led defaults, and roadmap item 9.7.4 is the dedicated
follow-up for leading user-facing examples with harness-only configuration.
This plan should only update user, developer, design, or ADR prose if test work
uncovers an actual mismatch that would mislead an implementer.

## Constraints

- Do not implement this plan until the maintainer explicitly approves it.
- Treat implementation as contingent while ADR-008 remains `Proposed`, unless
  the maintainer explicitly authorizes contingent implementation.
- Keep scope to roadmap item 9.7.3. Do not mark 9.7.4 done, and do not perform
  broad documentation rewrites that belong to 9.7.4.
- Preserve the ADR-008 precedence order:
  1. explicit `attributes = ...`
  2. known first-party `harness = ...` mapping
  3. deprecated `runtime = "tokio-current-thread"` compatibility alias
  4. existing runtime-mode or synchronous fallback
- Preserve ADR-005's crate boundary. Tokio and Graphical Processing User
  Interface (GPUI) test dependencies must remain in their first-party harness
  adapter crates, not in the core runtime or macro crates.
- Preserve `attributes`-only configuration. A caller that supplies
  `attributes = ...` without `harness = ...` must keep the current behaviour.
- Preserve strict third-party behaviour. Unknown harness paths must not infer
  first-party attributes because Rust procedural macros cannot evaluate
  arbitrary third-party `AttributePolicy::test_attributes()` implementations
  during expansion.
- Use `rstest` for parameterized unit tests. Use the existing trybuild and
  behavioural test infrastructure where applicable.
- The request mentions `rust-rspec`; if the workspace still does not include it,
  do not add it solely for this coverage milestone without explicit approval.
  Existing `rstest-bdd` behavioural scenario tests are the local
  Behaviour-Driven Development (BDD) validation surface.
- Property tests, Kani, or Verus are not required for the finite path and
  precedence matrix in this task. If the implementation expands into a
  generalized resolver over arbitrary input states, stop and add a
  property-test or bounded model-checking milestone before continuing.
- Do not add external dependencies unless the maintainer approves a separate
  dependency decision.
- Keep Rust source files under 400 lines. If a touched file would exceed that
  limit, refactor before committing.
- Run format, lint, and test gates sequentially. Write long command output to
  `/tmp` with `tee`.
- Use `coderabbit review --agent` after each major implementation milestone and
  clear all concerns before moving to the next milestone.
- Commit each approved implementation milestone after its focused gates pass.

## Tolerances

- Scope: if implementation requires more than 12 files changed or more than 500
  net lines outside tests and this ExecPlan, stop and re-check whether the work
  has drifted into 9.7.4 or a broader macro refactor.
- Interface: if any public trait, macro argument name, or existing public
  function signature must change, stop and escalate before continuing.
- Dependencies: if a new external crate is required, stop and escalate.
- Governance: if ADR-008 remains `Proposed`, stop before implementation unless
  explicit maintainer authorization is recorded in this plan.
- Validation: if `make check-fmt`, `make lint`, or `make test` fails for an
  unrelated reason, capture the log path and stop before marking roadmap 9.7.3
  done.
- Iterations: if the same gate fails three consecutive fix attempts, stop and
  escalate with the log path and current hypothesis.
- CodeRabbit: if `coderabbit review --agent` reports a concern that requires a
  design decision rather than a mechanical fix, record it in `Decision Log` and
  ask for direction.
- Ambiguity: if docs and code disagree on first-party path recognition,
  attribute precedence, or whether 9.7.3 is already delivered, stop and present
  the interpretations.

## Risks

- Risk: the implementation already contains much of the ADR-008 resolver shape,
  so new tests could overfit internals instead of proving user-visible macro
  behaviour. Severity: medium. Likelihood: medium. Mitigation: combine focused
  unit tests with trybuild compile-pass fixtures and behavioural scenario tests
  in the first-party harness crates.
- Risk: Tokio test attributes are only valid for `async fn` test signatures,
  while harness-delegated Tokio scenarios are generated as synchronous
  functions. Severity: high. Likelihood: medium. Mitigation: explicitly test
  both permitted and omitted cases. Harness-only Tokio coverage must prove that
  invalid `#[tokio::test]` emission is avoided for synchronous
  harness-delegated functions, while async `attributes`-only scenarios still
  receive Tokio attributes.
- Risk: GPUI tests can be hidden by feature gating or platform constraints.
  Severity: medium. Likelihood: medium. Mitigation: keep GPUI compile and
  behavioural coverage co-located in `rstest-bdd-harness-gpui`, follow its
  existing feature-gated test pattern, and run the repository gates that
  exercise the configured workspace.
- Risk: unknown third-party harness paths could silently receive first-party
  attributes if a test accepts name-only matches too broadly. Severity: high.
  Likelihood: low. Mitigation: include wrong-prefix, extra-segment and
  third-party-like negative cases at the unit level, then add only externally
  meaningful trybuild coverage.
- Risk: `attributes`-only behaviour could regress, while harness-only tests
  still pass. Severity: high. Likelihood: medium. Mitigation: include explicit
  `attributes`-only fixtures for Tokio and GPUI in the first-party adapter
  crates, and keep macro unit tests around explicit policy precedence.
- Risk: documentation updates may duplicate 9.7.4 scope.
  Severity: low. Likelihood: medium. Mitigation: leave `docs/users-guide.md`,
  `docs/developers-guide.md`, `docs/rstest-bdd-design.md`, and ADR prose
  unchanged unless a tested behaviour contradicts the current text.

## Progress

- [x] (2026-05-19) Loaded and applied the `leta`, `rust-router`,
      `arch-crate-design`, `execplans`, `firecrawl-mcp`,
      `en-gb-oxendict-style`, `commit-message`, and `pr-creation` skills.
- [x] (2026-05-19) Created the `leta` workspace for this checkout with
      `leta workspace add`.
- [x] (2026-05-19) Confirmed the starting branch was not `main` and renamed it
      to `9-7-3-comprehensive-test-coverage-for-harness-led-defaults`.
- [x] (2026-05-19) Checked that the requested remote branch did not yet exist.
- [x] (2026-05-19) Asked a Wyvern agent team to inspect roadmap and ADR
      constraints, implementation and test surfaces, and validation workflow.
- [x] (2026-05-19) Used Firecrawl to verify relevant prior-art and tool
      behaviour for trybuild, rstest, rust-rspec and Kani.
- [x] (2026-05-19) Drafted this pre-implementation ExecPlan.
- [x] (2026-05-19) Validated the draft with `make markdownlint`,
      `make nixie`, `make check-fmt`, `make lint`, and `make test`.
- [x] (2026-05-19) Ran `coderabbit review --agent` on the draft and addressed
      its prose findings.
- [ ] Await explicit maintainer approval before implementation.
- [ ] Reconcile current tests against the 9.7.3 coverage matrix.
- [ ] Add missing unit coverage for precedence, signature-permitted emission,
      explicit overrides, `attributes`-only cases and unknown harness paths.
- [ ] Add or extend trybuild fixtures in the Tokio and GPUI adapter crates.
- [ ] Add or extend behavioural scenario tests for first-party harnesses.
- [ ] Run focused validation and CodeRabbit review for the coverage milestone.
- [ ] Update only the documentation that demonstrably mismatches tested
      behaviour, if any.
- [ ] Run final repository gates and CodeRabbit review.
- [ ] Mark roadmap item 9.7.3 done only after implementation, review, and gates
      pass.

## Surprises & Discoveries

- ADR-008 remains `Proposed`, while roadmap items 9.7.1 and 9.7.2 are already
  marked delivered under maintainer authorization. This plan therefore keeps
  the same governance caveat until the maintainer approves 9.7.3 implementation.
- Existing code already contains the key ADR-008 implementation surfaces:
  `TestAttrPolicy`, `resolve_attribute_policy`, shared first-party policy path
  constants, and harness-path hint resolution. The first implementation step
  should therefore be a coverage reconciliation rather than a rewrite.
- Existing Tokio and GPUI adapter crates already own their trybuild and
  behavioural integration suites. This matches ADR-005's boundary and gives
  9.7.3 a natural place to add first-party coverage without pulling heavy
  dependencies into `rstest-bdd`.
- Firecrawl confirmed trybuild's documented role as a compiler diagnostics test
  harness for `pass` and `compile_fail` cases, including procedural macro use.
- Firecrawl confirmed rstest's own attribute model: synchronous tests get a
  default test attribute, while async tests require an explicit or implicit
  test attribute. This supports the plan's distinction between permitted Tokio
  attribute emission and synchronous harness-delegated omission.
- Firecrawl found rust-rspec as the `rspec` crate, a stable-Rust BDD-style test
  harness, but this workspace does not currently use it. Adding it would be a
  dependency decision, not a coverage-only implementation detail.
- Firecrawl confirmed Kani is useful for model-checking safety and correctness
  properties through proof harnesses, but the present task is a finite
  precedence and path-recognition matrix, so Kani is not justified unless the
  implementation scope changes.
- `make fmt` was attempted during plan drafting, but it reported broad
  pre-existing Markdown line-length findings and touched unrelated tracked
  files. The unrelated formatter churn was restored, and the committed plan is
  validated with the configured documentation gates instead.

## Decision Log

- Decision: keep this plan pre-implementation and `DRAFT` until maintainer
  approval is explicit. Rationale: the user specifically requested a plan and
  said the plan must be approved before implementation.
- Decision: centre 9.7.3 on tests, not production resolver rewrites.
  Rationale: roadmap item 9.7.3's finish line is coverage evidence, and the
  current code already contains the ADR-008 resolver and codegen shape from
  9.7.1 and 9.7.2.
- Decision: keep 9.7.4 documentation rewrites out of scope.
  Rationale: roadmap item 9.7.4 explicitly owns the user-guide and design-doc
  prose update that leads with harness-only examples.
- Decision: do not introduce rust-rspec as a new dependency for this task.
  Rationale: the repository already has BDD scenario behavioural tests through
  its own macros, and adding a new testing framework would exceed a
  coverage-only milestone without explicit approval.
- Decision: do not plan Kani or Verus work for the default coverage matrix.
  Rationale: the requirement concerns a finite, table-backed precedence rule
  rather than an unbounded state, ordering, transition, or proof obligation.

## Implementation Plan

Begin only after explicit approval. First, reconcile the current repository
state against the coverage matrix. Inspect
`crates/rstest-bdd-macros/src/codegen/scenario/tests/harness_defaults.rs`,
`crates/rstest-bdd-policy/src/lib.rs`, the Tokio and GPUI `macro_compile.rs`
suites, and the Tokio and GPUI `scenario_macros.rs` suites. Record in
`Surprises & Discoveries` which roadmap cases already have coverage and which
are missing.

Next, add the missing unit-level matrix cases. The preferred home is
`crates/rstest-bdd-macros/src/codegen/scenario/tests/harness_defaults.rs`
because it already tests `generate_test_attrs` and `TestAttrPolicy`. Use
`rstest` cases to cover:

- explicit Tokio and GPUI `attributes = ...` overriding mismatched harness
  defaults;
- `attributes`-only Tokio and GPUI behaviour with no harness path;
- unknown third-party harness paths with synchronous and Tokio fallback
  runtimes; and
- signature-permitted emission, especially Tokio's omission for synchronous
  harness functions and emission for async `attributes`-only functions.

If the shared resolver in `crates/rstest-bdd-policy/src/lib.rs` lacks
equivalent negative cases for wrong prefixes, extra path segments, or
third-party-like paths, add them there rather than duplicating path-table
checks in macro tests.

Then extend trybuild coverage where compile-time behaviour is more meaningful
than token-string inspection. For Tokio, use
`crates/rstest-bdd-harness-tokio/tests/macro_compile.rs` and fixtures under
`crates/rstest-bdd-harness-tokio/tests/fixtures_macros/`. Ensure there is
compile-pass coverage for harness-only `#[scenario]`, harness-only
`scenarios!`, explicit override, and `attributes`-only configuration. Include
an async-step or async-signature case only where it proves a real difference in
attribute emission and does not violate the harness `async fn` rejection rule.

For GPUI, use `crates/rstest-bdd-harness-gpui/tests/macro_compile.rs` and
fixtures under `crates/rstest-bdd-harness-gpui/tests/fixtures_macros/`. Mirror
the Tokio matrix where the framework semantics permit it: harness-only
`#[scenario]`, harness-only `scenarios!`, explicit override, and
`attributes`-only configuration. Keep GPUI-specific tests inside the GPUI
adapter crate, so the core crate remains free of GPUI dependencies.

After compile-time coverage, add or extend behavioural scenario tests in
`crates/rstest-bdd-harness-tokio/tests/scenario_macros.rs` and
`crates/rstest-bdd-harness-gpui/tests/scenario_macros.rs`. These tests should
prove end-to-end execution under harness-led defaults and explicit overrides
where the behaviour can be observed without depending on proc-macro token
format. For Tokio, observable behaviour can include a current Tokio runtime or
`spawn_local` availability under the harness. For GPUI, observable behaviour
can include access to `gpui::TestAppContext` where the existing shim supports
it.

Run focused validation after the coverage milestone. Use commands shaped like:

```bash
branch=$(git branch --show-current)
cargo test -p rstest-bdd-policy 2>&1 | tee "/tmp/policy-$branch.out"
cargo test -p rstest-bdd-macros \
  codegen::scenario::tests::harness_defaults 2>&1 \
  | tee "/tmp/macro-harness-defaults-$branch.out"
cargo test -p rstest-bdd-harness-tokio --test macro_compile 2>&1 \
  | tee "/tmp/tokio-macro-compile-$branch.out"
cargo test -p rstest-bdd-harness-tokio --test scenario_macros 2>&1 \
  | tee "/tmp/tokio-scenario-macros-$branch.out"
cargo test -p rstest-bdd-harness-gpui --test macro_compile 2>&1 \
  | tee "/tmp/gpui-macro-compile-$branch.out"
cargo test -p rstest-bdd-harness-gpui --test scenario_macros 2>&1 \
  | tee "/tmp/gpui-scenario-macros-$branch.out"
```

If the GPUI tests are feature-gated in the current workspace configuration,
follow the existing crate pattern rather than forcing an ad hoc feature set. If
any focused command requires a feature not included by `make test`, record the
exact command and rationale in `Decision Log`.

Run `coderabbit review --agent` after the focused coverage milestone. Address
or record every concern before moving on.

If implementation changes Markdown or Mermaid diagrams, run:

```bash
branch=$(git branch --show-current)
make fmt 2>&1 | tee "/tmp/fmt-$branch.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-$branch.out"
make nixie 2>&1 | tee "/tmp/nixie-$branch.out"
```

Finally, run the repository gates sequentially:

```bash
branch=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-$branch.out"
make lint 2>&1 | tee "/tmp/lint-$branch.out"
make test 2>&1 | tee "/tmp/test-$branch.out"
coderabbit review --agent 2>&1 | tee "/tmp/coderabbit-final-$branch.out"
```

When all focused checks, CodeRabbit review, and final gates pass, update
`docs/roadmap.md` to mark 9.7.3 done with a concise delivery note. Commit that
roadmap update with the tested coverage change, unless it is clearer as a
separate final documentation commit after the coverage commit has already
passed gates.

## Validation Plan

The final implementation is acceptable only when the following are true:

- unit tests prove ADR-008 precedence, signature-permitted emission, explicit
  overrides, `attributes`-only cases, and unknown harness fallback;
- trybuild fixtures compile or fail in the first-party adapter crates exactly
  where expected;
- behavioural scenario tests prove Tokio and GPUI first-party harnesses still
  execute under harness-led defaults;
- `make check-fmt`, `make lint`, and `make test` pass;
- `coderabbit review --agent` reports no unresolved concerns; and
- `docs/roadmap.md` marks item 9.7.3 done only after the coverage, review, and
  gates are complete.

## External references

Firecrawl was used to resolve tooling and prior-art gaps:

- <https://docs.rs/trybuild/latest/trybuild/> documents trybuild as a compiler
  diagnostics test harness with `pass` and `compile_fail` cases.
- <https://docs.rs/rstest/latest/rstest/attr.rstest.html> documents rstest's
  fixture, parameterized-case and test-attribute behaviour.
- <https://crates.io/crates/rspec/1.0.0-beta.3> documents rust-rspec as the
  `rspec` crate, a BDD-style Rust test harness.
- <https://model-checking.github.io/kani/> documents Kani as an open-source Rust
  verifier using model checking and proof harnesses.

## Outcomes & Retrospective

Not started. This section must be updated during and after implementation with
what changed, which gates passed, any review findings, and whether the coverage
matrix exposed gaps in the existing ADR-008 implementation.
