# ExecPlan 5.1.7: Normalize implicit fixture keys consistently

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Decision log`, and
`Outcomes & retrospective` must be kept up to date as work proceeds.

Status: DRAFT

## Objective

Implement one consistent rule for implicit fixture keys derived from Rust
parameter names across both `#[scenario]` fixture registration and step wrapper
extraction. Reuse `normalize_param_name()` so `_world` behaves like `world`,
`__world` continues to mean `_world`, and explicit `#[from(...)]` names remain
authoritative. Success means scenario-side registration and step-side lookup
agree, runtime missing-fixture diagnostics stop diverging, unit and behavioural
coverage prove the rule in both directions, ADR-009 records any final design
decisions, the users' guide documents the rule and escape hatch, and
`docs/roadmap.md` marks item 5.1.7 done after the feature lands.

## Context

The implementation surface is already narrowed:

- Scenario fixture registration derives keys in
  `crates/rstest-bdd-macros/src/utils/fixtures.rs` via
  `resolve_fixture_name()`, which now normalizes implicit parameter names.
- Step wrapper extraction classifies parameters in
  `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify.rs`, where
  placeholder matching already uses `normalize_param_name()` but fixture
  fallback still records the raw implicit name.
- Existing unit coverage lives in
  `crates/rstest-bdd-macros/src/utils/fixtures.rs`,
  `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/tests.rs`, and
  `crates/rstest-bdd-macros/src/utils/pattern/tests.rs`.
- Existing behavioural coverage for underscore-prefixed fixtures lives in
  `crates/rstest-bdd/tests/underscore_fixture.rs`.

ADR-009 defines the intended direction, but its current status is `Proposed`.
The roadmap entry says implementation depends on ADR-009 being accepted, so the
delivery work must either update that status first or stop for confirmation if
acceptance has not happened elsewhere.

## Relevant documentation and skills

Documentation signposts:

- `docs/roadmap.md` for item 5.1.7 and the completion checkbox.
- `docs/adr-009-consistent-implicit-fixture-name-normalization.md` for the
  governing design decision and any follow-up decisions taken during delivery.
- `docs/users-guide.md`, especially `### Fixtures and implicit injection`, for
  the user-facing rule and `#[from(...)]` escape hatch.
- `docs/rstest-bdd-design.md`, especially `### 3.8 Fixture integration
  implementation`, for the macro/runtime fixture flow.
- `docs/rstest-bdd-language-server-design.md` for future tooling consistency if
  fixture-name inference or diagnostics are indexed there.
- `docs/rust-testing-with-rstest-fixtures.md` for `rstest` fixture naming and
  `#[from(...)]` expectations.
- `docs/rust-doctest-dry-guide.md` for documentation example discipline.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` to keep the
  change focused and avoid scattering fixture-name policy.
- `docs/gherkin-syntax.md` as supporting context for behavioural examples.

Skill signposts:

- Project policy requests the `execplans` skill for planning work, but no
  `execplans` skill is installed in this session. Use
  `docs/execplans/policy-crate.md` and existing execplans in `docs/execplans/`
  as the operative template.
- Available system skills `skill-creator` and `skill-installer` are not needed
  for this implementation task.

## Constraints

- Reuse the existing `normalize_param_name()` helper; do not introduce a second
  normalization rule.
- Keep `#[from(...)]` authoritative and unchanged. Explicit fixture names must
  not be normalized.
- Preserve the established single-underscore contract:
  `_world -> world`, `world -> world`, `__world -> _world`.
- Keep runtime fixture lookup exact in `StepContext`; fix the mismatch in macro
  expansion rather than by adding runtime fuzzy matching.
- Add unit tests and behavioural tests that cover both scenario registration
  and step extraction paths, including explicit override precedence.
- Update ADR-009 if any design detail changes during implementation.
- Update `docs/users-guide.md` to document the rule and the `#[from(...)]`
  escape hatch.
- Mark roadmap item 5.1.7 done only after implementation and quality gates
  succeed.
- Run repository quality gates with `tee` and `set -o pipefail`. At minimum,
  the feature turn must finish with successful `make check-fmt`, `make lint`,
  and `make test`; documentation edits should also satisfy `make fmt`,
  `make markdownlint`, and `make nixie`.

## Tolerances

- Stop and escalate if implementing the rule requires changing `StepContext`'s
  public runtime API rather than only macro output.
- Stop and escalate if supporting this rule needs broader canonicalization than
  a single leading underscore.
- Stop and escalate if ADR-009 remains unaccepted and no explicit instruction
  is given to accept it as part of the feature.
- Stop and escalate if coverage requires new crates, dependencies, or a major
  test harness rewrite.

## Risks

- Risk: placeholder matching and implicit fixture fallback may drift apart
  again if they normalize names in different places.
  Mitigation: centralize use of `normalize_param_name()` in the existing
  classifier flow and add regression tests that assert both paths agree.

- Risk: explicit `#[from(...)]` names could accidentally be normalized.
  Mitigation: preserve current parsing flow and add unit and behavioural tests
  that prove `#[from(_world)]` stays `_world`.

- Risk: runtime diagnostics may still mention different keys if only one macro
  layer is updated.
  Mitigation: add behavioural coverage that exercises scenario-side fixture
  registration and step-side extraction together.

- Risk: documentation could overstate the rule and imply broader name
  canonicalization.
  Mitigation: document the exact one-underscore rule and show the explicit
  escape hatch.

## Progress

- [x] 2026-04-12: Gathered roadmap, ADR-009, users' guide, design docs, and
  current code/test touchpoints.
- [x] 2026-04-12: Confirmed the likely implementation split between
  `resolve_fixture_name()` and `classify_fixture_or_step()`.
- [ ] Draft and land the implementation.
- [ ] Update ADR-009, users' guide, and roadmap entry.
- [ ] Run formatting, lint, and test gates for the feature change.

## Plan of work

### Stage A: confirm the governing rule and prerequisite

- Confirm whether ADR-009 is accepted. If not, accept it as part of this work
  or pause for direction before code changes.
- Re-read ADR-009 against the current implementation to verify that only
  implicit names are normalized and explicit `#[from(...)]` names remain exact.
- Record any newly discovered edge-case decision in ADR-009 before or during
  implementation.

### Stage B: align step-side implicit fixture extraction with scenario-side registration

- Update the step argument classifier in
  `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify.rs` so that when
  an argument falls back to fixture injection without `#[from(...)]`, the
  stored fixture key uses `normalize_param_name()`.
- Keep placeholder matching behaviour unchanged, but make the fallback fixture
  path use the same normalized key derivation rule as
  `resolve_fixture_name()`.
- Preserve raw parameter identifiers for Rust bindings and diagnostics where
  needed; only the implicit fixture key should be normalized.

### Stage C: strengthen regression coverage

- Extend macro unit tests in
  `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/tests.rs` to
  cover implicit fixture fallback for `_world`, `__world`, and
  `#[from(_world)]`.
- Keep or extend unit coverage in
  `crates/rstest-bdd-macros/src/utils/fixtures.rs` to prove scenario fixture
  registration keeps the same normalization contract.
- Add or update behavioural coverage in
  `crates/rstest-bdd/tests/underscore_fixture.rs` so the scenario and step
  paths meet in one end-to-end case and missing-fixture diagnostics no longer
  diverge.
- Add a negative behavioural or UI-style regression if needed to lock down the
  explicit `#[from(...)]` escape hatch.

### Stage D: document and close the loop

- Update `docs/users-guide.md` in the implicit fixture section with the exact
  normalization rule and a short example showing when `#[from(...)]` is still
  required.
- Update `docs/adr-009-consistent-implicit-fixture-name-normalization.md` with
  any implementation-time decision, acceptance status, or resolved
  outstanding question.
- Mark roadmap item 5.1.7 as done in `docs/roadmap.md` once the feature and
  gates are complete.

### Stage E: validate and finish

- Run `make fmt`, `make markdownlint`, and `make nixie` after documentation
  edits.
- Run `make check-fmt`, `make lint`, and `make test` with `tee` logs and
  `set -o pipefail`.
- Review nearby code for small follow-on refactors only if they are directly
  enabled by the change and can remain atomic.

## Concrete steps

1. Confirm ADR-009 acceptance state and update the ADR if acceptance or an
   implementation clarification is required.
2. Modify step fixture fallback in
   `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify.rs` so implicit
   fixture keys use `normalize_param_name()` unless `#[from(...)]` supplied the
   key.
3. Keep `crates/rstest-bdd-macros/src/utils/fixtures.rs` aligned with the same
   rule and refactor lightly only if that reduces duplicated naming logic.
4. Add unit tests in
   `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/tests.rs` for:
   implicit `_world`, implicit `__world`, and explicit `#[from(_world)]`.
5. Extend behavioural coverage in `crates/rstest-bdd/tests/underscore_fixture.rs`
   or a nearby integration test so scenario-side registration and step-side
   extraction agree in one executable scenario.
6. Update `docs/users-guide.md` and ADR-009 with the final documented rule.
7. Mark roadmap item 5.1.7 done in `docs/roadmap.md`.
8. Run the full validation suite and inspect logs before considering the work
   complete.

## Validation

Implementation is complete only when all of the following are true:

- Implicit fixture keys derived from parameter names follow the same
  normalization rule in both the scenario and step macro paths.
- `#[from(...)]` remains authoritative and unnormalized.
- `_world` and `world` interoperate implicitly, while `__world` continues to
  mean `_world`.
- Unit tests prove the macro classifier and scenario fixture resolver agree on
  the rule.
- Behavioural tests prove the end-to-end runtime path uses matching keys and
  no longer produces mismatched missing-fixture diagnostics.
- `docs/users-guide.md` documents the rule and the `#[from(...)]` escape hatch.
- ADR-009 reflects any design decisions taken during implementation and is no
  longer left ambiguous relative to the roadmap prerequisite.
- `docs/roadmap.md` marks item 5.1.7 done.
- `make check-fmt`, `make lint`, and `make test` succeed. For documentation
  edits, `make fmt`, `make markdownlint`, and `make nixie` also succeed.

## Decision log

- 2026-04-12 / Codex: planned the feature around macro-expansion changes rather
  than runtime lookup changes because ADR-009 explicitly prefers exact runtime
  matching and reuse of `normalize_param_name()`.
- 2026-04-12 / Codex: treated ADR-009 acceptance state as a prerequisite check
  because the roadmap names acceptance as a dependency while the ADR currently
  still says `Proposed`.
- 2026-04-12 / Codex: recorded the lack of an installed `execplans` skill and
  used the repository's existing execplan documents as the fallback template.

## Outcomes & retrospective

To be completed after implementation. Capture the final code changes, test
coverage added, any documentation follow-up, and any refactors deliberately
deferred.
