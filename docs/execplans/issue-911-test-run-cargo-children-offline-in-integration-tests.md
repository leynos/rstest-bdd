# Run Cargo children offline in integration tests

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: APPROVED

Tracking issue: <https://github.com/leynos/rstest-bdd/issues/911>

Approved by the user on 2026-11-06, with the instruction to implement through
to a merged pull request.

## Purpose / big picture

Tests in this repository start their own `cargo` processes. Today some of those
children may download packages or update the crates.io index in the middle of a
test. That makes a test result depend on the state of the package cache and on
the network, and it can hold the package-cache lock that every repository on a
shared host contends for.

After this change, three things are true and observable:

1. A named preflight, `make prefetch-fixture-deps`, is the only step that
   downloads what the tests need, and it reads only committed lockfiles.
2. If a locked package is missing from the cache, a Nextest run stops before any
   test starts, with a message naming the package. No test deadline is consumed.
3. A CI run proves the contract by running every Linux test process in a network
   namespace that has no route out. The suite still passes.

`cargo bdd` keeps its behaviour for its real users. It may still fetch
dependencies when somebody runs it in their own project.

## Requirements traced from the issue

Four acceptance criteria govern this plan. The identifiers are used throughout.

- `AC-1`: every Cargo process spawned by a test or test-support helper is
  explicitly offline.
- `AC-2`: fixture dependency preparation uses committed lockfiles and is a
  distinct preflight step; missing cached packages fail before the timed test
  body.
- `AC-3`: CI validation can disable outbound network access for nested Cargo
  while the relevant tests still pass.
- `AC-4`: an audit covers trybuild, Clippy UI fixtures, cargo-bdd CLI and
  registry fixtures, and rebuild-invalidation tests.

## Constraints

- `cargo-bdd` production code must not become offline.
  `crates/cargo-bdd/src/registry/mod.rs` calls `nested_cargo::build_child_env`
  and `nested_cargo::cargo_command`, so the offline policy must not be added to
  `NestedCargoEnv`, `build_child_env` or `cargo_command`. A user running
  `cargo bdd steps` in a fresh project must keep network access.
- `rstest-bdd-server` production code must not become offline.
  `crates/rstest-bdd-server/src/discovery/workspace.rs` runs inside a user's
  editor.
- `make check-fixture-lockfiles` must stay online. It proves that a lockfile
  resolves against a full registry view, and `docs/developers-guide.md` and
  `.github/workflows/ci.yml` both record why it never passes `--offline`.
- No test deadline may be raised to absorb this work, and no test may be skipped
  to satisfy it.
- Do not create an isolated `CARGO_HOME`. Tests and experiments use the shared
  default Cargo cache and let Cargo's package-cache lock serialize access.
- Do not use `/tmp` as a Cargo target directory. Scratch sources may live there;
  build output goes under the repository's `target/`.
- Do not bypass compile admission. `RUSTC_WRAPPER` and `RUSTDOC` stay as they
  are, including inside the network namespace.
- No file may exceed 400 lines, and the cap is met by cohesive splits rather
  than `#[allow]` or allowlist entries. The Whitaker `module_max_lines` lint
  does not honour `scripts/rs-length-allowlist.txt`.
- Run formatting, lint and test gates sequentially for this worktree, through
  the `scrutineer` agent.

## Tolerances (exception triggers)

All counts exclude this ExecPlan file, per the ExecPlan convention.

- Scope: if implementation needs more than 25 files or more than 700 net lines
  of code and other documentation, stop and escalate.
- Interface: the public signatures of `build_binary`,
  `target_directory_for_manifest`, `trybuild_crate_root` and everything in
  `nested_cargo` must not change. If one must, stop and escalate.
- Dependencies: no new external crate. `temp-env` and `cargo_metadata` are
  already present. If another is required, stop and escalate.
- CI time: if the Linux lane's wall-clock time rises by more than 5 minutes
  against the comparison run recorded at the start of `EP-M4`, stop and
  escalate.
- Experimental features: if Nextest removes or renames `setup-scripts` or
  `wrapper-scripts` in the version CI installs, stop and escalate.
- Namespace feasibility: if `EP-M0` shows `unshare` is unusable on
  `ubicloud-standard-2` even with the sysctl change, use the fallback in
  `Decision log` entry `D7` and record that AC-3 is met by denial at Cargo's
  own proxy rather than by the kernel. Escalate if that fallback fails too.
- Iterations: if a gate still fails after three fix attempts for the same cause,
  stop and escalate.
- Sequencing: if PR 916 or PR 782 lands and a rebase conflict changes the design
  rather than the text, stop and escalate.

## Risks

- Risk: a warm cache makes the network denial prove nothing. A child with no
  offline flag makes no network request when everything is already cached.
  Severity: high. Likelihood: certain, and measured during planning.
  Mitigation: AC-1 is proved by the execution trace in `EP-M5` and by unit
  tests, not by denial alone. Denial is proved non-vacuous with a seeded
  offender.
- Risk: `sccache` inside a network namespace cannot reach a server listening on
  `127.0.0.1`, and nested compilation would slow down or fail on CI. Severity:
  high. Likelihood: high without mitigation. Mitigation: export
  `SCCACHE_SERVER_UDS` on Linux CI lanes before the server starts, because a
  filesystem socket path crosses network namespaces. Measured on both runner
  types in `EP-M0`: the socket is reachable and a wrapped `rustc` compiles
  inside the namespace. Residual: the wrapper is only as good as that one
  variable being exported before the server starts.
- Risk: unprivileged user namespaces are restricted on the Ubuntu images.
  Severity: medium. Likelihood: certain. Mitigation: `EP-M0` measured
  `kernel.apparmor_restrict_unprivileged_userns = 1` on both
  `ubicloud-standard-2` (24.04.5) and `ubuntu-latest` (26.04.1), with
  `unshare` failing until `sudo sysctl -w
  kernel.apparmor_restrict_unprivileged_userns=0` is applied. `EP-M4` makes
  that prelude part of the lane, and a lane that cannot apply it must fail
  rather than run the wrapper with a non-functional namespace.
- Risk: experimental Nextest features change. Severity: medium. Likelihood: low.
  Mitigation: declare `nextest-version`, and pin behaviour with contract tests.
  CI installs Nextest 0.9.120 through the coverage action; wrapper `env` needs
  0.9.131, so this plan does not use it.
- Risk: PR 916 (issue 910) and PR 782 (issue 751) change the same files.
  Severity: medium. Likelihood: high. Mitigation: see
  `Sequencing with related work` below, and repeat the audit after any rebase.
- Risk: `crates/rstest-bdd/tests/trybuild_macros/staging.rs` is exactly 400
  lines, so even the small metadata change breaches the cap. Severity: low.
  Likelihood: certain. Mitigation: `EP-M2` moves the target directory helpers
  into a child module. PR 782 also splits this file, so check it first.
- Risk: `[env]` in `.cargo/config.toml` also makes `cargo run` inside this
  repository offline. Severity: low. Likelihood: certain. Mitigation:
  documented in `docs/developers-guide.md`, with the one-line override
  `CARGO_NET_OFFLINE=false cargo run ...`, which works because the `[env]`
  entry does not set `force`.
- Risk: the Python gate test needs the staged GPUI fixture's graph, which
  discovery excludes. Severity: low. Likelihood: medium on lanes without the
  `tools` matrix flag. Mitigation: see `EP-M3`; on `make test`,
  `check-published-gpui-e2e-lock` warms that graph immediately beforehand.

## Progress

- [x] (2026-11-05) Renamed the branch and set the Lody session title.
- [x] (2026-11-05) Audited every Cargo spawn path and read the upstream sources.
- [x] (2026-11-05) Ran the Concordat rule for a baseline, and prototyped the
  mechanisms locally.
- [x] (2026-11-05) Drafted this ExecPlan.
- [x] (2026-11-06) Obtained user approval and set the plan to `APPROVED`.
- [x] `EP-M0`: prove namespace feasibility on real CI runners. The first probe
  (`0ba2766a`, run 42258902324) aborted at the stock `unshare`. On `3b8662c7`
  (run 42266303024) both lanes completed and agree, which settles `D7` in its
  kernel form: stock denial under
  `kernel.apparmor_restrict_unprivileged_userns = 1`, a successful
  `sudo sysctl -w` relaxation, a working namespace afterwards, no egress inside
  it, a filesystem socket reachable across it, and `rustc` through
  `RUSTC_WRAPPER=sccache` completing with exit 0 on the hosted lane. The
  Ubicloud job's `failure` conclusion is a probe defect (`| head -5` closing the
  pipe under `pipefail`), not a namespace result, and `continue-on-error` kept
  it from gating. `D7`'s proxy fallback is not needed. Remaining for this
  milestone: delete the temporary workflow as its own commit, which the
  full-lane evidence now permits. Evidence is under `Artifacts and notes`.
- [x] `EP-M1`: preflight and fail-fast verification. Red observed on
  2026-11-06 for all eight new assertions; green on the same day with
  `make test-workflow-contracts` at 377 passed. The Concordat rule now reports
  `verdict: indeterminate` with only the two `EP-M2` findings left, so the
  Makefile and Nextest halves of NC-008 are satisfied. The first full gate run
  found one deterministic defect in the new contract test, a ruff `DOC201`
  missing-`Returns` on the `expected_manifests` fixture, which is repaired in
  the milestone commit; the remaining gate result, a `cargo-bdd::cli
  list_steps_runs` timeout at 180.006 s, is queue-induced rather than a
  regression, and the re-run that establishes it is recorded under
  `Surprises & discoveries`.
- [ ] `EP-M2`: explicit offline mode at every repository-owned spawn.
- [ ] `EP-M3`: test-process offline default for third-party and indirect spawns.
- [ ] `EP-M4`: CI network denial.
- [ ] `EP-M5`: execution-trace audit, ADR and documentation.

## Surprises & discoveries

- Observation: the issue's premise about trybuild is out of date. Evidence:
  `trybuild-1.0.122/src/cargo.rs:49` adds `--offline` to every build, check,
  run, clean and `generate-lockfile` call. Only `metadata()` at line 100 uses
  `raw_cargo()` without it, and that call passes `--no-deps`. Impact: trybuild
  needs no argument change, but it does resolve without `--locked`, so a run
  can pick a different cached version rather than fail. That is recorded as a
  residual gap.
- Observation: the rebuild-invalidation harness is not fully offline, despite
  the issue's description. Evidence: `clean_fixture_units` in
  `crates/rstest-bdd/tests/feature_rebuild_invalidation/harness.rs:259` runs
  `cargo clean -p ...` with no offline flag. Impact: `EP-M2` fixes this at the
  harness's single command constructor rather than at each call.
- Observation: the issue's list misses several spawns. Evidence: full-graph
  `cargo metadata` in `trybuild_macros/staging.rs:216`, `cargo build --bin` in
  `binary_test_support.rs:235`, metadata in `binary_test_support.rs:158` and
  `macrotest_support.rs:90`, `make lint-whitaker` in
  `trybuild_macros/whitaker.rs:112`, the third-party `macrotest` crate, and a
  real `make` gate in
  `scripts/tests/test_published_gpui_e2e_lockfile_contract.py:73`. Impact: the
  audit table below is the authoritative list.
- Observation: Concordat already defines this contract across the estate, as
  rules NC-007 and NC-008, currently at warning severity. Its reader judges
  evidence on the command itself, so an inherited environment does not count.
  Impact: spawn-level evidence must be visible at each spawn, and this plan
  cannot rely on a runner-wide variable alone.
- Observation: the rule's reader does not scan `src/` test-support modules. It
  reports nothing for `binary_test_support.rs` or `macrotest_support.rs`, even
  though they spawn Cargo. Impact: a `compliant` verdict is necessary but not
  sufficient for AC-1. Repository tests must cover those modules.
- Observation: network denial is vacuous on a warm cache. During planning a
  deliberately online child run outside the namespace downloaded `fnv-1.0.5`
  into the shared cache, and later runs against that package then passed or
  failed for the wrong reason. The stray crate is harmless and was left in
  place. Impact: every witness must use a package that cannot be cached, or be
  checked first; see `Verification plan`.
- Observation: `cargo metadata --no-deps` needs no packages at all. Evidence: it
  exits 0 against a lock naming an uncached package, where the full form fails.
  Impact: `--no-deps` is both a robustness gain and a reduction in lock use, so
  `EP-M2` adds it where the code reads only `target_directory`.
- Observation: `[env]` in `.cargo/config.toml` reaches tests under both
  `cargo test` and `cargo nextest`, but does not configure the outer Cargo.
  Impact: it is a usable runner-wide default that does not disturb the outer
  build.
- Observation: `ci.yml` declares all three `Test and Measure Coverage` steps in
  the one `build-test` job, and their guards select at most one per lane.
  Impact: "a prefetch step before each coverage step" is one step before the
  first of them; three identical network fetches in one job would be redundant.
  The contract asserts exactly one prefetch per job and that it precedes every
  coverage step in that job, which is the same guarantee without the repetition.
- Observation: the `nextest-version` floor is set to `0.9.115`, not the
  `0.9.98` that introduced setup scripts. Evidence: a custom profile inherits
  `profile.default.scripts` only from 0.9.115, and `EP-M4`'s `offline-proof`
  profile depends on that inheritance. Impact: the shared coverage action's
  installer pins cargo-nextest 0.9.120, so CI satisfies the floor; a developer
  with an older local Nextest gets a refusal rather than a silently inert
  configuration.
- Observation: the Concordat reader never inspects a workflow file's `run:`
  step. Evidence: after `EP-M1`, the `run: make prefetch-fixture-deps` steps
  produced no finding at all, neither `compliant` evidence nor the `uncertain`
  delegation the milestone anticipated, and the Makefile finding disappeared
  only once the literal `$(CARGO) fetch --locked` recipe line was a
  prerequisite of `test`. Impact: the plan's escalation clause for a delegated
  step did not apply, and workflow ordering is covered by the contract test
  rather than by the rule.
- Observation: the reader's verdict moved from `noncompliant` to
  `indeterminate`, not to `compliant`, once the Makefile and Nextest findings
  cleared. Evidence: the two remaining warnings are the `EP-M2` spawns in
  `harness/process.rs:98` and `trybuild_macros.rs:185`. Impact: `compliant` is
  only reachable at the end of `EP-M2`, so `EP-M1`'s conformance check is the
  removal of the two non-spawn findings, which the verdict's finding list shows
  directly.
- Observation: GitHub's `shell: bash` template runs the script as
  `/usr/bin/bash --noprofile --norc -e -o pipefail {0}`, so a step-level
  `set -uxo pipefail` does not turn `-e` off. Evidence: the second probe's
  sccache step ended with `failed printing to stdout: Broken pipe` and exit
  101, because `sccache --show-stats | head -5` closed the pipe under
  `pipefail`. Impact: a probe step must guard every command itself and must not
  pipe into `head`; the first probe's `set -e` assumption was wrong for the
  same reason, not merely a missing `|| true`.
- Observation: `unshare --net --map-current-user` needs the apparmor sysctl
  relaxed on both runner types, and both ship it enabled. Evidence: on
  `ubicloud-standard-2` (Ubuntu 24.04.5) and `ubuntu-latest` (Ubuntu 26.04.1)
  the probe reports `kernel.apparmor_restrict_unprivileged_userns = 1`,
  `STOCK_RC=1` with `write failed /proc/self/uid_map: Operation not permitted`,
  then `RELAX_RC=0` and `RELAXED_RC=0`. Impact: `EP-M4`'s wrapper cannot assume
  a usable namespace. The workflow step that configures the runner must relax
  the sysctl before Nextest starts, and the local recipe in `EP-M4` needs the
  same prelude or it will report a false failure on a developer's machine.
- Observation: the two lanes agree on every other probe, so the namespace
  behaviour is not runner-specific. Evidence: `EGRESS_RC=2` (no name resolution
  inside the namespace), `UDS_ACROSS_NAMESPACE_RC 0` with
  `SERVER_RECEIVED b'ping'`, and on the hosted lane `SHOW_STATS_RC=0` and
  `INSIDE_WRAPPED_RUSTC_RC=0` for `rustc` run under `RUSTC_WRAPPER=sccache`
  inside the namespace. Impact: `D6` holds, and `sccache` over
  `SCCACHE_SERVER_UDS` keeps working for nested compilation under denial, so
  the lock contention that issue 910 addresses is not reintroduced.
- Observation: `make lint` rejected the new contract test with ruff `DOC201`
  (`docstring-missing-returns`) at `offline_test_contract_test.py:56`, not with
  any Rust finding. Evidence: the gate log ends
  `make[1]: *** [Makefile:170: lint-python] Error 1`, and the same invocation
  over the full `PYTHON_TARGETS` set reports `Found 1 error`. The repository's
  `pydocstyle` convention is numpy, so a fixture returning a value needs a
  `Returns` section. Impact: every new Python fixture that returns a value
  needs that section, and the documentation-convention gate runs before the
  Rust lints in the same target.
- Observation: the only test-side failure in the first full run was
  `cargo-bdd::cli list_steps_runs` at `TIMEOUT [180.006s]`, with 2001 passed,
  1 timed out and 7 skipped, and 81 of 2083 tests not run. Evidence: the
  milestone touches no file under `crates/cargo-bdd`; `build-limits-status`
  reported the pool healthy but saturated (`capacity 4, held 4`) with 77 to 89
  crates queued ahead of the fixture's own dependencies; and the 13-1-1 plan
  records the same test at 180.002 s in a full run against 94.079 s in
  isolation. Impact: this is contention on the shared compiler admission, not a
  regression, and the cancelled tests have no verdict from that run, so a green
  result for them has to come from a completed one.

## Decision log

- Decision `D1`: keep the policy out of `nested_cargo`, and inject it from the
  test side. Rationale: `cargo-bdd` shares that module in production, so a
  default there would remove network access from end users.
  `cli_child_environment` in `crates/cargo-bdd/tests/cli.rs` adds the variable,
  and `cargo-bdd` passes it to its own children because `build_child_env`
  preserves unrelated variables. Date/Author: 2026-11-05, planning agent.
- Decision `D2`: use two layers. Layer 1 is explicit offline evidence on every
  spawn this repository owns. Layer 2 is `CARGO_NET_OFFLINE = "true"` under
  `[env]` in `.cargo/config.toml`, for spawns the repository does not own or
  cannot reach: trybuild's `metadata()`, the `macrotest` crate and
  `cargo expand`, in-process `discover_workspace` tests, and the LSP smoke
  tests. Rationale: Layer 1 satisfies NC-007. Layer 2 is one line, avoids
  `temp_env` wrappers around four trybuild suites that PR 916 is also editing,
  and, unlike exporting the variable on the CI step, cannot break the coverage
  action's own tool installation. Date/Author: 2026-11-05, planning agent.
- Decision `D3`: extend `prefetch-fixture-deps` rather than adding a new target.
  Rationale: it is already the mutation lane's contract and is pinned by
  `tests/workflow_contracts/mutation_testing_test.py`. A second, similar target
  would be duplication. It gains a literal `$(CARGO) fetch --locked` line for
  the root lockfile, which NC-008 recognizes when the target is a prerequisite
  of `test`. Date/Author: 2026-11-05, planning agent.
- Decision `D4`: separate fetching from verifying. Make and CI steps fetch
  online. Nextest setup scripts run `cargo fetch --locked --offline`, which
  downloads nothing and fails fast. Rationale: AC-2 requires failure before the
  timed body on every Nextest route, including filtered local runs and the
  coverage action, neither of which passes through `make test`. A setup script
  is the only hook all of them share. An online setup script would hide a
  download inside the test command, which the issue forbids. Date/Author:
  2026-11-05, planning agent.
- Decision `D5`: list setup-script manifests literally, and add a parity test.
  Rationale: Nextest runs setup commands without a shell, and the Concordat
  reader only recognizes a visible `cargo fetch --locked`. A contract test
  holds the list equal to the discovered fixture set, minus the documented
  exclusion for `tests/fixtures/published-gpui-0-2-2`, which no test spawns.
  Date/Author: 2026-11-05, planning agent.
- Decision `D6`: run the existing Linux coverage lane under the denial profile
  rather than adding a job. Rationale: a separate job would rebuild the
  workspace and repeat cold nested builds on a two-vCPU runner, which is the
  cost issue 910 is trying to remove. Setting `NEXTEST_PROFILE` on the existing
  step needs no new compilation. The wrapper uses `filter = "all()"`, so a
  future spawner is covered without a list to maintain. A search found no test
  using TCP or UDP sockets, so the namespace's downed loopback should affect
  nothing; `EP-M4` confirms that. Date/Author: 2026-11-05, planning agent.
- Decision `D7`: prefer kernel denial through `unshare`, with a defined
  fallback. If `EP-M0` rules out namespaces, the same wrapper mechanism runs
  `env CARGO_HTTP_PROXY=http://127.0.0.1:9 CARGO_NET_RETRY=0`, which planning
  measured failing a real download in 0.07 s. Rationale: the kernel route also
  catches non-Cargo network use and is not specific to one Cargo setting; the
  fallback needs no privileges but covers only Cargo's own HTTP client.
  Date/Author: 2026-11-05, planning agent. Settled 2026-11-06 by `EP-M0`: the
  kernel route is available on both runner types once
  `kernel.apparmor_restrict_unprivileged_userns` is set to `0`, so the proxy
  fallback is not used. The relaxation is a required part of the wrapper's
  setup, not an optional extra.
- Decision `D8`: do not adopt Concordat as a CI gate here. Rationale: the rule
  ships at warning severity, always exits 0 at that severity, and the estate
  owns its promotion. This plan records the `verdict` as evidence, read from
  the JSON output rather than the exit code. Date/Author: 2026-11-05, planning
  agent.
- Decision `D9`: Windows lanes get the preflight and both offline layers but not
  the denial profile. Rationale: they use `cargo test` because of the
  documented Nextest hang, so they have no setup phase and no wrapper. AC-3
  asks that CI can disable the network, and one lane satisfies it. Under
  `cargo test` there are no per-test deadlines, so a late failure consumes no
  timed budget. Date/Author: 2026-11-05, planning agent.
- Decision `D10`: no behaviour-driven scenario. Rationale: this is a contract
  about child processes for contributors, with no user-visible behaviour to
  state in Gherkin. The subprocess test in `EP-M2` provides the end-to-end
  evidence. Date/Author: 2026-11-05, planning agent.
- Decision `D11`: one prefetch step per job, not one per coverage step.
  Rationale: `ci.yml`'s three coverage steps share the `build-test` job and are
  mutually exclusive by guard, so a single step ahead of the first precedes all
  three. The contract asserts exactly one prefetch per job and that its index
  is below every coverage step's, which is the ordering guarantee `EP-M1` asks
  for without three identical fetches. Date/Author: 2026-11-06, implementation
  agent.

## Outcomes & retrospective

Not yet started. To be completed at each milestone and at the end of the work.

## Conformance basis

- Issue 911 as filed, with the four criteria restated above.
- Base revision: `f58aa42f8b731cef17c0b237c9ae64ea06e75734`.
- Concordat `nested-cargo-offline-tests`, rules NC-007, NC-008 and EN-001, at
  revision `075a69f30b4a732a0e07d18d35b212faa4a8261c`.
- `docs/developers-guide.md`, the "Nested Cargo" and fixture lockfile material
  and "Test timeouts: four tiers, outermost last".
- `docs/testing-strategy.md`.
- ADR-023 for the shared workflow contract library. This plan adds a new ADR;
  use the next free number when `EP-M5` starts, because other open PRs add ADRs
  too.
- Pattern reference, not a binding standard: `leynos/whitaker` PR 537.
- No Terms of Reference or technical design document governs this work.

Trace links:

```plaintext
AC-1 -> EP-M2, EP-M3 -> unit tests, NC-007 verdict, execution trace in EP-M5
AC-2 -> EP-M1 -> setup-script abort transcript, contract tests, NC-008 verdict
AC-3 -> EP-M0, EP-M4 -> CI run under the denial profile, seeded offender
AC-4 -> Audit section below -> re-checked by the execution trace in EP-M5
```

## Context and orientation

Definitions used in this plan:

- A nested Cargo process is a `cargo` started by a test, rather than the `cargo`
  that started the tests.
- The preflight is a step that runs before any test, and is the only place
  allowed to download packages.
- A setup script is a Nextest feature: a command that runs before tests. If it
  fails, the whole run stops. A wrapper script is a Nextest feature that
  prefixes the test binary's command line. Both are experimental and need
  opting in.
- A network namespace is a Linux kernel feature giving a process its own network
  stack. A new one has no routes, so nothing inside can reach the outside.
  `unshare --net --map-current-user <command>` runs a command in a new one
  without root access.
- A lockfile is `Cargo.lock`. `--locked` refuses to change it, `--offline`
  refuses the network, and `--frozen` is the two combined.

Six lockfiles matter to the preflight: the root `Cargo.lock`, and one beside
each manifest that fixture discovery returns (measured during planning):

```plaintext
crates/cargo-bdd/tests/fixtures/minimal/Cargo.toml
crates/rstest-bdd/tests/fixtures/feature_addition/Cargo.toml
crates/rstest-bdd/tests/fixtures/rebuild_invalidation/Cargo.toml
crates/rstest-bdd/tests/ui_lints/Cargo.toml
tests/fixtures/published-gpui-0-2-2/Cargo.toml
```

A seventh, `tests/fixtures/published-gpui-e2e`, is committed but resolves only
after `make stage-published-gpui-e2e`, so discovery excludes it.

`scripts/check_fixture_lockfiles.py --fetch` already fetches every discovered
fixture, and `make prefetch-fixture-deps` runs it. Today only the mutation lane
calls that target, from `.github/workflows/mutation-testing.yml`.

### Audit of test-spawned Cargo (AC-4)

Layer 1 means an explicit change at the spawn. Layer 2 means the `[env]`
default. Line numbers are for the base revision.

| Area          | Spawn site                                                                   | Command                              | Today                | Plan                                      |
| ------------- | ---------------------------------------------------------------------------- | ------------------------------------ | -------------------- | ----------------------------------------- |
| Clippy UI     | `crates/rstest-bdd/tests/trybuild_macros.rs:185`                             | `cargo clippy --locked`              | Online               | Layer 1: add `--offline`                  |
| trybuild      | `trybuild-1.0.122/src/cargo.rs:49`, all four compile suites                  | build, check, run, clean             | Offline, not locked  | No change; Layer 2 as a backstop          |
| trybuild      | `trybuild-1.0.122/src/cargo.rs:100`                                          | `metadata --no-deps`                 | Needs no packages    | Layer 2                                   |
| trybuild      | `crates/rstest-bdd/tests/trybuild_macros/staging.rs:216`                     | full `cargo metadata`                | Online               | Layer 1: `no_deps` and the variable       |
| Whitaker      | `crates/rstest-bdd/tests/trybuild_macros/whitaker.rs:67`                     | `cargo dylint --version`             | Probe                | Layer 1: the variable                     |
| Whitaker      | `crates/rstest-bdd/tests/trybuild_macros/whitaker.rs:112`                    | `make lint-whitaker`                 | Online               | Layer 1: the variable                     |
| macrotest     | `crates/rstest-bdd-harness/src/macrotest_support.rs:34`                      | `cargo expand --version`             | Probe                | Layer 1: the variable                     |
| macrotest     | `crates/rstest-bdd-harness/src/macrotest_support.rs:90`                      | `metadata --no-deps`                 | Needs no packages    | Layer 1: the variable                     |
| macrotest     | `macrotest-1.2.1/src/cargo.rs`, opt-in                                       | full metadata, `cargo expand`        | Online               | Layer 2                                   |
| Binary helper | `crates/rstest-bdd-harness/src/binary_test_support.rs:158`                   | `metadata --no-deps`                 | Needs no packages    | Layer 1: the variable                     |
| Binary helper | `crates/rstest-bdd-harness/src/binary_test_support.rs:235`                   | `cargo build --bin`                  | Online               | Layer 1: `--locked --offline`             |
| cargo-bdd     | `crates/cargo-bdd/src/registry/mod.rs:115` and `:242`, via tests             | full metadata, `test --no-run`       | Online               | `cli_child_environment` adds the variable |
| cargo-bdd     | `crates/cargo-bdd/tests/nested_target/mod.rs:75`                             | `metadata --no-deps`                 | Needs no packages    | Same environment                          |
| Rebuild tests | `crates/rstest-bdd/tests/feature_rebuild_invalidation/harness/process.rs:98` | `test`, `metadata`                   | `--locked --offline` | Layer 1: the variable, last               |
| Rebuild tests | `crates/rstest-bdd/tests/feature_rebuild_invalidation/harness.rs:259`        | `clean -p`                           | Online               | Fixed by the row above                    |
| Server        | `crates/rstest-bdd-server/src/discovery/workspace.rs:55`, via tests          | `metadata --no-deps`                 | Needs no packages    | Production unchanged; Layer 2             |
| Example       | `examples/todo-cli/tests/cli.rs:7`                                           | helper above                         | Online               | Fixed by the helper                       |
| Python        | `scripts/tests/test_published_gpui_e2e_lockfile_contract.py:73`              | `cargo package`, `metadata --locked` | Online               | Set the variable on the subprocess        |

Reviewed and out of scope, because no test spawns them:
`make check-published-gpui`, `make e2e-published-gpui`,
`make check-fixture-lockfiles`, `make publish-check`, and the outer
`cargo nextest` command. Other `Command::new` sites in tests run the test
binary itself, the LSP server, `true` or `sh`; `EP-M5` confirms this by trace.
The other Python tests that touch Cargo use recording stand-ins.

### Sequencing with related work

PR 916 (issue 910) is a draft that moves cold builds into preparation tests. It
adds new spawn sites, notably `ui_clippy_command` with `--locked` and no
`--offline`, and it edits `.config/nextest.toml`, `process.rs`, `Makefile` and
`macrotest_support.rs`. PR 782 (issue 751) splits `staging.rs` and folds the
rebuild harness into shared support. It currently conflicts with `main`.

This plan does not stack on either. Its design does not depend on merge order,
because preflight must run first in every order and setup scripts precede
preparation tests. Whichever lands second must rebase and repeat the `EP-M5`
trace. `EP-M1` must add `experimental` beside PR 916's `nextest-version` line
if that has landed, and otherwise add both.

## Plan of work

### `EP-M0`: Prove namespace feasibility on real runners (prototyping)

Assigned: AC-3. On this branch, add a temporary first step to the Linux lane
that runs the commands below on both `ubicloud-standard-2` and `ubuntu-latest`,
and reports the results without failing the job. Set `SCCACHE_SERVER_UDS` for
the probe.

```bash
sysctl kernel.apparmor_restrict_unprivileged_userns || true
unshare --net --map-current-user sh -c 'id -u; sccache --show-stats | head -3'
```

End state: `D7` is settled from evidence, and the temporary step is reverted.
Acceptance: the run URL and output are recorded under `Artifacts and notes`.
Conformance check: did this falsify `D6` or `D7`? Recovery: revert the commit.
Remaining gaps: Windows, per `D9`.

Outcome, 2026-11-06: neither `D6` nor `D7` was falsified. The stock image
refuses the namespace, `sudo sysctl -w
kernel.apparmor_restrict_unprivileged_userns=0` succeeds, and the same
`unshare` then works on both runner types; inside it there is no egress, while
a filesystem socket and `sccache` behind it still work. `EP-M4` therefore
relaxes the sysctl on the lane and keeps `SCCACHE_SERVER_UDS`. The temporary
workflow is deleted in its own commit, and its content stays in this plan's
`Artifacts and notes` rather than in the tree.

### `EP-M1`: Preflight and fail-fast verification

Assigned: AC-2. Write tests first, and watch them fail:

1. In new `tests/workflow_contracts/offline_test_contract_test.py`, assert that
   each setup script is `cargo fetch --locked --offline`, that together they
   cover the root and the discovered fixtures minus the exclusion in `D5`, that
   a `profile.default.scripts` rule runs them under `filter = "all()"`, and
   that the prefetch step precedes the coverage step in every lane of `ci.yml`
   and `coverage-main.yml`.
2. In `scripts/tests/test_fixture_lockfile_prefetch.py`, update the
   recording-Cargo expectation so the Make route fetches the root first.

Then make them pass:

1. In `Makefile`, add `$(CARGO) fetch --locked` as the first recipe line of
   `prefetch-fixture-deps`, make it a prerequisite of `test`, and update its
   help text and comment, which still say it is for the mutation lane alone.
2. In `.config/nextest.toml`, add `experimental = ["setup-scripts"]`, a
   `nextest-version` of at least `0.9.98`, one `[scripts.setup.*]` table per
   manifest, and a `[[profile.default.scripts]]` rule. Any new tables must
   satisfy `crates/rstest-bdd/tests/nextest_config.rs` and
   `tests/workflow_contracts/nextest_config.py`, so adjust those readers if
   they reject the new keys.
3. In `ci.yml` and `coverage-main.yml`, add a `make prefetch-fixture-deps` step
   before each `Test and Measure Coverage` step. Keep the two files' lane
   pairings intact, since a contract test compares them. Windows has GNU Make
   already.

Re-run the Concordat rule here, because this is the first milestone that
changes what NC-008 reads. The planning prototype proved the Makefile and
Nextest forms, but not a `run: make prefetch-fixture-deps` workflow step. If
the reader reports that step as `uncertain` because it delegates to Make,
record it and escalate rather than restructure the workflow to satisfy a
warning.

End state: all test routes fetch first, and a missing package stops a Nextest
run before any test starts. Conformance check: is
`make check-fixture-lockfiles` still online? Recovery: revert the commits; no
persistent state changes. Remaining gap: spawns are still not explicitly
offline.

### `EP-M2`: Explicit offline mode at repository-owned spawns

Assigned: AC-1, AC-4. To test each argument list without running Cargo, extract
a pure command-building function where none exists, then assert with `get_args`
and `get_envs`. Red first:

- `trybuild_macros.rs`: extract the Clippy command; assert `--locked` and
  `--offline`.
- `binary_test_support.rs`: extract the build command from `build_binary`;
  assert `--locked` and `--offline`. Extract the metadata command from
  `target_directory_for_manifest`; assert the variable through
  `MetadataCommand::cargo_command`.
- `macrotest_support.rs`: the same for `trybuild_crate_root` and the expand
  probe.
- `process.rs`: extend the existing `#[cfg(test)]` module. A snapshot containing
  `CARGO_NET_OFFLINE=false` must still produce a command whose last value is
  `true`.
- `cli.rs`: an `rstest` table over absent, `false`, empty and `true` starting
  values shows that `cli_child_environment` always yields `true`.
- `cli.rs`: a new end-to-end test
  `cargo_bdd_fails_closed_without_cached_packages`. It writes a scratch
  manifest and lock under the temporary directory naming the package
  `rstest-bdd-offline-witness-does-not-exist`, runs `cargo-bdd steps` through
  `cli_child_environment`, and asserts failure with `offline mode` in stderr.
  Because no such package exists, this witness cannot be cached, so it stays
  valid on any host and can never repeat the `fnv` contamination recorded
  above. Cargo prints that note only when offline, so removing the injected
  variable fails the test. Confirm this by mutation and record it.

Then green, applying exactly the changes in the audit table. In `process.rs`,
set the variable after the loop that applies the snapshot, because `env_clear`
and the loop would otherwise take precedence, and NC-007 reads the last write.
Move the target-directory helpers in `staging.rs` to a new child module before
changing them, so the file stays within 400 lines. Do this as its own
refactoring commit, unless PR 782 has already split the file.

End state: `verdict: compliant` from Concordat for spawn findings, and every
Layer 1 row is enforced by a test. Conformance check: is
`crates/cargo-bdd/src/registry/mod.rs` unchanged? Recovery: revert. Remaining
gap: third-party and indirect spawns.

### `EP-M3`: Test-process default for spawns not owned here

Assigned: AC-1. Red first: new
`crates/rstest-bdd-harness/tests/offline_contract.rs` asserts that
`CARGO_NET_OFFLINE` is `true` in the test process, with a message pointing to
`.cargo/config.toml`. A contract test also pins the `[env]` entry and that it
does not set `force`. Green: add to `.cargo/config.toml`:

```toml
[env]
# Test processes inherit this, so a Cargo child that this repository does not
# construct, such as trybuild's metadata query, cannot reach the network. The
# outer Cargo ignores `[env]`, so ordinary builds still resolve normally.
CARGO_NET_OFFLINE = "true"
```

Confirm that the entry does not invalidate compiled units: after a full build,
`cargo build -v` must report `Fresh` for workspace crates. Confirm that a
cargo-mutants copy keeps `.cargo/` by reading the mutation lane's baseline,
since `offline_contract` fails there otherwise. Run the Concordat
`rust-build-defaults` rule, which also reads this file.

In `test_published_gpui_e2e_lockfile_contract.py`, set `CARGO_NET_OFFLINE=true`
on the subprocess environment. Planning measured the gate passing this way.
Confirm that its assertion on `--locked was passed` still holds, because the
refusal text could differ under offline mode. Mark the test's dependency on the
warmed staged graph in its docstring. If a lane fails because that graph is
absent, make the test run its online stage target first, as a distinct setup
call, rather than dropping the variable.

End state: a test run started through Cargo has offline mode by default.
Recovery: revert. Remaining gap: nothing yet proves the network is unneeded.

### `EP-M4`: CI network denial

Assigned: AC-3. Record a comparison run of the Linux lane's wall-clock time
first. Add to `.config/nextest.toml`:

```toml
[scripts.wrapper.no-network]
command = ["unshare", "--net", "--map-current-user"]

[profile.offline-proof]

[[profile.offline-proof.scripts]]
filter = "all()"
platform = 'cfg(target_os = "linux")'
run-wrapper = "no-network"
```

Add `wrapper-scripts` to the `experimental` list. On the Linux coverage steps
in both workflow files, set `NEXTEST_PROFILE: offline-proof`. Do not use a
`--profile` argument, since the action does not expose one, and confirm that
the action, which writes a default Nextest configuration when none exists,
leaves this one alone. Confirm that `profile.default`'s overrides, timeouts and
setup scripts still apply to the custom profile by running
`cargo nextest show-config`, and confirm the group limits apply. Export
`SCCACHE_SERVER_UDS` in the existing
`Configure runner parallelism and compiler cache` step on Linux, and run
`sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0` there as well:
`EP-M0` shows both runner images ship that restriction enabled, so the wrapper
would fail every test without it. A step that cannot relax the sysctl must fail
loudly rather than let the wrapper turn into a false pass. Extend the contract
test so that it checks the wrapper, the filter, the profile, the workflow
setting and the sysctl prelude.

Add a seeded offender as the negative control, in `offline_contract.rs`: a test
that does its work only on Linux when `NEXTEST_PROFILE` is `offline-proof`, so
the ordinary suite never makes its index request. It removes the variable from
a child and runs `cargo fetch --locked` on the uncachable witness from `EP-M2`,
which forces an index lookup however warm the cache is. It asserts that stderr
contains `unable to update registry` and does not contain
`no matching package`. Planning measured that the first string is what a denied
child reports, under both the namespace and the `D7` fallback, and that the
second is what a child with a working network reports. Removing the wrapper
therefore fails the test.

End state: CI's Linux lane runs every test in a namespace, green. Local proof:

```bash
NEXTEST_PROFILE=offline-proof make test 2>&1 | tee /tmp/test-offline-rstest-bdd-issue-911.out
```

Planning confirmed that `/run/user/1000/buildlimits/*.sock` are filesystem
sockets and that `build-limits-status` reports healthy inside the namespace, so
compile admission should keep working locally. Conformance check: is the time
within tolerance, and is the watchdog ladder in `docs/developers-guide.md`
still correct? Recovery: remove the `NEXTEST_PROFILE` setting, which restores
the previous behaviour.

### `EP-M5`: Execution-trace audit, ADR and documentation

Assigned: AC-1, AC-4. The trace answers what the static rule cannot see. It
observes real processes, once, as evidence rather than as a permanent gate:

```bash
strace -f -qq -e trace=execve -v -s 8192 -o target/tests/issue-911-exec.trace \
  cargo nextest run --workspace --all-targets --all-features
```

Every `execve` of a `cargo` binary with `NEXTEST_RUN_ID` in its environment
must carry `--offline`, `--frozen`, or `CARGO_NET_OFFLINE=true`. Excluding the
Layer 2 default for one more run, by temporarily removing the `[env]` entry,
gives the list of children relying only on Layer 2, which must match the audit
table's Layer 2 rows. Record both results under `Artifacts and notes`. Also
trace the opt-in `RSTEST_BDD_RUN_MACROTEST=1` route, if `cargo expand` is
installed. If it is not, say so in the plan rather than claim coverage.

Write the ADR as `docs/adr-NNN-offline-nested-cargo-test-contract.md`, and
reference it from `docs/rstest-bdd-design.md`. Update
`docs/developers-guide.md`: the contract, how to add a spawn, the `cargo run`
override, what to do on a setup-script failure, and the corrected description
of the prefetch, which is currently scoped to the mutation lane. Update
`docs/testing-strategy.md` and the `ci.yml` comment on the fixture lockfile
step, which is still true but incomplete. Update `docs/repository-layout.md`
for any new file. `docs/users-guide.md` needs no change, because `cargo bdd`
behaves as before; state that in the PR.

End state: all acceptance evidence recorded, the plan set to `COMPLETE`, and
the retrospective written.

## Verification plan

### Invariants

- `INV-1`: every Cargo process with a test as its ancestor runs in offline mode.
  Method: unit tests over argument and environment construction, plus the
  execution trace. Artefacts: tests in `EP-M2` and `EP-M3`, the trace in
  `EP-M5`. Discharge: the tests pass and the trace has no violations. Why it
  can fail: the trace reads real `execve` calls, so an unlisted spawn shows up.
  Non-vacuity: the Layer 2 subtraction run must report the known third-party
  children, proving the check sees them, and its offender list must be
  non-empty when the `[env]` entry is removed.
- `INV-2`: production `cargo-bdd` and the server stay online-capable. Method: a
  source-level check that `crates/cargo-bdd/src/` and
  `crates/rstest-bdd-server/src/` gain no offline setting, plus a table case in
  `crates/rstest-bdd-harness/tests/nested_cargo_env.rs` showing that
  `build_child_env` neither adds nor strips `CARGO_NET_OFFLINE`. Why it can
  fail: adding the default to `from_vars` fails the "absent" case. Non-vacuity:
  apply that mutation once and record the failure.
- `INV-3`: `make prefetch-fixture-deps` precedes tests on every route. Method:
  contract tests reading `Makefile`, `ci.yml`, `coverage-main.yml` and the
  Nextest configuration, plus the Concordat NC-008 verdict. Non-vacuity: the
  contract test must fail on a seeded reorder of the workflow steps.
- `INV-4`: a missing locked package stops a Nextest run before any test starts.
  Method: a run against a scratch copy with a setup script naming the
  uncachable package. Expected evidence is shown under `Artifacts and notes`.
  Why it can fail: a test starting first shows up in the summary as a non-zero
  count.
- `INV-5`: tests pass with no route out. Method: the CI lane under
  `offline-proof`. Why it can fail: see the next item.
- `INV-6`: the denial is real. A warm cache makes `INV-5` vacuous by itself, so
  the seeded offender in `EP-M4` uses a package name no cache can hold, which
  forces a network attempt. It must report `unable to update registry` inside
  the namespace, and it must fail if the wrapper is removed.

### Lemmas

- `L-1`: `[env]` reaches test processes under `cargo test` and `cargo nextest`,
  and is not applied to the outer Cargo. Together with `L-2` this connects
  Layer 2 to `INV-1`. Evidence: planning measurement, and `offline_contract`
  keeps it true.
- `L-2`: a child that inherits the variable and is not `env_clear`ed is offline.
  Evidence: Cargo documentation and the measured failure below.
- `L-3`: `build_child_env` preserves an unrelated variable, so `cargo-bdd`'s
  children inherit the variable from the test's child environment. Evidence:
  the existing filter tests and the new end-to-end test.

### Axioms

- `A-1`: Cargo honours `--offline` and `CARGO_NET_OFFLINE` as documented, for
  values `true` and `1`.
- `A-2`: `cargo fetch --locked` makes everything in the lockfile available to a
  later `--offline` build, for all targets, as documented.
- `A-3`: the installed `trybuild` passes `--offline`. This holds for 1.0.122,
  and the requirement in `Cargo.toml` is `1`. A later release could change it;
  the `EP-M4` CI run and Layer 2 protect against that.
- `A-4`: a new network namespace has no external route. Evidence: an uncached
  download failing in it with `Could not resolve host`.
- `A-5`: Nextest stops a run when a setup script fails, per its documentation,
  and confirmed below.

### Method selection

Finite parametrized cases are adequate for the environment rule, because its
result does not depend on the starting value beyond absent or present. A
property test would add nothing. Bounded model checking and deductive proof are
not applicable: no state machine, arithmetic or `unsafe` code is introduced,
and the core question is about processes outside any proof boundary. No Rust
invariant here warrants Kani or Verus.

### Residual gaps

- trybuild and macrotest resolve without `--locked`, because each generates its
  own manifest. Offline mode makes them fail closed on a missing package, but
  they can select another cached version instead. This is existing behaviour,
  and issue 911 does not ask to change it.
- Windows has the preflight and both layers, but no kernel denial.
- A plain `cargo test` from outside the repository, using `--manifest-path`,
  does not read this `.cargo/config.toml`. `offline_contract` reports that
  clearly.

## Concrete steps

Run from the repository root. Use `scrutineer` for full gates, one at a time:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-rstest-bdd-issue-911.out
make lint 2>&1 | tee /tmp/lint-rstest-bdd-issue-911.out
make test 2>&1 | tee /tmp/test-rstest-bdd-issue-911.out
make test-workflow-contracts 2>&1 | tee /tmp/contracts-rstest-bdd-issue-911.out
make markdownlint 2>&1 | tee /tmp/markdownlint-rstest-bdd-issue-911.out
```

Run the audit rule against the candidate tree, with Concordat at a pinned
revision, and read `verdict` from the JSON rather than the exit code (`D8`):

```bash
uv run --no-dev concordat artefact rule run nested-cargo-offline-tests \
  --repo "$PWD" --format json
```

Commit after each passing step, with the refactoring commit separate from the
behaviour commit.

## Validation and acceptance

- AC-1: the trace in `EP-M5` reports no violation, every Layer 1 unit test
  passes, and Concordat reports `compliant`.
  `cargo_bdd_fails_closed_without_cached_packages` fails before `EP-M2` and
  passes after it.
- AC-2: `prefetch-fixture-deps` reads only committed lockfiles through
  `--locked`, and a missing package gives `0/N tests run` and
  `error: setup script failed`.
- AC-3: a CI run with `NEXTEST_PROFILE=offline-proof` is green, and the seeded
  offender test passes only with the wrapper in place.
- AC-4: the audit table is complete, and the trace reveals no unlisted spawn.

Quality criteria: `make check-fmt`, `make lint`, `make test`,
`make test-workflow-contracts`, `make markdownlint` and `make nixie` all pass,
and the required `build-test` checks pass on the Linux and Windows lanes.

## Idempotence and recovery

`make prefetch-fixture-deps` can be repeated, and costs one lock acquisition
per manifest once the cache is warm. On a setup-script failure, run it online
and re-run the tests. Each milestone reverts cleanly as commits. The
`NEXTEST_PROFILE` setting turns denial off with a one-line change, without
touching the rest of the contract. Delete `target/tests/issue-911-exec.trace`
after recording its summary.

## Artifacts and notes

All of the following were measured during planning, on the base revision, with
Cargo 1.99.1 and Nextest 0.9.133. They come from scratch projects, not this
repository's suite, so each milestone must still produce its own evidence.

The Concordat baseline, four warnings with `verdict: noncompliant`:

```plaintext
.config/nextest.toml:1 runs the tests (default) with no success-gated cargo fetch --locked
crates/rstest-bdd/tests/feature_rebuild_invalidation/harness/process.rs:98 ... reader cannot see
Makefile:121 may run the tests (test), but its ordering could not be read
crates/rstest-bdd/tests/trybuild_macros.rs:185 ... reader cannot see
```

A scratch copy with `--offline` on the Clippy command, the variable in
`process.rs`, a prerequisite containing `$(CARGO) fetch --locked`, and a
default-profile setup script of `cargo fetch --locked --offline` reported
`compliant` with no findings.

The candidate tree at the end of `EP-M1` reports `verdict: indeterminate` with
two findings, both spawns that `EP-M2` fixes:

```plaintext
crates/rstest-bdd/tests/feature_rebuild_invalidation/harness/process.rs:98
  starts cargo with arguments or environment the reader cannot see ...
  flags ["--color"]
crates/rstest-bdd/tests/trybuild_macros.rs:185
  starts cargo with arguments or environment the reader cannot see ...
  flags ["--locked", "--manifest-path", "--bin", "--"], environment CARGO_TARGET_DIR
```

The baseline's `.config/nextest.toml:1` and `Makefile:121` findings are gone,
and no finding was raised against either workflow's
`run: make prefetch-fixture-deps` step.

A locked package that is not cached fails closed, whichever way offline mode is
set:

```plaintext
$ cargo fetch --locked --offline --manifest-path .../Cargo.toml   # also: full metadata
error: failed to download `fnv v1.0.5`
Caused by:
  attempting to make an HTTP request, but --offline was specified
$ cargo metadata --locked --offline --no-deps ...                 # exit 0
```

A setup script stops the run before any test:

```plaintext
SETUP FAIL [   0.025s] fetch-fixture: cargo fetch --locked --offline --manifest-path fixture/Cargo.toml
   Summary [   0.046s] 0/2 tests run: 0 passed, 0 skipped
error: setup script failed
```

An online child inside the namespace wrapper cannot resolve the registry, and
the package stays uncached:

```plaintext
error: failed to download from `https://static.crates.io/crates/fnv/1.0.3/download`
  [6] Could not resolve hostname (Could not resolve host: static.crates.io)
PASS [ 11.528s] (2/2) nx::t online_child_blocked
```

The outer Cargo is not affected by `[env]`; with a refusing proxy it tries the
download rather than reporting offline mode:

```plaintext
[7] Could not connect to server (Failed to connect to static.crates.io:443 over proxy 127.0.0.1 ...
```

One uncachable package name separates all three states, which is what the tests
in `EP-M2` and `EP-M4` assert on:

```plaintext
offline, by flag or by variable:
  error: no matching package named `rstest-bdd-offline-witness-does-not-exist` found
  note: offline mode (via `--offline`) can sometimes cause surprising resolution failures
online with a network:
  Updating crates.io index
  error: no matching package named `rstest-bdd-offline-witness-does-not-exist` found
online, denied by the namespace or by a refusing proxy:
  unable to update registry `crates-io`
  [6] Could not resolve hostname (Could not resolve host: index.crates.io)
```

All six lockfiles passed `cargo fetch --locked --offline` on this host.
`CARGO_NET_OFFLINE=true make lint-whitaker` on a dependency-free scratch crate
exited 0, and `CARGO_NET_OFFLINE=true make check-published-gpui-e2e-lock`
exited 0.

`EP-M0`: run <https://github.com/leynos/rstest-bdd/actions/runs/42266303024>, on
`3b8662c7`, completed with `conclusion: success` and both matrix jobs finished.
The Ubicloud job (`ubicloud-standard-2`, Ubuntu 24.04.5, kernel
6.14.0-1017-azure) reports both halves of `D7`:

```plaintext
=== unshare, stock image ===
unshare: write failed /proc/self/uid_map: Operation not permitted
STOCK_RC=1
=== relax the apparmor restriction, then retry ===
RELAX_RC=0
RELAXED_RC=0
=== egress from inside a namespace ===
EGRESS_RC=2
SUMMARY stock=1 relax=0 relaxed=0 egress=2
UDS_ACROSS_NAMESPACE_RC 0
SERVER_RECEIVED b'ping'
```

The hosted job (`ubuntu-latest`, Ubuntu 26.04.1 LTS) reports the same four
values, so the result is not specific to one runner image:

```plaintext
kernel.apparmor_restrict_unprivileged_userns = 1
STOCK_RC=1
RELAX_RC=0
RELAXED_RC=0
EGRESS_RC=2
SUMMARY stock=1 relax=0 relaxed=0 egress=2
UDS_ACROSS_NAMESPACE_RC 0
SERVER_RECEIVED b'ping'
SHOW_STATS_RC=0
INSIDE_WRAPPED_RUSTC_RC=0
```

So the stock image refuses an unprivileged user namespace,
`sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0` succeeds, the
same `unshare` then succeeds, and a namespace so created cannot resolve
`static.crates.io`. A filesystem socket opened in the parent namespace is
reachable from inside the child, which is the property `EP-M4` needs for
`sccache`. `sccache --start-server` printed
`Listening on address /tmp/issue-911-sccache.sock`; `sccache --show-stats` run
under `unshare --net --map-current-user` reached it on both lanes, and on the
hosted lane `rustc` wrapped by `sccache` compiled inside the namespace with exit
0. The Ubicloud job's `failure` conclusion comes from the probe itself: its
`sccache --show-stats | head -5` step ended on
`failed printing to stdout: Broken pipe` with exit 101, because GitHub's
`shell: bash` template runs the script under `-e -o pipefail` and `head` closes
the pipe. That is why the Ubicloud lane did not reach the wrapped-`rustc` line;
the hosted lane did, and `EP-M4`'s real CI run is the evidence for the full
suite. `continue-on-error` kept the probe from gating the run.

## Interfaces and dependencies

No public signature changes, no new crate. New private functions build commands
for testing, in `binary_test_support.rs`, `macrotest_support.rs` and
`trybuild_macros.rs`. No shared helper is added, because NC-007 reads evidence
on the command itself, and a helper in another file would read as unresolved. A
repository sweep found no existing equivalent. The scope and re-use rule for
the contract goes in `docs/developers-guide.md` under `EP-M5`, per the
abstraction policy in `AGENTS.md`.

Tool versions: Nextest at least 0.9.98 for experimental setup and wrapper
scripts, which the action's 0.9.120 satisfies on paper, though the prototypes
ran only on 0.9.133, so `EP-M0` must confirm it on CI; `unshare` from
util-linux 2.37 or later for `--map-current-user`; `sccache` with
`SCCACHE_SERVER_UDS`, against CI's 0.12.0.

External documentation consulted:

- <https://nexte.st/docs/configuration/setup-scripts/>
- <https://nexte.st/docs/configuration/wrapper-scripts/>
- <https://nexte.st/docs/configuration/env-vars/>
- <https://doc.rust-lang.org/cargo/reference/config.html>
- <https://doc.rust-lang.org/cargo/commands/cargo-fetch.html>
- <https://github.com/mozilla/sccache>
- <https://github.com/actions/runner-images/issues/10443>

## Revision note

2026-11-05: initial draft, from the audit and prototypes recorded above. No
implementation has started; the next action is user approval, then `EP-M0`.
