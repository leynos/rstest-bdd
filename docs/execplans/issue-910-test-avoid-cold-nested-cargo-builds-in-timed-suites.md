# ExecPlan issue 910: keep cold nested Cargo builds out of timed test bodies

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE — implementation finished and CI timing evidence recorded; see
Progress for the cold and warm runs.

`PLANS.md` is not present in this repository at the time of writing, so this
ExecPlan is the governing plan for issue 910.

## Purpose / big picture

Several integration tests drive a real `cargo` invocation against a fixture
crate. That is the point of them: they prove what the framework generates, what
a diagnostic says, or what a rebuild recompiles. What they do not assert on is
compiling the fixture's dependency closure, and yet the first run against a
cold or evicted target directory does exactly that inside the test body, so the
test's own deadline pays for it. On a busy machine with a cold compiler cache
the work routinely outlasts the deadline, and issue 910 records three tests
failing that way while passing in under a second once the same build was paid
for in advance.

After this work:

- a clean or evicted Cargo target cache no longer makes a nested-fixture test
  compile its dependency graph inside a per-test deadline;
- each affected suite has a named preparation test that compiles the
  prerequisite graphs, with a budget sized for that job and separate from every
  assertion deadline;
- the timed tests keep their existing budgets and still prove the rebuild,
  diagnostic and CLI behaviour they were written for;
- the preparation is idempotent and self-invalidating: a changed toolchain,
  feature selection or fixture source forces it to run again, and evicting the
  artefact directory evicts the record of preparedness with it.

Success is observable when, on a clean checkout with an empty target directory,
the full suite passes without any assertion spending its deadline on a cold
dependency compile, and when `make prepare-nested-fixtures` followed by a
filtered `cargo nextest run` passes the timed tests in seconds.

## Constraints

- Do not widen an assertion deadline. Raising a `slow-timeout` to accommodate
  a cold dependency build hides the problem rather than moving it, and issue
  910's acceptance criteria say so explicitly.
- Keep the four-tier timeout ordering intact: per-test `slow-timeout` below
  the run-level `global-timeout` below the CI watchdog below the job
  `timeout-minutes`. Any new budget must fit inside the tiers above it.
- The preparation must run under both `cargo nextest` and plain `cargo test`.
  The mutation lane (`cargo-mutants`) and a developer's focused run use the
  latter, and neither can be asked to run a separate nextest invocation first.
- Preparation must reuse the code path the timed test uses. A preparation
  that took a shortcut would warm a different build graph than the assertion
  needs, which is the failure mode this work exists to avoid.
- No single code file may exceed 400 lines (AGENTS.md). Split a module rather
  than growing one past the limit.
- Compiler admission is shared and must not be bypassed: do not unset or
  override `RUSTC_WRAPPER` or `RUSTDOC`, and do not invoke `rustc` directly.

## Tolerances (exception triggers)

- Scope: if the work needs changes to more than the four affected test suites,
  the harness preparation module, `.config/nextest.toml` and the Makefile, stop
  and record why before continuing.
- Verification: if a preparation test cannot be made to run before the timed
  tests under nextest, stop. A preparation that runs after the assertion is no
  better than none.
- Budget: if a single preparation test exceeds its 30-minute budget on a cold
  two-vCPU runner, that is evidence the budget or the family split is wrong;
  record the measurement and revisit rather than raising the number.
- Platform: if the Windows under-nextest deadlock documented for trybuild
  reappears in a new suite, skip that suite's preparation on that platform
  combination as `step_macros_compile` does, rather than serializing
  differently.

## Risks

- **Nextest override precedence.** `priority` is scheduling, not precedence.
  Overrides are applied first-match-in-file-order, so a preparation override
  placed after a per-binary override that also matches would silently inherit
  the timed assertion's budget. The preparation override must stay first.
  Pinned by `preparation_nextest_override_preserves_timeout_contract`.
- **Stale stamps.** A stamp that survives a change it should not is worse than
  no stamp, because it makes a stale tree look prepared. The fingerprint covers
  the build-relevant environment, the toolchain identity, a variant label and a
  fixture-input hash, and the stamp lives inside the directory it describes.
- **`--target-dir` blind spot.** `macrotest_support::trybuild_target_root` runs
  `cargo metadata`, which sees `CARGO_TARGET_DIR` from the environment but
  cannot see an outer `--target-dir` flag. `trybuild_crate_root` only appends
  `target_subdir` to that root. A lane that passes the flag directly, rather
  than through the environment, prepares a different tree than it builds in.
- **Runner divergence.** A preparation that passes under nextest can still be
  exercised only lazily under `cargo test`, so both paths need coverage.

## Progress

- [x] Add the shared preparation policy to `rstest-bdd-harness`: stamp
      protocol, environment fingerprint, toolchain identity, input hashing,
      lazy path under plain `cargo test`, fail-fast path under nextest.
- [x] Prepare the `cargo-bdd` CLI fixture before the timed smoke tests and
      gate those tests on the stamp.
- [x] Prepare the rebuild-invalidation and feature-addition scratch fixtures
      before the timed rebuild experiments.
- [x] Prepare the trybuild tree and the UI-lint Clippy tree before
      `step_macros_compile`, and gate both timed paths on the stamps.
- [x] Make the dep-info tracking assertion compare against the start of its
      own trybuild run, so a prepared tree cannot satisfy it with an earlier
      run's artefacts.
- [x] Add the first-position nextest override with its own 30-minute budget
      and highest scheduling priority, plus the top-level `nextest-version`
      that makes `priority` readable.
- [x] Pin the override contract in `nextest_config.rs` and keep the existing
      timed overrides at their exact periods.
- [x] Add `make prepare-nested-fixtures` for a filtered nextest run that would
      otherwise skip the preparation tests.
- [x] Prepare the Tokio and GPUI harness trybuild trees and the server crate's
      `workspace_discovery_compile`, all through one shared `TrybuildTree`
      helper in `macrotest_support`.
- [x] Record a decision on the mutation lane, which runs `cargo test` and so
      takes the lazy path.
- [x] Update `docs/developers-guide.md`'s suite table with the harness and
      server trybuild tests.
- [x] Collect single-host timing evidence on a cold nested target, and record
      the per-family and per-assertion figures in Stage F, with the caveat
      that this host queues on a shared admission pool CI does not have.
- [x] Fix the self-invalidating stamp: `trybuild` writes `wip/` inside the
      `crates` input root, which made every stamp stale as it was written.
- [x] Clear the `clippy::unnecessary_wraps` diagnostics the shared contract
      produces for the two suites whose preparation has no fallible step, and
      the `cloned_ref_to_slice_refs` pair in the preparation tests.
- [x] Correct the budget rationale's misattributed figure: the comments cited
      386.8 s as the largest cold preparation, but that is the old *timed*
      `step_macros_compile` run on CI with a partial sccache. The recorded cold
      preparations are 864.239 s (server trybuild tree) and 645.412 s (the
      `rstest-bdd` trybuild binary's two graphs), both single-host figures
      behind the shared admission pool.
- [x] Address the CodeRabbit review's seven findings. Two were behavioural: a
      doc/code contradiction in `was_written_since` (an unreadable timestamp
      was treated as stale, not fresh as documented), and three preparation
      families whose stamps did not hash the workspace crates their fixtures
      take as path dependencies. The rest corrected prose: the `#[serial]`
      cross-process claim, the "parallel" and "quarter budget" claims about a
      serial cold run, and a focused-run recipe that mixed `RUSTFLAGS`.
- [x] Answer the CodeScene advisory findings on the diff. Seven files were
      flagged; four were regressions this branch introduced (10.00 → 9.19,
      9.39, 9.53 and 9.59) and three were pre-existing. CodeScene is not a
      required check — the `main-required-checks` ruleset names only the three
      `build-test` lanes — but `clippy.toml` states that this repository's
      thresholds follow CodeScene's ceiling, so a regression there is a real
      one. The three Large Method findings shared a cause: each harness step
      gained an early return whose whole struct literal was failing
      placeholders, and four such literals already existed. Naming that value
      once (`DepInfoOutcome::abandoned`, `RebuildOutcome::abandoned`,
      `AdditionOutcome::abandoned`) removed the repetition. The duplication
      findings were test setup: `fixture_package` and `scratch_preparation`.
      Three findings survived that pass, and the duplication ones were only
      resolved once they were measured rather than guessed at. The
      `trybuild_environment` consolidation was real and worth keeping — it
      removed two copies of "take this process's environment, drop
      `CARGO_TARGET_DIR`, and put the family's own directory in its place" —
      but it was not what CodeScene was reporting. `cs review` named the
      duplicated functions directly: `assert_snapshot_contains` and
      `assert_snapshot_omits`, untouched by this branch, whose identical
      read-assert shapes tipped the file below the gate once the file grew.
      Diverging them structurally (collecting every missing needle, which also
      improves the message) took the file from 9.38 to 10.00. The
      `feature_rebuild_invalidation` duplication was the real pair
      `rebuild_preparation`/`addition_preparation`, now one
      `fixture_preparation`, taking that file to 10.00. The third finding was
      `Preparation::new`'s five-argument signature — and calling it
      pre-existing was wrong: `Preparation` does not exist on `main` at all,
      so the whole module is this branch's. It was fixed rather than
      reclassified. The three values that travel together (family, variant,
      input hash) are now a `Sources` value object, so `new` takes three
      arguments and a caller cannot pair one family's name with another's
      hash. All three previously flagged files now score 10.00 locally, and
      CodeScene agrees on the pushed commit `8c97c2f7`: the check that had
      failed on every earlier head reports `completed/success`, "Quality Gate
      Passed", 6 gates, no findings.
- **"Pre-existing" is a claim to verify, not a safe default.** Both the
  execplan and a commit message described the `Preparation::new` argument count
  as pre-existing at 9.69 on `main`.
  `git grep "struct Preparation" origin/main` returns nothing: the module is
  new here. The claim survived because it was phrased plausibly and never
  checked against `main` — the same mistake, in a different guise, as the three
  wrong duplication hypotheses. Check the base revision before calling a
  finding inherited.
- [x] Keep the `Lint` gate working against a rolling suite. `--lib
      whitaker_suite` fixed the duplicate-registration panic, and the suite's
      new `string_continuation_style` lint then required 24 `concat!()`
      rewrites in 12 files. None of those files is part of issue 910's
      change; the lint arrived through the rolling release and would have
      broken `main` on its next run regardless of this branch.
- [x] Record CI-lane timings for the affected suites once the PR runs, since
      only that lane answers acceptance criterion (c). Run `41981885921`
      (head `bca39c7d`) passed all three required lanes; the figures below are
      from the Linux coverage step, which ran the whole suite in 1402.014 s —
      2101 tests across 133 binaries, 2101 passed, 7 skipped.

The six preparation families did the cold work inside their own 30-minute
budget:

| Test                                                                                     | CI time   | Limit |
| ---------------------------------------------------------------------------------------- | --------- | ----- |
| `cargo-bdd::cli preparation::nested_fixture_preparation`                                 | 89.382 s  | 30 m  |
| `rstest-bdd::feature_rebuild_invalidation nested_fixture_preparation`                    | 31.154 s  | 30 m  |
| `rstest-bdd::trybuild_macros preparation::nested_fixture_preparation`                    | 438.214 s | 30 m  |
| `rstest-bdd-harness-gpui::macro_compile preparation::nested_fixture_preparation`         | 332.096 s | 30 m  |
| `rstest-bdd-harness-tokio::macro_compile preparation::nested_fixture_preparation`        | 43.881 s  | 30 m  |
| `rstest-bdd-server::workspace_discovery_compile preparation::nested_fixture_preparation` | 239.213 s | 30 m  |
| `cargo-bdd::cli list_steps_runs`                                                         | 0.584 s   | 180 s |
| `rstest-bdd::feature_rebuild_invalidation feature_addition_triggers_rebuild`             | 1.745 s   | 600 s |
| `rstest-bdd::trybuild_macros runner::step_macros_compile`                                | 197.642 s | 20 m  |

The three tests the issue names as failures now run at 0.3%, 0.3% and 16.5% of
their deadlines, with the cold graph compiled by a separate test under a budget
the assertion does not share.

Run `42005188689` on the final head `851c9b1c` passed all three required lanes
and is the second, independent measurement. Its suite finished in 348.345 s
(2101 tests, 2101 passed, 7 skipped) — 4.0× faster than the figure above. That
difference is not the branch improving something: **the two runs differ in
compiler-cache state, and the timings must not be compared without saying so.**
`sccache` reported a 13.80% Rust hit rate on run `41981885921` and 95.71% on run
`42005188689`, so the first is the cold-cache case that acceptance criterion
(a) is about and the second is a warm runner. The cold measurement stays the
evidence for that criterion; the warm one shows a normal CI day costs minutes,
not half an hour:

| Test                                                                                     | cold (`41981885921`) | warm (`42005188689`) |
| ---------------------------------------------------------------------------------------- | -------------------- | -------------------- |
| `cargo-bdd::cli preparation::nested_fixture_preparation`                                 | 89.382 s             | 20.742 s             |
| `rstest-bdd::feature_rebuild_invalidation nested_fixture_preparation`                    | 31.154 s             | 10.535 s             |
| `rstest-bdd::trybuild_macros preparation::nested_fixture_preparation`                    | 438.214 s            | 100.781 s            |
| `rstest-bdd-harness-gpui::macro_compile preparation::nested_fixture_preparation`         | 332.096 s            | 68.909 s             |
| `rstest-bdd-harness-tokio::macro_compile preparation::nested_fixture_preparation`        | 43.881 s             | 11.955 s             |
| `rstest-bdd-server::workspace_discovery_compile preparation::nested_fixture_preparation` | 239.213 s            | 41.501 s             |
| `cargo-bdd::cli list_steps_runs`                                                         | 0.584 s              | 0.564 s              |
| `rstest-bdd::feature_rebuild_invalidation feature_addition_triggers_rebuild`             | 1.745 s              | 1.597 s              |
| `rstest-bdd::trybuild_macros runner::step_macros_compile`                                | 197.642 s            | 65.744 s             |
| whole suite                                                                              | 1402.014 s           | 348.345 s            |

Run
[`42013457153`](https://github.com/leynos/rstest-bdd/actions/runs/42013457153)
on head `5b872477` is the third measurement and the first on the final code. It
passed all three required lanes — Linux in 30 m 44 s, both Windows lanes green
— with a 95.71% Rust hit rate, so it is a warm run like `42005188689`. Its
suite finished in 323.344 s across **2102** tests (2102 passed, 7 skipped): one
more than the earlier runs, which is the new
`preparation_command_bound_fires_before_the_runner_kills_the_test` contract
test. The three issue-named assertions ran in 0.331 s, 1.783 s and 63.263 s
against 180 s, 600 s and 20 m, and the largest preparation took 97.046 s —
13.5% of its 30-minute budget, and 18.0% of the 9-minute per-command bound that
sits inside it. This is the run that shows the corrected bound changes no
outcome: the suite is the same speed as the previous warm run within noise.

The largest cold preparation used 24.3% of its 30-minute budget and the three
issue-named assertions used 0.3%, 0.3% and 16.5% of theirs. Every timed
override is byte-identical to `main`'s — 180 s, 20 m, 600 s, and the 60 s
default — which `git show origin/main:.config/nextest.toml` confirms directly
rather than by reading this branch's comments.

- [x] Push the final head `851c9b1c` and confirm it against the remote. The two
      commits it carries are execplan-only: the CodeScene CLI measurement note
      and the record of check run `127148421451` passing. They were held back
      until the in-flight rerun of `41998761582` finished, because
      `cancel-in-progress: true` means a push starts a fresh run and cancels the
      one already in progress. The push went over the explicit SSH URL with
      `GIT_CONFIG_*`, `GH_TOKEN`, `GITHUB_TOKEN`, `BASH_ENV`, `GIT_SSH` and
      `GIT_SSH_COMMAND` stripped, then the tracking ref was refreshed with a
      leading-`+` refspec and `ls-remote` returned the same
      `851c9b1caf8eaf92d9aed46365b305e99701dd20` as `git rev-parse HEAD`. The
      resulting CI run is `42005188689`.
- [x] Re-run `coderabbit review --agent` after the refactoring milestone. It
      completed over 33 changed files with zero findings. It was requested only
      after the deterministic gates it can be checked against had passed:
      `make check-fmt`, `make markdownlint` and `make lint`, plus
      `rstest-bdd-harness`'s 51 unit tests and the Tokio, GPUI and server test
      binaries. The full `make test` and `make test-workflow-contracts` had
      already passed at `bca39c7d` and again on CI; the later commits touch one
      private function body and prose, so the targeted suites are the
      relevant evidence rather than a rerun of the whole workspace.

- [x] Address the final `coderabbit review --agent` pass. It reported two
      minor findings. The substantive one was a real ordering defect:
      `PREPARATION_COMMAND_TIMEOUT` was 45 minutes while `.config/nextest.toml`
      limits `nested_fixture_preparation` to 30, so under nextest the runner
      always killed the test before the per-command bound could fire, and the
      named error carrying the child's captured output was unreachable. The
      bound became 12 minutes at this stage, on a count of two sequential
      commands, which totalled 24 minutes inside the 30-minute period. The
      first replacement was 15 minutes, which the new contract test rejected:
      `2 × 900` is exactly the period, and the assertion requires strictly
      less. That is the test doing its job on a plausible but wrong value.
      Twelve minutes still exceeds the largest cold preparation *test*
      measured on CI (438.214 s), so every command inside one is covered — a
      command cannot outlast the test that runs it. The count of two was wrong
      and the bound moved to 9 minutes in the next entry; the 438.214 s figure
      is 81.2% of that. The misleading `nextest.toml`
      comment now says which path reports which overrun, and
      `preparation_command_bound_fires_before_the_runner_kills_the_test` pins
      the ordering so it cannot drift again. The other finding was stale
      status text in this plan, corrected at all four sites.
- [x] Commit the fix as `5b872477` and push it. The commit also reflows the
      table added in `716d1985` through `mdtablefix`, repairing that commit's
      CI "Check formatting" failure. `make check-fmt` and `make lint` both
      passed on the working tree before the commit, and the remote head was
      confirmed equal to `git rev-parse HEAD` after the push. `716d1985` is
      worth remembering as a self-inflicted failure: `make markdownlint` does
      not run `mdtablefix`, so a table can pass the Markdown gate and still
      fail `check-fmt`, which is the gate that owns table reflow.
- [x] Confirm CI run `42013457153` on head `5b872477` passes all three required
      lanes: `build-test (linux, default features)` in 30 m 44 s, and both
      Windows lanes. This is the first green run on the corrected bound, and it
      is the gate the merge ruleset actually requires. Its suite ran 2102
      tests, one more than the earlier runs because of the new contract test.
- [x] Address the CodeRabbit pass on the accumulated diff. It reported two
      findings, and both were valid. The first is the more serious: the
      contract test pinned `sequential_commands` at 2, but
      `feature_rebuild_invalidation`'s `prepare_all` runs **three** bounded
      commands — `prepare_rebuild` compiles once, then `prepare_addition` runs
      a `cargo metadata` preflight and a compile. The earlier reading counted
      the two families rather than the commands inside them, so `3 × 12 m =
      36 m` exceeded the 30 m period and a late third command would have lost
      the named error. The bound is now 9 minutes and the count is a named
      constant, `MAX_SEQUENTIAL_COMMANDS = 3`, which the contract test reads
      rather than repeating a literal: `3 × 9 m = 27 m`.
      The cold run shows how much headroom that leaves — the whole
      three-command preparation took 31.154 s.
      The preflight is counted rather than exempted. It is a real
      `cargo` invocation that can wedge on the package-cache lock, and
      CodeRabbit's own second finding is that an unbounded child hangs the run;
      moving the preflight off the bounded path would have created the hazard
      the bound exists to prevent.
      The second finding is also correct: `prepare_ui_clippy` used
      `Command::output()`, which has no wall-clock bound, while
      `.config/nextest.toml` claimed every preparation command was bounded. It
      now goes through `run_bounded_with_timeout` with the same constant as
      every other command.
      The trybuild half of that family cannot be bounded the same way —
      `trybuild` spawns its own `cargo` and offers no hook to control it — so
      the `nextest.toml` comment and the developers guide now name that one
      exception instead of implying universal coverage.
      Gates after both fixes: `make fmt`, `make check-fmt`, `make lint`,
      `make markdownlint`, `make test-workflow-contracts` (364) and
      `nextest_config` (5) all pass.
- [x] Commit the fixes as `7d6bbd5a` and confirm CI run `42021251818` on that
      head passes all three required lanes: both Windows lanes and
      `build-test (linux, default features)`, conclusion `success`. This is the
      timing evidence on the final bound. The Linux lane ran 32 m 09 s against
      `42013457153`'s 30 m 44 s, and its suite ran 332.981 s across the same
      2102 tests (2102 passed, 7 skipped) against 323.344 s — the same work
      within run-to-run noise, which is what a change to an internal deadline
      no run approaches should look like. The six preparations on that lane
      took 20.210 s, 9.768 s, 94.240 s, 59.415 s, 11.658 s and 36.875 s, each
      of which is 17.5% or less of the 540 s per-command bound and therefore
      far inside the 27 m the three sequential commands may take. The three
      issue-named assertions ran in 0.593 s, 1.864 s and 70.731 s. This run is
      the timing evidence on the final source change; the cold run's
      1402.014 s remains the evidence for criterion (a).
- [x] Commit the evidence write-ups as `3034760b` and `434d8309`. Both are
      documentation-only, and the later one narrows the earlier's claim that
      `42021251818` was the final-head evidence: `3034760b` and `434d8309`
      change prose after it, so that run measured near-identical but not
      byte-identical source. `42027006140`, the attempt on `3034760b`, was
      cancelled by the push that superseded it, which is the repository's
      `cancel-in-progress: true` behaving as configured rather than a failure.
- [x] Confirm CI run `42028526380` on the true final head `434d8309` passes all
      three required lanes: both Windows lanes and
      `build-test (linux, default features)`, conclusion `success`. Its suite
      ran 360.664 s across the same 2102 tests (2102 passed, 7 skipped). The
      six preparations took 17.054 s, 10.693 s, 100.523 s, 65.171 s, 8.918 s
      and 38.427 s; the largest is 18.6% of the 540 s per-command bound. The
      three issue-named assertions ran in 0.614 s, 2.127 s and 81.341 s, which
      is 0.3%, 0.4% and 6.8% of their 180 s, 600 s and 20-minute deadlines.
      The lane took 36 m 08 s against `42021251818`'s 32 m 09 s, which reads
      like a regression until the two are decomposed. The nextest summary
      occupies about a sixth of the lane in every run — 17.5% at
      `42013457153`, 17.3% at `42021251818`, 16.6% here — so roughly 30 of the
      36 minutes happen before and after it, in image setup, the Cargo build
      and coverage handling. Those phases absorbed the 3.5-minute difference
      (26.6 minutes outside the suite before, 30.1 now) while the suite itself
      grew 8.3%, from 332.981 s to 360.664 s, on code this diff does not touch.
      Neither figure is evidence about a Markdown-only change; the point of
      recording both is that the lane figure alone would have suggested one.
- [x] Correct the plan's final-head claim and record the head that actually
      ships. `434d8309` was itself superseded by `df26170d`, the commit that
      made this correction, so the plan had the same defect one level down:
      naming a head "final" while more commits followed it. The distinction
      that survives is between the head whose *source* the timing evidence
      describes (`434d8309`, and `42021251818` before it) and the head CI last
      validated (whatever is on the branch now). Run `42048627869` on
      `df26170d` and run `42057352055` on `3ffa7295` both pass all three
      required lanes with conclusion `success`, which is the merge-gate
      evidence for the branch as pushed.
- [x] Address the follow-up `coderabbit review --agent` pass, which reported
      two documentation findings and no code findings. Both were valid. Four
      passages still described the superseded 12-minute bound, and two of them
      divided measured times by it: 97.046 s is 18.0% of 9 minutes, not 13.5%
      of 12, and 438.214 s is 81.2% of 9 minutes (540 s), not 60.9%. The second
      figure is the one worth keeping visible: 81.2% is not a comfortable
      margin, so the text now says the bound still clears the largest cold
      preparation without claiming a wide gap. The other corrections were an
      attribution error — `cargo metadata` runs in
      `macrotest_support::trybuild_target_root`, while `trybuild_crate_root`
      only appends `target_subdir` — and an incomplete `nested_cargo` export
      list, missing `MAX_SEQUENTIAL_COMMANDS`, `Sources` and
      `ProfileDestination`. The same pass flagged first-person pronouns in
      this plan, which the repository's Markdown path instructions forbid;
      those are now impersonal.
- [x] Close two stamp-validity gaps that still allowed cold compilation inside
      a timed assertion. (Both are closed; see the later entries for the third,
      fourth and fifth gaps review found in the same area.)
      1. **The stamp did not always share the artefacts' directory.**
         `fixture_preparation` stamped the scratch root under the workspace
         `target/`, but `process::child_env_from_vars` preserves an inherited
         `CARGO_TARGET_DIR`, so a nested build can put its artefacts somewhere
         else entirely. Prepare with an external target, evict that directory,
         run again: the scratch stamp survived, its fingerprint still matched,
         and both the preparation test and the assertion gate accepted it while
         the dependency artefacts were gone. The child environment now resolves
         its target directory once (`process::child_target_directory`), the
         preparation view fingerprints that same value, and the stamp is
         written under it. Under the default target it coincides with the old
         location, so warm trees stay warm; under an inherited one it moves
         with the artefacts. The scratch `.stamp` in `fixtures` stays separate:
         it records which fixture source the copy came from, which is a
         different question from which artefacts exist.
      2. **Cargo configuration was not a preparation input.** All four input
         lists hashed the workspace manifest, lockfile and `crates/` but not
         `.cargo/config.toml`, which sets the development profile options and
         the Linux linker flags. Cargo discovers that file by walking up from
         the nested build's working directory, so editing it rebuilds
         dependency artefacts without touching a fixture source — and a stamp
         that ignored it would skip preparation and leave the rebuild to the
         timed assertion. `nested_cargo::cargo_config_inputs` now reports the
         workspace-root `.cargo/config.toml` and the legacy `.cargo/config` when
         present, and every family — CLI, trybuild, `TrybuildTree` and both
         rebuild-invalidation fixtures — includes them.
- [x] Regression coverage —
      `an_inherited_target_directory_moves_the_stamp_with_the_artefacts`
      and `evicting_the_artefacts_invalidates_the_preparation` in the rebuild
      harness (the second removes the resolved artefact directory while leaving
      the scratch fixture intact and asserts the nextest gate answers
      `NotFound`); `cargo_configuration_is_an_input` and
      `the_workspace_cargo_configuration_is_an_input` in the shared policy, the
      trybuild binary, the CLI binary and `macrotest_support`. The eviction test
      passes `under_nextest = true` through the newly exposed hidden
      `ensure_prepared_with`, so it exercises the real gate without mutating the
      process environment.
- [x] Close the drain hang that made the per-command bound unbounded.
      `run_bounded_with_timeout` killed the direct child on timeout and then
      joined two `read_to_end` threads. A killed `cargo` does not reap the
      `rustc` it started, that grandchild inherits both pipe write ends, so the
      join waited for the grandchild rather than for the bound. Measured on the
      host: the 540 s preparation bound expired on schedule, the UI-lint
      `rustc` children kept writing into
      `crates/rstest-bdd/target/tests/ui_lints_clippy/debug/deps` for another
      five minutes, and nextest killed the test at its 30-minute period with no
      timeout error in the log. The bound was therefore not merely ineffective
      — it reported nothing, which is the property it exists for.
      `nested_cargo::drain::Drain` now moves chunks over a channel so
      `collect_until` stops at EOF **or** at a 2 s post-kill grace period, and
      the rebuild harness's private copy of the bounded runner — which had the
      same bug — now delegates to the shared one instead of duplicating it.
- [x] Confirm the drain fix on a real preparation, not only in the unit test.
      Re-running `make prepare-nested-fixtures` on an evicted UI-lint tree
      reproduced the same overrun — that graph is genuinely cold behind this
      host's admission pool, so it is a fair reproduction — and the difference
      is exactly the property the fix claims. The old code printed nothing and
      was killed by the runner at 1441.185 s. The new code printed
      `nested cargo exceeded its 540 s wall-clock bound; the child was killed`
      at the expected moment, **with the captured `Compiling` list and the
      `[build-limits] ... waiting for a compile slot (39 ahead, 45s)` lines
      attached**. That header is what identifies the cause as queue contention
      rather than a wedged build; without it the failure is indistinguishable
      from a hang, which is what made the original symptom so hard to place.
      The trybuild family in the same run stamped at 896.5 s, inside its 30 m
      period but over the 540 s per-command bound — as designed, since that
      family cannot be bounded (trybuild owns its nested `cargo`).
- [x] Show that the preparation is idempotent across a re-run, which is the
      property the whole design rests on. After the six families had been
      prepared once, a second `make prepare-nested-fixtures` skipped five of
      them in **under a second each** — 0.935 s, 0.173 s, 0.056 s, 0.302 s,
      0.084 s against the 64.9 s, 397.7 s, 224.0 s, 972.3 s and 281.2 s they
      took cold. Only the sixth rebuilt, and only because the first attempt had
      been cut off mid-graph; it then completed in 773.844 s and the whole run
      took 775.453 s, **6 of 6 passed**. A stamp that matched too eagerly would
      have shown the opposite — every family "passing" in milliseconds while
      the artefacts were absent — so the slow first run and the fast second one
      are both needed as evidence, and the regression tests
      (`evicting_the_artefacts_invalidates_the_preparation`) are what
      distinguish the two.
- [x] Separate a host-contention failure from a code failure before acting on
      it. The first of those two runs failed one family at its 1800 s period,
      which looks like the bug this branch exists to fix. It was not: the
      nested build itself reported `Finished dev profile ... in 23m 56s`, every
      crate in the log carried `[build-limits] … waiting for a compile slot
      (181 ahead, 45s)`, and other lines said
      `the managed sccache server is not serving compilations; compiling
      without the cache`. The same family took 239.213 s on CI. The signal that
      made the diagnosis possible was the captured output — which the drain fix
      from the previous item is what preserves. Re-running against the now-warm
      tree passed in 773.844 s. Neither the bound nor the period needed
      widening; the host did.

- [x] Close two further stamp-validity gaps raised by the CodeRabbit follow-up.
      1. **A relative target path was resolved twice.** `process::ChildEnv` and
         the CLI's `cli_preparation` both copied `CARGO_TARGET_DIR` into a
         `PathBuf` without making it absolute, while Cargo resolves a relative
         value against the working directory of the command that reads it —
         and the nested build runs in the scratch fixture directory, not in the
         test process's directory. Stamp and artefacts therefore named
         different trees, and evicting the artefacts left a matching stamp.
         `NestedCargoEnv::from_vars` now resolves the value once through
         `absolute_target_directory`, so the child and every stamp derived from
         the same environment agree; `process::child_env_from_vars` applies the
         same helper to its own snapshot, which is kept separate so the two
         runs of each experiment stay byte-identical. `NestedCargoEnv` gained
         `target_directory()` so callers never re-parse the variable.
      2. **A stamp survived an eviction of only the profile.** The stamp lives
         under `<target>/tests/<family>` and the dependency graph in
         `<target>/debug`, so `cargo clean`, or a plain `rm -rf <target>/debug`,
         removed the work while leaving the stamp matching. `Preparation` now
         carries `artefacts`, the directories the stamp vouches for, derived by
         `Preparation::new` as `<resolved target>/debug` (`DEFAULT_PROFILE`),
         and `ensure_prepared` admits a timed assertion only when every one of
         them exists. The intentional fixture-package clean that the rebuild
         assertion performs is untouched: `clean_fixture_units` removes the
         fixture's own units, and the gate's new requirement is that the
         *profile directory* exists, which it still does.
- [x] Regression coverage for both:
      `a_relative_target_directory_is_resolved_once_before_it_reaches_the_child`
      and `evicting_only_the_profile_invalidates_the_preparation` in the shared
      policy; `a_relative_target_directory_stamps_where_the_child_builds` and
      `evicting_only_the_profile_invalidates_the_preparation` in the rebuild
      harness; `for_fixture_vouches_for_the_profile_its_own_builds_populate`
      pins the trybuild/UI-lint shape, where the stamped root *is* the child's
      `CARGO_TARGET_DIR` and the profile is therefore a descendant of it rather
      than a sibling. `cli_child_environment_reuses_target` now asserts the
      resolved accessor agrees with the value the child receives.
- [x] Keep every changed file inside the 400-line limit the length gate
      enforces. Three had crossed it. `nested_cargo.rs` was split into
      `environment` (the filtered environment and the absolute-path resolver)
      and `bounded` (the bounded runner, which the existing `drain` submodule
      already supported); the module root now only re-exports and holds the
      command builders and `CapturedOutput`. The two test modules became
      directories: `preparation/tests/` with `stamping` and `inputs`, and
      `harness/preparation/` with its regression cases in `tests.rs`.
- [x] Fix the two further stamp-validity gaps CodeRabbit reported, committed as
      `821f249a`. First, a relative inherited `CARGO_TARGET_DIR` was resolved
      twice — once by the child, against the fixture directory it runs in, and
      once by whoever derived the stamp — so the two named different trees.
      `env_from_vars` now resolves it once, and `NestedCargoEnv::target_directory`
      exposes that same resolution. Second, the stamp at
      `<target>/tests/<family>` outlived an eviction of `<target>/debug`, the
      directory the dependency graph actually occupies, so the gate admitted a
      timed assertion against an empty profile. `Preparation` now carries
      `artefacts` and both gates require every one of them to exist.
- [x] Gate the change: `make fmt`, `make check-fmt`, the clippy stages of
      `make lint`, and `make lint-whitaker` all pass. Two real findings were
      fixed on the way: a `clippy::option_if_let_else` violation in the new
      `absolute_target_directory`, and a rustdoc private-intra-doc link to
      `NestedCargoEnv::from_vars` (now `env_from_vars`, the public re-export).
- [x] Note the `#[path = "harness/preparation.rs"]` attribute on
      `mod preparation`: converting that file to a directory module also
      required updating the attribute to `harness/preparation/mod.rs`, without
      which rustfmt cannot resolve the module at all. Ordinary `mod x;`
      declarations resolve to `x/mod.rs` on their own; an explicit `#[path]`
      does not.
- [x] Reconcile the review of `1ab5c4a0` against the branch head. The two P2
      findings it reports are the two gaps already closed three commits later
      by `45e1fa12` and `821f249a`; `1ab5c4a0` is an ancestor of both
      (`git merge-base --is-ancestor 1ab5c4a0 45e1fa12` succeeds), and the
      review was written before either commit existed. The cited lines confirm
      it: at the reviewed commit `fixture_preparation` stamped
      `fixtures::scratch_root()` and no input root included
      `.cargo/config.toml`, which is exactly what `45e1fa12`'s
      `artefact_directory(process::child_target_directory(env), subdir)` and
      `nested_cargo::cargo_config_inputs` replaced. No further code change was
      required for either finding; the reply on the PR states the commit
      relationship and asks for confirmation on the current head.
- [x] Fix one real defect found by this round's gate, committed as `6ee6035f`:
      `evicting_the_artefacts_invalidates_the_preparation` prepared through a
      no-op closure, so the family it stamped had no `<target>/debug` for the
      stamp to vouch for and the first gate refused for the wrong reason. The
      test now creates the profile directory before preparing, matching its
      sibling `evicting_only_the_profile_invalidates_the_preparation`, so the
      first gate passes because a real preparation leaves the profile
      populated and the last gate fails only because the eviction removed it.
      The whole `feature_rebuild_invalidation` binary then passed, 31 of 31,
      including both eviction regressions.

- [x] Close the fifth stamp-validity gap, found by review of `38d15524`:
      `artefacts_are_present` accepted any existing directory, so
      `rm -rf <target>/debug/deps` left the profile, the stamp root and the
      stamp intact while removing the compiled units — a partial eviction the
      gate admitted. The predicate now lives in
      `nested_cargo/preparation/artefacts` and requires `deps` and
      `.fingerprint` (`DEPENDENCY_ARTEFACT_DIRS`) to exist and hold at least
      one entry each, which is what Cargo reads to skip recompiling. The gate
      and its failure message share the one predicate, so a refusal and the
      explanation of it cannot drift. Regression coverage:
      `evicting_the_dependency_artefacts_invalidates_the_preparation` and
      `an_empty_profile_directory_is_not_a_prepared_family` in the shared
      policy, `evicting_the_dependency_artefacts_invalidates_the_preparation`
      in the rebuild harness. The shared fixtures now populate a profile as a
      real build does (`populate_profile`), so the previously passing cases
      prove the strong gate rather than a `mkdir`. The whole rebuild binary
      passed 32 of 32, including the real `nested_fixture_preparation`, which
      confirms genuine Cargo output satisfies the stricter check. Two lessons
      worth keeping: `is_dir()` is not evidence that work survived, because
      Cargo's layout keeps the parent while its contents go; and a test that
      pre-creates the directory the gate checks will happily pass against a
      gate weaker than the one being described.

- [x] Fix the two defects CI found on `b92a929d`. Both were in the branch's own
      test and documentation, not in the preparation design. The Windows
      failure, present on both lanes, was
      `fingerprint_excludes_runner_and_coverage_variables` asserting
      `CARGO_TARGET_DIR=/tmp/target`: `absolute_target_directory` resolves a
      path with no drive letter against the working directory, so on Windows
      the fixture was a *relative* path and the resolved value legitimately
      differed from the literal. The assertion was platform-specific and the
      product was right. The fixture now derives its target from
      `std::env::temp_dir()`, so it is absolute everywhere, and the test
      asserts the invariant that matters — `Path::new(&target).is_absolute()`
      and that the fingerprint records it — rather than a Unix spelling. The
      Linux failure was `make check-fmt` on `docs/developers-guide.md`, which
      `mdtablefix` wanted to rewrap by four lines because the Markdown had been
      edited by hand and never passed through it; `mdtablefix --in-place`
      restored a clean check. Both were reproduced locally first: `make
      check-fmt` exited 2 with the exact file listing, and the Windows failure
      was read from the completed run's `--log-failed` output rather than
      guessed at from the check annotation. Two lessons: a test fixture built
      from an absolute-looking Unix literal silently becomes a relative path on
      Windows, so it cannot describe the rule it claims to test; and a
      multi-paragraph list item changes markdownlint's inferred MD046 block
      style for the whole document, so a long bullet must stay one paragraph
      rather than being split for readability.

- [x] Close the sixth stamp-validity gap, found by CodeRabbit's review of
      `b9cb1d3b`: the gate accepted a `deps` directory that held dep-info
      records whose compiled units had gone. `deps` was non-empty and
      `.fingerprint` untouched, so both directory checks passed while rustc
      had to recompile the missing unit. `artefacts.rs` now pairs every `.d`
      record with its output (`lib<stem>.rlib` for a library, `<stem>` or
      `<stem>.exe` for a linked binary) and reports the orphaned stems. The
      pairing was measured against the two real prepared trees on this host —
      739 and 104 rlibs, every one with its `.d`, and every `.d` stem with an
      output — so the rule describes genuine Cargo output rather than a guess,
      and `cargo clean -p`, which removes both halves together, keeps the
      rebuild harness's intentional per-package clean safe. `prepare_locked`
      also now verifies the artefacts *after* the preparation command and
      before writing the stamp, so a successful command that left nothing is
      an error naming the missing paths instead of a stamp advertising a
      prepared family. Regressions:
      `a_dep_info_record_without_its_output_invalidates_the_preparation` removes
      one `.rlib` while `deps`, `.fingerprint` and the stamp all survive and
      requires the gate to answer `NotFound`; `an_empty_profile_directory_is_not_a_prepared_family`
      now expects the preparation itself to fail. The shared fixture had to be
      corrected for this: it wrote a placeholder named `deps/unit`, which the
      bare-binary half of the rule reads as unit `unit`'s linked output, so the
      fixture made the predicate look satisfied for the wrong reason.

- [x] Correct the sixth gap's pairing rule against a real tree, which the
      first version refused. It accepted `lib<stem>.rlib`, `<stem>` and
      `<stem>.exe` only, and the rebuild harness — the one suite that runs a
      genuine nested Cargo — failed all 32 cases with `preparation of family
      feature-rebuild-invalidation finished but left no usable artefacts in
      …/target/debug (missing outputs for rstest_bdd-…, tracing-…, …)`:
      over a hundred stems. The measurement that should have preceded the rule
      showed why. That `target/debug/deps` holds 1,295 `.d` records against
      1,061 `lib<stem>.rmeta`, 739 `rlib` and 83 `so` outputs, and **zero**
      unpaired records once `rmeta` and `so` are accepted. Metadata-only units
      are the commonest form there, because `cargo check` and metadata-only
      dependency requests leave no `rlib` at all. The rule now accepts the
      full output set. Two further corrections came out of the same failure:
      the shared fixture gained a metadata-only unit so the widened set is
      specified rather than merely tolerated
      (`a_metadata_only_unit_is_paired_with_its_dep_info_record` pins both the
      accept-while-present and refuse-once-removed halves), and the failure
      message now names at most five stems before summarizing, because naming
      every one printed 108 KB on a single line. Lesson worth keeping: a
      predicate over another tool's on-disk output must be measured against
      that output before it gates anything, and a synthetic fixture that only
      ever writes the shape the predicate already accepts cannot catch the
      mistake.

- [x] Replace the enumerated output set with a naming rule, after the
      enumeration was wrong a second time. The widened set listed `so` and
      `dylib` but not `dll`, so the Windows CI lane failed on `18430347` with
      `nested CLI fixture should be prepared … (missing outputs for
      derive_more_impl-…, displaydoc-…, linktime_proc_macro-… and 13 more)`:
      eighteen proc-macro units, every one of them a `lib<stem>.dll` on that
      target, and nine `cargo-bdd` CLI tests failed behind the one fixture.
      The insight is that the extension list was the wrong shape of solution,
      not merely incomplete: extensions differ per target and per crate type,
      so any list is a guess about a toolchain this code does not control.
      `is_output_of` now matches `lib<stem>.<ext>` and `<stem>` / `<stem>.<ext>`,
      requiring the dot so `unit_extra.rlib` cannot pair with `unit.d`. It also
      excludes a record from being its own witness — `unit.d` matches the rule
      for `unit`, so an unguarded search over the whole listing would make
      every record pair with itself and the check would never fire; the
      `a_record_is_not_its_own_output` case pins that. Re-measured against the
      real `target/debug/deps`: 1,752 records, zero orphans, and the rebuild
      suite back to 32/32 with `cargo-bdd` at 17/16.

- [x] Correct a property test whose central claim was false, caught by trying
      to falsify it rather than by watching it pass. The first version of
      `equivalent_trees_hash_identically` wrote one tree in two creation
      orders and asserted equal hashes, on the stated theory that `read_dir`
      order follows creation order. Replacing `entries.sort()` with
      `entries.reverse()` left the test **passing**, so the test proved
      nothing. Measurement showed why: on this host's ext4, `read_dir`
      returns entries in a hash of their names — independent of creation order
      and of how many times the directory grew, both verified directly. No
      sequence of file creations can produce two listings for the same tree,
      so that property was unfalsifiable as written. It was replaced by
      `entries_hash_in_sorted_order_not_listing_order`, which hands
      `hash_sorted` a known-unsorted sequence — the only instrument that can
      see the sort — after separating the sort from the traversal. All three
      remaining properties were then mutation-tested: removing the sort, the
      path hash, and the build-output exclusion each fail a named test. The
      surviving properties (`moving_an_input_changes_the_hash`,
      `changing_an_input_changes_the_hash`,
      `excluded_entries_do_not_reach_the_hash`) still test real invariants, and
      the module documents what it cannot show rather than implying coverage
      it does not have. Lesson: a property test must be falsified before it is
      believed, exactly like the predicate it guards.
- [x] Answer the review of `449e1caf`: three CodeRabbit findings and one
      CodeScene regression, committed as `ed80e103`.
      1. **CodeScene failed the gate on the property strategy.** `tree()` held
         two separately filtered sets — `inputs` then `excluded` — so it
         carried two nested `if`s, one per loop. That is a Bumpy Road with
         `bumps = 2`, which is the threshold, and the quality gate recorded
         9.84 and went red. `cs review` named the function and the bump count
         directly, which is what made the fix obvious rather than guessed: one
         loop over `inputs.into_iter().chain(excluded)` keeps the precedence
         (inputs win) and scores 10.00. This is the same lesson as the
         duplication findings earlier in this plan — ask the tool which
         function and which metric, do not infer it from the summary.
      2. **The eviction tests did not cover each directory independently.**
         `evicting_the_dependency_artefacts_invalidates_the_preparation`
         removed `deps` whole, which fails the non-empty check before
         `.fingerprint` is ever consulted; removing both would have passed even
         if only one were checked. `evicting_either_dependency_directory_alone_invalidates_the_preparation`
         now loops over `DEPENDENCY_ARTEFACT_DIRS`, removes one at a time, and
         asserts the sibling survives so the two checks cannot collapse into
         one. `emptying_a_dependency_directory_invalidates_the_preparation`
         covers the state `cargo clean -p` leaves behind — the directory exists
         and holds nothing, which `exists()` cannot see.
      3. **`stamping` was exactly at the 400-line budget**, so it could not take
         the two cases above. They went into a new `eviction` test module
         rather than being squeezed in: the six cases there all share one shape
         (the stamp survives, something beneath the profile does not), which is
         what distinguishes them from the lifecycle tests that remain. The
         shared setup became `stamped_preparation`, so each case starts from a
         genuinely stamped tree instead of resetting one inline.
      4. **Deferred, with evidence, the process-tree finding on
         `bounded.rs:83`.** `child.kill()` leaves a `rustc` that `cargo`
         started running, and that orphan can hold the build-directory lock.
         The finding is correct and its proposed fix is implementable — I had
         nearly deferred it on a false premise, that `unsafe_code = "deny"`
         blocks `process_group`, when `CommandExt::process_group` is a safe
         API. What makes it a follow-up is scope and risk: it is a behavioural
         change to the timeout path, outside issue 910; it needs a Unix path
         and a Windows job object, and the Windows lanes are the ones that
         already caught two under-specified predicates on this branch; and the
         lock-contention premise is unmeasured, which CodeRabbit itself
         recorded. The existing test proves the *call* returns in bounded time
         and deliberately does not assert the grandchild is gone. Tracked for a
         separate issue rather than folded in at the last gate.
- [x] Gate `ed80e103` with the full sequence, one gate at a time:
      `make check-fmt`, `make markdownlint`, `make lint` and `make test` all
      exit 0. `make fmt` was needed first because rustfmt reflowed two
      expressions in the new test file. The `make test` run reports every new
      case passing by name, including
      `evicting_either_dependency_directory_alone_invalidates_the_preparation`
      and `emptying_a_dependency_directory_invalidates_the_preparation`, and
      `cs review` on the strategy file reads 10.00 with no findings.

- [x] Close the two stamp-validity gaps the merge directive named, and correct
      the seventh gap's remedy rather than patching it. Both requirements were
      kept: the overall design stands, no assertion deadline was widened, and
      the trybuild exception remains documented as relying on the runner's
      preparation-test period rather than the harness's per-command bound.
      1. **The stamp now records artefact *names* instead of deciding by
         naming rule.** The review that raised this was right, and the proof is
         not a reading of the code: measured against a real crate, removing
         only `lib<stem>.rlib` while its `.rmeta` and `.d` stayed made
         `cargo build` print `Compiling <crate>` — a genuine cold rebuild the
         naming gate accepted. Re-measuring the same `deps` directory also
         showed why no repair to the rule can work: the stripped tree
         (`<stem>.d` plus `lib<stem>.rmeta`) is structurally identical to a
         `cargo check` unit, which Cargo *does* reuse, so any predicate over
         names is either wrong or a guess. `prepare_locked` therefore lists the
         outputs the dep-info records in `deps` describe, writes them into the
         stamp after the command succeeds, and the gate requires every one of
         them to still be present. `is_output_of` survives only to discover
         which entries belong to which record; it never decides whether a tree
         is usable. `STAMP_VERSION` moved to `2`, because a version-1 stamp
         records no names and cannot answer the question. The stamp stays one
         file with one rename, so a reader never sees a new fingerprint beside
         a stale name list.
      2. **`toolchain_identity` is fallible.** A fingerprint that could not
         identify the toolchain used to record `"unknown"`, which is the same
         text on a host where identification fails as on the run before, so a
         stamp would survive an actual toolchain change. `fingerprint` now
         returns `io::Result` and the failure is reported instead. The
         `OnceLock` went with it: one `cargo -vV` per gate call is cheap
         against a build.
      3. **The fail-open paths in the artefact queries are closed.**
         `has_entries` mapped `read_dir` failure to "empty", which is the
         unsafe direction — an unreadable profile would have passed. It now
         returns `io::Result<bool>` and maps only `NotFound` to `Ok(false)`,
         because an absent directory is the ordinary eviction worth naming
         while an I/O fault is not. `entries_of` propagates.
      4. **Regression coverage, both of the gaps the directive named.** In the
         shared policy, `a_recorded_output_that_was_removed_is_an_eviction`
         removes a linked unit's `.rlib` while its record and a genuine
         metadata-only sibling survive, and requires the gate to name exactly
         the artefact that went; `a_metadata_only_unit_is_accepted` pins the
         opposite direction. In the rebuild harness,
         `evicting_the_artefacts_invalidates_the_preparation` removes the
         resolved artefact directory while preserving the scratch fixture, and
         `a_configuration_only_change_invalidates_the_preparation` (added in
         this round) hashes the family's own `workspace_inputs` with and
         without the committed `.cargo/config.toml` and requires the two to
         differ. The shared-policy and CLI/trybuild/macrotest configuration
         cases cover the same rule at their own boundaries.
      5. **Two property-test helpers were wrong, and the deeper generator
         exposed them.** `excluded_path` only ever produced a root-level
         excluded name, which left the recursive half of `hash_sorted`'s rule
         untested; adding a prefix of up to two ordinary segments broke two
         cases and revealed both bugs: `is_excluded` judged only the first
         segment, so `a/target` counted as an input, and writing `p/target`
         under a tree with no `p` creates `p`, which is itself a new input
         name. Fixed by checking every segment, filtering generated inputs, and
         requiring the parents of an added excluded path to exist.
      6. **The length gate, again.** `artefacts.rs` reached 524 lines with the
         new unit tests, over the 400-line limit the lint gate enforces, so the
         tests moved to `artefacts/tests.rs` beside the module. This is the
         same split `stamping`/`eviction` already used, and the reason the
         limit is worth honouring rather than working around: the implementation
         file now reads as the rule and its two layers, with no fixture code
         interleaved.

## Surprises & Discoveries

- **A `#[cfg(unix)]` test can break the Windows *build* through its import
  alone.** 2026-11-09. Two cycle tests were marked `#[cfg(unix)]` because
  creating a link on Windows needs a privilege the runner lacks, but their
  import of `input_hash` sat at module scope. On Windows that import was
  therefore unused, and the workspace's `-D warnings` turned it into
  `error: unused import: input_hash` — so both required Windows lanes failed
  before a single test ran. The gate is cheap to check locally, which is the
  lesson worth keeping:
  `cargo check --target x86_64-pc-windows-msvc --all-features --tests` with
  `-Dwarnings` reproduces it in about two minutes. Verified by reverting the
  `#[cfg(unix)]` on the import and watching that command fail with exactly the
  hosted message.

- **Escaping a value changes the strings tests must compare against.** The
  fingerprint escaping added for the forging fix made a Windows target
  directory render as `C:\\Users\\RUNNER~1\\...`. One test still asserted on
  the raw path and so failed on Windows only, while passing locally where the
  path has no backslashes. The production text was right and the expectation
  was stale; the expectation now escapes the same way. This is the general
  shape to watch for: a change to how a value is *rendered* is invisible to
  Linux-only assertions when the Linux rendering happens to be unchanged.

- **Following a symlink turns a tree walk into a possibly cyclic graph.**
  2026-11-09. The fix for the configuration-link gap made `hash_entry` resolve
  a link instead of skipping it. That is correct for the case that motivated it
  — a `.cargo/config.toml` committed as a link — but a link may also point at
  an *ancestor of the walk*, and following that without a bound never
  terminates. The earlier code was safe here only by accident: because it
  skipped every link, it could not recurse through one. Review flagged the
  hazard as a reason to keep `hash_entry` unchanged for nested entries; the
  stronger fix is to keep the resolution and bound the descent, since a nested
  link is an input for the same reason a root link is. `hash_directory` now
  canonicalizes each directory as it enters it, refuses to re-enter one already
  on the stack, and hashes a `symlink-cycle` marker so a cut descent cannot
  read as an empty directory. Verified non-vacuous by deleting the check and
  watching both tests hang past a 120-second cap.

- **A required check can fail for a reason that is not in the diff, and the
  distinction is provable rather than rhetorical.** On 2026-11-08 the required
  lane `build-test (linux, default features)` began failing in its `Lint` step,
  inside `make lint-whitaker`, before any lint of this branch's code ran. The
  Dylint driver panics at registration:
  `duplicate specification of lint string_continuation_style`. The failing
  invocation passes two libraries,
  `libstring_continuation_style@nightly-2026-05-28.so` and
  `libwhitaker_suite@nightly-2026-05-28.so`, and the lint is registered by both.

  What makes this host state rather than this PR: the failure is identical at
  the same rustc commit (`57d06900`) on `8d45fae5`, a branch unrelated to this
  one, and it begins at a clean time boundary — the last success is `0c13d4a7`
  at 01:38Z and the first failure is `8d45fae5` at 05:02Z. `dylint.toml` is
  untouched by this branch and the runner label is `ubicloud-standard-2` on
  both.

- **The first explanation of that failure was wrong, and the corrected cause
  turned it from an impasse into an in-repo fix.** The entry above originally
  blamed the Whitaker cache key, arguing that `WHITAKER_INSTALLER_VERSION` is a
  version string rather than a content digest, so a republished installer could
  change contents under a stable key, and concluded this was an "unhealthy
  external service" impasse. Three observations refuted that:

  1. The **passing** run `42065583194` restored the *same* cache key
     (`whitaker-v1-Linux-X64-self-hosted-0.2.11-75f5f665…`) that the failing
     runs restore. The cache is not the variable.
  2. `install-whitaker` downloads the lint archive from the **rolling release at
     run time**; the cache only holds the installer binary and driver. The
     manifest `manifest-x86_64-unknown-linux-gnu.json` published at
     `generated_at: 2026-11-08T07:35:30Z` lists eleven files, including both
     `libstring_continuation_style.so` and `libwhitaker_suite.so`. Purging or
     re-keying the cache would change nothing.
  3. Whitaker merged `string_continuation_style` into **both** its `LINT_CRATES`
     and its suite in `67c5afaa` on 2026-11-07 23:53Z, thirteen hours before the
     first consumer failure. The cause is server-side content, not consumer
     state.

  The actual mechanism is in the installer: `prune_prebuilt_libraries` is meant
  to delete every staged library that was not requested, but it iterates the
  installer's **compiled-in** `LINT_CRATES`. The pinned `v0.2.11` predates the
  lint, so its nine-entry list never names `string_continuation_style`, the
  pruning loop skips the file, and `cargo dylint --all` loads it beside the
  suite. Upstream tracks this as **leynos/whitaker#565**, which specifies a
  filename-based prune; that fix is not in `main`, and `v0.2.12` (PR #559) is a
  pure version bump whose `LINT_CRATES` is still nine entries, so a version
  bump would not help either.

- **The consumer side owns the last step, and the fix is one recipe line.**
  Because the installed tree cannot be trusted to hold exactly the suite,
  `make lint-whitaker` now names the aggregated suite directly:
  `whitaker --lib whitaker_suite`. The suite already registers every
  constituent lint, and the installer's own default (`requested_crates` empty)
  resolves to suite-only, so this loads exactly the set `--all` was meant to
  load while being independent of the pruning bug. Verified two ways: a
  negative control proved `--lib whitaker_suite` still rejects
  `unwrap_or_else(|| panic!(..))` with `no_unwrap_or_else_panic`, so the gate
  is not weakened; and `test_lint_whitaker_selects_the_suite_by_name` fails on
  the old `--all` recipe and passes on the new one. The local install carries
  only `libwhitaker_suite@…`, which is why `make lint` never reproduced this
  here.

- **A required check and a quality signal are different things.** CodeScene
  failed on every push to this branch, which looks like a blocker and is not
  one: `gh api repos/leynos/rstest-bdd/rulesets/18427987` shows the merge gate
  is `main-required-checks`, and it names only the three `build-test` contexts.
  Classic branch protection is absent, so a failing CodeScene check neither
  blocks nor reports a structural problem. The two are still worth separating
  in the other direction as well — the repository means it about code health
  (`clippy.toml` opens with "Align with CodeScene's ceiling"), so an advisory
  regression is a regression even when the merge button stays green.
- **A quality gate reports the files a diff touched, not only the lines it
  changed.** Three of the seven CodeScene findings were in files this branch
  already degraded — duplications scored 9.39 both before and after — while the
  four with a `10.00 →` prefix were files the diff pushed below a perfect
  score. Reading the arrow, rather than the presence of a filename, is what
  distinguishes "this change made it worse" from "this change landed near an
  existing problem". It is also what stops a refactor aimed at the wrong file.
- **A passing timing is not a comparable timing until you check the cache.** The
  final CI run measured the whole suite in 348 s against the earlier run's 1402
  s, which reads as a four-fold improvement from this branch and is nothing of
  the kind: sccache served 95.71% of Rust compilations on that runner and
  13.80% on the other. The cold run is the one acceptance criterion (a) asks
  about, because it is the run that actually pays for the dependency graphs;
  quoting the warm number in its place would have made the evidence look better
  while measuring the wrong condition. Both belong in the record, labelled with
  their cache state. The lesson generalizes past this issue: any CI duration is
  a joint measurement of the code and the runner's cache, and the cache state
  is the half that changes without a commit.
- **A figure can be true and still be the wrong figure.** The preparation
  budget's rationale cited 386.8 s as "the largest cold dependency build". That
  number is real, but it measures the old *timed* `step_macros_compile` run on
  `ubicloud-standard-2` against a partial sccache — the very cost this change
  removes from the assertion. The cold *preparations* measured here are 645.412
  s and 864.239 s. Quoting a correctly transposed number under a new heading is
  easy to miss in review because every digit checks out; what fails is the
  referent.

- **Count the commands, not the functions that call them.** The per-command
  bound was first paired with a count of two, taken from the two preparation
  families in `prepare_all`. The families are not the unit: one of them runs a
  metadata preflight and then a compile, so the test issues three bounded
  commands, and `3 × 12 m` exceeded the 30 m period the pair was supposed to
  fit inside. A contract test that recomputes the inequality is only as good as
  the constant it multiplies, and a constant chosen by reading the call
  structure can be wrong in exactly the direction that matters — too low.
  Counting the child processes the test actually spawns is the measurement;
  counting the functions that spawn them is an inference.

- **A blank line inside a list item can turn prose into a code block.** Adding
  a second paragraph to this plan's `- [x]` bullet broke `make fmt` with two
  `MD046/code-block-style` errors on fenced blocks a thousand lines further
  down. The cause was not near the errors. The bullet's content column is 2, so
  continuation lines at 6 spaces are indented exactly 4 columns past it, and
  CommonMark reads an indented run after a blank line as a *code block*, not as
  paragraph text. `MD046` runs in "consistent" mode, takes its expected style
  from the document's first code block, and that block had just become the
  indented one — so every later fence was reported. The renderer agrees with
  the parser, so this would have shown up as a stray code block on GitHub too.
  Joining the paragraphs fixed both. The lesson is that a Markdown error can be
  caused by a blank line far away from where it is reported: bisect the diff,
  do not read the reported line.

- **`priority` is not precedence.** The first version of the nextest override
  comment claimed `priority` decided which override won. It does not: nextest
  documents `priority` as "higher values run sooner", a scheduling hint, and
  applies the first override in file order that configures a setting. The
  comment and the contract test were corrected; the test now asserts the
  override's file position separately from its priority.
- **The 30-minute preparation budget changes what the workflow contracts
  read.** `largest_slow_timeout` takes the maximum over every `slow-timeout`
  table in the file, overrides included, so the preparation period became the
  "largest per-test budget". The tier ordering still holds (30 m below 90 m),
  and the watchdog arithmetic is unaffected because it is derived from the
  global timeout plus a fixed cold-build allowance rather than from the largest
  per-test budget. It is recorded in the developers guide so the figure is not
  mistaken for an assertion budget.
- **A duplication finding names a file, not the duplicated lines.** CodeScene
  reports `macrotest_support.rs` with an empty `method=` parameter, so it says
  which file regressed without saying which block repeats. Three guesses were
  made from reading the diff — that `trybuild_target_root` duplicated
  `binary_test_support`'s `target_directory_for_manifest`, then that
  `prepare_now`/`ensure_prepared` shared a prologue, then that it was
  `Preparation::with_target_directory` — and all three were wrong. The
  `cargo metadata` call was already in this file on `main`, and the shared
  prologue was four lines.
- **The CodeScene CLI measures the finding locally, so stop guessing and
  measure.** `cs check <file>` scores one file, and `cs review <file>` names
  the duplicated *functions*. Both are installed at `~/.local/bin`, need no
  configuration, and answer the question in seconds. On this file `cs review`
  named `assert_snapshot_contains` and `assert_snapshot_omits` — two functions
  this branch never touched, which is why every guess aimed at the new code
  missed. Verifying the hypothesis outside the repository is not enough:
  `cs check` on a copy of `main`'s file reported 10.00, and the same file
  scored 9.38 once it sat in the worktree, so the *pair* is duplicated either
  way and the added bulk only tipped a threshold. The fix was to make the two
  functions structurally different: `assert_snapshot_contains` now collects
  every missing needle and reports them together, which both removes the
  duplicated block and gives a better diagnostic than failing on the first one.
  `cs check` then reported 10.00 with no warnings. Measure first; the
  measurement is cheaper than one wrong refactor.
- **A file's score can be limited by functions the diff never touched.** The
  finding was filed as a *regression this branch introduced*, and in the sense
  CodeScene means — the file's score fell below the gate — it was. But the
  duplicated pair predates the branch. That is not an argument for ignoring it:
  the gate compares the file, and the file now fails.
- **The CodeScene CLI is installed at `~/.local/bin/cs` and answers this
  offline.** `cs check <file>` scores one file; `cs review <file>` names the
  duplicated *functions*, which the GitHub check summary never does. A whole
  hypothesis cycle — edit, commit, push, wait for CI, read the check run —
  costs minutes; `cs review` costs a second and points at the exact pair. Two
  caveats learned the hard way: it needs a git repository (it shells out to
  `git` and errors out of one), and `--check` and the in-place rewrite can
  disagree by a few lines, so write the formatter's stdout to a file and copy
  it in rather than trusting in-place mode.
- **`input_hash` already skips `target` directories by entry name,** so
  `crates/rstest-bdd/target/tests/ui_lints_clippy` is excluded from the
  `crates` input root even though it lives inside it. No special case was
  needed.
- **`nextest-version` is a top-level key,** not a profile key, and nextest
  ignores settings a file does not declare itself new enough to use. A
  `priority` in a file without it would silently be ordinary.
- **A preparation can invalidate its own stamp.** The input hash is computed
  *before* the preparation runs, so anything the preparation writes inside an
  input root becomes a new input. `trybuild` writes a `wip/` scratch directory
  beside the crate under test — not inside the target directory — and creates
  it during the very build the stamp describes. Hashing it meant the stamp was
  stale the moment it was written: the cold preparation passed in 645 s and
  `step_macros_compile` then failed in 0.219 s with "missing or stale". `wip`
  is now excluded by name, like `target` and `.git`, and
  `preparation_survives_the_build_output_it_creates` pins the ordering rule
  rather than just the name list. The other families hash source trees their
  builds do not write into, so the exclusion is what keeps them safe too.
- **A predicate four names wide is a Whitaker failure.** Clippy's
  `pedantic` set accepted the four-term exclusion, so the gate reported it only
  from the Dylint suite: `conditional_max_n_branches` allows two branches per
  `if`. The fix is not an `#[expect]` in library code but the named predicate
  `is_build_output`, backed by a const slice the way `stamp.rs` records its
  fingerprint variables. Two different linters policing the same function is a
  reason to name the concept, not to silence one of them.

- **A stamp is only as good as its input set.** Three families hashed their
  fixture directory alone, and every fixture there declares a path dependency on
  `crates/rstest-bdd` or `crates/rstest-bdd-macros`. Editing a workspace crate
  therefore left the stamp valid while the timed assertion compiled the changed
  crates itself. The rule the review implies: a family's inputs are everything
  that changes what its artefacts compile, not everything that sits next to the
  fixture. The cost is more frequent preparation; the alternative is a gate
  that silently stops protecting anything.
- **`is_ok_and` encodes failure as "does not match".** `was_written_since`
  documented that an unreadable timestamp counts as fresh and then wrote
  `is_ok_and(|modified| modified >= threshold)`, which counts it as stale. The
  two readings differ only on platforms whose filesystems omit modification
  times, which is why nothing caught it. Where a `Result` failure has a
  documented meaning other than `false`, unwrap it with `let Ok(..) = .. else`
  and state the default explicitly.
- **A stamp in the wrong directory is worse than no stamp.** The harness's own
  environment code deliberately preserves an inherited `CARGO_TARGET_DIR`, and
  the preparation layer independently stamped the workspace scratch root. Each
  was defensible alone; together they meant that under a coverage driver the
  stamp and the artefacts lived in different trees, and evicting the artefacts
  left a matching stamp behind. The general lesson is that a cached claim of
  readiness has to be co-located with the thing it claims, and the way to
  guarantee that is to derive both from one resolved value rather than to write
  the same expression twice. `ChildEnv` carries the resolved directory for that
  reason.
- **An input set is only as complete as the tool's discovery rules.** Hashing
  the workspace manifest, lockfile and `crates/` looks exhaustive until one
  remembers that Cargo walks *up* from the working directory for
  `.cargo/config.toml` and applies it to the nested build. The file is not near
  the fixture, is not near the crates, and is still an input. The review found
  it by asking what else Cargo reads; the fix is a named helper so the next
  caller does not have to rediscover the discovery rule.
- **A hidden-API escape hatch can be the honest choice.** Testing the nextest
  refusal means either mutating `NEXTEST_RUN_ID` — which races every other test
  in the same binary — or letting the test say which side of the boundary it is
  on. `ensure_prepared_with` is `#[doc(hidden)] pub` for that reason, matching
  the crate's existing `macrotest_support` and `binary_test_support` pattern.
  Exporting test-only policy to the public surface would be worse; silently
  skipping the coverage would be worse still.
- **A bound that kills the child is not yet a bound.** The timeout path killed
  the direct child and then joined both pipe readers, which only return at EOF.
  `cargo` killed by a signal does not reap its `rustc` children, they inherit
  the same pipe write ends, and the join therefore waits for them. The 540 s
  preparation bound expired on time and the call did not return for another
  five minutes; the runner killed the test first, so the harness's carefully
  worded timeout error — the entire reason the bound captures output — was
  never printed. Any "wall-clock bound" over a process that spawns children has
  to bound its *own* bookkeeping too, which is why the drain now stops on a
  deadline rather than on EOF.
- **The test that would have caught it could not be written from the failure.**
  The failing run's log showed a completed preparation followed by silence, and
  the obvious reading — a second family's build overran — is wrong: the UI-lint
  target's mtimes kept advancing after the bound, which is only visible if you
  look at the filesystem rather than the log. The regression test therefore
  asserts on *elapsed wall-clock time* with a grandchild holding the pipe, not
  on the error text, because the defect is precisely that the error text never
  arrives.
- **The rolling suite, not this branch, is what changes the lint gate.** After
  `--lib whitaker_suite` removed the duplicate-registration panic, CI's `Lint`
  step failed again on `string_continuation_style`, a lint Whitaker added to
  the suite in `67c5afaa`. Every file that failed —
  `implementation_navigation.rs`, `examples/todo-cli/tests/todo.rs`,
  `runner_cancel/cases.rs`, `runner_instrumentation.rs` — is byte-identical to
  `main`, which passed CI at `42434205582`. Two independent facts rule out a
  branch-local cause: `git diff base...HEAD` lists none of them, and the shared
  action's `suite-version` input is *declared only to fail the step*, because
  "Whitaker's lint suite is a rolling release that every consumer takes from
  the default branch tip, and a pin would force a source build." There is no
  pinning lever; `main` will fail the same way on its next run.
- **The remediation is bounded, and far smaller than a textual scan suggests.**
  A grep for trailing backslashes finds 169 candidates, but the lint only
  diagnoses proven `concat!()` candidates: it exempts layout trims, byte and C
  strings, literals at non-zero wrapper depth, and any format string with a
  `FormatArgumentKind::Captured` argument. Driving the real suite with
  `--keep-going` over `--workspace --all-targets --all-features` found 24
  literals in 12 files. Two sites the scan flagged were *correctly* not
  diagnosed because they use implicit capture (`{polls}`, `{MAX_POLLS}`) — the
  exact case the RFC calls a required continuation. The final scan of the whole
  workspace exits 0.
- **`concat!()` survives rustfmt, which is why the fix is stable.** `make fmt`
  re-wrapped several fragments back onto `\`-continuations inside the
  `concat!()` arguments. That is not a regression: a literal inside a macro
  expansion has `is_source_literal == false`, which the decision matrix routes
  to `Ignore`. Re-running the suite after formatting still exits 0.
- **A check that fails open is worse than no check, and this module had one.**
  `has_entries` decides whether `deps` or `.fingerprint` is populated, and its
  own documentation says an inspection failure "must fail closed rather than be
  read as an empty directory, which is the unsafe direction". The body did the
  opposite one level down: `entries.next().is_some()` maps an `Err` from the
  directory iterator — a read fault partway through the listing — to `true`, so
  an unreadable profile passed the structural check. `read_dir` reports such a
  fault only from `next()`, and no portable fixture produces it as the *first*
  item, so the decision was split into `has_entries_from` to keep the branch
  reachable from a test; reverting the fix now fails that test.
- **A recoverable error path still has to name its cause.** `matching_stamp`
  logged an unreadable stamp and returned `None`, and `eviction_of` used
  `is_dir()`, which folds a metadata fault into "evicted". Both fail closed,
  but both read a tree that cannot be inspected as one that is known to be
  absent, and the rebuild that follows has no visible reason. Both now return
  the error; the caller still re-prepares, because the alternative to a
  readable stamp is to prepare again either way.
- **The 400-line module cap and the two failure modes it creates.** Moving an
  inherited bug's fix into `preparation.rs` pushed it from 399 to 418 lines and
  tripped `check_rs_file_lengths.py`. The helper belonged beside the read it
  wraps anyway, so it moved to `stamp.rs`; `preparation.rs` returned to 399.
  The earlier discovery is the more useful one: `preparation.rs` was *one line
  under* the cap before this work, so the gate was green only by luck and any
  addition to the module would have failed it.
- **A pre-existing timeout must be proved pre-existing, not assumed.** The
  harness suite reported `binary_test_support_cargo::locate_or_build…` as a 60
  s nextest `slow-timeout`. Nextest's default period is 60 s and the test
  spawns a nested `cargo` build, so with the compile-admission pool fully held
  (`capacity 4, held 4`, ~100 crates queued) it legitimately overruns. Stashing
  the whole diff and re-running reproduced the timeout on clean `HEAD`, which
  is what makes it environmental rather than a regression.
- **A preparation bound can be spent entirely in the admission queue, and that
  is the failure issue 910 describes showing up on the preparation side.** On
  2026-11-09 the host was running several agents' builds at once; the shared
  compile-admission pool held `capacity 4, held 4` with 100–300 crates waiting.
  `make test` then failed in two different preparation families for the same
  reason, and neither was a code defect:
  - `cargo-bdd::cli` spent its whole 540 s bound queued 115–195 deep and was
    killed having just printed `Finished … in 8m 55s` — the compile succeeded,
    the next command never started.
  - `rstest-bdd::trybuild_macros` *prepared and stamped* its family in
    1195.9 s, then lost a later bounded `cargo clippy` command to the same
    540 s bound while queued behind ~100 crates.

  Two things follow. First, the bound is doing its documented job: it names the
  command and reports the captured output rather than hanging the runner, and
  `3 × 540 s < 30 m` still holds, so widening it is not available and is not
  the fix. Second, the *evidence* for this branch has to be read from a host
  that is not oversubscribed, or from the hosted lanes. Both are recorded
  separately in the Progress section; a red local run under a 300-deep queue
  says nothing about the diff, and a green hosted run says nothing about this
  host.
- **A bare `cargo nextest run` is not the make target, and the fingerprint
  knows the difference.** Rerunning the failed CLI preparation directly
  (`cargo nextest run -p cargo-bdd --test cli -E …`) reproduced the failure
  against a *different* cause: without
  `RUSTFLAGS="-D warnings -Clink-arg=-fuse-ld=mold"` the fingerprint changed,
  so the prepared tree was correctly rejected and the child paid for a full
  cold build inside its bound. The gate was working; the invocation was wrong.
  This is the trap the "Concrete steps" section already documents, and it is
  worth restating as an operational rule: a preparation failure has to be
  diagnosed by reading *whether the stamp matched*, not by the timeout alone.
- **A fail-closed check can still be reported as the wrong failure.** The first
  version of the unreadable-stamp handling logged the read error and returned
  `None`, which is indistinguishable from "no stamp" at the gate. Under
  `nextest` that turned a present-but-unreadable stamp into a "missing or
  stale" message naming evicted artefacts that were not evicted. The two
  runners need opposite outcomes from the same fact — fail fast under nextest,
  prepare again under `cargo test` — so the read error now reaches the boundary
  that owns the runner policy, and the nextest message names the read failure
  and its remedy. Reported by CodeRabbit on `70edad19`; the "reported rather
  than silent" half was right, the "therefore treat as absent" half was not.
- **Removing a derivation can empty a test of meaning.** The fingerprint
  coverage test originally built its expected variable list from
  `FINGERPRINT_EXACT` and `FINGERPRINT_PREFIXES`. Deleting `RUSTUP_TOOLCHAIN`
  from the constant also deleted its case, so the test kept passing over a
  fingerprint that no longer covered the toolchain selector. The list is now
  written out independently and checked both ways against the constants, so
  each is a claim the other must confirm. The same review round found that a
  failure test injecting into `fingerprint_from` never exercised the production
  probe; `identity_from` now takes the exit flag and output text, making the
  non-zero exit and the unusable-output branches reachable without a
  subprocess, and it moved to its own `toolchain` module so the fingerprint
  query itself contains no process execution.
- **A hash that reads an entry as "neither file nor directory" hashes
  nothing.** `input_hash` classified with `fs::symlink_metadata`, which reports
  a symbolic link as neither, so the link fell through both branches and
  contributed no bytes and no name. That was invisible until the configuration
  input made it matter: `cargo_config_inputs` follows the link with
  `fs::metadata` and reports it present, while the hash silently dropped it, so
  a repository committing `.cargo/config.toml` as a symlink would keep a
  matching stamp after the file it points at changed. The walk now resolves a
  link with `fs::metadata` and hashes the target — as a file's contents, or as
  a directory walk — plus the link's own name. A dangling link stays "absent" in
  `cargo_config_inputs` and is safe in both directions, because either
  transition changes the hash; that is pinned rather than changed.
- **Text compared for equality needs its field boundaries escaped.** The
  fingerprint is a line-oriented record compared as text, and each value was
  written verbatim. A `RUSTFLAGS` of `-Dwarnings\ninputs: h` composed exactly
  the same bytes as the same flags with an `inputs` of `h`, so two genuinely
  different builds could share one stamp — and the test that pins this had to
  be written against that specific collision, because a first attempt compared
  two descriptions that already differed and would have passed over a
  fingerprint with no boundary handling at all. Values now escape `\`, `\n` and
  `\r` onto one line, and `STAMP_VERSION` moved to `3` so every stamp written
  under the old encoding is invalidated once.

## Decision Log

- **Move the work, do not extend the deadline.** The acceptance criteria
  forbid hiding a timeout behind a larger number. Every assertion budget is
  unchanged from `origin/main`; the only new budget belongs to a test that
  exists solely to prepare.
- **One shared policy, not four copies.** Each suite needs the same stamp,
  fingerprint and lock behaviour. It lives once, in
  `rstest-bdd-harness::nested_cargo`, and each suite supplies only its family
  name, directory, environment, variant label and input hash.
- **Stamps live inside the artefact directory.** This makes eviction
  self-invalidating without a separate garbage-collection rule: delete the
  target directory and the claim of preparedness goes with it. "The artefact
  directory" means *the directory the child environment resolved*, not the
  workspace default. `process::build_child_env` preserves an inherited
  `CARGO_TARGET_DIR`, so the two can differ; deriving the stamp path from
  `process::child_target_directory` is what keeps the rule true when they do.
  The alternative — dropping the inherited variable so the workspace target is
  always the answer — would have been simpler and wrong: it would ignore the
  coverage driver's target directory and rebuild artefacts the harness had been
  told to reuse.
- **The scratch `.stamp` and the preparation stamp stay separate.** They answer
  different questions — *which fixture source was copied* versus *which
  artefacts exist* — and under the default target they happen to share a root.
  Merging them would make a source refresh imply an artefact rebuild, or an
  artefact eviction imply a source recopy, and neither implication holds.
- **Cargo configuration is an input, not ambient host state.** Cargo discovers
  `.cargo/config.toml` by walking up from the working directory, so the
  committed file's development profile options and Linux linker flags change
  dependency artefacts. `cargo_config_inputs` hashes the workspace-root file;
  configuration above the workspace is excluded because it varies by host and
  including it would make stamps non-portable between a developer machine and
  CI. Configuration *below* the workspace root needs no listing: those files
  already sit inside the hashed `crates/` and fixture trees.
- **Fail fast under nextest, prepare lazily under `cargo test`.** Under
  nextest each test is its own process, so a timed test that prepared would pay
  the cost it exists to avoid; failing with the preparation command named is
  more useful than a timeout. Under plain `cargo test` there is no scheduling
  guarantee that a preparation test runs first, so the gate prepares behind a
  process-wide lock instead.
- **One bounded runner, and it is the shared one.** The rebuild harness had its
  own copy of the spawn-drain-kill loop, which meant the drain hang would have
  needed fixing twice and the timed assertions would have been the tests that
  discovered it. It now delegates to
  `rstest_bdd_harness::nested_cargo::run_bounded_with_timeout` and keeps only
  its own two constants. The harness still owns its *environment*, because the
  two runs of each experiment must be byte-identical and the coverage
  redirection differs from the shared helper's; owning the environment and
  owning the process loop are separate concerns.
- **Drain on a deadline, not on EOF.** The reader thread sends chunks over a
  channel and the waiting side stops at EOF or at a fixed post-kill grace
  period, whichever comes first. EOF remains the normal path — output is
  complete the moment the child exits — so the grace period is pure overhead
  only in the case that used to hang, and it is bounded overhead rather than
  none.
- **The preparation override goes first in the file.** Nextest's first-match
  precedence means position, not priority, is what keeps the preparation from
  inheriting a timed assertion's budget.

## Outcomes & Retrospective

Implementation and evidence collection are complete. Every head pushed after the
`.config/nextest.toml` change passes all three required lanes; the latest is
`3ffa7295` in run `42057352055`. Two earlier CI runs on the Linux coverage lane
supply the timing figures: `41981885921` with an evicted compiler cache, which
is the cold-cache evidence acceptance criterion (a) asks for, and `42005188689`
with a warm cache. Every run recorded in this plan passed every required lane.

Naming a "final" head in a living document is a trap this plan fell into twice.
`434d8309` was called final and then followed by `df26170d`; any commit that
corrects the claim supersedes it again. The durable distinction is between the
head whose source the timing evidence describes and the head CI last validated,
because the second moves with every documentation commit while the first does
not. The timing figures below are attributed to their runs rather than to "the
head" for that reason.

The two evidence runs precede the commits that answered CodeRabbit and fixed
the formatting failure those commits carried, so the code they measured is not
byte-identical to the final head. The behavioural change after them is the
per-command preparation bound, from 45 minutes to 9 (three sequential commands
at 9 minutes each total 27 minutes, inside the 30-minute period): a deadline
inside the preparation test that no passing run approaches. The largest cold
preparation measured 438.214 s, 81.2% of 9 minutes (540 s) — the bound still
clears it, but the margin is no longer large, and the bound is not a budget any
run is expected to spend.

Run `42013457153` closes part of that gap: it is the first run on the corrected
ordering, and it passes all three required lanes with the same suite time as
the earlier warm run (323.344 s against 348.345 s), which is what a change to
an unapproached internal deadline should look like. It measures the 12-minute
bound, though, not the 9-minute one.

Run `42021251818` measures the 9-minute bound on the code the fix landed in,
and run `42028526380` measures the true final head after the prose commits that
followed it. The second matters for a reason the first does not cover. Its
Linux lane took 36 m 08 s against the first's 32 m 09 s, which reads like a
regression until the two are decomposed. In all three runs the nextest summary
is about a sixth of the lane (17.5%, 17.3% and 16.6%), so roughly 30 minutes of
each run happens before and after it — image setup, the Cargo build, coverage
handling. That outside-the-suite time is where the 3.5-minute lane difference
sits (26.6 minutes before, 30.1 now), while the suite grew 8.3% on code this
diff does not touch. A diff that touches only Markdown cannot account for
either figure. The lane duration is therefore not evidence about this change,
and the suite duration is; recording both is what keeps the conclusion
checkable. `make test-workflow-contracts` (364 tests), `nextest_config` (5) and
`rstest-bdd-harness` (51) also pass locally on the final head.

Every suite that compiles a nested Cargo graph is prepared and stamped,
including the harness and server trybuild trees. On the development host a cold
nested target prepared all six families in 2213.657 s, well inside their 30 m
budgets, and the timed assertions then ran against those trees:
`list_steps_runs` in 2.223 s against 180 s, `feature_addition_triggers_rebuild`
in 224.433 s against 600 s, and the CLI and trybuild helper cases in seconds.
No assertion deadline was widened.

Two things surfaced while wiring the remaining suites. `trybuild` derives its
build directory from the *workspace* target root, not from the crate being
tested, so all four compile-test binaries share `<target>/tests/trybuild`. That
made per-crate families necessary rather than merely tidy: one crate's
preparation warms the shared dependency graph, but its stamp must not stand in
for another crate's differently shaped generated manifest. Second, the server
crate's compile test previously ran with no preparation at all, so on a cold
target its dependency graph — `async-lsp`, `tokio`, `tower`, `gherkin`, `syn` —
was compiled inside a 20-minute assertion budget; it now follows the same
pattern, which needed `rstest-bdd-harness` as a dev-dependency there.

A third surfaced only by running a cold preparation end to end, and it would
have defeated the whole design on CI. `trybuild` writes `wip/` beside the crate
under test — inside `crates/`, which is one of the hashed input roots — and it
creates that directory during the very build the stamp describes. The stamp
records a hash computed *before* the build, so every cold preparation wrote a
stamp that was stale on arrival and the timed test was refused its gate. Fixed
by excluding `wip` from the hash, with two regression tests: one for the
exclusion and one asserting that a preparation survives the build output it
creates.

One consequence of sharing the gate is worth recording. The server suite is the
only prepared tree with nothing to stage: its fixtures reference no feature
file, so `prepare` has no fallible step and Clippy reports
`clippy::unnecessary_wraps`. Returning the bare summary would not compile, since
`TrybuildTree::prepare_now` and `ensure_prepared` both take an
`io::Result`-returning closure so that a tree whose preparation can fail uses
the same gate. The signature therefore stays, with a scoped
`#[expect(clippy::unnecessary_wraps, reason = ...)]` recording why the result
is never `Err` — the repository denies `allow_attributes`, so this is the
`#[expect]` idiom the lint policy is written for. The `rstest-bdd` trybuild
binary's own `prepare_trybuild` needed the same treatment for the same reason:
`trybuild` panics on a failing fixture, so its result is never `Err` either.

A narrower Clippy invocation is not a substitute for the gate. Checking with
`cargo clippy -p <crate> --all-targets` compiled a
`#[cfg(not(feature = "mutable_world_macro"))]` test that `main` has always had
and reported an `uninlined_format_args` failure in it, while the gate — which
passes `--all-features` and so never compiles that item — reports nothing.
Reproduce a gate result with the gate's own flags, and prefer running the gate.

The remaining caveat is about interpreting local measurements. This host routes
`rustc` through a shared four-slot admission pool, and CI does not, so local
elapsed times under contention measure the queue rather than the suite. The
figures above establish that the cold work moved and that the assertions reuse
the prepared trees; the CI coverage lane is what establishes that the suite
fits its budget there.

Review found two remaining ways a timed assertion could still pay for a cold
dependency compile, both of which survived a full suite of passing tests. The
first was co-location: the stamp went to a fixed workspace path while the child
environment was free to resolve a different `CARGO_TARGET_DIR`, so an evicted
external target left a matching stamp behind. Nothing in the passing evidence
could have shown it, because the default target is the workspace target and CI
sets no override — the failure required an environment the tests never used.
The second was input completeness: the workspace Cargo configuration is read by
Cargo's upward walk and was in no input list, so a configuration-only edit
would rebuild dependency artefacts under a stamp that never noticed. Both are
now closed, and both have regression tests that fail on the old code. The
lesson worth carrying is that a passing suite is evidence about the paths it
exercises, not about the paths an environment variable can reach.

That is also why the new eviction test guards its own preconditions. It asserts
the stamp is inside the external target *before* preparing, so a regression
cannot quietly write one into the shared scratch root and leave the shared
workspace state dirty for every other test in the binary.

## Context and orientation

The shared preparation policy is
`crates/rstest-bdd-harness/src/nested_cargo/preparation.rs`, with the stamp
protocol in `preparation/stamp.rs` and directory hashing in
`preparation/input_hash.rs`. `Preparation::new` takes the three values that
identify a family (`Sources`: name, variant label, input hash), the directory
it stamps, and the filtered `NestedCargoEnv` its commands run with;
`prepare_now` runs a closure unconditionally and writes the stamp on success;
`ensure_prepared` fails fast under nextest and prepares lazily otherwise.

The five suites:

| Suite                                                                        | Preparation module                                                                 | Stamp location                                                                 |
| ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `cargo-bdd::cli`                                                             | `crates/cargo-bdd/tests/cli/preparation.rs`                                        | the CLI child environment's target root                                        |
| `rstest-bdd::trybuild_macros`                                                | `crates/rstest-bdd/tests/trybuild_macros/preparation.rs`                           | `<target>/tests/trybuild` and `crates/rstest-bdd/target/tests/ui_lints_clippy` |
| `rstest-bdd::feature_rebuild_invalidation`                                   | `crates/rstest-bdd/tests/feature_rebuild_invalidation/harness/preparation.rs`      | `<resolved target>/tests/<family>`, one per fixture family                     |
| Tokio and GPUI harness `macro_compile`, server `workspace_discovery_compile` | `<crate>/tests/.../preparation.rs`, described by `macrotest_support::TrybuildTree` | `<target>/tests/trybuild`, one stamp per family                                |

`.config/nextest.toml` carries the override ordering, and
`crates/rstest-bdd/tests/nextest_config.rs` pins it.

## Plan of work

### Stage A: shared policy (complete)

The harness gains a preparation module with the stamp protocol, environment
fingerprint, toolchain identity, input hashing, a locked lazy path and a
fail-fast nextest path. The stamp is written last, atomically, so a killed
preparation never looks complete.

### Stage B: the three suites named in issue 910 (complete)

`cargo-bdd::cli`, `rstest-bdd::trybuild_macros` and
`rstest-bdd::feature_rebuild_invalidation` each gain a
`nested_fixture_preparation` test and gate their timed tests on
`ensure_prepared`. Assertions and assertion budgets are unchanged. The trybuild
dep-info assertion is tightened to compare against the start of its own run,
because a prepared tree means the artefacts are no longer necessarily fresh.

### Stage C: scheduling and contract (complete)

The preparation override is added first in `.config/nextest.toml` with a
30-minute period, the `cargo-spawning` group and `priority = 100`, and the file
declares `nextest-version = "0.9.91"`. `nextest_config.rs` pins the override's
position, period, termination settings, group and priority, and requires the
version. `make prepare-nested-fixtures` names the same tests in the `make test`
configuration for filtered runs.

### Stage D: the remaining trybuild suites (complete)

The Tokio and GPUI harness crates and the server crate each gained a
`nested_fixture_preparation` test, and each timed compile test gates on
`TrybuildTree::ensure_prepared`. All three describe their tree through one
shared `macrotest_support::TrybuildTree`, which resolves the root through
`cargo metadata` — the same route `trybuild` takes — and surfaces the generated
crate's directory so support files stage where `trybuild` reads them.

The generated crate is built at `<target>/tests/trybuild`, a directory the four
trybuild binaries share, so preparation is keyed per family rather than per
directory: whichever crate prepares first warms the dependency graph, and the
others find their own stamp missing and add theirs after compiling only what
their own generated manifest adds. Preparing one crate cannot vouch for
another's graph.

### Stage E: the mutation lane (complete)

Decision: accept the lazy path; do not add a preparation step to
`setup-commands`.

The reusable workflow at the pinned SHA runs
`cargo mutants --in-place --timeout-multiplier 3` from `mutation_run_cargo.py`.
`--in-place` tests mutations in the checkout itself rather than in a copy per
mutant, so the workspace `target/` persists across the whole run and costs paid
by one mutant are not re-paid by the next.

The trade is that under `cargo test` the first invocation that reaches a
preparation gate does the cold compile inside its own timeout. `cargo nextest`
cannot be assumed, because the mutation lane invokes `cargo test`; and
`ensure_prepared` deliberately prepares lazily there rather than failing, so
the run continues from a cold target instead of stopping.

Two costs are real and worth naming. First, that first invocation pays the cold
compile on its own timeout, exactly the failure mode issue 910 fixes for
nextest. Second, the preparation policy itself is mutable source:
`nested_cargo/preparation.rs`, `preparation/stamp.rs` and
`preparation/input_hash.rs` all carry mutants, so a run mutates the code that
decides whether to prepare — usually changing the stamp's error paths rather
than the happy-path gate. The consequence of the first is bounded by
cargo-mutants' `--timeout-multiplier 3`: a mutation whose test run exceeds
three times the baseline is recorded as a timeout and reported, not silently
discarded, and the baseline itself is measured on the same tree.

Rejected: extending `setup-commands`. It would need a second command in the
caller and a matching change to the pinned-string assertion in
`tests/workflow_contracts/mutation_testing_test.py`, and any target directory
it produced would still be revalidated by cargo-mutants' own first
`cargo test`. The one real saving — that cold compile happening before the
per-mutant timeout clock starts — is small against a per-mutant timeout already
set to three times the baseline, and the change would add a second place that
must track the fixture set. Rejected: excluding the affected tests from the
mutation run. `exclude-globs` names source files, not test selections, so there
is no supported filter for this, and excluding
`crates/rstest-bdd-harness/src/**` would drop the shared policy — the thing
whose tests should catch a mutant — from mutation coverage altogether.

### Stage F: documentation and evidence

The prose is written: `docs/testing-strategy.md` carries the "Cold fixture
builds belong to preparation, not to an assertion" section, and
`docs/developers-guide.md` describes the preparation override, the timeout
tiers, the `TrybuildTree` helper and the shared trybuild directory. Two further
subsections record the stamp-validity rules the later review added — "Stamps
live inside the artefact directory" and "Cargo configuration as a preparation
input" — so the next suite author inherits them as policy rather than
rediscovering them from the code.

The evidence is recorded in Progress. It comes from the Linux PR coverage lane
on a two-vCPU runner: preparation elapsed times, `Compiling` counts, timed-test
durations against their limits, and the rebuild counters, for both the cold and
the warm compiler-cache state. Notes say which figures are single-host
observations rather than CI measurements.

### Collected evidence (single-host, 6-core Rocky 10, cold nested target)

These are observations on the development host, not the CI runner, so they
understate the queueing a two-vCPU lane sees. The nested target directories
were empty when the run began; the workspace target was warm.

`make prepare-nested-fixtures` on a cold nested target, six families:

| Family                                           | Elapsed   | Budget |
| ------------------------------------------------ | --------- | ------ |
| `cargo-bdd::cli`                                 | 169.847 s | 30 m   |
| `rstest-bdd::feature_rebuild_invalidation`       | 166.902 s | 30 m   |
| `rstest-bdd::trybuild_macros`                    | 645.412 s | 30 m   |
| `rstest-bdd-harness-gpui::macro_compile`         | 304.252 s | 30 m   |
| `rstest-bdd-harness-tokio::macro_compile`        | 62.976 s  | 30 m   |
| `rstest-bdd-server::workspace_discovery_compile` | 864.239 s | 30 m   |

Total 2213.657 s for six tests run serially under the `cargo-spawning` group
(`max-threads = 1`), each on its own budget. The slowest family was the server
crate's trybuild tree at 864.239 s, 48 % of its 1,800 s budget; the
`rstest-bdd` trybuild binary's two graphs took 645.412 s, 36 %. All six
finished inside half of their budget despite a saturated compiler pool
(capacity 4, four slots held throughout, with waits of up to 115 compiles
ahead). The trybuild families share `<target>/tests/trybuild`, so the first to
run paid for the dependency graph and the others compiled only what their own
generated manifests added.

The timed assertions then ran against those prepared trees with no preparation
in their bodies:

| Test                                                                     | Elapsed   | Limit |
| ------------------------------------------------------------------------ | --------- | ----- |
| `cargo-bdd::cli::list_steps_runs`                                        | 2.223 s   | 180 s |
| `cargo-bdd::cli` smoke cases (12 others)                                 | ≤ 1.8 s   | 180 s |
| `feature_rebuild_invalidation::feature_addition_triggers_rebuild`        | 224.433 s | 600 s |
| `feature_rebuild_invalidation::bound_feature_file_is_tracked_dependency` | 53.304 s  | 600 s |
| `trybuild_macros` helper tests                                           | ≤ 0.022 s | 20 m  |

`list_steps_runs` is the issue's headline case: it took 2.2 s against a 180 s
limit, having previously exhausted that limit on a cold build. The rebuild
cases keep their deliberate recompiles and finish well inside 600 s.

Re-running preparation over a warm target costs seconds per family (15.2 s and
31.2 s observed for `trybuild_macros`), which is the stamp read plus Cargo's
own freshness check, so the idempotent path is cheap.

### A local-host measurement artefact, and why it is not a defect

A prepared-tree run of `step_macros_compile` timed out locally at exactly 1200
s, and an initial reading blamed *admission queueing*: roughly forty sequential
nested `cargo` invocations, each needing a compiler slot, with 836
`waiting for a compile slot` lines in the log. That reading was wrong, and the
correction matters for interpreting every local timing in this document.

On this development host `RUSTC_WRAPPER` is `build-limits-rustc`, which admits
compiler work into a pool of four shared by every agent on the machine. In CI
`RUSTC_WRAPPER` is `sccache` (`.github/workflows/ci.yml` writes it to
`GITHUB_ENV`), and there is no admission pool and no queue in front of the
compiler at all. The 836 waits came from other agents' builds saturating this
host, not from anything the test does; a `cargo clippy` invocation measured
`real 2m29s` against `user 0.311s` — the work was already cached, and the real
time was spent queued.

So the local timings above establish what they were collected to establish —
the cold dependency graphs now compile during preparation rather than during an
assertion, and the assertions reuse exactly those artefacts — but they cannot
show the suite fits its budget on the CI runner, because this host is not the
CI runner. The figures that carry over are the ones that describe work rather
than waiting: preparation elapsed times under contention (all far inside the
30-minute budget), and `user` time for the cached invocations.

The 20-minute `trybuild_macros` limit is unchanged from `origin/main` and the
per-invocation cost on an uncontended 2-vCPU lane is a fraction of the queued
cost measured here. The lane that can actually confirm criterion (c) is the CI
coverage lane, and that is where the remaining timing evidence belongs.

The second prepared-tree attempt is the clearest illustration of that gap. It
ran the gate (accepted, no `NotFound`), then worked through the fixture set in
order and was killed at 1200.018 s on the final `compile_fail` case, with 40
`trybuild*` bins built and no fixture failure. Its 187
`waiting for a compile slot` lines name crates that were already cached — the
pool, not the work, was the cost. The same run passed its per-fixture
assertions throughout, so what it demonstrates is that every timed case reuses
the prepared tree; it cannot demonstrate that the whole binary fits 1200 s when
nothing else is compiling.

## Concrete steps

All commands run from the worktree root.

```bash
make check-fmt
make lint
make test
```

For a focused run after a target eviction, prepare and assert under the same
environment. The stamp fingerprints `RUSTFLAGS` and the feature selection, so a
bare `cargo nextest run` after `make prepare-nested-fixtures` fails its gate:
the `make` target sets `RUSTFLAGS`, the bare invocation does not, and the two
describe different builds.

```bash
export RUSTFLAGS="-D warnings -Clink-arg=-fuse-ld=mold"
cargo nextest run -E 'test(/nested_fixture_preparation$/)'
cargo nextest run -p rstest-bdd --test trybuild_macros
```

Use the flags `make test` actually sets on the host. The second command then
passes the timed assertions in seconds, because the first already paid for the
dependency graphs.

## Validation and acceptance

- `make check-fmt`, `make lint` and `make test` pass.
- A cold-target run passes without any assertion timing out.
- `preparation_nextest_override_preserves_timeout_contract` and its two
  sibling override contracts pass, and the workflow contract suite under
  `tests/workflow_contracts` agrees with the configuration.
- The timed overrides keep the periods they had on `origin/main`.

## Idempotence and recovery

Preparation is idempotent: a warm stamp costs one read per family. Deleting a
stamped directory is safe and forces preparation on the next run. Deleting a
stamp alone has the same effect. If a preparation is killed, the temporary
stamp file it leaves beside the real one is ignored and overwritten.

## Artefacts and notes

Issue 910 records the three failures and their timings, including
`list_steps_runs` passing in 0.368 s after the same build was paid for in
advance, and `step_macros_compile` passing after 6,864 s when run directly.

## Interfaces and dependencies

`rstest_bdd_harness::nested_cargo` exposes `Preparation`, `PreparationSummary`,
`Sources`, `prepare_now`, `ensure_prepared`, `input_hash`,
`PREPARATION_COMMAND_TIMEOUT`, `MAX_SEQUENTIAL_COMMANDS`, `ProfileDestination`,
`running_under_nextest` and `count_compiling`. The stamp and hashing modules
stay private. `TrybuildTree`, which the harness and server crates share, lives
one level up in `macrotest_support` rather than in `nested_cargo`.
