# ExecPlan issue 397: close the stale policy-enum drift recommendation

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

`PLANS.md` is not present in this repository at the time of writing, so this
ExecPlan is the governing plan for issue 397.

## Purpose / big picture

PR [#397](https://github.com/leynos/rstest-bdd/pull/397) recommended adding
drift guards for `RuntimeMode` and `TestAttributeHint` because, at that time,
the runtime crate and the proc-macro crate each had their own copies of those
enums. That architectural risk has already been removed: the workspace now has
`crates/rstest-bdd-policy/src/lib.rs`, and both `rstest-bdd` and
`rstest-bdd-macros` source these policy types from that shared crate.

The remaining work is to close the stale recommendation correctly. That means
proving the shared crate is the only source of truth, updating outdated
documentation that still claims the macro crate keeps its own copies, and
adding a focused regression check that fails if either crate stops using the
shared policy types.

After this work:

- the repository no longer documents a manual synchronization requirement that
  no longer exists;
- the policy-crate architecture is explicitly recorded as the reason drift is
  prevented;
- small compile-time regression tests prove the runtime crate re-exports, and
  the macro crate imports, the exact shared policy types from
  `rstest-bdd-policy`;
- the quality gates pass, so issue 397 can be closed as resolved by
  architectural consolidation rather than by duplicate-enum synchronization
  tests.

Success is observable when a search for `pub enum RuntimeMode` and
`pub enum TestAttributeHint` finds only the definitions in
`crates/rstest-bdd-policy/src/lib.rs`, the updated docs describe the shared
crate accurately, and the regression tests fail if a duplicate enum is
reintroduced locally in either crate.

## Constraints

- Treat the current repository state as authoritative. Do not reintroduce
  duplicate enums just to satisfy the historical wording from PR #397.
- Keep the public API
  `rstest_bdd::execution::{RuntimeMode, TestAttributeHint}` stable.
- Keep the proc-macro crate independent of the runtime crate. Shared policy
  ownership must continue to live in `rstest-bdd-policy`.
- Scope this work to closing the stale drift-guard recommendation and updating
  the supporting documentation. Do not redesign the harness or attribute policy
  model.
- Keep any regression checks lightweight and local to the existing test suites.
  Do not add bespoke parsing scripts or a second validation framework unless
  source inspection proves the existing tests cannot cover the requirement.
- Update the documentation that currently describes the old duplicated-enum
  design, especially `docs/rstest-bdd-design.md` and
  `docs/adr-004-policy-crate.md`.
- Run all applicable gates with `set -o pipefail` and `tee`:
  `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`,
  and `make test`.

## Tolerances (exception triggers)

- Scope: if this closure requires changes to more than 8 files or 350 net
  lines, stop and re-check whether the work is drifting into a broader
  architecture update.
- Interfaces: if keeping the shared policy crate requires a public API change
  in `rstest-bdd`, `rstest-bdd-macros`, or `rstest-bdd-policy`, stop and
  escalate.
- Dependencies: if a new crate or external tool is needed, stop and escalate.
- Validation model: if the proposed regression check cannot be expressed as a
  normal Rust test in an existing crate, stop and document why before adding a
  custom script.
- Ambiguity: if issue 397, ADR-004, and the current source tree imply
  different intended outcomes, stop and present the competing interpretations.
- Iterations: if the same gate fails three consecutive fix attempts, stop and
  escalate with the captured log path.

## Risks

- Risk: the issue prompt reflects a past architecture and could push the
  implementation toward obsolete work. Severity: high. Likelihood: high.
  Mitigation: begin with an explicit inventory proving that the shared policy
  crate already owns the enums, and treat the stale recommendation as a
  documentation and regression-testing closure task.

- Risk: documentation drift could persist even if the code is already correct.
  Severity: medium. Likelihood: high. Mitigation: update the design doc and ADR
  in the same change that adds the regression tests, then verify the wording
  against the actual source paths.

- Risk: a weak regression test could only re-check enum behaviour without
  proving type origin, leaving room for future duplication. Severity: medium.
  Likelihood: medium. Mitigation: use compile-time type assertions that require
  `rstest_bdd::execution::RuntimeMode` and
  `crate::macros::scenarios::macro_args::RuntimeMode` to be the exact
  `rstest_bdd_policy::RuntimeMode` type, and do the same for
  `TestAttributeHint`.

- Risk: ADR-004 still says `Proposed`, which may conflict with the already
  implemented policy crate. Severity: medium. Likelihood: high. Mitigation:
  confirm whether the repository treats the ADR as accepted and, if so, update
  the ADR status in the same change.

## Progress

- [x] (2026-04-09 00:00Z) Reviewed the issue prompt, PR context, and existing
      ExecPlan conventions.
- [x] (2026-04-09 00:00Z) Verified the current source tree uses
      `rstest-bdd-policy` as the single source of truth for these enums.
- [x] (2026-04-09 00:00Z) Identified stale documentation in
      `docs/rstest-bdd-design.md` and `docs/adr-004-policy-crate.md`.
- [x] (2026-04-09 00:00Z) Drafted this ExecPlan.
- [ ] Stage A: capture baseline evidence and confirm the exact closure scope.
- [ ] Stage B: add focused regression tests for shared-type ownership.
- [ ] Stage C: update docs and ADRs to describe the implemented architecture.
- [ ] Stage D: run all required gates and record the results.

## Surprises & Discoveries

- Observation: the issue prompt cites
  `crates/rstest-bdd/src/execution.rs` and
  `crates/rstest-bdd-macros/src/macros/scenarios/macro_args.rs`, but the
  current repository uses `crates/rstest-bdd/src/execution/mod.rs` and
  `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/mod.rs`. Evidence:
  `leta files` shows the module directories and `mod.rs` files. Impact: the
  plan must target the current paths, not the historical ones.

- Observation: `crates/rstest-bdd/src/execution/mod.rs` now re-exports
  `RuntimeMode` and `TestAttributeHint` from `rstest_bdd_policy`, and
  `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/mod.rs` imports
  them from the same crate. Evidence:
  `pub use rstest_bdd_policy::RuntimeMode;`,
  `pub use rstest_bdd_policy::TestAttributeHint;`, and
  `pub(crate) use rstest_bdd_policy::{RuntimeMode, TestAttributeHint};`.
  Impact: issue 397 is no longer a duplicate-enum problem.

- Observation: `docs/rstest-bdd-design.md` section 2.6.2 still says the macro
  crate maintains its own copies of the enums. Evidence: the note under the
  section says manual duplication remains because proc-macro crates cannot
  depend on runtime crates. Impact: the documentation is now misleading and
  must be corrected before the issue can be considered closed cleanly.

- Observation: `docs/adr-004-policy-crate.md` still has status `Proposed`
  while the shared crate already exists and is wired into both crates.
  Evidence: the ADR file and the presence of `crates/rstest-bdd-policy`.
  Impact: the ADR should likely move to `Accepted` or another repository-valid
  terminal status as part of this closure.

## Decision Log

- Decision: treat issue 397 as a stale architectural review item that should be
  resolved by documenting the already-implemented policy crate and guarding
  against regression, not by adding synchronization tests for enums that are no
  longer duplicated. Rationale: adding drift checks between crates would solve
  a problem the repository no longer has and would dilute the value of the
  shared-crate architecture. Date/Author: 2026-04-09 / Codex.

- Decision: prefer compile-time type-origin tests over variant-count or
  behavioural parity tests. Rationale: behaviour tests already exist in
  `rstest-bdd-policy`, `rstest-bdd`, and `rstest-bdd-macros`; what issue 397
  still needs is proof that both crates continue to use the same shared types.
  Date/Author: 2026-04-09 / Codex.

- Decision: update the design document and ADR alongside the regression tests.
  Rationale: the current gap is partly documentary, and the issue should not be
  closed while the docs still describe the obsolete manual-sync architecture.
  Date/Author: 2026-04-09 / Codex.

## Outcomes & Retrospective

Pending. The intended outcome is not a new synchronization mechanism between
duplicate enums. The intended outcome is a documented and test-backed
confirmation that duplication has already been eliminated and will be caught if
it returns.

## Context and orientation

The current single source of truth is `crates/rstest-bdd-policy/src/lib.rs`. It
defines `RuntimeMode`, `TestAttributeHint`, their helper methods, and the
canonical attribute-policy path mapping.

The runtime crate uses those shared types via re-export in
`crates/rstest-bdd/src/execution/mod.rs`. Downstream users still import
`rstest_bdd::execution::RuntimeMode` and
`rstest_bdd::execution::TestAttributeHint`, so the public API remains stable.

The proc-macro crate uses those shared types in
`crates/rstest-bdd-macros/src/macros/scenarios/macro_args/mod.rs`, where they
are brought into the parser module with
`pub(crate) use rstest_bdd_policy::{RuntimeMode, TestAttributeHint};`.

Existing behaviour coverage already exists:

- `crates/rstest-bdd-policy/src/lib.rs` tests the canonical enum behaviour.
- `crates/rstest-bdd/src/execution/tests.rs` tests the runtime-facing re-export
  behaviour and mapping.
- `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/tests.rs` tests
  macro parsing and `RuntimeMode::test_attribute_hint()`.

What is missing is a direct statement, in code and in docs, that the shared
crate is the architectural guard against drift. Two docs still describe the old
state:

- `docs/rstest-bdd-design.md` section 2.6.2
- `docs/adr-004-policy-crate.md`

This plan closes issue 397 by aligning those files with the implemented
architecture and adding regression tests that prove both crates still use the
shared policy types.

## Plan of work

### Stage A: baseline inventory and scope confirmation

Goal: prove the issue is already architecturally fixed, and identify the
smallest remaining closure work.

Implementation details:

- Inventory enum definitions with a repository search for
  `pub enum RuntimeMode` and `pub enum TestAttributeHint`.
- Confirm that the only public definitions are in
  `crates/rstest-bdd-policy/src/lib.rs`.
- Confirm the runtime crate re-exports the policy types from
  `crates/rstest-bdd/src/execution/mod.rs`.
- Confirm the proc-macro crate imports the policy types from
  `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/mod.rs`.
- Record the outdated documentation language that still describes duplication.

Go/no-go validation:

- The inventory proves there is only one owning enum definition for each type.
- The closure task can be restated as regression coverage plus documentation
  correction.

### Stage B: add regression tests for shared ownership

Goal: make the single-source-of-truth architecture fail fast if someone
reintroduces local copies later.

Implementation details:

- Extend `crates/rstest-bdd/src/execution/tests.rs` with tests that require
  `rstest_bdd::execution::RuntimeMode` and
  `rstest_bdd::execution::TestAttributeHint` to type-check as
  `rstest_bdd_policy::RuntimeMode` and `rstest_bdd_policy::TestAttributeHint`.
- Extend
  `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/tests.rs` with
  equivalent tests for the macro-facing imports.
- Keep these as compile-time identity checks with a small behavioural assertion
  only where needed to keep the test readable.
- Do not add a grep-based CI script or doc-test workaround unless the normal
  Rust tests prove insufficient.

Go/no-go validation:

- If either crate stops using the shared policy types, the new tests fail to
  compile or fail at test time with a clear message.

### Stage C: update the supporting documentation

Goal: remove the obsolete manual-sync narrative and replace it with the shared
policy-crate architecture.

Implementation details:

- Update `docs/rstest-bdd-design.md` section 2.6.2 so it states that
  `RuntimeMode` and `TestAttributeHint` live in `rstest-bdd-policy`, with the
  runtime crate re-exporting them and the macro crate importing them directly.
- Update `docs/adr-004-policy-crate.md` so the status reflects the current
  implemented state, and so the ADR explicitly records that this architecture
  resolves the drift risk that issue 397 originally identified.
- If the implementation touches code comments, keep them limited to files that
  directly expose the policy ownership boundary:
  `crates/rstest-bdd/src/execution/mod.rs` and
  `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/mod.rs`.

Go/no-go validation:

- The docs no longer claim the macro crate keeps local copies of the enums.
- The ADR and design doc describe the same ownership model.

### Stage D: validation and closure evidence

Goal: prove the small closure change is correct and safe.

Implementation details:

- Run targeted evidence commands first to show the single-source inventory and
  the focused test coverage.
- Then run the full required repository gates for a docs-and-tests change:
  `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`,
  and `make test`.
- Capture all command output with `set -o pipefail` and `tee`.

Go/no-go validation:

- Inventory output shows only the policy crate owns the enum definitions.
- The new regression tests pass.
- All required gates pass before the issue is marked complete.

## Concrete steps

Run all commands from the repository root, `/home/user/project`.

1. Baseline inventory:

   ```bash
   set -o pipefail; rg -n "pub enum (RuntimeMode|TestAttributeHint)" crates \
     2>&1 | tee /tmp/issue-397-enum-inventory.log
   ```

   Expected result:

   ```plaintext
   crates/rstest-bdd-policy/src/lib.rs:<line>:pub enum RuntimeMode {
   crates/rstest-bdd-policy/src/lib.rs:<line>:pub enum TestAttributeHint {
   ```

2. Focused policy and ownership tests after implementation:

   ```bash
   set -o pipefail; cargo test -p rstest-bdd-policy 2>&1 | \
     tee /tmp/issue-397-policy-tests.log
   set -o pipefail; cargo test -p rstest-bdd --lib execution::tests 2>&1 | \
     tee /tmp/issue-397-runtime-tests.log
   set -o pipefail; cargo test -p rstest-bdd-macros --lib \
     macros::scenarios::macro_args::tests 2>&1 | \
     tee /tmp/issue-397-macro-tests.log
   ```

   Expected result:

   ```plaintext
   test result: ok. <N> passed; 0 failed
   ```

3. Full repository gates:

   ```bash
   set -o pipefail; make fmt 2>&1 | tee /tmp/issue-397-make-fmt.log
   set -o pipefail; make markdownlint 2>&1 | \
     tee /tmp/issue-397-make-markdownlint.log
   set -o pipefail; make nixie 2>&1 | tee /tmp/issue-397-make-nixie.log
   set -o pipefail; make check-fmt 2>&1 | \
     tee /tmp/issue-397-make-check-fmt.log
   set -o pipefail; make lint 2>&1 | tee /tmp/issue-397-make-lint.log
   set -o pipefail; make test 2>&1 | tee /tmp/issue-397-make-test.log
   ```

   Expected result:

   ```plaintext
   make fmt
   ...
   make markdownlint
   ...
   make nixie
   ...
   make check-fmt
   ...
   make lint
   ...
   make test
   ...
   ```

## Validation and acceptance

Acceptance means all of the following are true:

- Repository search shows `RuntimeMode` and `TestAttributeHint` are defined
  only in `crates/rstest-bdd-policy/src/lib.rs`.
- `crates/rstest-bdd/src/execution/tests.rs` contains regression coverage that
  proves the runtime-facing types are re-exports of the policy-crate types.
- `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/tests.rs`
  contains regression coverage that proves the macro-facing types come from the
  same policy crate.
- `docs/rstest-bdd-design.md` no longer claims the macro crate maintains local
  copies of these enums.
- `docs/adr-004-policy-crate.md` records the policy crate as the accepted
  architectural answer to the old drift risk.
- `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
  `make lint`, and `make test` all succeed.

Quality method:

- Use the baseline inventory command to prove single ownership.
- Use the focused crate tests to prove shared-type identity.
- Use the full Makefile gates to prove the repository remains healthy.

## Idempotence and recovery

The inventory commands and test commands are safe to re-run. If a focused test
fails, inspect the corresponding log in `/tmp/issue-397-*.log`, fix the issue,
and rerun only the failed step before rerunning the full gates. Do not mark the
issue complete until the docs and tests agree on the shared policy-crate
architecture.

## Artifacts and notes

Expected logs:

```plaintext
/tmp/issue-397-enum-inventory.log
/tmp/issue-397-policy-tests.log
/tmp/issue-397-runtime-tests.log
/tmp/issue-397-macro-tests.log
/tmp/issue-397-make-fmt.log
/tmp/issue-397-make-markdownlint.log
/tmp/issue-397-make-nixie.log
/tmp/issue-397-make-check-fmt.log
/tmp/issue-397-make-lint.log
/tmp/issue-397-make-test.log
```

Useful source anchors while implementing:

- `crates/rstest-bdd-policy/src/lib.rs`
- `crates/rstest-bdd/src/execution/mod.rs`
- `crates/rstest-bdd/src/execution/tests.rs`
- `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/mod.rs`
- `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/tests.rs`
- `docs/rstest-bdd-design.md`
- `docs/adr-004-policy-crate.md`

## Interfaces and dependencies

No new external dependencies should be introduced.

The shared policy owner must remain:

```rust
pub enum RuntimeMode {
    Sync,
    TokioCurrentThread,
}

pub enum TestAttributeHint {
    RstestOnly,
    RstestWithTokioCurrentThread,
    RstestWithGpuiTest,
}
```

The required architectural boundaries at the end of the work are:

- `crates/rstest-bdd-policy/src/lib.rs` owns the enum definitions and helper
  methods.
- `crates/rstest-bdd/src/execution/mod.rs` re-exports the shared types for the
  public runtime API.
- `crates/rstest-bdd-macros/src/macros/scenarios/macro_args/mod.rs` imports
  the shared types for macro parsing and code generation.
- Tests in the runtime and macro crates compile only if those boundaries
  remain true.

Revision note: Initial draft created on 2026-04-09. The plan reframes issue 397
around the current repository state, because ADR-004 and the
`rstest-bdd-policy` crate have already eliminated the original duplicate-enum
risk. The remaining work is to add regression tests and update stale
documentation so the issue can be closed accurately.
