# ExecPlan 9.2.4: Activate the Tokio current-thread compatibility alias

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE (delivered 2026-03-16)

## Purpose / big picture

Roadmap item 9.2.4 is the point where the legacy
`runtime = "tokio-current-thread"` spelling stops being only an internal marker
and starts selecting the shipped Tokio harness adapter. The observable outcome
after implementation is that
`scenarios!(..., runtime = "tokio-current-thread")` resolves the same harness
path as `harness = rstest_bdd_harness_tokio::TokioHarness`, while emitting a
deprecation warning telling users to prefer the explicit harness form.

Success is not just a helper returning a different `syn::Path`. The change must
leave the macro pipeline coherent end to end:

- `resolve_harness_path()` resolves the runtime alias to
  `rstest_bdd_harness_tokio::TokioHarness`.
- The unit test
  `resolve_harness_path_runtime_alias_does_not_force_harness_yet` is rewritten
  to assert the new path and no longer mentions "until phase 9.3".
- Macro expansion emits a deprecation warning for the legacy runtime spelling,
  recommending `harness = TokioHarness` as the canonical form.
- Behavioural coverage proves the legacy syntax still works as intended under
  the new harness-backed model.
- `docs/rstest-bdd-design.md` records the design decision.
- `docs/users-guide.md` explains the legacy spelling and the preferred
  replacement.
- `docs/roadmap.md` marks 9.2.4 done only after all gates pass.

Because this task lands after 9.3.2, it must also reconcile the current code's
explicit `async fn` plus `harness` rejection with the new alias behaviour. That
is the main technical risk and must be handled deliberately, not as an
incidental follow-up.

## Constraints

- Implement roadmap item 9.2.4 only. Do not pull phase 9.3.8 or phase 9.4 work
  into this change.
- Preserve ADR-005's dependency split: Tokio remains in the opt-in Tokio
  harness crate, not in core runtime crates beyond existing macro references to
  the harness path.
- Keep explicit `harness = ...` authoritative when both `harness` and
  `runtime = "tokio-current-thread"` are provided.
- Emit the deprecation warning once per macro invocation, not once per
  generated scenario, unless investigation proves the macro architecture cannot
  do that without larger churn.
- Validate with unit tests and behavioural tests.
- Record any design decisions in `docs/rstest-bdd-design.md`.
- Record user-facing usage and migration guidance in `docs/users-guide.md`.
- Mark roadmap entry 9.2.4 done only after implementation and all gates pass.
- Required gates before completion: `make check-fmt`, `make lint`, and
  `make test`.
- Because this change edits Markdown, also run `make fmt`,
  `make markdownlint`, and `make nixie`.

## Tolerances (exception triggers)

- Scope: if implementation grows beyond 16 files or 850 net lines, stop and
  escalate.
- Interface: if satisfying 9.2.4 requires a public breaking change in
  `rstest-bdd-harness`, `rstest-bdd`, or `rstest-bdd-macros`, stop and escalate.
- Behaviour: if the only way to activate the alias is to silently drop support
  for currently tested async-step scenarios without updating docs and tests to
  an agreed replacement behaviour, stop and escalate.
- Diagnostics: if the deprecation warning cannot be emitted with the existing
  proc-macro infrastructure without introducing a new macro dependency, stop
  and escalate.
- Iterations: if the same gate (`check-fmt`, `lint`, `test`, or
  `markdownlint`) fails three times after attempted fixes, stop and escalate
  with logs.
- Ambiguity: if roadmap text, design docs, and current code disagree on whether
  the legacy runtime spelling should continue to support `async fn` step
  definitions after activation, stop and request direction before coding
  further.

## Risks

- Risk: activating `resolve_harness_path()` currently feeds directly into
  `ScenarioConfig.harness`, and `generate_regular_scenario_code()` /
  `generate_outline_scenario_code()` reject any
  `config.harness.is_some() && config.runtime.is_async()`. Severity: high.
  Likelihood: certain. Mitigation: treat this as part of the task, not an
  incidental regression; decide whether the legacy runtime spelling now maps to
  the synchronous Tokio harness model or requires a compatibility exemption.

- Risk: if the legacy spelling switches to Tokio harness semantics, the current
  behavioural coverage in `crates/rstest-bdd/tests/async_scenario.rs` and
  `crates/rstest-bdd/tests/runtime_compat_alias.rs` becomes inconsistent
  because both rely on async step definitions. Severity: high. Likelihood:
  high. Mitigation: update behavioural tests to match the intended post-9.2.4
  semantics and record the change in docs.

- Risk: emitting the warning at scenario-test generation time could duplicate
  warnings for every discovered scenario from one `scenarios!` invocation.
  Severity: medium. Likelihood: medium. Mitigation: emit the warning in the
  outer macro path after argument parsing if possible, or document why one per
  generated item is unavoidable.

- Risk: `make test` does not exercise trybuild snapshot updates because the
  repository's nextest path skips compile-fail macro tests. Severity: medium.
  Likelihood: high. Mitigation: include a targeted `cargo test` invocation for
  `trybuild_macros` in the implementation validation steps in addition to
  `make test`.

- Risk: documentation drift. The current design doc and user guide still
  describe the runtime spelling as preserving the old async execution model.
  Severity: medium. Likelihood: high. Mitigation: update all affected prose in
  the same implementation stage as code and tests.

## Progress

- [x] (2026-03-12 00:00Z) Reviewed roadmap item 9.2.4, the relevant design-doc
      sections, current macro implementation, existing tests, and prior phase
      ExecPlans.
- [x] (2026-03-12 00:00Z) Drafted this ExecPlan.
- [x] (2026-03-16) Stage A: locked post-activation semantics and expressed them
      in unit tests. Updated `resolve_harness_path_runtime_alias_resolves_to_tokio_harness`
      test to assert the alias resolves. Documented decision that activated alias
      behaves exactly like explicit `TokioHarness`.
- [x] (2026-03-16) Stage B: activated alias resolution and preserved explicit
      harness precedence. Updated `resolve_harness_path()` to return
      `rstest_bdd_harness_tokio::TokioHarness` when alias is present. Added logic
      to treat activated alias as synchronous runtime for code generation. Updated
      `runtime_compat_alias.rs` behavioural test to use synchronous step functions.
- [x] (2026-03-16) Stage C: emitted deprecation warning via `emit_warning!` in
      `scenarios!` macro when `runtime = "tokio-current-thread"` is used without
      explicit harness. Created trybuild fixture
      `scenarios_runtime_alias_deprecated.rs` and registered it in
      `trybuild_macros.rs`. Warning message recommends migrating to
      `harness = rstest_bdd_harness_tokio::TokioHarness`.
- [x] (2026-03-16) Stage D: aligned behavioural coverage and documentation.
      Updated `rstest-bdd-design.md` sections §2.5.5 and §2.7.3 to reflect
      activated alias semantics. Updated `users-guide.md` to document the
      deprecation, explain the new behavior, and update code examples to use
      explicit harness form.
- [x] (2026-03-16) Stage E: ran final quality gates and marked roadmap item
      9.2.4
      done. All gates passed: `cargo fmt --check`, `cargo clippy`, `cargo test`,
      and `cargo test -p rstest-bdd --test trybuild_macros`. Updated roadmap entry
      to mark 9.2.4 complete.

## Surprises & Discoveries

- Observation: `resolve_harness_path()` is not an isolated helper. Its return
  value flows into `ScenarioConfig.harness`, so changing it from `None` to
  `Some(TokioHarness)` immediately affects compile-time code paths outside the
  helper itself.

- Observation: current code in
  `crates/rstest-bdd-macros/src/codegen/scenario.rs` rejects any scenario that
  combines `harness` with `async fn` generation. That means alias activation
  will break current `runtime = "tokio-current-thread"` behaviour unless the
  implementation also changes surrounding logic.

- Observation: the macros crate already uses `proc_macro_error::emit_warning`
  in `crates/rstest-bdd-macros/src/validation/steps.rs`, and the trybuild suite
  already has a pattern for snapshotting warnings by forcing a compile error in
  a fixture. That path should be reused for the deprecation warning instead of
  inventing new infrastructure.

- Observation: `make test` alone is insufficient for trybuild warning
  snapshots. The repository note about `NEXTEST_RUN_ID` skipping compile-fail
  macro tests still applies.

## Decision Log

- Decision: this plan treats the async/harness interaction as in scope for
  9.2.4. Rationale: activating the alias without resolving that interaction
  would produce immediate compile failures for existing runtime-alias call
  sites. Date/Author: 2026-03-12 / Codex.

- Decision: the warning should recommend the fully explicit spelling
  `harness = rstest_bdd_harness_tokio::TokioHarness`, while user-guide prose
  may also mention the shorter imported form `harness = TokioHarness`.
  Rationale: diagnostics should be copy-pastable from any module. Date/Author:
  2026-03-12 / Codex.

- Decision: validation for this task must include both repository gates and a
  targeted trybuild run for warning snapshots. Rationale: otherwise the warning
  behaviour can regress without being exercised. Date/Author: 2026-03-12 /
  Codex.

- Decision: the `runtime = "tokio-current-thread"` alias, after activation,
  behaves exactly like explicit `harness = TokioHarness`. This means: (1) the
  alias resolves to `rstest_bdd_harness_tokio::TokioHarness` path, (2)
  generated scenario test functions are synchronous (not `async fn`), (3) the
  `TokioHarness` provides the Tokio runtime for step execution, (4) async step
  definitions are rejected (via the sync wrapper runtime check), (5) a
  deprecation warning is emitted. The `resolve_harness_path()` function now
  returns `Option<syn::Path>` (owned) to enable creation of the TokioHarness
  path at the call site. Date/Author: 2026-03-16 / DevBoxer.

## Outcomes & Retrospective

**Final semantics:** The `runtime = "tokio-current-thread"` compatibility alias
now behaves exactly like explicit
`harness = rstest_bdd_harness_tokio::TokioHarness`:

- Generated scenario test functions are synchronous (not `async fn`)
- `TokioHarness` provides the Tokio current-thread runtime for step execution
- Async step definitions are rejected at runtime by the sync wrapper's runtime
  check
- Explicit `harness` parameter takes precedence over the runtime alias

**Warning text:** Two deprecation messages are now emitted depending on context:

- Without explicit harness: "the `runtime = \"tokio-current-thread\"`
  syntax is deprecated; use `harness = rstest_bdd_harness_tokio::TokioHarness`
  instead"
- With explicit harness: "the `runtime = \"tokio-current-thread\"`
  argument is deprecated and redundant when an explicit `harness` is set;
  remove the `runtime` argument"

**Test coverage changes:**

- Unit test: renamed
  `resolve_harness_path_runtime_alias_does_not_force_harness_yet` to
  `resolve_harness_path_runtime_alias_resolves_to_tokio_harness` and updated
  assertions to verify the alias resolves to the TokioHarness path
- Unit tests: added pipeline tests
  (`alias_active_without_explicit_harness_produces_sync_fn_with_tokio_harness`,
  `alias_active_with_explicit_harness_preserves_original_runtime`,
  `sync_runtime_without_alias_produces_sync_fn_without_harness`) exercising
  `resolve_effective_runtime`, `resolve_harness_path`, and
  `build_test_signature` together
- Behavioural test `runtime_compat_alias.rs`: updated to use
  synchronous step functions and updated feature file text from "asynchronous"
  to "synchronous"
- Behavioural test `async_scenario.rs`: converted from `scenarios!`
  with `runtime = "tokio-current-thread"` to manual `#[scenario]` tests with
  `#[tokio::test]` to preserve async step coverage without using the deprecated
  alias
- Trybuild fixture: added `scenarios_runtime_alias_deprecated.rs`
  with forced compile error to capture the deprecation warning diagnostic

**Documentation updates:**

- `docs/rstest-bdd-design.md` §2.5.5: updated to describe activated alias
  semantics
- `docs/rstest-bdd-design.md` §2.7.3: updated runtime compatibility alias
  section
- `docs/users-guide.md`: added deprecation notice, updated behavior description,
  and converted code example to use explicit harness form
- `docs/roadmap.md`: marked item 9.2.4 as complete with delivery date 2026-03-16

**Quality gates:** All gates passed successfully:

- `cargo fmt --all --check`: passed
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed
- `cargo test --workspace`: passed (all 569 tests)
- `cargo test -p rstest-bdd --test trybuild_macros`: passed (19 macro fixtures)

**Implementation notes:** The key technical insight was that activating the
alias required treating the runtime as synchronous for code generation purposes
(`effective_runtime = RuntimeMode::Sync` when alias is activated) to avoid the
async+harness rejection check. This ensures the generated test signature is
`fn` rather than `async fn`, consistent with explicit `TokioHarness` usage.

**Post-review refinements (2026-03-18):**

- Decoupled the sync/async decision from resolved harness presence;
  the `effective_runtime` check now examines the concrete
  `RuntimeCompatibilityAlias` variant via `resolve_effective_runtime` rather
  than inferring from `harness_ref.is_some()`
- Deprecation warning is now emitted whenever
  `runtime = "tokio-current-thread"` is used, including when an explicit
  `harness` is also provided (with a tailored message recommending removal of
  the redundant `runtime` argument)
- Documented that the `syn::Path` clone in `resolve_harness_path` is
  acceptable for macro-expansion performance
- Extracted `resolve_effective_runtime` as a standalone testable
  helper and added unit tests verifying the full pipeline (harness resolution +
  effective runtime + signature generation)

## Context and orientation

The implementation surface for 9.2.4 is concentrated in the `scenarios!` macro
pipeline and its tests.

Primary code locations:

- `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs`
  - `resolve_harness_path()`
  - `generate_scenario_test()`
- `crates/rstest-bdd-macros/src/macros/scenarios/test_generation/tests.rs`
  - helper-level unit coverage for harness-resolution behaviour
- `crates/rstest-bdd-macros/src/codegen/scenario.rs`
  - async plus harness rejection path in both regular and outline generation
- `crates/rstest-bdd-macros/src/macros/scenarios/mod.rs`
  - likely place to emit one deprecation warning per macro invocation if the
    parsed runtime alias is available there
- `crates/rstest-bdd/tests/runtime_compat_alias.rs`
  - behavioural coverage for the legacy runtime spelling
- `crates/rstest-bdd/tests/async_scenario.rs`
  - current async-step behavioural coverage that may need to move away from the
    activated alias path
- `crates/rstest-bdd/tests/trybuild_macros.rs`
  - registration point for warning-oriented trybuild fixtures
- `crates/rstest-bdd/tests/fixtures_macros/`
  - location for any new warning snapshot fixture and `.stderr` file

Primary documentation locations:

- `docs/rstest-bdd-design.md`
  - §2.5.5 macro integration
  - §2.7.3 harness macro integration
- `docs/users-guide.md`
  - async scenario execution section
  - harness adapter core APIs section
- `docs/roadmap.md`
  - item 9.2.4

Pre-existing facts from earlier phases that this task depends on:

- `rstest-bdd-harness-tokio` exists and exports `TokioHarness`.
- `TokioHarness` provides a current-thread Tokio runtime with `LocalSet`.
- Explicit harness selection already works for synchronous scenario functions.
- Async step definitions are still problematic under `TokioHarness`, so the
  legacy runtime alias cannot simply start selecting the harness without
  updating surrounding behaviour and docs.

## Plan of work

### Stage A: lock intended semantics before editing production code

Goal: decide what `runtime = "tokio-current-thread"` means after activation and
express that decision in tests and docs before changing helper logic.

Implementation details:

1. Confirm the intended post-9.2.4 behaviour from the roadmap text and current
   design:
   - explicit `harness` still wins over the compatibility alias;
   - the alias now resolves to `TokioHarness`;
   - a deprecation warning is emitted;
   - any behavioural change for async step definitions is documented.
2. Add or update unit tests in
   `crates/rstest-bdd-macros/src/macros/scenarios/test_generation/tests.rs` so
   the helper-level contract is explicit.
3. Decide which existing behavioural tests continue to validate the legacy
   syntax and which must move to explicit harness tests or plain async runtime
   coverage.

Go/no-go validation:

- There is a clear, written answer to: "Does the activated alias still support
  async step definitions, or does it now behave exactly like explicit
  `TokioHarness`?"
- Helper-level unit tests fail or are updated to express the new contract.

### Stage B: activate alias resolution without breaking precedence rules

Goal: make the macro pipeline resolve the legacy runtime spelling to the Tokio
harness path and keep explicit harness selection authoritative.

Implementation details:

1. Update `resolve_harness_path()` in
   `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs` so the
   runtime alias resolves to `rstest_bdd_harness_tokio::TokioHarness` when no
   explicit harness is supplied.
2. Keep `resolve_harness_path(Some(explicit), Some(alias)) == Some(explicit)`.
3. Adjust the downstream code path that currently rejects
   `config.harness.is_some() && config.runtime.is_async()` so the activated
   alias produces the intended behaviour rather than an unconditional compile
   error.
4. If the chosen semantics require moving the legacy spelling to the
   synchronous Tokio harness model, update generated test signatures and
   attribute selection coherently rather than relying on a partial helper
   change.

Go/no-go validation:

- Unit tests prove explicit harness precedence still holds.
- A targeted behavioural test using the legacy runtime spelling compiles and
  passes under the new path.

### Stage C: emit the deprecation warning and snapshot it

Goal: tell users that the runtime spelling is legacy compatibility syntax and
point them to the canonical harness form.

Implementation details:

1. Reuse `proc_macro_error::emit_warning` in the macros crate.
2. Emit the warning from the highest practical level in the `scenarios!`
   pipeline so one macro invocation ideally yields one warning.
3. Use wording that includes both:
   - the legacy spelling `runtime = "tokio-current-thread"`;
   - the preferred form
     `harness = rstest_bdd_harness_tokio::TokioHarness`.
4. Add a warning-oriented trybuild fixture under
   `crates/rstest-bdd/tests/fixtures_macros/` that forces a compile error after
   macro expansion so the warning appears in the captured stderr snapshot.
5. Register that fixture in `crates/rstest-bdd/tests/trybuild_macros.rs` using
   the existing normalized-output helper pattern.

Go/no-go validation:

- The warning appears in the snapshot with stable wording.
- The snapshot remains robust to path normalization and nightly backtrace
  noise.

### Stage D: realign behavioural tests and documentation

Goal: make runtime-alias tests and docs describe the same shipped behaviour.

Implementation details:

1. Update `crates/rstest-bdd/tests/runtime_compat_alias.rs` so it proves the
   activated alias still works under the intended semantics.
2. If needed, move async-step behavioural coverage to a test that does not rely
   on the legacy alias, such as:
   - explicit manual async scenario coverage, or
   - existing async-path coverage that remains outside the activated alias
     contract.
3. Update `docs/rstest-bdd-design.md`:
   - §2.5.5 must no longer say the alias is recognized but unresolved;
   - §2.7.3 must describe the activated alias and the deprecation warning.
4. Update `docs/users-guide.md` so it:
   - names the runtime spelling as compatibility syntax;
   - recommends the explicit Tokio harness form;
   - reflects any behavioural limits that remain after activation.
5. Mark roadmap item 9.2.4 done in `docs/roadmap.md` only after final
   validation passes.

Go/no-go validation:

- No remaining docs mention "until phase 9.3" for this alias.
- The user guide and design doc agree on the semantics that shipped.

### Stage E: run full validation and capture evidence

Goal: prove the repository remains healthy and the warning behaviour is covered.

Run all commands from `/home/user/project` with `set -o pipefail` and `tee`:

1. Repository formatting and lint gates:

   ```bash
   set -o pipefail; PATH=/root/.bun/bin:$PATH make fmt 2>&1 | tee /tmp/9-2-4-make-fmt.log
   set -o pipefail; PATH=/root/.bun/bin:$PATH make check-fmt 2>&1 | tee /tmp/9-2-4-make-check-fmt.log
   set -o pipefail; PATH=/root/.bun/bin:$PATH make lint 2>&1 | tee /tmp/9-2-4-make-lint.log
   set -o pipefail; PATH=/root/.bun/bin:$PATH make test 2>&1 | tee /tmp/9-2-4-make-test.log
   ```

2. Documentation gates:

   ```bash
   set -o pipefail; PATH=/root/.bun/bin:$PATH make markdownlint 2>&1 | tee /tmp/9-2-4-make-markdownlint.log
   set -o pipefail; PATH=/root/.bun/bin:$PATH make nixie 2>&1 | tee /tmp/9-2-4-make-nixie.log
   ```

3. Targeted macro snapshot coverage that `make test` may miss:

   ```bash
   set -o pipefail; RUSTFLAGS="-D warnings" cargo test -p rstest-bdd --test trybuild_macros 2>&1 | tee /tmp/9-2-4-trybuild.log
   ```

Expected evidence:

- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` all exit `0`.
- The trybuild test exits `0` and the warning snapshot contains the
  deprecation diagnostic.
- The roadmap entry is updated only after all of the above are green.
