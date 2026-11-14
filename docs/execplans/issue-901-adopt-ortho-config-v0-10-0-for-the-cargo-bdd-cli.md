# Adopt OrthoConfig v0.10.0 for the `cargo-bdd` command-line interface

This execution plan (ExecPlan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IMPLEMENTATION COMPLETE (the head `83a0e488` failed hosted run
`43596802177` on the required Windows strict lane, and the failing test was
this branch's own span test. That was a flake in a test this branch added, not
a regression in the diff: the same code passed the lane at `a634e98a`. It has
been diagnosed and fixed at the source rather than by loosening the assertion,
and the fix is measured — see `Progress` and `Surprises & discoveries`. The
fix's own hosted verdict is run `43604994289` at `0b798c1e` and run
`43608966901` at `17cf96e7`, which each passed with all three required
`build-test` jobs green, so the failing lane's verdict has been re-taken for
the fixed tree and for the pushed head rather than inferred from the earlier
run. A later plan-only commit, `413b46c3`, then failed the same lane on the
formatting gate because its own edit needed a rewrap; `2a017af9` applied that
rewrap and passed run `43617023742` with all three lanes green, so the
delivered head is that one. Earlier green runs, including `43592197882` at
`a634e98a` and `f10c0e4d` under `43582906292`, are historical evidence recorded
in `Progress`; merge pending)

## Purpose / big picture

`cargo-bdd` is the diagnostic front end for `rstest-bdd`: it reports registered
step definitions, unused steps, duplicate definitions, and skipped scenarios.
Today every knob it has is a command-line flag and nothing else. A project
cannot commit "in this repository, `cargo bdd skipped` always shows reasons" to
a file, cannot set it once through the environment for a CI lane, and cannot
run the tool from Cargo's dependency form at all — `cargo bdd steps` is
rejected.

After this change, an operator can put `reasons = true` under `[cmds.skipped]`
in a discovered `.cargo_bdd.toml` or XDG `cargo_bdd/config.toml`, get the same
effect from `CARGO_BDD_CMDS_SKIPPED_REASONS=true`, and still override either on
the command line with `--reasons`. `cargo bdd steps` and `cargo-bdd steps` both
work, and running the tool with no configuration file and no environment
variable produces byte-identical output, exit statuses and streams to today.

Success is observable without reading code:

```plaintext
CARGO_BDD_CMDS_STEPS_SKIPPED=true cargo-bdd steps --skipped --json
printf '[cmds.skipped]\nreasons = true\n' > .cargo_bdd.toml
cargo-bdd skipped                    # now lists reasons, exit 0
```

The configuration examples are new. Every existing test under
`crates/cargo-bdd/tests/` must keep its current assertions and pass unchanged.

## Constraints

Hard invariants. Violation requires escalation, not a workaround.

- **Library crates stay free of OrthoConfig.** `rstest-bdd` (runtime),
  `rstest-bdd-macros`, `rstest-bdd-patterns`, `rstest-bdd-policy`,
  `rstest-bdd-harness`, `rstest-bdd-harness-tokio`, `rstest-bdd-harness-gpui`,
  `rstest-bdd-test-macros`, and `rstest-bdd-server` must not gain an
  `ortho_config` dependency, direct or feature-gated. Only `crates/cargo-bdd`
  may depend on it.
- **No-configuration behaviour is frozen.** With no configuration file
  discoverable and no `CARGO_BDD_*` variable set, the flat invocation form
  (`cargo-bdd <subcommand>`) must keep the same flags, defaults, output, exit
  statuses and streams. Exact help and parser-failure output captured below
  pins the cases that could be exercised without building the nested fixture.
  Existing tests in `crates/cargo-bdd/tests/` must pass **unmodified** — not
  edited to accommodate the change.
- **Workspace MSRV split.** The workspace stays at `rust-version = "1.88"`.
  Only `crates/cargo-bdd/Cargo.toml` moves to `"1.89"`, and it does so with a
  literal value rather than `rust-version.workspace = true`. The raise is
  authorized by issue #901 because OrthoConfig v0.10.0's floor is Rust 1.89.0.
- **Default features on.** `ortho_config` v0.10.0 does not build with
  `default-features = false` (leynos/ortho-config#627). The dependency is taken
  with default features; do not add a `default-features = false` specifier.
- **Caret requirement.** The manifest entry follows repository convention:
  `ortho_config = "0.10.0"` in `[workspace.dependencies]`, referenced from the
  crate with `ortho_config.workspace = true`.
- **Libraries never print or exit.** Any `std::process::exit`, `print!`,
  `println!`, `eprint!` or `eprintln!` added by this work must be inside
  `crates/cargo-bdd`. Help and version display requests are handled at the
  application boundary only.
- **Imported guides are immutable.** `docs/ortho-config-users-guide.md` and
  `docs/ortho-config-v0-10-0-migration-guide.md` are byte-for-byte upstream
  copies recorded in `docs/ortho-config-v0-10-0-provenance.md`. Do not edit,
  reformat, or correct them.
- **File length.** No code file may exceed 400 lines. Check with
  `python3 scripts/check_rs_file_lengths.py` before committing.
- **No gate suppression.** Do not add `#[allow]`, `#[expect]`,
  `--ignore-rust-version`, `.gitignore` entries, or nextest overrides to make a
  gate pass. Do not silence warnings emitted during tests.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 25 files, or more than
  500 net lines of code outside tests, stop and escalate.
- Interface: if a public API of a library crate must change, stop and escalate.
  (No such change is anticipated; `cargo-bdd` is a binary-only crate.)
- Dependencies: if a production dependency other than `ortho_config` must be
  added, stop and escalate. The `cargo-bdd` test targets may use the existing
  workspace `camino` and `cap-std` dependencies for capability-safe isolated
  binary tests; they remain dev-dependencies and do not enter the production
  graph.
- MSRV: if a Rust 1.89 requirement leaks into a library crate's resolution,
  stop and escalate.
- Iterations: if the same gate fails three times after distinct fixes, stop and
  escalate with the captured logs.
- Ambiguity: if the `SelectedSubcommandMerge` reshape or the unit-variant
  obstacle admits more than one defensible resolution with materially different
  user-visible consequences, stop and present the options.

## Risks

- Risk: reshaping `Commands::{Unused, Duplicates}` to satisfy
  `#[derive(SelectedSubcommandMerge)]` changes the generated help text.
  Severity: high. Likelihood: medium. Mitigation: the derive emits its help from
  `clap::Command` metadata, which is already frozen by the baseline; assert
  byte equality against the baseline capture in a test before and after, and if
  the reshape moves help output, abandon the derive arm and hand-roll dispatch.
- Risk: `cli_default_as_absent` interacts with `--json`'s
  `requires = "skipped"` constraint and produces a different error message for
  `cargo-bdd steps --json`. Severity: high. Likelihood: medium. Mitigation: the
  baseline records that exact message; assert it verbatim.
- Risk: config file discovery picks up an unrelated `cargo_bdd.toml` from a
  developer's home directory or `/etc/xdg`, making tests non-hermetic and local
  runs behave differently from CI. Severity: high. Likelihood: medium.
  Mitigation: unit tests inject sources (`MapEnv`, `_at` loaders,
  `SubcommandFileContext`) and never rely on discovery; binary-level tests
  isolate `HOME`, `XDG_CONFIG_HOME`, `XDG_CONFIG_DIRS`, and `APPDATA`, and
  remove the four configurable `CARGO_BDD_CMDS_*` variables from the child
  process environment. `USERPROFILE` is deliberately *not* isolated: `HOME`
  alone closes the home dotfile because discovery reads it first, and removing
  `USERPROFILE` from the filtered child breaks the `sccache` compiler wrapper
  on Windows. See the Progress entry for 2026-11-14 and the Decision log.
- Risk: the Cargo-dispatch wrapper's `external_subcommand` help differs from
  the flat `cargo-bdd --help`. Severity: medium. Likelihood: high. Mitigation:
  select the helper only when argv begins with Cargo's injected `bdd` token;
  keep the existing parser for the flat form and pin both surfaces in tests.
- Risk: `make test` contention with other agents' builds causes a nextest
  timeout that looks like a defect. Severity: medium. Likelihood: high.
  Mitigation: check `build-limits-status` before diagnosing; see the baseline
  surprise entry.
- Risk: documentation drift already present (the users' guide advertises
  `cargo bdd unused --quiet` and `cargo bdd duplicates --json`, neither of
  which the parser accepts) is mistaken for a regression introduced here.
  Severity: low. Likelihood: high. Mitigation: record the drift in the baseline
  inventory and decide explicitly whether to correct or defer it.

## Progress

- [x] (2026-10-25) §1: import the two OrthoConfig v0.10.0 guides from the
      resolved tag commit; record provenance and SHA-256 checksums; gate and
      commit as `c2a8306f`.
- [x] (2026-11-01) Re-fetch `v0.10.0`, confirm tag object
      `996e5a1c9f22ace4f55bf5dcc196d2c0524262bf` peels to
      `38812c563e3953a490e3efc202609f37e21a0517`, and verify both imported
      guides match the pinned commit checksums. Firecrawl scraped the migration
      guide from that immutable commit to check the v0.10.0 capability summary.
- [x] (2026-11-01) Record the execution baseline compiler identity:
      `rustc 1.99.1 (58cf8f9c35b355199727627d07ae34a4e2846d67)`, host
      `x86_64-unknown-linux-gnu`, LLVM `23.1.1`. The current cargo-bdd sources,
      manifests and lockfile match baseline commit `2ee10a13`.
- [x] (2026-11-01) Keep the generated Tokio spelling patterns in the separate
      base commit `8c23a75e` on `chore/refresh-generated-typos-config`, based on
      `origin/main` `4d585b78`. `make check-fmt`, `make markdownlint`, and
      `make nixie` passed. CodeRabbit returned “All files are ignored” for the
      `typos.toml`-only change and produced no findings. Rebase the issue branch
      onto that commit; `git range-diff` confirms all three issue commits are
      content-equivalent to their pre-rebase versions.
- [x] (2026-11-01) §2: record baseline commit `2ee10a13`, compiler identity,
      command inventory, callers, and supported output formats. Capture exact
      help, version and parser-failure output in
      `/tmp/baseline-cli-errors-issue-901.out`; confirm `cargo-bdd bdd steps`
      is rejected with exit 2. A successful listing could not be captured:
      the nested fixture build failed in `convert_case`, so its stdout/stderr
      and exact byte counts remain unknown. Do not treat that probe as a pass.
- [x] (2026-10-25) §2: inventory callers (`Makefile`,
      `.github/workflows/mutation-testing.yml`, `.github/dependabot.yml`,
      `scripts/fixture_lockfile_cargo.py`, three test modules under
      `scripts/tests/` and `tests/workflow_contracts/`). No workflow invokes
      `cargo bdd`.
- [x] (2026-10-25) §1–§2 gates: `make spelling` and `make check-fmt` green.
- [x] §2: capture the baseline package test inventory and result. The
      `cargo nextest run -p cargo-bdd` command did not complete cleanly.
      `/tmp/baseline-cargo-bdd-issue-901.out` records 19/31 passing tests; the
      next test, `cargo-bdd::cli::list_steps_runs`, timed out at 180 s while
      building its nested Cargo fixture. At timeout, `build-limits-status`
      showed a healthy but saturated shared compiler pool and unrelated queued
      builds. This is consistent with compile-admission contention, but is not
      a pass and must not be cited as one. Re-run the full package suite after
      implementation validation; classify any repeat using the test child and
      admission logs.
- [x] §3: write this plan.
- [x] §3: write ADR 025 recording the configuration decision.
- [x] §3: add the configuration and integration-boundary sections to
      `docs/rstest-bdd-design.md`; add the ADR 025 entry and reference
      definition to `docs/contents.md`; add the decision to roadmap §6.1.
- [x] (2026-11-01) Review the recorded design with CodeRabbit after the
      documentation gates. The finalized design candidate at
      `7396a1a951a696078e6789636c3bcf3cc8b639d4` had zero findings; see
      `/tmp/coderabbit-e2f456a7-6816-4650-baec-7f12c426fa51-issue-901-adopt-ortho-config-v0-10-0-for-the-cargo-bdd-cli-7.out`.
- [x] (2026-11-01) Align the ADR and this plan with the two-parser invocation
      design in §3.6a: the existing parser handles flat argv, and
      `external_subcommand` handles argv with Cargo's injected `bdd` token.
      `make fmt`, `make check-fmt`, `make markdownlint`, and `make nixie` passed.
- [x] (2026-11-01) Address all three CodeRabbit findings: leave roadmap item
      6.1.6 unchecked during implementation, describe MSRV metadata per package,
      and specify platform-specific unreadable-file fixtures. The three docs
      gates passed; request a fresh review on the committed correction.
- [x] (2026-11-01) Resolve the current CodeRabbit plan findings: correct the
      V1–V5 obligation count, isolate the discovery test's working directory,
      and link to the design and ADR from this nested plan. All three docs
      gates passed; the final candidate review returned zero findings.
- [x] (2026-11-01) Correct the plan's baseline description: retain verified
      help/parser captures, and mark the listing probe and baseline package
      run as unsuccessful. `make fmt`, `make check-fmt`, `make markdownlint`,
      and `make nixie` passed; imported guide SHA-256 checksums remain exact.
- [x] (2026-11-01) Address the seven minor CodeRabbit findings on the design
      candidate: make dependency absence an explicit assertion, preserve
      upstream guide cross-links with checksum-identical copies, use the Cargo
      external-subcommand term, and document the post-merge `steps.json`
      constraint. `make fmt` passed and all four imported-file checksums match;
      the three documentation gates passed, then a fresh review found three
      additional link and Windows-discovery issues. Those are being resolved
      before implementation.
- [x] (2026-11-01) Resolve the three follow-up CodeRabbit findings: isolate
      `USERPROFILE` and `APPDATA`, index the link-preserving guide mirror, and
      restore all relative upstream link targets through pinned pointer pages.
      A direct link-path check passed, all three docs gates passed, and the
      requested imports and guide copies retain matching SHA-256 checksums; the
      candidate review returned zero findings.
- [x] (2026-11-02) Implement the configuration types and merge boundary,
      preserve the flat CLI parser, and add the Cargo external-subcommand path.
      The first focused run exposed an isolated-PATH test setup failure; the
      next exposed that wrapper help uses `cargo bdd`, as expected from the
      design. Both were corrected. A Camino-to-`std::path::Path` conversion
      was then added at OrthoConfig's explicit-source boundary. The final
      source-injected and binary focused suite passed 31/31, including the
      no-configuration baseline and Unix unreadable-file cases.
- [x] (2026-11-02) Resolve the first full-lint findings: make binary-test setup
      helpers return I/O errors, document validated settings, borrow the
      configuration during validation, and avoid test output when root bypasses
      Unix permission bits. Formatting passed; the full candidate gates must be
      rerun before this work is considered validated.
- [x] (2026-11-02) Diagnose `make test`'s `list_steps_runs` timeout. The same
      timeout is recorded at baseline; a direct trace showed the nested fixture
      build waiting for shared compiler admission. Prebuilding both fixture
      targets with the normal shared target completed in 59.8 s, after which
      the unchanged test passed in 0.368 s. The full suite retry remains pending.
- [x] (2026-11-02) Classify the next full-suite timeout in
      `feature_rebuild_invalidation`: its nested build hit the harness's 300 s
      bound while at least 62 unrelated compiler jobs were queued. The shared
      build-limits supervisor was healthy. The harness cleans only the fixture
      package and retains dependency artefacts, so prewarming its exact locked,
      offline fixture build in the shared workspace target is valid retry
      preparation. That build completed in 3m34s; the full-suite retry remains
      pending. No test timeout or library code change is warranted.
- [x] (2026-11-02) Record the latest `make test` result: 2,076 tests passed,
      seven were skipped, then `trybuild_macros::runner::step_macros_compile`
      hit its configured 20-minute nextest limit while nested Cargo was still
      compiling under healthy but saturated shared admission. The log reached
      checks for `rstest-bdd` and `rstest-bdd-test-macros` and showed many
      queued compiler requests. Prewarmed the generated trybuild workspace in
      the shared `target/` with a passing fixture; the locked offline check
      completed in 8m05s, but the full-suite retry still timed out at 20m.
      The nested check used `default`, `diagnostics`, `mutable_world_macro` and
      `test-support` and finished only near the deadline. The initial prewarm
      did not request those features; this test also runs three `cargo clippy`
      UI fixtures in its own prescribed target directory. Prewarmed those exact
      Clippy commands sequentially: 20m09s for the cold first target, then
      1m28s and 1m56s for the remaining fixtures. The full-feature trybuild
      check completed in 2m41s, but a third full-suite retry still timed out
      at 20m. Its nested Cargo was compiling `rstest-bdd-policy` and
      `rstest-bdd-harness`; the earlier prewarm used `cargo check`, which
      produced metadata rather than code-generation artefacts. Prewarm the
      exact passing trybuild bin with `cargo build` and the same feature set in
      the shared target, then rerun without changing the nextest limit. An
      initial `cargo build --bins` prewarm failed because the generated
      manifest also declares intentionally compile-failing UI fixtures; use
      `--bin trybuild000` for the passing fixture. That exact full-feature
      build completed in 1m03s after the broad attempt had populated the
      dependency artefacts. The full-suite retry remains pending. No
      implementation or library-test change is indicated.
- [x] (2026-11-02) Record the next `make test` result: 2,045 tests passed,
      seven were skipped, then `feature_addition_triggers_rebuild` hit the
      existing nested-Cargo 300-second bound during its baseline `cargo test`.
      The log showed the `feature_addition` fixture beginning to compile while
      many dependency compiler requests waited in the shared queue; the
      supervisor was healthy. The child uses the shared workspace `target/`
      and `--locked --offline`. The exact fixture command, with the gate's
      `RUSTFLAGS`, completed in 54.33s in the shared target; its baseline
      scenario and doc-test phase passed. Restart the complete sequential gate
      run. Do not alter the fixture timeout or test harness.
- [x] (2026-11-02) Record full test run #8: 2,076 tests passed and seven were
      skipped before `step_macros_compile` exceeded its existing 20-minute
      Nextest limit; 20 tests were not run. The captured log shows the
      generated trybuild workspace compiling, then processing its fixture
      checks while many requests waited in the healthy shared compiler queue.
      A trybuild work-in-progress snapshot was reported, but no repository
      files outside the candidate changed. Before another full run, exercise
      this one test directly with Cargo so the entire fixture sequence can
      finish without Nextest's per-test limit and warm the shared target. Do
      not change the timeout or test harness.
- [x] (2026-11-02) Run the prescribed focused test without Nextest's per-test
      timeout: `cargo test -p rstest-bdd --test trybuild_macros --all-features
      runner::step_macros_compile -- --exact`. It completed successfully with
      1 passed, 0 failed in 6,864.45 seconds; all generated fixtures passed.
      The output is `/tmp/issue-901-step-macros-focused.out`, and the worktree
      gained no generated snapshot changes. Retry the full sequential gates on
      this warmed candidate; do not alter the existing timeout or harness.
- [x] (2026-11-02) Record full gate run #9: `make check-fmt`, `make lint` and
      `make typecheck` passed. `make test` had 2,046 passes and seven skips
      before the existing `feature_edit_forces_rebuild_and_fresh_failure`
      nested `cargo test --locked --offline` exceeded its 300-second bound;
      50 tests did not run. The nested build waited under healthy shared
      compiler admission. No timeout or harness change is warranted. After
      restoring the fixture's tracked feature value and verifying its SHA-256,
      its exact baseline command passed two integration tests and doc tests in
      55.92 seconds. This scratch-only prewarm does not change the candidate.
- [x] (2026-11-02) Make the Linux unreadable-file integration test exercise a
      genuine permission failure even when the test runner is root: the helper
      now runs the child binary as UID 65534 with the mode-000 config file.
      The focused unreadable-file test passed on this host. Restart all
      sequential candidate gates after updating this coverage and the plan.
- [x] (2026-11-02) Record full gate run #10: formatting, lint and typecheck
      passed. `make test` had 2,045 passes and seven skips before the existing
      `feature_addition_triggers_rebuild` fixture exceeded its 300-second
      nested-Cargo limit; 51 tests did not run. The log records the fixture and
      its `addition` binary waiting 442–448 compile requests behind shared
      admission. The supervisor was healthy. Earlier preparation restored and
      verified the fixture's tracked baseline and passed its exact locked,
      offline test command, but the full suite ran under a much larger shared
      queue. This is an infrastructure timeout, not a test assertion failure;
      leave the harness limit unchanged and retry the full sequence.
- [x] (2026-11-02) Record full gate run #11: formatting, lint and typecheck
      passed. The same existing fixture test again exceeded its 300-second
      nested-Cargo limit after 2,045 passes and seven skips; 51 tests did not
      run. Its nested `cargo test --locked --offline` finished compiling in
      3m39s, then queued rustdoc 219 requests deep in the healthy shared
      admission pool. The timeout occurred at 305.229s without an assertion
      failure. Preserve the documented test limit and retry after this run has
      populated the nested fixture's shared target.
- [x] (2026-11-02) Record full gate run #12: formatting, lint and typecheck
      passed. `make test` again had 2,045 passes and seven skips before
      `feature_addition_triggers_rebuild` exceeded its 300-second nested-Cargo
      bound; 51 tests did not run. The captured log shows fixture compilation
      finishing in 3m36s, followed by the doc-test `rustdoc` request waiting in
      the healthy shared compiler queue until the child was cancelled. No
      assertion failed. Run the exact nested command outside Nextest's bounded
      child wrapper to finish populating the fixture target, then retry the
      full sequence without changing the timeout.
- [x] (2026-11-02) Record full gate run #13: formatting, lint and typecheck
      passed. `make test` again had 2,045 passes and seven skips before
      `feature_addition_triggers_rebuild` exceeded its 300-second child bound;
      51 tests did not run. The nested command took 4m22s to compile before
      doc-test rustdoc queued in the healthy shared pool. Running its exact
      baseline command directly then passed in under a minute; running the
      added-feature command directly passed both fixture scenarios and doc
      tests. Logs are `/tmp/issue-901-feature-addition-nested-prewarm.out` and
      `/tmp/issue-901-feature-addition-added-scenario-prewarm.out`. The
      harness limit remains unchanged; retry the full sequence with both
      fixture paths warmed.
- [x] (2026-11-02) Record full gate run #14: formatting, lint and typecheck
      passed. `make test` again had 2,045 passes and seven skips before
      `feature_addition_triggers_rebuild` hit its 300-second child bound; 51
      tests did not run. The nested baseline command queued up to 288 compiler
      requests behind the healthy shared admission pool and was cancelled at
      300.471s without an assertion failure. The earlier direct runs proved
      both fixture commands can pass when they complete outside the wrapper.
      Do not change the unrelated fixture or its timeout; continue with the
      remaining independent checks and report this full-suite blocker.
- [x] (2026-11-02) Keep `ortho_config` out of reusable library dependency
      graphs while making their Rust 1.88 claim executable. The first direct
      Rust 1.88 check found the base lockfile's `cargo-platform 0.3.3`, which
      requires Rust 1.91 and is selected through `cargo_metadata 0.23.1` in
      library packages. Firecrawl's crates.io metadata confirmed
      `cargo-platform 0.3.2` supports Rust 1.88, and cargo_metadata's
      `^0.3.0` requirement permits it. Downgrade only the lockfile entry to
      0.3.2. The retry, `cargo +1.88.0 check --workspace --exclude
      cargo-bdd --locked`, passed after 52m 04s; log:
      `/tmp/issue-901-cargo-1.88-libraries-check.out`. See the decision and
      evidence in ADR 024 and `docs/rstest-bdd-design.md`.
- [x] (2026-11-02) Verify the other side of the MSRV split with
      `cargo +1.89.0 check -p cargo-bdd --locked` (passed in 50m 45s; log:
      `/tmp/issue-901-cargo-1.89-cargo-bdd-check.out`). `cargo metadata`
      reports Rust 1.89 for `cargo-bdd` and 1.88 for all nine `rstest-bdd*`
      workspace packages. Separate `cargo tree --locked -p <package> -e
      normal` outputs under `/tmp/issue-901-*-dependency-tree.out` confirm
      `ortho_config` is reachable from `cargo-bdd` and absent from each of
      those libraries.
- [x] (2026-11-02) Record the current candidate gates: `make check-fmt`,
      `make lint`, and `make typecheck` passed on the unchanged implementation
      candidate. `make spelling`, `make markdownlint`, `make nixie`, and
      `python3 scripts/check_rs_file_lengths.py` also passed. The full
      `make test` run (#19) executed 2,046/2,097 tests: 2,045 passed, seven
      were skipped, and `feature_addition_triggers_rebuild` failed when its
      nested Cargo command exceeded the existing 300-second bound while
      waiting for shared compiler admission (250 requests ahead); 51 tests
      were not run. A focused rerun of that exact test reproduced the timeout
      at 301.009 seconds with 81 compiler requests ahead. The supervisor was
      healthy in both runs. Logs: `/tmp/test-e2f456a7-6816-4650-baec-7f12c426fa51-issue-901-adopt-ortho-config-v0-10-0-for-the-cargo-bdd-cli-19.out`
      and `/tmp/test-feature-addition-e2f456a7-6816-4650-baec-7f12c426fa51-issue-901-adopt-ortho-config-v0-10-0-for-the-cargo-bdd-cli-2.out`.
      The user directed that the timeout and cache-policy problems be tracked
      separately; do not change library test infrastructure in this branch or
      commit while the required full-suite gate remains red.
- [x] (2026-11-03) Follow the user's direction to track test timeouts and
      prohibit cold nested-Cargo dependency builds or network package retrieval
      in tests. Opened #910 for eliminating cold fixture builds from timed
      tests and #911 for an offline-only contract for test-spawned Cargo
      commands. These are separate test-infrastructure follow-ups; this issue
      branch remains scoped to `cargo-bdd` configuration. The prior full-suite
      timeout is recorded above. Do not change library test infrastructure in
      this branch; report the full-suite test gate as incomplete under the
      user's no-cold-child-build and no-network constraint.
- [x] (2026-11-03) Remove the new binary tests that built a scratch Cargo
      project just to reach the successful CLI handler. The CLI unit test now
      checks that both argv shapes select `steps`, while the binary test invokes
      both real entry forms with `--help`. Configuration files still exercise
      actionable binary errors, and source-injected tests cover valid merged
      settings. This keeps new tests from compiling a cold fixture or fetching
      packages; the existing nested-Cargo test timeouts remain tracked by
      #910 and #911. Revert the invocation test's temporary Nextest override.
- [x] (2026-11-03) In the first final gate cycle after the plan update,
      `make check-fmt` passed. `make lint`'s Clippy and `cargo doc` stages
      completed without diagnostics, but the target exited 143 while
      `lint-whitaker` was building test dependencies. `build-limits-status`
      returned healthy; the captured log has only queue waits and a terminal
      `Terminated`, not a lint finding. Typecheck and tests were not started.
      Treat this run as incomplete and validate the revised candidate from the
      beginning.
- [x] (2026-11-03) Final deterministic gates passed: `make check-fmt`,
      `make lint` (including Clippy, workspace docs, Whitaker, and Python
      checks), `make typecheck` (workspace all-target/all-feature Cargo check
      and Python `ty check`), Markdown lint, spelling, Nixie, file-length, and
      `git diff --check`. The fresh full `make test` invocation was stopped
      during its initial cold `cargo build` before starting any test binary;
      it did not use `--offline`, and no package fetch was observed. Do not
      report the full workspace suite as passed.
- [x] (2026-11-03) Prewarm and run only the affected `cargo-bdd` unit tests,
      `baseline_output`, and `invocation` targets under `CARGO_NET_OFFLINE=true`
      and `--offline --locked`. The build-only prewarm produced all three test
      executables. Test execution then completed without `Compiling`,
      `Checking`, or `Downloading`: 24 unit tests, one baseline-output test,
      and five invocation tests passed. `strace` showed no compiler process,
      network socket activity, or nested `cargo test` child. Excluded the
      existing `tests/cli.rs` target because it starts Cargo fixture builds;
      track the cold-build and offline-policy work in #910 and #911.
- [x] (2026-11-03) The staged implementation review found two issues: the
      isolated invocation test dropped `.exe` from the copied binary name on
      Windows, and shared config-test helpers used `.expect(...)`. Preserve
      `binary_name` for the copy destination and use explicit failure handling
      in helpers. Clippy then caught a `manual_assert` in the write helper;
      replace it with `assert!`. The corrected candidate passes `make lint`,
      `make typecheck`, and the offline focused test run above. Repeat CodeRabbit
      on the staged corrected diff before committing.
- [x] (2026-11-03) The next staged review requested `rstest` fixtures for the
      repeated `ConfigFixture` and `IsolatedWorkspace` setup, and found that the
      dependency tolerance did not allow the already-declared test-only
      `camino` and `cap-std` dependencies. Add local fixtures, record that these
      dev-dependencies are authorized and absent from the production graph,
      and document the helpers' module-local reuse boundary. Fixture expansion
      also requires the repository's `allow_fixture_expansion_lints` test macro.
      The corrected candidate passes formatting, lint, typecheck, the 30
      prewarmed offline CLI tests, and documentation gates. Repeat CodeRabbit
      on the staged final diff before committing.
- [x] (2026-11-03) Draft PR #912's first CodeScene run found duplicate
      structure in the two subcommand precedence tests and the platform-specific
      unreadable-file tests. Parameterize the precedence test by subcommand and
      combine the platform branches into one test. The first lint rerun caught
      E0382 because the JSON environment key was moved before reuse; borrow it
      and restart the gates. The corrected candidate passes `make check-fmt`,
      `make lint`, `make typecheck`, Markdown lint, spelling, Nixie, Rust file
      length, and `git diff --check`. The offline, locked prewarm built the
      three focused test executables, then all 30 tests passed. Test execution
      had no compile, check, download, compiler-process, or network activity.
      Local `cs delta` reports both clone findings fixed. The full workspace
      test suite remains incomplete under the no-cold-build/no-network policy.
- [x] (2026-11-03) Resolve the follow-up CodeRabbit findings: the three
      invocation forms now reach the `steps --skipped --json` handler in a
      dependency-free package with no test targets, and precedence is tested
      independently for all four configurable fields. The child environment
      sets `CARGO_NET_OFFLINE=true`, so the handler's Cargo metadata query
      cannot retrieve packages and has no test targets to build. Clippy then
      flagged an unnested pattern in the table helper; after nesting it, the
      fresh gate sequence passed: `make check-fmt`, `make lint`,
      `make typecheck`, offline/locked prewarm and all 33 focused tests, Markdown
      lint, spelling, Nixie, file length, `git diff --check`, and `cs delta`.
      A traced handler test showed only Cargo metadata and rustc `--print`
      queries, no source compilation, no nested `cargo test`, and no external
      network connection. The full workspace suite remains incomplete under
      the no-cold-build/no-network policy.
- [x] §4: implement. Manifest and MSRV split; `OrthoConfig` derives; the
      Cargo external-subcommand wrapper; boundary error handling.
- [x] (2026-11-03) Mark roadmap item 6.1.6 complete after implementing the
      configuration boundary, both CLI entry forms, and the documented MSRV
      split. Validation remains subject to the final gate cycle below.
- [x] (2026-11-03) The final CodeRabbit review found that Clap's `requires`
      rejected `steps --json` before TOML or environment `skipped = true` could
      satisfy the rule. Resolved by removing that early constraint and merging
      configuration before `validate_steps`; the application replays Clap's
      original exit-2 parse error only when explicit `--json` still has no
      effective `skipped = true`. Injected-source and isolated child-process
      coverage now exercises file, environment, direct, and Cargo forms, and
      the users' guide and ADR describe the merged-settings validation. The
      complete deterministic gate
      sequence is pending for the latest assertion-helper edits. The first
      `make check-fmt` passed; offline `make lint` then found missing Rustdoc,
      one missing code mark in a comment, and a duplicate test match arm. Fix
      those before restarting the full sequence. The next lint run caught the
      required nested spelling of that test's or-pattern; the rerun stopped at
      lint, so typecheck, tests and documentation gates remain pending. The
      following lint run passed Clippy/rustdoc but Whitaker found `cli` at 407
      lines and conditional complexity in `run`. Move parsing and compatibility
      errors into private `cli::invocation`, and extract merged step-settings
      resolution before restarting all gates. That full retry passed check-fmt,
      lint, typecheck, offline/locked prewarm and all 37 focused tests, docs
      gates, file length and diff check. `cs delta` then found a large assertion
      block and duplicated assertions around command output. Extract the
      repeated output checks into test-local assertion helpers and rerun gates.
      The next CodeRabbit pass found that the ADR and design document still
      described this as Clap validation after merging. Clarify that
      `validate_steps` enforces merged settings and Clap is replayed only for
      the legacy explicit-flag parse error. Docs-only gates passed, then the
      complete staged CodeRabbit review finished with zero findings.
- [x] §5: tests. Precedence tiers per subcommand with injected sources;
      unchanged-behaviour proof; unhappy paths; both invocation forms. The
      latest full focused run passed 28 unit/config, one baseline-output and
      eight invocation tests offline from prewarmed binaries. Formatting,
      offline lint/typecheck, documentation gates, file length, diff check and
      CodeScene all passed. On the current candidate, `CARGO_NET_OFFLINE=true
      make test` ran 2,052 tests successfully before
      `feature_addition_triggers_rebuild` failed at 555.787 seconds because its
      nested offline Cargo build exceeded its 300-second bound while compiling
      the fixture and rustdoc. Seven tests were skipped and 51 were not run.
      The unchanged `cargo-bdd::cli` and invocation suites passed, including
      `list_steps_runs` in 2.800 seconds; no package downloads occurred. Keep
      this explicit validation exception tracked under #910 and child-Cargo
      network isolation under #911. Do not claim the workspace suite passed.
- [x] (2026-11-13) Hosted CI run `43453087168` validated `76f8f63e`, the
      current head of pull request #912, and finished `success`. All three
      required `build-test` jobs passed: Linux in 30m33s and both Windows
      lanes in 43m15s and 40m48s. Both coverage ratchets passed with the
      `+/-1.00` percentage-point tolerance — Linux 91.17% against a 91.93%
      baseline, Windows 89.27% against 90.13%, and the Python lanes 85.02%
      against 84.97% and 81.03% against 81.23% — so the earlier Windows
      shortfall of 0.01 pp is cleared with 0.86 pp of headroom. CodeScene now
      passes as well: it had failed the previous head on an advisory
      `Code Duplication` finding in `crates/cargo-bdd/src/cli/tests.rs`, and
      two tests that repeated their fixture setup now share `accounts_step`
      and `render_listing`. The recorded `cs delta origin/main` comparison
      reported the finding on the parent commit and nothing with the fix.
      Gecko Security passed. CodeRabbit, Sourcery and automerge were skipped.
- [ ] §6: documentation, gates, follow-up review, commit, push, hosted CI,
      review reconciliation, and merge. Merge is still pending: this box stays
      unchecked until the pull request lands on `main`. The head `83a0e488`
      failed hosted run `43596802177` on the required Windows strict lane, in
      this branch's own span test; that flake is diagnosed and fixed at its
      source (see `Progress`), and the fix passed hosted run `43604994289` at
      `0b798c1e` with all three `build-test` jobs green; the pushed head
      `17cf96e7` then passed run `43608966901`, again with all three. The
      failing lane's verdict is therefore re-taken for the fixed tree and for
      the pushed head, not carried over from an earlier one. Those runs
      supersede CI run `43592197882` at `a634e98a` as the latest green
      verdict. The commit `413b46c3` that recorded those verdicts then failed
      the same lane on `make check-fmt`, because its own prose needed a
      rewrap; `2a017af9` applied it and passed run `43617023742`, whose three
      lanes are the delivered head's verdict. The `make test`
      row is met locally at `f7fa8cd6`, and the commits after it edit only this
      plan, which no Rust gate reads, so that row's input has not moved. The
      Markdown gates do read this file, and were re-run at the head.
      The candidate was rebased onto `origin/main`
      (`693bba91`) and gated at revision `96e38df8`; the rebase described in
      `Outcomes & retrospective` is the later one.
      `make check-fmt`, `make lint`, `make typecheck`, `make markdownlint`,
      `make nixie` and `make test-workflow-contracts` (373 tests) all exited
      zero there, as did the workspace doc tests, `make check-fixture-lockfiles`
      and `scripts/tests` (290 tests). The full suite passed every test that
      reached a verdict there. Those verdicts are all historical now. The
      earlier delivery history follows. Its first hosted
      run #41673314102 failed before tests started on Windows because the
      Unix-only `ConfigFixture::local_directory` helper triggered `dead_code`
      under `-D warnings`; the helper now has a `#[cfg(unix)]` guard. Linux also
      found three stale fixture locks resolving `cargo-platform` 0.3.3 instead
      of the workspace's locked 0.3.2. The offline fixture-lock refresh
      0.3.2 and the offline `make check-fixture-lockfiles` passed with five
      fixture lockfiles up to date. Unrelated published-GPUI lock churn was
      removed. No timeout occurred in that hosted run. Issues #910 and #911
      track cold nested compilation and network-enabled child Cargo commands.
      CodeRabbit then found the baseline version output pinned to 0.6.0. A
      red/green check confirmed the finding and the test now expands a
      `<CARGO_PKG_VERSION>` marker using `CARGO_PKG_VERSION`; its target was
      prewarmed separately with `--no-run`. The final candidate passed
      `make check-fmt`, offline `make lint` and `make typecheck`, offline/locked
      prewarm and all 37 focused tests, Markdown and Mermaid checks, file length,
      diff check and CodeScene. CodeRabbit reviewed 37 files with zero findings.
      Head `63b01c4d` passed hosted run 41891162299, and later commits produced
      `9a1f3265`, which this rebase supersedes.
- [x] (2026-11-08) The round-10 review found the opt-in observability path
      incomplete. The subscriber had no span-event configuration, so the
      `cargo_bdd.configuration` spans rendered nothing and the documented
      timing was unobservable. `install()` now enables `FmtSpan::CLOSE`, which
      emits one timed close line per load at the span's `INFO` level. New
      child-process cases in `tests/invocation/observability.rs` drive both
      command labels and pin the timing fields and the absence of paths and
      values. `CARGO_BDD_LOG` is documented in the users' guide and the
      developers' guide, and the stale "installs no subscriber" claims in
      `src/config.rs` and this plan are corrected. The remaining configuration
      fields (`steps.skipped`, `steps.json`, `skipped.reasons`) now have
      application-level precedence cases, and both reasons-output paths now
      assert the absence of an application diagnostic (an empty-stderr
      assertion is host-dependent there, because the fixture build's compiler
      wrapper writes its own progress lines). The head that carried the earlier
      green hosted run is `0c13d4a7`; this round's changes were gated on top of
      it, in the working tree, with every gate run once and sequentially. All
      of the following exited zero: `make check-fmt`, `make lint` (all nine
      steps), `make typecheck`, `make markdownlint`, `make nixie`,
      `make spelling`, the whole `cargo-bdd` suite (79 tests),
      `cargo nextest run --workspace --all-targets --all-features` (2239
      passed, 8 skipped), `cargo test --doc --workspace --all-features`,
      `cargo nextest run -p rstest-bdd --all-targets --no-default-features`
      (783 passed, 8 skipped), `make check-fixture-lockfiles` (5 lockfiles up
      to date) and `uv run pytest scripts/tests` (290 passed). The two nextest
      legs exclude only `step_macros_compile`, the retained validation
      exception described in Outcomes & retrospective.
- [x] (2026-11-08) Reconciliation: `crates/cargo-bdd/tests/cli.rs` is not
      byte-identical to `origin/main`. Its existing tests are unchanged, but
      the file gained the environment-tier `reasons` test and its shared
      helpers gained an extra-environment parameter. That addition came from
      the earlier review round, so the "unmodified file" wording in
      `Validation and acceptance` is narrowed to the existing tests.

- [x] (2026-11-08) Hosted CI on `8d45fae5` is green on both Windows lanes, on
      CodeScene and on Gecko Security, and red on the Linux lane for a reason
      outside this diff. The `Lint` step dies in the Dylint driver probe with
      `duplicate specification of lint string_continuation_style`: installer
      `0.2.11` prunes only the library names compiled into it, the rolling
      suite has since gained `string_continuation_style`, and `whitaker --all`
      therefore loads that standalone library beside the aggregated suite.
      The same failure hit an unrelated branch in this repository on the same
      day, so no commit under review is involved. `make lint-whitaker` now
      names the suite it installs — `whitaker --lib whitaker_suite` — which
      loads the same lint set without depending on installer bookkeeping.
      `scripts/tests/test_whitaker_lint_gate.py` reads the recipe and asserts
      the named suite and the absence of `--all`; with the old recipe restored
      that test fails, so it discriminates rather than restating the Makefile.
      The change is recorded in an ADR-013 addendum and in the developers'
      guide. Upstream tracks the pruning defect as `leynos/whitaker#565`.

- [x] (2026-11-08) Review round on `74f066af` closed four documentation and
      test-scope gaps and one design gap. `crates/cargo-bdd/src/config.rs` now
      takes its discovery roots and environment from a `ConfigSources` value:
      the application edge builds process-backed sources in `cli::run`, and
      the unit tests build isolated ones, so the tests drive the production
      resolvers instead of repeating the `ortho_config` call beside them. A
      subprocess test now covers the merged cross-field rejection — a
      configured `steps.json` with no effective `steps.skipped`, from both a
      file and the environment — asserting the status, the empty stdout and
      the exact diagnostic. Three doc comments were moved above their
      attributes to match the repository convention, design §3.6a now
      describes the settings the handlers receive and forward, and ADR-013's
      status section marks the 2026-07-20 invocation as historical.

- [x] (2026-11-08) The required Linux check on `74f066af` failed in the `Lint`
      step, not in this branch's code: naming the aggregated suite now selects
      the rolling suite's `string_continuation_style` lint, and 162 string
      literals across 81 files used the backslash continuation it rejects. The
      fix is adoption, not suppression. The continued literals now either use
      `concat!` with one argument per line or stay on a single source line, the
      latter where a macro attribute accepts only a literal or an implicitly
      captured format argument would not resolve through `concat!`.
      `.rustfmt.toml` no longer sets `format_strings`, because that option
      rewrapped long literals with the same continuation, including inside
      `concat!` arguments, so the formatter manufactured what the lint
      rejects. `make lint-whitaker` now exits 0 locally. Recorded in a second
      ADR-013 addendum and in the developers' guide.

- [x] (2026-11-12) Rebased onto `origin/main` at `009012b2`, which carries
      #938's repin of `install-whitaker` to the `0.2.12` installer. That
      installer prunes by staged file name, so the duplicate-specification
      failure recorded above no longer occurs in CI. The rebase took main's
      workflow files without conflict, because the branch touches none of
      them. The named-suite invocation is kept, and ADR-013's addendum now
      records why it is a design choice rather than only a workaround.

- [x] (2026-11-13) Gates were run in order on the rebased tree `3e94e1b6`:
      `make check-fmt`, `make lint` and `make typecheck` exited zero, and
      `make test` lost one nested-Cargo test to its own wall-clock bound in
      each of two runs. Admission reported capacity four with more than 200
      compiler requests queued from other repositories throughout, so the
      remaining recipe steps were run directly: the doc tests, the fixture
      lockfiles and `scripts/tests` (291 passed) all exited zero. The
      no-default-features leg was re-run with `--no-fail-fast` to give all 784
      tests a verdict; its two failures are the same class of nested-Cargo
      bound, and both tests pass in the all-features run at this revision. The
      exception that remains is recorded in `Outcomes & retrospective`.

- [x] (2026-11-13) Rust 1.100.0 reached the hosted `stable` channel, and its
      Clippy reports `used_underscore_binding` for a write-only use of an
      underscore-prefixed binding, which the 1.99.1 toolchain does not. The
      Linux lane failed this branch's required lint step on
      `crates/rstest-bdd/tests/underscore_fixture.rs:20` and `:23`, a file this
      branch does not otherwise change. The two steps now read their `_world`
      binding before mutating it, so the file's existing `#[expect]` idiom is
      fulfilled on both toolchains, and each step also asserts the value the
      previous step left behind — evidence that both steps share one fixture
      instance. Verified with
      `cargo +1.100.0 clippy --workspace --all-targets --all-features -- -D warnings`
      and with the local 1.99.1 toolchain, and `make check-fmt` passes.

- [x] (2026-11-13) Cargo 1.100.0's build-dir layout broke both Windows
      coverage lanes at `e8fbd8ab`. Test executables moved from
      `<target>/<profile>/deps` into
      `<target>/<profile>/build/<package>/<unit>/out`, so the helper that read
      the root as "two levels above the executable" returned `None` and
      `staging::tests::target_directory_uses_running_test_executable` panicked
      with "integration tests should run from Cargo's `deps` directory" in
      jobs 131673923291 and 131673923282. On the original base the repair
      recognized the build-dir shape explicitly, kept `None` for an
      unrecognized layout so `cargo metadata` stayed the fallback, and pinned
      all of it with `rstest` cases plus a rejection case.
      Reconciled during the rebase onto the #910 branch: that branch resolves
      the root by walking ancestors for Cargo's `CACHEDIR.TAG`
      (`rstest_bdd_harness::binary_test_support::target_directory_of`), which
      is layout-agnostic and so already covered the build-dir shape. Its
      `staging/target_root.rs` and `staging/target_root/tests.rs` carry the
      four-layout table and the untagged-tree rejection case, so the
      branch-local helper and its `staging/target_directory.rs` module were
      dropped rather than duplicated.

- [x] (2026-11-13) Teaching the helper the build-dir shape pushed
      `crates/rstest-bdd/tests/trybuild_macros/staging.rs` past Whitaker's
      400-line module limit, so `make lint` failed at `lint-whitaker` with
      "Module staging spans 419 lines, exceeding the allowed 400". On the
      original base the target-directory concern moved out to
      `crates/rstest-bdd/tests/trybuild_macros/staging/target_directory.rs`,
      leaving `staging.rs` at 356 lines. The #910 branch had already made the
      same split into `staging/target_root.rs`, so the rebase keeps that one:
      `trybuild_target_directory` lives there and stays crate-visible because
      `runner.rs` and `tracking.rs` call it, and `staging.rs` is 354 lines.

- [x] (2026-11-13) Hosted run `43386083081` validated `e28a927a` and all three
      `build-test` jobs passed. The Linux coverage lane ran the full workspace
      suite: 2,245 tests run, 2,245 passed, 7 skipped, including the two
      nested-Cargo tests this host could not finish
      (`runner::step_macros_compile` in 631.5 s and
      `whitaker::whitaker_lint_gate_accepts_clean_and_rejects_panicking_fixtures`
      in 0.9 s). Both Windows lanes passed with the build-dir fix, and both
      `staging::tests::target_directory_uses_running_test_executable` and
      `staging::tests::unrecognized_executable_layout_has_no_target_root`
      report `ok` under `cargo test` and `PASS` under nextest in the job logs.
      The hosted result covers both nested-Cargo tests; it does not cover the
      recipe's `--no-default-features` leg, and the single-invocation form of
      `make test` was still without a local green verdict.

- [x] (2026-11-13) Ran the `--no-default-features` leg that the hosted lanes do
      not run, at `cf7f7945`, using the filtered command recorded in
      `Outcomes & retrospective` and the recipe's `RUSTFLAGS`. It exited zero
      after 754.3 s, with 786 tests passed and 8 skipped.
      `whitaker::whitaker_lint_gate_accepts_clean_and_rejects_panicking_fixtures`
      passed in 266.1 s, which the earlier no-default-features runs had timed
      out. `runner::step_macros_compile` is excluded: an unfiltered attempt at
      the same head was stopped by its invoking command's own time limit after
      1,136 s of compiling, short of its 1,200-second bound, and no local run
      here has ever finished it. `Outcomes & retrospective` and the acceptance
      section now record the exception as one requirement and one test rather
      than as discharged.

- [x] (2026-11-13) The exception above is now cleared. The single-invocation
      `make test` form was run whole at `8a5cb67f`, unfiltered, with no
      `-E` and no test excluded, and every step of the recipe finished. It
      exited zero. The workspace leg reported 2,251 tests run, 2,251 passed,
      7 skipped, and the no-default-features leg 787 passed, 7 skipped.
      `rstest-bdd::trybuild_macros::runner::step_macros_compile` passed in both
      (275.0 s and 279.5 s), so the test that no earlier local run had finished
      now has a local verdict.
      `whitaker::whitaker_lint_gate_accepts_clean_and_rejects_panicking_fixtures`
      passed too, and `cargo test --doc --workspace --all-features`,
      `make check-fixture-lockfiles` (5 lockfiles up to date) and
      `uv run pytest scripts/tests` (291 passed) all completed behind them.
      No `FAILED` line, no `^error`, and no `make: ***` appears in the log.
      The earlier filtered no-default-features result is retained in
      `Outcomes & retrospective` as history, not as the current verdict. This
      also retires the `#910` reference as a blocker for the row: the budgets
      it owns still make the nested-Cargo tests slow here, but they no longer
      prevent the recipe from finishing.

- [x] (2026-11-13) A `coderabbit review --agent` pass over the branch reported
      five findings. Three were real and are fixed; one was not a defect; one
      was a documentation overstatement corrected in this plan. Each verdict
      was established by reading the cited code and, for the two test-scope
      findings, by a mutation that the old assertion or the old helper failed
      to reject. First, `crates/cargo-bdd/src/cli/tests.rs` asserted
      `!rendered.contains("executed '")`, but `write_step` renders
      `{keyword} '{pattern}' [...]`, so that substring cannot appear in a
      rendered line at all and the assertion could never fail. It now anchors
      on `'executed' [accounts]`. Restoring the filter mutation
      (`.filter(|_step| true)`) fails the new assertion and, measured, passed
      the old one, so the fix discriminates rather than restating the code.
      Second, `crates/cargo-bdd/tests/cli/configuration.rs` redirected only
      `XDG_CONFIG_HOME` on Unix. `ortho_config` 0.10.0's Unix discovery
      (`src/subcommand/paths.rs`, `collect_unix_paths`) pushes
      `$HOME/.cargo_bdd.toml` and `$HOME/.config/cargo_bdd/` before the XDG
      bases, and `load_from_files` merges every existing candidate, so the
      real `HOME` stayed reachable. `HOME` is now redirected on both platforms,
      with `CARGO_HOME` and `RUSTUP_HOME` carried so the nested registry and
      toolchain still resolve. With a malformed `~/.cargo_bdd.toml` planted in
      the parent's `HOME` the four cases pass on the new helper and all four
      failed on the old one. Third,
      `crates/cargo-bdd/tests/invocation/workspace.rs` cleared the child
      environment without passing `LLVM_PROFILE_FILE` through. The copied
      binary is the process under test, so its exercised lines were dropped
      from the coverage run silently, exactly as the `Caller` profile
      destination exists to prevent elsewhere. It is now inherited when set.

- [x] (2026-11-13) One of the five findings was declined. It asked for
      `cargo-bdd.exe` in `expected_flat_stderr` on Windows. That message is
      not produced from the real argv: `missing_required_skipped_error`
      re-parses a fixed `["cargo-bdd", "steps", "--json"]` vector, and Clap
      renders the echoed argv[0], which is the literal `cargo-bdd` on every
      platform. Measured locally with argv[0] set to `cargo-bdd`,
      `/tmp/…/cargo-bdd.exe` and a Windows-style path: all three render
      `Usage: cargo-bdd steps --skipped --json`. Adding the `.exe` suffix
      would *break* the byte-for-byte assertion on Windows, so the finding is
      declined. The fifth finding was documentation: the two acceptance
      passages that said the `make test` row was unmet and its exception
      retained are now marked as superseded by the `8a5cb67f` record above.

- [x] (2026-11-13) The review fixes were gated at `e153116b`, after
      `mdtablefix` reflowed the passages above. `make check-fmt`,
      `make markdownlint` (spelling included), `make nixie`, `make lint` (all
      nine steps), `make typecheck` and the whole `make test` recipe each
      exited zero. The workspace leg ran 2,251 tests with 2,251 passed and 7
      skipped in 594.4 s, the no-default-features leg 787 with 787 passed and
      7 skipped in 330.8 s, and `runner::step_macros_compile` passed in both
      (426.6 s and 320.5 s). The doc-test leg, `make check-fixture-lockfiles`
      (5 lockfiles up to date) and `uv run pytest scripts/tests` (291 passed)
      finished behind them, and no `FAILED`, `error[` or `make: ***` line
      appears in the log.

- [x] (2026-11-13) A `/simplify` pass over the same files produced the final
      pre-commit tree. Its first attempt narrowed the discovery helper's keys
      to `&'static str`, which made `key.as_str()` at the call site resolve to
      the unstable inherent `str_as_str` (`E0658`) rather than
      `String::as_str`; `make lint`, `make typecheck` and `make test` all
      stopped at that one compile error. The key type is restored: the
      allocation is immaterial in a test helper, and a `&'static str` key that
      silently changes which `as_str` a later edit resolves to is a trap worth
      more than the allocation it saves. Only the `filter_map` cleanup
      survived, so the gate verdict below supersedes `e153116b`. Gates were
      run in order on the tree whose diff against `e153116b` hashes to
      `5aed98256d0f0df2f54fb5f920e98f60368156036772c2255718eb4d115eb258`:
      `make check-fmt`, `make markdownlint` (0 errors, spelling included),
      `make nixie`, `make lint` (all nine steps), `make typecheck` and the
      whole `make test` recipe each exited zero. The workspace leg ran 2,251
      tests with 2,251 passed and 7 skipped in 847.6 s, the
      no-default-features leg 787 with 787 passed and 7 skipped in 464.8 s,
      and `runner::step_macros_compile` passed in both (696.3 s and 444.2 s).
      The doc-test leg, `make check-fixture-lockfiles` and
      `uv run pytest scripts/tests` (291 passed in 24.0 s) finished behind
      them, and the log holds no `FAILED`, `error[` or `make: ***` line.

- [x] (2026-11-13) The next review round on `8b6a8002` reported two minor
      findings, and both were real. First, `discovery_variables` redirected
      `HOME` but not `XDG_CONFIG_DIRS`. The vendored
      `ortho_config` source settles this: `source_xdg_bases`
      (`src/subcommand/paths.rs`) extends the base list with every absolute
      `XDG_CONFIG_DIRS` entry and, when the variable is absent or wholly
      relative, with `/etc/xdg`; `load_from_files` (`src/subcommand/mod.rs`)
      merges *every* existing candidate. The `cli` suite's child starts from
      the ambient environment through `cli_child_environment`, so a host
      `/etc/xdg/cargo_bdd/config.toml` really was reachable. Measured with
      `XDG_CONFIG_DIRS` pointed at a directory holding a malformed
      `cargo_bdd/config.toml`, 2 of the 4 cases failed before the change and
      all 4 passed after it. `XDG_CONFIG_DIRS` and `USERPROFILE` are now both
      redirected, and `cargo_home_variables` falls back from `HOME` to
      `USERPROFILE` so redirecting the latter cannot strand Cargo's home. The
      Windows native configuration directory comes from the Windows API, not
      `APPDATA`, so it is unredirectable by any variable; the module
      documentation now states that as the one residual rather than implying
      full isolation. Second, ADR 025 and the developers' guide claimed the
      `cargo-platform` v0.3.2 lock resolution preserved the Rust 1.88 library
      MSRV without qualification. That is true only for builds resolving
      through this repository's lockfile: a downstream resolver-2 consumer
      keeps its own, where the caret requirement still admits v0.3.3 and its
      Rust 1.91 floor. Both passages now say so, matching the qualification
      the plan's decision log already carried. Gates on this round's tree
      (`8b6a8002` plus a diff hashing to
      `8404b0099cdf55b3ffddb63331d724b9328d8d38d688b4dd37880e59df29037b`)
      ran `make markdownlint` 0, `make nixie` 0, `make lint` 0, `make
      typecheck` 0 and the whole `make test` recipe 0, with 2,251/2,251
      passed in 486.6 s, 787/787 passed in 309.1 s, `step_macros_compile`
      PASS in both and 291 pytest passed, and no `FAILED` or `error[` line.
      `make check-fmt` alone failed there, because nightly rustfmt wanted the
      new `env::var` chain joined onto one line; the fix is whitespace-only,
      so `make check-fmt`, `make markdownlint`, `make nixie`, `make lint`,
      `make typecheck` and the `cargo-bdd` `cli` target were re-run on the
      final tree and all exit zero.

- [x] (2026-11-13) The third review round on `5d990774` reported one major
      finding in
      `crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly/lint_config.rs`,
      a file this branch's `3e94e1b6` had already converted to `concat!`. Its
      substantive half is right and was proven twice. `\\` at the end of a
      literal is an escaped backslash, not a line continuation, so the
      string's value was `...patterns for \` + newline + five spaces +
      `parameter extraction...\`, and that value is emitted into the
      `#[expect(reason = "...")]` of every generated wrapper. A standalone
      reproduction printed the three source lines with `\\` and their
      indentation intact, and the `insta` snapshot
      `marker_spelling_generated_wrapper_snapshot` pinned the same corruption
      inside the generated code a user can read. Each argument is now one
      whole line, the snapshot was re-accepted, and a new
      `wrapper_expect_reason_is_single_line_prose` test asserts the shape
      rather than comparing the constant to itself; restoring the old literal
      makes it fail with the backslash and newline it rejects. Its lint half
      is not reproducible: it claims `make lint-whitaker` rejects this form,
      but that target is the fourth step of the nine-step `make lint` recipe,
      which passed on the tree that contained the old literal. The shape test
      therefore stands in for a lint that does not currently fire, and the
      conversion guidance in ADR 013's addendum still defers to the installed
      suite. Gates ran on the tree whose diff against `5d990774` hashes to
      `ba90b5f82c8dcf1f036e496c63b1cb62ae16dfd35442a2b518d53697cb76bef6`:
      `make check-fmt`, `make markdownlint`, `make nixie`, `make lint` (all
      nine steps, `lint-whitaker` among them), `make typecheck` and the whole
      `make test` recipe each exited zero. The workspace leg ran 2,252 tests
      with 2,252 passed and 7 skipped (one more than before, the new guard),
      the no-default-features leg 787 with 787 passed and 7 skipped, and
      `runner::step_macros_compile` passed in both. `uv run pytest
      scripts/tests` ran 291 passing tests, and the log holds no `FAILED`,
      `error[` or `make: ***` line.

- [x] (2026-11-13) The fourth review round on `155487af` reported two minor
      findings and both are real. First, the previous round fixed discovery
      isolation only inside `cli/configuration.rs`; `crates/cargo-bdd/tests/cli.rs`
      drives the same binary through `cli_child_environment`, which dropped
      inherited `CARGO_BDD_*` variables but still passed `HOME` and the XDG
      roots through. Measured with a `HOME` holding
      `[cmds.skipped] json = true`, `skipped_subcommand_includes_reasons_and_lines`
      failed on a run that otherwise passes, exactly as the review predicted.
      Redirecting one root would repeat the previous mistake, so
      `empty_configuration_home_variables` now overrides `HOME`,
      `USERPROFILE`, `XDG_CONFIG_HOME` and `XDG_CONFIG_DIRS` with three empty
      sibling directories under one process-keyed parent, while
      `ambient_cargo_home_variables` carries `CARGO_HOME` and `RUSTUP_HOME`
      from the ambient user home so the nested build keeps its registry. The
      same poisoned `HOME` now passes, and two new tests in
      `cli/environment.rs` assert that every root exists, is empty, and is not
      where Cargo's homes point. Writing them caught a flaw in the first
      attempt: the home root held the two XDG directories, so it was not empty.
      Second, the plan's Table 1 listed `bdd steps` at exit 2 as a `V1` case
      while `V3` requires that form to succeed, which makes the two
      obligations contradictory. The checked-in fixture
      `tests/fixtures/baseline-cli.json` already held exactly the seven
      flat-form rows and omitted `bdd steps`, so only the plan text was wrong;
      the row is now labelled pre-change evidence for `V3`, and `V1` says so.

- [x] (2026-11-13) Those helpers took `crates/cargo-bdd/tests/cli.rs` to 433
      lines, past the 400-line gate
      (`python3 scripts/check_rs_file_lengths.py`, `LEN_RC=1`). Rather than add
      a temporary allowlist entry, the redirection moved to a new
      `crates/cargo-bdd/tests/cli/discovery.rs`, attached to the target with
      `#[path = "cli/discovery.rs"] mod discovery;` beside `configuration` and
      `environment` so Cargo does not compile it as a second test target. The
      split is by cohesion: the module owns the file tier's environment policy
      and the sibling `environment` module asserts the map it produces.
      `cli.rs` is now 383 lines and the gate reports `LEN_RC=0`. Behaviour is
      unchanged: `cargo test -p cargo-bdd --test cli` ran 21 tests with 21
      passed, the four `configuration::` cases among them, and the poisoned
      `HOME` control recorded in the entry above still passes.

- [x] (2026-11-13) The first gate cycle on the split tree caught two defects
      of the new code, both in what this round added rather than in the split.
      `make check-fmt` rejected the import order in `cli.rs` (nightly rustfmt
      wants `use discovery::…` after `use assert_cmd::Command;`). `make lint`
      rejected `assert_ne!(roots[0], roots[1], …)` in
      `cli/environment.rs` with `-D clippy::indexing-slicing` twice, because
      `roots` is a `Vec` the helper builds at run time. The replacement asserts
      that all three roots are pairwise distinct rather than only the first
      two, using a `HashSet` of references; that is both slice-free and a
      stronger claim, since `XDG_CONFIG_DIRS` shadowing `XDG_CONFIG_HOME` would
      otherwise pass. `make check-fmt` and `make lint` then exited zero.

- [x] (2026-11-13) Final deterministic gates on the split tree, `REV_START`
      `155487afc95b9a8278c491973beea1979b86ae40`. The gate inputs were hashed
      rather than assumed. The Rust files' combined diff sha256 — the tracked
      diffs of `cli.rs` and `cli/environment.rs` plus the object id of the new
      `cli/discovery.rs` — is
      `c7a1faa8d81a2d07403fd3bc968e0f98cb7052f486e10303b3772a6a39225127`,
      and it was unchanged when `make lint`, `make typecheck` and `make test`
      ran. A whole-arm hash cannot be recorded as a stable figure, because the
      plan document is itself one of the inputs and every edit to this entry
      changes it; the Markdown-reading gates were instead re-run last, after
      the final edit, and each exited zero on that tree.
      `make check-fmt` exited zero with `142 files left unchanged`;
      `make markdownlint` exited zero over `141 file(s)` with
      `Summary: 0 error(s)`; `make nixie` exited zero with
      `All diagrams validated successfully`; `make lint` exited zero through
      all nine steps, `lint-whitaker` and `check_serial_nextest_matrix.py`
      among them; `make typecheck` exited zero with `All checks passed!`; and
      `make test` exited zero. The workspace leg of `make test` ran 2,254
      tests with 2,254 passed and 7 skipped, the `--no-default-features` leg
      787 with 787 passed and 7 skipped, `runner::step_macros_compile` passed
      in both, and `uv run pytest scripts/tests` ran 291 passing tests. The
      workspace count is two above the 2,252 recorded at `ba90b5f8`: the two
      new `cli/environment.rs` tests this round added. Each log was checked
      for `FAILED`, `error[` and `make: ***` and holds none.

- [x] (2026-11-13) The first attempt at writing up these results put a
      multi-line list inside a list item, which `make markdownlint` rejects as
      `MD046` indented code block, and it duplicated the
      `## Surprises & discoveries` heading. Rewriting the entry as prose fixed
      both. Check-fmt, markdownlint and nixie were then re-run on the changed
      document and exited zero; lint, typecheck and test were not re-run
      because none of them reads this document, and the Rust gate-input hash
      above shows the files they do read were byte-identical.

- [x] (2026-11-13) Review round 10: `coderabbit review --agent --base
      origin/main` on `69b4aefc` returned seven findings, in
      `/tmp/coderabbit-issue-901-adopt-ortho-config-v0-10-0-for-the-cargo-bdd-cli.out`.
      Each was settled by measurement rather than by the review text; the
      verdicts are recorded below, one entry each.

- [x] (2026-11-13) Finding 1, `no_unwrap_or_else_panic` on
      `environment.rs:81`, was first marked **not confirmed** on the strength
      of Whitaker's `should_flag` exemption and a mutation control: a
      plain-`panic!` variant of the same line made `make lint-whitaker` fail
      while the interpolated one passed (`/tmp/lint-whitaker-mutation-issue-901.out`).
      Round 12 re-raised it citing ADR 013, and that citation is decisive:
      the ADR says the shape "remains rejected" and that `clippy.toml` permits
      `.expect(...)` and `panic!(...)` in tests instead. The exemption is an
      implementation detail of the lint, not a repository policy, so the
      earlier verdict was wrong in the way that matters. The code now uses
      `let … else`, which keeps the interpolated message. Grep confirms the
      repository has no other live instance of the shape.

- [x] (2026-11-13) Finding 2, a custom `CARGO_HOME` or `RUSTUP_HOME`, is
      **confirmed**. The old assertion required the value's last component to
      be `.cargo` or `.rustup`, which a supported custom home need not
      satisfy. The replacement recomputes the expected value with the helper's
      own precedence — an explicit variable wins, otherwise the user-home
      default — and additionally asserts the value does not fall inside the
      redirect directory, which is the property the test exists for.

- [x] (2026-11-13) Finding 3, an ambient-policy mutation test in
      `skip_parity/resolution_once.rs`, was declined on round 10 because the
      finding as then written mutated the global *between* runs, which no test
      can distinguish from a correct implementation. Round 11 restated it
      narrowly — mutate inside the handler, on a scope that took its policy
      from the global — and that version describes a real gap. The row was
      added, and a faithful mutation of the driver (re-read the global for
      ambient scopes at record-assembly time) fails it **alone**, 9 passed and
      1 failed, with its own assertion message. The earlier verdict was too
      broad and is corrected in the Decision log.

- [x] (2026-11-13) Findings 4 and 5 are **confirmed** and were
      self-inflicted: `V5` claimed the isolation check covered every discovery
      root, and the acceptance criterion demanded a `bdd steps` baseline row
      that is pre-change evidence for `V3`. `V5` now names the Windows native
      directory as the one root a variable cannot redirect, and the criterion
      reproduces every `V1` row except `bdd steps`.

- [x] (2026-11-13) Finding 6, importing the MSRV guarantee without
      qualification, is **confirmed** and branch-caused: `origin/main` had
      `rust-version.workspace = true`, and this branch moved `cargo-bdd` to
      `"1.89"`, so "every `Cargo.toml` declares `rust-version = 1.88`" became
      false. The users' guide now states the split, and the migration guide
      adds the dependency-resolution qualification — a consumer lockfile that
      resolves a newer `cargo-platform` inherits its higher floor.

- [x] (2026-11-13) Finding 7, `let _ = create_dir_all(…)` in the new
      `discovery.rs`, is **confirmed**; the repository has no precedent for
      discarding a directory-creation error. The module now creates its
      directories through `cap_std::fs_utf8::Dir::create_ambient_dir_all` and
      returns the error.

- [x] (2026-11-13) Final gates on the review-round-10 tree, `REV_START` and
      `REV_END` both `69b4aefcebb3e40c28e74b1ebcbf636f0e069340`. The Rust gate
      inputs — the tracked diffs of `cli.rs`, `cli/discovery.rs` and
      `cli/environment.rs` — hashed
      `a487ad03b5c2e8f00c65daddce2e18c756f77d0bf91f7154c110825312ae00ad` at
      the start of `make lint` and again at the end of `make test`, so every
      Rust-reading gate saw the same bytes. `make check-fmt` exited zero with
      `142 files left unchanged` and `93 files already formatted`;
      `make markdownlint` exited zero with `Summary: 0 error(s)`; `make nixie`
      exited zero with `All diagrams validated successfully`; `make lint`
      exited zero through all nine steps, `cargo clippy` and
      `whitaker --lib whitaker_suite` each reporting `Finished` for
      `cargo-bdd`; `make typecheck` exited zero with `All checks passed!`; and
      `make test` exited zero with 2,255 workspace tests passed and 7 skipped,
      787 passed and 7 skipped on the `--no-default-features` leg,
      `runner::step_macros_compile` passing in both, and 291 pytest tests
      passing. The workspace count is one above the 2,254 recorded at
      `69b4aefc`, which is
      `cli_child_environment_carries_an_explicit_cargo_home`; every
      `cargo-bdd::cli` test is `PASS` in the log, the four
      `configuration::skipped_handler_uses_configured_reasons` cases and the
      eight `environment::` and `nested_target::` cases included. No log
      contains `FAILED`, `error[` or `make: ***`.

- [x] (2026-11-13) Round 11: `coderabbit review --agent --base origin/main` on
      `1fcbc3d2` returned one finding, on
      `crates/rstest-bdd/tests/skip_parity/resolution_once.rs:150`, asking for
      an ambient-policy in-flight mutation row. It is **confirmed** and the row
      was added; see the Progress entry above and the revised Decision log
      entry. The review log is `/tmp/coderabbit-issue-901-r11.out`.

- [x] (2026-11-13) Round 12, on `ea88d0b1`, returned one finding: re-raise the
      `unwrap_or_else(|| panic!(…))` shape, this time citing ADR 013
      (`/tmp/coderabbit-issue-901-r12.out`). **Confirmed**, and the round-10
      verdict is withdrawn — see the amended entry above and the Decision log.
      The rewrite is `let … else`, not `.expect(…)`: the review offered
      `expect` as an alternative, but a fixed string cannot name which of the
      two variables was missing, which is what the interpolated message does.

- [x] (2026-11-13) Gates on the round-11 tree, `REV_START` and `REV_END` both
      `1fcbc3d2eeed03981b4b36f237df792be6bd3c76`. The Rust gate-input hash was
      `27b15655eb10195b2479a8af2531c7a6f8ba86fb74ce4cdfc8c0b1d755baa742` at
      the start of `make lint` and unchanged after `make typecheck` and
      `make test`. `make check-fmt` 0 (`142 files left unchanged`),
      `make markdownlint` 0 (`Summary: 0 error(s)`), `make lint` 0 through all
      nine steps, `make typecheck` 0 (`All checks passed!`), and `make test` 0
      with 2,256 workspace tests passed and 7 skipped, 788 passed and 7 skipped
      on the `--no-default-features` leg, and 291 pytest tests passing. The new
      row is `PASS` in both legs. No log contains `FAILED`, `error[` or
      `make: ***`. `make nixie` exited zero with
      `All diagrams validated successfully!`; the plan and migration guide hold
      no Mermaid block, so it ran for the users' guide's one diagram.

- [x] (2026-11-13) Two defects were found and fixed in the gate cycle above
      rather than carried. The first was `rustfmt` wanting
      `ambient_cargo_home_variables` on one line after the signature had been
      shortened. The second was a repeat of an earlier `MD046` failure: the
      verdicts were written as one bullet with blank-line-separated
      paragraphs indented six spaces, which `markdownlint` reads as an
      indented code block. Rewriting them as one bullet per verdict, in the
      section's existing style, cleared it. The Markdown gates were re-run
      after that rewrite, and `check-fmt` was re-run last because
      `mdtablefix` had reflowed two of the paragraphs.

- [x] (2026-11-13) The `cap_std` rewrite of `discovery.rs` needed two rounds
      to compile. `create_ambient_dir_all` takes camino paths, so the cached
      `PathBuf` is converted per call through `Utf8PathBuf::from_path_buf`,
      which reports a non-UTF-8 temporary directory instead of naming a
      different one by lossy encoding. The converted value is deliberately not
      cached: the conversion can fail, and caching a failure would remember it
      for the whole test binary, the same argument the cache already makes
      about `io::Result`. The shared `home_root` gained a `clone()` because
      `HOME` and `USERPROFILE` were both redirected to it; the round-13 entry
      below removes that second redirect and with it the clone. `cargo check
      -p cargo-bdd --tests` is clean, `cargo test -p cargo-bdd --test cli` ran
      22 tests with 22 passed, and the poisoned-`HOME` control — a real
      `$HOME/.cargo_bdd.toml` setting `cmds.skipped.json = true` — leaves
      `skipped_subcommand_includes_reasons_and_lines` passing, which is the
      behaviour the redirection exists to buy.

- [x] (2026-11-14) Round 13, on `28b6adef`, returned zero findings
      (`/tmp/coderabbit-issue-901-r13.out`).

- [x] (2026-11-14) Both Windows CI jobs were red from `5d990774` onward while
      every local Linux gate was green. Root cause: that commit added
      `USERPROFILE` to the CLI test child environment's discovery redirect,
      and the child environment is *filtered* — `NestedCargoEnv::apply_to`
      calls `env_clear()` and re-applies only the map — so the variable
      vanished from every nested process. `sccache` 0.12.0, which CI installs
      as `RUSTC_WRAPPER`, resolves its own configuration through
      `directories::ProjectDirs`; on Windows that routes to the Known Folder
      API, which resolves through the user profile. With `USERPROFILE`
      removed the lookup returned `None`, `sccache` panicked at
      `config.rs:961` with "Unable to get config directory", and every nested
      `cargo metadata` exited 101. The failure was misattributed at first to
      the `configuration.rs` helper alone; the reason it hit unrelated tests
      such as `nested_target::an_absent_target_falls_back_to_the_workspace_target`
      is that the clear-and-reapply is process-wide, so one missing key
      affects every child.

- [x] (2026-11-14) Fixed by not redirecting `USERPROFILE`, which is redundant
      as well as harmful: `ortho_config-0.10.0/src/subcommand/paths.rs` reads
      `HOME` first and falls back to `USERPROFILE` only when `HOME` is absent
      (non-Unix `collect_non_unix_paths`), so redirecting `HOME` already keeps
      a developer's home dotfile out of the child. Removed the redirect from
      `discovery_variables` in `crates/cargo-bdd/tests/cli/configuration.rs`
      and from `empty_configuration_home_variables` in
      `crates/cargo-bdd/tests/cli/discovery.rs`, dropping the now-unneeded
      `clone()`. The two remaining `USERPROFILE` writes —
      `tests/baseline_output.rs` and `tests/invocation/workspace.rs` — are not
      the same defect and are unchanged: both call `env_clear()` themselves
      and then *add* a valid `USERPROFILE`, so `sccache` has a profile to
      resolve and never runs without one.

- [x] (2026-11-14) Guarded by
      `cli_child_environment_keeps_userprofile_inherited` in
      `crates/cargo-bdd/tests/cli/environment.rs`. Verified non-vacuous by
      mutation control: reinstating `USERPROFILE` in
      `empty_configuration_home_variables` fails that row alone — 6 passed, 1
      failed — with `left: Some("/tmp/cargo-bdd-cli-empty-home-…/home")`,
      `right: Some("/home/carried-through")`. The mutation was reverted and
      the file restored from a saved copy.

- [x] (2026-11-14) Gates on the round-13 tree, `REV_START` and `REV_END` both
      `28b6adef`. The Rust gate-input hash was
      `aa2204ae3138a045c91cac6ae309f241e529bdf169220dcbdbcef9ab3b89f3c2` at
      the start of `make check-fmt` and unchanged after the last test run. It
      differs from the round-12 value `7af567e0…` because this round edits Rust
      gate inputs. `make check-fmt` 0, `make markdownlint` 0, `make nixie` 0,
      `make lint` 0 through all nine steps, `make typecheck` 0
      (`All checks passed!`), and `cargo test -p cargo-bdd` 0 with 46 + 1 + 23
      + 21 tests passing across its four targets. The nested-fixture target
      took 676.63 s under compile-admission contention and still passed.

- [x] (2026-11-14) CI run `43561135082` on `f93347ef`: both Windows jobs
      **pass**, which is the fix this round is for. Windows `default features`
      in 34m54s and `strict-compile-time-validation` in 44m58s, both
      completing the step that was previously dying, so the `sccache` panic is
      gone at its source. The Linux lane failed instead, on a 60 s timeout of
      `rstest-bdd-harness::binary_test_support_cargo
      locate_or_build_returns_command_for_workspace_binary` — a test in a
      crate and file this branch has only reformatted, and one that is *not*
      in the `cargo-spawning` test group, so it runs in the default 60 s lane
      while spawning a nested `cargo build`.

- [x] (2026-11-14) Diagnosed the Linux timeout as host contention, with three
      independent lines of evidence. First, the run was uniformly slow: the
      same test took 8.49 s on the previous green run at `28b6adef` and
      exceeded 60 s here, while
      `configuration::skipped_handler_uses_configured_reasons::case_1_file_tier`
      went from 14.95 s to 74.15 s in the same commit — a 5x slowdown across
      every nested-`cargo` test rather than a defect confined to one test.
      Second, the change
      is provably a no-op on Linux: `crates/cargo-bdd/src/` never reads
      `USERPROFILE`, and `ortho_config`'s `collect_unix_paths` consults only
      `HOME` and the XDG bases, so dropping a variable no Linux code path
      reads cannot alter behaviour there. Third, `main` is green on all three
      lanes, so the branch is the only difference from a passing baseline — but
      this specific test's failure mode is timing, not logic. A re-run of the
      failed job was triggered rather than any code change.

- [x] (2026-11-14) The two stale plan statements the fix invalidated were
      corrected in place: the Risks mitigation that listed `USERPROFILE` among
      the variables the binary-level tests isolate, and the Verification plan's
      contract-level discovery check. Both now state the policy and point at
      this entry. The earlier Progress entries that describe the redirect as it
      was are left as written, because they record what was true at the time.

- [x] (2026-11-14) The re-run of the failed Linux job confirms the contention
      diagnosis: `f93347ef`'s `build-test` suite completes **SUCCESS** on all
      three jobs, Linux included, with no code change to the test that timed
      out. That makes `f93347ef` the latest completed hosted verdict, and the
      status line now names it rather than the earlier `76f8f63e`. The
      superseded `8a532cac` suite shows `CANCELLED`, which is the PR-scoped
      concurrency rule cancelling a head's run when the next push arrives, not
      a failure.

- [x] (2026-11-14) CodeRabbit round 16 raised two findings, both actioned.
      The first was a real defect in this branch's own test harness: the
      `CARGO_BDD_` isolation filter in `crates/cargo-bdd/tests/cli.rs` compared
      the prefix case-sensitively, while Windows resolves variable names
      case-insensitively and `std::env::vars_os` returns the parent's spelling.
      An inherited `cargo_bdd_log` therefore reached the binary's literal
      `CARGO_BDD_LOG` lookup in `observability.rs` and could turn on stderr
      tracing during a smoke test. The filter now compares
      `eq_ignore_ascii_case` on Windows and stays exact elsewhere, with a
      `cfg(windows)` regression case and a prefix-boundary case that proves the
      comparison did not widen to the `CARGO_BDD` stem.

- [x] (2026-11-14) The second finding was correct about the symptom and stale
      about its premise. It reported that no later hosted run turned the Linux
      job green; in fact `f93347ef`'s re-run did, as the entry above records.
      The status line was nevertheless out of date in naming `76f8f63e`, so it
      was corrected. The current head's suite was still in progress when it was
      read, so the line names `f93347ef` as the latest *completed* verdict and
      claims nothing about `0952d888`.

- [x] (2026-11-14) CodeRabbit round 17 raised seven findings. Four were code
      changes, two were record corrections, and one is an upstream defect this
      repository must not fix in the imported text.

- [x] (2026-11-14) Toolchain-home resolution was wrong on Windows, in three
      copies. `toolchain_home` in `cli/discovery.rs`, `cargo_home_variables` in
      `cli/configuration.rs` and `ambient_location` in `cli/environment.rs` all
      derived `CARGO_HOME`/`RUSTUP_HOME` from `HOME` first. Cargo and Rustup
      resolve their home through the `home` crate, which on Windows reads
      `USERPROFILE` and the profile Known Folder API and never `HOME`; the file
      *discovery* order is a different thing and does read `HOME` first, which
      is why only `HOME` is redirected. A Windows shell that sets both to
      different values was therefore handed a registry under the wrong root.
      All three now use `USERPROFILE`-first on Windows, skip an empty value,
      and keep explicit overrides.

- [x] (2026-11-14) Two shared helpers panicked outside a test boundary, which
      `docs/developers-guide.md` forbids: `cli_child_environment` discarded the
      `io::Error` from `empty_configuration_home_variables` and panicked, and
      `render_listing` used two `.expect()` calls. Both now return `Result` and
      propagate with `?`; their callers keep the terminal assertion at the test
      boundary. `cli_child_environment`'s `?` still returns before any
      environment is built, so a failure cannot fall back to the host's
      discovery roots.

- [x] (2026-11-14) The seven shared assertion helpers in the `invocation`
      target became macros, so a failure reports the case that chose the input
      rather than a line inside the helper body that six cases share. The
      macros are defined above their first use because `macro_rules!` is
      textually scoped, and the two the child module calls are re-exported.

- [x] (2026-11-14) Two plan claims were corrected rather than defended. The
      `make test` row claimed final-tree acceptance from the `8a5cb67f` run,
      which is historical: `git rev-list --count 8a5cb67f..41f50191` reports
      fifteen commits between it and this text. That claim was withdrawn, the
      recipe was then re-run on the clean tree at `404c8861` with `REV_START`
      and `REV_END` both recording that revision, and the row now cites that
      anchored verdict instead. The §6 entry said the current head's hosted
      checks were green; it now names `f93347ef` as the latest completed
      verdict and claims nothing about later heads.

- [x] (2026-11-14) The seventh finding is a real defect in the imported
      OrthoConfig guide and is *not* fixed here. Its `guide-subcommand-sources`
      comment claims "nothing outside this map and the explicit base is
      consulted", which on non-Unix targets is false: the same example still
      reads `HOME`/`USERPROFILE` or the platform home fallback and
      `EnvSource::config_dir_fallback`. The provenance contract forbids editing
      the verbatim copies, so the correction was recorded in
      `docs/rstest-bdd-design.md` and belongs upstream in `leynos/ortho-config`.

- [x] (2026-11-14) Round 18 reported four findings, all in scope and all fixed.
      Findings 1 and 2 found a *fourth* copy of the Windows home defect, in
      `crates/cargo-bdd/tests/invocation/workspace.rs`, which redirects four
      roots and so was missed when the first three were corrected. It read
      `HOME` first on both platforms, and it skipped both explicit toolchain
      homes whenever neither ambient home existed even though `env_clear()`
      had already removed them. Each override is now resolved independently,
      and the fallback derives from `toolchain_home()`. The invocation target
      passed 21 of 21 cases after the change, which includes the
      Cargo-dispatched forms the finding said could not find the toolchain.
      Finding 3 was right that the users' guide named only the first existing
      XDG directory; `ortho_config`'s `source_xdg_bases` consults every
      absolute `XDG_CONFIG_DIRS` entry in its listed order, so the guide now
      says so. Finding 4 is now stated precisely rather than dropped: the two
      fixtures redirect different amounts, only Unix and Redox are fully
      isolated because only there are the XDG bases the whole search, and the
      `cli` helper deliberately leaves `USERPROFILE` inherited for `sccache`.

- [x] (2026-11-14) The round-18 commit `f10c0e4d` passed hosted CI in run
      `43582906292`: all three `build-test` lanes succeeded, and those three are
      the only contexts the `main-required-checks` ruleset requires. The
      advisory CodeScene gate failed on that head for `String Heavy Function
      Arguments` in `crates/cargo-bdd/tests/invocation/workspace.rs` — a rule
      about code this branch had just written, since the new helper took the
      variable name and its default directory as two `&str` parameters. The
      pair was transposable without a type error, so `de4de598` replaces it with
      a `ToolchainHome` enum: one value carrying both facts, iterated at the
      call site. Suppressing the rule would have left the hazard in place.

- [x] (2026-11-14) `make test` has been re-taken at `de4de598` and then at
      `f7fa8cd6`, each time because the commit before it changed a gate input,
      and each exited zero with `REV_START` and `REV_END` both the revision it
      names: 2258 of 2258 nextest tests passed with 7 skipped, 788 of 788 doc
      tests passed with 7 skipped, and 292 pytest tests passed. The earlier
      verdicts were not reused for a tree with a changed gate input, and the
      paragraph that once explained why an older one still applied was replaced
      rather than left to misdescribe the current evidence.

- [x] (2026-11-14) Round 19 reported three findings, and all three were right
      about code or prose this branch had introduced. Finding 1 named the
      `HOME` fallback in the Windows toolchain-home selection; a fourth copy of
      that selection existed too, so the rule was consolidated into
      `discovery::toolchain_home` and the three sibling modules now call it
      instead of restating it. `home` reads `USERPROFILE` then the profile
      Known Folder API on Windows and `HOME` on Unix, so no platform-ordered
      fallback list is correct. Finding 2 was the relative-path case: `home`
      joins a relative home onto the current directory, and the invocation
      fixture changes that directory before the child starts, so explicit homes
      are now resolved against the parent's directory first. Finding 3
      corrected this branch's own edit to the users' guide: `cargo-bdd`
      selects one XDG file and never merges later bases, so the "first that
      defines the setting" wording was replaced with the rule and its
      consequence. The design document already stated it correctly.

- [x] (2026-11-14) Hosted CI run `43592197882` passed at `a634e98a` with all
      three required `build-test` lanes green, and CodeScene and Gecko passed
      with it. That closed the last gap the previous status line named. A
      correction followed: the line had said the head's own hosted run was
      green at `f7fa8cd6`, and GitHub had recorded no run for that revision —
      the branch's runs at the time of writing were `dbede7ea`, `f10c0e4d`,
      `a634e98a`, and the head current then. Both the
      status line and the section 6 checkbox now state the reachability
      properly instead: `a634e98a` carries every line of code the local
      `make test` row was taken on, the two commits after `f7fa8cd6` edit only
      this plan, and the Markdown gates that do read it were re-run at the new
      head. Those re-runs were taken at `3d9aaab5` with `REV_START` and
      `REV_END` both recording it: `make check-fmt`, `make markdownlint`,
      `make spelling` and `make nixie` exited zero, as did `make lint` (all
      nine steps, zero warnings in the log), `make typecheck`, and
      `cargo test -p cargo-bdd --test invocation --test cli` (21 and 24 cases).
      Separately, PR #912 is not a draft: GitHub's timeline shows a
      `ready_for_review` event by `leynos` at 2026-11-07T23:27:23Z, before this
      round's work, and no `convert_to_draft` event. The brief asked for a
      draft PR, so the state is recorded here rather than silently reversed.

- [x] (2026-11-14) Hosted CI run `43596802177` failed at the then-head
      `83a0e488` on the required
      `build-test (windows, strict-compile-time-validation)` lane: one test
      failed and 44 passed, in
      `config::tests::observability::both_loaders_emit_one_bounded_configuration_span`,
      with `left: 1` against `right: 2`. The same code had passed the hosted
      lane at `a634e98a`, so the assertion was reporting a flake rather than a
      regression in the diff. The flake was reproduced locally before anything
      was changed — 1 failure in 25 runs of the `cargo-bdd` unit-test binary,
      and 0 in 80 runs of the test on its own — and diagnosed rather than
      papered over.
      A first hypothesis was that the callsite's cached interest went stale,
      and that `tracing::callsite::rebuild_interest_cache` inside the
      `with_default` scope would fix it. It was written, and then measured: 5
      failures in 60 runs, worse than the unmodified test's rate. The rebuild
      is not the fix, so it was not kept. The failing capture was then made to
      report the command label and the process-global `LevelFilter`. The
      `LevelFilter` stayed `TRACE` in every failing run, which rules out level
      gating, and the label showed that *either* loader could be the missing
      one. The remaining mechanism is callsite interest, which is process-wide
      while the dispatcher that computes it is per-thread.
      `DefaultCallsite::register` resolves interest through
      `dispatcher::get_default`, i.e. the calling thread's default falling
      back to the global one; `tracing::subscriber::with_default` installs a
      thread-local default without recomputing anything. A sibling test that
      reaches one of these loaders on its own thread therefore caches an
      interest computed against the process-wide `NoSubscriber`, and the
      loader that loses that race opens a disabled span no layer ever sees.
      The fix installs the capturing subscriber with
      `tracing::subscriber::set_global_default`, so no thread falls back to
      `NoSubscriber`. Because that makes the capture process-wide, each
      captured span now records its `ThreadId` and the assertions read back
      only the calling thread's spans, which keeps a concurrent sibling's span
      from being counted as this test's. The fix was confirmed load-bearing by
      a control: the same body with a thread-local `with_default` restored
      failed 6 of 60 runs, while the global-default version failed 0 of 200
      (0 of 150 before that). The exact lane that failed was then reproduced
      with its own command shape — `cargo test --tests --workspace
      --no-default-features --features 'rstest-bdd/diagnostics
      rstest-bdd-server/test-support strict-compile-time-validation'` — and
      exited zero with 2214 passed, 0 failed and 7 ignored.
      The fix was committed as `0b798c1e` and pushed, and hosted run
      `43604994289` passed there with all three required `build-test` jobs
      green, including `build-test (windows,
      strict-compile-time-validation)` — the lane that had failed at
      `83a0e488`. The failing test's own verdict is therefore re-taken for the
      fixed tree rather than carried over from the earlier run. The commit
      recording that verdict, `17cf96e7`, was then pushed and run again as
      `43608966901`, which also passed all three lanes, so the pushed head
      carries its own verdict rather than one inherited from the commit
      before it. The next plan-only commit, `413b46c3`, failed the required
      Linux lane in its `Check formatting` step at 28 seconds: `mdtablefix
      --check` reported this file as `+1 -2`, so `make check-fmt` exited 2 and
      the Markdown, spelling, lint and test steps behind it never ran. The
      cause was a paragraph break the previous commit introduced — a line
      ending `run.` followed by one beginning `Earlier` — which mdtablefix
      joins into a single line of exactly 80 columns. No word changed and no
      inline code span was split. `2a017af9` applied that rewrap; `make
      check-fmt` then reported 142 files unchanged and exited zero, `make
      markdownlint` still reported 0 errors, and hosted run `43617023742`
      passed all three lanes there. My own gate for the two preceding commits
      had been `make markdownlint`, which does not run mdtablefix, even though
      `make check-fmt` is the gate the hosted lane runs first and the file
      being edited is its input.

## Surprises & discoveries

- Observation: `make markdownlint` passing is not evidence that a Markdown
  edit passes the gate the hosted Linux lane runs. `make check-fmt` runs
  `mdtablefix --check` with `--wrap` after the rustfmt steps, and
  `make markdownlint` does not run mdtablefix at all. Evidence: commit
  `413b46c3`, which edited only this plan, passed `make markdownlint` locally
  and failed hosted run `43612834665` in `Check formatting` with `+1 -2` on
  this file; reproducing with `make check-fmt` exited 2 in the same step.
  Impact: a plan-only commit still needs the formatting gate, and the specific
  thing it enforces here is that a paragraph break between two prose lines is
  not preserved — the joined line was exactly 80 columns, so it is a fixed
  point of a re-run and does not oscillate. Applying the fixer needs
  `--in-place`; without it mdtablefix only prints the result, and `--check` and
  `--diff` agreeing with an unchanged worktree is not evidence the fix was not
  needed.

- Observation: `cargo-bdd bdd steps` — the form Cargo produces — was rejected
  by the pre-change binary with `error: unrecognized subcommand 'bdd'`, exit 2,
  0 bytes on stdout and 102 bytes on stderr. Evidence: captured in
  `/tmp/baseline-cli-errors-issue-901.out`. Impact: this is a real defect, not
  a documentation gap, and issue #901 requires it fixed while preserving the
  flat form. Both forms need a test.

- Observation: successful listings and help/version write to **stdout**;
  parser failures and no-argument usage text write to **stderr**. Evidence: the
  captured help/parser cases and the existing integration test helper, which
  returns stdout for successful commands and stderr for failures. The
  successful listing byte counts were not captured because the nested fixture
  build failed. Tests must compare both streams and the exit status.

- Observation: bare `cargo-bdd` with no arguments prints command help to
  **stderr** and exits **2**, while `cargo-bdd --help` prints to stdout and
  exits 0. Evidence: baseline capture. Impact: the wrapper must not convert the
  no-argument case into a display request.

- Observation: `#[derive(SelectedSubcommandMerge)]` hard-errors on unit
  variants — "unit variants are not supported; use `Variant(Args)`" — and
  `Commands::{Unused, Duplicates}` are unit variants. Evidence:
  `ortho_config_macros-0.10.0/src/selected_subcommand_merge.rs`. Impact: the
  derive cannot be applied to `Commands` without reshaping two variants. See
  Decision log.

- Observation: a `tracing` callsite's cached `Interest` is process-wide while
  the dispatcher that computes it is per-thread, so
  `tracing::subscriber::with_default` cannot make span capture deterministic in
  a multi-threaded test binary. Evidence: the hosted Windows strict lane failed
  on this branch's own span test at `83a0e488`; locally it failed 1 run in 25
  while passing 80 of 80 in isolation. `DefaultCallsite::register` resolves
  interest through `dispatcher::get_default`, which is the calling thread's
  default falling back to the global one, and `with_default` installs a
  thread-local default without recomputing anything. A sibling test that
  reaches an instrumented function on its own thread therefore caches an
  interest computed against `NoSubscriber`, and whichever loader loses that
  race opens a span no layer sees. Impact: such a test must install its
  subscriber with `set_global_default`, and must then read back only its own
  thread's spans, because the capture now sees the whole binary's spans. An
  in-scope `rebuild_interest_cache` is not a fix — it was measured at 5
  failures in 60 runs — and the process-global `LevelFilter` was `TRACE` in
  every failing run, so the level gate is not involved. The control that
  settles it: thread-local 6 failures in 60 runs, global default 0 in 200.

- Observation: `#[derive(OrthoConfig)]` emits a *separate hidden* CLI struct
  (named from the config struct) rather than adding fields to the user's
  struct. Evidence: `ortho_config_macros-0.10.0/src/derive/generate/structs.rs`,
  `generate_cli_struct`, which emits `struct #cli_ident` with
  `#[derive(clap::Parser, serde::Serialize, Default)]`. Impact: the user-visible
  `StepsArgs` help text cannot be polluted by the generated `--config-path`
  flag; the freeze risk is confined to the fields the derive is asked to
  generate.

- Observation: the first hosted run for PR #912 found a Windows-only
  `dead_code` warning promoted to an error, plus three standalone fixture lock
  files out of alignment with the workspace lock. The test jobs did not start,
  and the run reported no timeout. Impact: gate the Unix-only helper with
  `#[cfg(unix)]` and refresh the affected locks; keep cold-build and offline
  child-Cargo follow-up in issues #910 and #911.

- Observation: CodeRabbit found that the captured `--version` output was pinned
  to `0.6.0`. Replacing the literal with `<CARGO_PKG_VERSION>` made the
  baseline test fail only for that output. Expanding the marker with the test
  package's `CARGO_PKG_VERSION` restored the captured bytes; a separate
  offline, locked `--no-run` build preceded the passing test. Evidence is in
  `/tmp/baseline-version-red-issue-901.out`,
  `/tmp/baseline-version-green-prewarm-issue-901.out`, and
  `/tmp/baseline-version-green-issue-901.out`. Impact: other baseline output
  remains byte-exact while the version case follows the package release.

- Observation: only TOML is discovered with default features.
  Evidence: `ortho_config-0.10.0/src/subcommand/paths.rs` gates the
  `json`/`json5` and `yaml`/`yml` entries of `EXT_GROUPS` behind crate
  features. Impact: the design answer to "format features" is TOML only, and
  enabling `json5` or `yaml` would be an unforced scope increase.

- Observation: `load_config_file` returns `Ok(None)` for a file that does not
  exist. Evidence: `ortho_config-0.10.0/src/file/loader.rs`. Impact:
  "configuration failures are distinct from missing optional files" is
  satisfied by the upstream loader; only an explicitly requested file is
  required to exist.

- Observation: the derive adds no `#[serde(deny_unknown_fields)]`, so unknown
  keys in a configuration file are silently ignored. Evidence: the generated
  structs in `derive/generate/structs.rs` carry no such attribute. Impact: the
  §5 "unknown keys" case must assert the actual non-erroring behaviour, not an
  expected failure. Recording this keeps the test honest.

- Observation: the generated `--config-path` flag is created only on the hidden
  derive CLI struct, and no subcommand loader consults it. Evidence:
  `derive/build/cli_tokens.rs` and the subcommand loaders in
  `ortho_config-0.10.0/src/subcommand/`. Impact: offering `--config-path` on
  `cargo-bdd` requires either passing an explicit file to
  `SubcommandFileContext` or reading the path in application code. Decide and
  record; a hidden flag that silently does nothing is unacceptable.

- Observation: subcommand loading ignores profiles entirely.
  Evidence: the migration guide records the upstream decision (D11); there is no
  `--profile` on the subcommand path. Impact: the design's "named profiles and
  `--profile` coexistence" answer is that `cargo-bdd` has no profile tier, and
  the reason is recorded rather than left implicit.

- Observation: the baseline `make test` target timed out one test at its 180 s
  nextest budget while the compile-admission pool was saturated. Evidence:
  `build-limits-status` at the same moment reported capacity 4, held 4,
  healthy, with a queue of unrelated crates. Impact: a re-run is owed before
  any "baseline green" claim, and later timeouts must be diagnosed against
  `build-limits-status` before being treated as regressions.

- Observation: the users' guide and `crates/cargo-bdd/README.md` document
  flags that do not exist (`unused --quiet`, `duplicates --json`). Evidence:
  `docs/users-guide.md` "## Diagnostic tooling" and the README. Impact:
  pre-existing drift. Correct it in §6 or record why it is deferred; do not
  leave it looking like an artefact of this change.

- Observation: the pre-existing lockfile selected `cargo-platform 0.3.3`,
  whose Rust 1.91 floor made a direct Rust 1.88 check of reusable libraries
  fail before compilation. The dependency is reached through
  `cargo_metadata 0.23.1` and already existed on the base branch. Firecrawl
  confirmed `cargo-platform 0.3.2` has a Rust 1.88 floor; the upstream `^0.3.0`
  requirement accepts it. Impact: pin the compatible transitive version in
  `Cargo.lock` so the libraries remain at their declared MSRV without manifest
  changes. Evidence: initial failure and retry are recorded in
  `/tmp/issue-901-cargo-1.88-libraries-check.out`.

- Observation: a Nextest run that aborts on one failing test leaves later tests
  with no verdict, so a single red gate hides which binaries never ran.
  Evidence: the first runs of this candidate reported `2,196/2,217 tests run`
  with 14 tests not run after `step_macros_compile` timed out. Impact: obtain
  the suite result from a run with `--no-fail-fast`, and give every test an
  explicit verdict before claiming coverage.

- Observation: a killed trybuild run leaves its snapshot deleted.
  `compile_fail_with_normalized_output` removes the `.stderr` file and restores
  it after comparing; killing the run between those points leaves a
  `D crates/rstest-bdd/tests/ui_macros/*.stderr` in the worktree. Evidence:
  stopping an in-flight run produced exactly that deletion. Impact: restore the
  file with `git checkout --` before rebasing, and check for it after any
  interrupted trybuild run.

- Observation: the pre-merge review asked for product-level coverage of
  configured `skipped` settings and for a property test over the merge. Both
  were genuine gaps: the subprocess cases configured `steps.skipped` only, and
  the example-based precedence cases pinned one conflict sequence per field.
  Evidence: `crates/cargo-bdd/tests/invocation/configuration.rs` and
  `crates/cargo-bdd/src/config/tests/property.rs`. Impact: the `skipped`
  handler is now driven through file and environment configuration in a child
  process, and both subcommands' fields are compared against an independent
  precedence model with the other namespace carrying conflicting values.

- Observation: `cargo fmt -p cargo-bdd` on the stable toolchain reformats
  files the change does not own, because `.rustfmt.toml` uses unstable options
  only the pinned nightly honours. Evidence: an unrelated five-file drift after
  one stable format run, reverted with `git checkout --`. Impact: format with
  `cargo +$(FMT_TOOLCHAIN) fmt`, and stage by path so unrelated drift cannot
  reach the commit.

- Observation: the pre-merge review also asked for observability on the new
  configuration path, and the developers' guide described the settings as
  reaching the registry module, which never receives them. Evidence: the
  guide's "cargo-bdd configuration boundary" section against
  `crates/cargo-bdd/src/cli.rs`, where handlers forward only booleans to the
  output module. Impact: both loaders now carry a `cargo_bdd.configuration`
  span recording only the command name, and the guide states the actual
  boundary. The binary installs a stderr subscriber only when `CARGO_BDD_LOG`
  is set, so the default output contract is unchanged.

- Observation: the follow-up review asked for the remaining configuration
  sources to be covered through the handlers, for the parse/resolve path to
  return typed Clap failures instead of exiting, and for the Docstring Coverage
  check to be re-measured on its own touched-function scope. All three were
  valid. Evidence: `crates/cargo-bdd/tests/invocation/configuration.rs` now
  drives `steps.json` and `skipped.reasons` from files and the environment and
  proves file < environment < explicit flag through the application path;
  `crates/cargo-bdd/tests/cli/configuration.rs` covers a file-configured
  `reasons` setting against the scenario-bearing fixture; `CliError` carries
  either a Clap error or an `eyre` report, and `main` renders the Clap variant
  with Clap's own exit path. Impact: the touched-function docstring population
  is now documented, which required splitting the two largest test modules
  (`tests/invocation/workspace.rs` and `src/config/tests/fixture.rs`) to stay
  inside the 400-line budget. The §6 box above stays unchecked until the merge
  actually lands.

- Observation: the documented configuration timing was not observable. The
  subscriber installed by `CARGO_BDD_LOG` had no span-event configuration, and
  `tracing-subscriber`'s default formatter drops span events, so the
  instrumented loaders produced no output at all for the opt-in variable.
  Evidence: `crates/cargo-bdd/src/observability.rs` before this round, against
  the tracing-subscriber 0.3 default `FmtSpanConfig`, whose `trace_close()` is
  false. Impact: the subscriber now requests `FmtSpan::CLOSE`; the close event
  inherits the span's `INFO` metadata, so `CARGO_BDD_LOG=info` renders one
  `cargo_bdd.configuration{command=…}: close time.busy=… time.idle=…` line per
  load. `tests/invocation/observability.rs` pins that output for both labels.

- Observation: `tracing-subscriber`'s formatter emits ANSI escapes
  unconditionally, not only on a terminal, so the first captured close line
  contained escape sequences between the span fields. Evidence: the failing
  assertion output showed `command` + `ESC[0m` + `=` + `ESC[0m` + `"steps"`.
  Impact: `install()` now enables colour only when stderr is a terminal
  (`io::stderr().is_terminal()`), and the observability cases assert that a
  captured log contains no `ESC` byte.

- Observation: the Rust 1.100.0 rollout changed two of this workspace's
  assumptions at once. Besides the new Clippy finding, Cargo 1.100.0 moved test
  executables into its build-dir layout, so a helper that climbed out of the
  executable's `deps` parent found no target root. Evidence: the Windows
  coverage logs for jobs 131673923291 and 131673923282 at `e8fbd8ab` show the
  test binary running from
  `target\llvm-cov-target\debug\build\rstest-bdd\b3732e51d0359c5e\out\`, and
  the same lanes passed at `74f066af` on Cargo 1.99.1 with `deps\` paths.
  Impact: the layout is now a covered contract rather than an assumption; a
  later layout change returns `None` and defers to `cargo metadata` instead of
  guessing.

- Observation: a *redundant* environment redirect can still break an unrelated
  tool. Redirecting `USERPROFILE` alongside `HOME` narrowed the discovery
  search space by nothing — `HOME` is read first, so the home dotfile was
  already covered — yet it removed the only value the Windows Known Folder API
  could resolve a profile from, which made `sccache` panic on every nested
  `cargo` invocation. Evidence: Windows was green at `76f8f63e`, red at
  `1caf9302`, and red from `5d990774` onward, the commit that introduced the
  redirect; `main` was green throughout. Impact: the redirect is removed, and
  `cli_child_environment_keeps_userprofile_inherited` now pins the absence so a
  future "belt and braces" addition fails on Linux before reaching CI. The
  general lesson is recorded in the Decision log: in a *filtered* child
  environment, adding a variable is never a no-op, so a redirect must be
  justified by a search root it actually closes.

- Observation: a green local gate run is not evidence about a Windows-only
  failure, and the pre-existing gate list gives no such evidence. The `sccache`
  panic needed `RUSTC_WRAPPER` to reach a nested process, which only CI
  arranges, so no Linux gate could have caught it. Impact: the only local check
  that now covers this class is the unit test on the environment helper itself,
  which asserts the *shape* of the map rather than the outcome in a child. That
  is deliberate — it is the strongest assertion available without reproducing
  the Windows toolchain.

- Observation: case-insensitivity is a property of the *platform*, not of the
  comparison the filter happens to use. The first draft of the mixed-case
  regression case asserted the variable was dropped on every host, and failed
  here on Linux, where the two names are genuinely different variables. Impact:
  a host-behaviour assertion belongs in a `cfg`-gated case, and the ungated
  test should assert only what holds everywhere. Because a `cfg(windows)` case
  runs nowhere on this repository's Linux lane, its non-vacuity was established
  by forcing the Windows arm in a scratch harness: with the pre-fix exact-case
  filter, the assertion it makes fails, so it is a real witness rather than a
  test that would pass against any implementation.

- Observation: a review finding can be right about the symptom and wrong about
  its premise, and the premise has to be checked rather than inherited. The
  round-16 finding reported no hosted run turning the Linux job green; the
  branch's own re-run had done exactly that. Impact: the status line was still
  stale in a different way (it named an older head), so the fix was to name the
  latest *completed* verdict and explicitly claim nothing about the in-progress
  head. Accepting either the finding or the document as read would have written
  a false statement into the living record.

## Decision log

- Decision: take `ortho_config` v0.10.0 with default features enabled.
  Rationale: v0.10.0 does not build with `default-features = false`
  (leynos/ortho-config#627), and the issue requires the published release.
  Consequently only TOML configuration files are discovered, because the
  `json`/`json5`/`yaml` extensions are feature-gated. That is the intended
  scope, not a limitation to work around. Date/Author: 2026-10-25, implementing
  agent.

- Decision: raise `crates/cargo-bdd/Cargo.toml` to a literal
  `rust-version = "1.89"` and leave the workspace at 1.88. Rationale: only the
  application crate needs the new floor; library consumers keep their 1.88
  guarantee, which the users' guide advertises. A literal value is required
  because `rust-version.workspace = true` would inherit 1.88. Date/Author:
  2026-10-25, implementing agent.

- Decision: retain `cargo-platform 0.3.2` in `Cargo.lock` while keeping
  `cargo_metadata`'s declared caret requirement unchanged. Rationale: the base
  lockfile's 0.3.3 entry requires Rust 1.91 and breaks the user-requested Rust
  1.88 library check; 0.3.2 supports 1.88 and is allowed by upstream's `^0.3.0`
  requirement. The claim holds for builds that resolve through this
  repository's lockfile. A downstream resolver-2 consumer is not constrained by
  it and can still select `cargo-platform` 0.3.3, which requires Rust 1.91, so
  any downstream build relying on the 1.88 floor must make the same resolution
  choice in its own lockfile. This lock resolution keeps reusable libraries at
  their own MSRV in locked builds and adds no OrthoConfig dependency to them.
  Date/Author: 2026-11-02, implementing agent; qualified 2026-11-08 after
  review.

- Decision: use `ortho_config::cargo::external_subcommand` only for argv that
  begins with Cargo's injected `bdd` token; keep the original parser for the
  flat `cargo-bdd <subcommand>` form. Rationale: the upstream helper correctly
  models `cargo bdd <subcommand>` and `cargo-bdd bdd <subcommand>`, but its
  synthetic parent changes the flat form's frozen usage and help text. The
  alternative — an `Option<String>` first positional in `Cli` — cannot
  reproduce Cargo's namespace and would accept a stray `bdd` token in the flat
  form too. Date/Author: 2026-11-01, implementing agent.

- Decision: settings configurable through files and environment are the
  per-subcommand booleans `--skipped`, `--json` (on `steps`) and `--reasons`,
  `--json` (on `skipped`). Everything else is CLI-only. Rationale: these are
  the only settings that exist; they are also the ones a project would
  plausibly want to pin (a CI lane that always wants JSON, a repository that
  always wants reasons). Inventing further settings would be scope the issue
  does not ask for. Date/Author: 2026-10-25, implementing agent.

- Decision: prefix `CARGO_BDD_`, environment spelling of a subcommand setting
  `CARGO_BDD_CMDS_<SUBCOMMAND>_<FIELD>`, file table `[cmds.<subcommand>]`.
  Rationale: this is exactly what the derive produces for
  `#[ortho_config(prefix = "CARGO_BDD_")]` plus a command name of `steps` or
  `skipped`; matching it means no handwritten mapping to keep in step.
  Date/Author: 2026-10-25, implementing agent.

- Decision: precedence is defaults < configuration files < environment <
  command line, with every boolean field marked
  `#[ortho_config(cli_default_as_absent)]`. Rationale: without that marker a
  `bool` field's `false` is a real value and outranks the file and environment
  tiers, so configuration could never enable a flag. With it, only a value the
  user actually typed on the command line participates in the CLI tier. The
  matching `Default` implementation must agree with the clap defaults because
  the subcommand merge starts from `T::default()`. Date/Author: 2026-10-25,
  implementing agent.

- Decision: no configuration profiles and no `--profile` flag.
  Rationale: OrthoConfig v0.10.0's subcommand path ignores profiles by design
  (upstream decision D11). Adding a repository-side notion of profiles would
  mean hand-rolling a tier upstream does not load, and nothing in issue #901 or
  the roadmap asks for it. Date/Author: 2026-10-25, implementing agent.

- Decision: help text stays English-only; no `FluentLocalizer` is installed.
  Rationale: `cargo-bdd` emits no localized strings today, and installing a
  localizer would change the wording of help and parse failures — which the
  freeze constraint forbids. OrthoConfig's `NoOpLocalizer` is the default and
  remains in force. Date/Author: 2026-10-25, implementing agent.

- Decision: do **not** use `#[derive(SelectedSubcommandMerge)]`. Hand-roll the
  per-arm merge with `SubcmdConfigMerge::load_and_merge_with_matches`.
  Rationale: the derive hard-errors on the unit variants `Unused` and
  `Duplicates`, and the only way to satisfy it would be to invent empty payload
  structs for two subcommands that have no settings. Those structs would exist
  solely to please a macro, would appear in `--help` as a change if the derive
  generated their metadata, and would make the "no configurable settings here"
  fact less obvious than it is today. Two arms that call a loader and two arms
  that do not is the honest shape. The derive remains worth using if a later
  subcommand needs settings that the other units do not. Date/Author:
  2026-10-25, implementing agent.

- Decision: do not offer `--config-path`.
  Rationale: the derive emits it only on its own hidden CLI struct, and no
  subcommand loader consults it. Exposing it would mean hand-wiring a second
  discovery path, or shipping a flag that accepts a path and ignores it. The
  documented discovery locations and the environment variable cover the need.
  Date/Author: 2026-10-25, implementing agent.

- Decision: track nested-Cargo timeout, cold-build, and network-isolation work
  separately from issue #901. Rationale: the current failing full-suite test is
  in the reusable `rstest-bdd` library and does not assert cargo-bdd behaviour;
  the user requested issues for the test infrastructure and set the requirement
  that tests must not compile cold dependency graphs or fetch packages over the
  network. Issues [#910](https://github.com/leynos/rstest-bdd/issues/910) and
  [#911](https://github.com/leynos/rstest-bdd/issues/911) record those
  follow-ups. No library test harness changes are included in this branch.
  Date/Author: 2026-11-03, user direction.

- Decision: decline the review's request to expect `cargo-bdd.exe` in the flat
  `steps --json` parse failure on Windows, and do not add a platform branch
  there. Rationale: that message is not rendered from the process's argv.
  `resolve_steps_settings` calls `missing_required_skipped_error`, which
  re-parses a fixed `["cargo-bdd", "steps", "--json"]` vector, so the echoed
  argv[0] is the literal `cargo-bdd` on every platform and the existing
  byte-for-byte expectation is already correct. Measured locally with argv[0]
  spelled `cargo-bdd`, as an absolute `…/cargo-bdd.exe`, and as a Windows-style
  path: all three print `Usage: cargo-bdd steps --skipped --json`. Deferring to
  the finding would fail the assertion on Windows, so the platform-dependent
  spelling stays confined to the real `--help` output, which does read argv[0]
  and which `flat_injected_and_real_cargo_forms_share_ the_same_command_help`
  already branches. Date/Author: 2026-11-13, implementing agent.

- Decision: rewrite the interpolated
  `unwrap_or_else(|| panic!("{variable} …"))` in `cli/environment.rs` as
  `let … else`, superseding an earlier decision to keep it. Rationale: the
  first defence rested on Whitaker's `should_flag`, which exempts a test panic
  that interpolates only variables, plus a mutation control showing that
  exemption is load-bearing. Both observations are true and both are beside the
  point: ADR 013 states outright that "`unwrap_or_else(|| panic!(…))` remains
  rejected", and that `.expect(...)` is what `clippy.toml` permits in tests. A
  lint that happens not to fire is not a licence, and the repository holds no
  live instance of the shape — the only two grep hits are comments explaining
  why it is avoided. `let … else` satisfies the ADR while keeping the
  interpolated message, which a fixed `expect` string would lose: the message
  names which of `CARGO_HOME` and `RUSTUP_HOME` failed. Date/Author:
  2026-11-13, implementing agent; revised after review round 12.

- Decision: add the ambient-mutation row to
  `crates/rstest-bdd/tests/skip_parity/resolution_once.rs`, reversing an
  earlier "not expressible" verdict. Rationale: the round-10 statement of the
  finding mutated the global between runs, which no test can distinguish from a
  correct implementation, and was correctly declined. The round-11 statement is
  narrower and does describe a real gap: the existing in-flight row supplies
  the policy through `with_skip_policy`, so the override is the only input and
  a driver that re-read the global *only on the ambient path* would pass every
  other test in the module. The new row constructs the scope ambiently under an
  armed `false` and lets the handler flip the global to `true` before skipping.
  Evidence it is not redundant, and not vacuous: mutating the driver to re-read
  the global for ambient scopes at record-assembly time fails this row alone (9
  passed, 1 failed) with its own assertion message, while the other nine rows —
  including the explicit-policy in-flight row — pass. The mutation was reverted
  and `git diff` confirms `crates/rstest-bdd/src/` is clean. Date/Author:
  2026-11-13, implementing agent; revised after review round 11.

- Decision: leave `USERPROFILE` inherited in the CLI fixture's child
  environment, and never redirect a variable that does not close a search root
  of its own. Rationale: the redirect was redundant — discovery reads `HOME`
  first on non-Unix and consults `USERPROFILE` only as a fallback — but not
  harmless, because the child environment is built by filtering and then
  `env_clear()`, so any variable absent from the map is absent from every
  nested process. `sccache` resolves its configuration through the Windows
  Known Folder API, which needs a resolvable profile; without one it panics and
  every nested `cargo` in the suite dies. The two other `USERPROFILE` writes in
  the suite (`tests/baseline_output.rs`, `tests/invocation/workspace.rs`) are
  left alone: each clears the environment itself and then *adds* a valid
  `USERPROFILE`, so they set the profile rather than remove it. Alternatives
  considered and declined: keeping the redirect and clearing `RUSTC_WRAPPER` in
  the child (hides the tool the gate is meant to exercise, and would not fix
  other profile-dependent children), and redirecting `USERPROFILE` but also
  setting `SCCACHE_CONF` (works, but adds a second unexplained special case to
  buy back a redirect that closes nothing). Evidence: Windows green at
  `76f8f63e`, red from `5d990774` onward, green on `main`; local guard test
  fails on the reintroduced redirect and passes without it. Date/Author:
  2026-11-14, implementing agent; from review round 13's CI evidence.

## Outcomes & retrospective

Delivered as specified. `cargo-bdd` resolves settings from `.cargo_bdd.toml`,
the XDG `cargo_bdd/config.toml`, environment variables and command-line flags
in that precedence order through `OrthoConfig`, and both invocation forms
dispatch to the same behaviour. ADR 025 records the decision, and the
developers' guide points at it.

Gate evidence at revision `3e94e1b6`, the rebased candidate this section
describes:

- `make check-fmt`, `make lint` and `make typecheck` each exited zero, run in
  that order on the rebased tree.
- `make test` did not produce a single green invocation. Two runs of its
  Nextest step passed every test that reached a verdict and lost one test to
  that test's own wall-clock bound while the shared compiler pool was
  saturated. Run one reported 2,220 passed, seven skipped, one timed out and 14
  cancelled; run two reported 2,150 passed, seven skipped, one timed out and 91
  not run. The timeouts are different tests, and each test that timed out in
  one run passed in the other apart from `step_macros_compile`.
- The remaining `make test` steps were run directly at this revision and
  exited zero: `cargo test --doc --workspace --all-features` (seven doc tests),
  `make check-fixture-lockfiles` (five fixture lockfiles up to date) and
  `uv run pytest scripts/tests` (291 passed in 3,729.53 seconds).
- The no-default-features leg was run twice. The second run used
  `--no-fail-fast`, so all 784 tests received a verdict: 780 passed, seven
  skipped, two failed and two timed out. Both failures report
  `nested cargo exceeded its 300 s wall-clock bound; the child was killed`
  after repeated `Blocking waiting for file lock on package cache` lines, and
  both tests pass in the all-features run at this same revision, in 2.56 and
  1.91 seconds.
- The 14 tests cancelled in run one were re-run focused and passed: 37 passed
  with two skipped in the `trybuild_macros` binary.

The retained exception is two tests, both in `rstest-bdd::trybuild_macros`.
`runner::step_macros_compile` compiles 56 fixture crates and did not finish
inside its 1,200-second bound in any run; a focused re-run compiled 13 fixtures
cleanly before reaching the same bound.
`whitaker::whitaker_lint_gate_accepts_clean_and_rejects_panicking_fixtures`
shells out to `make lint-whitaker` and timed out on that bound in the
no-default-features run, and was cancelled before starting in both all-features
runs. Admission reported capacity four, healthy, with more than 200 compiler
requests queued from other repositories on this host across these runs. The
budgets in `.config/nextest.toml` are sized for a quiet runner, and raising
them changes the project's hang detection rather than this branch's behaviour.
Cold nested compilation and child-Cargo network isolation remain tracked by
issues #910 and #911.

The complete test result for this revision is therefore the hosted runs rather
than a local one. Formatting, lint, type checking, the doc tests, the
fixture-lockfile gate and the Python suite have local green verdicts.

That exception is now cleared, on a later tree. The whole `make test` recipe
was run once, sequentially, at `8a5cb67f`, with no test excluded and no `-E`
filter, and every step finished with exit status zero. The workspace leg
reported 2,251 tests run, 2,251 passed, 7 skipped, and the no-default-features
leg 787 passed, 7 skipped. Both legs ran
`rstest-bdd::trybuild_macros::runner::step_macros_compile` to completion (275.0
s and 279.5 s), which no earlier local run here had managed.

```plaintext
Summary [ 373.014s] 2251 tests run: 2251 passed, 7 skipped
Summary [ 289.014s]  787 tests run:  787 passed, 7 skipped
PASS [ 275.026s] (2227/2251) rstest-bdd::trybuild_macros runner::step_macros_compile
PASS [ 279.498s] (770/787) rstest-bdd::trybuild_macros runner::step_macros_compile
5 fixture lockfile(s) up to date
291 passed in 20.03s
MAKE_TEST_EXIT=0
```

No `FAILED` line, no `^error`, and no `make: ***` appears in the log. `#910`
still owns the budgets that make those nested-Cargo tests slow on this host,
but it no longer gates this row. This plan cites the gated revision rather than
its own commit, and review follow-ups after that revision are recorded in the
progress log and surprises below.

At `e28a927a` the hosted Linux coverage lane produced the complete result the
local runs could not: 2,245 tests run, 2,245 passed, 7 skipped, including
`runner::step_macros_compile` (631.5 s) and
`whitaker::whitaker_lint_gate_accepts_clean_and_rejects_panicking_fixtures`
(0.9 s), and both Windows lanes passed at that head. The hosted result covers
`runner::step_macros_compile`, the one test no local run here has finished, but
it does not cover the recipe's `--no-default-features` leg, which hosted lanes
do not run.

That leg was therefore run on the final tree, `cf7f7945`, on 2026-11-13:

```sh
cargo nextest run -p rstest-bdd --all-targets --no-default-features \
  --no-fail-fast -E 'not test(~step_macros_compile)'
```

With the recipe's `RUSTFLAGS` it exited zero after 754.3 s with 786 tests
passed and 8 skipped. The Whitaker lint-gate test in the same binary passed in
266.1 s, which the earlier no-default-features runs had timed out. An
unfiltered attempt at the same head was stopped by its invoking command's own
time limit while `runner::step_macros_compile` was still compiling, 1,136 s
into the test and short of its 1,200-second bound; that test is excluded from
the filtered run rather than reported as a failure.

The exception was therefore narrowed to two things: the single-invocation form
of the `make test` row, and a local verdict for `runner::step_macros_compile`.
Both are covered by the hosted run above.

That exception is superseded by the later record in `Outcomes & retrospective`:
the whole recipe was run once at `8a5cb67f` with no filter and exited zero, so
its two conditions are now met locally. Issue #910 still owns the budgets that
make the nested-Cargo tests slow here, but it no longer gates the row.

## Context and orientation

`crates/cargo-bdd` is a **binary-only** Cargo crate: it has `src/main.rs` and no
`src/lib.rs`, so nothing in it is a public API and there is no library
consumer to keep compatible. Its modules are `cli.rs` (argument parsing and
subcommand handlers), `output.rs` (rendering and snapshots), and `registry/`
(finding and parsing step registries). `main.rs` is three lines:

```rust
//! Command line diagnostic tooling for rstest-bdd.

mod cli;

fn main() -> eyre::Result<()> { cli::run() }
```

`src/cli.rs` defines the whole command surface. `Cli` derives `clap::Parser`
with `#[command(author, version, about)]` and holds one `Commands` value.
`Commands` derives `clap::Subcommand` with four variants: `Steps(StepsArgs)`,
`Unused`, `Duplicates`, `Skipped(SkippedArgs)`. `StepsArgs` has `--skipped` and
`--json` where `--json` carries `#[arg(long, requires = "skipped")]`;
`SkippedArgs` has `--reasons` and `--json`. Both derive `clap::Args` only — no
`CommandFactory`, and no explicit command name.

The integration tests live in `crates/cargo-bdd/tests/cli.rs`. They spawn the
built binary through `rstest-bdd-harness`'s `nested_cargo` helper, which calls
`env_clear()` and then sets only the variables it is given. Any test that needs
`CARGO_BDD_*` or `XDG_CONFIG_*` visible to the child must therefore pass those
variables into `env_from_vars` explicitly; setting them on the test process
will not reach the child. This is also why process-environment mutation is
forbidden here: the child already receives a clean, explicit environment.

Terms used in this plan:

- **OrthoConfig** — the `ortho_config` crate, which layers configuration from
  defaults, files, environment, and command line, and exposes the result as a
  typed struct.
- **Precedence tier** — one source in that layering. A higher tier wins.
- **`cli_default_as_absent`** — an `ortho_config` field attribute meaning "a
  value that came from a clap default was not supplied by the user", so the
  command-line tier stays empty and lower tiers can still win.
- **External subcommand** — the Cargo convention where `cargo foo` runs the
  `cargo-foo` binary and passes it `foo` as its first argument.
- **`cargo-bdd` flat form** — `cargo-bdd steps`, what the binary sees when
  invoked directly.
- **`cargo-bdd` Cargo form** — `cargo bdd steps`, which reaches the binary as
  `cargo-bdd bdd steps`.

Baseline commit and identity: `2ee10a136b3ae641f3feab295d4b2f48aa30ec76`, built
with the workspace `rust-toolchain.toml` stable toolchain.

Baseline parser/help behaviour captured verbatim from the built binary. The
successful listing commands could not be captured in the baseline probe because
their nested fixture build failed; the existing `tests/cli.rs` assertions
remain the behavioural baseline for those commands.

*Table 1: Captured baseline exit status and stream sizes for help and parser
invocations.*

| Invocation       | Exit | stdout | stderr |
| ---------------- | ---- | ------ | ------ |
| `--help`         | 0    | 446 B  | 0 B    |
| `--version`      | 0    | 16 B   | 0 B    |
| `help`           | 0    | 446 B  | 0 B    |
| `help steps`     | 0    | 238 B  | 0 B    |
| `steps --json`   | 2    | 0 B    | 149 B  |
| `bogus-cmd`      | 2    | 0 B    | 108 B  |
| `--nope`         | 2    | 0 B    | 107 B  |
| *(no arguments)* | 2    | 0 B    | 446 B  |
| `bdd steps`      | 2    | 0 B    | 102 B  |

Row `bdd steps` is pre-change evidence for `V3`, not a `V1` case. Reproducing
it would require the post-change binary to keep rejecting the invocation that
`V3` requires to succeed, so the two obligations would contradict each other;
the row records that the shipped binary rejected Cargo's own dispatch form
before this work. Every other row is a `V1` no-configuration case, and the
checked-in baseline fixture
(`crates/cargo-bdd/tests/fixtures/baseline-cli.json`) holds exactly those seven
and omits `bdd steps`.

The exact captured text is in `/tmp/baseline-cli-errors-issue-901.out`. This
scratch record is not durable; §5 must add checked-in assertions for the stable
help and parser-failure cases. The failed successful-listing probe wrote 0
stdout bytes because the nested Cargo build failed before the CLI could list
the fixture; that result says nothing about successful output.

Existing callers, none of which invoke the binary as `cargo bdd`: the
`Makefile`, `.github/workflows/mutation-testing.yml`, `.github/dependabot.yml`,
`scripts/fixture_lockfile_cargo.py`, and the test modules
`scripts/tests/test_fixture_lockfile_discovery.py`,
`scripts/tests/test_fixture_lockfile_alignment.py`, and
`tests/workflow_contracts/mutation_testing_test.py`. The workflow references
are to the `cargo-bdd` **package**, not to its command line, so no caller
depends on the flat form's flags.

## Conformance basis

Upstream artefacts:

- Issue #901, "Adopt the published `ortho_config` v0.10.0 release for the
  `cargo-bdd` command-line interface".
- `docs/ortho-config-users-guide.md` and
  `docs/ortho-config-v0-10-0-migration-guide.md`, imported verbatim at
  `leynos/ortho-config` commit `38812c563e3953a490e3efc202609f37e21a0517` and
  recorded in `docs/ortho-config-v0-10-0-provenance.md`.
- [design §3.6a](../rstest-bdd-design.md#36a-cargo-bdd-configuration-boundary-adr-025)
  and [ADR 025](../adr-025-adopt-ortho-config-for-the-cargo-bdd-cli.md), both
  authored by this plan.
- `docs/roadmap.md` §6.1 "Diagnostic tooling".

No pre-existing Terms of Reference or technical-design identifiers cover the
`cargo-bdd` configuration surface; the design document gains that section as
part of §3 rather than inheriting one. Trace links:

```plaintext
issue-901 -> EP-M1 (baseline freeze)   -> tests/cli.rs unchanged, baseline fixture
issue-901 -> EP-M2 (configuration)     -> src/config/tests.rs per-subcommand tiers
issue-901 -> EP-M3 (cargo entry)       -> tests/invocation.rs both forms
issue-901 -> EP-M4 (documentation)     -> users' guide, developers' guide, ADR 025
```

## Verification plan

The change introduces five obligations worth stating.

- Obligation: `V1 no-configuration-equivalence`. For the captured help and
  parser cases in Table 1, the post-change binary produces the same exit
  status, stdout bytes and stderr bytes as the baseline when no configuration
  is present. The `bdd steps` row is excluded: it is pre-change evidence for
  `V3`, which requires that form to succeed afterwards, so pinning its exit 2
  would make the two obligations contradictory. Method: checked-in byte
  assertions generated from the captured pre-change binary output. Successful
  listing behaviour is covered by the existing integration tests, which must
  remain unmodified, and by source snapshots for output formatting; the
  baseline probe could not produce a successful fixture listing. Artefact:
  focused tests in `crates/cargo-bdd/tests/`. Discharge condition: all
  applicable cases pass in the full test run.

- Obligation: `V2 precedence`. For each configurable field of each subcommand,
  a value set at a higher tier wins, and a value absent from every tier yields
  the current default. Method: per-subcommand tests with injected sources
  (`MapEnv`, explicit file sources, `SubcommandFileContext::new`), one row per
  `(subcommand, field, tier-that-wins)` combination, plus the absent-everywhere
  row. Rationale: the tier structure is a small finite product; explicit cases
  are clearer than generation and cover the whole domain. Domain: 2 subcommands
  x 2 fields x 4 tiers, plus 2 "absent" rows. Artefact:
  `crates/cargo-bdd/src/config/tests.rs`. Evidence: a row fails if the merge
  order is wrong, if `cli_default_as_absent` is missing (the "environment wins
  over an untyped flag" row fails), or if `Default` disagrees with the clap
  default (the "absent" row fails). Non-vacuity: the file-injected and
  environment-injected rows set genuinely different values, so a passing test
  cannot be explained by every tier agreeing. Negative control: dropping
  `cli_default_as_absent` from one field must fail exactly the rows for that
  field. Residual gap: this exercises the loader against injected sources, not
  OrthoConfig's internal discovery. Discovery itself is third-party and treated
  as an axiom, with the caveat recorded under `V5`.

- Obligation: `V3 invocation-forms`. The flat form `cargo-bdd steps`, the
  injected form `cargo-bdd bdd steps`, and real `cargo bdd steps` dispatch to
  `steps`; help and parse-error streams are also checked. Method: isolated
  child processes exercise help, handler dispatch in a fixture workspace, and
  an unknown inner command. Rationale: the defect is a property of argument
  shape, which only a real process exercises. Artefact:
  `crates/cargo-bdd/tests/invocation.rs`. Evidence: the pre-change binary
  rejected the injected form with exit 2 and
  `error: unrecognized subcommand 'bdd'`; the post-change forms reach the same
  handler and output. Discharge condition: green, and the flat-form rows still
  match Table 1. Non-vacuity: the pre-change failure is recorded, so the test
  demonstrably distinguishes the two implementations. Negative control:
  invoking with `cargo-bdd bdd bogus` must still exit 2.

- Obligation: `V4 failure-surface`. Malformed and unreadable configuration
  files, invalid values and unknown keys produce the documented boundary
  behaviour, and a missing optional file is not reported as an error. Method:
  source-injected tests in `src/config/tests.rs` and binary-level tests in
  `tests/invocation.rs`. Domain: malformed TOML; an unreadable file;
  `skipped = "maybe"` (type error); an unknown key; and `steps.json = true`
  with resolved `steps.skipped = false`. The latter must fail at the
  application boundary with an actionable error that identifies the
  configuration value and requires `steps.skipped = true` or `--skipped`; the
  existing CLI-only `steps --json` parse error remains unchanged. The
  unreadable-file fixture is platform-specific: on Unix, set mode `0o000` and
  exercise the read failure from a non-root test process; if the process can
  still open the file, report that case as unavailable rather than passing it.
  On Windows, hold a config file open with `OpenOptionsExt::share_mode(0)` so
  OrthoConfig's file probe cannot open it. Evidence: failure rows assert a
  non-zero result and actionable context; the missing-file row asserts no
  configuration error. Discharge condition: all cases applicable to the
  platform pass. Note: unknown keys are **ignored**, because the derive emits no
  `deny_unknown_fields`; asserting an error would test behaviour OrthoConfig
  does not have.

- Obligation: `V5 discovery axiom`. The claim "with no configuration file
  present, no file tier contributes" depends on OrthoConfig's discovery rules
  (home `.cargo_bdd.toml`, the first existing XDG configuration file, then a
  local `.cargo_bdd.toml` on Unix and Redox; platform-native locations
  elsewhere; TOML only with default features). Method: treated as an axiom
  about a third-party interface, with one contract-level check: run the child
  from an empty temporary working directory, assert that it contains no local
  `.cargo_bdd.toml`, and point `HOME`, `XDG_CONFIG_HOME`, `XDG_CONFIG_DIRS`, and
  `APPDATA` at empty temporary directories. `USERPROFILE` is left inherited:
  `HOME` closes the home dotfile, and removing `USERPROFILE` from a filtered
  child breaks `sccache` on Windows (see the Progress entry for 2026-11-14).
  Remove the four `CARGO_BDD_CMDS_*` setting variables from the child
  environment. This confirms that none of the discovery roots settable from the
  environment, nor the environment tier, contributes a value. It does **not**
  establish full isolation: on Windows the native configuration directory is
  obtained through the Windows API rather than from a variable, so `APPDATA`
  does not redirect it and the check cannot speak for that root. The Unix roots
  — the home dotfile, both XDG bases including the `/etc/xdg` fallback — are
  all covered. Rationale: verifying OrthoConfig's internals is out of scope,
  but the repository's hermeticity depends on this boundary, so it is exercised
  once rather than assumed. Residual gap: a developer with a real
  `~/.cargo_bdd.toml` will see it affect local runs. The users' guide must say
  so.

No non-trivial lemma or formal proof is introduced: the change selects and
merges values from a finite set of sources, and the properties above are
decidable by enumeration. A property test or model check would restate the
table. This conclusion is recorded rather than omitting the section.

## Plan of work

Stage A — design record (no behaviour change). Write ADR 025 and the design
document section; add them to `docs/contents.md`; add the roadmap note. Settle
the two open decisions in the Decision log. Validate with `make markdownlint`,
`make nixie`, and `make check-fmt`.

Stage B — test-first evidence. Add the baseline fixture and
`tests/baseline_output.rs`, and confirm it passes against the current binary
before changing production code. Capture the Cargo-form rejection and add
source-based precedence and failure-surface tests. Keep any intentionally
failing intermediate state uncommitted: repository instructions require every
commit to pass its gates.

Stage C — implementation. Manifest and MSRV; derives; wrapper; boundary error
handling. Validate with the focused tests, then `make lint` and
`make check-fmt`.

Stage D — documentation and wider validation. Users' guide and developers'
guide; correct the pre-existing flag drift; run the full gate set.

Go/no-go: do not leave Stage B until the baseline freeze and pre-change Cargo
form rejection are recorded; do not enter Stage D until Stage C's focused tests
are green. Commit only after the complete candidate passes its required gates.

## Milestones and plateaus

- `EP-M1` baseline freeze (end state: a fixture and test that prove the
  no-configuration CLI contract against the pre-change binary). Acceptance:
  `V1` green. Deliver the fixture with the validated implementation so no
  intentionally failing commit is required.
- `EP-M2` configuration (end state: per-subcommand file and environment tiers
  work for all four booleans, with the empty-origin behaviour unchanged).
  Acceptance: `V2` and `V4` green, `V1` still green. Recovery: drop the derives
  and return to `clap::Args`; `EP-M1` keeps the freeze honest.
- `EP-M3` Cargo entry (end state: both invocation forms work). Acceptance: `V3`
  green, `V1` flat rows unchanged. Recovery: revert the wrapper; the flat form
  is unaffected.
- `EP-M4` documentation and delivery (end state: users' guide, developers'
  guide, ADR, roadmap and `contents.md` agree with the implementation; draft PR
  opened). Acceptance: every gate green, PR body complete.

Each milestone is a coherent work state. Commits follow the repository rule
that the candidate must pass its gates; intermediate test-first evidence stays
in the plan and captured logs until then.

## Concrete steps

1. Write `docs/adr-025-adopt-ortho-config-for-the-cargo-bdd-cli.md` following
   the house style of ADR 023: status, date, context, drivers, options with
   `Accepted.`/`Rejected.`, decision outcome, known risks, architectural
   rationale.
2. Add the configuration section to `docs/rstest-bdd-design.md` after §3.6
   ("Workspace layout decisions"), covering: configurable versus CLI-only
   settings; file name, discovery locations, and TOML-only formats; the
   `CARGO_BDD_` prefix and the absence of legacy variables or files; precedence
   with `cli_default_as_absent`; per-subcommand namespaces; English help and
   parse failures; no profiles; and the Cargo entry shape.
3. Add the ADR 025 bullet and the `[adr-025]` reference definition to
   `docs/contents.md`.
4. Add the decision to `docs/roadmap.md` §6.1 as a new unchecked sub-item under
   the diagnostic tooling heading, naming OrthoConfig v0.10.0. Mark it checked
   only after the implementation is complete.
5. Add the baseline fixture and `tests/baseline_output.rs`; run it.
6. Add `tests/invocation.rs`; record the pre-change rejection in the plan and
   captured baseline log, then prove both forms after implementation.
7. In `Cargo.toml` (workspace) add `ortho_config = "0.10.0"` under
   `[workspace.dependencies]`; in `crates/cargo-bdd/Cargo.toml` add
   `ortho_config.workspace = true` and set `rust-version = "1.89"`.
8. In `crates/cargo-bdd/src/cli.rs`, derive `OrthoConfig` on the argument
   structs, give each a `#[command(name = "…")]`, mark the booleans with
   `#[ortho_config(cli_default_as_absent)]`, and implement `Default` to agree
   with the clap defaults.
9. Select the existing parser for the flat form and the Cargo
   external-subcommand wrapper for argv beginning with `bdd`; route both forms
   through one dispatch function.
10. Handle display requests and configuration errors at the boundary, keeping
    clap's exit statuses and streams.
11. Add source-injected precedence and failure tests in
    `src/config/tests.rs`, plus binary-level invocation and failure tests in
    `tests/invocation.rs`.
12. Update `docs/users-guide.md` (configuration file, environment variables,
    precedence, the MSRV split) and `docs/developers-guide.md` (the integration
    boundary and why libraries stay free of OrthoConfig).
13. Run every gate; commit atomically; push; open the draft PR.

## Validation and acceptance

Acceptance means all of the following, in one gate run against the final tree:

- `make check-fmt`, `make lint`, and `make test` succeed.
- `make markdownlint`, `make spelling`, and `make nixie` succeed.
- The pre-existing tests in `crates/cargo-bdd/tests/cli.rs` pass with no
  edit to their bodies. The file itself did change: an earlier review round
  added the environment-tier `reasons` test and an extra-environment parameter
  to the shared helpers. Check the surviving tests, not the file, for unchanged
  assertions.
- Every `V1` row of Table 1 is reproduced, that is every row except `bdd steps`,
  which is pre-change evidence for `V3` and must succeed after the change.
- `V2`, `V3`, and `V4` are green, and the pre-change Cargo-form rejection is
  recorded in this plan and `/tmp/baseline-cli-errors-issue-901.out`.
- No crate other than `cargo-bdd` appears in the `ortho_config` dependency
  graph. For each library package, first save
  `cargo tree -p <package> -e normal` to a temporary file and check its status;
  then run this explicit assertion against that file:

  ```sh
  if rg -q 'ortho_config v' /tmp/<package>-dependency-tree.out; then
      printf 'unexpected ortho_config dependency in <package>\n' >&2
      exit 1
  fi
  ```

  Repeat for every library package. The `cargo tree` command runs separately so
  a tree-generation failure cannot be mistaken for an absent dependency.
- `cargo metadata` reports `rust_version` 1.88 for every library package and
  1.89 for `cargo-bdd`. The workspace root is a virtual manifest and has no
  package-level `rust_version`.
- Rust 1.88.0 checks the library packages without `ortho_config`; Rust 1.89.0
  checks `cargo-bdd` with OrthoConfig. These commands test the package-level
  MSRV split directly.

The `make test` row asks for one gate run against the final tree. It is met
locally at `f7fa8cd6`: the whole recipe ran unfiltered on a clean tree,
`REV_START` and `REV_END` both `f7fa8cd66b820ea9be1beab079d055e16d329652`, and
exited zero with 2258 of 2258 nextest tests passing (7 skipped), 788 of 788 doc
tests passing (7 skipped), and 292 pytest tests passing, with no `FAILED` line
and no `make: ***` in the log.

That run was taken because the commit before it changed production-adjacent
test code, which is a gate input; the earlier verdict at `404c8861` is kept in
the history below rather than reused. If a later commit changes a file a gate
reads, the run has to be re-taken again.

The earlier run at `8a5cb67f` is *historical* by comparison:
`git rev-list --count 8a5cb67f..41f50191` reports fifteen commits between it
and this text. Every remaining statement in this subsection is older evidence
still, accumulated before that run; where it says the row is unmet or an
exception is retained, the later record in `Outcomes & retrospective`
supersedes it.

- Hosted run `43386083081` on `e28a927a` passed all three `build-test` jobs.
  The Linux coverage lane ran the full workspace suite with `--all-features` —
  2,245 tests run, 2,245 passed, 7 skipped — including both nested-Cargo tests
  (`runner::step_macros_compile` in 631.5 s and
  `whitaker::whitaker_lint_gate_accepts_clean_and_rejects_panicking_fixtures`
  in 0.9 s). Both Windows lanes passed at the same head.
- The recipe's `--no-default-features` leg, which no hosted lane runs, was
  executed at `cf7f7945` with the recipe's `RUSTFLAGS`:

  ```sh
  cargo nextest run -p rstest-bdd --all-targets --no-default-features \
    --no-fail-fast -E 'not test(~step_macros_compile)'
  ```

  It exited zero after 754.3 s with 786 tests passed and 8 skipped, and the
  Whitaker lint-gate test in the same binary passed in 266.1 s. The filter
  excludes exactly one test, `runner::step_macros_compile`, whose 56-crate cold
  compile has never finished inside its budget on this host.
- The doc tests, `make check-fixture-lockfiles` and
  `uv run pytest scripts/tests` cover the recipe's other steps. On the same
  tree, `make check-fmt`, `make markdownlint`, `make spelling`, `make nixie`,
  `make lint` (all nine steps) and `make typecheck` exited zero.

That exception has since been cleared by running the whole recipe once at
`8a5cb67f`, sequentially, with no `-E` filter and no test excluded. It exited
zero. Both nextest legs ran `runner::step_macros_compile` to completion (275.0
s and 279.5 s) and the recipe's remaining steps finished behind them.
[#910](https://github.com/leynos/rstest-bdd/issues/910) still owns the budgets
that make those nested-Cargo tests slow on this host, but the row is no longer
blocked on it. This is recorded rather than waived.

There is no dedicated MSRV CI job in this repository. Report the direct local
MSRV checks above and do not imply a pipeline covers them.

## Idempotence and recovery

Every step is re-runnable. Gate outputs are captured with `tee` to
`/tmp/$ACTION-$(get-project)-$(git branch --show-current).out`; note that these
names are branch-scoped and collide across re-runs and subagents, so establish
a log's revision by reading it, not by its name or mtime.

If a gate fails, read the cited log rather than re-running it, and re-run only
after a fix. If `make lint` aborts, remember it is a nine-step recipe that
stops at the first failure: a red run leaves the steps behind the abort with
**no** verdict, so read the recipe rather than the log tail.

Rollback is per milestone (see `Milestones and plateaus`). Because `EP-M1`
records the freeze before any production change, reverting an implementation
change does not cost the evidence that the no-configuration contract held.

## Artefacts and notes

Expected gate logs, with `$ACTION` one of `check-fmt`, `markdownlint`,
`spelling`, `nixie`, `lint`, `test`:

```plaintext
/tmp/$ACTION-$(get-project)-$(git branch --show-current).out
```

Durable evidence (checked in, unlike `/tmp`):

- `crates/cargo-bdd/tests/fixtures/baseline-cli.json` — the captured bytes per
  invocation.
- `/tmp/baseline-cli-errors-issue-901.out` — the observed pre-change Cargo
  form rejection, also summarized in this plan.

## Interfaces and dependencies

New dependency: `ortho_config = "0.10.0"` at workspace level, consumed only by
`crates/cargo-bdd`. Transitive additions follow the crate's own graph; the
lockfile change is expected and belongs in the same commit as the manifest
change.

Repository-owned interface introduced, inside `crates/cargo-bdd` only:

- An argument struct per subcommand that is both a clap `Parser` and an
  `OrthoConfig`, defaults in agreement.
- One dispatch entry point that accepts both invocation forms and returns a
  `Result` for `main` to terminate on, so that the process exit point stays in
  `main.rs`.

No public interface of any library crate changes. `crates/cargo-bdd` remains
binary-only, so there is no source-compatibility surface to preserve and no
compatibility shim is warranted at any milestone.

## Revision note

Hosted run
[#41766284928](https://github.com/leynos/rstest-bdd/actions/runs/41766284928) on
`e46dd3e9` found two portability/documentation issues. Linux `make check-fmt`
reported that the revised ExecPlan needed Markdown table/paragraph formatting.
Both Windows coverage jobs reached `baseline_output` and found that `--version`
prints `cargo-bdd` without `.exe`, while the fixture had substituted the
Windows executable basename for every output. These were assertion/formatting
failures, not test timeouts. The fixture now keeps the version banner basename
fixed and substitutes the platform executable name only for clap usage output.

Before this hosted run, the correct PR worktree passed formatting, lint,
typecheck, fixture-lockfile, Markdown, Mermaid, file-length, and diff checks.
The focused baseline test passed after an offline, locked prewarm, and
CodeRabbit reported no findings. The workspace typecheck took 58m59s under
shared compiler admission; the focused test prewarm took 18m21s. A review of
v0.10.0's published source also found that `PostMergeHook` is not invoked by
the subcommand merge helpers; upstream issue
[#652](https://github.com/leynos/ortho-config/issues/652) records that API gap.
`make fmt` has corrected the Markdown formatting, and the version-banner
fixture now preserves `cargo-bdd` without a Windows extension. The remaining
CodeRabbit review found an ambiguity about overriding file values when both
`steps` settings are true; the users' guide now shows setting both environment
values to `false` for one non-JSON run. Its next review found that the
developers' guide names `cargo-bdd::invocation` in the 180 s `cargo-spawning`
budget while nextest omitted that test binary. The filter and its regression
contract now cover both `cargo-bdd` test binaries. The final local candidate
passed `make check-fmt`, offline lint and typecheck, offline Markdown lint (137
files), Mermaid, file-length and diff checks. Offline locked prewarms prepared
the `nextest_config` and `baseline_output` targets; the focused tests passed
4/4 and 1/1, with no compilation or package downloads during test execution.
The final CodeRabbit review reported zero findings. Hosted run
[#41777155784](https://github.com/leynos/rstest-bdd/actions/runs/41777155784) on
`71400866` passed Linux, but both Windows coverage jobs failed
`baseline_output`: Clap displayed `cargo-bdd` in usage text while the fixture
substituted `cargo-bdd.exe`. These were fast assertion failures, not timeouts,
so they do not add a timeout issue beyond #910 and #911. The fixture now uses
Clap's displayed basename on every platform. The CodeScene check on `eecb891b`
still flagged duplicated exact-filter lookup in the `cargo-bdd` and
feature-rebuild timeout tests. This candidate combines those cases in one
`rstest` function and shares exact-filter selection. The test-local timeout
expectation types stay in this integration test and do not become library
configuration. Local `cs delta` now reports no findings. The full deterministic
gate sequence passed on this working-tree candidate. The focused tests passed
4/4 and 1/1 after offline, locked prewarms, with no compile or package-download
output during test execution. CodeRabbit reported zero findings. These results
included working-tree changes after `eecb891b`; those changes were subsequently
committed and pushed. They are historical evidence and are superseded by the
current-head gate and hosted validation recorded above.

Hosted run
[#41782484279](https://github.com/leynos/rstest-bdd/actions/runs/41782484279) on
`eecb891b` exposed a second Windows-only baseline difference: root help uses
`cargo-bdd.exe`, while subcommand help and parse errors use `cargo-bdd`. The
fixture had expected the extensionless name for both. Both Windows jobs failed
quickly on this assertion; neither failure was a timeout. The test now uses
separate root and subcommand placeholders, and the fixture assigns them to the
observed output forms. The merged-configuration validation ergonomics gap
reported in this revision remains tracked upstream as
[`ortho-config#652`](https://github.com/leynos/ortho-config/issues/652). The
current pushed candidate is `7bacb672`; its hosted run is being superseded by
this baseline correction. The corrected fixture passed `make check-fmt`,
offline lint and typecheck, offline Markdown lint, Mermaid, file-length, and
diff checks. Offline, locked prewarms built both focused test executables;
direct execution then passed `nextest_config` 4/4 and `baseline_output` 1/1
with no compiler, download, or build-limit output during execution. Cargo-based
test invocations emitted build-limit compiler-slot requests, so their output is
not used as the no-compilation evidence. CodeRabbit reviewed the corrected
source and plan files with zero findings. This evidence was on the dirty tree
based at `7bacb672`; that candidate was subsequently committed and pushed.

Hosted run
[#41789683010](https://github.com/leynos/rstest-bdd/actions/runs/41789683010) on
`7a807fb2` failed the CodeScene advisory gate because
`cli_without_configuration_matches_the_captured_baseline` was a Large Method.
The test now delegates fixture setup, process execution, output rendering, and
assertions to small private helpers. A repository sweep found the related
`IsolatedWorkspace` in `tests/invocation.rs`; it also copies the binary and
supports Cargo dispatch, so the baseline fixture remains deliberately local.
The helper scope and reuse policy are recorded in `docs/testing-strategy.md`.
The first lint run caught `expect_used` in helpers; they now propagate
`io::Result`, and the test returns `Result` without lint suppressions. The
corrected dirty tree passed the full sequential deterministic gate sequence.
Locked offline prewarms built the two focused test executables; direct
execution passed `nextest_config` 4/4 and `baseline_output` 1/1, with no
compiler, download, or build-limit output during execution. Local `cs delta`
against the PR base reports no issues, and CodeRabbit reports zero findings.
Commit and push these changes, then validate the resulting hosted CI run and
update the draft PR summary.

Inspection of run #41789683010's Windows logs showed a second output context:
`cargo-bdd help steps` uses `cargo-bdd.exe`, while the `steps --json` parse
failure uses `cargo-bdd`. Both Windows jobs failed quickly on the former
fixture mismatch; neither was a timeout. The fixture now assigns the root-name
marker to `help steps` and retains the extensionless subcommand marker for the
parse failure. Linux passed on that head. The latest correction was uncommitted
at the time of this entry and was subsequently committed and pushed. It passed
the full sequential deterministic gate sequence; locked offline prewarms
followed by direct test-binary execution passed `nextest_config` 4/4 and
`baseline_output` 1/1, with no compiler, download, or build-limit output during
execution. Local `cs delta` against the PR base reports no issues. After the
requested cooldown, CodeRabbit reviewed this final tree with zero findings. The
correction was subsequently committed and pushed; later hosted runs found the
Windows invocation-name issue recorded below.

The correction was committed and pushed as `5a6acf35`. Hosted CI run
[#41801681477](https://github.com/leynos/rstest-bdd/actions/runs/41801681477)
then exposed a Windows-only usage-name mismatch in
`flat_injected_and_real_cargo_forms_share_the_same_command_help`; both Windows
coverage jobs failed there, while the no-configuration baseline test passed.
The invocation test now accounts for Windows' `.exe` argv[0] and passes locally
with `cargo test --locked --offline`. `make check-fmt` and offline `make lint`
pass on the current dirty tree. The offline full `make test` run reached 2,083
of 2,104 tests before
`rstest-bdd::trybuild_macros::runner::step_macros_compile` timed out at its
configured 20-minute limit while compiling its isolated UI fixture. This
reproduces the cold nested-build concern tracked in #910; child Cargo remained
offline. The Windows usage correction was subsequently committed and pushed in
the revisions leading to `63b01c4d1cb7e333d6039357daace37d1e435185`, which was
the head at that point in the cycle rather than the head under review now.
Finish the validation and merge cycle described above against whatever head the
pull request currently carries.

The rebase onto `origin/main` at `009012b2`, the gate results at `3e94e1b6` and
the retained validation exception are recorded in `Outcomes & retrospective`.
In summary: `make check-fmt`, `make lint` and `make typecheck` exited zero; the
doc-test, fixture-lockfile and `scripts/tests` steps of `make test` exited zero
when run directly; and two nested-Cargo tests, `runner::step_macros_compile` and
`whitaker::whitaker_lint_gate_accepts_clean_and_rejects_panicking_fixtures`,
have no local verdict because compile admission was saturated for the whole
run. The hosted runs now carry that evidence: run `43386083081` at `e28a927a`
passed all three `build-test` jobs, and its Linux coverage lane passed 2,245 of
2,245 tests. The Windows lanes needed a repair for Cargo 1.100.0's build-dir
layout before they could produce theirs; that repair, its evidence and its
verified toolchains are recorded in the progress log.
