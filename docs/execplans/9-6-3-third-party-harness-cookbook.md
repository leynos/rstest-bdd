# ExecPlan 9.6.3: add a third-party harness cookbook

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE - implemented and validated on 2026-05-08

`PLANS.md` is not present in this repository at the time of writing, so this
ExecPlan is the governing plan for roadmap item 9.6.3.

## Purpose / big picture

Roadmap item 9.6.3 closes the author-facing documentation gap for third-party
harness adapters. Architecture Decision Record 005 (ADR-005) introduced a small
harness adapter layer so Tokio, Graphical Processing User Interface (GPUI),
Bevy, and other framework-specific integrations can live in opt-in crates
rather than the core runtime or macros. ADR-007 then added
`HarnessAdapter::Context` so a harness can pass typed framework state, such as
a Bevy `World`, into generated scenario execution.

After this work, a reader can open `docs/users-guide.md`, find a cookbook
section for a third-party adapter crate such as `rstest-bdd-harness-bevy`, and
follow a complete example covering:

- the adapter crate's `Cargo.toml`;
- a `HarnessAdapter` implementation with an explicit `Context` type;
- the reserved `rstest_bdd_harness_context` fixture key used by step
  functions;
- an `AttributePolicy` implementation and the current path-based policy
  limitation for third-party crates; and
- a scenario using the custom harness through the public macro surface.

Success is observable when the user guide contains a coherent cookbook with a
working example, any new example claims are backed by focused tests or existing
test references, `docs/roadmap.md` marks 9.6.3 done only after the cookbook is
implemented, and these commands pass:

```bash
set -o pipefail; make check-fmt 2>&1 | tee /tmp/check-fmt-9-6-3-third-party-harness-cookbook.out
set -o pipefail; make lint 2>&1 | tee /tmp/lint-9-6-3-third-party-harness-cookbook.out
set -o pipefail; make test 2>&1 | tee /tmp/test-9-6-3-third-party-harness-cookbook.out
```

Implementation was approved on 2026-05-08. Continue milestone by milestone
within the tolerances in this plan.

## Constraints

- Implement roadmap item 9.6.3 only. Do not implement 9.7 harness-led default
  changes, Bevy runtime integration, or any public API redesign.
- Keep the roadmap item unchecked while this plan is only a draft. Mark
  `docs/roadmap.md` item 9.6.3 done only after the cookbook implementation,
  validation, and final plan updates have landed.
- Preserve ADR-005 crate boundaries: third-party harness guidance must keep
  framework dependencies in adapter crates, not in `rstest-bdd`,
  `rstest-bdd-macros`, or `rstest-bdd-harness`.
- Preserve ADR-007's typed context contract: cookbook examples must use
  `type Context = ...`, `ScenarioRunRequest<'_, Self::Context, T>`, and either
  `request.run(context)` or `request.run_without_context()`.
- Preserve the current attribute-policy trust model. First-party canonical
  policy paths can affect generated test attributes; unknown third-party policy
  paths trait-check but currently fall back to `#[rstest::rstest]` during macro
  expansion.
- Use en-GB Oxford spelling and sentence-case headings in documentation.
- Follow the project Markdown rules: paragraphs wrapped at 80 columns, code
  blocks tagged with a language, `make fmt` after documentation changes, and
  `make markdownlint` plus `make nixie` for Markdown validation.
- Validate observable behaviour with `rstest` unit tests and `rust-rspec`
  behavioural tests where applicable. If this task remains documentation-only,
  explicitly map each cookbook claim to existing tests and add focused tests
  only where the cookbook introduces a new unvalidated example or contract.
- Do not use property tests, Kani, or Verus for this task unless the
  implementation introduces a new invariant over a range of inputs, states, or
  transitions. A cookbook that documents existing contracts does not by itself
  require those tools.
- Use relevant skills during implementation: `execplans` for this living plan,
  `leta` for code navigation, `rust-router` with `arch-crate-design` for crate
  boundary checks, and `en-gb-oxendict-style` for documentation prose.

## Tolerances (exception triggers)

- Scope: if the cookbook implementation requires more than 8 files changed or
  more than 500 net lines, stop and confirm whether the task is expanding into
  a sample adapter crate or public API work.
- Interfaces: if making the cookbook example work requires changing
  `HarnessAdapter`, `ScenarioRunRequest`, `AttributePolicy`, `#[scenario]`, or
  `scenarios!`, stop and split that API change into a separate approved task.
- Dependencies: if a new real framework dependency such as Bevy is required in
  the workspace, stop and ask for approval. The cookbook should prefer
  illustrative snippets or lightweight local test fixtures unless a real crate
  is explicitly approved.
- Validation: if a documentation claim cannot be proven by existing tests or a
  modest new focused test, tighten the claim instead of documenting behaviour
  the suite does not enforce.
- Test topology: if a new end-to-end cookbook fixture requires complex Cargo
  workspace changes, stop and compare it against a smaller trybuild fixture or
  doctest before proceeding.
- Iterations: if the same gate fails three consecutive fix attempts, stop and
  record the failure, log path, and options in `Decision Log`.
- Ambiguity: if `docs/users-guide.md`, `docs/rstest-bdd-design.md`, ADR-005,
  ADR-007, and the implementation disagree on a custom harness contract, stop
  and list the conflict before editing user-facing guidance.

## Risks

- Risk: the cookbook could imply that arbitrary third-party
  `AttributePolicy::test_attributes()` implementations are evaluated during
  procedural macro expansion. Severity: high. Likelihood: medium. Mitigation:
  state the limitation directly and describe `attributes = ...` as a
  trait-checking and future-proofing hook unless the policy path is one of the
  recognized first-party paths.

- Risk: the example could be too Bevy-specific and accidentally promise a
  first-party Bevy adapter. Severity: medium. Likelihood: medium. Mitigation:
  frame `rstest-bdd-harness-bevy` as an illustrative third-party crate and keep
  the example focused on the adapter contract rather than Bevy internals.

- Risk: the cookbook could duplicate the existing API reference in
  `docs/users-guide.md` and drift later. Severity: medium. Likelihood: medium.
  Mitigation: keep the cookbook task-oriented and link to the harness adapter
  core API section for the deeper contract.

- Risk: code snippets in the user guide may look plausible but fail to compile
  when copied into an adapter crate. Severity: high. Likelihood: medium.
  Mitigation: add a compile-pass fixture or doctest when the final cookbook
  includes complete Rust snippets, and otherwise label non-compiled snippets as
  schematic.

- Risk: a documentation-only milestone could be marked done without proving
  behaviour. Severity: medium. Likelihood: medium. Mitigation: require a
  validation matrix that maps cookbook claims to existing or new tests before
  updating the roadmap checkbox.

## Progress

- [x] (2026-05-08) Loaded the `execplans`, `leta`, `rust-router`,
      `pr-creation`, `commit-message`, and `en-gb-oxendict-style` guidance.
- [x] (2026-05-08) Confirmed the working branch was not `main`.
- [x] (2026-05-08) Reviewed roadmap item 9.6.3 and neighbouring 9.6.1 and
      9.6.2 completion conventions.
- [x] (2026-05-08) Used a Wyvern agent team for read-only planning support:
      one pass on cookbook content, one pass on validation, and one pass on
      plan and roadmap conventions.
- [x] (2026-05-08) Drafted this pre-implementation ExecPlan.
- [x] (2026-05-08) Validated the draft plan with targeted Markdown linting
      and repository gates for the pre-implementation branch.
- [x] (2026-05-08) Received explicit approval to proceed with implementation.
- [x] (2026-05-08) Stage A: inventoried existing harness documentation and
      tests.
- [x] (2026-05-08) Stage B: drafted the user-guide cookbook in
      `docs/users-guide.md`.
- [x] (2026-05-08) Stage C: added
      `crates/rstest-bdd/tests/fixtures_macros/scenario_third_party_harness_cookbook.rs`
      and wired it into `crates/rstest-bdd/tests/trybuild_macros.rs`.
- [x] (2026-05-08) Stage C: validated the new fixture with
      `RUSTFLAGS="-D warnings" cargo test -p rstest-bdd --test
      trybuild_macros step_macros_compile -- --exact`.
- [x] (2026-05-08) Stage C: validated the existing harness unit contract with
      `RUSTFLAGS="-D warnings" cargo test -p rstest-bdd-harness`.
- [x] (2026-05-08) Stage C: validated macro-generated custom harness
      behaviour with `RUSTFLAGS="-D warnings" cargo test -p rstest-bdd --test
      scenario_harness`.
- [x] (2026-05-08) Stage D: confirmed no design-document update was required
      because the implementation documents existing ADR-005 and ADR-007
      contracts without changing them.
- [x] (2026-05-08) Stage E: ran focused validation plus repository gates.
- [x] (2026-05-08) Stage F: marked roadmap item 9.6.3 done and recorded
      outcomes.

## Surprises & Discoveries

- Observation: `docs/users-guide.md` already contains a short "Writing a
  custom harness adapter" section and a later "Harness adapter core APIs"
  section. Impact: the implementation should extend those sections into a
  cookbook instead of creating a disconnected new chapter.

- Observation: the current user guide and design document already state that
  unknown third-party policy paths fall back to `#[rstest::rstest]`. Impact:
  the cookbook must repeat that caveat near the custom `AttributePolicy`
  example so adapter authors do not misread the trait as runtime-discovered.

- Observation: existing tests already cover custom harness delegation,
  metadata visibility, context injection, missing `Default`, invalid harness
  types, and async harness rejection. Impact: the implementation may only need
  new focused validation if the cookbook introduces a full new external-crate
  example that existing tests do not cover.

- Observation: the prompt names `rust-doctest-dry-guide.md`, while the real
  repository path is `docs/rust-doctest-dry-guide.md`. Impact: use the real
  path in implementation references and plan signposts.

- Observation: the first `make test` run exposed an existing GPUI trybuild
  staging issue: the macro compile test copied fixtures into
  `target/tests/trybuild/rstest-bdd-harness-gpui/tests/features/auto` without
  first creating the `tests/features` parent. Impact: the planning branch
  includes a small test-infrastructure fix so the required gate can complete.

- Observation: Stage A found enough existing coverage for the harness
  execution contract but not a single compile-pass fixture that looks like a
  third-party adapter cookbook.
  `crates/rstest-bdd-harness/tests/harness_behaviour.rs` proves
  `ScenarioRunRequest` can carry non-unit context, and
  `crates/rstest-bdd/tests/scenario_harness.rs` proves macro-generated
  scenarios inject harness context into step functions. Impact: add one small
  trybuild fixture that combines a Bevy-like adapter type, a custom
  `AttributePolicy`, and `#[scenario(..., harness = ..., attributes = ...)]`
  without adding a Bevy dependency.

- Observation: the focused trybuild run passed with
  `scenario_third_party_harness_cookbook.rs` in the compile-pass fixture list.
  Impact: the cookbook's custom-harness, custom-policy macro shape now has a
  direct compile check, while existing behavioural tests continue to cover
  runtime context injection and unhappy paths.

- Observation: `make fmt` still fails because its `mdformat-all` step invokes
  `markdownlint --fix` in a way that reports many pre-existing MD013 line
  length findings across unrelated documents. Impact: unrelated formatter side
  effects were restored, the changed Markdown files were validated with
  `markdownlint-cli2`, and the repository's actual `make markdownlint` gate
  passed.

## Decision Log

- Decision: keep this branch as a pre-implementation plan branch and leave the
  roadmap checkbox unchecked. Rationale: the user explicitly stated that the
  plan must be approved before implementation. Date/Author: 2026-05-08 / Codex.

- Decision: locate the cookbook in `docs/users-guide.md` under the existing
  `Harness adapter and attribute policy` material, with cross-links to the later
  `Harness adapter core APIs` section. Rationale: that keeps user-facing
  usage, custom adapter steps, and API reference discoverable without
  duplicating all low-level contract details. Date/Author: 2026-05-08 / Codex.

- Decision: make validation evidence part of the implementation plan even
  though the task is documentation-centred. Rationale: the roadmap request
  requires unit and behavioural validation where applicable, and examples that
  claim to work should either compile or be explicitly tied to existing tests.
  Date/Author: 2026-05-08 / Codex.

- Decision: do not plan property testing, Kani, or Verus for the cookbook
  unless implementation introduces a new invariant. Rationale: documenting the
  existing harness adapter contract does not create a new state-space property
  or formal business axiom. Date/Author: 2026-05-08 / Codex.

- Decision: fix the GPUI trybuild staging parent-directory setup in this
  planning branch. Rationale: the requested pre-implementation branch must pass
  `make test`, and the failure was in existing test support rather than the
  cookbook plan. Date/Author: 2026-05-08 / Codex.

- Decision: validate the cookbook's "working example" shape with a local
  trybuild compile-pass fixture rather than a new workspace adapter crate.
  Rationale: the roadmap asks for third-party adapter documentation, not a
  published Bevy integration, and a fixture can prove the macro contract while
  preserving ADR-005's dependency boundary. Date/Author: 2026-05-08 / Codex.

## Outcomes & Retrospective

Implemented roadmap item 9.6.3 by replacing the short custom-harness passage in
`docs/users-guide.md` with `Third-party harness adapter cookbook`. The cookbook
documents a third-party adapter crate shape, `Cargo.toml` dependencies, a
Bevy-like `HarnessAdapter` using `type Context = World`, step access through
`#[from(rstest_bdd_harness_context)]`, an `AttributePolicy`, scenario macro
configuration, and the current third-party policy fallback to
`#[rstest::rstest]`.

Added
`crates/rstest-bdd/tests/fixtures_macros/scenario_third_party_harness_cookbook.rs`
and wired it into `crates/rstest-bdd/tests/trybuild_macros.rs` so the cookbook
shape has a compile-pass check without adding Bevy to the workspace. No design
document change was required because no new architecture decision was made.
`docs/roadmap.md` now marks 9.6.3 done.

Validation completed successfully:

```plaintext
markdownlint-cli2 docs/users-guide.md docs/execplans/9-6-3-third-party-harness-cookbook.md
RUSTFLAGS="-D warnings" cargo test -p rstest-bdd --test trybuild_macros step_macros_compile -- --exact
RUSTFLAGS="-D warnings" cargo test -p rstest-bdd-harness
RUSTFLAGS="-D warnings" cargo test -p rstest-bdd --test scenario_harness
make check-fmt
make lint
make test
make markdownlint
make nixie
```

`make fmt` was attempted, but it failed on unrelated pre-existing Markdown
line-length findings emitted by `markdownlint --fix`; the relevant changed
files and repository Markdown gate passed afterwards.

## Context and orientation

Primary roadmap target:

- `docs/roadmap.md` item 9.6.3 requires a third-party harness cookbook for a
  custom `HarnessAdapter`, including `Context`, attribute policy, and
  `Cargo.toml` configuration. The finish line is a user-guide cookbook section
  with a working example.

Primary user-facing documentation:

- `docs/users-guide.md` has the existing `Harness adapter and attribute policy`
  section and the smaller `Writing a custom harness adapter` passage. This is
  the main implementation target.
- `docs/users-guide.md` also has `Harness adapter core APIs`, which should
  remain the deeper API reference for `HarnessAdapter`, `ScenarioRunRequest`,
  and `AttributePolicy`.
- `docs/rstest-bdd-design.md` section 2.7 records ADR-005 and ADR-007
  architecture, the path-based attribute-policy trust model, first-party Tokio
  and GPUI adapters, and validation layers.

Primary ADRs:

- `docs/adr-005-harness-adapter-crates-for-framework-specific-test-integration.md`
  explains why framework-specific harnesses belong in opt-in crates and names
  Bevy as the future adapter pattern.
- `docs/adr-007-harness-context-injection.md` defines the associated
  `HarnessAdapter::Context` contract and the reserved
  `rstest_bdd_harness_context` fixture convention.
- `docs/adr-008-harness-led-attribute-policy-defaults.md` is relevant only as
  a boundary. Do not implement 9.7 default inference behaviour as part of this
  task.

Primary Rust API references:

- `crates/rstest-bdd-harness/src/adapter.rs` defines `HarnessAdapter`.
- `crates/rstest-bdd-harness/src/runner.rs` defines `ScenarioMetadata`,
  `ScenarioRunner`, and `ScenarioRunRequest`.
- `crates/rstest-bdd-harness/src/policy.rs` defines `AttributePolicy`,
  `DefaultAttributePolicy`, and `TestAttribute`.
- `crates/rstest-bdd-harness/src/std_harness.rs` shows the unit-context
  default harness.
- `crates/rstest-bdd-harness-tokio/src/tokio_harness.rs` and
  `crates/rstest-bdd-harness-gpui/src/gpui_harness.rs` show first-party adapter
  implementations.

Primary validation references:

- `docs/testing-strategy.md` distinguishes structural macro tests from
  semantic behaviour tests.
- `crates/rstest-bdd-harness/tests/harness_behaviour.rs` covers the core
  harness request and runner contract.
- `crates/rstest-bdd-harness/tests/attribute_policy_behaviour.rs` covers the
  default attribute policy.
- `crates/rstest-bdd/tests/scenario_harness.rs` covers custom harness
  delegation, context injection, metadata capture, and harness failure paths.
- `crates/rstest-bdd/tests/trybuild_macros.rs` plus fixtures under
  `crates/rstest-bdd/tests/fixtures_macros/` cover compile-time macro
  constraints such as invalid harness types, missing `Default`, and async
  harness rejection.

Relevant skills to use when implementing:

- `execplans`: keep this file current through implementation and close-out.
- `leta`: navigate Rust symbols and references before changing code or tests.
- `rust-router` and `arch-crate-design`: check Rust crate boundaries and public
  API assumptions if any cookbook-supporting test fixture or example crate is
  introduced.
- `en-gb-oxendict-style`: keep documentation prose in project style.
- `pr-creation` and `commit-message`: use file-based commit messages and draft
  pull request metadata when preparing review.

## Plan of work

### Stage A: documentation and validation inventory

Goal: establish the exact gap between the current short custom-harness prose
and the roadmap's cookbook finish line.

Implementation details:

- Re-read `docs/users-guide.md` from `Harness adapter and attribute policy`
  through `Harness adapter core APIs`.
- Re-read `docs/rstest-bdd-design.md` section 2.7 and the summary around
  section 3.12 to confirm the current architecture narrative.
- Re-read ADR-005 and ADR-007 to keep the cookbook aligned with the accepted
  crate-boundary and context-injection decisions.
- Inspect existing custom harness tests in
  `crates/rstest-bdd/tests/scenario_harness.rs` and trybuild fixtures under
  `crates/rstest-bdd/tests/fixtures_macros/`.
- Build a small validation matrix in this plan's `Surprises & Discoveries` or
  `Decision Log` that maps cookbook claims to existing tests or identifies new
  tests required.

Go/no-go validation:

- Proceed only when the implementation can name the cookbook sections to add,
  the examples to compile or label as schematic, and the tests that prove each
  behavioural claim.

### Stage B: draft the cookbook in the user guide

Goal: add a task-oriented cookbook section that a third-party adapter author
can follow without reading macro internals.

Implementation details:

- Add `#### Third-party harness adapter cookbook` under the existing harness
  adapter chapter in `docs/users-guide.md`.
- Cover the adapter crate layout and `Cargo.toml` dependencies, including
  `rstest-bdd-harness`, `rstest-bdd`, `rstest-bdd-macros`, and a placeholder
  framework dependency for an illustrative `rstest-bdd-harness-bevy` crate.
- Show a `HarnessAdapter` implementation with:
  - `#[derive(Default)]` or an explicit `Default` implementation;
  - `type Context = ...`;
  - `fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) ->
    HarnessResult<T>`; and
  - `request.run(context)` for non-unit context or
    `request.run_without_context()` for unit context.
- Show a step function that receives context using
  `#[from(rstest_bdd_harness_context)]`.
- Show a custom `AttributePolicy` with `TestAttribute::new(...)` or
  `TestAttribute::with_arguments(...)`.
- Add a scenario usage example with `harness = ...` and, where needed,
  `attributes = ...`.
- State clearly that unknown third-party policy paths currently fall back to
  `#[rstest::rstest]` during macro expansion, so framework-specific test
  attributes may still need explicit native attributes or first-party policy
  support until the policy resolver is extended.
- Link to the first-party `examples/tokio-reminders` and
  `examples/gpui-counter` examples as contrast, not as third-party templates.

Go/no-go validation:

- The cookbook must include `Context`, `AttributePolicy`, and `Cargo.toml`
  configuration before this stage is complete.
- Do not proceed if the prose implies Bevy support already ships in this
  repository.

### Stage C: validate examples and contracts

Goal: ensure the cookbook's working example is genuinely backed by tests or
compile checks.

Implementation details:

- If the cookbook Rust snippets can be expressed as crate doctests without
  pulling in a real external framework, prefer compiling them as doctests or as
  a small trybuild pass fixture.
- If the cookbook uses Bevy-like names without depending on Bevy, define a
  minimal local context type in the test fixture to prove the harness contract
  without adding Bevy to the workspace.
- Use `rstest` for focused unit validation where new helper code is introduced.
- Use existing `rust-rspec` behavioural coverage where applicable, especially
  `crates/rstest-bdd/tests/scenario_harness.rs`, to validate user-observable
  custom harness behaviour.
- Add a focused behavioural or compile-pass test only if the cookbook creates
  a new claim that the existing suite does not cover, such as a complete
  external-adapter crate layout.
- Do not add property, Kani, or Verus validation unless a new invariant is
  introduced.

Focused validation commands may include:

```bash
set -o pipefail
RUSTFLAGS="-D warnings" cargo test -p rstest-bdd-harness 2>&1 \
  | tee /tmp/harness-9-6-3-third-party-harness-cookbook.out

set -o pipefail
RUSTFLAGS="-D warnings" cargo test -p rstest-bdd \
  --test scenario_harness 2>&1 \
  | tee /tmp/scenario-harness-9-6-3-third-party-harness-cookbook.out

set -o pipefail
RUSTFLAGS="-D warnings" cargo test -p rstest-bdd \
  --test trybuild_macros step_macros_compile -- --exact 2>&1 \
  | tee /tmp/trybuild-9-6-3-third-party-harness-cookbook.out
```

Go/no-go validation:

- Proceed only when the implementation has either added focused validation for
  the cookbook-specific example or documented why existing tests fully cover
  the example's claims.

### Stage D: update design documentation if decisions changed

Goal: keep internal architecture documentation accurate without duplicating the
cookbook.

Implementation details:

- If implementation only adds user-facing cookbook prose, no design-doc change
  is required beyond a small reference if it improves discoverability.
- If implementation makes a new decision about third-party adapter validation,
  attribute-policy guidance, or example crate shape, record it in
  `docs/rstest-bdd-design.md` section 2.7 or the relevant appendix.
- Do not create a new ADR unless the implementation changes an architectural
  decision rather than documenting the existing one.

Go/no-go validation:

- Proceed only when user-facing guidance and design documentation do not
  contradict each other on policy resolution, context injection, or crate
  boundaries.

### Stage E: run formatting, documentation, and repository gates

Goal: prove the branch is ready before marking the roadmap item done.

Run commands sequentially, not in parallel:

```bash
set -o pipefail; make fmt 2>&1 | tee /tmp/fmt-9-6-3-third-party-harness-cookbook.out
set -o pipefail; make markdownlint 2>&1 | tee /tmp/markdownlint-9-6-3-third-party-harness-cookbook.out
set -o pipefail; make nixie 2>&1 | tee /tmp/nixie-9-6-3-third-party-harness-cookbook.out
set -o pipefail; make check-fmt 2>&1 | tee /tmp/check-fmt-9-6-3-third-party-harness-cookbook.out
set -o pipefail; make lint 2>&1 | tee /tmp/lint-9-6-3-third-party-harness-cookbook.out
set -o pipefail; make test 2>&1 | tee /tmp/test-9-6-3-third-party-harness-cookbook.out
```

Go/no-go validation:

- Do not update `docs/roadmap.md` until the focused validation from Stage C
  and all gates in this stage pass, or until an unrelated environmental failure
  has been documented and the user explicitly authorizes proceeding.

### Stage F: close out the roadmap item and plan

Goal: leave a reviewer and future implementer with a complete record of what
was delivered.

Implementation details:

- Mark `docs/roadmap.md` item 9.6.3 from `[ ]` to `[x]` only after validation
  succeeds.
- Update this plan's `Progress`, `Surprises & Discoveries`, `Decision Log`,
  and `Outcomes & Retrospective` with the final implementation and validation
  evidence.
- Commit the cookbook implementation separately from any follow-up refactor if
  the implementation reveals a small cleanup need.
- Create or update the draft pull request with title:
  `Document third-party harness adapters (9.6.3)`.
- Mention this execplan,
  `docs/execplans/9-6-3-third-party-harness-cookbook.md`, in the pull request
  summary.

Go/no-go validation:

- The task is complete only when the cookbook is in the user guide, the
  roadmap item is checked, this plan records the outcome, and the required
  gates have passed.

## Acceptance criteria

- `docs/users-guide.md` contains a third-party harness cookbook with a working
  custom adapter example.
- The cookbook covers `Cargo.toml`, `HarnessAdapter::Context`,
  `rstest_bdd_harness_context`, `AttributePolicy`, and scenario macro usage.
- The cookbook states the current limitation for unknown third-party attribute
  policy paths.
- Any new behaviour or example claim is covered by existing tests referenced
  in this plan or by new focused validation added during implementation.
- `docs/rstest-bdd-design.md` is updated only if implementation records a new
  internally facing decision or practice.
- `docs/roadmap.md` marks 9.6.3 done after implementation validation, not
  during plan drafting.
- `make check-fmt`, `make lint`, and `make test` pass.
