# Adopt OrthoConfig v0.10.0 for the `cargo-bdd` command-line interface

This execution plan (ExecPlan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS

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
- Dependencies: if a crate other than `ortho_config` must be added to any
  manifest, stop and escalate.
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
  isolate `HOME` and `XDG_CONFIG_HOME` in the child process environment.
- Risk: the Cargo-dispatch wrapper's `external_subcommand` help differs from
  `cargo-bdd --help`. Severity: medium. Likelihood: high. Mitigation: the
  upstream helper deliberately gives the installed binary its own command name;
  decide and record which surface each form shows, then pin both in tests.
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
- [x] §3: write ADR 024 recording the configuration decision.
- [x] §3: add the configuration and integration-boundary sections to
      `docs/rstest-bdd-design.md`; add the ADR 024 entry and reference
      definition to `docs/contents.md`; add the decision to roadmap §6.1.
- [ ] §3: review the recorded design with CodeRabbit after the deterministic
      documentation gates; resolve any valid concerns before implementation.
- [x] (2026-11-01) Correct the plan's baseline description: retain verified
      help/parser captures, and mark the listing probe and baseline package
      run as unsuccessful. `make fmt`, `make check-fmt`, `make markdownlint`,
      and `make nixie` passed; imported guide SHA-256 checksums remain exact.
- [ ] §4: implement. Manifest and MSRV split; `OrthoConfig` derives; the
      Cargo external-subcommand wrapper; boundary error handling.
- [ ] §5: tests. Precedence tiers per subcommand with injected sources;
      unchanged-behaviour proof; unhappy paths; both invocation forms.
- [ ] §6: documentation, gates, commit, push, draft pull request.

## Surprises & discoveries

- Observation: `cargo-bdd bdd steps` — the form Cargo produces — is rejected
  today with `error: unrecognized subcommand 'bdd'`, exit 2, 0 bytes on stdout
  and 102 bytes on stderr. Evidence: captured in
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

- Decision: use `ortho_config::cargo::external_subcommand` for the Cargo entry
  shape, so both `cargo bdd steps` and `cargo-bdd steps` are accepted.
  Rationale: it is the upstream-supported helper, and it enforces the
  `cargo-<name>` / `<name>` pairing with debug assertions. The alternative — an
  `Option<String>` first positional in `Cli` — cannot reproduce Cargo's
  namespace and would accept a stray `bdd` token in the flat form too.
  Date/Author: 2026-10-25, implementing agent.

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

## Outcomes & retrospective

Not yet applicable. To be completed at each milestone boundary and reconciled
against the conformance basis before the plan is marked `COMPLETE`.

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
- `docs/rstest-bdd-design.md` and the new ADR 024, both authored by this plan.
- `docs/roadmap.md` §6.1 "Diagnostic tooling".

No pre-existing Terms of Reference or technical-design identifiers cover the
`cargo-bdd` configuration surface; the design document gains that section as
part of §3 rather than inheriting one. Trace links:

```plaintext
issue-901 -> EP-M1 (baseline freeze)   -> tests/cli.rs unchanged, baseline fixture
issue-901 -> EP-M2 (configuration)     -> tests/precedence.rs per-subcommand tiers
issue-901 -> EP-M3 (cargo entry)       -> tests/invocation.rs both forms
issue-901 -> EP-M4 (documentation)     -> users' guide, developers' guide, ADR 024
```

## Verification plan

The change introduces four obligations worth stating.

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
  the current default. Method: parameterized test with injected sources
  (`MapEnv`, explicit file sources, `SubcommandFileContext::new`), one row per
  `(subcommand, field, tier-that-wins)` combination, plus the absent-everywhere
  row. Rationale: the tier structure is a small finite product; enumeration is
  clearer than generation and covers the whole domain. Domain: 2 subcommands x
  2 fields x 4 tiers, plus 2 "absent" rows. Artefact:
  `crates/cargo-bdd/tests/precedence.rs` (or a `#[cfg(test)]` module if the
  loaders are crate-internal). Evidence: a row fails if the merge order is
  wrong, if `cli_default_as_absent` is missing (the "environment wins over an
  untyped flag" row fails), or if `Default` disagrees with the clap default
  (the "absent" row fails). Non-vacuity: the file-injected and
  environment-injected rows set genuinely different values, so a passing test
  cannot be explained by every tier agreeing. Negative control: dropping
  `cli_default_as_absent` from one field must fail exactly the rows for that
  field. Residual gap: this exercises the loader against injected sources, not
  OrthoConfig's internal discovery. Discovery itself is third-party and treated
  as an axiom, with the caveat recorded under `V5`.

- Obligation: `V3 invocation-forms`. `cargo-bdd steps` and `cargo-bdd bdd steps`
  both run the `steps` subcommand; each form is separately pinned for exit
  status and streams. Method: isolated child-process test, one case per form,
  both reaching a fixture workspace. Rationale: the defect is a property of
  argument shape, which only a real process exercises; an in-process
  `Cli::try_parse_from` would also work for the accept/reject decision, and
  both are used — the parse-level test for breadth, the child-process test for
  the streams. Domain: 2 forms x 2 subcommands (`steps`, `skipped`) = 4 rows.
  Artefact: `crates/cargo-bdd/tests/invocation.rs`. Evidence: against the
  pre-change binary the Cargo-form rows fail with exit 2 and
  `error: unrecognized subcommand 'bdd'`; against the post-change binary they
  pass. Discharge condition: green, and the flat-form rows still match Table 1.
  Non-vacuity: the pre-change failure is recorded, so the test demonstrably
  distinguishes the two implementations. Negative control: invoking with
  `cargo-bdd bdd bogus` must still exit 2.

- Obligation: `V4 failure-surface`. Malformed and unreadable configuration
  files, invalid values, unknown keys, and localized parse failures each
  produce the documented exit status and stream, and none of them is confused
  with "no configuration file present". Method: parameterized table over the
  failure cases. Domain: malformed TOML; unreadable file (mode 0o000);
  `skipped = "maybe"` (type error); an unknown key; an invalid value for a
  typed field. Artefact: `crates/cargo-bdd/tests/config_errors.rs`. Evidence:
  each row asserts a non-zero exit and an actionable message naming the file;
  the missing-file row asserts exit 0 and the unchanged listing. Discharge
  condition: green. Non-vacuity: the missing-file row is the negative control
  for the error rows — if the loader treated absence as an error, that row
  would fail. Note: the unknown-key row asserts that the key is **ignored**,
  because the derive emits no `deny_unknown_fields`; asserting an error there
  would be a test written against behaviour OrthoConfig does not have.

- Obligation: `V5 discovery axiom`. The claim "with no configuration file
  present, no file tier contributes" depends on OrthoConfig's discovery rules
  (home `.cargo_bdd.toml`, the first existing XDG configuration file, then a
  local `.cargo_bdd.toml` on Unix and Redox; platform-native locations
  elsewhere; TOML only with default features). Method: treated as an axiom
  about a third-party interface, with one contract-level check: a child process
  with `HOME` and `XDG_CONFIG_HOME` pointed at empty temporary directories and
  `XDG_CONFIG_DIRS` likewise, confirming the file tier is empty. Rationale:
  verifying OrthoConfig's internals is out of scope, but the repository's own
  hermeticity depends on this boundary, so it is exercised once rather than
  assumed. Residual gap: a developer with a real `~/.cargo_bdd.toml` will see
  it affect local runs. The users' guide must say so.

No non-trivial lemma or formal proof is introduced: the change selects and
merges values from a finite set of sources, and the properties above are
decidable by enumeration. A property test or model check would restate the
table. This conclusion is recorded rather than omitting the section.

## Plan of work

Stage A — design record (no behaviour change). Write ADR 024 and the design
document section; add them to `docs/contents.md`; add the roadmap note. Settle
the two open decisions in the Decision log. Validate with `make markdownlint`,
`make nixie`, and `make check-fmt`.

Stage B — red tests. Add the baseline fixture and `tests/baseline_output.rs`,
confirm it passes against the current binary (this is the freeze, established
before any change), then add `tests/invocation.rs` and confirm the Cargo-form
rows **fail** with `error: unrecognized subcommand 'bdd'`. Add the precedence
and failure-surface tests against the not-yet-existing API and confirm they
fail to compile or fail for the expected reason. Commit the red state.

Stage C — implementation. Manifest and MSRV; derives; wrapper; boundary error
handling. Validate with the focused tests, then `make lint` and
`make check-fmt`.

Stage D — documentation and wider validation. Users' guide and developers'
guide; correct the pre-existing flag drift; run the full gate set.

Go/no-go: do not leave Stage B until the red failure is observed and recorded;
do not enter Stage D until Stage C's focused tests are green.

## Milestones and plateaus

- `EP-M1` baseline freeze (end state: a checked-in fixture and a test that
  proves it, green against the pre-change binary). Acceptance: `V1` green; no
  production code changed. Recovery: revert the commit; nothing else depends on
  it.
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

Each milestone is a coherent repository state: a reader can stop after `EP-M1`
with a documented freeze, or after `EP-M2` with configuration working but the
Cargo form still rejected.

## Concrete steps

1. Write `docs/adr-024-adopt-ortho-config-for-the-cargo-bdd-cli.md` following
   the house style of ADR 023: status, date, context, drivers, options with
   `Accepted.`/`Rejected.`, decision outcome, known risks, architectural
   rationale.
2. Add the configuration section to `docs/rstest-bdd-design.md` after §3.6
   ("Workspace layout decisions"), covering: configurable versus CLI-only
   settings; file name, discovery locations, and TOML-only formats; the
   `CARGO_BDD_` prefix and the absence of legacy variables or files; precedence
   with `cli_default_as_absent`; per-subcommand namespaces; English help and
   parse failures; no profiles; and the Cargo entry shape.
3. Add the ADR 024 bullet and the `[adr-024]` reference definition to
   `docs/contents.md`.
4. Add the decision to `docs/roadmap.md` §6.1 as a new checked sub-item under
   the diagnostic tooling heading, naming OrthoConfig v0.10.0.
5. Add the baseline fixture and `tests/baseline_output.rs`; run it.
6. Add `tests/invocation.rs`; run it and record the red failure.
7. In `Cargo.toml` (workspace) add `ortho_config = "0.10.0"` under
   `[workspace.dependencies]`; in `crates/cargo-bdd/Cargo.toml` add
   `ortho_config.workspace = true` and set `rust-version = "1.89"`.
8. In `crates/cargo-bdd/src/cli.rs`, derive `OrthoConfig` on the argument
   structs, give each a `#[command(name = "…")]`, mark the booleans with
   `#[ortho_config(cli_default_as_absent)]`, and implement `Default` to agree
   with the clap defaults.
9. Add the Cargo external-subcommand wrapper and route both forms through one
   dispatch function.
10. Handle display requests and configuration errors at the boundary, keeping
    clap's exit statuses and streams.
11. Add `tests/precedence.rs` and `tests/config_errors.rs`.
12. Update `docs/users-guide.md` (configuration file, environment variables,
    precedence, the MSRV split) and `docs/developers-guide.md` (the integration
    boundary and why libraries stay free of OrthoConfig).
13. Run every gate; commit atomically; push; open the draft PR.

## Validation and acceptance

Acceptance means all of the following, in one gate run against the final tree:

- `make check-fmt`, `make lint`, and `make test` succeed.
- `make markdownlint`, `make spelling`, and `make nixie` succeed.
- `crates/cargo-bdd/tests/cli.rs` passes **unmodified** — verified with
  `git diff --stat` showing no change to that file.
- Every row of Table 1 is reproduced by `V1`.
- `V2`, `V3`, and `V4` are green, and the pre-change failure for the Cargo form
  is recorded in the commit that adds the test.
- No crate other than `cargo-bdd` appears in the `ortho_config` dependency
  graph: `cargo tree -p rstest-bdd -e normal | grep -c ortho_config` returns 0,
  and likewise for each other library crate.
- `cargo metadata` reports `rust-version` 1.88 for the workspace and 1.89 for
  `cargo-bdd`.

Reported as unavailable, if they remain unavailable: there is no MSRV CI job in
this repository, so the 1.89 split is proven by `cargo metadata` and by the
crate's own build rather than by a dedicated pipeline. State this rather than
implying a pipeline covers it.

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
commits the freeze fixture before any production change, reverting a later
milestone never costs the evidence that the freeze held.

## Artefacts and notes

Expected gate logs, with `$ACTION` one of `check-fmt`, `markdownlint`,
`spelling`, `nixie`, `lint`, `test`:

```plaintext
/tmp/$ACTION-$(get-project)-$(git branch --show-current).out
```

Durable evidence (checked in, unlike `/tmp`):

- `crates/cargo-bdd/tests/fixtures/baseline/` — the captured bytes per
  invocation.
- The commit that adds `tests/invocation.rs`, recording the observed red
  failure in its message.

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
