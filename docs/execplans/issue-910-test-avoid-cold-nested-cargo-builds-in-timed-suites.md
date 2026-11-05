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
- **`--target-dir` blind spot.** `macrotest_support::trybuild_crate_root` runs
  `cargo metadata`, which sees `CARGO_TARGET_DIR` from the environment but
  cannot see an outer `--target-dir` flag. A lane that passes the flag
  directly, rather than through the environment, prepares a different tree than
  it builds in.
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
      bound is now 15 minutes — under the period even with the two sequential
      commands a preparation test runs — the misleading `nextest.toml` comment
      now says which path reports which overrun, and
      `preparation_command_bound_fires_before_the_runner_kills_the_test` pins
      the ordering so it cannot drift again. The other finding was stale
      status text in this plan, corrected at all four sites.

## Surprises & Discoveries

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
  target directory and the claim of preparedness goes with it.
- **Fail fast under nextest, prepare lazily under `cargo test`.** Under
  nextest each test is its own process, so a timed test that prepared would pay
  the cost it exists to avoid; failing with the preparation command named is
  more useful than a timeout. Under plain `cargo test` there is no scheduling
  guarantee that a preparation test runs first, so the gate prepares behind a
  process-wide lock instead.
- **The preparation override goes first in the file.** Nextest's first-match
  precedence means position, not priority, is what keeps the preparation from
  inheriting a timed assertion's budget.

## Outcomes & Retrospective

Implementation and evidence collection are complete. Two CI runs on the Linux
coverage lane supply the figures: `41981885921` with an evicted compiler cache,
which is the cold-cache evidence acceptance criterion (a) asks for, and
`42005188689` on the final head with a warm cache. Both passed every required
lane.

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
| `rstest-bdd::feature_rebuild_invalidation`                                   | `crates/rstest-bdd/tests/feature_rebuild_invalidation/harness/preparation.rs`      | each scratch fixture root                                                      |
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
tiers, the `TrybuildTree` helper and the shared trybuild directory.

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
`prepare_now`, `ensure_prepared`, `input_hash`, `PREPARATION_COMMAND_TIMEOUT`,
`running_under_nextest` and `count_compiling`. The stamp and hashing modules
stay private.
