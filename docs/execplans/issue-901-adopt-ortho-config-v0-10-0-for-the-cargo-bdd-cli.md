# Adopt OrthoConfig v0.10.0 for the `cargo-bdd` command-line interface

This execution plan (ExecPlan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IMPLEMENTATION COMPLETE (validation exception retained; merge pending
the hosted checks on the final head)

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
  isolate `HOME`, `XDG_CONFIG_HOME`, `XDG_CONFIG_DIRS`, `USERPROFILE`, and
  `APPDATA`, and remove the four configurable `CARGO_BDD_CMDS_*` variables from
  the child process environment.
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
- [ ] §6: documentation, gates, follow-up review, commit, push, hosted CI,
      review reconciliation, and merge. Merge is still pending: this box stays
      unchecked until the pull request lands on `main`. Everything in this
      section before merge is done apart from the validation exception this
      plan retains rather than claims passed; the exception and its evidence
      are recorded in `Outcomes & retrospective`, and merge waits on the hosted
      checks for the final head. The candidate was rebased onto `origin/main`
      (`693bba91`) and gated at revision `96e38df8`; the rebase described in
      `Outcomes & retrospective` is the later one.
      `make check-fmt`, `make lint`, `make typecheck`, `make markdownlint`,
      `make nixie` and `make test-workflow-contracts` (373 tests) all exited
      zero there, as did the workspace doc tests, `make check-fixture-lockfiles`
      and `scripts/tests` (290 tests). The full suite passed every test that
      reached a verdict; the single-invocation `make test` exception is
      recorded in Outcomes & retrospective. The earlier delivery history
      follows. Its first hosted
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

## Surprises & discoveries

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
fixture-lockfile gate and the Python suite have local green verdicts; the
workspace suite and the no-default-features leg each leave the nested-Cargo
tests named above without a passing local verdict. That exception is retained
and recorded here, not waived: `make test` is not claimed to have passed
locally, the pull request's validation section states the same, and the
exception is tracked by #910. This plan cites the gated revision rather than
its own commit, and review follow-ups after that revision are recorded in the
progress log and surprises below.

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
  is present. Method: checked-in byte assertions generated from the captured
  pre-change binary output. Successful listing behaviour is covered by the
  existing integration tests, which must remain unmodified, and by source
  snapshots for output formatting; the baseline probe could not produce a
  successful fixture listing. Artefact: focused tests in
  `crates/cargo-bdd/tests/`. Discharge condition: all applicable cases pass in
  the full test run.

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
  `.cargo_bdd.toml`, and point `HOME`, `XDG_CONFIG_HOME`, `XDG_CONFIG_DIRS`,
  `USERPROFILE`, and `APPDATA` at empty temporary directories. Remove the four
  `CARGO_BDD_CMDS_*` setting variables from the child environment. This
  confirms that neither platform discovery roots nor the environment tier
  contributes a value. Rationale: verifying OrthoConfig's internals is out of
  scope, but the repository's hermeticity depends on this boundary, so it is
  exercised once rather than assumed. Residual gap: a developer with a real
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
- Every row of Table 1 is reproduced by `V1`.
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

The `make test` row is met for the final tree by evidence assembled from more
than one place, because no single local invocation produces it under this
host's compile admission: the Linux pull-request coverage lane runs the
workspace suite on a dedicated runner, and the locally run doc tests,
`make check-fixture-lockfiles` and `uv run pytest scripts/tests` cover the
recipe's other steps. The local single-invocation `make test` exception is
retained rather than claimed green. It is two nested-Cargo tests in
`rstest-bdd::trybuild_macros` — `runner::step_macros_compile` and
`whitaker::whitaker_lint_gate_accepts_clean_and_rejects_panicking_fixtures` —
whose 1,200-second budgets are sized for a quiet runner; raising them is
[#910](https://github.com/leynos/rstest-bdd/issues/910)'s subject. The
exception is recorded here, in `Outcomes & retrospective` and in the pull
request's validation section, and the pull request does not merge until the
hosted checks on its final head are green.

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
run. The hosted runs carry the completing evidence for those two.
