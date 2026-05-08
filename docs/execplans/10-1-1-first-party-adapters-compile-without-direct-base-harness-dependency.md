# First-party adapters compile without direct base harness dependency

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

Implementation must not begin until this plan is explicitly approved.

## Purpose / big picture

Roadmap item 10.1.1 removes avoidable harness adoption friction for v0.6.0 beta
feedback. After this work, a downstream project that uses a first-party adapter
should list only `rstest-bdd`, `rstest-bdd-macros`, and the selected adapter
crate in `Cargo.toml`. The downstream project should not also need to list
`rstest-bdd-harness` unless it implements a custom harness or directly uses
base harness API types such as `HarnessAdapter`, `ScenarioRunRequest`, or
`StdHarness`.

The visible success condition is a plain dependency matrix in
`docs/v0-6-0-migration-guide.md` covering plain BDD, Tokio, GPUI, and custom
harnesses, plus compile tests or example manifests proving that Tokio and GPUI
first-party adapter scenarios compile without a direct `rstest-bdd-harness`
dependency in the consuming crate.

This plan is planning-only until approved. It prepares the implementation path,
the validation strategy, and the documentation updates needed to mark roadmap
item 10.1.1 done.

## Constraints

- Do not change public trait contracts. `HarnessAdapter`, `AttributePolicy`,
  `ScenarioRunner`, `ScenarioRunRequest`, and existing macro arguments must
  remain source-compatible.
- Keep the scope to roadmap item 10.1.1. Do not implement 10.1.2 missing
  fixture diagnostics or 10.1.3 GPUI stateful regression coverage in this
  branch.
- Preserve ADR-005's crate boundary: Tokio and GPUI remain outside the default
  `rstest-bdd` dependency graph.
- Preserve ADR-007 harness context injection. Steps still request harness
  context with `#[from(rstest_bdd_harness_context)]`.
- Preserve ADR-008 precedence: explicit `attributes = ...` beats harness-led
  first-party defaults, which beat deprecated runtime compatibility aliases,
  which beat fallback attributes.
- Preserve ADR-009 implicit fixture-name normalization. Adapter dependency
  simplification must not alter fixture naming or `#[from(...)]` resolution.
- Do not add new external crates. If validation needs helper code, prefer
  existing test harnesses and existing workspace crates.
- Do not use property tests or model checking unless the implementation
  introduces a new invariant over a range of inputs, states, orderings, or
  transitions. The expected change is dependency and code-generation plumbing,
  so compile tests and behavioural tests are the primary validation.
- Keep every Rust source file at or below 400 lines. If a touched file would
  exceed that limit, split the implementation before continuing.
- Documentation must use en-GB Oxford spelling and wrap Markdown paragraphs at
  80 columns.
- Run gates sequentially, not in parallel. Use `tee` for long-running gates so
  the complete output remains available under `/tmp`.
- After approval and implementation, commit the functional change only after
  gates pass, then perform any necessary refactor as a separate gated commit.

## Tolerances (exception triggers)

- Scope: if implementation requires more than 18 files or more than 900 net
  lines, stop and ask for approval before widening the change.
- Public API: if any public signature in `rstest-bdd-harness`,
  `rstest-bdd-harness-tokio`, `rstest-bdd-harness-gpui`, `rstest-bdd-macros`,
  or `rstest-bdd` must change, stop and present options.
- Dependencies: if a new external dependency is needed, stop and explain why
  existing workspace tooling cannot cover the proof.
- Code generation: if custom harness users would lose support for direct
  `rstest-bdd-harness` API paths, stop and present a compatibility plan.
- Test iterations: if the same gate fails three times after attempted fixes,
  stop and summarize the remaining failure with the relevant `/tmp` log path.
- GPUI: if GPUI compile or behavioural tests fail for platform setup reasons
  rather than this change, stop and record the environment-specific blocker
  before weakening the test.
- Ambiguity: if "adapter-only dependency" can only be satisfied by choosing
  between re-exporting base harness API from adapters and changing generated
  macro paths, stop and ask for direction if both options remain viable after
  reconnaissance.

## Risks

- Risk: generated scenario code currently calls
  `crate::codegen::rstest_bdd_harness_path()` and emits paths such as
  `rstest_bdd_harness::ScenarioRunRequest` and
  `rstest_bdd_harness::HarnessAdapter`. Severity: high. Likelihood: high.
  Mitigation: first add failing adapter-only compile fixtures so the current
  friction is reproduced, then adjust the generated path strategy or adapter
  re-exports without changing trait contracts.

- Risk: adapter crates currently depend on `rstest-bdd-harness` internally but
  do not re-export the base harness types. Severity: medium. Likelihood: high.
  Mitigation: evaluate narrow re-exports from `rstest-bdd-harness-tokio` and
  `rstest-bdd-harness-gpui` only if generated code can target the selected
  first-party adapter crate path cleanly.

- Risk: dependency proof can be incomplete if trybuild fixtures still inherit
  the adapter crate's dev-dependencies, including `rstest-bdd-harness`.
  Severity: medium. Likelihood: medium. Mitigation: make the proof explicit:
  either use a staged standalone fixture manifest, or use examples whose
  manifests intentionally omit direct base harness dev-dependencies and are
  exercised by the workspace gates.

- Risk: GPUI tests are feature-gated with `native-gpui-tests` and may expose
  platform assumptions. Severity: medium. Likelihood: medium. Mitigation:
  retain existing feature gating, keep adapter-only compile proof minimal, and
  rely on `make test` plus `make lint` before commit.

- Risk: documentation may imply that custom harness users no longer need
  `rstest-bdd-harness`. Severity: medium. Likelihood: medium. Mitigation: make
  the migration matrix explicit that custom harness implementation and direct
  base API use still require `rstest-bdd-harness`.

## Skills and documentation signposts

Use these skills during implementation:

- `leta`: inspect symbols, references, and file structure before code changes.
- `rust-router`: route Rust-specific concerns before modifying Rust code.
- `arch-crate-design`: check crate-boundary, feature, and public API choices.
- `execplans`: keep this plan current as work proceeds.
- `commit-message`: write the gated commit message through a file, not `-m`.
- `pr-creation`: create the required draft PR after the plan branch is pushed.

Use these repository documents as source material:

- `docs/roadmap.md`, item 10.1.1, for the acceptance criteria and final
  roadmap status update.
- `docs/rstest-bdd-design.md`, especially sections 2.7.5 and 2.7.6.3, for
  first-party adapter roles and v0.6.0-beta2 quick-win scope.
- `docs/adr-005-harness-adapter-crates-for-framework-specific-test-integration.md`
  for adapter crate boundaries.
- `docs/adr-007-harness-context-injection.md` for context handoff rules.
- `docs/adr-008-harness-led-attribute-policy-defaults.md` for attribute
  default precedence.
- `docs/adr-009-consistent-implicit-fixture-name-normalization.md` for fixture
  name stability.
- `docs/v0-6-0-migration-guide.md` for the required dependency matrix.
- `docs/users-guide.md` for user-facing harness adapter behaviour.
- `docs/developers-guide.md` for internal crate-boundary guidance.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md`, `docs/gherkin-syntax.md`, and
  `docs/complexity-antipatterns-and-refactoring-strategies.md` for test,
  documentation, Gherkin, and refactoring conventions.

## Repository orientation

The workspace root is `Cargo.toml`. First-party harness crates live under:

- `crates/rstest-bdd-harness`
- `crates/rstest-bdd-harness-tokio`
- `crates/rstest-bdd-harness-gpui`

The procedural macro crate is `crates/rstest-bdd-macros`. The main generated
harness delegation path is currently in
`crates/rstest-bdd-macros/src/codegen/scenario/runtime/harness.rs`. It builds
metadata, a `ScenarioRunner`, a `ScenarioRunRequest`, and calls
`HarnessAdapter::run`.

Tokio adapter compile tests live in
`crates/rstest-bdd-harness-tokio/tests/macro_compile.rs` with fixtures in
`crates/rstest-bdd-harness-tokio/tests/fixtures_macros`. GPUI adapter compile
tests live in `crates/rstest-bdd-harness-gpui/tests/macro_compile.rs` with
fixtures in `crates/rstest-bdd-harness-gpui/tests/fixtures_macros`.

The example manifests currently act as useful downstream-style proof points:
`examples/tokio-reminders/Cargo.toml` and `examples/gpui-counter/Cargo.toml`.
Both should be checked during implementation because they currently list
`rstest-bdd-harness` directly as a dev-dependency even though they use a
first-party adapter.

## Implementation plan

### Milestone 1: establish the failing proof

Create the smallest adapter-only compile proof before changing code. The proof
must show that a consumer can select a first-party adapter without directly
declaring `rstest-bdd-harness`.

For Tokio, add or adapt a compile fixture that depends on `rstest-bdd`,
`rstest-bdd-macros`, and `rstest-bdd-harness-tokio`, plus Tokio runtime
dependencies, but does not list `rstest-bdd-harness`. The fixture should use a
canonical harness path such as:

```rust,no_run
#[scenario(
    path = "basic.feature",
    harness = rstest_bdd_harness_tokio::TokioHarness,
)]
fn scenario_uses_tokio_harness() {}
```

For GPUI, add the corresponding adapter-only proof for:

```rust,no_run
#[scenario(
    path = "basic.feature",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
)]
fn scenario_uses_gpui_harness() {}
```

If trybuild cannot express a manifest that excludes inherited dev-dependencies,
document that limitation in `Surprises & Discoveries` and use the example
manifests as the primary dependency proof instead.

Expected baseline: before implementation, the new proof should fail because
generated code still requires a visible `rstest_bdd_harness` crate path.

### Milestone 2: choose the narrow dependency mechanism

Use `leta` to inspect references to `rstest_bdd_harness_path`,
`HarnessAdapter`, `ScenarioRunRequest`, `ScenarioRunner`, and first-party
adapter paths. Decide whether the narrowest compatible mechanism is:

- to make first-party adapters re-export the base harness API needed by
  generated code and teach codegen to use the selected first-party adapter
  crate path for known first-party harnesses; or
- to alter generated code so downstream consumers only need the adapter path
  while custom harness users continue to use `rstest-bdd-harness` directly.

The decision must preserve renamed dependency support through
`proc_macro_crate`, custom harness compatibility, and ADR-008 default attribute
resolution. Record the final choice in `Decision Log` before coding past the
prototype.

### Milestone 3: implement the code-generation and adapter boundary change

Modify only the files needed by the chosen mechanism. Likely touch points are:

- `crates/rstest-bdd-macros/src/codegen/mod.rs`
- `crates/rstest-bdd-macros/src/codegen/scenario/runtime/harness.rs`
- focused macro codegen tests under
  `crates/rstest-bdd-macros/src/codegen/scenario/tests.rs` or
  `crates/rstest-bdd-macros/src/codegen/scenario/tests/`
- adapter crate roots if narrow re-exports are chosen:
  `crates/rstest-bdd-harness-tokio/src/lib.rs` and
  `crates/rstest-bdd-harness-gpui/src/lib.rs`

Keep custom harness output unchanged unless the selected mechanism requires a
small compatible abstraction. The generated code must still compile for
third-party or custom harnesses that depend on `rstest-bdd-harness` directly.

### Milestone 4: update examples and dependency documentation

Remove direct `rstest-bdd-harness` dev-dependencies from first-party adapter
example manifests where they are not otherwise using the base harness API:

- `examples/tokio-reminders/Cargo.toml`
- `examples/gpui-counter/Cargo.toml`

Update `docs/v0-6-0-migration-guide.md` with a dependency matrix that states:

- plain BDD needs `rstest-bdd`, `rstest-bdd-macros`, and `rstest`;
- Tokio first-party harness tests need `rstest-bdd-harness-tokio`, not a
  direct `rstest-bdd-harness` entry, unless base API types are used directly;
- GPUI first-party harness tests need `rstest-bdd-harness-gpui`, not a direct
  `rstest-bdd-harness` entry, unless base API types are used directly;
- custom harness authors and direct base harness API users need
  `rstest-bdd-harness`.

Audit `docs/users-guide.md` for wording that still implies all adapter users
must add `rstest-bdd-harness`. Update only behaviour-visible text. Update
`docs/rstest-bdd-design.md` section 2.7.6.3 or nearby implementation notes to
record the dependency-boundary decision if code changes alter how generated
paths are selected. Update `docs/developers-guide.md` only if internal
component ownership or crate-boundary guidance changes.

### Milestone 5: behavioural and compile validation

Run focused tests first, then full gates. Use `tee` for logs:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-rstest-bdd-10-1-1-first-party-adapters-compile-without-direct-base-harness-dependency.out
make lint 2>&1 | tee /tmp/lint-rstest-bdd-10-1-1-first-party-adapters-compile-without-direct-base-harness-dependency.out
make test 2>&1 | tee /tmp/test-rstest-bdd-10-1-1-first-party-adapters-compile-without-direct-base-harness-dependency.out
```

Because this branch changes Markdown docs, also run:

```bash
make markdownlint 2>&1 | tee /tmp/markdownlint-rstest-bdd-10-1-1-first-party-adapters-compile-without-direct-base-harness-dependency.out
make nixie 2>&1 | tee /tmp/nixie-rstest-bdd-10-1-1-first-party-adapters-compile-without-direct-base-harness-dependency.out
```

If Markdown formatting is required, run `make fmt` before re-running the format
and Markdown gates. Do not run format, lint, and tests in parallel.

Expected result: all required gates exit with status 0. The new adapter-only
proof should fail before the implementation and pass after it.

### Milestone 6: roadmap, commit, push, and PR

After gates pass, mark item 10.1.1 done in `docs/roadmap.md`. Re-run the
relevant documentation gate if the roadmap edit happens after the previous
Markdown validation.

Commit using the `commit-message` skill and a file passed to `git commit -F`.
Do not use `git commit -m`.

Push branch
`10-1-1-first-party-adapters-compile-without-direct-base-harness-dependency`
with upstream tracking against
`origin/10-1-1-first-party-adapters-compile-without-direct-base-harness-dependency`.

Create a draft PR using the `pr-creation` skill. The title must include
`(10.1.1)`, and the PR summary must mention this ExecPlan:
`docs/execplans/10-1-1-first-party-adapters-compile-without-direct-base-harness-dependency.md`.

## Validation checklist

Acceptance is met when all of the following are true:

- `docs/v0-6-0-migration-guide.md` contains the plain BDD, Tokio, GPUI, and
  custom harness dependency matrix.
- At least one automated compile proof or example-level manifest proof shows
  Tokio first-party adapter scenarios compile without a direct
  `rstest-bdd-harness` dependency in the consuming crate.
- At least one automated compile proof or example-level manifest proof shows
  GPUI first-party adapter scenarios compile without a direct
  `rstest-bdd-harness` dependency in the consuming crate.
- Custom harness examples or docs still state that implementing custom
  adapters requires `rstest-bdd-harness`.
- Existing harness-led attribute defaults continue to work for Tokio and GPUI.
- Existing custom harness tests still pass.
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` pass.
- `docs/roadmap.md` marks 10.1.1 done only after implementation validation.

## Progress

- [x] 2026-05-08: Read repository instructions, loaded `leta`, `execplans`,
  `rust-router`, `arch-crate-design`, `commit-message`, and `pr-creation`
  guidance.
- [x] 2026-05-08: Renamed the local branch from
  `chore/adapter-dependency-plan` to
  `10-1-1-first-party-adapters-compile-without-direct-base-harness-dependency`.
- [x] 2026-05-08: Used Wyvern reconnaissance agents to inspect roadmap/design
  acceptance criteria, documentation signposts, test surfaces, and risks.
- [x] 2026-05-08: Drafted this pre-implementation ExecPlan.
- [ ] Await explicit approval before implementation.
- [ ] Establish failing adapter-only dependency proof.
- [ ] Implement the approved dependency-boundary change.
- [ ] Update migration, user, design, and internal docs as required.
- [ ] Run focused validation and full gates.
- [ ] Mark roadmap item 10.1.1 done after validation.
- [ ] Commit, push with upstream tracking, and open the required draft PR.

## Surprises & Discoveries

- The macro codegen path in
  `crates/rstest-bdd-macros/src/codegen/scenario/runtime/harness.rs` currently
  emits direct `rstest_bdd_harness` paths for `HarnessAdapter`,
  `ScenarioMetadata`, `ScenarioRunner`, and `ScenarioRunRequest`.
- `examples/tokio-reminders/Cargo.toml` and
  `examples/gpui-counter/Cargo.toml` currently include direct
  `rstest-bdd-harness` dev-dependencies, so they are useful proof points for
  the requested friction reduction.
- Existing adapter compile tests import
  `rstest_bdd_harness::trybuild_staging`, so the current adapter crate test
  harness itself uses base harness helpers even when individual downstream
  fixture files are intended to prove adapter-only consumer dependencies.
  Implementation must distinguish test harness internals from consumer
  manifests.
- `docs/v0-6-0-migration-guide.md` explains first-party harness adoption but
  does not yet contain the explicit plain BDD, Tokio, GPUI, and custom harness
  dependency matrix required by roadmap item 10.1.1.

## Decision Log

- Decision: treat this as a dependency-boundary and validation task, not a
  trait redesign. Rationale: roadmap item 10.1.1 and design section 2.7.6.3
  explicitly call for small, non-breaking v0.6.0-beta2 quick wins without
  changing public contracts. Date/Author: 2026-05-08 / ExecPlan draft.

- Decision: require explicit approval before implementation. Rationale: the
  user requested a plan and stated that it must be approved before being
  implemented. Date/Author: 2026-05-08 / ExecPlan draft.

- Decision: make automated compile proof the preferred acceptance evidence,
  with example manifest proof as supporting evidence. Rationale: the roadmap
  allows "fixture-generation tests or docs", but compile tests catch the exact
  downstream failure mode and prevent regression. Date/Author: 2026-05-08 /
  ExecPlan draft.

## Outcomes & Retrospective

No implementation has started. The intended outcome, after approval, is a gated
implementation that lets first-party adapter users omit direct
`rstest-bdd-harness` dependencies while preserving custom harness support and
public trait contracts.
