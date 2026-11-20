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
- [x] **The preparation override's budget is stated for one family, but the
  test runs two — investigated and resolved as a local-host artefact, not a
  candidate defect.** `prepare_all` prepares `trybuild-macros` and then
  `ui-clippy`; only the second is a `run_bounded_with_timeout` command.
  `trybuild` spawns its own nested `cargo` and offers no hook to bound or
  capture it, so for `trybuild-macros` the *only* bound is the override's
  `slow-timeout` period. The contract test
  `preparation_command_bound_fires_before_the_runner_kills_the_test` checks
  `MAX_SEQUENTIAL_COMMANDS × PREPARATION_COMMAND_TIMEOUT < period` — 27 min <
  30 min — and passes, because it counts one family's commands and never counts
  the second family at all.

  The first local `make test` on `bc06a3d9` did fail
  `TIMEOUT [1800.011s] rstest-bdd::trybuild_macros
  preparation::nested_fixture_preparation`,
  with the log reading
  `nested preparation: family=trybuild-macros prepared in 1642.7s
  (0 Compiling lines)` —
  `trybuild-macros` finished with 157 s left of the period, so `ui-clippy`
  never started.

  The `0 Compiling lines` is **not** evidence that nothing compiled, and an
  earlier revision of this plan wrongly presented it as a load signal.
  `prepare_trybuild` returns `PreparationSummary::default()` because `trybuild`
  streams its nested Cargo's output to this process's own streams, so the unit
  count cannot be captured there (see the comment at
  `crates/rstest-bdd/tests/trybuild_macros/preparation.rs:323-326`). The zero
  is therefore the default, not a measurement: it reads the same whether the
  probe compiled nothing or compiled everything. What the log does establish is
  the elapsed time — 1642.7 s against a 1800 s period — and that `ui-clippy`
  never started. The queueing explanation rests on the separate
  `build-limits-status` observation recorded below (capacity 4, all held,
  ~80–115 foreign crates waiting), not on the unit count.

  **CI settled it.** Run `44673737187` on the same `bc06a3d9` ran the identical
  test green on the Linux lane:
  `PASS [127.575s] rstest-bdd::trybuild_macros
  preparation::nested_fixture_preparation`,
  and the lane finished `2470 tests run: 2470 passed, 7 skipped` in 400 s. The
  same lane also passed `bound_feature_file_is_tracked_dependency`, the
  coverage-lane case that had refused a stamp on the earlier head. So the
  30-minute period has ample headroom against real work (127.6 s); the local
  figure was 13× inflation from a pathologically saturated shared admission
  pool, and widening the period would size the budget for that host rather than
  for the job it protects. The override's own comment already states the
  intended contract — an overrun spanning several commands is reported by
  nextest as a slow-timeout, which is precisely a hang detector doing its job —
  and that contract is unchanged. **No budget change.** Recorded as resolved
  with the CI evidence retained above.

  **The same host artefact recurred, on a different test, and was settled the
  same way.** The local `make test` on `129e8c99` — after this change — reported
  `787/810 tests run: 786 passed, 1 timed out, 7 skipped`, the timeout being
  `TIMEOUT [1200.002s] rstest-bdd::trybuild_macros runner::step_macros_compile`.
  That is the trybuild binary's own 20-minute override, and the log shows the
  cause directly:

  ```text
  [build-limits] trybuild006: the managed sccache server is not serving
  compilations; compiling without the cache
  ```

  So the suite was compiling uncached *and* queueing behind foreign crates.
  `build-limits-status` at that moment reported `capacity 4, held 4, healthy`,
  `drain fault kind: live_work`, a `fallback recovery action:` line (automatic
  recovery pending, no operator action required), and roughly 25 waiting
  compilations from other agents' worktrees — femtologging, ortho-config,
  netsuke and wireframe among them.

  **CI again supplied the control.** Run `44694667108` on this exact commit
  `129e8c99` passed the same test on the Linux lane in `PASS [29.503s]`, with
  the lane finishing `2471 tests run: 2471 passed, 7 skipped` in 335.073 s. The
  same test therefore ran 40× slower here than on CI, on identical code. That
  comparison establishes the *inflation* between the two environments; it does
  not by itself apportion the 40× between admission contention and uncached
  compilation. Both were present — the `build-limits` lines show queue
  position, and the sccache-unavailable message shows the cache was not serving
  — and this log cannot say how much each contributed. Nor does it rule out a
  compilation-cost regression in the change: both runs used the same commit, so
  neither compares it with the code before it, and a regression that appears
  only under particular cache or load conditions would look the same. Deciding
  that needs a before-and-after run under comparable conditions, which was not
  taken. **No budget change**, and the gate was re-run rather than widened.
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
- [x] Clear the CodeScene regression the grandchild test introduced on
      `ac22bd22`. The check reported "Enforce advisory code health rules (1
      file with Large Assertion Blocks)" for
      `crates/rstest-bdd-harness/tests/nested_cargo_env.rs`, 10.00 → 9.39;
      `cs review` named four functions, and only the last
      (`run_bounded_returns_when_a_successful_child_leaves_a_pipe_holding_descendant`,
      line 345) is this branch's. `cs delta origin/main HEAD` supplied the
      mechanism the per-file score hides: the rule's threshold is three
      assertion *blocks* per file, the file already had three on `main`, and
      the new test's four consecutive asserts made a fourth. Merging the two
      stream assertions into one claim about `captured` — both streams are
      drained by the same collection, and the message still prints each — took
      the file back to 10.00 with `cs delta` reporting "Fixed issue". Verified
      the merged assertion is not vacuous: mutating the success-path window
      from `DRAIN_GRACE` to 60 s still fails the test at 30.01 s against its
      20 s bound, and the restored source passes 20/20 in 4.20 s.
- [x] Address the two findings from the CodeRabbit review of `02d33551`
      (`5850117480`). Both were valid and both were mine.
- [x] Fix `parse` splitting on the separator before matching the fingerprint.
      `render` writes `inputs` verbatim ahead of the separator line, so the
      delimiter's own spelling is reachable from a value. Reproduced by adding
      `a_stamp_admits_a_fingerprint_that_spells_the_separator` with
      `inputs: @@ recorded-artefacts` *before* changing `parse`: it failed on
      the first run, printing the stamp `render` had just written and `parse`
      had just refused. That is a self-invalidating stamp — the Nextest gate
      reports it missing or stale and `cargo test` prepares the family again —
      and it is the same defect class as the `wip/` finding earlier in this
      run, so it was fixed rather than argued down. `parse` now consumes the
      expected fingerprint and then the delimiter. I first transcribed the
      suggestion with `strip_line_break` on the remainder, which strips a
      *trailing* newline where a leading one was needed; 17 tests caught it
      immediately and the fix was `strip_prefix` on `'\n'`, tolerating a
      fingerprint that ends its own break. `fingerprint_props` now draws
      `OUTPUTS_SEPARATOR` as a value, and reverting `parse` to the split form
      fails `a_written_fingerprint_reads_back_as_itself` on generated input
      `variant = "@@ recorded-artefacts"` — so the property reaches this
      without the named test.
- [x] Correct the developers-guide example, which said `h` and `h` and called
      the fix "asymmetric". The first is a typo that made the example describe
      no failure at all; the second was wrong about the code, which applied
      `strip_line_break` to both sides. Both corrected, and — because the fix
      above changed the mechanism — the paragraph now explains why consuming
      the fingerprint before the separator cannot admit a stamp written for
      different inputs, and a second paragraph records the ordering rule.
- [x] Address the follow-up finding on `eed3752f`. Requiring the fingerprint
      and then the delimiter was not enough: I had written the newline between
      them as optional (`unwrap_or(rest)`), reasoning that `render` might omit
      it. `render` never omits it. Making it optional reintroduced the same
      class of stale acceptance from the other side, and worse: a stamp written
      for `inputs: h@@ recorded-artefacts` was admitted as one for
      `inputs: h`, because after consuming `h` the value's own remaining text
      still spells the delimiter. Reproduced with
      `a_stamp_for_inputs_ending_in_the_separator_does_not_admit_the_prefix`
      before the change, and the newline is now required. Also added
      `a_stamp_reads_back_whether_or_not_the_fingerprint_ends_its_own_break`
      for the pair the finding asked to retain. The mismatch property was
      rewritten too: it now *derives* the second `inputs` by appending the
      separator or a space, because two independent draws are almost never
      related as a value and one of its prefixes — measured, not assumed, as
      the first version of that rewrite still passed under the mutation. With
      the derived pair, the mutation fails on `inputs = ""`.
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

Run `42005188689` on `851c9b1c` — the head at the time, not the branch's final
head; later commits changed `preparation.rs` and the stamp protocol without
altering either measurement's subject — passed all three required lanes and is
the second, independent measurement. Its suite finished in 348.345 s (2101
tests, 2101 passed, 7 skipped) — 4.0× faster than the figure above. That
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
      surviving properties (`renaming_an_input_changes_the_hash`,
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
         The finding is correct and its proposed fix is implementable. The
         initial deferral rested on a false premise: that
         `unsafe_code = "deny"` blocks `process_group`, when
         `CommandExt::process_group` is a safe API. What makes it a follow-up
         is scope and risk: it is a behavioural change to the timeout path,
         outside issue 910; it needs a Unix path and a Windows job object, and
         the Windows lanes are the ones that already caught two
         under-specified predicates on this branch; and the lock-contention
         premise is unmeasured, which CodeRabbit itself recorded. The existing
         test proves the *call* returns in bounded time and deliberately does
         not assert the grandchild is gone. Tracked for a separate issue rather
         than folded in at the last gate.
- [x] Answer the review of `6905d6c3`: three inline findings and six pre-merge
      rows. Committed as `ac22bd22`, pushed, and every finding disposed with a
      reply: three inline threads, and a top-level reconciliation naming each
      row's outcome. Two rows are declined with reasoning recorded here rather
      than silently skipped.
      Gate sequence on the candidate: all twelve green — `make fmt`,
      `make check-fmt`, clippy (`-D warnings`), `cargo doc` (`-D warnings`),
      `make lint-whitaker`, `make lint-python`, the six lint scripts,
      `make spelling`, `make markdownlint` (127 files, 0 errors), the harness
      lib tests (109) and `nested_cargo_env` (20), and the Windows cross-target
      check. The first pass failed spelling on five of my own `-ise` forms,
      which is the en-GB-oxendict `-ize` rule the gate enforces.
      1. **A real stamp-validity bug, and the most valuable finding of the
         round.** `parse` compared `head.trim_end()` against
         `fingerprint.trim_end()`, and `render` wrote `trim_end`ed text, while
         the last field of a fingerprint is `inputs`. A stamp written for an
         input hash of `hash` therefore admitted a build whose hash was
         `hash`, so a changed fixture kept its prepared artefacts. Reproduced
         before fixing, by composing two texts differing only in that space and
         asking the real comparison predicate: both directions matched. The
         fix is `strip_line_break`, which removes the single terminating `\n`
         and nothing else, and `render` no longer normalizes on the way out.
         `a_trailing_space_in_the_inputs_does_not_reuse_another_build_s_stamp`
         pins it and fails against the old code.
      2. **The property that should have caught it did not, and the reason is
         worth keeping.** The first version of the round-trip property passed
         under a deliberate `trim_end` mutation. So did a second, after the
         generator was strengthened to emit trailing spaces. The blindness was
         structural, not statistical: the reject property varied an
         *environment variable* and held `inputs` fixed, so the whitespace
         never landed in the field the parser trims. A named regression on the
         exact field is what actually closes a boundary; a property over the
         same predicate can be arranged so the boundary is never reached.
      3. **The success path of `run_bounded` had the grandchild hazard too, and
         no test.** The existing grandchild case used `sleep 30 & wait`, so the
         direct child never exited. `run_bounded_returns_when_a_successful_child_leaves_a_pipe_holding_descendant`
         exits 0 while a descendant holds both pipes; against a mutated
         implementation that waits for EOF it fails after 30.00 s, which is the
         hang it describes.
      4. **Two documentation errors, both verified against source rather than
         against the finding.** The migration guide claimed `env_from_vars`
         drops an inherited `CARGO_TARGET_DIR`; `ensure_target_directory`
         returns early when one is present, and it is `env_for_directory` that
         drops it. The users guide said a lazy preparation is bounded by the
         per-command bound; that is false for trybuild, which has no per-command
         bound at all, so the two cases are now stated separately.
      5. **`prepare_now` recovery is now covered directly.** The unreadable-stamp
         cases exercised `ensure_prepared` under both runners but never the
         unconditional entry point, which has no runner to consult and must
         always replace. The new case also asserts the *replacement* is
         readable, since a recovery that wrote another unreadable stamp would
         re-prepare forever.
      6. **Three files were at or over the 400-line limit.** `stamping` reached
         412 and `inputs` 404 after the additions, and `stamp` sat exactly at
         400, so any later edit would have failed the gate. Split along seams
         that already existed: the round-trip cases into `stamp_file`, the
         unreadable-stamp cases into `unreadable`, and `render`/`write_stamp`
         into a production `stamp_file` module with `StampText::new` so the
         field stays private.
      7. **Deferred, with evidence, the filter-through-error row** (return the
         current-directory error through `absolute_target_directory` and
         `env_from_vars`). The finding is valid — a relative `CARGO_TARGET_DIR`
         that cannot be made absolute is silently left as-is — but changing
         those signatures reaches `build_child_env`, `env_for_directory`,
         `Preparation::new` and every test constructor, which is public API
         churn on a helper whose contract is documented as "leave it untouched
         rather than failing". Recording it as a follow-up rather than
         reworking the environment API in the last round before merge.
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

- [x] Close the six pre-merge rows CodeRabbit reported on `f3d05061`. Five are
      implemented across `6905d6c3`, `ac22bd22`, `5a1bd0d6` and `7dcbb059`;
      the sixth (the Makefile-driven end-to-end test) is answered by the
      harness-level sequence instead, with the reasoning recorded below.
      1. **The toolchain probe is injectable, and the boundary is tested
         through it.** `fingerprint` composed a fingerprint from a probe that
         production always supplied, so every existing test exercised only
         `compose` and a boundary that discarded the probe's error would have
         passed all of them. `fingerprint_with` takes the probe as a
         parameter, `ensure_prepared_with_probe` and `prepare_now_with_probe`
         expose it at the gate, and the non-injectable `fingerprint` is gone
         rather than left as a second place to swallow the error. Two new tests
         drive it: `a_changed_toolchain_invalidates_the_stamp_through_the_gate`
         prepares under one identity, then requires the next call under another
         identity to prepare again and the one after to be accepted; and
         `an_uncertain_toolchain_refuses_to_stamp_or_reuse_a_stamp` requires
         both entry points to propagate a failing probe without writing a
         stamp or running the closure.
      2. **`prepare_locked` logs a stamp it had to prepare over.** The previous
         `.ok().flatten()` was indistinguishable from a warm stamp that had
         merely missed, so a rebuild behind a corrupt stamp happened with no
         explanation. It now logs the post-lock read failure by name. It still
         prepares rather than propagating, and the comment says why: both
         callers have already decided to build, so refusing would leave a
         corrupt stamp with no remedy.
      3. **Property tests over the fingerprint's two relations.** The named
         cases pin one identity and one forged line break; the properties hold
         them over arbitrary values. `distinct_field_values_compose_distinct_
         fingerprints` draws both a variable name from the module's own lists
         and two values from an alphabet containing `\`, both line breaks, `:`
         and `=`, and requires different values to compose different text.
         `no_recorded_value_can_add_a_line_to_the_fingerprint` requires the
         line count to stay at one per recorded field however hostile the
         values are. Both were checked non-vacuous by replacing `escape`'s
         guard with `if true { return value.to_owned(); }`: the second then
         fails with `left: 7, right: 6` on a value of `"\n"`, and the first
         still passes — which is the point of having both.
      4. **The developers' guide claim about `cargo clean -p` was wrong and is
         replaced with the actual rule.** It said the per-package clean "does
         not trip the pairing". It does: the gate requires every name recorded
         from a dep-info stem to still be in `deps`, and a per-package clean
         removes exactly those. The replacement states the two rules, notes
         that `.fingerprint` is checked only for non-emptiness, and explains
         why the rebuild harness still works — it consults the gate once per
         process through `ensure_rebuild_prepared`, before its experiment's own
         `clean_fixture_units` evicts the units it is about to measure.
      5. **User-facing docs: the runner limits differ, and the migration guide
         now says so.** The users' guide previously implied the per-command
         bound was the whole story; it now states that Nextest gives the
         preparation tests a 30-minute `slow-timeout` while plain `cargo test`
         imposes no per-test timeout at all. The v0.7.0 migration guide gains a
         "Nested fixture preparation is a separate pass" section covering
         `ensure_prepared` versus `prepare_now`, the fail-fast behaviour under
         Nextest, the resolved-`CARGO_TARGET_DIR` rule, a checklist item, and
         the fail-fast error with its remedy.
      6. **The filtered-Nextest end-to-end row is answered at the harness
         level, not with a Makefile-driven test.**
         `a_family_prepared_by_the_preparation_test_satisfies_the_nextest_gate`
         already runs the sequence the row asks for — unprepared timed test
         refused, `prepare_now` builds, reopened timed test accepted — against
         a real temporary tree. A Makefile-driven variant would spawn a second
         Cargo build of this workspace inside the suite, which is the exact
         cold-work-inside-a-timed-test shape issue 910 exists to remove, and it
         would assert on `make` rather than on the gate. The harness-level test
         covers the same three steps without either cost.

- [x] Split `preparation.rs` when it crossed the length gate: the lifecycle
      (`ensure_prepared`, both probe-injectable forms, `prepare_now`,
      `prepare_locked`, `lock`) moved to `preparation/lifecycle.rs`, leaving
      the description types, the constants and the re-exports behind. The file
      went from 446 lines to 265; `lifecycle.rs` is 232. `fingerprint_props.rs`
      is a separate test module for the same reason.

- [x] Replace a test that could not fail, after review found the coverage
      claim false. `a_stamp_reads_back_whether_or_not_the_fingerprint_ends_its_own_break`
      hand-built a bare stamp and offered it to the break-terminated text
      `compose` had produced. `parse` removes the *expectation's* own break, so
      the comparison took the ordinary path and the bare case was never
      reached: deleting the `render` newline guard left the test green.
      `Fingerprint`'s field is private to the stamp module, so that module
      cannot spell a bare expectation at all. The bare case moved to
      `stamp::tests`, where `fingerprint_of` builds one directly, and the
      remaining test here was narrowed to the claim it can actually make.
      Verified by mutation: removing the guard fails the new test with
      `"inputs: h@@ recorded-artefacts\n…"`. Committed as `a2be2d35`.

- [x] Answer the two documentation findings on the same round: a first-person
      clause in this plan and a second-person clause in the users guide
      (`build_child_env`'s fallback paragraph, line 2770). Both rewritten in
      the impersonal voice and rewrapped to the 80-column limit;
      `make markdownlint` reported 127 files and 0 errors afterwards.

- [x] Make the target-directory resolution fallible, after review pointed out
      that it swallowed an I/O error. `absolute_target_directory` joined a
      relative `CARGO_TARGET_DIR` to `current_dir()` and, on failure, returned
      the *relative* path with a comment saying the build would report it
      better. It would not: the child resolves a relative value against the
      fixture directory it runs in, so the silent fallback pointed the build
      and the stamp at two different trees. `absolute_target_directory`,
      `build_child_env`, `env_from_vars`, `NestedCargoEnv::target_directory`,
      `Preparation::new`, `Preparation::for_fixture` and `env_for_directory`
      now return `io::Result`. The lookup is injected
      (`absolute_target_directory_with`) because no portable fixture can make
      the process's own working directory unreadable; the three new unit tests
      drive the absolute, resolved and failing cases. My earlier reason for
      declining this row was wrong and is recorded under Surprises.

- [x] Pin the `prepare-nested-fixtures` filter against the nextest override.
      First written as a Rust test in `nextest_config.rs`, which pushed that
      file to 455 lines and failed the 400-line gate. Moving the parsing into
      `nextest_config/support.rs` left it at 415, still over. The gate was
      right and the placement was wrong: the repository already reads Makefile
      recipes as data in `scripts/tests/test_users_guide_links_makefile_contract.py`,
      so the contract now lives in
      `scripts/tests/test_prepare_nested_fixtures_makefile_contract.py` with
      that file's `recipe` helper. It asserts the nextest invocation, the exact
      `-E` filter, the shared `RUSTFLAGS` and the `.PHONY` declaration.
      Nextest exits zero when a filter matches nothing, so a drifted filter
      would prepare nothing and report success. Verified by mutation: changing
      the Makefile's `$$` to `$` fails the filter assertion with both spellings
      printed.

- [x] Document the bounded drain and the fallible constructors. The users
      guide now states that a successful run's captured output is the direct
      child's rather than the process tree's, and that `env_for_directory` is
      internal to `Preparation::for_fixture`; the v0.7.0 migration guide
      carries the same two points plus a checklist entry for the signature
      change.

- [x] Make the CLI preparation test prove it built the fixture, not merely
      that it succeeded. `nested_fixture_preparation` now asserts both of the
      fixture's own test binaries are in the profile the timed tests read, via
      `fixture_binaries_present`. The witness is covered on three sides: a
      tree holding one of the two (and otherwise only unrelated units) is
      rejected, both present is accepted, and an absent `deps` is rejected —
      so the assertion cannot pass by always answering either way.
      The reviewer's literal request was a run against an *isolated* target.
      That was considered and declined, on a cost that is the whole subject of
      this change: a fresh target means compiling the fixture's full dependency
      graph (205 crates, measured with `cargo tree --offline`) inside the very
      preparation this issue exists to make cheap, and the family's `Compiling`
      output would then be dominated by that graph rather than by the fixture.
      The property the isolated target was there to prove — that a no-op
      preparation cannot satisfy the check — is proved instead by the negative
      witnesses above, which exercise the same predicate the timed assertion
      depends on without building anything.
      First draft of the positive control wrote each unit's output as a bare
      stem, and it failed: the witness requires `-` after the stem, which is
      what Cargo actually emits (`diagnostics_fixture-0b30ca8d7f4fe548`). The
      test caught the fixture rather than the code. Verified against the real
      tree afterwards: the warm `target/debug/deps` holds
      `diagnostics_fixture-0b30ca8d7f4fe548` and
      `unsupported_dump_steps-e8af36137db6ef48`, and holds
      `diagnostics_fixture-0cbdf3c87e1a0917.d` as well — which the witness
      correctly ignores.

- [x] Reconcile the filtered-Nextest row against what already covers it. The
      requested sequence — unprepared target fails, `make
      prepare-nested-fixtures` runs, the same filtered test then passes —
      already exists as
      `a_family_prepared_by_the_preparation_test_satisfies_the_nextest_gate`
      in `preparation/tests/stamping.rs`, which runs exactly those three steps
      against one scratch directory, reopening the family from a fresh
      description so the pass cannot lean on in-process state. What was *not*
      covered is the link from the Makefile target to the nextest filter, which
      the Python contract added above now pins.
      A literal test shelling out to `make prepare-nested-fixtures` was
      considered and declined: the target runs `cargo nextest run` over all six
      preparation tests, so a test that invoked it would nest nextest inside
      nextest, rebuild this workspace's fixtures from inside the
      `cargo-spawning` group whose `max-threads = 1` serializes them, and
      recurse into the very budget the target exists to spend once. There is no
      nested-nextest precedent anywhere in the workspace. The compositional
      alternative — spawn `cargo nextest run` on one small filtered suite — was
      also declined for the same recursion at smaller scale.
      Review pressed the point afterwards, correctly: an in-process scratch
      sequence and a Python assertion over recipe text do not establish that
      the *filtered* workflow works across processes. It was then measured
      directly, against the real workspace target and the real configuration,
      as three separate nextest invocations rather than one nested run. With
      `target/.cargo-bdd-cli.rstest-bdd-prepared` moved aside,
      `cargo nextest run -p cargo-bdd --test cli -E 'test(/^list_steps_runs$/)'`
      failed in 0.060 s with "nested fixture artefacts for family `cargo-bdd-cli`
      are not prepared … is missing or stale". `make prepare-nested-fixtures`
      then passed all six families in 408.051 s, and the same filtered command
      passed in 2.092 s.
      Steps 1 and 3 are separate processes from each other and from step 2, run
      with the same `RUSTFLAGS` and feature selection, against the same target.
      That is the requested sequence in full: the negative half fails for the
      right reason, preparation is what changes the outcome, and the positive
      half does not rebuild. It is not a repository test, and must not become
      one — the same recursion objection applies to automating it. Feeding it
      to CI would mean a job that deletes a stamp inside a shared target. The
      same evidence exists on hosted CI without any intervention. Run
      `43630984118`'s Linux lane shows `cargo-bdd::cli preparation::nested_fixture_preparation`
      passing at index 937 and the gated `cargo-bdd::cli list_steps_runs`
      passing at index 2212 of the same 2322-test run: different nextest
      processes, one prepared tree, and the timed assertion finishing in
      0.444 s rather than paying for the cold graph.

- [x] Clear the lint fallout from the now-fallible target resolution. The
      fallible `env_from_vars` left three violations in the CLI fixture, all
      caught by `make lint` and none by a plain `cargo check`:
      `case_sensitive_file_extension_comparisons` on `ends_with(".d")`,
      `indexing_slicing` on `present[index]`, and `expect_used` on the
      `.expect(...)` in `cli_child_environment` and in
      `fixture_binaries_present`'s caller. The first two were rewritten by
      name (`strip_suffix(".d")`, `get_mut`); the third is the interesting
      one. `clippy.toml` sets `allow-expect-in-tests = true`, and it does
      **not** cover a plain `fn` in an integration-test crate: only
      `#[cfg(test)]` modules and `#[test]` bodies. `cli_child_environment`,
      `fixture_binaries_present` and the rstest case body all had to return
      `Result` — which also means the stone `#[rstest]` body needed a trailing
      `Ok(())` — so the `expect` moved back to the assertion sites that are
      genuinely in test bodies.

- [x] Read the 2026-11-11 `make test` red as the documented queue-starvation
      mode, not a regression. The gate reported 2191/2299 passing with
      `preparation::nested_fixture_preparation` the only failure, and its
      stderr was the bound itself:
      `nested cargo exceeded its 540 s wall-clock bound; the child was killed`,
      with the captured output showing the nested build still compiling
      dependencies behind waits of ~100 compiles ahead. That is the failure
      already analysed under Surprises, first seen 2026-11-09 and reproduced by
      stashing the whole diff on clean `HEAD`. Three independent facts confirm
      this occurrence is the same thing rather than something new: the bound
      reports the command and the captured output instead of hanging; the
      family's stamp exists at `target/.cargo-bdd-cli.rstest-bdd-prepared` with
      recorded fingerprint `e1b18c15de4ee742`; and both fixture binaries are
      present in `target/debug/deps`, so the artefacts the timed tests read
      exist. The failed attempt also recompiled them with a new hash
      (`diagnostics_fixture-f7372c1b4a9607cf`), which is why the next `make
      test` needs a real build rather than a stamp hit.

- [x] Close the last three gate failures on the settled tree, all introduced by
      this branch rather than inherited. `make spelling` rejected
      `recognised`/`recognise` in the new witness test, which en-GB-oxendict
      requires as `-ize`; both renamed. `make lint` then found the same
      `expect_used` class in `rstest-bdd-harness/tests/nested_cargo_env.rs`,
      where `caller_env` and `scratch_env` unwrapped the now-fallible
      `env_from_vars`: both now return `io::Result`, the eleven affected
      `#[test]` bodies return `Result` and end in `Ok(())`, and the chained
      `redirected_profile_paths_are_unique_per_call` uses `?` before `.get`.
      That pushed the file from 397 to 411 lines and tripped the 400-line gate,
      so its four shared fixtures moved to
      `tests/nested_cargo_env/fixtures.rs` behind the same `#[path]` submodule
      arrangement `cargo-bdd/tests/cli.rs` already uses; the file is now 386
      lines. `make lint` also caught an intra-doc link from the public
      `absolute_target_directory` to the private
      `absolute_target_directory_with` under
      `-D rustdoc::private-intra-doc-links`; the sentence now describes the
      error instead of linking to it. Its own wording is what surfaced the
      lesson: the reason a helper is fallible belongs at the call site, not
      duplicated into every fixture.

- [x] Separate the two gate failures that this host produces from defects.
      `make test` on the settled tree improved to 2193/2299 with the CLI
      preparation now **passing** in 105.102 s and rebuild-invalidation in
      503.462 s; the only failure left was
      `rstest-bdd::trybuild_macros::nested_fixture_preparation`, timed out at
      nextest's 1800 s cap. That is the documented partial-preparation mode,
      and the filesystem says so directly: the family's trybuild stamp exists
      at `target/tests/trybuild/.trybuild-macros.rstest-bdd-prepared`
      (written 23:43) while its sibling UI-lint target at
      `crates/rstest-bdd/target/tests/ui_lints_clippy` holds 256 MB of
      `debug/` and **no stamp at all**. So the dependency graph compiled and
      the later `cargo clippy` command was what ran out of budget — the same
      shape recorded for 2026-11-09, when this family "prepared and stamped
      its family in 1195.9 s, then lost a later bounded `cargo clippy` command
      to the same 540 s bound". `make prepare-nested-fixtures` then completed
      it: trybuild_macros passed in 488.473 s, with CLI at 0.444 s and
      rebuild-invalidation at 0.775 s — stamp hits, which is the idempotent
      path the design predicts. GPUI took 721.099 s and tokio 258.708 s, both
      inside their 1800 s budgets. The sixth family was cut short by the outer
      `timeout 1500` I wrapped around the run (SIGTERM at 26.294 s), which is
      my own harness limit rather than a test result; it is the one family
      documented at 864 s cold and is being prepared on its own.

- [x] Show the last rebuild-invalidation failure was a wiped `target/`, not a
      defect. The 2026-11-12 run failed
      `feature_addition_triggers_rebuild` at 305.451 s on the harness's own
      300 s bound, which read as a possible defect in the fallible-resolution
      plumbing. Two measurements ruled that out. First, a solo run was
      SIGTERM'd at 105.249 s by session teardown while this workspace was still
      compiling its own test binary (`Finished test profile ... in 13m 12s`),
      so it never reached a verdict; the diff to `harness/process.rs` is only
      `io::Result` plumbing through `build_child_env`, `child_env_from_vars`,
      `resolve_target_directory` and `preparation_view`, with the 300 s bound
      and the rebuild work untouched. Second, the retry failed in 0.040 s with
      the *stamp-missing* message, which is the gate working: the whole
      `target/` tree had been recreated at 11:16 and every stamp was gone,
      including `target/tests/feature-addition/`. That run also carried my own
      mistake — `RUSTFLAGS="-D warnings -Clink-arg=-fuse-ld=mold"` alone,
      without the `--all-features` the fingerprint covers, so the stamp would
      have been stale regardless. `make prepare-nested-fixtures` then prepared
      all six families in 117.832 s (CLI 5.699 s, rebuild-invalidation
      4.569 s, trybuild 45.264 s, GPUI 8.128 s, tokio 9.683 s, server
      44.485 s), and the test passed in **5.956 s** against its documented
      1.745 s. The lesson is the one this ExecPlan already states in another
      place: read the stamp before reading the timeout, and run the timed
      families through the Makefile's own `RUSTFLAGS` and feature selection,
      not a hand-rolled approximation.

- [x] Stop reading a *replaced* unit as an evicted one, which the exact layer
      did on every run of the full suite. `make test` failed at
      `rstest_bdd::feature_rebuild_invalidation::feature_addition_triggers_rebuild`
      with `nested fixture artefacts for family feature-addition are not
      prepared (…/target/tests/feature-addition/.feature-addition.rstest-bdd-prepared
      is missing or stale, and the artefacts it vouched for are gone
      (…/target/debug (missing deps/invalidation-208b4c614aa6ee86,
      deps/librstest_bdd_rebuild_invalidation_fixture-6287f0c96ae1b378.rlib,
      …rmeta, deps/rstest_bdd_rebuild_invalidation_fixture-83897a1060c95132)))`,
      2246 passed and 1 failed out of 2314. The four names were the *sibling*
      fixture's units, and no rule about eviction was involved: both scratch
      families stamp the **shared** `<target>/debug`, so each family's stamp
      records every unit in the profile, and
      `clean_fixture_units` runs `cargo clean -p
      rstest-bdd-rebuild-invalidation-fixture` by design before each
      experiment's baseline. Cargo rebuilt those units under new metadata
      hashes — `invalidation-208b4c61…` became `invalidation-082d5309…`,
      `librstest_bdd_rebuild_invalidation_fixture-6287f0c9…` became
      `-4cb94ee2…` — so every recorded name for them looked absent at once
      while the profile held all four units. Measured before changing anything:
      each missing name had a same-form replacement differing only in the hash,
      and the stamp's own mtime (13:17:10) was *later* than the profile's
      (13:16:19), which ruled out "the tree went after the stamp" and left
      "the tree was rebuilt" as the only reading. `artefacts/replacement.rs`
      now splits a recorded name into directory, stem, hash and suffix, and
      accepts a name as vouched for when a present name matches all but the
      hash. The output form must match too, because excusing any same-stem
      entry would reopen the hole the exact layer exists to close — an `.rlib`
      removed while its `.rmeta` survived is exactly what
      `an_rlib_removed_beside_a_surviving_rmeta_is_an_eviction` refuses, and
      that case still passes. The two-layer check was right that something had
      changed; what it lacked was the distinction between a unit that was
      *deleted* and one that was *replaced under a new name*, which is the
      normal consequence of the per-package clean this harness performs
      deliberately. Lesson: a predicate over another tool's on-disk names must
      account for that tool's *own* renaming behaviour, not only its deletions,
      and the failure to look for is the one where a rebuild is
      indistinguishable from an eviction.

- [x] Repair the second CodeRabbit round on head `f9718e69`: the per-unit
      fingerprint records, the failures a recompile is allowed to excuse, the
      directory-boundary and entry-kind markers in the input hash, the
      triple-profile artefact root, the withdrawal of a refuted stamp, the
      profile-anchored binary fallback, and the build-mode/check-mode trybuild
      probes. Findings `4577395343` (withdrawal), `4577395347`, `4577395352`,
      `4577395358`, `4577395363`, `4577395367`, `4577395370`, `4577395384`,
      `4577395390` and `4577395395` are repaired in the tree;
      `4577395374` (`cap_std`) is deferred to issue #946 and the
      process-tree finding to the already-open issue #923.
- [x] Repair the `Maintainability` findings `4577395376` and `4577395379`:
      the shared preparation fixtures and the rebuild-invalidation setup
      helpers return `Result` and propagate with `?`, with `.expect(...)` left
      only at the consuming `#[test]` boundary.
- [x] Split four files that this work pushed past the enforced 400-line budget
      (`scripts/check_rs_file_lengths.py`, wired into `make lint`):
      `preparation/input_hash/tests.rs`, `preparation/artefacts/tests/records.rs`,
      `preparation/tests/target_profile.rs` and
      `cargo-bdd/tests/cli/preparation/tests/withdrawal.rs`. Every case moved
      verbatim; the harness preparation suite went from 99 to 100 cases, the
      one addition being the per-unit-record regression.

- [x] Close the seventh stamp-validity gap: **a family vouched for units it
      never built.** Found by the isolated Nextest lane on `649e9ec5`, where
      `feature_addition_triggers_rebuild` failed with "[the stamp] records
      artefacts it vouched for are gone". The mechanism was pinned by
      reimplementing `discover`, `outputs_in`, `recorded_name`, `UnitName` and
      the replacement rule in a scratch script against the actual tree the
      failure left behind: the addition family's stamp held **11,315** names
      for a profile then holding **11,299**, and **`replacements` was 0**. The
      superseded rule introduced at line 1286 exists for exactly this case, and
      could never fire, because the stamp had recorded *both* generations of
      the rebuild fixture's units. Sixteen names were reported missing, and
      every survivor was itself recorded five to six times.
      1. **The distinguishing fact is provenance, not tree state.** At check
         time a unit is present or absent, and a unit removed by a sibling's
         designed `cargo clean -p` is indistinguishable from one removed by a
         stray `rm`. No predicate over on-disk names can separate them — the
         `replacement` module's own documentation says so, and the measurement
         above is the proof. Only the family knows which packages it built.
      2. **`Preparation::owned_packages` declares that.** Empty for
         `Preparation::for_fixture`, which sets `CARGO_TARGET_DIR` to the
         directory it stamps so nothing else writes there; one package each for
         the rebuild, addition and CLI families, which all build into the shared
         workspace `target/debug`. This *corrects* the claim recorded at line
         588-605 that the intentional fixture-package clean "is untouched" — it
         was untouched only because the gate then checked directory existence,
         and ceased to be once the exact layer recorded names.
      3. **`artefacts/ownership.rs` answers the matching question**, by path
         component, accepting both Cargo spellings (`<package>-<hash>` in
         `.fingerprint/`, `lib<underscored>-<hash>.rlib` in `deps/`) and
         stripping the metadata suffix only when exactly sixteen hex characters
         follow the final `-`. A prefix test would claim `rstest-bdd`'s units for
         the `rstest-bdd-feature-addition-fixture` family.
      4. **The rest of the profile is not abandoned.** The structural layer
         still refuses a profile that lost its units or its fingerprint store —
         the states `cargo clean`, `rm -rf <target>/debug` and a cache eviction
         leave — so the shared dependency graph stays covered.
- [x] Regression coverage, in `artefacts/tests/siblings.rs` (four cases) and
      `artefacts/ownership/tests.rs` (six): the sibling's units are not
      recorded; the sibling's designed clean does not refuse the stamp; the
      family's own eviction still does; and a family naming no package still
      claims the whole profile, which is the *before* picture that reproduces
      the original defect.
- [x] Clear the lint fallout from moving those cases: three clippy errors in
      the relocated test modules (`case_sensitive_file_extension_comparisons`
      twice, `cloned_ref_to_slice_refs` once) and two rustdoc intra-doc links
      that pointed at private or non-existent items. `make lint` runs
      `cargo doc --workspace --no-deps` after clippy, so clippy passing is not
      the gate passing.
- [x] Fix a heading this work introduced that contradicted its own body: the
      migration guide read `### A bounded run's captured output is the direct
      child's`, while the two paragraphs under it explain that the capture is
      "neither exactly the direct child's text nor the whole tree's". Verified
      against `git show 7dcbb059` that this PR added the heading, so the
      contradiction is ours and not inherited. It now reads
      `### A bounded run's captured output stops at a deadline`, matching the
      body's actual claim. A reader who stopped at the heading would have
      believed the opposite of the documented behaviour.
- [x] Answer the fourteen CodeRabbit threads the review of `b9cb1d3b` raised,
      and clear the three CodeScene findings the same head produced. The
      threads were verified against the source at `fdcfdff9` before any reply,
      which separated twelve real repairs from two that needed a different
      disposition. `4503927232` (terminate the whole process tree on timeout)
      keeps its deferral to #923: `bounded.rs` is new on this branch but the
      `child.kill()` it holds is the same call `main`'s `nested_cargo.rs`
      already makes twice, the finding's own text says its lock-contention
      premise was unverified, and terminating a process group is a change to
      shared behaviour that deserves the measurement #923 asks for. The
      `4577395374` cap_std finding keeps its deferral to #946 for the reason
      recorded there: a faithful conversion has to establish the capability at
      `Preparation::directory` and thread it through six files, two of which
      have no mechanical `cap_std` equivalent.
      The three CodeScene findings were all in code this branch introduced, so
      they were fixed rather than dispositioned: `recorded_names` split into
      `unit_names` and `record_names`; `fixture_preparation`'s five arguments
      replaced by a `Family` value carrying the four that differ per fixture;
      and the duplicated idempotency pair in `stamping.rs` driven through one
      `runs_over_two_calls`. Coverage is unchanged — 110 preparation tests and
      33 `feature_rebuild_invalidation` tests pass — and the commit is
      `aec0c202`.
- [x] Spell the two names the spelling gate refused. `recognised` became
      `recognized` and `familys` became `family_s`, both in test names rather
      than in prose, so the fix is the name and not a local allowlist entry.
      The gate's dictionary resolves the `-ise` form to `-ize` here, which is
      the opposite direction from the en-GB-oxendict rule the repository
      otherwise follows; that is a dictionary question, so it is left as the
      dictionary decides. Commit `770d8616`.
- [x] Merge `origin/main` after #945 landed. That PR fixed the same two Cargo
      1.100 layout problems this branch had worked around independently, so
      three files conflicted. Each was resolved by keeping the stronger
      solution: main's pure `profile_dir_of_test_executable` for the profile
      question, with this branch's `fallback_binary_path` delegating to it;
      this branch's `staging/target_root.rs` module, with main's extra layout
      cases ported into its test file against a real tagged tree; and this
      branch's `underscore_fixture.rs`, which keeps the lint firing on every
      toolchain. Main's `staging/executable_layout.rs` is dropped as
      superseded. `cargo check --workspace --all-targets --all-features`
      passes on the merge. Commit `bdbf4ed4`.
- [x] Fix the merge commit's shape. The first attempt, `b4d0ecef`, carried the
      right *tree* but only one parent: `git merge --no-commit` had left a
      `MERGE_HEAD` that was gone by the time `git commit` ran, so an ordinary
      commit was written and `d70f37c1` never became an ancestor. The tree was
      correct and GitHub still reported the PR `CONFLICTING`, which is what
      caught it — a conflict state is about history, not content. Rebuilt from
      the unstaged tree as `bdbf4ed4`: stage everything, write `MERGE_HEAD`
      with `d70f37c1` at the path `git rev-parse --path-format=absolute
      --git-path MERGE_HEAD` reports, then commit. Two lessons. First, the
      path is not `$(git rev-parse --git-dir)/MERGE_HEAD` — this worktree's
      git-dir is a linked-worktree directory under the bare repo, and joining
      by hand produced a file git never read. Second, verify the shape rather
      than the label: `git log -1 --format='%h parents=%p'` and
      `git merge-base --is-ancestor origin/main HEAD` are the check, and
      neither was run against `b4d0ecef` before it was pushed.
- [x] Re-run the full gate sequence on the merged head through the
      scrutineer, sequentially and without a competing run in this worktree.
      The earlier `make test` failure at `2afbbf91` in
      `feature_edit_forces_rebuild_and_fresh_failure` was a stamp refusal
      naming two fingerprint generations absent from the tree, and it did not
      reproduce: a serialized run of the preparation plus all three
      `feature_rebuild_invalidation` scenarios passed 4/4 in 305 s. That run
      overlapped this worktree with the gate itself, and both processes run
      `cargo clean -p rstest-bdd-rebuild-invalidation-fixture` before their
      baselines, so the overlap is the leading explanation. On `64e50d5b` the
      clean serialized run confirmed it: `check-fmt`, `markdownlint` and
      `typecheck` all exited 0, and `make test` ran 2245 of 2368 tests with
      **2244 passed and zero assertion failures**. The one non-pass was
      `rstest-bdd::trybuild_macros preparation::nested_fixture_preparation`,
      killed by nextest's 30-minute override — not a failure of the test. It
      self-reported completion (`prepared in 1337.1s`, stamp written), the
      ceiling fired 463 s later with nothing logged in between, and the same
      test run alone afterwards passed in **352 s**. The log carries 178
      `[build-limits] ... waiting for a compile slot` lines, one naming 158
      crates ahead, so admission was saturated throughout. How much of the
      elapsed time that accounts for is not something this log can settle: the
      `[build-limits]` lines show waiting happened and show its position in
      the queue, not how long each wait lasted. The "0 Compiling lines" in the
      preparation's own summary does not help either, and an earlier revision
      of this plan was wrong to cite it — `prepare_trybuild` returns
      `PreparationSummary::default()`, so the zero is a default and not a
      measurement. What the log does establish is the elapsed time and that
      the same test passes in 352 s alone and 29.5 s on CI. This is a
      bounded-wait expiry under shared admission, which the operating
      contract classes as a wait and not an impasse.
- [x] Clear the `// weave: run 'weave explain ...'` annotations the conflict
      resolution left in resolved files. Four were present, not three, and
      `make check-fmt` named two of them as formatting diffs. They are
      resolution scaffolding, not source. Commit `bdbf4ed4` by amendment.
- [x] Re-run the gates, which found a real formatting failure and a real
      CodeScene finding rather than a clean tree. `make check-fmt` reported
      two committed formatting diffs from the hand resolution — a wrapped
      call argument, a one-line signature, and an import list in the wrong
      order — all mechanical and applied with `cargo fmt` rather than by
      hand. The CodeScene check is the **only** failing check on the PR and
      reported Code Duplication in `stamping.rs` at 9.38, naming
      `lazy_preparation_writes_the_stamp_and_is_idempotent` and
      `prepare_now_runs_under_nextest_and_stamps_once`. That was still
      present after `aec0c202`'s `runs_over_two_calls` extraction: the pair
      differed only in the entry point and a two-line fixture repeated
      verbatim, so the earlier fix had gone one level too shallow. Because
      `ensure_prepared_with` and `prepare_now` take the same
      `FnOnce() -> io::Result<PreparationSummary>`, the property is one of
      two functions; an `EntryPoint` value and a two-case `rstest` table now
      drive both. `cs review` scores the file 10.00 with no findings and both
      cases pass. Commits `26980c26` and `4c596fbd`.
- [x] Re-run the gates on `4c596fbd` and record each gate's result. Stopping
      the first attempt after `check-fmt` failed was deliberate: `make test` on
      this workspace takes far longer than the two-line source change that then
      became necessary, so gating to completion before fixing it would have
      produced evidence for a tree that could not be pushed. The second attempt
      reached `lint` and found two real errors on the dependency fix, both
      recorded below.
- [x] Clear the two `lint` errors the dependency-vouching commit introduced.
      `Module eviction spans 430 lines, exceeding the allowed 400` was the
      regression case pushed into a module already near the cap; it now lives
      in its own `tests/ownership.rs`, and the two helpers it needs moved up
      into `tests/mod.rs` so the rule is stated once. Fixing that then took
      `artefacts` from 400 to 401 through the doc paragraph the contract needs,
      so the paragraph was tightened to 399 — the module is again under the cap
      rather than pinned to it. The second error,
      `no_unwrap_or_else_panic` on `std::result::Result<(), std::io::Error>`,
      is the `unwrap_or_else` that performed the same panic as `expect`; it
      now says `expect` and names the entry point. `whitaker --lib
      whitaker_suite` over the harness crate exits 0, `cargo nextest run -p
      rstest-bdd-harness` passes 201 of 201 including the new ownership case,
      and `cs review` scores all five touched files 10.00. Commit `769933fa`.
- [x] Run the gate sequence on the head that carries the ownership split.
      `make check-fmt`, `make lint` and `make typecheck` all pass. `make test`
      passed 2238 of 2361 and failed one test,
      `rstest-bdd::trybuild_macros::preparation::nested_fixture_preparation`,
      which is the queue-starvation mode this plan already documents rather
      than a defect: its captured output is 40+ `[build-limits] the supervisor
      is restarting; retrying admission` lines and `waiting for a compile slot`
      depths of 130–170, and the reported cause is the family's own 540 s
      per-command bound, not an assertion or a stamp refusal. `build-limits-status`
      reports `capacity 4, held 3, healthy, caching through sccache` with other
      agents' work queued, so admission itself is healthy and this is not the
      unhealthy state that would be reported as an impasse. A focused re-run of
      the same binary reproduced the same picture: the preparation passed at
      489.632 s (inside its 540 s bound, so the bound is being spent in the
      queue rather than on work), and the timed `step_macros_compile` then hit
      its unchanged 1200 s budget with `the supervisor is restarting` throughout
      and both stamps reported prepared. No assertion deadline was widened and
      no test was changed to accommodate it; the hosted lanes are what establish
      the suite fits its budget on a runner with no admission pool.
- [x] Push the head that carries the ownership split and re-observe CI.
      The push had to replace the remote, not extend it: the tracking ref sat
      at `c6945f24`, which descends through `b4d0ecef` — the single-parent
      merge that HEAD replaced with `bdbf4ed4` — and the PR reported
      `mergeStateStatus: DIRTY`. A `--force-with-lease` against the exact
      remote SHA, over the explicit SSH URL and through an environment
      rebuilt key by key (the harness supplies a
      `url.https://github.com/.insteadof git@github.com:` rewrite through
      `GIT_CONFIG_*`, which a shell `unset` of the named variables leaves in
      place) replaced it with `5a94af5c`. Nothing was lost: `c6945f24`
      touched only this plan, and both of its entries are present here.
      The tracking ref was refreshed with a leading-`+` refspec and the
      upstream set to `origin`; `ls-remote` and `rev-parse HEAD` agree on
      `5a94af5cb4ddb512da60aa0767dd6a162f5e6af9`. The PR is now
      `mergeable: MERGEABLE`, so the bad merge was the whole of the
      conflict, and the three `build-test` lanes plus CodeScene are queued
      on the new head.
- [x] Clear the Whitaker finding CI reported on `9cbff182`, which `fdcfdff9`
      introduced. The Linux Lint lane refused
      `a_profile_without_both_halves_is_an_eviction` with
      `bumpy_road_function`: "Complexity bump 1 spans 3 lines", "Complexity
      bump 2 spans 3 lines", "Detected 2 complexity bumps above the threshold
      2.5", `-D bumpy-road-function implied by -D warnings`. That test mixed
      three unrelated arrangements — a unit leaving with its record, an
      emptied `deps`, an absent `.fingerprint`, and finally an absent profile
      — each with its own assert and remove sequence, which is exactly the
      clustered-conditional shape the lint names. It is now three tests, each
      one arrangement: `a_unit_leaving_with_its_record_is_structurally_invisible`,
      the two-case `a_profile_without_either_half_is_an_eviction` table, and
      `an_absent_profile_is_an_eviction`. The table takes the eviction as a
      function pointer rather than a directory name, so the `no_units` case
      can keep the directory in place and empty it — the shape a partial clean
      leaves, and the one `holds no compiled units` exists to name — while the
      `no_fingerprints` case removes the store outright. Each case now starts
      from a fresh fixture, which removed the save-and-restore dance the
      original needed to reuse one. The local full-workspace run does not
      reproduce this finding: it fails first on a pre-existing
      `vendor/gpui/src/lib.rs:181` `run_test` refusal and never reaches the
      harness crate, and a scoped `-p rstest-bdd-harness` run exits 0 without
      naming the file, so CI was the authoritative signal here rather than a
      local one.
- [x] Clear the trybuild snapshot mismatch CI reported on both coverage
      lanes, which is the same commit's other regression. `trybuild` ran
      `scenario_missing_file.rs` as the check-mode preparation probe and
      reported `mismatch`, EXPECTED
      `.../target/tests/trybuild/...` against ACTUAL
      `.../target/llvm-cov-target/tests/trybuild/...`; `mismatch` then took
      both `preparation::nested_fixture_preparation` and
      `runner::step_macros_compile` down with `1 of 1 tests failed`. The cause
      is that a preparation run queues one fixture and nothing else, so
      nothing calls `staging::stage_target_root_snapshots` to rewrite the
      expected text for the coverage target root — the guard only wraps the
      timed run. `scenario_missing_file` names the target directory in its
      diagnostic, so as a probe it could only match under a non-coverage
      target root. The probe is now `scenario_missing_path`, whose diagnostic
      is ``unexpected end of input, `path` argument is required`` — it fails
      while parsing the macro arguments, so it opens no file and reads the
      same under any target root. It is also already queued by the timed run,
      so a `.stderr` the probe blessed would be the one the timed run expects.
      The same failure was present on the earlier `fdcfdff9` run 43714400843,
      which is what makes it a regression rather than a queue artefact. This
      is the general rule the fixture now records in its doc comment: a
      preparation probe may not name a directory, because the preparation is
      not the timed run.
- [x] Answer the one CodeRabbit refinement that was still valid on
      `4a87996f`, and split the module it pushed over the line limit.
      `removing_one_units_fingerprint_records_invalidates_the_preparation`
      removed a record *directory* with `fs::remove_dir_all`, so it exercised
      the same shape as a missing store entry and proved nothing about a
      record whose files went while its directory stayed. That second shape is
      the one the fingerprint-record layer exists for: `layout::discover`
      descends into every directory it finds, and `record_directories` keeps
      listing an emptied one, so the structural layer still reports a
      populated profile and only the recorded *names* can refuse it. It is now
      `removing_a_units_record_files_leaves_its_directory_and_is_still_an_eviction`
      in a new `tests/fingerprint_records.rs`, which empties one unit's
      directory file by file and asserts the directory survives, is genuinely
      empty, and is still refused. Keeping the original case beside it is the
      point: the two differ only in whether the directory went, so a
      regression that recorded directories rather than files passes the first
      and fails the second. The file that held both reached 427 lines — over
      the repository's 400-line cap, enforced by both
      `scripts/check_rs_file_lengths.py` and the Whitaker
      `module-max-lines` lint — so the two fingerprint-record cases moved out
      together and `eviction.rs` returned to 277 lines, holding the shapes a
      directory listing can already decide. `stamped_preparation` widened from
      private to `pub(super)` so the new module shares one fixture rather than
      a second copy of it.
- [x] Record the MD038 failure my own plan prose caused, because it is the
      one class of failure in this work that no local gate caught.
      `docs/execplans/...md:1586` wrote a nested path argument inside a
      single-backtick span, and `markdownlint-cli2` read the result as a code
      span with spaces inside it: `MD038/no-space-in-code` on both the opening
      and closing context, exit 1. The span is now a double-backtick span.
      That failure was reported only by the hosted Linux Lint lane, minutes
      after a local `markdownlint-cli2` pass over the same file — worth
      recording precisely because it is the kind of thing a local run ought to
      have caught and did not. The lesson: any code span whose content ends or
      begins with a backtick, or contains a backtick at either edge, needs the
      double-backtick form; write it that way first rather than discovering it
      in CI.
- [x] Widen the CLI family's ownership declaration to the fixture's whole
      dependency closure, and add a case that reads it from outside.
      `FIXTURE_PATH_DEPENDENCIES` named `rstest-bdd` and `rstest-bdd-macros`,
      the two crates the fixture's manifest mentions. Preparing the fixture
      also compiles three more workspace crates the manifest never names:
      `rstest-bdd` depends on `rstest-bdd-patterns` and `rstest-bdd-policy`,
      and `rstest-bdd-macros` on those two and `rstest-bdd-harness`. All three
      were absent from `owned_packages`, so each one's evicted fingerprint
      records were filtered out of both name lists and admitted silently — the
      same defect as the fixture's own, one level down, and the defect the
      entry above records was fixed at the top layer only. The list now names
      all five. Measured from the fixture's own graph, not assumed: the
      unrestricted `cargo tree --prefix none` in `tests/fixtures/minimal`
      names every one of them, while the narrower `--edges normal,build` view
      omits the two proc-macro host units, and a first version of the list was
      written from that narrower view and was wrong for exactly that reason.
      The doc comment now records both measurements so the next reader does
      not repeat the mistake.
- [x] Give the declaration a regression it cannot drift past, and prove the
      case discriminates. The harness's ownership tests supply their own
      two-element lists by hand, so they prove the mechanism works for
      whichever list a case passes and say nothing about whether the family's
      real list still matches the fixture's graph: the mechanism stays correct
      while the declaration quietly omits a crate. The new
      `preparation/tests/ownership.rs` drives `ensure_prepared_with` — the
      public gate every timed CLI assertion reaches — against a scratch
      preparation carrying the family's actual declaration. Its first version
      iterated `FIXTURE_PATH_DEPENDENCIES` itself, which made it useless:
      reverting the declaration to the two directly-named crates left it
      green, because the loop simply stopped checking the crates that had gone.
      It now names the five crates a second time in an independent
      `REQUIRED_DEPENDENCIES` literal, asserts the declaration contains each,
      then evicts each crate's record directory from an already-stamped
      profile and requires the gate to refuse and to name the crate. The
      duplication is the point: the two lists can disagree, and when they do
      the case fails. Verified both ways — with the two-entry list it fails
      with `the family must declare rstest-bdd-harness: … Declared:
      ["rstest-bdd", "rstest-bdd-macros"]`; with all five it passes in 0.098 s.
- [x] Pin the replacement call site, which no test of the predicate could
      reach. `replacement::is_vouched_for` is given a set built by
      `artefacts::eviction_of`, which removes every name the stamp recorded
      from what is present; that exclusion lives at the call site, so a
      predicate test passing `&[]` by hand cannot exercise it. It matters
      because a profile can hold one crate built twice under different
      metadata hashes — a stale build and a fresh one — and both outputs are
      recorded. When one `.rlib` goes, its recorded sibling must *not* be read
      as its rebuild, or a genuinely lost unit is reported as present at
      exactly the moment Cargo recompiles. The new
      `artefacts/tests/replacements.rs` adds two cases: one where a recorded
      hashed sibling must not answer for the removed build, and the control
      where an unrecorded hashed rebuild still must. Both use hashed names on
      purpose — an unhashed `deps/libunit.rlib` carries no Cargo unit
      component, `UnitPath::parse` refuses it, and `is_vouched_for` returns
      before it looks at a replacement, so a case written that way passes
      whatever the call site does. Verified by deletion: removing the
      `recorded.contains` filter from `eviction_of` fails the new sibling case
      while *both* pre-existing sibling tests stay green, which is the
      measurement showing the call-site path was previously untested.
- [x] Record the spelling gate's reach into Rust doc comments, because it was
      the only gate that failed on an otherwise-clean head and no lint or
      typecheck would have found it. `make markdownlint` runs `spelling`
      first, and `typos-config-builder` reads Rust sources as well as
      Markdown: the doc comment on
      `an_unrecorded_rebuild_answers_for_the_name_it_replaced` used a British
      `-ise` spelling of the verb *to recognize*, and the gate rejected the
      word at `replacements.rs:99:5` with a suggestion to use the `-ize` form.
      The project's prose is en-GB-oxendict and its
      `typos.toml` does not carry a `-ise` allowance, so a British spelling in
      a code comment is a gate failure rather than a style preference. The
      sentence was reworded rather than respelled ("treating a new generation
      as a replacement"), which sidesteps the question entirely. The lesson is
      worth keeping because the failure mode is invisible to `cargo check`,
      `cargo fmt` and every Rust lint: only the spelling gate reads doc prose,
      and only in the Markdown gate's prerequisite, so a contributor watching
      `make typecheck` sees nothing.
- [x] Correct a factually wrong CodeRabbit reply before it could stand as
      evidence. The first reply about the trybuild probe claimed the
      preparation runs no compile-fail fixture and that the compiled-unit
      count was available as proof. Reading
      `crates/rstest-bdd/tests/trybuild_macros/preparation.rs:265-292` showed
      both claims false: the second probe *is* a compile-fail fixture
      (`scenario_missing_path.rs`, driven in check mode), the order is
      build-mode then check-mode, and `prepare_trybuild` returns
      `PreparationSummary::default()` because `trybuild` streams its nested
      Cargo output rather than letting it be captured. A follow-up reply in the
      same thread corrected all three points and quoted the source comments.
      The lesson: a reply that describes internal behaviour should be written
      from a read of the source, not from memory of having written it.
- [x] Learn that `make lint` cannot prove a change to this repository on a
      checkout whose `vendor/gpui` fails, and that CI is the only place the
      whole gate runs. The Whitaker `bumpy-road-function` lint refused
      `tagged_root` in `trybuild_macros/staging/target_root/tests.rs` and then
      `a_recorded_sibling_does_not_answer_for_a_removed_hashed_build` in
      `artefacts/tests/replacements.rs`, on two successive heads, while `make
      lint` reported only the `vendor/gpui` violation on both. The lint stops
      at the first failing crate, so a pre-existing failure anywhere earlier in
      its order hides every later one. Two things follow. Run
      `make lint-whitaker` directly with `CARGO_FLAGS` narrowed to the crates
      the change touches, because that reaches them regardless of what fails
      before them; and scope it to the *whole* set of touched crates, not the
      first one that comes to mind — the first attempt named only
      `-p rstest-bdd` and so reported the fix while the harness crate was still
      broken. `grep` the captured log for the finding rather than trusting the
      exit status, and prefer a full-workspace run once the earlier crate is
      known to pass. Both findings were the same defect shape: a test body with
      two small extraction-shaped closures or `?`-clusters. Extracting them
      into named helpers is what the lint asks for, and it reads better than an
      `#[allow]`.
- [x] Note that a live-environment test may assert a precondition its CI host
      happens to satisfy. `target_directory_uses_running_test_executable`
      asserted unconditionally that the running test executable has an ancestor
      carrying Cargo's `CACHEDIR.TAG`. That holds for a target directory Cargo
      created, and this worktree's did not have one, so the test failed locally
      while the production path correctly fell back to `cargo metadata`. The
      tag is a build artefact and is not tracked, so "it passes in CI" was the
      only thing making the test look sound. It now states the precondition and
      returns early, matching the two `an_untagged_*` cases beside it. Writing
      the tag by hand confirmed the diagnosis before any code changed: all
      eight cases passed with it and eight passed without it once guarded.
- [x] Record that discovery decides what a unit directory *is* by the dep-info
      it holds, which is a fixture-writing rule as much as an implementation
      one. The first version of `evicting_a_target_named_output_invalidates_the_preparation`
      wrote `deps/libengine-<hash>.rlib` with no `.d` beside it, and the gate
      accepted the profile after that output was removed — not because
      ownership was wrong but because `layout::classify` only treats a
      directory as a unit directory when it holds a `dep_info_stem`. An output
      alone is never recorded, so its removal cannot be seen however
      `owned_packages` is declared. Writing the pair, as rustc does, made the
      case pass and the mutation fail. A fixture that models half of what a
      compiler writes can make a regression test pass for a reason unrelated to
      the defect it names.
- [x] Close the two follow-ups CodeRabbit left rather than only the original
      findings. Its last word on two threads was that the finding was *not*
      addressed: the graph tests resolved fixtures with a bare `cargo metadata`
      that could reach the registry despite committed lockfiles, and the
      target-name contract was pinned only by a predicate test, which asks
      whether a name is owned and cannot ask whether a removal is seen. Both
      held. `--locked --offline` now matches the form the harness already used
      for the same call in `preparation/mod.rs`, and the eviction regression
      drives the real gate: prepare through `prepare_now`, remove only the
      target-named output, keep the fingerprint records, assert refusal.
      Mutation-checked by dropping the target name, which fails that case and
      leaves the dependency case green. A reviewer declining to mark a thread
      addressed is a coverage statement about the test, and the right answer is
      a test that fails without the contract.
- [x] Write the dep-info record the way rustc names it, `<stem>.d` beside
      `lib<stem>.rlib`, rather than `lib<stem>.d`. The first version of the
      target-name fixture used the `lib` prefix on both halves, and the prefix
      on the record is what decided which branch of `is_output_of` ran:
      `dep_info_stem` strips only `.d`, so `libengine-<hash>.d` yields the stem
      `libengine-<hash>` and the output matches by the *binary* branch — the
      name beginning with the stem. Only a record without the prefix yields the
      stem `engine-<hash>` and reaches the *library* branch, which strips `lib`
      and then matches. Real Cargo settles it: this worktree's
      `target/debug/deps` holds 1667 `.d` records and every one is
      `<crate>-<hash>.d`; the five `lib`-prefixed entries are the `libc` crate
      itself, a package name rather than an output prefix. The shared
      `write_unit` helper already wrote the correct shape, so the fixture now
      agrees with both the helper and the compiler. Two lessons: a fixture that
      is *nearly* faithful can still exercise the wrong branch, so trace which
      branch a naming reaches instead of assuming one; and read a reviewer's
      branch claim by running it, because the intuition "the output name
      matches the record, so it must be the same branch" is backwards here.
      Re-mutation-checked after the edit, since a fidelity fix is exactly the
      kind of change that can quietly cost a case its discriminating power.
- [x] Reconcile the two pre-merge rows CodeRabbit still reports failed, and
      tell the difference between a row that is answered and one that is
      deliberately declined. `Testing (Unit And Behavioural)` is *declined*:
      nothing runs `make prepare-nested-fixtures` across a process boundary —
      the only test that spawns `make` runs `lint-whitaker`, and the Python
      contract test pins recipe text by design, because `cargo nextest run -E`
      exits zero on an empty filter and the preventable failure is a drifted
      filter. The remedy as written needs nextest inside nextest, which is the
      recursion this plan already records as considered and declined. The row
      was answered that way once before; the honest position is to say it is
      still open and why, not to mark it addressed. `Out of Scope Changes` is
      *answered*: the `0.6.0` → `0.7.0` bump is load-bearing. `ad03c311`
      records that `build_child_env` became fallible, so `cargo package` —
      which strips workspace `path` overrides — compiled `cargo-bdd` against
      the published 0.6.0 harness and failed with `E0599` on `with_context` at
      the required `Publish dry run` step on all three build-test lanes.
      `--allow-unpublished-workspace-deps` cannot help, because lading
      downgrades index-lookup failures only, never compile errors. Moving the
      bump to a release PR would re-break that step, so the two cannot be
      separated. Lesson: a pre-merge row is not automatically work to do. Read
      what the check can and cannot see, and when the remedy is out of scope,
      say so with the evidence and leave it unresolved rather than
      manufacturing a fix or a dismissal.
- [x] Close the declined `Testing (Unit And Behavioural)` row instead of
      leaving it declined, because CodeRabbit supplied a non-recursive shape.
      Its first remedy needed nextest inside nextest, which is why it was
      declined; a standalone driver running separate commands needs no
      recursion at all, so the objection no longer applied. The driver is
      `scripts/check_prepared_fixture_workflow.py`: run the timed test against
      an empty private target and require the refusal, run the remedy against
      the same directory, require the test to pass *and* to report no nested
      compilation (a bare pass is also what a warm shared cache produces), then
      require a mutated fixture source to make the next run refuse and name the
      remedy, which proves the stamp covers its inputs.
- [x] Find — by building the gate rather than by reasoning about it — that
      `make prepare-nested-fixtures` and a hand-typed filtered
      `cargo nextest run` **do not compose**. The recipe pins
      `RUSTFLAGS="$(RUST_FLAGS) $(STANDARD_RUSTFLAGS)"`, which expands to
      `-D warnings -Clink-arg=-fuse-ld=mold`; `RUSTFLAGS` is one of the five
      `FINGERPRINT_EXACT` variables in `stamp.rs`; and a bare `cargo nextest
      run` inherits `RUSTFLAGS` from the caller, unset in an ordinary shell. The
      two therefore compose different fingerprint text and the run refuses the
      stamp the preparation has just written. The evidence is direct, not
      inferred: a stamp written by the recipe records
      `RUSTFLAGS=-D warnings -Clink-arg=-fuse-ld=mold`, and step 3 of the
      driver reproduced step 1's refusal verbatim, naming the same stamp path.
      The refusal message had warned about exactly this — "preparing under a
      different environment only warms a tree this run will not reuse" — while
      never naming the value it wanted, which is why the documented command in
      that same message is itself the broken half. Fixed by adding
      `make nextest-run EXPR=...`, so the build environment has one owner
      instead of a copy in Python; the driver's earlier
      `extra_env={"RUSTFLAGS": ...}` was the wrong shape for the same reason.
      Lessons: a two-command workflow with a shared fingerprint has to be
      *composed* by something, and "both commands are individually correct" is
      not evidence that they agree; and a gate is worth building because
      running it found a defect that reading the same two recipes did not.
- [x] Pin the run half in the Makefile contract test rather than trusting the
      new target to stay matched: `test_the_run_half_pins_the_same_build_environment`
      asserts the exact `RUSTFLAGS` assignment (not merely that `RUSTFLAGS=`
      appears, which a later edit could satisfy with a different value), and
      three siblings cover verbatim `EXPR` forwarding, the absent-`EXPR` guard
      and `.PHONY`. Mutation-checked by deleting the assignment from the
      `nextest-run` recipe: exactly that one test failed and the other seven
      passed, so the assertion discriminates. Verified end to end afterwards —
      the driver refused, prepared, then passed with zero `Compiling` lines;
      a bare `cargo nextest run` against that same prepared tree still exits
      100, which is the divergence the target exists to remove.
- [x] Settle where the driver runs in CI — and find that it belongs nowhere
      new. I had opened this as "a standalone job needs a `runner_placement_test`
      amendment", and that framing was wrong twice over. First, the contract's
      own rationale is about *duplicate execution*: `test_one_job_executes_the_
      workspace_suite` fails "a further job with the same platform and features",
      which "would execute nothing new". Second, and decisively, no lane needs to
      invoke the preparation at all: the six `nested_fixture_preparation` tests
      are ordinary members of the suite and carry `priority = 100`, so they run
      ahead of the assertions that read their stamps inside the *existing* Linux
      lane. `prepare-nested-fixtures` exists for the **filtered** case — one timed
      test on its own, where nothing else would warm the tree — which is exactly
      the case the driver covers and exactly what CI never does. So the driver is
      a local and PR-author aid, and adding a job would have paid for a cold
      compile (measured: a 5.7–7.3 GB private target) to re-test a sequence the
      suite already exercises for free.
      The real defect this surfaced was in prose, not structure:
      `.config/nextest.toml` claimed "`make prepare-nested-fixtures` runs these
      tests before the suite, so a normal lane spends only the stamp read", and
      nothing in `.github/workflows/` invokes that target — it is this PR's own
      addition. The claim described an intent no lane executes. Corrected to name
      `priority` as what actually orders the preparations, and to say the target
      serves the filtered case. Lesson: an invariant asserted in a comment is a
      claim about the tree, and `grep` over the workflows is what separates a
      policy from a wish; a "where does this run in CI?" question is worth asking
      before building the CI job, because the honest answer is sometimes "it does
      not, and does not need to".
- [x] Make the driver reachable from `make`, because a file nothing invokes is
      not coverage. It began as an orphan: every sibling `scripts/check_*.py` is
      named by a Makefile target, and this one could only be run by typing its
      path. That matters for the row it answers — the finding asked for "an
      automated, cross-process test that runs the filtered Nextest workflow end
      to end", and a script no target or suite calls is not automated by any
      reading a reviewer could defend. Added as
      `check-prepared-fixture-workflow`, deliberately *not* a leg of `make test`
      for the same reason the driver exists: it starts from an empty target
      directory, so it pays a cold build on every run and would tax every local
      edit. The sibling checkers are the precedent — reachable targets, not suite
      legs. Verified by running it through the target rather than by inspecting
      the recipe: every step held and it exited 0. Two contract tests pin the
      target to the script and to `.PHONY`; mutation-checked by deleting the
      recipe line, which failed exactly `test_the_workflow_driver_is_reachable_
      from_make` and left the other nine green.
- [x] Close the Major finding (review comment `4589698035`) that step 3 could
      not observe a nested compile at all. The finding was correct, and I
      verified each link against the source rather than taking it on the
      reviewer's word: `run_cargo_bdd` does
      `let (status, stdout, _stderr) = run_cargo_bdd_captured(args)?;`, so the
      nested `cargo`'s `Compiling` lines — which go to stderr — are discarded on
      success; `list_steps_runs` never prints the returned stdout; and nextest
      suppresses a passing test's output. The scan of the captured output
      therefore found nothing whatever the build did and read that emptiness as
      reuse, which is the exact defect the driver existed to catch, one layer
      up. Both plausible streams are dropped, so the report moves to a third
      carrier: a file named by `RSTEST_BDD_NESTED_BUILD_REPORT`, appended by the
      fixture's gate and read by the driver from inside its private target
      directory. The gate reports `0` unconditionally when it admits a stamp and
      runs no preparation, because a report only on the built path would make
      "nothing was written" and "nothing was compiled" the same observation. And
      because a check that cannot fail cannot vouch for anything, step 4 is a
      negative control: appending a comment line to the fixture source changes
      the stamp-relevant hash while changing nothing compiled, so the next run
      must *refuse* and name the remedy, with the original bytes restored in a
      `finally`. The driver's predicates now carry unit coverage of their own in
      `scripts/tests/test_check_prepared_fixture_workflow.py`, including a case
      that requires a missing report to fail and one that requires a reported
      compile to fail. Lesson: "read the output the step printed" is a claim
      about the output's *carrier*, and here two independent layers both
      dropped it — the check has to be built where the evidence is written, not
      where it is convenient to read.
- [x] Answer the same review round's Minor finding (`4589698026`) that a
      timed-out step leaves its `cargo` descendants alive. `subprocess.run`'s
      timeout kills only the direct child, which here is `make`; its `cargo` and
      `rustc` descendants keep writing into the target directory the driver then
      deletes. `run_step` now spawns with `start_new_session=True` and
      `terminate_tree` signals the whole process group with `SIGTERM`, then
      `SIGKILL` after a bounded wait; `os.getpgid` is used rather than
      `process.pid` because the child is the group leader, and
      `ProcessLookupError` is caught because a tree that exited between the
      `poll()` and the signal needs nothing further. The Rust side documents the
      same hazard in `nested_cargo/bounded.rs` and is tracked by #923; this is
      the Python driver's own copy of the fix, since it cannot reuse that code.
- [x] Reverse the earlier "the driver belongs in no CI lane" conclusion, because
      the pre-merge row's actual requirement was that it not be *reachable only
      by a manual Make target*. Both readings of "no job needed" were about
      duplicate execution, and that contract still holds: a
      `make check-prepared-fixture-workflow` step matches none of the three
      `WORKSPACE_TEST_COMMANDS` in `tests/workflow_contracts/cache_step_support.py`
      (`cargo test`, `cargo nextest`, `make test`), so it is not a second run of
      the suite and needs no runner-placement amendment. What the earlier
      reasoning missed is that reachability and *exercise* are different facts.
      The suite runs the preparation tests, but nothing anywhere runs the
      *filtered* path the target serves, so a green lane said nothing about the
      workflow this PR adds. Added as a step inside `build-test` rather than as
      a new job, guarded to Linux and to `matrix.tools`, and placed after the
      suite so the cold private build (measured 89 s warm, 239 s in CI) does not
      compete for the compiler slot the instrumented coverage build needs.
      Windows is excluded because those lanes run `cargo test`, not nextest, so
      the target's filter selects nothing there. Lesson: "does this need to
      run?" and "is anything checking this?" are separate questions, and
      answering the first with "the suite already covers it" left the second
      unanswered.
- [x] Close the surviving pre-merge row by measuring the timed command rather
      than the gate. CodeRabbit's reply to the reconciliation was right: the
      reuse count came from `record_reuse` inside `ensure_gated_with`, which
      describes the gate's decision and not the `cargo bdd steps` that runs
      afterwards. `run_cargo_bdd_steps_captured` now keeps the timed command's
      stderr and `list_steps_runs` files `count_compiling` over it before
      asserting, so a run leaves the gate's zero *and* the timed command's own
      count. The judgement already rejected any nonzero line this run added, so
      it needed no logic change — only a producer that describes the right
      command. A `--keep` run confirmed the three-line shape:
      `compiling=51` (step 2's preparation), `0` (step 3's gate), `0` (step 3's
      timed command). Stated to CodeRabbit rather than glossed: no automated
      case reaches a real nonzero timed count, because every input the stamp
      hashes is the whole input set, so any edit to force a rebuild makes the
      stamp stale and the timed command never runs.
- [x] Clear the Whitaker `bumpy_road_function` denial the republished rolling
      suite raised on `vendor/gpui/src/lib.rs:181`, by splitting `run_test` into
      `run_seed`, `announce_seed`, `announce_failing_seed` and
      `handle_terminal_failure`. Not suppressed with an allow: this file already
      decomposes its neighbours into named helpers, and the repository's own
      recorded position is that extraction is what the lint asks for. Each
      extracted function keeps the `#[expect]` for the macro it contains, which
      was verified rather than assumed — removing `run_seed`'s `print_stdout`
      expect failed clippy on its `println!`. Verified against the freshly
      extracted rolling suite (not the stale local one): the whole workspace
      `cargo check --workspace --all-targets --all-features` exits 0, so the new
      `?` charge does not flag any of this PR's own code either.
- [x] Obtain execution evidence for the new call site after the full suite
      failed to provide it. `make test` timed out on an unrelated cold
      preparation and cancelled 125 tests, so `list_steps_runs` — the one test
      this change edits — never ran in a gate. A filtered nextest selection
      (`-E 'test(list_steps_runs)'`) passed in 2.213 s, and re-running it with
      `RSTEST_BDD_NESTED_BUILD_REPORT` pointed at a fresh file produced exactly
      two `compiling=0` lines: the gate's count and the timed command's own.
      That two-line shape is the observable difference this change exists to
      make, and it was checked as a file rather than inferred from a pass.
- [x] Clear the second wave of `bumpy_road_function` denials the rolling suite
      raised, this time in first-party code rather than `vendor/gpui`:
      `crates/rstest-bdd/src/reporting/junit.rs` was flagged at `write` and
      `write_escaped`. Extracted `write_testcase`, `write_skipped_testcase`,
      `write_escaped_character`; the `if !valid { ...; continue }` shape that
      the suite charges as an early-return branch became a guard with an
      explicit `return`, and the escaping `match` now yields a string instead of
      writing from each arm. Verified against the same freshly extracted rolling
      suite the hosted lane loads (`libwhitaker_suite@nightly-2026-05-28.so`,
      nested through admission): zero `bumpy_road` diagnostics, `Finished`
      cleanly. The 13 `reporting` and `junit` tests pass, so the refactor is
      behavioural no-op as intended.
- [x] Clear the third wave of `bumpy_road_function` denials, at
      `crates/rstest-bdd-macros/src/codegen/wrapper/args/extract.rs:165`
      (`extract_args`). The `'args:`-labelled loop carried three
      `continue 'args;` early exits, each charged as a branch. Replaced the
      labelled loop with a plain `for` over `func.sig.inputs` delegating to a
      new `classify_argument` helper that returns `syn::Result<()>`; the three
      `continue`s became `return`s from that helper. Because a labelled
      `continue` also skips to the next iteration of an outer loop, the helper
      had to preserve the exact classifier ordering — notably that
      `classify_harness_context` runs *before* the placeholder
      short-circuit, or a placeholder-named `#[harness_context]` parameter
      would leak its marker into generated code. The reasoning is captured in
      the helper's doc comment so a later refactor cannot silently reorder it.
      Verified against the same freshly extracted rolling suite:
      `cargo check -p rstest-bdd-macros --all-features` finished cleanly with
      zero `Multiple clusters` diagnostics, and a **full-workspace** sweep
      (`--workspace --all-targets --all-features`) also finished with zero
      `bumpy_road_function` findings in 22m10s. Local gates: `cargo fmt --check`
      exit 0, `cargo clippy -p rstest-bdd-macros --all-targets --all-features
      -- -D warnings` exit 0, and 566/566 `rstest-bdd-macros` lib tests pass.
      The workspace sweep is the important new evidence: it enumerates *every*
      remaining site in one pass instead of discovering one crate per CI cycle.
- [x] Address review `5941784450` (`CHANGES_REQUESTED` on `f4245f37`), 25
      inline findings plus the two failed pre-merge rows. The pre-merge rows
      were cleared by evidence rather than by argument: the PR body now
      carries a `## Stack base` section recording `baseRefName: main`,
      `baseRefOid 0bf12ee7` (the current `main` head), merge base `d70f37c1`
      and the single commit the branch is behind, and a `##
      crates/rstest-bdd/tests/underscore_fixture.rs, and why it changed`
      section naming the failing gate (`build-test (linux, default features)`
      → `Lint`), the `unfulfilled_lint_expectations` error, and why the
      `0.7.0` bump was forced by `Publish dry run` rather than chosen.
- [x] Close the `fingerprint_with` boundary finding (`4602070911`). The
      previous test composed twice through `compose`, which is what the case
      before it already does, so a `fingerprint_with` that resolved the
      identity and then composed from a constant would have satisfied both.
      `the_probe_identity_reaches_the_fingerprint` now supplies two distinct
      identities *through* `fingerprint_with` and asserts both that the
      fingerprints differ and that each carries the identity its probe
      returned, so neither half alone can pass.
- [x] Close the effective-compiler finding (`4602070930`). Probing only
      `cargo -vV` cannot see a compiler replaced in place at a fixed `RUSTC`
      path: Cargo picks and fingerprints its compiler independently of its own
      identity, so a stamp written before the swap would still match and the
      timed assertion would recompile inside its deadline.
      `toolchain_identity` now runs `-vV` twice — once for Cargo, once for the
      compiler — and joins the two on separate labelled lines so a Cargo
      identity cannot be spelled to look like a compiler one. The compiler is
      resolved by path (the child's `RUSTC`, else the `rustc` shipped beside
      the `cargo` being run, else the bare name) rather than through the
      child's filtered `PATH`, because every synthetic fixture in
      `preparation/tests` sets `PATH=/usr/bin` and this host's compiler is not
      there. The probe overlays the nested environment on the inherited one
      rather than clearing it, so the child's `RUSTC` and `RUSTUP_TOOLCHAIN`
      still decide which compiler is identified while a `rustup` shim keeps
      `HOME`. `STAMP_VERSION` went to `4` so no version-3 stamp — which records
      Cargo alone — can be read as the text version 4 expects.
- [x] Answer the remaining 23 inline findings on the disposition that fits each
      rather than editing for its own sake. Eleven were already implemented on
      `f4245f37` and needed only the reply: the `cargo-nextest` install step
      (`4602070559`), the fixture's full `cargo tree` dependency closure
      (`4602070583`), both check-mode probes in the GPUI and Tokio harnesses
      (`4602070655`, `4602070674`), ANSI stripping in `count_compiling`
      (`4602070718`), the `drain` scheduling race (`4602070740`, already split
      into a generous content collection and a separate bounded one),
      `verify_declared` refusing an empty owned section before the stamp is
      written (`4602070831`), the replacement case table (`4602070847`),
      `only_eviction`'s `io::Result` (`4602070862`), `preparation_over`'s
      `io::Result` (`4602070870`), `write_units`' `io::Result`
      (`4602070881`), `write_tree`/`hash_of` (`4602070893`), the dep-info case
      table (`4602071012`), and both documentation rows (`4602071034`,
      `4602071051`) — the guide already states the discovered-directory rule
      and already describes captured output as a possibly truncated prefix on
      both paths, as does the plan's own "Drain on a deadline" entry. Four were
      fixed in this round: `scratch_family` now propagates its two
      `env_from_vars`/`Preparation::new` failures (`4602070604`), and the
      feature-rebuild fixtures reach the filesystem through a `Dir` capability
      — a new `evict` helper for the three eviction cases and a
      capability-built scratch tree (`4602070946`), with `tagged_root` in
      `staging/target_root/tests.rs` rebuilt the same way (`4602070996`). The
      remaining rows are conventions this PR did not introduce and that sweep
      six modules rather than one; they are deferred to `#946` with the
      reasoning recorded per thread.
- [x] Note that `-p rstest-bdd --tests` clippy reports one denial in
      `crates/rstest-bdd/tests/mutable_world_macro.rs` (`uninlined_format_args`),
      which no commit on this branch touches (`d209ec52` is its last change and
      it is absent from the whole-PR diff). Recorded rather than fixed: the
      file is not this PR's, and `make lint` runs `--workspace --all-targets
      --all-features` against the pinned stable toolchain, where the finding is
      not raised — the local reproduction used `cargo clippy -p ... --tests`
      outside the gate's flag set.

- [x] Record the commit-gate outcome for `2e46a6fb` and why three nested-build
      tests failed locally while the same head is green in hosted CI. The three
      failures are all wall-clock bounds spent in the shared compile-admission
      queue, not defects in the change:
      - `rstest-bdd-harness` — `binary_test_support_cargo::locate_or_build_returns_command_for_workspace_binary`
        hit its own 180 s override with 215/218 passing. The test builds
        `todo-cli` when `<target>/debug/todo-cli` is absent, which it was.
      - `cargo-bdd --test cli` — `preparation::nested_fixture_preparation` hit
        `PREPARATION_COMMAND_TIMEOUT` (540 s) after compiling 49 crates,
        queueing at 55–85 ahead.
      - `rstest-bdd --test feature_rebuild_invalidation` —
        `feature_addition_triggers_rebuild` killed its second nested
        `cargo test` at `CHILD_TIMEOUT` (300 s) after both scenario assertions
        had already reported `ok`.
      Admission depth rose monotonically across the three gates: 159, then 241
      "ahead" in the captured child output, with 290 requests waiting against
      capacity 4 by the end. `build-limits-status` reported `healthy` throughout
      and never printed a fault, `drain` or `recovery` line, so this is a
      saturated-but-healthy pool — queue time, which per the operating
      instructions is a wait and not a failure of this work.
      Evidence it is not this PR: `process.rs` (`CHILD_TIMEOUT`),
      `addition.rs`, and `binary_test_support_cargo.rs` are all unmodified since
      `d70f37c1`, and the only file this branch touches under
      `feature_rebuild_invalidation/` is the `preparation/tests.rs` helper.
      The controlling observation is hosted CI on `f4245f37`, which ran this
      same suite on clean runners with no competing agents and passed all three
      `build-test` lanes. The new head `2e46a6fb` is therefore gated on its own
      hosted run rather than on a re-run that would queue behind the same pool.
- [x] Clear the CodeScene findings the gate raised on `c8611a72`, all of them
      in code this branch introduces. Two rules were involved: one critical
      (`Bumpy Road Ahead` on `outputs_in`) and three advisory (argument counts
      on `Preparation::new`, `Preparation::for_fixture`, `prepare_locked` and
      `ensure_prepared_reported`, plus a `Similar Structure` cluster over the
      probe/reporter ladder). The fixes are structural rather than suppressive:
      - `outputs_in` now sorts the listing once and scans two prefix ranges
        through `prefixed_range` and `collect_outputs_of`, so a stem's outputs
        are found without visiting the rest of the listing.
      - `Sources` (already present) is joined by `Directories`, grouping the
        stamped root and the working directory. Both are `PathBuf`, so a caller
        that transposed them got a stamp recording the wrong tree and no
        compiler complaint; the struct removes the possibility.
      - `Instruments` groups the toolchain probe and the diagnostic reporter.
        Both are overridden for the same purpose and at the same time, and
        threading them separately is what pushed the innermost boundary past
        the argument budget.
      - `ensure_prepared_with_probe` and `prepare_now_with_probe` are **gone**.
        A caller audit found no use for the probe-only rung once `Instruments`
        existed: every test that supplied a probe also supplied a reporter, so
        the rung was a public entry point with no independent purpose. The
        ladder is now `ensure_prepared` → `ensure_prepared_with` →
        `ensure_prepared_reported`, with the same shape for `prepare_now`.
      - `Target` groups the stamp and fingerprint that `prepare_locked` judges,
        which were only ever read together.
      - `scripts/prepared_fixture_judgements.py`'s 51-line
        `expect_pass_without_rebuild` was split into `judge_run_reports`,
        `require_one_of_each` (now a `collections.Counter` over
        `describe_count_offence`) and `require_no_rebuild`, and its docstring's
        rationale moved to the module docstring where the other framing already
        lived.
      `cs delta main` now returns an empty array and `cs review` scores the
      Python module 10.0. `make check-fmt` exits 0 after a `make fmt` pass, and
      `make lint-python` and the 24 workflow-judgement tests pass.

- [x] Implement CodeRabbit's accepted remedy (`4626700815`, thread on
      `crates/cargo-bdd/tests/cli/preparation/closure.rs`) for the timed CLI
      gate's slow-success path, as one cooperative subprocess deadline rather
      than two per-command bounds. The family chose remedy 2: charge every
      command against a single budget, refusing before a command starts once
      the budget is spent.
      - `nested_cargo::Deadline` moved out of `toolchain.rs` into a new
        `preparation/deadline.rs`, with `remaining` (a spent budget is `None`,
        not zero, so a refusal is distinguishable from a timeout) and `cap`
        (the smaller of the remainder and a per-command limit). Its module
        doc records that this is a *cooperative* deadline: filesystem reads,
        spawning and child cleanup are not charged.
      - `ensure_cli_prepared` creates the one budget from
        `TIMED_GRAPH_QUERY_TIMEOUT` before `cli_preparation_description`, which
        charges the graph query with `deadline.cap(...)` and returns the
        deadline so `instruments_within` charges both probes against what is
        left. The timed test therefore cannot spend 119 s on a successful
        query and then hand `cargo -vV` and `rustc -vV` a fresh minute each.
      - `toolchain_identity_within` and `probe_bound` carry the deadline into
        the real probe; `toolchain_identity` is the `None` wrapper every other
        family keeps, so existing callers' behaviour is unchanged.
      - Regressions: `preparation/deadline/tests.rs` (seven arithmetic cases,
        including one that spends a real budget and shows the *next* command
        is refused), `preparation/toolchain/tests.rs` (three `probe_bound`
        cases: no deadline, live deadline, spent deadline refused by name),
        and `preparation/tests/deadline_gate.rs` (two cases on the production
        binding, asserting the gate reports `TimedOut` naming `cargo -vV`
        before the probe runs and still prepares under a live budget).
      - The comment in `cargo-bdd/tests/cli/preparation/tests/declaration.rs`
        that claimed the query and probe budgets were "not additive" is
        corrected: it generalized from a query that *failed*, while a slow
        success was the case that reached the runner's kill. The same stale
        claim on `TIMED_GRAPH_QUERY_TIMEOUT` is corrected too.
      - `witness_or_withdraw` and `require_fixture_binaries` moved from
        `cargo-bdd/tests/cli/preparation.rs` into
        `preparation/witness_gate.rs`, and `Deadline`/`instruments_within`
        moved into their own module, to stay inside the 400-line budget.
      Focused evidence: 22 tests passed across `preparation::deadline`,
      `preparation::toolchain` and `preparation::tests::deadline_gate`; clippy
      clean for both crates with `--all-targets --all-features -D warnings`;
      `RUSTDOCFLAGS="--cfg docsrs -D warnings" cargo doc -p rstest-bdd-harness
      --no-deps` clean.

- [x] Lesson: never edit the worktree while a gate set is running. The first
      attempt at the three-gate run for this change set was invalidated by my
      own edits — the scrutineer caught a rustdoc link error that I had
      already fixed under it, and one gpui trybuild test failed because its
      preparation stamp was written for a fingerprint the edited sources no
      longer matched. A gate verdict is only meaningful for a quiesced tree.

- [x] **The hosted-coverage stamp refusal: `.profraw` files were being hashed.**
  All three `build-test` lanes failed at `e8f17cb8` in *Test and Measure
  Coverage*, not in lint. Linux failed
  `bound_feature_file_is_tracked_dependency` with an **empty** evicted list,
  which by `missing_stamp_message`'s own rule proves the *fingerprint*
  disagreed rather than the artefacts having gone missing.

  Reproduced deterministically on this host:
  `RUSTFLAGS="--cfg=coverage -Cinstrument-coverage -Clink-arg=-fuse-ld=mold"
  cargo nextest run -p rstest-bdd --test feature_rebuild_invalidation
  --target-dir /tmp/covtarget-issue910`.
  Two fresh-target runs of the same commit produced different `inputs:`
  digests, and a separate consumer invocation then refused the stamp the
  preparation had just written.

  The diagnosis was driven by measurement, not inference. A per-root probe
  inside `fixture_preparation` showed the root *list* byte-identical between
  runs while the total hash moved; per-root hashes then isolated it to the
  `crates` root alone (`4c50b4e3d057d7c3` vs `eb650141f7e00b57`), with the
  fixture tree and all four configuration files stable. A full recursive
  content digest of every hashed root, taken before and after one run, named
  the churn exactly: `default_<hash>_0_<pid>.profraw` files appearing in
  `crates/rstest-bdd/`, `crates/rstest-bdd-macros/` and
  `crates/rstest-bdd-harness/`.

  `-Cinstrument-coverage` makes every instrumented test binary drop a profile
  beside the working directory it ran in — which is the crate whose test just
  ran, inside the hashed `crates` root. The name carries the process id, so
  each instrumented run *adds* a file rather than overwriting one. The
  preparation wrote a stamp, the instrumented binaries it had just been asked
  about then added profiles, and the consumers refused the stamp. A plain run
  writes no profile at all, which is why only the coverage lanes failed and the
  local gates were green.

  Fix: `input_hash::is_build_output` now also skips names ending `.profraw`,
  alongside `target`, `wip`, `.git` and the stamp files. A coverage profile
  records what a test executed; it is runtime output like `target`, and nothing
  a fixture compiles reads it. Regression:
  `input_hash::tests::coverage_profiles_are_skipped`, which adds *two* profiles
  under different ids so a fix matching one fixed name or the `default_` prefix
  would still fail.

  Verified: the full `feature_rebuild_invalidation` binary passes 34/34 under
  the coverage `RUSTFLAGS`, and a second, separate invocation then accepts the
  stamp written by the first — `bound_feature_file_is_tracked_dependency` and
  `scenarios_directory_filtered_file_is_tracked` both green against it.

  A wrong hypothesis was falsified on the way and is recorded so it is not
  retried: removing the `if scratch.is_dir()` guard around the scratch config
  walk did **not** stabilize the hash. The unconditional walk is still the
  right shape — a conditional input makes the fingerprint depend on state the
  preparation itself creates, and the comment now says so — but it was not the
  cause of this failure.

- [x] Third copy of the path-spelling defect, at
  `crates/rstest-bdd/tests/trybuild_macros/preparation/tests.rs:33`, which
  failed both Windows lanes at `e8f17cb8`. Fixed at its source, matching the
  two earlier copies: `trybuild_macros/preparation.rs::workspace_root` now
  resolves once at the definition (kept `Utf8PathBuf`, since
  `staging::trybuild_target_directory` and `input_roots` take camino paths),
  and the case compares by containment beneath the resolved `.cargo` directory
  instead of whole-path equality. The `without` control is cut down by the same
  predicate, so the two sides differ in exactly the configuration.

- [x] **The empty-host-profile refusal: a target-selected build is not
  damage.** CodeRabbit's second finding was valid and load-bearing. The
  structural layer demanded a compiled unit *and* a fingerprint record in every
  named profile, which is not what Cargo guarantees.

  Measured, not inferred. Two minimal crates were built under
  `/tmp/probe-f2-845335/`, both with `--target x86_64-unknown-linux-gnu`:

  - a dependency-free library left `target/debug` holding exactly
    `build/`, `examples/`, `incremental/`, `.cargo-artifact-lock`,
    `.cargo-build-lock` and `.cargo-lock` — no `.d` record and no
    `.fingerprint` store anywhere beneath it;
  - the same crate with one proc-macro dependency left real host units at
    `target/debug/build/pm/06ae238569adf39b/out/pm-06ae238569adf39b.d`
    and `target/debug/build/pm/06ae238569adf39b/fingerprint`.

  So the host profile's contents are conditional on the dependency graph. A
  blanket rule refused every target-selected preparation of a dependency-free
  crate — a command that had just exited zero — and no test caught it because
  every fixture in the suite has a host unit.

  The fix lives in the structural layer, not in `profiles()`: that function
  runs *before* the build and cannot know what the build will produce, so it
  can name the directories but never vouch for their shape. A new
  `artefacts/state.rs` reports what each directory holds — `Unvouched`,
  `Present(layout)` or `Absent(reason)` — and `evicted()` now inspects every
  directory into `states` first, computes `waive_empty = any_usable(&states)`,
  and only then decides. The waiver is deliberately narrow:

  - it applies only when the profile holds *neither* half of Cargo's
    layout (`whole_empty_profile`), so a half-evicted tree still refuses;
  - it applies only when another named profile in the same preparation
    holds a usable tree, so a command that produced nothing at all still
    refuses (`an_empty_profile_beside_no_populated_one_is_refused`);
  - it applies only on the structural-only path. A stamp section naming
    artefacts that are all gone still refuses, because an empty presence
    list makes every recorded name missing without a second rule.

  That third clause is the one a first design got wrong. I had `evicted()`
  return early with `Ok(Vec::new())` when *any* profile was populated, which
  would have skipped the exact layer entirely and accepted a genuinely evicted
  recorded profile. Threading a `waive_empty: bool` into `eviction_of` puts the
  waiver where it belongs and leaves the stamp's own verdict alone.

  `unusable_reason()` also had to split, because it conflated two refusals with
  different answers: `absence_reason()` (unvouched or absent — always refuses,
  however healthy a sibling is) and `empty_layout_reason()` (waivable).
  Replacing an exact message with a waivable one would have lost the "the stamp
  records no artefacts for this profile" diagnosis.

  Regressions, nine in `artefacts/state/tests.rs`. The headline one,
  `a_target_selected_preparation_with_an_empty_host_profile_is_admitted`, is
  the measured shape above: it asserts the stamp records an empty section for
  the host profile and a populated one for the selected profile, and that
  `evicted(&preparation, None)` is empty. Beside it:
  `a_sole_empty_profile_is_refused` (so `mkdir -p <root>/debug` is not a build),
  `an_empty_profile_beside_no_populated_one_is_refused`,
  `a_half_empty_profile_beside_a_populated_one_is_refused` (only `.fingerprint`
  removed), and `a_host_profile_the_stamp_had_recorded_refuses_once_emptied` —
  the case the waiver must not swallow, written so it first proves the fixture
  is admitted and then empties the profile.

  `no_profile_to_inspect` and `half_missing` were deleted from `artefacts.rs`;
  their logic is what `state.rs` now reports. The existing contract
  `preparation/tests/target_profile.rs` enforces — evicting only the target
  profile must still invalidate the preparation, with "artefacts it vouched for
  are gone" in the message — still passes.

- [x] **The third finding's regression, and two defects that only the gate
  found.** CodeRabbit's third finding (the symlinked specification file) had no
  regression, so one was added:
  `profiles::tests::a_symlinked_specification_file_names_the_resolved_stem`
  builds `real/actual.json`, links `alias.json` to it, selects
  `CARGO_BUILD_TARGET=alias.json` and asserts the named profile is
  `target/actual/debug`. The value is relative and the link exists only beside
  the working directory, so the case states both halves of the rule — resolve
  against the child's directory, then take the resolved stem — in one assertion.

  Two further defects surfaced while validating, and the gates are what
  surfaced them:

  - `profiles/tests.rs::profiles_for` still called `profiles` with two
    arguments after the working-directory parameter was added. That was a
    hard `E0061` breaking `make lint`, `make typecheck` and `make test`,
    and it is the scrutineer's headline finding. Fixed by threading the
    temporary directory through as the working directory.
  - `stabilise` at this plan's line 2280, which `make spelling` rejects
    under the en-GB-oxendict policy and which took `make markdownlint`
    down with it through its `spelling` prerequisite.

- [x] **A stable `cargo fmt --all` reformatted 340 unrelated files.** Worth
  recording because it looked like a catastrophically wide change set and was
  not. This repository pins the formatter —
  `FMT_TOOLCHAIN ?= nightly-2026-08-07`, per ADR-016 — because `.rustfmt.toml`
  enables unstable options (`imports_granularity`, `group_imports`,
  `fn_single_line`, `wrap_comments` and more). Running bare `cargo fmt --all`
  used the *stable* rustfmt, which warned that it could not set those options
  and then rewrote the tree in stable's layout: 349 dirty files, of which all
  but a handful were unrelated to this change. Re-running
  `cargo +nightly-2026-08-07 fmt --all` restored the intended twelve modified
  files plus the new `state` module exactly.

  Lesson: use `make fmt` / `$(CARGO_FMT)`, never bare `cargo fmt`, and read a
  suspiciously large `git status` as a toolchain question before reading it as
  a diff. `cargo +nightly-2026-08-07 fmt --all -- --check` is clean on the
  settled tree.

  Verification on the settled tree:
  `cargo clippy -p rstest-bdd-harness
  --all-targets --all-features -- -D warnings`
  exit 0; the `artefacts` module 66 passed; the `nested_cargo` module 200
  passed (30.05 s); `cargo-bdd --test cli` 37 passed;
  `rstest-bdd --test trybuild_macros` 51 passed;
  `rstest-bdd --test feature_rebuild_invalidation` 34 passed. The coverage
  regression was re-confirmed end to end: under
  `RUSTFLAGS="-Cinstrument-coverage"` the preparation wrote 327 `.profraw`
  profiles and a *second, separate* invocation still accepted the stamp
  ("nested preparation: family=feature-rebuild-invalidation is prepared"). All
  profiles were then removed. Line budget: `preparation.rs` 393, `artefacts.rs`
  375, `state/tests.rs` 285, `profiles/tests.rs` 235, `profiles.rs` 183,
  `state.rs` 114.
- [x] **A mid-gate edit is indistinguishable from a real stamp defect.**
  `make test` failed at 2331/2461 with `cargo-bdd::cli list_steps_runs`
  refusing the `cargo-bdd-cli` stamp, and the message carried **no** evicted
  artefact list — which by `missing_stamp_message`'s own rule proves the
  *fingerprint* disagreed. The stamp existed (written 16:00:24) and the
  preparation test immediately before it had passed in 21.6 s, so nothing was
  missing.

  The cause was this session's own edit to `input_hash/config.rs`, saved at
  16:00:18 — while the preparation was already running. `family_roots` hashes
  the whole `crates/` tree as one root (see
  `crates/cargo-bdd/tests/cli/preparation.rs`), so the preparation had computed
  its input hash from the pre-edit tree and written a stamp that no later run
  could match. `find -newer` against the stamp then found no input changed
  *after* it, which only ruled out the other direction and briefly looked like
  a contradiction.

  Diagnosis, not inference: a throwaway probe crate outside the worktree
  recomputed `input_hash` over the real roots and got `e0f37246a728354e`
  against the stamp's `34911382e366b99f`; instead of guessing further, a
  temporary `eprintln!` inside `is_build_output` was used to confirm the walk
  was skipping exactly `target`/`wip` and nothing else. Both were reverted. The
  lesson is the one already recorded for commits: **a gate verdict is only
  meaningful for a quiesced tree**, and a stamp makes that literal — the
  preparation and its consumers must see the same bytes, so an edit during a
  run poisons the result for every test after it.
- [x] **`MD046` came from 6-space continuations; the fix is indentation.**
  `make markdownlint` reported three `MD046/code-block-style` errors on a file
  that lints clean at `HEAD`. `MD046`'s `consistent` mode latches onto the
  file's *first* code block, and the added Progress entry put a continuation
  line at 6 spaces after a blank line inside a two-space-marker list item. That
  is exactly content-indent plus four, so CommonMark parses it as an
  **indented** code block; the three pre-existing fenced blocks then became
  inconsistent. The fix was to re-indent the added hunk's continuations to the
  document's dominant 2-space style and nest its sub-lists at the 2-space indent
  `MD007` wants. Verified with the gate's own binary:
  `/home/leynos/.bun/bin/markdownlint-cli2` reports `Summary: 0 error(s)`.

  **Recurred once more** while writing the budget entry above: 34 continuation
  lines were at 6 spaces inside a 0-indent list item, which CommonMark read as
  an indented code block, so `MD046` reported all four *fenced* blocks in the
  file as `Expected: indented; Actual: fenced` — including three that had
  linted clean at `HEAD`. The reported line numbers point at the fenced blocks,
  not at the offending continuations, so the symptom misleads: it is worth
  grepping for `^ {6,}` before touching any fence. Re-indenting the
  continuations to 2 spaces restored `Summary: 0 error(s)` with the verbatim CI
  evidence left intact. A first attempt "fixed" it by rewriting the pasted log
  lines; that is falsifying evidence and was reverted.

  **`cargo fmt` is not the format gate.** After that fix,
  `cargo +nightly-… fmt --all -- --check` passed locally, so CI run
  `44679681326` on the same head failing *Check formatting* looked
  inexplicable. It was not rustfmt: the step runs the whole `make check-fmt`
  target, whose last command is
  `mdtablefix
  --check --git --include-untracked --wrap --renumber --breaks --ellipsis
  --fences`,
  and that reported `docs/execplans/issue-910-...md +43 -42`. The hand-wrapped
  prose in the new entries did not match its 80-column reflow. The log line to
  look for is `make: *** [Makefile:277: check-fmt] Error 1` — 277 is the
  `mdtablefix` line, not a rustfmt one. Run `make check-fmt`, or at minimum
  `mdtablefix --check ...`, before pushing a Markdown-heavy change.

- [x] **CodeScene Code Duplication in the preparation harness, fixed by
  removing the duplication.** `CodeScene Code Health Review (main)` failed
  `b182734c` under the advisory gate *Enforce advisory code health rules* —
  `1 file with Code Duplication` — naming
  `crates/rstest-bdd/tests/feature_rebuild_invalidation/harness/preparation/mod.rs`
  at code health 9.39. `cs review` localized it exactly:
  `ensure_rebuild_prepared` (77–86) and `ensure_addition_prepared` (107–116)
  were byte-identical apart from which family they named.

  Extracted the shared sequence into `ensure_prepared`, parameterized by the
  family's `describe` and `prepare` functions. Both callbacks already had the
  signature `fn(&process::ChildEnv) -> io::Result<...>`, so this is a plain
  function-pointer parameter — no trait object, no generic builder. Re-measured
  with the project's own tooling: `cs review` on the file now reports **score
  10.0, no findings**. The rule was not suppressed and no allowlist entry was
  added.

- [x] **The cold-run probe directory: the same cold/warm asymmetry, in the
  toolchain probe.** CodeRabbit thread `4627594441` observed that
  `lifecycle.rs:199` (`read_stamp(preparation, probe)`) and `prepared.rs:77`
  (`fingerprint_with(preparation, probe)`) both run the toolchain probe
  *before* the `prepare` callback copies the scratch tree, so on the first gate
  `preparation.working_directory` — which `preparation/mod.rs:305-311` sets to
  `family.scratch()` — does not exist yet. `probe` then fell back to this
  process's own directory.

  The concrete consequence turned out to be unreachable *on this host*: the
  repository pins `channel = "stable"` in the root `rust-toolchain.toml`, the
  fixtures carry no `rust-toolchain` file of their own, and the scratch root is
  `target/tests/rebuild-invalidation` inside the same checkout — so the shim's
  upward walk reaches the same pin from either directory, and
  `RUSTUP_TOOLCHAIN` is in `FINGERPRINT_EXACT` besides. But that safety is an
  accident of where the scratch root sits, not a property the probe enforces,
  and it is exactly the cold/warm asymmetry this change already fixed for the
  configuration-input walk
  (`a_root_named_through_a_parent_contributes_the_same_walk_cold_and_warm`). A
  scratch root outside the checkout, or a pinned fixture, would select a
  different toolchain cold than warm; the stamp would then name a compiler the
  nested build never runs, and the timed gate would either refuse its own stamp
  or admit a tree built by a compiler it never identified.

  Fixed by starting the probe in the nearest **existing** ancestor of the
  nested directory (`probe_directory`). That is the deepest directory the shim
  would have visited on a warm run, so the toolchain file it resolves is the
  same one, and the only difference is the components that do not exist to be
  descended into yet. The inherited directory survives only as the last resort
  for a path with no existing ancestor at all, rather than as the answer for
  every cold run. The regression,
  `a_probe_runs_beneath_the_nested_directory_even_before_it_exists`, puts an
  existing ancestor above an absent tail and pins both halves of the rule.
  Focused check:
  `cargo nextest run -p rstest-bdd-harness -E 'test(/toolchain/)'` — 21 passed.

- [x] **The seventh copy of the path-spelling defect, and the one its own
  comment claimed was immune.** Run `44673737187` on `bc06a3d9` failed *both*
  Windows lanes — `build-test (windows, default features)` and
  `build-test (windows, strict-compile-time-validation)` — on the same single
  case, `242 passed; 1 failed`:

  ```text
  panicked at crates\rstest-bdd-harness\src\nested_cargo\preparation\input_hash\config\tests.rs:80:5:
  assertion `left == right` failed: the fallback root is the absolute spelling of the same relative root
    left: "\\\\?\\D:\\a\\rstest-bdd\\rstest-bdd\\crates\\rstest-bdd-harness\\a-directory-this-case-does-not-create"
   right: "D:\\a\\rstest-bdd\\rstest-bdd\\crates\\rstest-bdd-harness\\a-directory-this-case-does-not-create"
  ```

  `a_relative_root_resolves_to_an_absolute_walk` asserted
  `search_root(&relative) == current_dir().join(&relative)`. The implementation
  is `absolute_root` (lexical join) → `split_at_existing_prefix` → `reappend`,
  and `split_at_existing_prefix` calls `fs::canonicalize` on the longest
  **existing** prefix. On Windows `fs::canonicalize` returns the `\\?\`
  verbatim form for every path it resolves, so the walked root is verbatim
  while the lexically joined expectation is not.

  The case's own comment had asserted the opposite — that an absent leaf
  avoided resolution, and that "a name that resolved would compare a
  canonicalized path against a lexically joined one, and those spellings differ
  on Windows". The absent leaf does not stop `search_root` canonicalizing the
  *prefix above it*, which is precisely what the function exists to do. The
  sibling `checkout_with_nested_directory` in the same file already documented
  the correct rule ("on Windows that carries the `\\?\` verbatim prefix, and no
  path built from the raw spelling equals it"), so the file held both the right
  lesson and the assertion that contradicted it.

  Fixed in the test, as every earlier copy was. The expectation is now derived
  from the **resolved** process directory —
  `fs::canonicalize(current_dir())?.join(&relative)` — which is exactly the
  prefix `reappend` rebuilds, so both sides agree on every platform. Resolving
  the *walked root* was rejected: its leaf is deliberately absent, and on
  Windows `fs::canonicalize` opens the path, so it fails for anything that does
  not exist. That failure mode is the reason `split_at_existing_prefix` exists.
  The comment was rewritten to state the prefix rule and to record the observed
  failure. Focused check:
  `cargo test -p rstest-bdd-harness --lib input_hash::config` — 5 passed.

- [x] **The fourth, fifth and sixth copies of the path-spelling defect.**
  The first Windows run of `4210656c` failed
  `build-test (windows, default features)` in *Test and Measure Coverage* with
  three test failures, all in the cargo-configuration input path. The reported
  roots were `\\?\C:\Users\runneradmin\...` while the fixtures expected
  `C:\Users\RUNNER~1\...`, so the probe *was* finding the configuration and the
  assertions could not see it.

  This is the same defect as the three earlier copies but in the opposite
  direction: the probe canonicalizes the root it walks from, which is
  deliberate — a lexical walk of a relative root never reaches the checkout's
  own `.cargo` — so every path it reports is the resolved spelling. A fixture
  built from the raw `tempfile` directory names the same file with a different
  spelling, and on Windows the two never compare equal.

  Fixed in the tests, not the probe. `cargo_configuration_is_an_input`,
  `a_scratch_directories_configuration_is_an_input` and
  `trybuild_inputs_include_the_workspace_cargo_configuration` now derive their
  expected paths from `fs::canonicalize`, the way the shipped
  `input_hash::config` cases already did. The cases that pass an *absolute*
  `CARGO_HOME` are untouched: that value is used as given and reported through
  the spelling the caller supplied.

  Local Linux cannot prove this fix — `canonicalize` is close to the identity
  there, so both spellings agree and the old assertions passed. The Windows
  lane is the only evidence, which is why the fix was pushed before the
  superseded lanes had finished rather than after.

- [x] **The argument-count finding, fixed as a type rather than suppressed.**
  CodeScene failed `300cdac6` on the advisory rule *Excess Number of Function
  Arguments* for `eviction_of`, which took five positional parameters. Four of
  them described one directory — where it is, what the inspection found, what
  the stamp claims, and whether an empty tree may be excused — so the call site
  was a list whose order meant nothing and nothing stopped a state read from
  one directory being judged against another's stamp section.

  `ProfileUnderJudgement` now groups those four, and `eviction_of` takes the
  aggregate plus the owned package list. This is a structural fix, not a
  waiver: section 4 forbids suppressing a rule to make the gate green, and the
  typed aggregate is also the better code, because the mix-up it removes is
  exactly the one the gate exists to catch. Committed as `184d77da`.

- [x] **A mid-run mutation poisons a preparation stamp.** `make test` at
  `184d77da` failed one test in 0.2 s:
  `rstest-bdd::feature_rebuild_invalidation
  bound_feature_file_is_tracked_dependency`,
  refused as "missing or stale" with an **empty** evicted list. By the rule
  recorded above, an empty list means the fingerprint disagreed rather than the
  artefacts having gone missing, and the stamp written 7.6 s earlier in the
  same run proved the preparation had passed.

  Instrumenting the comparison showed the refusal precisely: toolchain,
  `RUSTFLAGS`, `CARGO_TARGET_DIR`, `RUSTC_WRAPPER`, `RUSTDOC`,
  `RUSTUP_TOOLCHAIN`, `RUSTC_HEARTBEAT_SECS` and the variant all matched
  line-for-line, `all_present` returned `Ok(true)`, and only `inputs` differed —
  `4a77c9a684dba3de` recorded against `9b6fc70b503e655e` wanted.

  Neither number is what the current code computes. An out-of-worktree probe
  over the family's real root list produces `82ac8b3d18fc6ffe`, and a fresh
  `nested_fixture_preparation` run writes exactly that, after which the test
  passes in 1.35 s. So the 16:49:39 stamp recorded a hash that no code path
  produces now: the `crates/` tree changed *while the gate was running*,
  between the preparation's hash and the test's. The gate is only meaningful
  for a quiesced tree, which is the lesson already recorded for
  `list_steps_runs` and now confirmed a second time. The full gate re-run on
  the settled tree passes 2461/2461.

- [x] **`4a77c9a684dba3de` is the cold-preparation hash, not a racing edit.**
  2026-11-19. The entry above read that number as evidence of a mid-run tree
  mutation. It is not: it is exactly what the *unfixed* code computes when the
  scratch tree does not exist yet, and `82ac8b3d18fc6ffe` is what it computes
  once the scratch exists. Both are reproduced deterministically by an
  out-of-worktree probe over the real root list, with nothing else changing.

  The mechanism is a root spelled through `..`. `fixtures::scratch_fixture_dir`
  spells the scratch as `<checkout>/crates/rstest-bdd/../../target/tests/...`
  so it survives a move, and `fixture_preparation` walks it *before* its own
  callback creates the tree. `search_root` resolved the root with
  `fs::canonicalize`, which fails on a path that does not exist yet, so a cold
  run fell back to the lexical spelling. `Path::ancestors` is purely lexical,
  so that spelling then contributed `crates`, `crates/..`, `crates/rstest-bdd`
  and `crates/rstest-bdd/..` to the walk — and the last of those resolves to
  the checkout, so the walk reported the checkout's own `.cargo/config.toml` a
  second time under the `crates/rstest-bdd/../../.cargo` spelling. `input_hash`
  hashes a named root by its full path (`PROVENANCE_KIND`), so the duplicate
  was not merely redundant: it contributed different bytes.

  Cold therefore produced eleven roots and warm ten, and the two hashes above.
  The preparation wrote a stamp under the cold list, every later consumer
  computed the warm one, and the gate refused the stamp **with an empty evicted
  list** — the documented signature of a fingerprint mismatch rather than
  missing artefacts. That is the hosted coverage-lane failure on `37d259a3`,
  reproduced locally.

  The fix resolves the *longest existing prefix* of the walk root and
  re-appends the remainder, so the `..` components collapse whether or not the
  tree exists. Cold and warm now both yield ten roots and `e91239ee98f85c9f`.
  The regression case
  `a_root_named_through_a_parent_contributes_the_same_walk_cold_and_warm` was
  confirmed to fail against the old `search_root` with precisely the duplicate
  entry (`other-crate/../.cargo/config.toml` beside `.cargo/config.toml`) and
  to pass with the fix.

  The lesson for the diagnosis, not only the fix: an "empty evicted list"
  proves the fingerprint disagreed, and a fingerprint disagreement can come
  from the *shape of the root list* rather than from any file changing. The
  earlier search for a mutated input file was looking in the wrong place.

- [x] **Two CodeRabbit inline findings, both real, both verified by reverting
  the fix.** 2026-11-19, against `37d259a3`.

  *`artefacts.rs`, the empty stamp section.* A section naming nothing is the
  record of a legitimately empty profile — a target-selected build leaves its
  secondary profile holding only lock files and `build/`. But the exact layer
  compares the presence list against the recorded names, so with no recorded
  names `missing` is empty however damaged the directory is: the comparison
  refuses nothing. A stamp written while the profile was empty, followed by an
  eviction of `.fingerprint`, was therefore waived, and a timed assertion would
  reuse a tree Cargo cannot treat as current. The empty-section path now
  returns the same structural answer the no-section path already used, factored
  into `structural_eviction` so the two cannot drift. The legitimate case is
  untouched: a wholly empty profile is still admitted beside a usable sibling,
  which
  `state::tests::a_target_selected_preparation_with_an_empty_host_profile_is_admitted`
  covers end to end. `an_empty_stamp_section_cannot_waive_a_partial_layout`
  fails against the old code with exactly the partial layout and passes with it.

  *`input_hash/tests.rs`, a vacuous test.* `coverage_profiles_are_skipped`
  wrote two `.profraw` files into the temporary directory but never named them
  in the `entries` vector, and `hash_sorted` walks only the sequence it is
  handed. Both hashes were therefore taken over the same one-file list, and the
  case passed whatever `is_build_output` did. The profiles are now pushed onto
  `entries`, and one basename deliberately does not start with `default_`, so a
  fix keyed to that prefix fails here too.

  Both findings are the same shape: a check that reads as covering a case and
  does not. The first waived a state its comment said it refused; the second
  asserted a property its fixture never exercised.

- [x] **The gate split into a read-only query and a named build-capable entry
  point.** 2026-11-19, after the `CHANGES_REQUESTED` review on `37d259a3`
  failed the Unit Architecture pre-merge check.

  The review's requirement was that a caller be able to ask whether a family is
  prepared without reaching a build through something named like a query, and
  that the runner decision be passed rather than read from the ambient process.
  ADR-025 had originally declined a public typed query, so the decision needed
  revisiting rather than overriding quietly; the ADR now records a
  `## Superseding decision (2026-11-19)` section and is marked superseded in
  part.

  The decline's stated cost was that a public query would either become a
  second source of truth for "what counts as prepared" or force `profiles.rs`
  and `artefacts/` internals into the public API. Neither is paid:
  `stamp::read_stamp` now returns a `StampReading { fingerprint, decision }` —
  one composition serving both the comparison and the later stamp write — and
  `admit` consumes that same private answer and calls the same
  `artefacts::all_present` predicate the gate does. No derivation is exposed.

  The surface:
  - `admit(preparation, under_nextest) -> io::Result<Admission>` — read-only;
    no build, no stamp write, no lock. `Admission` is `Reusable`,
    `NotPrepared` or `Unreadable(UnreadableStamp)`.
  - `admit_with_probe` — the same query with the toolchain probe supplied.
  - `ensure_prepared_building(preparation, under_nextest, prepare) ->
    io::Result<PreparationOutcome>` — the only entry point that can build, with
    `Reused` / `Prepared` stating whether one ran.
  - `ensure_prepared_building_reported` — the same with `Instruments` supplied.

  `running_under_nextest()` moved out of the gate entirely and is now resolved
  at the call sites that own the test: `TrybuildTree::ensure_prepared`, the two
  workspace preparation suites, and `ensure_gated_with` in `cargo-bdd`.

  `UnreadableStamp` carries the read failure as a `String` because `io::Error`
  is neither `Clone` nor `PartialEq`, and the query's answer is compared in
  tests.

  Seven cases in `preparation/tests/admission.rs` pin the answer and, beside
  it, each of the three refusals: a cold tree answers `NotPrepared` and leaves
  no stamp; a warm tree answers `Reusable` and its stamp comes back
  byte-identical; an evicted profile answers `NotPrepared`; an unreadable stamp
  answers `Unreadable` under Nextest and `NotPrepared` under plain
  `cargo test`; the query answers while another thread holds `PREPARE_LOCK`;
  the probe is called once per query and its identity is honoured.

  Verified: `cargo check --workspace --all-targets --all-features` clean, and
  `cargo test -p rstest-bdd-harness --lib` reporting 259 passed, 0 failed.

- [x] **The trybuild compile-test `slow-timeout` reverted from 30 m to 20 m.**
  2026-11-19, addressing the Linked Issues pre-merge check on `37d259a3`.

  The check's rule is that an assertion deadline must not move to accommodate
  work the change was meant to remove, and this plan's own claim was "No
  assertion deadline needs to change". The 30 m period had been raised earlier
  in this work because `step_macros_compile` took 1119.268 s on a saturated
  shared compile-admission pool — but that was *cold compile* work, and the
  cold dependency graph is now `nested_fixture_preparation`'s job. What remains
  inside the four compile-test bodies is a timed assertion over an already-warm
  graph: roughly one `cargo metadata` preflight per fixture, tens of seconds.
  Keeping 30 m would have hidden exactly the cost this issue exists to move.

  The 1119.268 s figure is retained as motivating evidence for the **30 m
  preparation period**, which is the one period that should be sized for a cold
  build. `.config/nextest.toml`, `nextest_config.rs`,
  `docs/developers-guide.md`, `docs/users-guide.md` and
  `docs/testing-strategy.md` now agree: preparation 30 m, compile tests 20 m.

- [x] **The local timeout and the CI failure are two different faults, and
  only one of them is ours.** 2026-11-19, after gate run 8.

  Local `make test` timed out one test at exactly `1200.003 s`:
  `rstest-bdd::trybuild_macros runner::step_macros_compile`. Hosted CI on the
  same head, `37d259a3`, failed a *different* test in 0.206 s:
  `rstest-bdd::feature_rebuild_invalidation
  bound_feature_file_is_tracked_dependency`.
  Reading them as one symptom would have produced the wrong fix, so they were
  separated before anything changed.

  *The local timeout is environmental, not a regression.* The timed body
  compiled no dependency graph — one `Checking rstest-bdd-tests` in 22.80 s,
  which is the fixture crate itself, exactly the work the change intends to
  leave there. What it did do was wait: 427 `waiting for a compile slot` lines
  with the queue 64 deep, and 682
  `the managed sccache server is not serving compilations` notices spanning the
  whole body. `build-limits-status` during this session showed
  `capacity 4, held 4, healthy` with most of the queue belonging to `netsuke`
  and `gauss` worktrees — other repositories' builds, which this agent must not
  kill. The decisive comparison is the last green CI run on this branch,
  `44511800661`: there `step_macros_compile` passed in **34.890 s** on the
  Linux coverage lane. A test that takes 35 s with a serving cache and a free
  slot does not need a wider period because it took 1200 s queued behind other
  agents. The 20 m revert stands.

  *The CI failure is ours, and it is the root-spelling defect already recorded
  above.* The refusal named the **workspace** target
  (`crates/rstest-bdd/../../target/tests/rebuild-invalidation/...`) while the
  coverage lane ran `--target-dir target/llvm-cov-target`, and the evicted list
  was empty — by `missing_stamp_message`'s own rule, a fingerprint disagreement
  rather than missing artefacts. The preparation test passed in 13.836 s at
  `16:22:20`; the assertion refused the stamp it wrote five minutes later.
  `search_root`'s cold/warm divergence explains it exactly: the preparation
  walked a scratch root that did not exist yet and hashed the lexical `..`
  ancestry, while the consumer walked the now-existing tree and hashed the
  resolved one. That is the same defect as `4a77c9a684dba3de` versus
  `82ac8b3d18fc6ffe`, and it is fixed by resolving the longest existing prefix
  in `search_root`.

  Two `wip` results in the local log (`step_return_alias_error_not_display`,
  `scenario_missing_step_warning`) are also environmental: both fixtures are
  unreached by this change, their tracked `.stderr` files are byte-identical to
  `HEAD`, and trybuild reported "successfully created new stderr files", which
  is what it says when a snapshot comparison ran before the expectations were
  staged in a run that was already being killed. Neither reproduces as a
  snapshot diff.

- [x] **Posting the same disposition from three pool accounts made CodeRabbit
  disengage, and that — not its findings — is what left 26 threads open.**
  2026-11-19.

  Of the 138 review threads on #916, 42 are unresolved and 26 of those have a
  CodeRabbit comment last. Reading them in order showed why. At
  2026-11-18T20:25–20:26 the *same reply text* was posted to the same threads
  from `buzzybee-df12`, `pandalump` and `wafflecat-df12` — three different pool
  accounts — because the token is selected with `shuf` per posting operation.
  CodeRabbit answered each with its disengagement notice ("Seems like the
  humans are having a chat") instead of a disposition, and the threads stayed
  unresolved.

  The lesson is a one-comment-per-thread rule, not a token-selection change:
  the pool is still the required route, but a single reply per thread is
  enough, and its disposition is what puts CodeRabbit back into
  answer-or-concede. Re-posting the same argument under a new identity reads as
  several people talking over each other and costs more than it buys. Reply
  once, cite the commit and the code, and let the single answer stand.

  The three threads that had already received a *substantive* CodeRabbit answer
  before the duplicates are the ones worth trusting: `recording.rs:123` (files
  directly inside per-unit `fingerprint/`), `input_hash/tests.rs:11` (the ext4
  wording), and `prop_tests.rs:227` (`cap_std`). For those the duplicate was
  noise on top of a real back-and-forth.

  A second, independent observation from the same pass: two findings dated
  2026-11-19T16:44:41 — `artefacts.rs:170` and `input_hash/tests.rs:113` — are
  already fixed in the staged tree, so they arrived after the push and will be
  satisfied by the commit that lands the staged work rather than needing new
  edits.

- [x] **A structural assertion is not a recorded-name assertion, and two cases
  were checking the wrong one.** 2026-11-19.

  Both eviction cases in
  `rstest-bdd/tests/feature_rebuild_invalidation/harness/preparation/tests.rs`
  asserted `preparation.directory.is_dir()` before calling the gate — the
  *parent* of the stamp. If the stamp file itself had gone missing the gate
  would still have returned `NotFound`, for the opposite reason, and the case
  would have passed while proving nothing about artefact eviction.

  They now assert `stamp_file(preparation).is_file()` immediately before the
  call. The helper reconstructs the path from the documented rule
  (`.{family}.rstest-bdd-prepared` inside `Preparation::directory`) because
  `stamp_path` and `PREPARED_FILE_SUFFIX` are both crate-private. The comment
  says why the file rather than the directory is the thing to check: a refusal
  caused by a missing stamp reports that nothing was ever prepared, which is a
  different fact from "the artefacts it recorded are gone".

  The same file also gained `require_stamp_under`, called inside the
  `prepared_family` fixture *before* it writes anything. A regression in target
  resolution would otherwise leave a stamp in the shared scratch root and be
  reported only after the damage.

- [x] **`prepare_now` is unconditional about fail-fast, not about the stamp.**
  2026-11-19. `trybuild_macros/preparation.rs` said `prepare_now` "rebuilds
  even when a current stamp is present". It does not: `prepare_locked` returns
  `PreparationOutcome::Reused` when the stamp matches and the artefacts are
  present. The unconditional part is that it never declines *because a family
  was never prepared* — which is what `make prepare-nested-fixtures` needs
  under nextest. The doc now says that, and says the re-run is cheap.

  The gpui and tokio `macro_compile/preparation.rs` files carried a matching
  overstatement: their `COMPILE_FAIL_PROBE` was "never asserted on". Dropping
  the `TestCases` after `compile_fail` does still compare the `.stderr`
  snapshot beside the fixture, so a drifted snapshot fails the preparation. The
  docs now say that instead, and say why it is the right place for it to fail.

- [x] **The eviction-test file crossed the 400-line budget, and the split was
  already named in its own prose.** 2026-11-19. `tests.rs` reached 463 lines
  after the assertions above. `prepared_family`'s doc already said the cases
  "share the same four steps", so that machinery — `stamp_file`,
  `require_stamp_under`, `child_env`, `populate_profile`, `PreparedFamily`,
  `prepared_family`, `evict`, `scratch_fixture` — moved to a sibling
  `tests/prepared.rs` (241 lines). `tests.rs` is now 253 lines and holds only
  the claims about the gate.

  The module is called `prepared`, not `fixtures`, because
  `crate::harness::fixtures` already exists and a second one shadowed it in
  every `use` in this tree. `check_rs_file_lengths.py` exits 0 with no
  allowlist entry.

- [x] **Gate results on the settled tree, 2026-11-20.** `make check-fmt`
  exit 0 (`129 files left unchanged`); `make markdownlint` exit 0
  (`Summary: 0 error(s)`, typos gate clean); `make lint` exit 0;
  `make typecheck` exit 0 (`All checks passed!`).

  `make test` reached **2432 passed, 1 timed out, 7 skipped** of 2470, and the
  timeout was `rstest-bdd::trybuild_macros runner::step_macros_compile` at its
  own 1200 s bound. Diagnosed rather than assumed:

  - The two stamps the timed test gates on were **reused**, not rebuilt — the
    log says `family=trybuild-macros is prepared` and
    `family=trybuild-ui-clippy is prepared`. So no cold preparation leaked into
    the timed body; the failure is not issue 910's defect returning. To be
    precise about what that proves: reuse means the **preparation callback did
    not run**, not that the timed command compiled nothing. The timed suite's
    own trybuild cases compile whatever they compile regardless; the claim
    here is only that the dependency preparation this change owns had already
    happened.
  - The timed window carries **1153** `[build-limits]` lines, including
    `waiting for a compile slot (60 ahead, 45s)` and repeated
    `managed sccache server is not serving compilations`. `build-limits-status`
    reported `capacity 4, held 4, healthy` with ~40 compilations queued, and
    several other agents were running `make test` on this host, one of them in
    this same repository.
  - Inside that window 52 of the 55 trybuild cases that ran passed. The last
    one, `result_fixture_requires_result_scenario.rs`, was still waiting when
    the bound expired.

  **Hosted CI on the same commit is the decisive comparison**: run 44561443682
  shows 0 `wip/` writes and no timeout. Its only failure is
  `bound_feature_file_is_tracked_dependency`, which is the CI fault already
  fixed in this tree by `search_root`'s longest-existing-prefix resolution. The
  local timeout therefore does not reproduce off this host's compile queue.

  Two `wip/` files did appear locally (`step_return_alias_error_not_display`,
  `scenario_missing_step_warning`), and **both were absorbed, not failures.**
  That is what these two cases are built to do: each is queued through
  `compile_fail_with_normalized_output` rather than plain `t.compile_fail`, so
  `trybuild` writes `wip/`, the normalizers run over both sides, and
  `normalized_outputs_match` deletes the file when the normalized texts agree.

  Replaying both normalizer chains outside the test proves the agreement. For
  `step_return_alias_error_not_display` the actual output carries the extra
  `conditionally` wording *and* the `----` underlining plus an
  `unsatisfied requirement introduced here:` block; the normalizer erases all
  three, and the two sides then compare equal. For
  `scenario_missing_step_warning` the two sides are already byte-identical after
  `normalize_fixture_paths`. Both `.stderr` files are byte-identical to HEAD,
  and `wip/` is empty after the run, which is the observable signature of both
  cases having passed.

- [x] **The preparation-budget tolerance fired, and was recorded rather than
  raised.** 2026-11-20. `make prepare-nested-fixtures` on a contended host:
  `cargo-bdd::cli` prepared in 23.8 s and `feature_rebuild_invalidation` in 0.5
  s (both warm), while `rstest-bdd::trybuild_macros` prepared its family
  successfully but took **1279.8 s** and was then terminated by its own 30 m
  `slow-timeout` at 1800.003 s.

  The preparation itself succeeded — its own log line says
  `nested preparation: family=trybuild-macros prepared in 1279.8s
  (0 Compiling lines)`
  and the 109 KB stamp was written. What overran was the whole preparation
  *test*, which also carries the queueing time behind the `cargo-spawning`
  group. A first attempt had been killed by my own background time limit at 435
  s, mid-compile, not by the repository.

  This is the plan's own Budget tolerance, exactly: "if a single preparation
  test exceeds its 30-minute budget on a cold two-vCPU runner, that is evidence
  the budget or the family split is wrong; record the measurement and revisit
  rather than raising the number." Recorded. Two facts bound how much weight it
  carries: this host had ~4 concurrent foreign `make test` jobs, load 19–38 on
  24 CPUs, and `sccache` was not serving, so it is not the two-vCPU runner the
  tolerance names; and the hosted CI run 44561443682 does not time out.

- [x] **The `20 m` trybuild period is a revert, not a tightening.** The branch
  head had widened that override from `origin/main`'s `20m` to `30m`, which the
  plan's first constraint forbids ("Do not widen an assertion deadline"). The
  staged working tree restores `20m`, and `nextest_config.rs`
  (`trybuild_nextest_override_preserves_timeout_contract`) asserts `20m`
  accordingly. The point is not that 20 m is generous — it is that the reason
  this issue exists is that the *cold* compile must not happen inside that
  period, so the correct response to a timeout is to check that the stamps were
  reused, not to buy more time.

- [x] **A `target/` eviction between two of my own commands, cause unknown.**
  2026-11-20. The `trybuild-macros` stamp was present and 109063 bytes at
  02:05; by 02:38 the entire worktree `target/` tree was gone (`target/debug`
  recreated 02:51, `target/tests/` absent). The focused re-run that had been
  intended to prove the timed body fits then failed instantly with
  `the artefacts it vouched for are gone (/…/target/tests/trybuild (absent))` —
  which is the stamp doing its job, and confirms the gate's message names a
  real cause.

  Ruled out: no `CARGO_TARGET_DIR` in the environment, `target/` is not a
  symlink, `~/.cargo/config.toml` sets only the compiler wrappers, no
  syncthing, and no `cargo clean`/`rm -rf target` in any agent log I can read.
  Disk was not full (178 G free, 90 % used). Recorded as unexplained rather
  than attributed to this change: nothing in the diff can delete a target
  directory, and a preparation stamp refusing a vanished tree is the designed
  behaviour either way.

- [x] **Close the review findings still held open at `92369bc4`.** 2026-11-20.
  Five were code or test gaps and are fixed on the branch:

  - The CLI family declared packages only, so the fixture's two test binaries
    (`diagnostics_fixture`, `unsupported_dump_steps`) were never recorded and
    their removal passed the gate. They are now declared beside the closure
    (`60faa67f`). The mutation was checked: with the declaration removed, the
    new regression fails.
  - The CLI witness accepted the binaries from any named profile. Under
    `CARGO_BUILD_TARGET` it now reads the selected profile alone (`60faa67f`).
  - The server trybuild preparation warmed build mode only; it now probes the
    compile-fail fixture too, as the Tokio and GPUI trees do (`c8184abf`).
  - The two-probe sequence and the CLI closure query each gained a regression
    that drives a slow, successful command into the next one (`857a9e43`).
  - A family whose callback creates its working directory was identified
    before that directory existed. `prepare_now_staged` and
    `ensure_prepared_building_staged` run a staging step first, and both
    rebuild-invalidation families use them (`2689423f`). A `trybuild` family
    cannot stage this way, because `trybuild` creates the generated crate
    itself; that crate holds no toolchain file and no compiler.

  The user-facing documentation for `binary_test_support` was added
  (`d57d2230`). Reading the source against the request showed the lookup order
  is unchanged from `main`: only `target_directory_of` is new, so the documents
  state the behaviour and do not call it a change.

- [x] **Follow-up findings on the fixes above, and on the workflow driver.**
  2026-11-20. Review of the commits above produced a second set of findings,
  each narrower than the last. They are recorded here because several were
  defects in code written that same day.

  - Staging ran outside the preparation lock, which the copy had been under
    while it lived in the preparation callback. `stage_locked` restores that
    (`fddebb47`); a two-thread case fails without it.
  - Under nextest an unstaged family went on to a compiler probe that could
    not succeed. The staged gate now refuses it with the preparation command
    (`a4a7e3f6`).
  - The CLI gate's budget hand-off moved into `preparation/budget.rs`. The
    query runs as soon as its bound is taken and the description is built
    from its answer, so the order is fixed by what each function is given
    (`2b9b023b`, `49622156`).
  - `scripts/prepared_fixture_step.py` stopped waiting once the step's own
    process had exited, so a descendant could outlive a timeout and the
    driver then removed the directory under it. Termination now lives in
    `scripts/prepared_fixture_tree.py`. It waits for the process *group*,
    sends both signals before the leader is reaped so the group id cannot
    have been reused, raises `TreeSurvivedError` when the tree cannot be
    confirmed gone, and never waits without a bound. The driver keeps the
    target directory in that case. An interrupt that arrives while the
    step's process is unreaped is cleaned up the same way. One that arrives
    after `Popen.communicate` has reaped an exited process is not: the group
    id may then have been reused, so the group is sent no signal, is only
    asked whether it exists, and is reported as a surviving tree if it does,
    which again keeps the directory.

  One lesson for the next change to that driver: three of its new tests failed
  the Windows lanes, because they signal process groups and `os.getpgid` does
  not exist there. `make test` runs `scripts/tests` on every lane, so a
  POSIX-only case needs a skip.

- [x] **Deliver the review items that were confirmed and queued but never
  landed.** 2026-11-20. An audit of the unresolved threads before the final
  approval request found six replies from 18 November that accepted a finding
  and promised it "for the follow-up batch". Two had since been done. Four had
  not, and are done now:

  - The rebuild-invalidation families claimed their five workspace crates
    and no registry package, so an evicted `inventory` passed the gate. They
    now claim their lockfile's packages through the new
    `Preparation::owned_if_built`, which checks a package that was built and
    excuses one that was not. The lockfile was preferred to `cargo tree`
    because the named preparation test already runs three bounded commands,
    and `3 x 9 m` leaves no room in its 30-minute period for two more.
  - The CLI closure query runs in the fixture build's environment, not the
    test process's.
  - The hash property renames a leaf in place, so it varies only the name.
    Names reach the hash twice, in the directory listing and in the file
    entry, so removing one alone does not fail it; removing both does.
  - The withdrawal case starts from a seeded stamp, and four witness cases
    share the `deps_directory` fixture.

  The lesson is about the ledger, not the code: a reply that says "queued" is
  an open item until a commit closes it, and it should be tracked as one.

## Surprises & Discoveries

- **A green lint run is a temporary observation, not a property of the tree.**
  2026-11-17. Fixing `vendor/gpui` cleared the `bumpy_road_function` finding
  the suite reported at 00:39Z, and the local rolling-suite check then passed.
  Five hours later the same rolling suite flagged `write` and `write_escaped` in
  `crates/rstest-bdd/src/reporting/junit.rs`, which had not changed since the
  branch began and which earlier heads had passed. The suite is republished
  continuously and consumers track the default-branch tip; only the installer
  is pinnable, and `install-whitaker` refuses `suite-version` outright. So a
  pass certifies the tree against one snapshot of a moving checker. The
  practical consequence is to re-run the lint gate against a freshly extracted
  suite after any republish, and to expect findings in untouched first-party
  code rather than treating the first clean run as closure.

- **A single-threaded test group turns one slow test into a cancelled suite.**
  2026-11-17. `.config/nextest.toml` puts every `cargo`-spawning test in
  `cargo-spawning` with `max-threads = 1`, and every test in
  `crates/cargo-bdd/ tests/cli.rs` is `#[serial]` in that group. When
  `rstest-bdd::trybuild_macros preparation::nested_fixture_preparation` burned
  its 30 m period, it was holding the only slot: 42 of that binary's tests
  never started, and the run reported `1 timed out` while silently skipping the
  test the change edits. "2246 passed" therefore said nothing about this PR.
  The lesson is to read which tests a green-looking number excludes, and to
  check a changed test's own result rather than the suite total. Nextest names
  `binary_id(cargo-bdd:: cli)` in its own override, so the config already knew
  this family competes for one slot.
- **Two full-suite timeouts in a row is a budget fact, not a flake.**
  2026-11-17. `make check-prepared-fixture-workflow` failed twice on its own
  `STEP_TIMEOUT_SECONDS = 1800` at step 2 (`make prepare-nested-fixtures`),
  with load average near 26 and roughly 270 compiler requests queued across
  seven other repositories. The same gate had completed all four steps at 02:44
  on byte-identical code files (the only later edit is ExecPlan prose, which is
  not an input to it). Repeating a third time would have consumed another 53
  minutes to reproduce a number already explained; the honest record is one
  green run plus two documented environmental failures, not a claim that the
  gate is green *now*.
- **Compile admission is worth watching before blaming the code.** The failing
  `make test` log carried 144 `waiting for a compile slot` lines and 72
  `sccache server is not serving compilations` notices, and the timed-out
  test's last stderr was a successful `Finished dev profile in 4m 39s` — it was
  progressing, not stuck. `build-limits-status` reported capacity 4 with 4 held
  and a `live_work` drain fault accompanied by a `cache recovery` line, which
  the host rules define as automatic recovery needing no operator action.

- **Evidence about the gate is not evidence about the command.** 2026-11-17.
  CodeRabbit located this and was right. `nested_build::record_reuse` is called
  from `ensure_gated_with`, which is the *gate*, and it writes `0` because a
  gate that admitted a stamp ran no preparation. The timed command is
  `run_cargo_bdd_steps`, and it had not run yet when that zero was written.
  Step 3 therefore reported "the gate skipped preparation" as though it were
  "the timed command reused the tree" — a true statement about the wrong
  subject. The count could not answer the question the step existed to ask, and
  the earlier entry above, which reads as though attribution had solved this,
  had only fixed *which* lines were judged, not *whose* work they described.
  `run_cargo_bdd_raw` already returns a full `Output`; `run_cargo_bdd` discards
  its stderr as `_stderr`, so the evidence existed and was thrown away one call
  above where it was needed. Fixed by a sibling that keeps both halves and
  files the timed command's own count from its own stderr, at the last point
  the text is in hand. A verified run now leaves three lines: `51` from step
  2's preparation, then two zeros from step 3 — the gate's and the timed
  command's. Two zeros is the whole fix; only the first existed before. Lesson:
  when a check consults a count, ask which command produced it, and whether
  that command has run yet.
- **A rolling lint suite makes a green head a temporary observation.**
  2026-11-17. Head `409a3884` failed the hosted Linux `Lint` step on
  `vendor/gpui/src/lib.rs:181` with `bumpy_road_function`, skipping the new
  prepared-fixture step as collateral, while the two Windows lanes passed. No
  change in this PR caused it: `vendor/` is byte-identical across the branch,
  and the *same head* had passed full CI six hours earlier. Upstream
  `leynos/whitaker#641` merged at 00:34:26Z, the rolling lint archive was
  republished at 23:26Z, and the `install-whitaker` action refuses
  `suite-version` outright — "the Whitaker lint suite is a rolling release and
  is never pinned" — so only the installer is pinned and every consumer tracks
  the default-branch tip. The new charge is that `?` now costs a branch, a
  nesting depth and flow weight 2.0, "as the handwritten early-return `if` it
  abbreviates", which pushed `run_test`'s existing loop-and-retry shape over
  the threshold. Lesson: a locally installed suite older than the head cannot
  reproduce a hosted Whitaker finding, so a local `make lint` pass proves
  nothing there; verify against a freshly extracted rolling suite, or trust
  only CI.
- **One republish, three sites: sweep the workspace, do not chase the lane.**
  2026-11-17. The same rolling charge has now surfaced at three distinct sites
  in this PR's lifetime, one crate per CI cycle: `vendor/gpui/src/lib.rs:181`
  (`run_test`), then `crates/rstest-bdd/src/reporting/junit.rs:33` (`write`) and
  `:109` (`write_escaped`), then
  `crates/rstest-bdd-macros/src/codegen/wrapper/args/extract.rs:165`
  (`extract_args`). Each was found by a hosted lane failing, fixed, pushed, and
  then the *next* lane found the next one — three round trips through a CI that
  takes tens of minutes per head. The suite charges every early exit, so the
  expensive shapes are uniform even where the causes differ: `?` counts as a
  branch (`leynos/whitaker#641`), and so do `continue` and `break`-style early
  exits, which is what forced the `junit.rs` guard rewrite and the
  `continue 'args;` elimination. Both fixes therefore had the same shape:
  replace the early exit with a call to a helper that decides and returns.
  Lesson: once a rolling suite has flagged two sites it never checked before,
  stop paying per-cycle discovery and run the freshly extracted suite over the
  **whole workspace** (`--workspace --all-targets --all-features`) once, to
  enumerate every remaining site in a single pass. That sweep is now clean and
  is the reason this commit claims completeness rather than another green
  observation.
- **A quarantined local suite is a fact to check, not to assume.** 2026-11-17.
  The local suite's library was dated 2026-11-15 05:17, so `make lint-whitaker`
  with `CARGO_FLAGS` narrowed to `-p gpui` exited `0` and reported nothing
  while CI reported the finding on the same tree. Extracting the republished
  archive into its own empty directory, symlinking it to the toolchain-suffixed
  name the driver requires (`libwhitaker_suite@nightly-2026-05-28.so`), and
  nesting the driver *through* `build-limits-rustc` reproduced CI's exact
  diagnostic locally — same two bumps, same 3- and 16-line spans — which is
  what made the fix iterable in one edit instead of one round trip per
  45-minute CI run. Lesson: reproduce the hosted failure locally before
  reasoning about the repair; the cost of the harness is minutes, and the cost
  of CI-only iteration is hours.
- **An accumulating evidence file needs attribution, not a stricter value
  test.** 2026-11-17. Step 3 first read the whole report file and required
  every count in it to be zero. Its first real run failed with
  `counts [51, 0]`. An isolated probe of step 1
  (`CARGO_TARGET_DIR=$PWD/target/probe-step1 make nextest-run …`) showed step 1
  writes *no* report at all — the gate refuses before reaching `record_reuse` —
  so `51` was step 2's cold preparation and `0` was step 3's genuine reuse. The
  driver was observing real evidence; the check was wrong, because it could not
  tell the lines a step wrote from the lines already there. Fixed by
  snapshotting `len(read_build_reports(target_dir))` before the judged run and
  reading `counts[previous_counts:]`, and the same attribution was given to the
  negative control, which had the same weakness hidden behind `counts[-1:]`. A
  verified run then left exactly two lines, `51` then `0`, confirming that
  steps 1 and 4 contribute nothing because both refuse. Lesson: when one file
  accumulates a line per run, "all the evidence" and "this run's evidence" are
  different questions, and the wrong one fails for the step that is supposed to
  come before.
- **A negative control has to ask for a state the design permits.** 2026-11-17.
  Step 4 originally required the run after a fixture edit to report a *nonzero
  compile*. It failed on the first end-to-end run, and the reason was the
  design, not the code: once the stamp is admitted the gate prepares nothing,
  and once it is stale the gate refuses rather than preparing — the nested
  build happens on the remedy path only. Asking the timed run for a compile
  asked for a state the gate can never produce. The control now requires the
  next run to *refuse and name the remedy*, which is the actual mechanism (the
  stamp hashes the fixture inputs) and is also stronger, since naming the
  remedy distinguishes the stamp's refusal from any other failure wearing the
  same exit status. Lesson: a control that fails is sometimes telling you its
  premise was impossible; check the mechanism before weakening the assertion.

- **`make check-fmt` is not the Markdown gate, and `mdtablefix --wrap` can
  emit a line markdownlint rejects.** 2026-11-16. The two formatters disagree:
  `mdtablefix --wrap` wrapped one prose line to 81 characters, while
  `.markdownlint-cli2.jsonc` sets `MD013` to 80. `make check-fmt` runs
  `mdtablefix --check` and passed, so the local gate was green while CI's
  `Markdown lint` step failed on `docs/execplans/...:1898`. The `--wrap` output
  is therefore not conformant by itself: after wrapping, markdownlint is a
  separate gate and it is the one that decides. Two habits follow. Verify a
  Markdown edit with `make markdownlint`, not `make check-fmt`, because the
  latter cannot see `MD013`. And when a hosted lane fails on something a local
  gate passed, suspect the gate's *coverage* before the code: the failure was
  one column of width that no local command I had run would report.

- **A third copy is what turns two tolerated functions into a CodeScene issue.**
  2026-11-16. The contract test already had two near-identical `*_is_phony`
  functions, and the hosted `CodeScene Code Health Review` passed on every head
  that had only those. Adding a third, for the driver target, produced a
  `CodeDuplication` finding and turned the check to FAILURE — the same shape
  that had been below the threshold twice became a defect on the third. So the
  failure was not caused by the new target being wrong; it was caused by the
  new test being the copy that tipped the module over. The repair is the one
  CodeScene asked for and was worth making on its own merits: collapse the
  three into `test_every_target_is_phony[target]` over the target names, and
  lift the shared `.PHONY` read into `phony_lines()`. Lesson: when a quality
  gate fails on a change that "only adds one more of the same", the addition is
  the trigger but the duplication is the defect, and suppressing or reordering
  the check hides a real simplification. Confirm the fix with the same tool
  that reported it — `cs delta origin/main` — rather than with the local suite,
  which never had an opinion about duplication.

- **A line-count lint is a real design signal, and here it named the right
  seam.** 2026-11-16. `preparation.rs` sat at 391 lines against the Whitaker
  suite's 400-line `module-max-lines` limit, and the reporting contract I added
  for the nested-build evidence pushed it to 459. The tempting repair is
  `#[allow(module_max_lines)]`; the cheap repair is to move *anything* out.
  Both would have been worse than what the limit was pointing at, which is that
  the module held two concerns: the family's description and gate, and the
  nested build plus the report it files. Splitting them left `preparation.rs`
  at 365 lines and put the marker, the variable name and the line format
  together in `preparation/nested_build.rs`, where a reader looking for the
  contract finds it as one thing. The same shape recurred in Python within the
  hour: the driver hit pylint's 400-line `too-many-lines` at 537, and the seam
  was again run-a-step versus judge-the-outcome, giving
  `prepared_fixture_step.py` for spawning, bounding and the report channel.
  Lesson: when a size limit trips, read it as a question about cohesion rather
  than a quota to satisfy, and split on the axis that makes each half
  explainable in one sentence — the suppression and the arbitrary split both
  leave the reader paying.

- **Filtering both sides by one rule makes an unowned name equal to itself.**
  2026-11-15. The ownership layer keeps only names `name_is_owned` accepts, and
  `eviction_of` filters the *present* list by the same predicate. That is
  consistent, and it hides a whole eviction: a package missing from
  `owned_packages` is absent from the recorded list *and* from the present
  list, so `missing` is empty and the gate admits a profile whose dependency
  fingerprints were removed — silently, because there is no name left to
  report. The CLI fixture declares `rstest-bdd-cli-fixture` alone, and one
  `cargo build` of it also builds `rstest-bdd` and `rstest-bdd-macros`, so that
  was the live case. The fix is not in the filter: only the caller knows what
  its build pulled in, so `owned_packages` now documents that it must name
  every package the preparation built, and the CLI call site chains the
  fixture's path dependencies onto its own name. A first attempt added an
  `ownership::with_dependencies` helper to derive the list from the tree; it
  was deleted. A heuristic that guesses ("names containing `fixture`") would
  under-claim quietly, which is the same defect with more code, and one that
  returns its input unchanged has no job. The regression is
  `tests/ownership.rs::evicting_a_dependency_owned_through_the_fixture_invalidates_the_preparation`,
  proved to discriminate by reverting `owned_packages` to the fixture alone
  and watching it fail with the empty error the silent path produces.

- **A rule about names cannot decide an eviction in a shared profile.**
  2026-11-15. The replacement rule was written for "a unit was rebuilt under a
  new hash", and it is correct for a profile one family owns. In a *shared*
  profile it is inert: a family that recorded every name it found had recorded
  both the old and the new generation, so `present - recorded` was empty and
  the rule had no candidate to compare against. The fix is not a better rule
  but a narrower claim — each family records only its own packages' units. When
  a heuristic has to distinguish two indistinguishable tree states, the answer
  is usually to stop asking the tree.

- **A hand-rolled `RUSTFLAGS` stales every stamp it touches.** 2026-11-12. The
  preparation fingerprint covers `RUSTFLAGS` *and* the feature selection, so
  `RUSTFLAGS="-D warnings -Clink-arg=-fuse-ld=mold" cargo nextest run -p …
  -E 'test(…)'`
  prepares and reads a different tree from `make prepare-nested-fixtures`,
  which adds `--workspace --all-targets --all-features`. The failure is
  immediate and looks like a missing stamp rather than an environment mismatch,
  and its own message names the cause only if you read to the end. Run timed
  families through the Makefile's recipes, and pass the same `CARGO_FLAGS` when
  running one alone.

- **`allow-expect-in-tests` stops at `#[cfg(test)]`.** 2026-11-11. A
  `#[test]` body and a `#[cfg(test)]` module are covered; a plain helper `fn`
  in an integration-test file under `crates/*/tests/` is not, even though
  nothing outside the test harness can call it. `-D warnings` turns that into a
  build failure with a message pointing at a line that is obviously test-only,
  which is why it first read as a false positive. The fix that survives review
  is to make the helper fallible and let the `#[test]` body assert, rather than
  to silence the lint.

- **A declined finding is not closed because the first reason was plausible.**
  2026-11-11. The `current_dir()` row was declined on the grounds that
  threading the error would change "documented public API behaviour".
  `git grep` against `origin/main` then showed that neither
  `absolute_target_directory` nor even `environment.rs` exists there: both are
  new in this PR, so there was no established API to protect. The decline had
  been reasoning about a codebase that does not exist. Re-measure a decline's
  premises against the base branch before reusing it, especially when the
  function under discussion was introduced by the branch itself.

- **A success-path drain has to be bounded, and the bound costs output.**
  2026-11-11. `collect_until(now + DRAIN_GRACE)` runs on the exit path too, not
  only after a kill. That is what keeps a successful `cargo` whose grandchild
  holds the pipe from blocking for the grandchild's lifetime, but it means the
  captured text is the direct child's. Both halves are now stated in the users
  guide and the migration guide rather than left to be discovered from
  `bounded.rs`.

- **`markdownlint`'s MD013 is not `mdtablefix`'s wrapper, and only the hosted
  gate catches the difference.** 2026-11-11. Rewrapping a paragraph by hand to
  80 columns satisfied `make markdownlint` with 0 errors and looked correct, but
  `4870b2c4` failed the hosted `build-test (linux, default features)` lane at
  "Check formatting" — a step this branch had passed on every earlier head. The
  local `make fmt` had run `mdtablefix --in-place` and rewritten the paragraph
  afterwards, so the working tree never held the hand-wrapped form while a gate
  was reading it. `mdtablefix` fills each line greedily to the limit rather
  than honouring sentence boundaries, so a hand wrap can be 80 columns and
  still wrong: the difference was `builds` belonging one line earlier. The
  lesson is ordering, not content. `make fmt` before `make check-fmt` is the
  required sequence, and after any hand edit to Markdown prose the fixer has to
  run again before the checker means anything.

- **A generator that draws both sides independently cannot reach a relation
  between them.** 2026-11-10. The stale-stamp defect was a stamp written for
  one value being admitted as another — `h@@ recorded-artefacts` matching `h`.
  I first "fixed" the property by drawing `inputs` and `other_inputs`
  separately with an assume that they differ, and the mutation still survived:
  two independent draws are almost never related as a value and one of its
  proper prefixes, so the ratio of useful cases to generated cases is
  effectively zero no matter how many run. Deriving the second value from the
  first (`format!("{inputs}{append}")`) makes the relationship certain, and the
  mutation then fails on the minimal input `inputs = ""`. The rule is that a
  property about a *relation* has to generate the relation, not hope to sample
  it — `proptest` samples the input space uniformly in spirit but not in
  structure, and structural coincidences are exactly what a prefix, suffix, or
  containment bug depends on.

- **A delimiter written into a data field is a parser problem, and only the
  generator can prove the parser survives it.** 2026-11-10. `parse` split the
  stamp on `@@ recorded-artefacts` and then compared the head with the expected
  fingerprint. Every named test passed, because none of them put the delimiter
  inside a value. Adding it to `inputs` failed on the first run, with `render`
  and `parse` disagreeing about a stamp neither had any question about. Two
  things follow. First, "write then read" is only a round trip if the reader
  does not search for a token the writer copied from arbitrary data, so the
  safe order is to consume the known quantity first and search only in what is
  left. Second, the property that is supposed to establish the round trip could
  not find this either until the generator's alphabet contained the token: the
  previous alphabet held the characters that could forge a *line* (`\n`, `\r`,
  `\`, `:`, `=`), and a delimiter is a *string*, not a character, so no
  combination of its letters would reliably assemble the whole token under a
  length bound. A generator has to draw the tokens the parser searches for, not
  just the characters it treats specially.

- **A per-file code-health score is a budget, and the budget is already
  spent.** 2026-11-10. Adding one ordinary four-assert test to
  `nested_cargo_env.rs` failed the CodeScene gate, and none of the four
  functions it named was the one that crossed a line: the rule's threshold is
  three assertion *blocks* per file, `main` already had three, and the fourth
  made the file unhealthy however good the new test was. Reading `cs review`'s
  per-function list therefore answers "which functions are blocks?" but never
  "which addition spent the last slot?", so it invites fixing the wrong
  function — three of the four were pre-existing and untouched.
  `cs delta origin/main HEAD` is what exposes the budget, because it reports
  `value-before: 3, value: 4` alongside the threshold. The fix was not to touch
  the old functions or suppress the rule, but to make the new test's claim
  smaller: two asserts about two pipes became one assert about their
  concatenation, which is also the more honest statement of what the code does,
  since one collection drains both. The general lesson is that a file-level
  health rule makes an addition cost depend on what the file already contains,
  so a clean diff can still regress the file, and the cheapest remedy is
  usually to make the new claim tighter rather than to rework code nobody asked
  about.

- **A property can hold the boundary still and still be unable to reach it.**
  2026-11-10. The stamp's read-back contract is "a written fingerprint admits
  exactly itself". The first property for it passed under a `trim_end`
  mutation, so I strengthened the generator to emit trailing spaces and tabs —
  the characters `trim_end` eats — and it *still* passed. The generator was not
  the problem. Both properties varied an environment variable and held `inputs`
  constant, and `inputs` is the *last* field `compose` writes, so it is the
  only one whose trailing whitespace the comparison can reach. No amount of
  further generation would have fixed a property that moved the wrong field.
  The habit this changes: before trusting a property to pin a contract, mutate
  the implementation and check it fails — if it does not, ask *which field,
  which position, which shape* the mutation needs to be visible, rather than
  widening the input space. The named regression over the real `inputs` field
  is what caught it, immediately.

- **A test that passes the two halves separately can still leave the seam
  untested.** 2026-11-10. Two existing tests covered the toolchain field:
  `distinct_toolchain_identities_produce_distinct_fingerprints` compared two
  `compose` calls, and `the_recorded_toolchain_reaches_the_fingerprint`
  asserted the identity appeared in the text. Neither touched `fingerprint`,
  the function that actually resolved the probe, and both would have passed if
  that function had discarded the probe's error and composed an empty identity
  — which is a stamp that outlives the compiler it describes. The general
  shape: when a boundary is "resolve a fallible thing, then compose from it",
  testing the composition is not testing the boundary, and the resolution has
  to be injectable or the failure is unreachable from a test. Making `probe` a
  parameter is what closed it, and the same parameter then had to be threaded
  through `ensure_prepared` and `prepare_now` or the gate would still have been
  the untested part.

- **"Final head" is a snapshot, not a property.** 2026-11-10. Several entries
  below name a commit as the final head and cite the run that measured it. Each
  was accurate when written and none of them is the branch tip now: a review
  round added the boundary tests and ran the gates on the merged tree, and
  `main` advanced far enough to conflict on `docs/CHANGELOG.md`, which required
  a merge commit. Read those entries as *the head at the time of measurement* —
  the numbers they report are valid for the tree they name, and a later head
  must be measured again rather than inheriting the verdict. The same rule
  applies to any "all gates green" claim: it belongs to a commit, not to the
  branch.

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
  `.cargo/config.toml` by walking up from the working directory and also reads
  `$CARGO_HOME/config.toml`, so any of those files' development profile options
  and Linux linker flags change dependency artefacts. `cargo_config_inputs`
  hashes every `.cargo` directory on that walk — the workspace root and each
  ancestor — plus `$CARGO_HOME` when it is set. Host configuration was excluded
  at first, on the argument that stamps should be reproducible from the
  checkout alone; that was unsound, because a `CARGO_HOME` build-flag override
  changes the nested build's output and a stamp that never saw the file would
  survive the change. Configuration *below* the workspace root still needs no
  listing: those files already sit inside the hashed `crates/` and fixture
  trees.
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
  period, whichever comes first. EOF remains the normal path, so the grace
  period is pure overhead only in the case that used to hang, and it is bounded
  overhead rather than none. Captured output is a *possibly truncated prefix*,
  not a complete transcript: a descendant can outlive the direct child while
  still holding the write end of the pipe, so "the child exited" does not mean
  "everything has arrived". Only EOF promises complete output, and callers must
  not read a partial capture as the whole of what was written.
- **The preparation override goes first in the file.** Nextest's first-match
  precedence means position, not priority, is what keeps the preparation from
  inheriting a timed assertion's budget.

## Surprises & discoveries: the packaging boundary

- **A fallible signature is a breaking change to the *published* surface, not
  only to the workspace.** `build_child_env` returning `io::Result` compiled
  everywhere in the workspace, so every local gate passed. `cargo package`
  strips workspace `path` overrides, so the verification build of `cargo-bdd`
  compiled against the *published* `rstest-bdd-harness` 0.6.0, which never had
  that signature, and failed with

  ```text
  error[E0599]: no method named `with_context` found for struct
  `NestedCargoEnv` in the current scope
     --> src\registry\mod.rs:254:6
  ```

  Only the hosted `Publish dry run` step, on all three lanes, could see it. The
  lesson is that `cargo check` and `cargo test` cannot tell whether a
  workspace-internal API change is packageable; `cargo package` is a
  *different* compiler input because of the stripped paths.
- **`--allow-unpublished-workspace-deps` does not rescue a compile error.** It
  downgrades only the index-lookup failure that lading recognizes — the
  `failed to select a version for the requirement` plus
  `location searched: crates.io index` pair. A version too low for the API
  produces a compile error instead, which lading raises as a preflight failure.
  The fix is a version that names the release train, not a weaker gate.
- **The version bump is the documented remedy, not an unrelated change.**
  `--allow-unpublished-workspace-deps` exists precisely for "a release train
  whose workspace crates share a new version before any crate has been uploaded"
  (`docs/releasing-crates.md`). `docs/v0-7-0-migration-guide.md` already filed
  this fallibility as a v0.7.0 change; the version was simply never bumped.
  After `lading bump 0.7.0`, `cargo package` fails with the index-lookup
  failure the gate tolerates instead of a compile error, and
  `make publish-check` exits 0.
- **Three couplings live outside `lading bump`.** `PUBLISHED_GPUI_E2E_VERSION`
  in the `Makefile` and the published-GPUI E2E fixture's pins and patch paths
  are asserted against the workspace version by
  `scripts/tests/test_published_gpui_e2e_lockfile_contract.py`, so the bump is
  incomplete without them. The fixture lockfile is then stale; relink it with
  `cargo update --workspace`, which touched six lines, rather than
  `cargo generate-lockfile`, which re-resolved all 786 packages and churned 888
  lines for the same result.
- **The repository's own `.git/config` rewrites SSH to HTTPS.** It carries
  `url.https://github.com/.insteadof git@github.com:`, so a push to an
  `git@github.com:` URL is transparently downgraded to HTTPS and prints the
  Lody shim's output despite `GIT_SSH_COMMAND=/usr/bin/ssh`. Override the
  rewrite for the single command with
  `-c url.git@github.com:.insteadof=https://github.com/`; a read-only
  `ls-remote` under the same override confirms the SSH transport actually
  reaches the repository before a push is trusted.
- **A test this issue did not write still belonged to its defect class.**
  `binary_test_support_cargo::locate_or_build_returns_command_for_workspace_binary`
  builds the `todo-cli` example through the shared helper, so it is a cold
  nested Cargo build. It had no nextest override and was not in the
  `cargo-spawning` group, so it competed with the preparations. Run 43607129223
  killed it at exactly `60.002s` while the sibling `nested_fixture_preparation`
  held the group for 86.578 s, and the lane took 49 minutes against 33.6 on the
  prior passing run. Two things had to be true at once: the file is untouched
  by this PR, so "we did not change it" was the wrong test for whether the
  failure belongs here, and the failure is the issue's own thesis at a site the
  plan had not audited. After the override, run 43612678941 passes that test in
  2.340 s with all 2321 green.
- **Group membership, not the longer period, is what fixed it.** Nextest does
  not count time spent waiting for a test group against `slow-timeout`, but it
  does count time spent blocked on a Cargo lock. Joining `cargo-spawning`
  converts the contention that killed the test into a queue that cannot; the
  180 s period is hang detection with headroom to spare, since the test
  completes in single-digit seconds once the graph is warm.
- **A property generator that writes files must not spell a Windows device.**
  Run 43617094223's Windows lane failed the new `prop_tests` suite with
  `input file: Os { code: 3, kind: NotFound }` for minimal input
  `tree = { "nul/a": [] }`. The segment alphabet is `[a-z][a-z0-9_]{0,6}`,
  which freely spells `nul`, `con`, `prn`, `aux` and the `com1`–`com9` and
  `lpt1`–`lpt9` families. Win32 resolves those to devices before NTFS is
  reached, so `nul/a` is not a rejected name but a device with no directory
  beneath it, and the failure surfaces from `create_dir_all`, not from a hash
  comparison. The filter now lives in an `ordinary_segment` strategy so both
  `input_path` and `excluded_path` inherit it: the excluded path's prefixes are
  written as directories too, so it had the same exposure and had not failed
  yet only because the two strategies draw from different sizes.
- **The regression test is a predicate test, not another property.** A
  property that happens to draw `nul` proves nothing on a host where `nul`
  works, and is the test that just failed. The new
  `windows_device_names_are_the_only_segments_excluded` test pins both halves
  from a fixed list: the eleven reserved names are excluded, and `nul_`, `null`,
  `com0`, `com10`, `lpt`, `lpt10` and `target` are not. Mutation-checked by
  making the predicate return `false`, which fails the test; the first rewrite
  of the predicate dropped the `len() == 1` check and the test caught that too,
  as `com10 is an ordinary name`.
- **A gate can fail for a reason the candidate cannot cause.** `make test`'s
  leg 3 (`-p rstest-bdd --no-default-features`) timed out
  `trybuild_macros runner::step_macros_compile` at 1200.004 s. The same test
  passed at 314.727 s in leg 1 of the same run, at 215.340 s on hosted run
  43612678941, and the log holds zero assertion failures — its nested
  `cargo build` was asleep in `futex_do_wait` while `build-limits-status`
  reported `capacity 4, held 4, healthy` with eighteen queued requests from
  other agents, and the managed sccache server had stopped serving. The change
  under test is a `#[cfg(test)]` unit module in `rstest-bdd-harness`, which
  that leg's fixture graph never builds, so this is queue starvation rather
  than a defect: report it, and do not weaken the budget to make it green on a
  run where nothing was measured. The 20 m period did later move to 30 m, but
  by the opposite route — an isolated run established the test's own cost
  (1119.268 s) before the budget was touched, and it moved because that cost
  was queue-dependent rather than because a red run needed a green one. See "A
  local-host measurement artefact" for the measurement.

## Outcomes & Retrospective

Implementation and evidence collection are complete. Two earlier CI runs on the
Linux coverage lane supply the timing figures: `41981885921` with an evicted
compiler cache, which is the cold-cache evidence acceptance criterion (a) asks
for, and `42005188689` with a warm cache.

Naming a "final" head in a living document is a trap this plan fell into twice.
`434d8309` was called final and then followed by `df26170d`; any commit that
corrects the claim supersedes it again. The durable distinction is between the
head whose source the timing evidence describes and the head CI last validated,
because the second moves with every documentation commit while the first does
not. The timing figures below are attributed to their runs rather than to "the
head" for that reason.

An earlier revision of this section claimed that every recorded run passed
every required lane, and named `3ffa7295` (run `42057352055`) as the latest.
Both claims went stale, and the first became false. Run `43617094223` on head
`34f29ce7` failed `build-test (windows, default features)`: 118 tests passed and
`nested_cargo::preparation::prop_tests::renaming_an_input_changes_the_hash`
panicked at `prop_tests.rs:150` with
`input file: Os { code: 3, kind: NotFound }` for the minimal input
`tree = { "nul/a": [] }`. That is the Windows device-name defect recorded under
progress, not a flake, and it was a regression this plan's own property test
introduced. Run `43630984118` on head `77355189` is green on all three required
lanes with the fix in place. A plan that reports its own failures as
all-passing is worse than one that names no head at all, so the history is
recorded here rather than replaced.

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
and run `42028526380` measures `434d8309`, the head at that point, after the
prose commits that followed it. Neither run is on the branch's final head: the
commits after them split `preparation.rs`, added the toolchain probe and
corrected the stamp round trip. Those changes touch no preparation timing the
runs measured — they add tests and move code between modules — but the
attribution above is by commit, not by "the head", for that reason.

The second run matters for a reason the first does not cover. Its Linux lane
took 36 m 08 s against the first's 32 m 09 s, which reads like a regression
until the two are decomposed. In all three runs the nextest summary is about a
sixth of the lane (17.5%, 17.3% and 16.6%), so roughly 30 minutes of each run
happens before and after it — image setup, the Cargo build, coverage handling.
That outside-the-suite time is where the 3.5-minute lane difference sits (26.6
minutes before, 30.1 now), while the suite grew 8.3% on code this diff does not
touch. A diff that touches only Markdown cannot account for either figure. The
lane duration is therefore not evidence about this change, and the suite
duration is; recording both is what keeps the conclusion checkable.
`make test-workflow-contracts` (364 tests), `nextest_config` (5) and
`rstest-bdd-harness` (51) also pass locally on the head those commits produced.

Every suite that compiles a nested Cargo graph is prepared and stamped,
including the harness and server trybuild trees. On the development host a cold
nested target prepared all six families in 2213.657 s, well inside their 30 m
budgets, and the timed assertions then ran against those trees:
`list_steps_runs` in 2.223 s against 180 s, `feature_addition_triggers_rebuild`
in 224.433 s against 600 s, and the CLI and trybuild helper cases in seconds.
No assertion deadline was widened.

**The same preparation on a contended host.**

The figures above are single-host measurements taken when the shared compile
pool was quiet. They do not describe what the preparation costs when other
agents are building on the same machine, and a red `make test` on this host has
to be read against that difference rather than against the isolated figures.

Measured on the development host while the pool was saturated, with
`build-limits-status` reporting every slot held, a healthy supervisor and a
queue in the hundreds: the trybuild `cargo test` binary's
`nested_fixture_preparation` prepared its first family in 1578.2 s, then had
the UI-lint family killed by the 9-minute `PREPARATION_COMMAND_TIMEOUT`. The
killed child was not wedged. Its captured standard error holds hundreds of real
`Compiling` lines — a genuine cold `cargo clippy` of the `ui_lints` fixture —
interleaved with admission waits, so the bound was spent on queue position, not
on anything this change introduced. The same bound is calibrated for a CI lane,
where the repository's own records put the whole two-graph trybuild preparation
at 645 s cold, and where no admission pool exists.

Two consequences follow, and both are host facts rather than candidate facts. A
red `make test` whose only failure is `nested_fixture_preparation` on a
saturated host is a queue artefact: every assertion in the suite passed, and
2261 of 2393 tests ran. And a stamp written by a bare `cargo test` does not
compose with `make test`, because the fingerprint covers `RUSTFLAGS` and the
gate sets it — so the remedy is `make prepare-nested-fixtures` followed by
`make nextest-run`, as recorded under Concrete steps, not a handwritten
invocation.

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
`TrybuildTree::prepare_now` and `TrybuildTree::ensure_prepared` both take an
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

Two later rounds of hardening are worth recording separately, because neither
was predicted by the plan and both came from a reviewer reading the code rather
than from a failing test.

The first is the *recorded-artefact* half of the gate. A fingerprint answers
"the inputs have not changed"; it cannot answer "the compiled units are still
there", because the profile directory holding them need not sit inside the
family's stamp root — `cargo clean`, or `rm -rf <target>/debug`, evicts the
profile and leaves the stamp untouched. The stamp therefore also records the
names the preparation actually left beside its dep-info records, and the gate
requires every one of them to still exist. Recording what was produced is the
only exact answer available: a rule about rustc's file naming cannot separate a
metadata-only unit, which Cargo reuses, from a linked unit whose `.rlib` was
removed, which Cargo rebuilds — measured against a real crate rather than
assumed, because `lib<stem>.rmeta` surviving alone is the same shape in both
cases. Neither layer names a directory: `layout::discover` finds where the
units are, because Cargo's build-directory layout changed under this suite once
already, in 1.100.0.

The second is the *fallible toolchain probe*. `cargo -vV` identifies the
compiler, and a probe that swallowed its error would silently fingerprint an
unknown toolchain — the state in which a stamp written by one compiler is read
by another. Resolution is now an `io::Result` propagated to the gate, and
`ensure_prepared_building_reported` takes the probe as a parameter — carried in
`Instruments` alongside the diagnostic reporter — for the same reason the
runner decision is one: no test can make `cargo -vV` fail, so a boundary that
discarded the error would be unreachable from any test using the real probe.
The probe is called exactly once, and its error stops the gate before any stamp
is read.

The same reasoning produced the fallible target-directory resolution behind
`absolute_target_directory`: a relative `CARGO_TARGET_DIR` names one directory
for this process and a different one for a nested build running in a fixture
directory, and returning `Option` there would record a relative artefact path
the gate then checks against the wrong root. It returns `io::Result<PathBuf>`
and takes its working-directory lookup as a parameter, so the absolute, the
relative and the failed-lookup branches are all reachable from a test.

## Context and orientation

The shared preparation policy is
`crates/rstest-bdd-harness/src/nested_cargo/preparation.rs`, with the stamp
protocol in `preparation/stamp.rs` and directory hashing in
`preparation/input_hash.rs`. `Preparation::new` takes a `Sources` grouping the
three values that identify a family (name, variant label, input hash), a
`Directories` grouping the stamped root and the working directory, and the
filtered `NestedCargoEnv` its commands run with; `prepare_now` runs a closure
unconditionally and writes the stamp on success; `ensure_prepared_building`
fails fast under nextest and prepares lazily otherwise. The build-capable entry
point's name states that it may build, and its `PreparationOutcome` return
value states whether one ran. `admit` is the read-only query beside it: it
performs no build, writes no stamp, takes no lock, and answers `Reusable`,
`NotPrepared` or `Unreadable`. Neither reads the process environment — the
runner decision arrives as a parameter, resolved once by the call site with
`running_under_nextest`. The injectable forms take an `Instruments` grouping
the toolchain probe and the diagnostic reporter, so a test can make `cargo -vV`
fail and read the gate's stderr without either one being reachable from
production's own binding. See
`docs/adr-025-nested-cargo-preparation-gate-query-boundary.md`.

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
`ensure_prepared_building`. Assertions and assertion budgets are unchanged. The
trybuild dep-info assertion is tightened to compare against the start of its
own run, because a prepared tree means the artefacts are no longer necessarily
fresh.

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
`ensure_prepared_building` deliberately prepares lazily there rather than
failing, so the run continues from a cold target instead of stopping.

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

The 20-minute `trybuild_macros` limit was originally left unchanged from
`origin/main`, on the reading that the per-invocation cost on an uncontended
2-vCPU lane is a fraction of the queued cost measured here. The lane that can
actually confirm criterion (c) is the CI coverage lane, and that is where the
remaining timing evidence belongs.

That reading held for two days and then stopped holding on this host. The same
prepared-tree test was timed in isolation, with nothing else of this
repository's running, and passed at `PASS [1119.268s]` — 93% of the old period,
inside it but with no margin. Two full-suite runs the same day crossed the
period outright at exactly 1200.002 s with the pool at `capacity 4, held 4`.
The difference between those outcomes and the 1200 s kills is not the work,
which was identical: it is how far the shared queue happened to have advanced
when the test's clock started. A period whose pass/fail depends on a
neighbour's queue position detects the queue, not a hang.

The limit is therefore 30 m, matching the preparation override, and the
decision is recorded rather than made quietly: a test that is compiling needs a
budget that admits compiling, and `terminate-after = 1` still ends anything
genuinely wedged one period later. This does not license raising budgets to
green a red run — the measurement above is what distinguishes the two, and a
future raise needs its own. The exact-period contract in
`crates/rstest-bdd/tests/nextest_config.rs` and the workflow contract under
`tests/workflow_contracts/` were updated together, so the policy cannot drift
from the file.

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
`PreparationOutcome`, `Admission`, `UnreadableStamp`, `Sources`, `Instruments`,
`admit`, `admit_with_probe`, `prepare_now`, `prepare_now_reported`,
`ensure_prepared_building`, `ensure_prepared_building_reported`, `input_hash`,
`PREPARATION_COMMAND_TIMEOUT`, `MAX_SEQUENTIAL_COMMANDS`, `ProfileDestination`,
`running_under_nextest` and `count_compiling`. `ensure_prepared_building` is
the only entry point that may build; `admit` cannot. The `*_reported` forms are
`#[doc(hidden)]` and take the injected boundaries. The stamp and hashing
modules stay private, so no caller can reconstruct the answer `admit` gives.
`TrybuildTree`, which the harness and server crates share, lives one level up in
`macrotest_support` rather than in `nested_cargo`; its `ensure_prepared`
resolves `running_under_nextest` once and passes it to the gate.
