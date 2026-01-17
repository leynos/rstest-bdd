# Add Clippy expect attributes to step wrappers

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

PLANS.md does not exist in this repository, so no additional plan governance
applies.

## Purpose / Big Picture

Downstream crates using `rstest-bdd` with strict Clippy pedantic settings
currently receive warnings from macro-generated step wrapper code. The goal is
for every generated wrapper function to include tightly scoped `#[expect(..)]`
attributes so downstream users no longer need module-level lint suppressions.
Success is observable by running `cargo clippy` (with pedantic enabled) in the
example projects and seeing no warnings tied to wrapper generation, and by
verifying the macro expansion includes the `#[expect(...)]` attributes.

## Constraints

- Only adjust macro-generated wrapper emission in
  `crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly.rs`.
- Use `#[expect(...)]`, not `#[allow(...)]`, and include a single clear reason
  string that matches the specified wording.
- Do not change public macro input syntax or runtime behaviour of steps; only
  add lint suppression attributes.
- Do not add new dependencies or change `Cargo.toml`.
- All commits must be gated by `make check-fmt`, `make lint`, and `make test`.

## Tolerances (Exception Triggers)

- Scope: if the change requires edits to more than 4 files or more than 200 net
  lines, stop and escalate.
- Interface: if any public API signature must change, stop and escalate.
- Dependencies: if a new dependency is required, stop and escalate.
- Iterations: if any required test/lint command fails more than twice, stop and
  escalate with the failure logs.
- Time: if any single milestone takes more than 2 hours, stop and escalate.
- Ambiguity: if the suppression list or reason text conflicts with existing
  documentation, stop and ask for confirmation.

## Risks

    - Risk: the attribute is emitted in the wrong position, causing parsing or
      attribute-order issues.
      Severity: medium
      Likelihood: low
      Mitigation: follow the existing `#[expect(...)]` pattern in
      `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs` and
      add a unit test that parses the generated wrapper into `syn::ItemFn` to
      verify the attribute list.

    - Risk: tests for wrapper generation are missing or hard to write, leading
      to unvalidated behaviour.
      Severity: medium
      Likelihood: medium
      Mitigation: add a focused unit test in
      `crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly.rs` that calls
      the local `assemble_wrapper_function` directly with minimal inputs.

## Progress

    - [x] (2026-01-17 00:00Z) Drafted ExecPlan.
    - [x] (2026-01-17 00:05Z) Inspected wrapper emission and lint suppression
          patterns.
    - [x] (2026-01-17 01:40Z) Added wrapper lint suppression emission and unit
          test.
    - [x] (2026-01-17 01:46Z) Ran format, lint, and test gates; captured logs.
    - [x] (2026-01-17 01:49Z) Committed the change with a descriptive message.

## Surprises & Discoveries

    - Observation: `make markdownlint` failed because `markdownlint` was not
      installed; the repository uses `markdownlint-cli2` via `mdformat-all`.
      Evidence: `make markdownlint` exited with `xargs: markdownlint: No such
      file or directory`.
      Impact: Overrode `MDLINT` to `markdownlint-cli2` for the lint step.

## Decision Log

    - Decision: Use `#[expect(...)]` on wrapper functions with a single reason
      clause covering all six Clippy lints.
      Rationale: Aligns with existing lint handling in scenario generation and
      keeps suppressions narrowly scoped to generated wrappers.
      Date/Author: 2026-01-17 (assistant)
    - Decision: Run `make markdownlint` with `MDLINT=markdownlint-cli2` because
      `markdownlint` is not installed in this environment.
      Rationale: The Makefile supports an override, and the CLI2 tool is
      already present via `mdformat-all`.
      Date/Author: 2026-01-17 (assistant)

## Outcomes & Retrospective

Added a wrapper-level `#[expect(...)]` attribute for the six unavoidable Clippy
lints and a unit test that parses the generated wrapper to assert the attribute
contents. All formatting, linting, and test gates passed, including Mermaid
validation. The only surprise was the missing `markdownlint` binary, handled
via the `MDLINT` override.

## Context and Orientation

Wrapper functions are emitted in the macro crate under
`crates/rstest-bdd-macros`. The wrapper body is assembled in
`crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly.rs` by the
`assemble_wrapper_function` function, which returns a `TokenStream2` for the
synchronous wrapper. The wrapper code is then included in the output of
`generate_wrapper_code` in
`crates/rstest-bdd-macros/src/codegen/wrapper/emit.rs`. The existing pattern
for emitting `#[expect(...)]` attributes lives in
`crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs` in
`build_lint_attributes`, which uses `syn::parse_quote!` to create a structured
attribute with a reason string.

The requested change is to emit a single `#[expect(...)]` attribute on every
wrapper function with these lint names: `clippy::shadow_reuse`,
`clippy::unnecessary_wraps`, `clippy::str_to_string`,
`clippy::redundant_closure_for_method_calls`, `clippy::needless_pass_by_value`,
and `clippy::redundant_closure`, plus the reason text: "rstest-bdd step wrapper
pattern requires these patterns for parameter extraction, Result normalization,
and closure-based error handling".

## Plan of Work

Stage A: understand and propose (no code changes).

Review `assemble_wrapper_function` in
`crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly.rs` to confirm
where wrapper attributes and signatures are assembled. Review
`crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs` to mirror
the existing `#[expect(...)]` attribute construction style and reason
formatting.

Stage B: scaffolding and tests (small, verifiable diffs).

Add a unit test in
`crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly.rs` (under
`#[cfg(test)]`) that constructs minimal wrapper inputs and calls the local
`assemble_wrapper_function`. Parse the resulting token stream into
`syn::ItemFn` and assert the `expect` attribute contains all six lints and the
reason string. This test should fail before the attribute is added and pass
once implemented.

Stage C: implementation (minimal change to satisfy tests).

Update `assemble_wrapper_function` to emit a `#[expect(...)]` attribute before
`fn #wrapper_ident(...)` using the exact lint list and reason clause specified
above. Follow the `syn::parse_quote!` pattern from `build_lint_attributes` and
ensure the attribute is applied to every wrapper function unconditionally.

Stage D: hardening, documentation, cleanup.

Run formatting, linting, and tests via Makefile targets. If the change affects
user-facing guidance or documented lint policy, update the relevant docs under
`docs/` (none expected for this change unless a new policy is introduced).

## Concrete Steps

All commands run from the repository root
`/data/leynos/Projects/rstest-bdd.worktrees/issue-381-step-macros-triggering-multiple-clippy-lints`.
 Use `tee` for long outputs as required. If `get-project` is unavailable,
replace `$(get-project)` with `$(basename "$PWD")`.

1. Inspect wrapper assembly and lint attribute patterns:

    rg -n "assemble_wrapper_function" \
      crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly.rs
    rg -n "expect\(" \
      crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs

2. Add the unit test in
   `crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly.rs` as described
   in Stage B.

3. Add the `#[expect(...)]` attribute emission in `assemble_wrapper_function`.

4. Run quality gates (capture logs):

    make check-fmt 2>&1 | tee /tmp/check-fmt-$(get-project)-$(git branch --show).out
    make lint 2>&1 | tee /tmp/lint-$(get-project)-$(git branch --show).out
    make test 2>&1 | tee /tmp/test-$(get-project)-$(git branch --show).out

5. Optional downstream verification (if time permits) to confirm example
   projects are clean with pedantic Clippy:

    (cd examples/todo-cli && cargo clippy --all-targets --all-features \
      2>&1 | tee /tmp/clippy-todo-$(get-project)-$(git branch --show).out)
    (cd examples/japanese-ledger && cargo clippy --all-targets --all-features \
      2>&1 | tee /tmp/clippy-jledger-$(get-project)-$(git branch --show).out)

6. Commit once all gates pass. Suggested message:

    Add wrapper expect attributes for Clippy lints

    Add a single `#[expect(...)]` attribute to generated step wrapper
    functions to suppress unavoidable pedantic Clippy warnings. Includes a
    unit test that validates the attribute contents.

## Validation and Acceptance

Acceptance means:

- The generated wrapper function includes a single `#[expect(...)]` attribute
  listing all six Clippy lints and the specified reason string.
- The new unit test passes and verifies the attribute emission.
- `make check-fmt`, `make lint`, and `make test` succeed without warnings.
- Optional: `cargo clippy` in the example projects emits no warnings tied to
  wrapper generation when pedantic lints are enabled.

Quality criteria:

- Tests: `make test` passes; the new wrapper emission unit test fails before
  the change and passes after.
- Lint/typecheck: `make lint` succeeds with `-D warnings`.
- Formatting: `make check-fmt` passes.

## Idempotence and Recovery

All edits are additive or small adjustments to existing code. If any command
fails, fix the reported issue and re-run the same command. If the new test is
flaky, stop and reassess rather than loosening assertions.

## Artifacts and Notes

Capture key command logs in `/tmp/*-$(get-project)-$(git branch --show).out`.
If needed, include a brief excerpt of the new test assertion and the emitted
attribute in a follow-up note or commit message body.

## Interfaces and Dependencies

No new dependencies are permitted. The wrapper function signature and runtime
behaviour remain unchanged. The only interface-level change is the addition of
one `#[expect(...)]` attribute applied to the generated wrapper function. The
attribute must include the six lint names and the exact reason string provided
in the task description.

## Revision note (required when editing an ExecPlan)

Initial draft created on 2026-01-17. No revisions yet.

Revision note (2026-01-17): Marked the plan as in progress and recorded the
initial inspection step as complete. No implementation changes yet.

Revision note (2026-01-17): Recorded completion of the wrapper lint suppression
emission and unit test work. Validation and commit steps remain.

Revision note (2026-01-17): Marked the format/lint/test gates as complete after
rerunning the required Makefile targets.

Revision note (2026-01-17): Marked the plan complete, added the markdownlint
tooling discovery, and summarised outcomes after the final gate runs.
