# ExecPlan issue 910: keep cold nested Cargo builds out of timed test bodies

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

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
  combination as `step_macros_compile` does, rather than serialising
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
- [ ] Collect timing evidence on a cold target and a cold compiler cache.

## Surprises & Discoveries

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
- **`input_hash` already skips `target` directories by entry name,** so
  `crates/rstest-bdd/target/tests/ui_lints_clippy` is excluded from the
  `crates` input root even though it lives inside it. No special case was
  needed.
- **`nextest-version` is a top-level key,** not a profile key, and nextest
  ignores settings a file does not declare itself new enough to use. A
  `priority` in a file without it would silently be ordinary.

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

Not yet complete. Every suite that compiles a nested Cargo graph is prepared
and stamped, including the harness and server trybuild trees; the mutation-lane
decision, the docs table and the timing evidence remain.

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

## Context and orientation

The shared preparation policy is
`crates/rstest-bdd-harness/src/nested_cargo/preparation.rs`, with the stamp
protocol in `preparation/stamp.rs` and directory hashing in
`preparation/input_hash.rs`. `Preparation::new` takes a family name, a
directory, a filtered `NestedCargoEnv`, a variant label and an input hash;
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
nextest. Second, the preparation policy itself is mutatable source:
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

### Stage F: documentation and evidence (in progress)

The prose is written: `docs/testing-strategy.md` carries the "Cold fixture
builds belong to preparation, not to an assertion" section, and
`docs/developers-guide.md` describes the preparation override, the timeout
tiers, the `TrybuildTree` helper and the shared trybuild directory.

The evidence remains. It is collected from the Linux PR coverage lane on a
two-vCPU runner with a cold target: preparation elapsed times, `Compiling`
counts, timed-test durations against their limits, and the rebuild counters.
Notes say which figures are single-host observations rather than CI
measurements.

## Concrete steps

All commands run from the worktree root.

```bash
make check-fmt
make lint
make test
```

For a focused run after a target eviction:

```bash
make prepare-nested-fixtures
cargo nextest run -p rstest-bdd --test trybuild_macros
```

The second command should pass the timed assertions in seconds, because the
first already paid for the dependency graphs.

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
