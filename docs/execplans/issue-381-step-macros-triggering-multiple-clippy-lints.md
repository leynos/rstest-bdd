# Emit Conditional Clippy Expect Lints

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

PLANS.md does not exist in this repository, so no additional plan governance
applies.

## Purpose / Big Picture

Downstream crates using `rstest-bdd` with strict Clippy pedantic settings still
receive warnings because `#[expect(...)]` is emitted unconditionally and trips
`unused_lint_expectations` on wrappers that do not exercise every lint. The
goal is to emit `#[expect(...)]` entries only when the corresponding wrapper
patterns can occur, while deduplicating the lint list used by codegen and tests
and keeping the tests simple. Success is observable by running `make lint`
without new warnings and by confirming the wrapper tests assert against the
same shared lint list and reason string.

## Constraints

- Keep lint suppression scoped to generated wrapper functions using
  `#[expect(...)]` with the existing reason string.
- Do not add new dependencies or change `Cargo.toml`.
- Preserve public macro input syntax and runtime behaviour; only adjust emitted
  wrapper attributes and supporting tests.
- Update documentation references without altering meaning.
- All commits must be gated by `make check-fmt`, `make lint`, and `make test`.

## Tolerances (Exception Triggers)

- Scope: if edits exceed 6 files or 250 net lines, stop and escalate.
- Interface: if any public API signature must change, stop and escalate.
- Dependencies: if a new dependency is required, stop and escalate.
- Iterations: if any required test or lint command fails more than twice, stop
  and escalate with logs.
- Ambiguity: if the conditions for emitting specific lints are unclear or
  contested, stop and confirm the intended mapping.

## Risks

```
- Risk: conditional lint emission misses a wrapper shape that triggers a
  lint, causing Clippy warnings to reappear downstream.
  Severity: medium
  Likelihood: medium
  Mitigation: map lint emission to explicit wrapper patterns and cover those
  in the unit test expectations.

- Risk: a shared lint list used in tests diverges from codegen logic.
  Severity: medium
  Likelihood: low
  Mitigation: centralise lint names in one constant and reuse it in both
  generation and tests.

- Risk: documentation reference changes could break existing links.
  Severity: low
  Likelihood: low
  Mitigation: keep reference definitions valid and let `make fmt` reformat.
```

## Progress

```
- [x] (2026-01-18 19:05Z) Update wrapper emit logic to build a conditional
      lint list per wrapper.
- [x] (2026-01-18 19:12Z) Share lint names between codegen and tests.
- [x] (2026-01-18 19:24Z) Simplify wrapper expect attribute test helpers and
      split tests into `assembly/tests.rs`.
- [x] (2026-01-18 19:26Z) Fix documentation reference indentation.
- [x] (2026-01-18 19:40Z) Run format, lint, and test gates with logs.
- [x] (2026-01-18 19:43Z) Commit and push the changes.
```

## Surprises & Discoveries

```
- Observation: `assembly.rs` exceeded the 400-line limit after adding
  conditional lint logic and expanded tests.
  Evidence: `make lint` failed with `check_rs_file_lengths.py` reporting
  the file length.
  Impact: moved tests into `assembly/tests.rs` to keep the module within
  size limits.
```

## Decision Log

```
- Decision: compute wrapper lint expectations from explicit wrapper
  shape flags (placeholders, return kind, quote stripping, step structs).
  Rationale: ensures `#[expect(...)]` only lists lints that can occur,
  avoiding `unused_lint_expectations`.
  Date/Author: 2026-01-18 (assistant)

- Decision: move wrapper lint tests into `assembly/tests.rs`.
  Rationale: keep `assembly.rs` below the 400-line limit enforced by the
  repository lint gate.
  Date/Author: 2026-01-18 (assistant)
```

## Outcomes & Retrospective

Wrapper emission now computes a conditional list of Clippy lint expectations
per wrapper shape, preventing `unused_lint_expectations` while keeping
suppression scopes tight. Lint names are shared between codegen and tests, and
the wrapper attribute tests were moved into a dedicated module to stay under
the file length limit. All format, lint, and test gates passed.

## Context and Orientation

Wrapper functions are emitted in
`crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly.rs`. Argument
preparation happens in
`crates/rstest-bdd-macros/src/codegen/wrapper/arguments.rs`. The existing unit
tests for wrapper attributes live in
`crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly/tests.rs`.
Documentation references appear in `docs/rstest-bdd-language-server-design.md`.

## Plan of Work

Stage A: inspect and confirm wrapper patterns.

Review wrapper assembly and argument preparation to identify which wrapper
patterns trigger specific Clippy lints (shadowing, wraps, quote stripping,
closure usage). Decide on explicit conditions for each lint in a small helper
function and confirm the reason string remains unchanged.

Stage B: implement conditional lint list and shared constants.

Introduce a single constant list of lint names. Add a helper that returns a
per-wrapper set based on the prepared arguments and return kind. Thread that
list into `generate_expect_attribute` so it emits only when non-empty. Update
argument preparation if needed to carry lint info into rendering.

Stage C: simplify tests and align with shared list.

Refactor the wrapper expect attribute test helper to parse only the expected
attribute shape and assert against the shared lint list and reason string.

Stage D: documentation, validation, and commit.

Fix the reference indentation in `docs/rstest-bdd-language-server-design.md`,
run formatting and lint/test Makefile targets, then commit and push once all
gates succeed.

## Concrete Steps

All commands run from the repository root
`/data/leynos/Projects/rstest-bdd.worktrees/issue-381-step-macros-triggering-multiple-clippy-lints`.
Use `tee` for long outputs as required. If `get-project` is unavailable,
replace `$(get-project)` with `$(basename "$PWD")`.

1. Inspect wrapper assembly and argument preparation:

   rg -n "assemble_wrapper_function|render_wrapper_function" \
   crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly.rs
   rg -n "PreparedArgs" \
   crates/rstest-bdd-macros/src/codegen/wrapper/arguments.rs

1. Implement conditional lint list and thread it into wrapper emission.

1. Simplify the wrapper attribute test helper and update expectations.

1. Fix the documentation reference indentation.

1. Run quality gates:

   make fmt 2>&1 | tee /tmp/fmt-$(get-project)-$(git branch --show).out
   make markdownlint MDLINT=markdownlint-cli2 2>&1 | tee \
   /tmp/markdownlint-$(get-project)-$(git branch --show).out
   make nixie 2>&1 | tee /tmp/nixie-$(get-project)-$(git branch --show).out
   make check-fmt 2>&1 | tee /tmp/check-fmt-$(get-project)-$(git branch --show).out
   make lint 2>&1 | tee /tmp/lint-$(get-project)-$(git branch --show).out
   make test 2>&1 | tee /tmp/test-$(get-project)-$(git branch --show).out

1. Commit and push once all gates pass.

## Validation and Acceptance

Acceptance means:

- Wrapper functions only emit `#[expect(...)]` lints that can trigger for their
  specific structure, avoiding `unused_lint_expectations` warnings.
- The wrapper expect attribute test passes and asserts against the shared lint
  list and reason string.
- Documentation reference definitions remain valid after formatting.
- `make check-fmt`, `make lint`, and `make test` succeed.

## Idempotence and Recovery

All steps are safe to repeat. If any gate fails, fix the issue and re-run the
same command. If conditional lint selection is ambiguous, stop and request
clarification rather than broadening suppressions.

## Artifacts and Notes

Capture logs in `/tmp/*-$(get-project)-$(git branch --show).out` for each gate.

## Interfaces and Dependencies

No new dependencies or public API changes are expected. The only interface
change is the set of `#[expect(...)]` lints emitted on wrapper functions, which
must remain scoped and reasoned.

## Revision note (required when editing an ExecPlan)

Revision note (2026-01-18): Marked the plan complete, recorded the conditional
lint selection and test module split, and captured the final gate runs.
