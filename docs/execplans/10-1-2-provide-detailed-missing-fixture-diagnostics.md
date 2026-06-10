# ExecPlan 10.1.2: provide detailed missing-fixture diagnostics

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
and `Outcomes & retrospective` must be kept up to date as work proceeds.

Status: PENDING

## Purpose / big picture

Roadmap item 10.1.2 makes missing fixture failures actionable for developers
adopting harness-backed scenarios. Today, a step can fail because a fixture was
not inserted into `StepContext`, but the diagnostic does not yet show the full
requested fixture identity or explain the common harness-context omission.

After this work, a developer who forgets to insert or select a required fixture
can see the requested fixture name, the requested Rust type, the fixtures
already inserted into `StepContext`, and a harness suggestion when the reserved
`rstest_bdd_harness_context` fixture is absent.

Success is observable when a regression test reproduces the missing-fixture
failure and asserts that the diagnostic contains the requested fixture name,
requested type, inserted fixture list, and harness suggestion.

## Constraints

- Preserve public trait contracts. Do not change `HarnessAdapter`,
  `ScenarioRunRequest`, public macro argument syntax, or step function
  signatures for this task.
- Preserve the existing `StepContext` insertion and borrowing semantics. This
  task improves diagnostics; it does not redesign fixture borrowing.
- Keep runtime fixture lookup exact. Do not add fuzzy matching or implicit
  fallback from ordinary fixture names to `rstest_bdd_harness_context`.
- Keep errors typed and inspectable. Do not make downstream consumers parse
  `Display` strings to discover requested fixture names, types, availability,
  or suggestions.
- Maintain localized formatting through the existing Fluent message system in
  `crates/rstest-bdd/i18n/*/rstest-bdd.ftl`.
- Do not add a diagnostic dependency. Firecrawl prior-art research found useful
  patterns in `miette`, but this quick win should use existing project
  infrastructure.
- Add unit tests with `rstest` and behavioural tests through the existing
  scenario test style where applicable.
- Add end-to-end coverage only where the final implementation affects generated
  scenario execution or another externally observable workflow.
- Do not use Kani, Verus, or property tests unless implementation introduces a
  real invariant over many input states or transitions.
- Update `docs/rstest-bdd-design.md`, `docs/users-guide.md`, and relevant
  internal documentation when behaviour or conventions change.
- Mark `docs/roadmap.md` item 10.1.2 done only after implementation,
  documentation, validation, CodeRabbit review, and commits are complete.
- Run validation commands sequentially, not in parallel, and write logs under
  `/tmp` with `tee`.
- Commit after each approved implementation milestone that passes its gates.

## Tolerances

- Scope: stop and escalate if implementation needs more than 10 files changed
  or more than 700 net lines, excluding generated lockfile noise and locale
  message updates.
- Interface: stop and escalate if satisfying the requirement requires a
  breaking change to public macros, `StepContext`, `Step`, `ExecutionError`, or
  harness-adapter traits.
- Registry metadata: stop and escalate if requested fixture type metadata
  cannot be added compatibly for both generated and manual step registrations.
- Registry compatibility: stop and escalate if implementation would remove or
  change the public name-only `Step::fixtures` field.
- Dependencies: stop and escalate before adding any external crate.
- Localization: stop and escalate if Fluent message changes cannot be kept
  compatible across the repository's existing locale files.
- Validation: stop and escalate if the same quality gate fails three
  consecutive fix attempts.
- Ambiguity: stop and present options if "suggested harness to select" cannot
  be phrased accurately from available runtime metadata.

## Risks

- Risk: `ExecutionError::MissingFixtures` currently knows fixture names but not
  Rust types. Severity: high. Likelihood: high. Mitigation: add compatible
  name-and-type step metadata and make macro output populate it.

- Risk: manual uses of the public `step!` macro can only provide fixture names
  today. Severity: medium. Likelihood: high. Mitigation: keep existing `step!`
  forms working, and synthesize a name-only or unknown-type requirement for
  manual registrations.

- Risk: harness suggestions could overpromise a specific adapter when runtime
  metadata only proves that `rstest_bdd_harness_context` is absent. Severity:
  medium. Likelihood: medium. Mitigation: phrase the suggestion generically
  unless generated metadata can identify a concrete harness path.

- Risk: changing localized messages can break exact-string tests.
  Severity: medium. Likelihood: high. Mitigation: update exact display tests
  deliberately and add substring tests for required diagnostic facts.

- Risk: adding type metadata to `Step` can ripple through registry tests,
  macro-generated code, and manual registrations. Severity: medium. Likelihood:
  medium. Mitigation: introduce a small `FixtureRequirement` type and preserve
  the old name-only surface until call sites are bridged.

- Risk: documentation may imply this diagnostic fixes the current mutable
  borrow limitation. Severity: medium. Likelihood: low. Mitigation: document
  the diagnostic as observability only, and link the borrow redesign to later
  roadmap work.

## Progress

- [x] (2026-05-10T20:19:39Z) Loaded `execplans`, `leta`,
  `firecrawl-mcp`, `rust-router`, `rust-errors`, `rust-types-and-apis`, and
  `arch-crate-design` guidance for planning.
- [x] (2026-05-10T20:19:39Z) Confirmed the working branch is
  `${PR_BRANCH}`, not the main branch.
- [x] (2026-05-10T20:19:39Z) Reviewed `AGENTS.md`, `docs/roadmap.md` item
  10.1.2, `docs/rstest-bdd-design.md` section 2.7.6.3, and nearby harness
  context documentation.
- [x] (2026-05-10T20:19:39Z) Used a Wyvern agent team for read-only planning
  support on relevant code, tests, risks, and milestones.
- [x] (2026-05-10T20:19:39Z) Used Firecrawl to check prior art in Rust
  diagnostic reporting and Cucumber Rust's world/context model.
- [x] (2026-05-10T20:19:39Z) Drafted this pre-implementation ExecPlan.
- [ ] Receive explicit user approval to implement this ExecPlan.
- [ ] Confirm the branch is `${PR_BRANCH}`, tracking
  `origin/${PR_BRANCH}`, with a clean worktree.
- [ ] Add failing regression tests for the requested fixture name, requested
  type, inserted fixture list, and harness suggestion.
- [ ] Capture the expected red state before implementing the diagnostic
  fields.
- [ ] Implement typed fixture requirement metadata while preserving
  `Step::fixtures` and existing public `step!` forms.
- [ ] Verify focused green runs for `execution_error`, `step_registry`, and
  generated harness scenario coverage.
- [ ] Run `coderabbit review --agent` after the code milestone and clear every
  actionable finding.
- [ ] Update user-facing and internal documentation for the richer diagnostic
  and fixture-name convention.
- [ ] Run commit gates for each implementation milestone: `make check-fmt`,
  `make lint`, and `make test`.
- [ ] Run `coderabbit review --agent` after the documentation milestone and
  clear every actionable finding.
- [ ] Mark roadmap item 10.1.2 done in `docs/roadmap.md` only after the
  implementation PR is merged and validated.
- [ ] Run final documentation and Rust gates: `make markdownlint`,
  `make nixie`, `make check-fmt`, `make lint`, and `make test`.

## Surprises & discoveries

- Observation: `StepContext::available_fixtures()` already exists and current
  execution-time validation already includes a sorted `available` fixture list.
  Evidence: `crates/rstest-bdd/src/context/mod.rs` and
  `crates/rstest-bdd/src/execution/mod.rs`. Impact: refine the existing
  diagnostic path instead of adding a parallel availability mechanism.

- Observation: execution validation stores required and missing fixture names,
  but not requested Rust types. Evidence: `Step` in
  `crates/rstest-bdd/src/registry/mod.rs` has
  `fixtures: &'static [&'static str]`. Impact: satisfying the roadmap likely
  needs compatible registered fixture requirement metadata.

- Observation: generated wrapper code already knows the requested fixture type
  and builds `StepError::MissingFixture { name, ty, step }`. Evidence:
  `crates/rstest-bdd-macros/src/codegen/wrapper/arguments/fixtures.rs`. Impact:
  macro generation should reuse that source of truth when registering typed
  fixture requirements.

- Observation: adding a new field to public `Step` would make downstream
  literal `Step` construction source-incompatible, even if the existing
  `fixtures` field stayed in place. Evidence: `Step` is publicly re-exported
  from `crates/rstest-bdd/src/lib.rs`, and the public `step!` macro constructs
  it directly. Impact: the implementation uses an `inventory` sidecar
  `StepFixtureRequirements` keyed by keyword and pattern instead of extending
  `Step`.

- Observation: `make fmt` runs Markdown formatting after `cargo fmt` and then
  fails on repository-wide pre-existing Markdown line-length findings.
  Evidence: the run reported many MD013 failures outside files touched for this
  task. Impact: keep Rust formatting via `cargo fmt --all`; run task-scoped
  Markdown validation while preserving the final `make markdownlint` evidence
  separately.

- Observation: Firecrawl prior-art research found `miette`'s diagnostic model,
  where structured diagnostics can include help text while remaining ordinary
  Rust errors. Evidence: <https://docs.rs/miette/latest/miette/>. Impact: keep
  the suggestion as structured error data rendered by the existing localization
  system, without adding `miette`.

- Observation: Cucumber Rust centres shared scenario state in a per-scenario
  `World`. Evidence: <https://cucumber-rs.github.io/cucumber/main/> and
  <https://docs.rs/cucumber/latest/cucumber/>. Impact: user documentation can
  explain harness context as `rstest-bdd`'s typed scenario context path while
  preserving the fixture-based design.

## Decision log

- Decision: treat the initial document as a draft plan only and do not
  implement code before approval. Rationale: the user explicitly stated that
  the plan must be approved before implementation. Date/Author: 2026-05-10 /
  Codex.

- Decision: plan for typed fixture requirement metadata instead of parsing
  generated `StepError` strings. Rationale: `rust-errors` and
  `rust-types-and-apis` guidance favours inspectable typed data over string
  parsing. Date/Author: 2026-05-10 / Codex.

- Decision: keep the harness suggestion generic unless implementation finds
  reliable generated metadata for a specific harness path. Rationale: runtime
  execution can know that the reserved harness fixture is missing, but it may
  not know which adapter was intended. Date/Author: 2026-05-10 / Codex.

- Decision: avoid adding `miette`.
  Rationale: the project already has typed errors plus Fluent localization, and
  the task is a small non-breaking beta quick win. Date/Author: 2026-05-10 /
  Codex.

- Decision: render requested fixture types using the same effective type string
  that generated wrappers already use for `StepError::MissingFixture`.
  Rationale: this keeps execution-time validation and wrapper-time borrow
  errors aligned. In practice, `world: &World` and `world: &mut World` report
  `World`, while owned `world: World` also reports `World`. Date/Author:
  2026-05-10 / Codex.

- Decision: keep `Step::fixtures` as the public name-only compatibility field
  and add typed fixture metadata alongside it. Rationale: `Step` is publicly
  re-exported, and the public `step!` macro constructs `Step` with
  `fixtures: &'static [&'static str]`. Replacing that field with
  `&'static [FixtureRequirement]` would be a breaking public API change and
  would violate this plan's interface tolerance. Date/Author: 2026-05-11 /
  Codex.

- Decision: store generated typed fixture requirements in an inventory sidecar
  rather than in `Step`. Rationale: this preserves external code that
  constructs or inspects `Step::fixtures` while still giving execution-time
  validation structured fixture names and Rust type strings for macro-generated
  steps. Date/Author: 2026-05-11 / Codex.

- Decision: manual `step!` registrations without sidecar metadata report the
  requested type as `<unknown>`. Rationale: manual registrations currently
  provide fixture names only; making callers provide types would break existing
  public macro forms. Date/Author: 2026-05-11 / Codex.

- Decision: replace stale branch-specific validation examples with
  `PR_BRANCH=10-1-2-provide-detailed-missing-fixture-diagnostics`. Rationale:
  log paths and commands must be reproducible on the actual implementation
  branch. Date/Author: 2026-05-11 / Codex.

## Outcomes & retrospective

Pending. Fill this section only after the implementation PR is merged and the
roadmap entry has been updated.

## Context and orientation

The core runtime crate is `crates/rstest-bdd`. A `StepContext` stores named
fixtures and prior step-return values. Step execution uses a `Step` registered
in the global inventory registry, validates required fixture names against
`StepContext::available_fixtures()`, and then calls the step wrapper.

Important files:

- `crates/rstest-bdd/src/context/mod.rs` defines `StepContext`,
  `StepContext::available_fixtures()`, and the reserved harness fixture key.
- `crates/rstest-bdd/src/registry/mod.rs` defines `Step` and the public
  `step!` macro. Today, `Step` stores fixture names in
  `fixtures: &'static [&'static str]`.
- `crates/rstest-bdd/src/execution/mod.rs` contains
  `validate_required_fixtures`, `execute_step`, and `execute_step_async`.
- `crates/rstest-bdd/src/execution/error.rs` defines
  `ExecutionError::MissingFixtures` and `MissingFixturesDetails`.
- `crates/rstest-bdd-macros/src/codegen/wrapper/arguments/fixtures.rs`
  generates wrapper code that already knows fixture names and Rust types.
- `crates/rstest-bdd-macros/src/codegen/scenario/runtime/harness.rs` handles
  harness context insertion for generated scenarios.
- `crates/rstest-bdd/tests/step_registry/execute_step_tests.rs`,
  `crates/rstest-bdd/tests/execution_error.rs`, and
  `crates/rstest-bdd/tests/step_execution_error.rs` are likely test anchors.
- `crates/rstest-bdd/i18n/*/rstest-bdd.ftl` contains localized display text.

Definitions used in this plan:

- A fixture is a named value inserted into `StepContext`.
- The requested fixture name is the key a step wrapper will use to borrow or
  get a fixture from `StepContext`.
- The requested fixture type is the Rust type expected by that step parameter,
  for example `u32`, `&World`, or `&mut gpui::TestAppContext`.
- The harness context fixture is the reserved fixture named
  `rstest_bdd_harness_context`, used to pass `HarnessAdapter::Context` into
  step functions.

## Relevant documentation and skills

Documentation signposts:

- `docs/roadmap.md` for item 10.1.2 and the final completion checkbox.
- `docs/rstest-bdd-design.md` section 2.7.6.3 for the beta2 quick-win scope,
  and section 2.7.6.1 for the borrow limitation that this task must not solve.
- Architecture Decision Record (ADR)-007, recorded in
  `docs/adr-007-harness-context-injection.md`, for the reserved harness context
  fixture convention.
- `docs/users-guide.md` sections covering fixture injection, manual mutable
  sharing, harness adapters, single-leading-underscore normalization, and
  `#[from(...)]` as the exact-name escape hatch.
- `docs/rust-testing-with-rstest-fixtures.md` for `rstest` fixture naming,
  single-leading-underscore normalization, and the `#[from(...)]` workaround.
- `docs/rust-doctest-dry-guide.md` for documentation example discipline,
  including examples that do not obscure the underscore-normalization rule.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` to keep any
  metadata changes small.
- `docs/gherkin-syntax.md` for behavioural feature examples.
- `docs/rstest-bdd-language-server-design.md` if final terminology affects
  diagnostics shared with language-server documentation.

Skill signposts:

- Use `execplans` to keep this document current during implementation.
- Use `leta` for symbol navigation and reference checks.
- Use `rust-router` to choose additional Rust-specific skills as needed.
- Use `rust-errors` for the diagnostic payload and error boundary.
- Use `rust-types-and-apis` for typed fixture requirement metadata.
- Use `arch-crate-design` if the change affects crate or macro/runtime
  boundaries.
- Use `firecrawl-mcp` only for fresh external gaps; current prior-art checks
  are recorded above.

## Plan of work

### Stage A: baseline and failing tests

Re-read the exact code paths before editing: `validate_required_fixtures`,
`Step`, `step!`, generated wrapper fixture registration, and harness context
insertion. Confirm whether current manual `step!` callers can remain name-only.

Add or update tests first. The failing tests should prove the roadmap finish
line before implementation changes:

- a unit-style `rstest` case for `ExecutionError::MissingFixtures` formatting;
- an execution test around `execute_step` or `execute_step_async` with one
  unrelated inserted fixture and a missing harness context fixture;
- a behavioural generated-scenario test if the final diagnostic is observable
  through scenario panic output.

This stage ends when the new tests fail because the diagnostic lacks the
requested type and harness suggestion.

### Stage B: model typed fixture requirements

Introduce a small runtime type such as `FixtureRequirement` in
`crates/rstest-bdd/src/registry/mod.rs` or a nearby module:

```rust
pub struct FixtureRequirement {
    pub name: &'static str,
    pub ty: &'static str,
}
```

Prefer borrowed `&'static str` fields because macro output can produce
`stringify!(...)` literals and existing step metadata is static.

Extend `Step` so execution can access typed requirements. The preferred shape
is to add `fixture_requirements: &'static [FixtureRequirement]` while keeping
the existing public `fixtures: &'static [&'static str]` field unchanged. Do not
replace `fixtures` with typed metadata in this roadmap item.

Update `step!` so existing manual registrations still compile. If manual
registrations only pass names, synthesize requirements whose type renders as
`"<unknown>"` or add a name-only helper path. Macro-generated step wrappers
should use the typed path.

This stage ends when the registry and manual tests compile with no behaviour
change except the new metadata being available.

### Stage C: emit typed metadata from macros

Update `crates/rstest-bdd-macros` wrapper code generation, so each fixture
parameter contributes a `FixtureRequirement` with the same fixture name and
effective requested type used by borrow generation.

Use the existing wrapper missing-fixture convention for type strings:

- `world: World` should report `World`;
- `world: &World` should report `World`;
- `world: &mut World` should report `World`;
- `#[from(rstest_bdd_harness_context)] app: &AppContext` must report the
  reserved fixture name and the requested context type.

This stage ends when generated step registrations expose typed fixture
requirements and existing macro tests pass.

### Stage D: render richer diagnostics

Update `MissingFixturesDetails` to include requested fixture data and an
optional suggestion string or structured suggestion enum. Preserve existing
fields where possible to reduce downstream breakage.

Update `validate_required_fixtures` to:

- collect available fixtures from `ctx.available_fixtures()`;
- compute missing requirements by fixture name;
- sort available fixtures deterministically;
- include each missing requested fixture's name and type;
- include a harness suggestion when the reserved harness context fixture is
  required but absent.

Update Fluent messages, so the rendered diagnostic includes the new facts. The
English text should remain concise. Example shape:

```plaintext
Missing fixtures: rstest_bdd_harness_context: AppContext.
Available fixtures from scenario: world.
Hint: this step requests harness context; select a harness-backed scenario,
for example #[scenario(..., harness = ...)].
```

This stage ends when unit tests prove the structured details and display text.

### Stage E: behavioural coverage and documentation

Add behavioural coverage that exercises the generated scenario path if Stage A
showed the diagnostic is externally observable through scenario panic output.
Use an existing feature fixture or a small new feature file under
`crates/rstest-bdd/tests/features/`.

Update documentation:

- `docs/rstest-bdd-design.md` section 2.7.6.3 with the final diagnostic
  behaviour and metadata source;
- `docs/users-guide.md` near fixture and harness sections with a short example;
- `docs/users-guide.md` and `docs/rust-testing-with-rstest-fixtures.md` with
  the rule that implicit fixture injection normalizes one leading underscore;
- `docs/users-guide.md` and `docs/rust-testing-with-rstest-fixtures.md` with
  `#[from(...)]` documented as the escape hatch for exact fixture names;
- `docs/rust-doctest-dry-guide.md` if examples need to show that distinction
  without duplicating brittle snippets;
- `docs/developers-guide.md` if a new internal convention is introduced;
- an architecture decision record only if the final design makes a substantive
  public API or compatibility decision.

This stage ends when documentation is accurate and the roadmap remains
unchecked pending final validation.

Apply formatting before running validation:

```bash
make fmt
```

### Stage F: review, gates, commit, and close

Run CodeRabbit after each major implementation milestone. Address every concern
or record why it is not applicable, and do not proceed while an actionable
concern remains unresolved.

Run the final gates sequentially:

```bash
PR_BRANCH=10-1-2-provide-detailed-missing-fixture-diagnostics
set -o pipefail; make check-fmt 2>&1 | tee "/tmp/check-fmt-rstest-bdd-${PR_BRANCH}.out"
set -o pipefail; make lint 2>&1 | tee "/tmp/lint-rstest-bdd-${PR_BRANCH}.out"
set -o pipefail; make test 2>&1 | tee "/tmp/test-rstest-bdd-${PR_BRANCH}.out"
```

For documentation changes, also run:

```bash
PR_BRANCH=10-1-2-provide-detailed-missing-fixture-diagnostics
set -o pipefail; make markdownlint 2>&1 | tee "/tmp/markdownlint-rstest-bdd-${PR_BRANCH}.out"
set -o pipefail; make nixie 2>&1 | tee "/tmp/nixie-rstest-bdd-${PR_BRANCH}.out"
```

After all gates pass, update `docs/roadmap.md` to mark item 10.1.2 done, update
this ExecPlan's status and retrospective, run relevant documentation validation
again, and commit the close-out as its own atomic commit.

## Concrete steps

- Step 1: Explicit approval has been received; continue implementing this
  ExecPlan.
- Step 2: Confirm the worktree is clean, and the branch is still
  `${PR_BRANCH}`.
- Step 3: Inspect the current symbols with `leta`:

  ```bash
  leta grep \
    "Step|Fixture|MissingFixtures|available_fixtures|ExecutionError|StepError" \
    crates/rstest-bdd -k struct,enum,function,method
  ```

- Step 4: Add failing tests for the final diagnostic facts in
  `crates/rstest-bdd/tests/execution_error.rs` and
  `crates/rstest-bdd/tests/step_registry/execute_step_tests.rs`.
- Step 5: Run the focused failing tests and record the expected red output in
  this plan:

  ```bash
  PR_BRANCH=10-1-2-provide-detailed-missing-fixture-diagnostics
  set -o pipefail
  RUSTFLAGS="-D warnings" cargo test -p rstest-bdd \
    --test execution_error missing_fixtures -- --nocapture 2>&1 \
    | tee "/tmp/test-red-execution-error-rstest-bdd-${PR_BRANCH}.out"
  RUSTFLAGS="-D warnings" cargo test -p rstest-bdd \
    --test step_registry execute_step_returns_missing_fixtures_error \
    -- --nocapture 2>&1 \
    | tee "/tmp/test-red-step-registry-rstest-bdd-${PR_BRANCH}.out"
  ```

- Step 6: Implement typed fixture metadata in the registry and update manual
  macro compatibility.
- Step 7: Emit typed fixture metadata from generated wrapper registrations.
- Step 8: Update `validate_required_fixtures`, `MissingFixturesDetails`,
  display formatting, and localization messages.
- Step 9: Add generated-scenario behavioural coverage if not already covered by
  the execution tests.
- Step 10: Update design and user documentation, plus `docs/developers-guide.md`
  if the new metadata is an internal convention maintainers need to know.
- Step 11: Run CodeRabbit and clear concerns:

  ```bash
  coderabbit review --agent
  ```

- Step 12: Run final gates sequentially with `tee` logs.
- Step 13: Mark roadmap item 10.1.2 done only after the feature is implemented
  and validated.
- Step 14: Commit each gated milestone with a file-based commit message
  following the `commit-message` skill.

## Validation and acceptance

The implementation is acceptable when all the following are true:

- A regression test fails before implementation and passes after
  implementation.
- The missing-fixture diagnostic contains the requested fixture name.
- The diagnostic contains the requested Rust type for macro-generated fixture
  parameters.
- The diagnostic contains the sorted list from
  `StepContext::available_fixtures()`, including at least one inserted fixture
  in the regression.
- When `rstest_bdd_harness_context` is required and absent, the diagnostic
  includes a suggestion to select a harness-backed scenario.
- The no-harness-suggestion path remains clean for ordinary missing fixtures.
- `execute_step` and `execute_step_async` share the same validation behaviour.
- Existing manual `step!` registrations still compile.
- `docs/users-guide.md` documents the user-visible behaviour.
- `docs/rstest-bdd-design.md` records the implementation decision.
- `docs/roadmap.md` item 10.1.2 is marked done only after validation.
- `coderabbit review --agent` reports no unresolved actionable concerns.
- These final commands pass:

  ```bash
  PR_BRANCH=10-1-2-provide-detailed-missing-fixture-diagnostics
  set -o pipefail; make check-fmt 2>&1 | tee "/tmp/check-fmt-rstest-bdd-${PR_BRANCH}.out"
  set -o pipefail; make lint 2>&1 | tee "/tmp/lint-rstest-bdd-${PR_BRANCH}.out"
  set -o pipefail; make test 2>&1 | tee "/tmp/test-rstest-bdd-${PR_BRANCH}.out"
  ```

## Idempotence and recovery

Most implementation steps are normal source edits and are safe to repeat. If a
focused test is added and the implementation direction changes, update or
delete only the test added for this task; do not rewrite unrelated fixtures.

If typed fixture metadata causes broad registry churn, stop at the tolerance
threshold and record alternatives in `Decision log`. The main fallback option
is to add a compatibility layer that derives name-only requirements for manual
`step!` calls and typed requirements only for generated macro output.

If localization changes create widespread exact-string failures, update only
tests whose asserted user-visible message changed because of this feature.
Avoid weakening tests that protect unrelated messages.

If a full gate fails for an unrelated pre-existing reason, preserve the log
path in `Artifacts and notes`, run the smallest focused validation that proves
this task, and ask for direction before committing.

## Artifacts and notes

Planning evidence:

```plaintext
Working branch: ${PR_BRANCH}
Wyvern sidecar: identified StepContext, Step, validate_required_fixtures,
MissingFixturesDetails, macro wrapper fixture generation, and harness context
insertion as the main implementation surfaces.
Firecrawl prior art: miette keeps help text as structured diagnostic metadata;
Cucumber Rust models scenario state through a per-scenario World.
```

Expected red-test shape before implementation:

```plaintext
error[E0432]: unresolved import `rstest_bdd::execution::MissingFixtureDiagnostic`
error[E0560]: struct `MissingFixturesDetails` has no field named `missing_requirements`
error[E0560]: struct `MissingFixturesDetails` has no field named `suggestion`
```

Record focused red and green evidence here as implementation proceeds. Keep log
paths parameterized with `${PR_BRANCH}` so they are reproducible across
worktrees.

Expected final diagnostic facts:

```plaintext
requested fixture: rstest_bdd_harness_context
requested type: AppContext
available fixtures: world
suggestion: select a harness-backed scenario
```

## Interfaces and dependencies

The likely new runtime interface is a small metadata type, placed where the
registry can expose it without introducing a new crate:

```rust
pub struct FixtureRequirement {
    pub name: &'static str,
    pub ty: &'static str,
}
```

`Step` keeps its compatibility field unchanged:

```rust
pub fixtures: &'static [&'static str],
```

Generated wrappers submit typed fixture requirements through the hidden
`StepFixtureRequirements` inventory sidecar. Execution looks up that sidecar by
step keyword and pattern; manual registrations without sidecar metadata fall
back to `<unknown>` for the requested type.

`MissingFixturesDetails` should remain inspectable and may gain fields such as:

```rust
pub struct MissingFixtureDiagnostic {
    pub name: &'static str,
    pub ty: &'static str,
}

pub struct MissingFixturesDetails {
    pub missing: Vec<&'static str>,
    pub missing_requirements: Vec<MissingFixtureDiagnostic>,
    pub available: Vec<String>,
    pub suggestion: Option<String>,
    // existing fields omitted
}
```

Do not add new dependencies. Use the existing Fluent localization, `rstest`,
scenario tests, and repository Makefile gates.

## Revision note

Initial draft created on 2026-05-10. It records local repository findings,
Wyvern read-only reconnaissance, Firecrawl prior-art checks, proposed
milestones, validation commands, tolerances, and the approval gate before any
implementation.
