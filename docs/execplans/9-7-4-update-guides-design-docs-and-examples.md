# Update harness-led defaults documentation and examples

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
 `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This plan covers roadmap item 9.7.4 only. It must be approved before
implementation begins. While
`docs/adr-008-harness-led-attribute-policy-defaults.md` remains in `Proposed`
status, implementation remains contingent unless a maintainer explicitly
authorizes it.

## Purpose / big picture

Roadmap item 9.7.4 updates the user-facing documentation and first-party
example prose after roadmap items 9.7.1, 9.7.2, and 9.7.3 delivered harness-led
attribute-policy defaults. The user-visible behaviour is that first-party Tokio
and Graphical Processing User Interface (GPUI) integrations can be configured
with `harness = ...` alone. When the harness path is one of the recognized
first-party paths, the macro infers the matching first-party attribute policy
unless the caller supplies an explicit `attributes = ...` override.

After this work is implemented, a reader can start in `docs/users-guide.md`,
`docs/rstest-bdd-design.md`, `docs/v0-6-0-migration-guide.md`, or the
first-party example crates and see the same recommendation: use harness-only
configuration for first-party Tokio and GPUI scenarios by default; use
`attributes = ...` only for explicit overrides, attributes-only use, or
third-party caveats. The finish line is observable when the examples no longer
teach paired `harness = ...` and `attributes = ...` arguments as the normal
first-party path, the third-party limitation remains explicit, and the
repository quality gates pass.

This task is documentation-led, but it may touch example test source if prose
and snippets would otherwise disagree with compiling first-party examples. If
example source changes are necessary, they must be treated as implementation
changes and validated with the affected example tests as well as the full
repository gates.

## Constraints

- The maintainer approved implementation on 2026-05-26 by asking Codex to
  proceed with the planned functionality.
- Treat implementation as contingent while ADR-008 remains `Proposed`, unless
  the maintainer explicitly authorizes contingent implementation.
- Keep scope to roadmap item 9.7.4. Do not redesign the harness adapter layer,
  attribute policy resolver, macro argument syntax, or first-party adapter
  crates.
- Preserve the ADR-008 precedence order:
  1. explicit `attributes = ...`
  2. known first-party `harness = ...` mapping
  3. deprecated `runtime = "tokio-current-thread"` compatibility alias
  4. existing runtime-mode or synchronous fallback
- Preserve support for `attributes`-only configuration.
- Preserve the third-party caveat: arbitrary user-defined
  `AttributePolicy::test_attributes()` implementations are not evaluated during
  procedural macro expansion, so unknown third-party harnesses must document
  explicit `attributes = ...` use where custom test attributes are required.
- Preserve ADR-005 crate boundaries. Tokio and GPUI dependencies must remain
  in opt-in adapter crates, not in `rstest-bdd`, `rstest-bdd-macros`, or
  `rstest-bdd-harness`.
- Update `docs/v0-6-0-migration-guide.md` for every stage 9.7 change that
  affects library consumers, usage, or newly available functionality.
- Keep documentation in en-GB Oxford spelling and follow
  `docs/documentation-style-guide.md`.
- Use `make fmt` after documentation changes unless it would touch unrelated
  files. If unrelated formatter churn appears, stop, record the finding in
  `Decision Log`, and validate changed Markdown with focused linting before
  continuing.
- Validate Markdown with `make markdownlint`; validate diagrams with
  `make nixie` if any Mermaid diagram is changed.
- Run `make check-fmt`, `make lint`, `make typecheck`, and `make test`
  sequentially before each CodeRabbit review and before committing the
  completed implementation.
- Use `coderabbit review --agent` after each major implementation milestone.
  CodeRabbit concerns must be addressed or explicitly recorded before moving on.
- Commit focused changes after their gates pass. Use the commit-message skill
  and `git commit -F`, not `git commit -m`.
- Do not mark roadmap item 9.7.4 done until the documentation/example update
  has been implemented, validated, reviewed, and committed. This draft plan
  does not authorize that roadmap status change.
- Use relevant skills during implementation: `execplans` for this living plan,
  `leta` for code navigation, `rust-router` plus `arch-crate-design` for crate
  boundary checks, `en-gb-oxendict-style` for prose, `firecrawl-mcp` for
  external tooling checks, `commit-message` for commits, and `pr-creation` for
  pull request metadata.

## Tolerances (exception triggers)

- Scope: if implementation requires more than 10 files changed or more than
  700 net lines, stop and confirm whether the work has expanded beyond roadmap
  item 9.7.4.
- Code scope: if any Rust source change is needed beyond removing redundant
  first-party `attributes = ...` arguments from example tests, stop and update
  this plan before proceeding.
- Interface: if any public trait, macro argument, crate feature, or documented
  public API signature must change, stop and escalate.
- Dependencies: if a new external dependency is required, stop and escalate.
- Governance: if ADR-008 is still `Proposed` at implementation time, stop
  before changing canonical guidance unless maintainer authorization is
  recorded in `Decision Log`.
- Validation: if `make check-fmt`, `make lint`, `make typecheck`, or
  `make test` fails for an unrelated reason, capture the log path and stop
  before marking roadmap 9.7.4 done.
- Iterations: if the same gate fails three consecutive fix attempts, stop and
  escalate with the log path, current hypothesis, and options.
- CodeRabbit: if `coderabbit review --agent` reports a concern requiring a
  design decision rather than a mechanical documentation fix, record it in
  `Decision Log` and ask for direction.
- Ambiguity: if `docs/users-guide.md`, `docs/rstest-bdd-design.md`,
  `docs/v0-6-0-migration-guide.md`, ADR-008, and the implementation disagree on
  first-party inference or third-party caveats, stop and present the
  interpretations.

## Risks

- Risk: documentation could overstate ADR-008 while the ADR remains
  `Proposed`. Severity: high. Likelihood: medium. Mitigation: keep the plan
  contingent until approval and, during implementation, either wait for ADR-008
  acceptance or record explicit maintainer authorization.
- Risk: examples could imply all harnesses infer attribute policies.
  Severity: high. Likelihood: medium. Mitigation: keep first-party wording
  narrow and repeat that third-party harnesses need explicit policy guidance
  when custom attributes are required.
- Risk: removing `attributes = ...` from example source could hide explicit
  override coverage. Severity: medium. Likelihood: medium. Mitigation: keep at
  least one clear override example in the user guide and rely on existing
  roadmap 9.7.3 tests for explicit override behaviour.
- Risk: `docs/users-guide.md` already contains duplicated or misplaced harness
  snippets, so a narrow edit could leave contradictory examples behind.
  Severity: medium. Likelihood: high. Mitigation: audit all `harness =`,
  `attributes =`, `TokioHarness`, and `GpuiHarness` occurrences in the guide,
  design document, migration guide, first-party example READMEs, and example
  tests.
- Risk: documentation-only changes may still break Markdown formatting or
  Mermaid validation. Severity: low. Likelihood: medium. Mitigation: run
  `make markdownlint`, `make nixie` when diagrams change, and full repository
  gates before review.
- Risk: CodeRabbit review can take 7 to 30 or more minutes according to the
  current CodeRabbit CLI documentation. Severity: low. Likelihood: medium.
  Mitigation: run reviews as explicit milestones and wait for completion rather
  than treating a long-running review as a failure.
- Risk: property tests, Kani, or Verus could be requested by the generic task
  template but are not justified by a prose-only update. Severity: low.
  Likelihood: low. Mitigation: record that no new invariant, state machine, or
  proof obligation is introduced unless implementation unexpectedly changes
  resolver code.

## Progress

- [x] (2026-05-24T16:36:48Z) Loaded `leta`, `rust-router`, `execplans`,
      `firecrawl-mcp`, `commit-message`, and `pr-creation` guidance.
- [x] (2026-05-24T16:36:48Z) Created a Leta workspace for this checkout with
      `leta workspace add`.
- [x] (2026-05-24T16:36:48Z) Confirmed the starting branch was not `main` and
      renamed it to `9-7-4-update-guides-design-docs-and-examples`.
- [x] (2026-05-24T16:36:48Z) Created context pack `pk_c63lbw6a` for the
      Wyvern-assisted planning activity.
- [x] (2026-05-24T16:36:48Z) Used a Wyvern agent team for read-only planning
      reconnaissance over the roadmap, ADR-008, user guide, design document,
      migration guide, and first-party examples.
- [x] (2026-05-24T16:36:48Z) Used Firecrawl to verify external tooling
      assumptions for CodeRabbit CLI `--agent` mode and Markdown linting.
- [x] (2026-05-24T16:36:48Z) Drafted this pre-implementation ExecPlan.
- [x] (2026-05-24T16:36:48Z) Validated this ExecPlan directly with
      `markdownlint-cli2 docs/execplans/9-7-4-update-guides-design-docs-and-examples.md`.
- [x] (2026-05-24T16:36:48Z) Ran `make markdownlint`; it failed on
      pre-existing `docs/users-guide.md` lint errors not introduced by this
      plan.
- [x] (2026-05-24T16:36:48Z) Removed duplicate `macro_compile` test target
      entries from the Tokio and GPUI harness manifests, so Cargo metadata can
      load.
- [x] (2026-05-24T16:36:48Z) Validated the draft-plan branch with
      `make check-fmt`, `make lint`, `make typecheck`, and `make test`.
- [x] (2026-05-24T16:36:48Z) Ran `coderabbit review --agent`; it reported
      five trivial prose findings in this ExecPlan.
- [x] (2026-05-24T16:36:48Z) Addressed CodeRabbit's prose findings, reran
      `make check-fmt`, `make lint`, `make typecheck`, and `make test`, and
      reran `coderabbit review --agent`.
- [x] (2026-05-24T16:36:48Z) Confirmed the follow-up CodeRabbit review
      completed with zero findings.
- [x] (2026-05-24T16:36:48Z) Committed the duplicate `macro_compile` manifest
      cleanup as `5de9886`.
- [x] (2026-05-24T16:36:48Z) Committed the draft ExecPlan for review.
- [x] (2026-05-24T16:36:48Z) Pushed the branch and opened draft pull request
      <https://github.com/leynos/rstest-bdd/pull/497> for plan review.
- [x] (2026-05-26T00:00:00+02:00) Received explicit maintainer approval to
      proceed with implementation from this ExecPlan.
- [x] (2026-05-26T00:00:00+02:00) Re-read the plan, current branch state, and
      audited documentation/example occurrences of harness and attribute
      policy guidance.
- [x] (2026-05-26T00:00:00+02:00) Updated first-party Tokio and GPUI examples
      to demonstrate harness-only defaults.
- [x] (2026-05-26T00:00:00+02:00) Updated the user guide, migration guide, and
      design document to lead with harness-only first-party configuration.
- [x] (2026-05-26T00:00:00+02:00) Ran focused example tests for
      `tokio-reminders` and `gpui-counter`; both passed.
- [x] (2026-05-26T00:00:00+02:00) Ran `make markdownlint`; it now passes.
- [x] (2026-05-26T00:00:00+02:00) Validated with `make nixie`,
      `make check-fmt`, `make lint`, `make typecheck`, and `make test`.
- [x] (2026-05-26T00:00:00+02:00) Ran `coderabbit review --agent` after the
      documentation/example milestone; it reported zero findings.
- [x] (2026-05-26T00:00:00+02:00) Marked roadmap item 9.7.4 done after
      validation and CodeRabbit review.
- [x] (2026-05-26T00:00:00+02:00) Re-ran final gates after the roadmap and
      ExecPlan status updates.
- [x] (2026-05-26T00:00:00+02:00) Ran final `coderabbit review --agent`; it
      reported zero findings.
- [x] (2026-05-26T00:00:00+02:00) Committed the completed implementation as
      `4539cd8`.
- [ ] Push the completed implementation.

## Surprises & discoveries

- Observation: roadmap items 9.7.1, 9.7.2, and 9.7.3 are already marked
  delivered while ADR-008 remains `Proposed`. Evidence: `docs/roadmap.md`
  records the items as delivered under maintainer authorization;
  `docs/adr-008-harness-led-attribute-policy-defaults.md` still says
  `Status: Proposed`. Impact: this plan keeps implementation contingent and
  avoids marking 9.7.4 done during plan drafting.
- Observation: `docs/users-guide.md` already recommends harness-led defaults
  in one section but still carries forward-looking 9.7.4 notes and several
  paired or override examples that can be mistaken for canonical first-party
  usage. Evidence: the guide contains a note that 9.7.4 will revise examples,
  plus repeated `GpuiHarness` examples paired with `DefaultAttributePolicy`.
  Impact: implementation should audit the whole guide rather than only editing
  the first matching snippet.
- Observation: the first-party example READMEs currently describe binding
  scenarios with both `harness = ...` and `attributes = ...`. Evidence:
  `examples/tokio-reminders/README.md` and `examples/gpui-counter/README.md`
  both teach paired arguments. Impact: first-party example prose is definitely
  in scope, and example test source may also need to drop redundant
  `attributes = ...` arguments so the prose and working examples match.
- Observation: Firecrawl found the current CodeRabbit CLI documentation
  describes `--agent` as structured JSON output for agent integrations and says
  reviews can take 7 to 30 or more minutes depending on scope. Evidence:
  Firecrawl search result for <https://docs.coderabbit.ai/cli>. Impact: the
  plan treats CodeRabbit review as a long-running milestone that must complete
  before moving on.
- Observation: the full `make markdownlint` gate currently fails before this
  task changes any implementation docs. Evidence:
  `/tmp/markdownlint-9-7-4-update-guides-design-docs-and-examples.out` reports
  93 errors in `docs/users-guide.md`; the focused lint command for this
  ExecPlan reports zero errors. Impact: this planning branch records the
  blocker honestly and does not attempt the unapproved user-guide cleanup as
  part of the pre-implementation plan.
- Observation: Cargo metadata initially failed because both first-party
  harness manifests declared duplicate `macro_compile` test targets. Evidence:
  `make check-fmt` failed first on `crates/rstest-bdd-harness-gpui/Cargo.toml`,
  then on `crates/rstest-bdd-harness-tokio/Cargo.toml`, with "found duplicate
  test name macro_compile". Impact: the plan branch includes a separate
  gate-unblocking manifest cleanup so `make check-fmt`, `make lint`,
  `make typecheck`, and `make test` can run.
- Observation: implementation approval arrived while ADR-008 still says
  `Status: Proposed`. Evidence: the maintainer asked Codex on 2026-05-26 to
  proceed with this ExecPlan;
  `docs/adr-008-harness-led-attribute-policy-defaults.md` still has proposed
  status. Impact: proceed under explicit maintainer authorization and record
  that the canonical documentation change is still tied to ADR-008's staged
  rollout.
- Observation: `docs/users-guide.md` contains duplicated GPUI override snippets
  inside the Tokio harness section and a forward-looking 9.7.4 note. Evidence:
  the audit found repeated `my_gpui_scenario_with_explicit_override` examples
  around the harness overview, Tokio section, and GPUI section. Impact: clean
  up the duplicated snippets as part of the user-guide update so the guide
  presents one normal first-party path and one explicit override pattern.
- Observation: `make fmt` ran Rust formatting and Markdown fixers, but the
  Markdown linting phase initially failed on repository-wide line-length
  reports after the fixers completed. Evidence:
  `/tmp/fmt-9-7-4-update-guides-design-docs-and-examples.out` contains the
  failed `markdownlint --fix` output; unrelated formatter churn was restored
  before continuing. Impact: keep only task-owned files in the worktree and use
  `make markdownlint` plus the Rust formatting check as the authoritative
  gates for the final branch state.

## Decision log

- Decision: Keep this branch as a pre-implementation planning branch and do
  not mark roadmap item 9.7.4 done. Rationale: the user explicitly requested an
  ExecPlan and stated that the plan must be approved before implementation.
  Date/Author: 2026-05-24T16:36:48Z / Codex.
- Decision: Treat example test-source edits as allowed only when needed to
  keep first-party examples truthful and compiling. Rationale: roadmap item
  9.7.4 says examples should no longer require both parameters by default. In
  this repository, the first-party examples include both prose and Rust test
  files, so the implementation may need a small source edit to make the
  examples lead with harness-only defaults. Date/Author: 2026-05-24T16:36:48Z /
  Codex.
- Decision: Do not plan property tests, Kani, or Verus for the default
  implementation path. Rationale: the requested work updates documentation and
  examples for already delivered behaviour. It introduces no new resolver
  invariant, state transition, unsafe code, or contractual business rule
  requiring proof. Date/Author: 2026-05-24T16:36:48Z / Codex.
- Decision: Use the exact pull request title task marker requested by the
  user, `(7.7.4)`, for the draft plan pull request even though the roadmap item
  is 9.7.4. Rationale: the pull request instruction is explicit and separate
  from the roadmap numbering in the plan body. Date/Author:
  2026-05-24T16:36:48Z / Codex.
- Decision: Fix duplicate first-party harness `macro_compile` test target
  declarations in this planning branch. Rationale: the duplicates prevented
  Cargo metadata from loading, which made the repository gates impossible to
  run. The fix is a minimal manifest cleanup and does not implement roadmap
  item 9.7.4. Date/Author: 2026-05-24T16:36:48Z / Codex.
- Decision: Do not fix `docs/users-guide.md` Markdown lint failures in this
  pre-implementation planning branch. Rationale: those failures are outside the
  new ExecPlan and overlap the documentation implementation area that must wait
  for plan approval. Date/Author: 2026-05-24T16:36:48Z / Codex.
- Decision: Treat the user's 2026-05-26 "proceed with implementation" request
  as the required approval gate for the plan and for contingent ADR-008
  documentation work. Rationale: the request explicitly names this ExecPlan and
  asks for implementation of the planned functionality. Date/Author:
  2026-05-26T00:00:00+02:00 / Codex.
- Decision: Remove redundant first-party `attributes = ...` arguments from the
  Tokio reminders and GPUI counter scenario tests. Rationale: the roadmap item
  requires first-party examples to no longer require both parameters by
  default, and focused example tests prove that the delivered harness-led
  defaults cover these examples. Date/Author: 2026-05-26T00:00:00+02:00 /
  Codex.

## Outcomes & retrospective

The implementation aligns the user guide, design document, migration guide, and
first-party example crates with harness-led defaults for recognized Tokio and
GPUI harnesses. The examples now demonstrate `harness = ...` alone as the
normal first-party path, while the guide and migration prose retain
`attributes = ...` as an override, attributes-only, and third-party caveat
surface. Focused Tokio and GPUI example tests passed, full deterministic gates
passed twice, and both implementation CodeRabbit reviews reported zero
findings. Roadmap item 9.7.4 is marked done.

## Context and orientation

The harness adapter architecture comes from ADR-005 and is documented in
`docs/adr-005-harness-adapter-crates-for-framework-specific-test-integration.md`.
 It keeps framework-specific runtime integration out of the core runtime and
macro crates. The shared `rstest-bdd-harness` crate defines `HarnessAdapter`,
`ScenarioRunRequest`, `ScenarioRunner`, `AttributePolicy`, and
`DefaultAttributePolicy`.

ADR-008 proposes the harness-led default rule for first-party integrations. The
rule keeps `HarnessAdapter` and `AttributePolicy` separate, but it makes
`harness = ...` the lead user decision for known first-party harnesses. If the
caller writes `harness = rstest_bdd_harness_tokio::TokioHarness` and omits
`attributes = ...`, the macro infers the Tokio first-party attribute policy.
The same model applies to `rstest_bdd_harness_gpui::GpuiHarness`. An explicit
`attributes = ...` argument always overrides the inferred default.

Roadmap items 9.7.1, 9.7.2, and 9.7.3 have already delivered resolver,
code-generation, unit, trybuild, and behavioural coverage for the rule. This
plan updates the documentation and first-party examples so the public guidance
matches that delivered behaviour once ADR-008 is accepted or explicitly
authorized for contingent documentation.

Important repository files:

- `docs/roadmap.md` contains item 9.7.4 and must be marked done only after
  implementation, validation, and review complete.
- `docs/adr-008-harness-led-attribute-policy-defaults.md` defines the
  proposed precedence and caveats.
- `docs/users-guide.md` is the main user-facing guide and currently contains
  mixed harness-led and explicit-pair examples.
- `docs/rstest-bdd-design.md` records the architecture, code-generation
  model, and first-party plugin targets.
- `docs/v0-6-0-migration-guide.md` is the upgrade guide for consumer-visible
  stage-9.7 behaviour.
- `examples/tokio-reminders/README.md` and
  `examples/gpui-counter/README.md` are first-party example prose.
- `examples/tokio-reminders/tests/reminders.rs` and
  `examples/gpui-counter/tests/counter.rs` are compiling first-party example
  tests that may need to drop redundant explicit first-party attribute policies.

Relevant background documents to signpost during implementation:

- `docs/rstest-bdd-language-server-design.md` for language-server scope
  boundaries.
- `docs/rust-testing-with-rstest-fixtures.md` for fixture guidance.
- `docs/rust-doctest-dry-guide.md` for snippet maintenance discipline.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for avoiding
  broad rewrites.
- `docs/gherkin-syntax.md` for Gherkin terminology.
- `docs/documentation-style-guide.md` for Markdown, ADR, and en-GB Oxford
  style.

## Plan of work

Stage A is the approval and readiness checkpoint. Confirm whether ADR-008 has
been accepted. If it remains `Proposed`, record maintainer authorization before
implementing documentation that presents harness-led defaults as canonical.
Re-read `docs/roadmap.md` item 9.7.4, ADR-008, the 9.7.1 to 9.7.3 execplans,
and the current guide/design/migration/example surfaces. Do not edit files in
this stage except for keeping this ExecPlan current.

Stage B audits all documentation and example occurrences. Search for
`harness =`, `attributes =`, `TokioHarness`, `TokioAttributePolicy`,
`GpuiHarness`, `GpuiAttributePolicy`, `runtime = "tokio-current-thread"`, and
`9.7.4` across `docs/`, `README.md`, `crates/*/README.md`, and `examples/`.
Classify each occurrence as one of four cases: canonical first-party
harness-only guidance, explicit override guidance, attributes-only guidance, or
third-party caveat. The audit is complete when there are no unclassified paired
first-party examples.

Stage C updates the user guide and migration guide. In `docs/users-guide.md`,
make harness-only first-party snippets the normal Tokio and GPUI examples, so
the public guidance leads with the same default path. Remove stale
forward-looking notes about 9.7.4, keep one explicit `attributes = ...`
override example, and preserve the third-party limitation near custom harness
guidance. In `docs/v0-6-0-migration-guide.md`, make the stage 9.7's impact
explicit: first-party `StdHarness`, `TokioHarness`, and `GpuiHarness` infer
matching defaults when named by recognized paths; explicit `attributes = ...`
remains the override; renamed, aliased, re-exported, or third-party paths still
need caveat-aware guidance.

Stage D updates the design document. In `docs/rstest-bdd-design.md`, align the
architecture summary and first-party plugin target sections with harness-led
defaults as the recommended first-party user path. Keep internal details about
path-based recognition, `TestAttrPolicy`, `resolve_attribute_policy`, and the
ADR-008 precedence order. If the implementation work requires a substantive new
design decision, update ADR-008 or create a new ADR before changing the design
document; otherwise, cite ADR-008 as the governing decision.

Stage E updates first-party examples. In `examples/tokio-reminders/README.md`
and `examples/gpui-counter/README.md`, replace prose that says examples bind
scenarios with both harness and attribute policy by default. If the Rust test
files still show redundant `attributes = ...` on their primary first-party
scenarios, remove those arguments so the example code demonstrates the
harness-only default. Keep or add prose explaining that explicit
`attributes = ...` remains available for overrides, not for the normal
first-party path.

Stage F performs validation and review. Run the focused example tests if any
example source changed, then run the full repository gates sequentially with
`tee` logs in `/tmp`. Run `coderabbit review --agent` after the documentation
milestone and again after any CodeRabbit-driven fixes. When validation and
review are clean, update this plan's `Progress`, `Surprises & discoveries`,
`Decision log`, and `Outcomes & retrospective`, then mark roadmap item 9.7.4
done in `docs/roadmap.md`.

Stage G commits, pushes, and opens the implementation pull request. Use
file-based commit messages. Push the branch to
`origin/9-7-4-update-guides-design-docs-and-examples`. The pre-implementation
plan pull request should be draft, mention this ExecPlan in the summary, use
the requested title marker `(7.7.4)`, and include the Lody session link in a
`## References` section.

## Concrete steps

Run commands from the repository root:

```bash
cd /home/leynos/.lody/repos/github---leynos---rstest-bdd/worktrees/cf6a7d98-d416-4269-a866-1b7f701d1f2d
```

During implementation, begin with status and audit commands:

```bash
git branch --show-current
git status --short
rg -n \
  "harness =|attributes =|TokioHarness|TokioAttributePolicy|GpuiHarness" \
  docs README.md crates examples -g '*.md' -g '*.rs'
rg -n \
  "GpuiAttributePolicy|runtime = \"tokio-current-thread\"|9\\.7\\.4" \
  docs README.md crates examples -g '*.md' -g '*.rs'
```

Expected transcript shape:

```plaintext
9-7-4-update-guides-design-docs-and-examples
```

Use these focused tests if example source changes:

```bash
SLUG=9-7-4-update-guides-design-docs-and-examples
set -o pipefail; cargo test -p tokio-reminders 2>&1 | tee /tmp/test-tokio-reminders-$SLUG.out
set -o pipefail; cargo test -p gpui-counter 2>&1 | tee /tmp/test-gpui-counter-$SLUG.out
```

Run repository gates sequentially before each CodeRabbit review and final
commit:

```bash
set -o pipefail; make markdownlint 2>&1 | tee /tmp/markdownlint-9-7-4-update-guides-design-docs-and-examples.out
set -o pipefail; make nixie 2>&1 | tee /tmp/nixie-9-7-4-update-guides-design-docs-and-examples.out
set -o pipefail; make check-fmt 2>&1 | tee /tmp/check-fmt-9-7-4-update-guides-design-docs-and-examples.out
set -o pipefail; make lint 2>&1 | tee /tmp/lint-9-7-4-update-guides-design-docs-and-examples.out
set -o pipefail; make typecheck 2>&1 | tee /tmp/typecheck-9-7-4-update-guides-design-docs-and-examples.out
set -o pipefail; make test 2>&1 | tee /tmp/test-9-7-4-update-guides-design-docs-and-examples.out
```

Run CodeRabbit after gates:

```bash
set -o pipefail; coderabbit review --agent 2>&1 | tee /tmp/coderabbit-9-7-4-update-guides-design-docs-and-examples.out
```

Use file-based commits:

```bash
git diff --check
git status --short
git add <changed files>
COMMIT_MSG_DIR=$(mktemp -d)
cat > "$COMMIT_MSG_DIR/COMMIT_MSG.md" << 'ENDOFMSG'
Update harness-led defaults guidance

Align the guide, design document, migration guide, and first-party examples
with the ADR-008 harness-led default configuration model.
ENDOFMSG
git commit -F "$COMMIT_MSG_DIR/COMMIT_MSG.md"
rm -rf "$COMMIT_MSG_DIR"
```

## Validation and acceptance

Acceptance for the approved implementation:

- `docs/users-guide.md` recommends first-party harness-only configuration for
  Tokio and GPUI as the normal path.
- `docs/rstest-bdd-design.md` records the same recommendation while preserving
  the internal path-based resolver and precedence details.
- `docs/v0-6-0-migration-guide.md` reflects the consumer-visible stage-9.7
  behaviour, including first-party inference and explicit override guidance.
- `examples/tokio-reminders` and `examples/gpui-counter` no longer teach that
  both `harness = ...` and first-party `attributes = ...` are required by
  default.
- `attributes = ...` remains documented as an override pattern and
  attributes-only escape hatch.
- Third-party harness caveats remain explicit.
- If example Rust source changes, `cargo test -p tokio-reminders` and
  `cargo test -p gpui-counter` pass.
- `make markdownlint`, `make nixie` when relevant, `make check-fmt`,
  `make lint`, `make typecheck`, and `make test` pass.
- `coderabbit review --agent` reports no unresolved concerns requiring code or
  documentation changes.
- `docs/roadmap.md` marks item 9.7.4 done only after the approved
  implementation, validation, and review are complete.

Quality method:

- Use `rg` for Markdown/prose audit of harness and attribute examples.
- Use focused example tests for changed example source.
- Use full Makefile gates for repository health.
- Use CodeRabbit CLI agent mode as a reviewer after deterministic gates pass.

## Idempotence and recovery

The audit and validation commands are safe to repeat. Documentation edits
should be small and reviewable; if a stage produces confusing churn, inspect
`git diff`, revert only the edits from the current stage, and re-apply a
smaller patch. Do not revert unrelated work in the tree.

If `make fmt` changes unrelated Markdown, stop before staging those files.
Record the formatter behaviour in `Surprises & discoveries`, restore unrelated
formatter churn, and use focused `markdownlint` plus the full repository gates
to validate the intended files.

If CodeRabbit reports findings, update the relevant documentation or this plan
and rerun deterministic gates before invoking CodeRabbit again. If a finding
contradicts ADR-008, record the conflict and ask for maintainer direction.

## Artifacts and notes

Wyvern reconnaissance identified these high-signal planning facts:

- ADR-008 is still `Proposed`, so 9.7.4 is contingent until acceptance or
  explicit maintainer authorization.
- 9.7.1, 9.7.2, and 9.7.3 are already delivered, so implementation should not
  change resolver behaviour unless documentation exposes a real mismatch.
- `docs/users-guide.md`, `docs/rstest-bdd-design.md`,
  `docs/v0-6-0-migration-guide.md`, `examples/tokio-reminders/README.md`, and
  `examples/gpui-counter/README.md` are the main documentation surfaces.
- Example Rust files may need a narrow edit because they still pair first-party
  harnesses with explicit first-party attribute policies.

Firecrawl resolved external tooling assumptions:

- CodeRabbit CLI documentation at <https://docs.coderabbit.ai/cli> describes
  `--agent` as structured output for agent integrations and recommends letting
  reviews run in the background when they take several minutes.
- CodeRabbit's Markdown tooling page identifies `markdownlint-cli2` as the
  Markdown linter, matching the repository `Makefile` target.

Current Lody session:

```plaintext
https://lody.ai/leynos/sessions/cf6a7d98-d416-4269-a866-1b7f701d1f2d
```

## Interfaces and dependencies

No new Rust API, crate dependency, feature flag, protocol, or data format is
planned. The implementation should document and demonstrate the existing
interfaces:

```rust,no_run
#[scenario(
    path = "tests/features/reminders.feature",
    name = "Scheduling a reminder queues it for later delivery",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn queues_a_scheduled_reminder(#[from(service)] _: ReminderService) {}
```

```rust,no_run
#[scenario(
    path = "tests/features/counter.feature",
    name = "Increment a counter and observe GPUI context",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
)]
fn increment_and_observe_gpui_context(#[from(app)] _: CounterApp) {}
```

Explicit override examples should remain available:

```rust,no_run
#[scenario(
    path = "tests/features/my_ui.feature",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
)]
fn my_gpui_scenario_with_explicit_override() {}
```

## Revision note

Initial draft created on 2026-05-24. It captures the plan approval gate,
ADR-008 contingency, Wyvern reconnaissance, Firecrawl tooling check, expected
documentation surfaces, validation commands, CodeRabbit review points, and the
pull request requirements for the pre-implementation plan branch.
