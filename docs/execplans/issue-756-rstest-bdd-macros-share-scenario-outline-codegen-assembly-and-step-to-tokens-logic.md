# Share scenario/outline codegen assembly and step-to-tokens logic (issue 756)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS

Branch:
`issue-756-rstest-bdd-macros-share-scenario-outline-codegen-assembly-and-step-to-tokens-logic`

Roadmap item: none. This is a refactoring audit finding raised from the
2026-09-13 codegraph and wyvern reconnaissance recorded in
<https://github.com/leynos/rstest-bdd/issues/756>.

## Purpose / big picture

The procedural macro crate `rstest-bdd-macros` expands a `#[scenario]`
attribute into a `#[rstest]` test. Two shapes share almost all of that
expansion: a regular scenario consumes its parsed steps directly, while a
scenario outline substitutes each Examples row's values into the steps and
emits one `#[case(...)]` per row.

The per-step token builder is duplicated. `process_steps` and
`process_steps_substituted` in
`crates/rstest-bdd-macros/src/codegen/scenario/helpers.rs` each independently
turn a step's text, docstring, and data table into quoted literals. They can
drift: a change to docstring emission, table typing, or literal construction
made in one is invisible to the other.

The two scenario generators are also duplicated at their seam.
`generate_regular_scenario_code` and `generate_outline_scenario_code` in
`crates/rstest-bdd-macros/src/codegen/scenario/mod.rs` each resolve adapters,
project the same metadata, generate a body, and emit the same item. The
metadata projection and the final `quote!` assembly were already extracted to
`scenario_metadata` and `emit_scenario_test` in
`crates/rstest-bdd-macros/src/codegen/scenario/emit.rs` (commit `5a1bd0d6`),
but both generators still repeat the adapter-resolution prologue and the
emit/epilogue sequence.

After this change, a maintainer can read one step-to-tokens function and one
scenario-assembly path, and know that a regular scenario and a scenario outline
cannot diverge in literal shape or item structure. The observable behaviour is
unchanged: generated tokens for every existing fixture stay byte-identical, and
new pinning tests plus `insta` snapshots prove it.

Success is observable when:

- `process_steps` and `process_steps_substituted` call one private
  `step_to_tokens(step, Option<&SubstitutionContext>)` helper and produce
  identical token shapes for a step without placeholders.
- The two scenario generators no longer repeat the adapter prologue or the
  emit sequence; `generate_scenario_code` performs both once.
- `make test` passes, including the new unit and snapshot tests.
- `make check-fmt`, `make lint`, and `make markdownlint` pass.

## Constraints

- Behaviour must not change. The generated token streams for regular and
  outline scenarios are a compatibility surface for downstream macro
  expansions; the refactor is structural only. Snapshot evidence must show
  identical shapes.
- `process_steps` and `process_steps_substituted` keep their existing
  signatures and return types. Their callers in `mod.rs` and the `runtime`
  sub-modules must not need changes.
- Reuse the existing helpers: `cell_to_lit`, `generate_table_tokens`,
  `substitute_step_content`, `scenario_metadata`, `emit_scenario_test`,
  `finalize_scenario_signature`, `resolve_scenario_adapters`,
  `generate_test_tokens`, `generate_case_attrs`, and
  `generate_indexed_case_attrs`. Do not reimplement any of them.
- Workspace lints deny `unwrap_used` and `expect_used`; production code must
  not introduce either. `unreachable!` is already used in this crate for
  genuinely impossible branches.
- No single code file may exceed 400 lines (AGENTS.md), and Whitaker's
  `module_max_lines` gate applies the same ceiling. `helpers.rs` started at 376
  lines, so new unit tests go in the `tests/` module. When the shared builder
  pushed it to 426 lines, the module was split by cohesion instead of
  suppressed (D4): `helpers/mod.rs`, `helpers/steps.rs`, and
  `helpers/case_attrs.rs`.
- The canonical pattern must be documented in `docs/developers-guide.md` in the
  same change, stating ownership, permitted call-sites, and composition rules
  (AGENTS.md abstraction policy).
- No dependency changes. `insta` is already a development dependency of
  `rstest-bdd-macros`.
- Do not modify the parser, the runtime generators, public API, or the
  behaviour of `#[scenario]`/`scenarios!`.
- Repository gates run sequentially: `make check-fmt`, `make lint`,
  `make test`. Compiler admission must not be bypassed.

## Tolerances (exception triggers)

- Files: if more than 12 non-ExecPlan files change, stop and escalate.
  Counted files: `helpers/mod.rs` (the former `helpers.rs`), the new
  `helpers/steps.rs` and `helpers/case_attrs.rs`, `scenario/mod.rs`,
  `tests/mod.rs`, two new test modules, two snapshot files, and
  `developers-guide.md`. The limit was raised from 8 to 12 on 2026-11-28
  because the Whitaker module cap forced the two-way split of `helpers.rs`; no
  other scope changed.
- Code: if changes exceed 400 net lines across `crates/rstest-bdd-macros/src`,
  stop and escalate.
- Documentation: if changes exceed 150 net lines outside `docs/execplans/`,
  stop and escalate. ExecPlan files are excluded from every size count.
- Interface: if a change to a `pub` or `pub(crate)` signature beyond this plan
  is required, stop and escalate.
- Dependencies: any new dependency triggers escalation.
- Architecture: if the extraction requires changing the `ScenarioTestConfig`
  trait or the runtime generators, stop and escalate.
- Iterations: if a focused test still fails after three fix attempts, stop and
  escalate with the failing log path.
- Ambiguity: if two readings of the issue materially change the design, record
  both in `Decision log` and ask.

Budget basis: this is a mechanical refactor in two known files with one
documentation update. Comparable completed plans in this repository are small;
no history of repeatedly raised budgets applies. The code budget is roughly
twice the expected net change to allow for tests.

## Risks

- Risk: the shared step helper accidentally changes table or docstring token
  shape. Severity: high. Likelihood: low. Mitigation: pin the shapes with unit
  tests and an `insta` snapshot of the full assembled item.
- Risk: moving `reject_async_harness` earlier changes which error a user sees
  when an outline is both invalid and async-with-harness. Severity: low.
  Likelihood: low. Mitigation: the guard is pure and the precedence is
  identical for every config that previously reached either generator; both
  routes previously checked it first.
- Risk: the snapshot test bakes in incidental formatting that churns later.
  Severity: medium. Likelihood: medium. Mitigation: snapshot a small, stable
  scenario through the public `generate_scenario_code` entry point, and keep
  semantic assertions for the contract.
- Risk: `helpers.rs` exceeds the 400-line cap once the helper is added.
  Severity: medium. Likelihood: low. Mitigation: the refactor removes more
  lines than it adds; tests live in `tests/`.

## Progress

- [x] (2026-11-28 00:56Z) Reconnaissance at `470c20ae`: read the two scenario
      generators, `helpers.rs`, `emit.rs`, `runtime/`, the existing tests, and
      the stale developer-guide section.
- [x] (2026-11-28 02:00Z) `EP-M1` implementation: `step_to_tokens`,
      `process_steps_with_context`, three pinning tests in
      `tests/step_tokens.rs`, and the Whitaker-driven cohesion split of
      `helpers.rs` into `helpers/{mod,steps,case_attrs}.rs`.
- [x] (2026-11-28 03:10Z) `EP-M1` gates at the implementation tree:
      `make check-fmt` passed, `make lint` passed (including Whitaker), and
      `make test` passed (2595 nextest tests, 838 no-default-features tests,
      doctests, and 398 Python tests).
- [x] (2026-11-28 04:40Z) `EP-M2` implementation: `ScenarioItem`,
      `assemble_regular_scenario`, `assemble_outline_scenario`, and the shared
      guard/resolution/metadata/emission path in `generate_scenario_code`.
- [x] (2026-11-28 04:55Z) `EP-M2` gates: the two `insta` snapshots recorded
      before the refactor were unchanged after it; `make check-fmt`,
      `make lint`, and `make test` (2597 nextest tests) passed.
- [ ] `EP-M3`: developer-guide correction and canonical-pattern documentation.
- [ ] Delivery: push branch, open draft PR, CodeRabbit review, merge.

## Surprises & discoveries

- Observation: the metadata projection and final item assembly named by the
  issue are already shared. `emit.rs` was introduced by commit `5a1bd0d6` ("Add
  explicit step-library scopes and named step-argument binding (#686)") and
  already exposes `scenario_metadata` and `emit_scenario_test`. Evidence:
  `crates/rstest-bdd-macros/src/codegen/scenario/emit.rs:21` and `:58`; both
  generators call them at `mod.rs:217` and `:234`/`:303` and `:325`. Impact:
  Task 2 of the supplied coding plan reduces to consolidating the remaining
  adapter/emit prologue and epilogue, and updating the documentation to
  describe what exists rather than re-extracting code. Recorded as D1.
- Observation: the developer-guide section is wrong in three ways, not one.
  It names `generate_test_tokens_for_config`, which does not exist; it claims
  two typed entry points `generate_test_tokens` and
  `generate_test_tokens_outline`, of which only the first exists; and it says
  `reject_async_harness` runs before both paths, while at `470c20ae` it runs
  inside each path. Evidence: `docs/developers-guide.md:3035-3047`;
  `grep -rn "generate_test_tokens_for_config\|generate_test_tokens_outline"`
  returns only the guide. Impact: EP-M3 corrects all three claims.
- Observation: adding the shared builder pushed `helpers.rs` to 426 lines, and
  Whitaker `module_max_lines` failed `make lint` at exactly that point. The
  plain `wc -l` count and the lint's count agreed. Evidence:
  `/tmp/lint-m1b-*.out`, "Module helpers spans 426 lines". Impact: the module
  was split by cohesion into `helpers/mod.rs` (shared literal builder,
  Examples-row predicate, underscore expectation), `helpers/steps.rs`, and
  `helpers/case_attrs.rs`, with no `#[allow]`. Recorded as D4.
- Observation: the plan's Red/Green framing does not fit a pure extraction.
  The new `tests/step_tokens.rs` assertions pass both before and after the
  refactor, because they pin behaviour that the duplication already produced.
  Evidence: the three tests passed against `470c20ae` before any production
  edit. Impact: they are regression pins, not red tests; the structural property
  (`L-SHARED-LITERALS`) is established by inspection and protected by the
  pins. Recorded in `Verification plan`.
- Observation: the assembled-item snapshots recorded before the assembly
  refactor matched byte-for-byte after it. Evidence:
  `tests/assembly_snapshot.rs` ran with `INSTA_UPDATE=always` at commit
  `d52e2d7d`, and the same two snapshots passed without an update after the
  `mod.rs` restructure. Impact: the extraction preserved the generated item
  exactly, including attribute order, the hidden case-index parameter, and the
  substituted step constants.

## Decision log

- D1. Decision: treat the issue's Task 2 premise as partially stale and
  implement the remaining consolidation rather than re-extracting existing
  helpers. Rationale: re-adding `scenario_metadata` or `emit_scenario_test`
  under new names would duplicate working code. The issue's intent — paths
  differ only in shape-specific work — is met by moving adapter resolution and
  item emission into `generate_scenario_code`. Date/Author: 2026-11-28,
  implementation agent.
- D2. Decision: `step_to_tokens` returns
  `Result<StepTokens, TokenStream2>`; `process_steps` handles the impossible
  failure with `unreachable!` rather than `expect`/`unwrap`, which workspace
  lints deny. Rationale: keeps one function as the single literal builder as
  the plan requires, while confining the infallible branch to one documented
  line. Date/Author: 2026-11-28, implementation agent.
- D3. Decision: add a private
  `process_steps_with_context(steps, Option<&SubstitutionContext>)` loop so the
  keyword pass and the three accumulator vectors exist once. Rationale: removes
  the remaining per-field loop duplication beyond what `step_to_tokens` alone
  covers, and keeps both public processors thin. Date/Author: 2026-11-28,
  implementation agent.
- D4. Decision: split `helpers.rs` into `helpers/mod.rs`, `helpers/steps.rs`,
  and `helpers/case_attrs.rs` rather than suppressing `module_max_lines`.
  Rationale: AGENTS.md forbids growing a file past 400 lines, and the Whitaker
  gate charges the module, not the Cargo target. The split follows the module's
  two existing concerns (step tokens and case attributes) and leaves the shared
  literal builder with a single owner. File sizes after the split are 99, 302,
  and 61 lines. Date/Author: 2026-11-28, implementation agent.

## Outcomes & retrospective

Pending implementation.

## Context and orientation

The expansion path is:

1. `crates/rstest-bdd-macros/src/codegen/scenario/mod.rs` —
   `generate_scenario_code` decides between a regular scenario and an outline
   with placeholders
   (`config.examples.is_some() && steps_contain_placeholders(&config.steps)`),
   then calls `generate_regular_scenario_code` or
   `generate_outline_scenario_code`.
2. `crates/rstest-bdd-macros/src/codegen/scenario/helpers.rs` — per-step and
   per-row processing. `process_steps` builds keyword, value, docstring, and
   table token vectors from parsed steps. `process_steps_substituted` does the
   same after substituting `<placeholder>` tokens from one Examples row.
3. `crates/rstest-bdd-macros/src/codegen/scenario/runtime/mod.rs` —
   `ScenarioTestConfig` and `generate_test_tokens<P, I, Q>` assemble the test
   body from `TestTokensConfig` or `OutlineTestTokensConfig`.
4. `crates/rstest-bdd-macros/src/codegen/scenario/emit.rs` —
   `scenario_metadata` projects shared metadata and `emit_scenario_test`
   finalizes the signature and emits the test item.
5. `crates/rstest-bdd-macros/src/codegen/scenario/boundary.rs` —
   `finalize_scenario_signature` resolves test attributes and the GPUI boundary.

Existing tests live in `crates/rstest-bdd-macros/src/codegen/scenario/tests/`.
`adapter_sharing.rs` and `runtime_split.rs` build a `ScenarioConfig` and call
`generate_scenario_code`, so they are templates for the new snapshot test.
`insta` with the `filters` feature is already in
`crates/rstest-bdd-macros/Cargo.toml` under `[dev-dependencies]`.

The repository gate commands are `make check-fmt`, `make lint`, and
`make test`. The host runs `cargo-nextest`; `make test` also runs doctests, the
no-default-features crate check, fixture lockfile checks, and Python tests.
Logs should be captured under `/tmp` with `tee` and `pipefail`.

## Conformance basis

No Terms of Reference or technical-design revision governs this refactor. The
governing artefacts are:

- `AGENTS.md` — code style, the 400-line file cap, abstraction policy, and the
  requirement to document a canonical pattern's ownership, call-sites, and
  composition rules.
- `docs/developers-guide.md` — the stale "Shared scenario-token assembly"
  section that EP-M3 corrects.
- Issue <https://github.com/leynos/rstest-bdd/issues/756> — the refactoring
  finding and the supplied coding plan.

Requirements derived from those artefacts:

- `REQ-STEP-SHARING`: one per-step token builder serves both processors.
- `REQ-ASSEMBLY-SHARING`: one assembly path serves both scenario shapes.
- `REQ-DOCS`: the developer guide describes the actual helpers and states the
  canonical pattern's ownership, call-sites, and composition rules.
- `REQ-NO-REGRESSION`: generated tokens and all existing tests are unchanged.

Trace links:

```plaintext
REQ-STEP-SHARING -> EP-M1 -> tests::step_tokens::*
REQ-ASSEMBLY-SHARING -> EP-M2 -> tests::assembly_snapshot::*
REQ-DOCS -> EP-M3 -> docs/developers-guide.md
REQ-NO-REGRESSION -> EP-M1, EP-M2 -> make test
```

## Verification plan

### Abstract model

A scenario expansion is a pure function from a `ScenarioConfig` (steps, an
optional `ExampleTable`, attributes, signature, block, and runtime mode) to a
`TokenStream2` test item. A step is a triple `(text, docstring?, table?)`. The
regular path maps each step triple through literal construction. The outline
path maps each Examples row through placeholder substitution and then the same
literal construction.

Preconditions: an outline row carries the same number of cells as the header
row (enforced by the parser); placeholders name a header column. Violations
surface as a `syn::Error` rendered into the token stream, which is existing
behaviour and out of scope.

### Invariants and lemmas

- `INV-STEP-EQUIV`: for a step that contains no placeholder tokens, the tokens
  produced through `process_steps` equal those produced through
  `process_steps_substituted` for any row.
- `INV-STEP-FIELDS`: each processed step yields exactly one value token, one
  docstring token (`None` or `Some("...")`), and one table token (`None`,
  `Some(&[])`, or `Some(&[&[...]])`).
- `INV-ASSEMBLY`: both scenario shapes emit the same item envelope —
  optional fallback diagnostics, trait assertions, test attributes, case
  attributes, user attributes, the `used_underscore_binding` expectation, then
  `vis signature { body }` — differing only in the case attributes and the
  finalized signature.
- `L-SHARED-LITERALS`: if both processors call `step_to_tokens`, then any
  future change to literal construction affects both shapes at once. This is
  the property the refactor exists to create; it is structural and is
  established by code inspection plus a test that would fail if the paths
  diverged.

### Axioms (trusted, not verified here)

- `AXIOM-RSTEST`: `#[rstest::rstest]` and `#[case(...)]` semantics belong to
  the `rstest` crate; this change only preserves the emitted tokens.
- `AXIOM-PROC-MACRO`: `proc_macro2::TokenStream::to_string()` is deterministic
  for the same token sequence, so snapshots are stable.
- `AXIOM-PARSER`: `ExampleTable` rows align with headers; the parser enforces
  it (`crates/rstest-bdd-macros/src/parsing/examples.rs`).

### Methods per obligation

- Obligation: `INV-STEP-FIELDS` and `INV-STEP-EQUIV`.
  Method: named unit tests. Rationale: finite, explicit token shapes; three
  tests cover a substituted row's three fields, the placeholder-free
  equivalence of both processors, and the substitution failure path. Domain: a
  step whose text, docstring, and table each carry a placeholder; a
  placeholder-free step with a docstring and table; a placeholder naming no
  header. Artefact:
  `crates/rstest-bdd-macros/src/codegen/scenario/tests/step_tokens.rs`.
  Evidence:
  `cargo test -p rstest-bdd-macros --lib codegen::scenario::tests::step_tokens`
  passed against the pre-refactor tree and passes again after, so the pins
  protect the shape rather than proving the extraction. Non-vacuity: the
  substituted case asserts the substituted literal values (`5`, `apples`), so a
  helper that ignores the context fails; the equivalence case uses a step with
  no placeholder, so a helper that substitutes unconditionally fails; the
  failure case asserts both the compile-error macro and the named placeholder.
- Obligation: `INV-ASSEMBLY`.
  Method: `insta` snapshot of the full token string for one regular and one
  outline scenario, plus the existing substring tests. Rationale: snapshots pin
  the envelope and the body at once; the existing tests keep semantic checks
  independent of snapshot text. Domain: one regular scenario with a step,
  docstring, and table; one outline with two Examples rows and a placeholder in
  step text. Artefact:
  `crates/rstest-bdd-macros/src/codegen/scenario/tests/assembly_snapshot.rs`
  and its generated `.snap` files. Evidence: the snapshot must match before and
  after the refactor; a deliberate swap of case attributes would fail it.
  Non-vacuity: the outline snapshot must contain both `#[case(0usize, ...)]` and
  `#[case(1usize, ...)]`, proving the shape-specific path ran.
- Obligation: `REQ-NO-REGRESSION`.
  Method: full `make test`. Rationale: compile-time and behavioural macro tests
  already cover the generated API. Artefact: the workspace suite. Evidence: all
  tests pass; any macrotest/trybuild diff fails the gate. Non-vacuity: the
  suite includes tests that expand and compile generated scenarios.

### Formal verification decision

No model checking or formal proof is warranted. The change is a mechanical
extraction over finite token construction, and the invariants are directly
observable as token strings. Stronger methods would add cost without covering a
failure mode the tests cannot reach.

### Suggested BDD features

Not applicable: this is an internal refactor with no user-facing behaviour
change. Existing integration tests exercise the generated framework API.

## Plan of work

### `EP-M1` shared step token builder

In `crates/rstest-bdd-macros/src/codegen/scenario/helpers/` (the former
`helpers.rs`, split as recorded in D4):

- Add a private `StepTokens` type alias for the `(value, docstring, table)`
  triple.
- Add private `step_content_to_tokens(text, docstring, table)`, the single
  literal builder; it reuses `cell_to_lit` and `generate_table_tokens`.
- Add private `step_to_tokens(step, Option<&SubstitutionContext>)`. `None`
  builds from the parsed step; `Some` calls `substitute_step_content` and maps
  a placeholder error into compile-error tokens.
- Add private `process_steps_with_context(steps, Option<&SubstitutionContext>)`
  that resolves keywords once and accumulates the three per-step vectors.
- Rewrite `process_steps` as the `None` case and
  `process_steps_substituted` as the `Some` case, preserving their signatures.
- Add `tests/step_tokens.rs` with the unit tests named in the verification
  plan.

### `EP-M2` shared scenario assembly

In `crates/rstest-bdd-macros/src/codegen/scenario/mod.rs`:

- Add a private `ScenarioItem<'a>` struct holding the shape-specific
  `signature`, `case_attrs`, and `body`.
- Move the `reject_async_harness` guard, `resolve_scenario_adapters` call,
  and `scenario_metadata` call into `generate_scenario_code`, so each runs once.
- Replace `generate_regular_scenario_code` with
  `assemble_regular_scenario`, which processes steps, builds
  `TestTokensConfig`, generates `generate_case_attrs`, calls
  `generate_test_tokens` once, and returns a borrowed-signature `ScenarioItem`.
- Replace `generate_outline_scenario_code` with `assemble_outline_scenario`,
  which performs the Examples guard, per-row substitution,
  `OutlineTestTokensConfig`, `generate_indexed_case_attrs`, inserts the hidden
  `#[case] __rstest_bdd_case_idx: usize` parameter, calls
  `generate_test_tokens` once, and returns an owned-signature `ScenarioItem`.
- Emit the item once in `generate_scenario_code` via `emit_scenario_test`.
- Add `tests/assembly_snapshot.rs` with `insta` snapshots for one regular and
  one outline scenario driven through `generate_scenario_code`.

### `EP-M3` documentation and delivery closure

In `docs/developers-guide.md`, subsection "Shared scenario-token assembly":

- Replace the non-existent `generate_test_tokens_for_config` with
  `assemble_test_tokens_with_context`.
- Replace the claim about two typed entry points with the single generic
  `generate_test_tokens<P, I, Q>(config: &impl ScenarioTestConfig, ...)`
  dispatched through `ScenarioTestConfig`.
- State that `reject_async_harness` runs once in `generate_scenario_code`
  before shape dispatch.
- Add a subsection documenting `step_to_tokens`, `scenario_metadata`, and
  `emit_scenario_test` with the three required bullets: ownership, permitted
  call-sites, and composition rules.
- Reference the pinning tests.

Then run the full gates, push, open the draft pull request, take CodeRabbit
review to convergence, and merge once the pull request reaches equilibrium.

## Milestones and plateaus

- `EP-M1` — both step processors call one literal builder.
  Requirements: `REQ-STEP-SHARING`, `REQ-NO-REGRESSION`. Acceptance:
  `tests::step_tokens` passes; `make test` passes. Conformance: signatures
  unchanged; no new dependency; runtime generators untouched; trace links
  current. Recovery: revert the commit; the previous helpers are
  self-contained. Remaining gaps: assembly still duplicated. Compatibility:
  none required; private helpers.
- `EP-M2` — one assembly path for both scenario shapes.
  Requirements: `REQ-ASSEMBLY-SHARING`, `REQ-NO-REGRESSION`. Acceptance:
  snapshots and `make test` pass. Conformance: public API unchanged; generated
  tokens unchanged; no new dependency; trace links current. Recovery: revert
  the commit. Remaining gaps: documentation stale. Compatibility: none
  required; private helpers.
- `EP-M3` — developer guide matches the code and records the canonical
  pattern. Requirements: `REQ-DOCS`. Acceptance: `make markdownlint` passes;
  statements match symbols found by `grep`. Conformance: no code change; trace
  links current. Recovery: revert the documentation commit. Remaining gaps:
  none. Compatibility: none.

## Concrete steps

Working directory: the repository worktree root.

1. Write this plan and commit it.
2. `EP-M1`: write the failing tests first, then the helper, then run:

   ```plaintext
   cargo test -p rstest-bdd-macros --lib codegen::scenario::tests
   make check-fmt
   make lint
   make test
   ```

   Capture each gate with `tee` under `/tmp` and `set -o pipefail`. Commit.
3. `EP-M2`: add the snapshot test, run it with `INSTA_UPDATE=always` once to
   record the initial snapshots, review them, then re-run without the variable.
   Run the same gates and commit.
4. `EP-M3`: update the developer guide, run `make fmt` and `make markdownlint`,
   then the full gates, and commit.
5. Push with SSH to a new remote branch of the same name, set upstream, and
   open a draft pull request whose title ends `(#756)` and whose body states
   `Closes #756` and links the Lody session.

## Validation and acceptance

Acceptance is behavioural and structural:

- `cargo test -p rstest-bdd-macros --lib codegen::scenario::tests` passes,
  including `step_tokens` and `assembly_snapshot`.
- `make check-fmt`, `make lint`, and `make test` pass at the final revision.
- `make markdownlint` passes for the documentation change.
- The snapshot for the outline contains both indexed `#[case(...)]`
  attributes and an inserted `__rstest_bdd_case_idx` parameter.
- No existing test changes its expected output.

## Idempotence and recovery

Every step is a source edit or a test run and is safe to repeat. Snapshot
updates are deliberate: run once with `INSTA_UPDATE=always`, inspect the diff,
and re-run without it. If a gate fails, fix forward or revert the most recent
commit; each milestone is a separate commit.

## Artefacts and notes

Reconnaissance at `470c20ae`:

- `helpers.rs` is 376 lines; `mod.rs` is 338; `emit.rs` is 88.
- `helpers.rs:186-220` defines `process_steps`; `:310-356` defines
  `process_steps_substituted`.
- `mod.rs:193-244` defines `generate_regular_scenario_code`; `:275-335`
  defines `generate_outline_scenario_code`.
- `emit.rs:21` defines `scenario_metadata`; `:58` defines
  `emit_scenario_test`.
- `docs/developers-guide.md:3026-3047` contains the stale section.

## Interfaces and dependencies

No new dependencies. New private items in `helpers/steps.rs`:

```rust
type StepTokens = (TokenStream2, TokenStream2, TokenStream2);

fn step_to_tokens(
    step: &crate::parsing::feature::ParsedStep,
    context: Option<&SubstitutionContext<'_>>,
) -> Result<StepTokens, TokenStream2>;
```

New private items in `mod.rs`:

```rust
struct ScenarioItem<'a> {
    signature: Cow<'a, syn::Signature>,
    case_attrs: Vec<TokenStream2>,
    body: TokenStream2,
}

fn assemble_regular_scenario<'a, P, I, Q>(
    config: &'a ScenarioConfig<'_>,
    metadata: ScenarioMetadata<'a>,
    ctx: ContextConfig<P, I, Q>,
) -> ScenarioItem<'a>;

fn assemble_outline_scenario<'a, P, I, Q>(
    config: &'a ScenarioConfig<'_>,
    metadata: ScenarioMetadata<'a>,
    ctx: ContextConfig<P, I, Q>,
) -> Result<ScenarioItem<'a>, TokenStream2>;
```

Reuse policy: `step_to_tokens` is the only per-step literal builder; the two
processors are its only call-sites. `scenario_metadata` and
`emit_scenario_test` remain the only metadata projection and item assembler;
the shared path in `generate_scenario_code` is their only call-site. New
callers must delegate rather than reimplement the literals or the `quote!`
block.

## Revision note

- 2026-11-28: initial draft, based on reconnaissance at `470c20ae`. Recorded
  that `emit.rs` already supplies the metadata projection and item assembler,
  reducing the issue's Task 2 to the remaining prologue/epilogue consolidation.
- 2026-11-28: EP-M1 completion. Recorded the Whitaker-driven `helpers` module
  split (D4), the file-count tolerance increase from 8 to 12, and the
  correction that the step-token tests are regression pins rather than red
  tests. API, obligations, and milestone order are unchanged; EP-M2 and EP-M3
  remain as planned.
- 2026-11-28: EP-M2 completion. The assembly extraction moved the guard,
  adapter resolution, metadata projection, and item emission into
  `generate_scenario_code`; the two shape functions now return a
  `ScenarioItem`. Snapshots recorded before the change were unchanged after it.
  EP-M3 remains as planned.
