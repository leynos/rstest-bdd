# Make a `.feature`-only edit rebuild the scenario binary

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

Roadmap item: 10.3.3 (`docs/roadmap.md`, phase 10.3 "v0.6.0 final
requirements").

## Purpose / big picture

`rstest-bdd` is a Rust library that lets a test author write a test scenario in
Gherkin — a plain-text, `Given`/`When`/`Then` notation stored in a `.feature`
file — and bind it to a Rust test function with the `#[scenario]` attribute
macro or the `scenarios!` function-like macro. Those macros read the `.feature`
file while the compiler is running, at what Rust calls *macro-expansion time*,
and generate a Rust test from its contents.

Cargo, Rust's build tool, decides whether to recompile a crate by comparing the
timestamps of the files it believes the crate depends on. It learns that set of
files from a *dep-info* file (a `.d` file that `rustc` writes next to the
compiled output). `rustc` records a file in dep-info only when the file was
pulled in through `include_str!`, `include_bytes!`, `include!`, or a build
script's `cargo::rerun-if-changed` directive. A procedural macro that opens a
file with ordinary filesystem calls is invisible to that machinery.

The consequence today is a silent and dangerous foot-gun. Edit only a
`.feature` file and Cargo does not rebuild. The stale test binary, compiled
from the *old* Gherkin text, is re-run and reports success. In a testing
framework this is close to the worst possible failure mode: a corrupted
expectation appears to pass. The downstream `gauss` project hit exactly this
during its v0.6.0-beta3 migration and had to falsify a scenario by editing a
Rust file instead, because editing the `.feature` file had no effect.

After this change, a user can open a `.feature` file, change a `Then` step so
that it no longer matches what the code does, save the file, run `cargo test`
(or `make test`), and watch the crate recompile and the test fail. Nothing else
about their project changes: no new `build.rs`, no new dependency, no edit to
any existing `#[scenario]` or `scenarios!` call site.

You can see it working like this, and this is the acceptance behaviour the plan
delivers:

```console
$ cargo test -p my-scenarios
   Compiling my-scenarios v0.1.0
test greeting_scenario ... ok

$ $EDITOR tests/features/greeting.feature      # change only the Then step

$ cargo test -p my-scenarios
   Compiling my-scenarios v0.1.0               # <-- this line is the fix
test greeting_scenario ... FAILED
```

Before the change, the second run prints no `Compiling` line and the test still
passes.

## Constraints

These are hard invariants. If satisfying the objective would require breaking
one, stop and escalate rather than working around it.

1. **No public API change and no call-site change.** Roadmap 10.3.3 states the
   fix is non-breaking. The signatures and accepted arguments of `#[scenario]`,
   `scenarios!`, `#[given]`, `#[when]`, `#[then]`, and every runtime type in
   `crates/rstest-bdd` must be unchanged. Existing user code must compile
   untouched.
2. **No absolute path may be baked into the macro's emitted token stream.**
   This is binding constraint 1 of
   `docs/adr-010-feature-file-change-detection.md`. The macro must not write a
   literal such as `"/home/alice/project/tests/features/x.feature"` into the
   code it generates. See the *Decision Log* for the precise, testable form of
   this constraint, which had to be sharpened after measurement.
3. **Minimum supported Rust version (MSRV) stays at 1.85.** The workspace
   `Cargo.toml` declares `rust-version = "1.85"` and `edition = "2024"`. Any
   mechanism that needs a newer stable compiler is out of bounds. In
   particular this rules out `proc_macro::Span::local_file`, stabilized in
   1.88.
4. **No nightly-only compiler feature on the default path.**
   `proc_macro::tracked_path` remains unstable (rust-lang/rust#99515 is still
   open and unstabilized as of this plan's date), so it cannot be the shipping
   mechanism.
5. **Invalidation is a tested contract, not advisory prose.** This is binding
   constraint 2 of ADR-010. A regression test must fail before the fix and pass
   after it.
6. **`make check-fmt`, `make lint`, and `make test` must all pass** at the end
   of every milestone. `make lint` runs `clippy` with `-D warnings` across
   `--workspace --all-targets --all-features`, `cargo doc` with
   `RUSTDOCFLAGS="--cfg docsrs -D warnings"`, the Whitaker Dylint suite, Ruff,
   and four Python structural checkers (see *Context and orientation*).
7. **No new external dependency** without escalation. See *Tolerances*.
8. **Do not use `/tmp` as a build target.** Scratch build output belongs under
   the repository's own `target/` directory. `/tmp` is for logs only.

## Tolerances (exception triggers)

Stop and escalate — do not improvize — when any of these is reached.

- **Scope.** More than 25 files changed, or more than 900 net added lines
  across the whole change (documentation excluded).
- **New dependency.** Any addition to `[workspace.dependencies]` or to a
  crate's `[dependencies]`/`[dev-dependencies]` that is not already present in
  the workspace. This tolerance is *already engaged* — see the
  `googletest`/`pretty_assertions` decision in the *Decision Log*, which needs
  a ruling before Milestone 2 starts.
- **New published crate.** Creating `rstest-bdd-build` (or any other new
  workspace member that would be published) changes the release surface. Stop
  and escalate. See the `scenarios!` directory-tracking decision.
- **Public API.** Any change to a `pub` item's signature in `crates/rstest-bdd`,
  `crates/rstest-bdd-macros`, `crates/rstest-bdd-harness`, or
  `crates/rstest-bdd-policy`.
- **Test-suite wall clock.** `.config/nextest.toml` sets
  `global-timeout = "5m"` for the whole default-profile run. If adding the new
  regression test pushes `make test` past four minutes on this machine, stop
  and escalate rather than silently raising the ceiling.
- **File length.** `scripts/check_rs_file_lengths.py` caps every non-`target`
  `*.rs` file at 400 lines and refuses stale allowlist entries. If a file must
  exceed 400 lines, split it; do not add an allowlist entry without
  escalating.
- **Iterations.** If a milestone's tests still fail after five focused
  attempts, stop and escalate with the captured log.
- **Ambiguity.** If a choice materially changes the outcome and the plan does
  not already settle it, stop and present the options with trade-offs.

## Risks

- **Risk: the trybuild staging environment resolves `CARGO_MANIFEST_DIR`
  differently from a real user crate.**
  Severity: high. Likelihood: medium.
  `crates/rstest-bdd/tests/trybuild_macros/staging.rs` copies `.feature` files
  into `<target>/tests/trybuild/rstest-bdd/` and `trybuild` compiles fixtures
  with that directory as the crate root. The emitted `include_str!` must
  resolve there too.
  Mitigation: Milestone 3 adds a compile-pass fixture specifically to prove it,
  and the existing fixtures under `crates/rstest-bdd/tests/fixtures_macros/`
  already hand-write `const _: &str = include_str!("basic.feature");` — direct
  evidence the staged layout supports the construct.

- **Risk: Windows path separators.**
  Severity: medium. Likelihood: low.
  The emitted path is built as `concat!(env!("CARGO_MANIFEST_DIR"), "/", rel)`.
  On Windows `CARGO_MANIFEST_DIR` uses backslashes, producing a mixed-separator
  path. `rustc` accepts forward slashes on Windows, but this is unverified on
  this Linux host.
  Mitigation: the CI matrix in `.github/workflows/ci.yml` has two
  `windows-latest` legs. Treat a Windows CI failure as a blocking finding, not
  a flake. Emit the relative component with `/` separators always.

- **Risk: the regression test is slow enough to breach the 5-minute nextest
  global timeout.**
  Severity: medium. Likelihood: medium.
  The test compiles a fixture crate that depends on `rstest-bdd` and
  `rstest-bdd-macros`.
  Mitigation: copy the proven pattern from `crates/cargo-bdd/tests/cli.rs`,
  which sets `CARGO_TARGET_DIR` to the shared workspace `target/` so path
  dependencies are already warm, runs `#[serial]`, and sits in the
  `cargo-spawning` nextest test-group with `max-threads = 1`. Measure and
  record the wall clock in *Artefacts and notes*; escalate on breach.

- **Risk: changing the embedded feature-path literal from absolute to relative
  churns more expectations than anticipated.**
  Severity: low. Likelihood: low.
  Both macros currently emit an absolute path into
  `const __RSTEST_BDD_FEATURE_PATH`, which is used in runtime tracing and error
  text. A blast-radius survey during planning found that **no checked-in
  `.stderr` or `.snap` expectation contains a real macro-emitted absolute
  path** — every snapshot and CLI assertion that mentions a feature path uses a
  hand-written synthetic string fed directly to `ScenarioMetadata::new`,
  bypassing the macros. The risk is therefore much smaller than it first
  appears, but it is not zero: macro-crate unit tests and doctests were not
  exhaustively surveyed.
  Mitigation: Milestone 5 is deliberately separable and sequenced last, so the
  core invalidation fix can land even if this milestone is deferred. Re-run the
  survey before editing.

- **Risk: unpicking `canonical_feature_path` loses symlink resolution
  silently.**
  Severity: medium. Likelihood: medium.
  `crates/rstest-bdd-macros/src/macros/scenario/paths.rs` resolves symlinks
  through `cap-std` as a side effect of producing an absolute path, and its
  memoization cache is keyed on the absolute form. Switching the *emitted*
  value to relative must not accidentally drop symlink canonicalization from
  the *diagnostic* path, nor corrupt the cache key.
  Mitigation: keep `canonical_feature_path` intact for diagnostics and add a
  separate relative-path accessor, rather than mutating the existing function's
  return value.

- **Risk: coarse filesystem mtime granularity makes the regression test flaky.**
  Severity: medium. Likelihood: medium.
  Some filesystems record modification times with one- or two-second
  granularity, so an edit made immediately after a build can look "not newer".
  Mitigation: after rewriting the `.feature` file, explicitly set its
  modification time to two seconds in the future using
  `std::fs::File::set_modified` (stable since Rust 1.75, no new dependency).
  Do not use a bare `sleep`.

- **Risk: `scenarios!` cannot detect a newly *added* `.feature` file.**
  Severity: low. Likelihood: certain.
  Per-file `include_str!` registration tracks edits to files that were present
  when the macro last ran. It cannot see a file that did not exist then,
  because nothing references it.
  Mitigation: this residual gap is documented explicitly rather than hidden,
  and a follow-up roadmap item is proposed. See the *Decision Log*.

## Progress

- [ ] Milestone 0: orientation and go/no-go on the two open decisions (no code
      changes).
- [ ] Milestone 1: red tests — the failing regression test, the failing
      expansion assertion, and the BDD feature specification.
- [ ] Milestone 2: green — emit the tracking binding from `#[scenario]`.
- [ ] Milestone 3: green — emit the tracking binding from `scenarios!`, plus
      the trybuild compile-pass and compile-fail fixtures.
- [ ] Milestone 4: the redacted `insta` diagnostic snapshot with semantic
      assertions.
- [ ] Milestone 5: remove the absolute path from `__RSTEST_BDD_FEATURE_PATH`
      (conditional — see Decision D3).
- [ ] Milestone 6: documentation, ADR status, migration-guide caveat removal,
      roadmap tick.

## Surprises & discoveries

Recorded during planning; keep appending during implementation.

- Observation: **the `concat!(env!(…))` form of `include_str!` embeds nothing
  into a compiled binary** — neither the absolute path nor the feature text —
  yet still registers the file in dep-info and still forces a rebuild.
  Evidence: measured on this host with stable `rustc`; see *Artefacts and
  notes*, transcripts A and B. A control crate without the construct showed the
  *same* absolute-path presence in the dev-profile binary (it comes from debug
  info, not from the mechanism) and both crates were free of it in release.
  Impact: this is the load-bearing discovery of the plan. It makes the
  zero-friction Option A path viable on MSRV 1.85 without the span gymnastics
  ADR-010 anticipated, and it requires ADR-010's rejection rationale to be
  amended rather than merely obeyed.

- Observation: **both macros already bake an absolute path into the artefact
  today**, independently of any `include_str!`, and they do it by two different
  routes.
  Evidence: `crates/rstest-bdd-macros/src/macros/scenario/paths.rs:113`
  (`canonical_feature_path`) returns an absolute, cap-std-canonicalized,
  symlink-resolved path for `#[scenario]`.
  `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs:307`
  computes a manifest-*relative* `rel_path` and then immediately re-absolutizes
  it with `ctx.manifest_dir.join(ctx.rel_path)` — a plain string join with no
  canonicalization. `create_scenario_literals`
  (`crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs:143`) wraps
  whichever value it receives in a `syn::LitStr`, and
  `crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs:261` emits
  `const __RSTEST_BDD_FEATURE_PATH: &str = #feature_literal;`. That constant
  *is* read at runtime for tracing and error context, so it survives
  optimization into release binaries.
  Impact: the roadmap finish line "no absolute `CARGO_MANIFEST_DIR` path
  appears in the artefact" is not satisfiable by the invalidation work alone.
  Milestone 5 exists to address it, and must change **both** routes in
  lockstep or the two macros will disagree. Decision D3 records the scoping
  question.

- Observation: **nothing re-opens the feature file at runtime using the
  embedded path.** Traced exhaustively: the constant flows into
  `ScenarioMetadata::new`, `ExecutionError`, `HarnessError::with_scenario_context`,
  the JSON and JUnit reporters, and `cargo-bdd`'s display formatting — all of
  which treat it as an opaque `Display` string. There is no
  `Path::new(feature_path)` or `PathBuf::from(feature_path)` anywhere in the
  repository, and no doc comment claims the value is absolute.
  Impact: Milestone 5 is safe from a runtime-correctness standpoint; the only
  visible effect is the text of failure messages and reporter output.

- Observation: **several existing trybuild fixtures already hand-write the very
  construct this plan automates.** `crates/rstest-bdd/tests/fixtures_macros/`
  contains lines such as `const _: &str = include_str!("basic.feature");`.
  Impact: strong prior evidence the construct behaves in the staged trybuild
  environment. Once the macro emits its own binding, those manual lines become
  redundant; removing one of them is a cheap, honest end-to-end signal that the
  macro-emitted binding really works. Keep at least one fixture with the manual
  line removed.

- Observation: **`googletest` and `pretty_assertions` are not used anywhere in
  this workspace.** Evidence: no match in any `Cargo.toml`. The house testing
  stack is `rstest`, `insta`, `proptest`, `serial_test`, `trybuild`,
  `macrotest`, `tempfile`, `temp-env`.
  Impact: see Decision D1.

## Decision log

- **Decision D0: use macro-emitted `include_str!` with deferred path
  construction — `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", REL))`
  — bound to `const _: &str` inside the generated scenario function, for both
  `#[scenario]` and `scenarios!`.**
  Rationale: it is the only candidate that satisfies every constraint at once.
  It works on stable Rust 1.85 (no MSRV bump, unlike the call-site-`Span`
  variant ADR-010 assumed). It needs no consumer action (unlike a build
  script). It embeds neither the absolute path nor the feature text into the
  compiled binary (measured — see transcripts A and B), so it does not breach
  Constraint 2. `const _` needs no name, so there is no collision risk and no
  `dead_code` warning even under `-D warnings` (measured — transcript C).
  Alternatives rejected: the call-site-relative `include_str!` of ADR-010
  Option A needs `Span::local_file` (Rust 1.88, breaches Constraint 3) and
  returns `None` under `--remap-path-prefix`; `proc_macro::tracked_path`
  (Option C) is still unstable (breaches Constraint 4); a mandatory build
  script (Option B) breaches Constraint 1's spirit by requiring every consumer
  to add `build.rs`.
  Date/Author: 2026-08-15, planning agent.

- **Decision D0a: amend ADR-010 rather than silently diverge from it.**
  Rationale: ADR-010 rejects "absolute-path `include_str!`" on the ground that
  it "embeds the full … path into the binary", producing byte-divergent
  artefacts from different build directories. Measurement shows that rationale
  applies to the variant where *the proc macro itself writes an absolute string
  literal into the token stream*, and does not apply to the variant where the
  macro emits `env!("CARGO_MANIFEST_DIR")` and lets `rustc` construct the path
  at expansion time: the resulting token stream is identical regardless of
  build directory, and the compiled binary contains neither the path nor the
  file contents. ADR-010's decision outcome explicitly defers the mechanism
  choice to this ExecPlan, so recording the correction and marking the ADR
  *Accepted* with the chosen mechanism is within remit. The ADR text must gain
  the distinction and the evidence, so a future reader does not re-reject the
  working design on the disproved premise.
  Date/Author: 2026-08-15, planning agent.

- **Decision D1 (OPEN — needs a ruling before Milestone 2): assertion
  libraries.**
  The task brief asks for `googletest` assertions and `pretty_assertions`.
  Neither appears anywhere in this workspace, and adding them would trip the
  *new dependency* tolerance. The plan's default is to follow the established
  house stack (`rstest` for parameterization and fixtures, `insta` for
  snapshots, `proptest` for properties, `serial_test` for serialization) and
  not introduce two new dev-dependencies for one feature. Adopting them is
  perfectly reasonable if the intent is to start migrating the whole suite, but
  that is a repository-wide decision, not a 10.3.3 decision.
  Options: (a) proceed on the house stack, propose a separate roadmap item for
  a workspace-wide assertion-library adoption; (b) add both as workspace
  dev-dependencies and use them in the new tests only, accepting an
  inconsistent suite; (c) add both and migrate the touched files.
  Recommendation: (a).
  Date/Author: 2026-08-15, planning agent. **Awaiting ruling.**

- **Decision D2 (OPEN — needs a ruling before Milestone 3): how far to close
  the `scenarios!` directory-addition gap.**
  Per-file `include_str!` makes `scenarios!` notice *edits* to every `.feature`
  file it discovered, which is the reported foot-gun and is exactly what the
  roadmap sentence asks for ("registers each bound feature file as a Cargo
  rebuild dependency"). It cannot notice a *newly added* `.feature` file,
  because no emitted binding references a file that did not exist at expansion
  time. Closing that needs a `cargo::rerun-if-changed=<dir>` directive, which
  needs a build script, which needs either a new published helper crate
  (`rstest-bdd-build`, ADR-010's Option B) or a documented hand-written
  `build.rs` recipe.
  Options: (a) ship per-file tracking now, document the residual
  addition/deletion gap in the users' guide and design document, and propose a
  follow-up roadmap item for the build-script helper; (b) also ship
  `rstest-bdd-build` in this change, accepting a new published crate and the
  release-surface change that entails (trips the *new published crate*
  tolerance); (c) ship per-file tracking plus a documented copy-paste `build.rs`
  recipe in the users' guide, with no new crate.
  Recommendation: (c) — it closes the gap for anyone who wants it, costs no
  release surface, and keeps 10.3.3 within its stated finish line. Option (b)
  is then a clean follow-up if adoption shows the recipe is too fiddly.
  Date/Author: 2026-08-15, planning agent. **Awaiting ruling.**

- **Decision D3 (OPEN — needs a ruling before Milestone 5): whether removing
  the pre-existing absolute `__RSTEST_BDD_FEATURE_PATH` literal is in scope.**
  The roadmap finish line says "no absolute `CARGO_MANIFEST_DIR` path appears
  in the artefact". Read narrowly, that constrains only what *this change*
  introduces, and Milestone 5 is unnecessary. Read literally, it is an
  acceptance criterion about the artefact, and today's artefact fails it for a
  reason that predates this work. ADR-010's decision drivers ("Do not embed
  absolute paths into compiled artefacts") support the literal reading. Both
  macros are affected and both must change together.
  Recommendation: in scope, sequenced last so it cannot block the core fix.
  The measured cost is lower than first feared: no checked-in `.stderr` or
  `.snap` expectation carries a real macro-emitted absolute path, and no
  runtime code re-opens the file. The visible change is to runtime failure text
  and reporter output (relative instead of absolute feature
  paths in failure messages). That change in wording is precisely what the
  finish line's "redacted `insta` snapshot with semantic assertions pins any
  touched diagnostic wording" clause anticipates.
  Date/Author: 2026-08-15, planning agent. **Awaiting ruling.**

- **Decision D4: handle a user-supplied *absolute* `path =` argument by
  emitting no tracking binding, and warn.**
  Rationale: `#[scenario(path = "/abs/x.feature")]` cannot be expressed as
  `concat!(env!("CARGO_MANIFEST_DIR"), …)`, and writing the absolute literal
  directly would breach Constraint 2. An absolute `path =` is already
  non-portable, so the honest behaviour is to skip tracking and say so at
  compile time rather than silently regress invalidation. The diagnostic must
  name the path and explain the consequence.
  Date/Author: 2026-08-15, planning agent.

## Context and orientation

Everything below is in the repository at the root of this working tree. You
need no other checkout.

### The crates

`crates/` holds the workspace members. Three matter here.

`crates/rstest-bdd-macros` is the procedural-macro crate — the code that runs
*inside the compiler* and generates tests. This is where almost all of the
change lives. Its public entry points are in
`crates/rstest-bdd-macros/src/lib.rs`: the `scenario` attribute macro at around
line 150, delegating to `macros::scenario`
(`crates/rstest-bdd-macros/src/macros/scenario/mod.rs:64`), and the `scenarios!`
function-like macro at around line 207, delegating to `macros::scenarios`
(`crates/rstest-bdd-macros/src/macros/scenarios/mod.rs:199`).

`crates/rstest-bdd` is the runtime library the generated code calls into. It
also owns the workspace's compile-time test suites: `trybuild` fixtures,
`.feature` files for the project's own behavioural tests, and `insta`
snapshots.

`crates/cargo-bdd` is a diagnostic command-line tool. It matters only as
*precedent*: `crates/cargo-bdd/tests/cli.rs` is the existing, working example of
an integration test that spawns `cargo` against a fixture crate, and the new
regression test copies its shape.

### How a feature file is found today

Both macros resolve the user's `path =` argument against the
`CARGO_MANIFEST_DIR` environment variable, which Cargo sets to the directory
containing the consuming crate's `Cargo.toml`.

`crates/rstest-bdd-macros/src/parsing/feature/mod.rs:150` does the join:

```rust
let feature_path = std::env::var("CARGO_MANIFEST_DIR")
    .map_or_else(|_| PathBuf::from(path), |dir| PathBuf::from(dir).join(path));
```

`crates/rstest-bdd-macros/src/macros/scenarios/mod.rs:56` has the companion
`resolve_manifest_directory`, which errors with `"CARGO_MANIFEST_DIR is not set.
This macro must run within Cargo."` when the variable is missing.

The file is then read — by `gherkin::Feature::parse_path` at
`crates/rstest-bdd-macros/src/parsing/feature/mod.rs:175`, and by a
`std::fs::read_to_string` at line 178 on the error-recovery path. Neither is
visible to Cargo. Before any read, `validate_feature_file_exists`
(`crates/rstest-bdd-macros/src/parsing/feature/mod.rs:121`) produces the
user-facing diagnostics:

```text
feature file not found: {path}
feature path is not a file: {path}
failed to access feature file ({path}): {err}
```

For `scenarios!`, a companion directory diagnostic lives in
`crates/rstest-bdd-macros/src/utils/errors.rs:24` (`normalized_dir_read_error`).

There is **no `build.rs` anywhere in the workspace**, no
`cargo::rerun-if-changed` directive in any source file, and no use of
`proc_macro::tracked_path`. The only `include_str!` calls that name a
`.feature` file are hand-written lines in trybuild fixtures under
`crates/rstest-bdd/tests/fixtures_macros/`.

### How the generated code carries the feature path

`crates/rstest-bdd-macros/src/macros/scenario/paths.rs:113`
(`canonical_feature_path`) joins `CARGO_MANIFEST_DIR`, canonicalizes through
`cap-std`, caches the result, and returns an **absolute** `String`.
`create_scenario_literals`
(`crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs:143`) wraps it in a
`syn::LitStr` without touching the path, and
`crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs:261` emits it:

```rust
const __RSTEST_BDD_FEATURE_PATH: &str = #feature_literal;
```

That constant is read at runtime by the harness for scenario context and by the
step executor for diagnostics.

`scenarios!` reaches the same place by a different route, and this trips people
up. `process_feature_file`
(`crates/rstest-bdd-macros/src/macros/scenarios/mod.rs:94`) *does* compute a
manifest-relative `rel_path` with `strip_prefix` — but
`generate_scenario_test`
(`crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs:307`)
immediately re-absolutizes it:

```rust
let feature_path = ctx.manifest_dir.join(ctx.rel_path).display().to_string();
```

So both macros emit absolute literals, `#[scenario]` symlink-resolved through
`cap-std` and `scenarios!` merely string-joined. Milestone 5 reconciles them,
subject to Decision D3, and must change both sites together. The good news for
Milestone 3 is that `ctx.rel_path` is already exactly the manifest-relative
value the tracking binding needs.

### The gates

`make check-fmt` runs `cargo fmt --all -- --check` and `ruff format --check`.

`make lint` runs `cargo clippy --workspace --all-targets --all-features --
-D warnings`, then `cargo doc --workspace --no-deps` with
`RUSTDOCFLAGS="--cfg docsrs -D warnings"`, then `make lint-whitaker` and
`make lint-python`, then four Python structural checkers:
`scripts/check_rs_file_lengths.py` (400-line cap per `.rs` file, with
`scripts/rs-length-allowlist.txt`), `scripts/check_users_guide_links.py`,
`scripts/check_gpui_mapping_table.py`, and
`scripts/check_serial_nextest_matrix.py` (keeps the duplicated `#[serial]`
runner-behaviour table in `docs/users-guide.md` and `docs/rstest-bdd-design.md`
byte-identical after whitespace normalization).

`make test` builds the `cargo-bdd` and `todo-cli` binaries, then runs
`cargo nextest run --workspace --all-targets --all-features` (falling back to
`cargo test`), then `cargo test --doc`, then `uv run pytest scripts/tests`,
all with `RUSTFLAGS="-D warnings"`.

`.config/nextest.toml` sets a 60-second per-test slow timeout, a **5-minute
global timeout for the entire run**, and a `cargo-spawning` test-group with
`max-threads = 1` that already carries `binary_id(cargo-bdd::cli)` at a
180-second timeout and the three `trybuild`/`macro_compile` binaries at 300
seconds.

`.github/workflows/ci.yml` runs four legs: two on `ubuntu-latest` (with
nextest and the lint tooling) and two on `windows-latest` (with plain
`cargo test`, no nextest, most tool steps skipped). There is **no macOS leg**.

### The existing cargo-spawning test pattern

`crates/cargo-bdd/tests/cli.rs:35` is the model to copy:

```rust
let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimal");
// Reuse the workspace target directory so path dependencies (rstest-bdd and
// rstest-bdd-macros) are already compiled before invoking `cargo bdd`.
let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
```

Its fixture, `crates/cargo-bdd/tests/fixtures/minimal/`, is kept out of the
workspace by a trailing empty `[workspace]` stanza in its `Cargo.toml`, depends
on `rstest-bdd` and `rstest-bdd-macros` by relative path, and has a committed
`Cargo.lock`. The root `Cargo.toml` has no `exclude` key; fixture crates opt
out purely with that `[workspace]` stanza. Every test in `cli.rs` is
`#[serial]`.

### The trybuild environment

`crates/rstest-bdd/tests/trybuild_macros.rs` drives the compile tests; its
`step_macros_compile` test is skipped only under nextest on Windows.
`crates/rstest-bdd/tests/trybuild_macros/staging.rs` copies `.feature` files
from `tests/features` and `tests/fixtures_macros` into
`<target>/tests/trybuild/features` and `<target>/tests/trybuild/rstest-bdd`.
When `trybuild` compiles a fixture, the crate root it sees — and therefore
`CARGO_MANIFEST_DIR` — is `<target>/tests/trybuild/rstest-bdd`. The checked-in
`crates/rstest-bdd/tests/fixtures_macros/scenario_missing_file.stderr` confirms
this:

```text
error: feature file not found: $WORKSPACE/target/tests/trybuild/rstest-bdd/tests/features/does_not_exist.feature
```

### Skills and documents to load before starting

Load these; they encode conventions this plan assumes.

- `leta` — semantic code navigation. Use `leta show <symbol>` instead of
  reading whole files, and `leta refs <symbol>` instead of grepping for
  usages. Add this worktree with `leta workspace add <repo root>` first.
- `rust-router` — routes to the smallest useful Rust skill. From it you will
  want `rust-unit-testing` (rstest fixtures, table tests, `serial_test`,
  `insta`) and `arch-decision-records` (the Y-Statement ADR format) for the
  ADR-010 amendment.
- `nextest` — for the `.config/nextest.toml` test-group and slow-timeout
  override added in Milestone 1.
- `proptest` — for the path-normalization property test in Milestone 2.
- `execplans` — this document's own conventions; re-read before revising it.
- `commit-message` — file-based commit messages, never `-m`.
- `en-gb-oxendict` — British English with Oxford `-ize` spelling, which the
  existing documentation follows ("canonicalization", "artefact").

Read these documents:

- `AGENTS.md` — repository-wide agent guidance.
- `docs/adr-010-feature-file-change-detection.md` — the governing decision
  record, especially its *Testing strategy*.
- `docs/rstest-bdd-design.md` §2.7.6.6 (line 2153) — the design-document
  statement of the foot-gun, and §3.2.2 (line 2453) for the orthogonal
  OUT_DIR-caching concern that must **not** be conflated with invalidation.
- `docs/v0-6-0-migration-guide.md` — the "Feature-file edits do not trigger a
  rebuild" caveat at line 714, which this work retires.
- `docs/documentation-style-guide.md`, `docs/developers-guide.md`,
  `docs/testing-strategy.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` — the house
  guidance on keeping generated-code helpers small, relevant to the 400-line
  cap.
- `docs/rust-testing-with-rstest-fixtures.md` and `docs/rust-doctest-dry-guide.md`.
- `docs/gherkin-syntax.md` — for the `.feature` file added in Milestone 1.

## Plan of work

The work is staged so that the expensive, risky end-to-end test is written and
seen to fail *first*, the minimal macro change makes it pass, and the
scope-adjacent cleanup comes last where it can be dropped without harm.

### Milestone 0 — orientation and go/no-go (no code changes)

Read the documents listed above. Confirm the three open decisions D1, D2, and
D3 have rulings. Reproduce the two measurements in *Artefacts and notes* on
your own machine so you trust them. Do not edit tracked files in this
milestone.

Go/no-go: do not proceed to Milestone 1 until D1, D2, and D3 are settled and
recorded in the *Decision Log* with the ruling and its date.

### Milestone 1 — red

Add the failing tests before any production change. Three artefacts, in this
order.

**1a. The regression fixture crate.** Create
`crates/rstest-bdd/tests/fixtures/rebuild_invalidation/` as a self-contained,
non-workspace crate modelled exactly on
`crates/cargo-bdd/tests/fixtures/minimal/`:

- `Cargo.toml` with `edition = "2024"`, path dependencies on `rstest-bdd` and
  (as a dev-dependency) `rstest-bdd-macros`, `rstest` as a dev-dependency, and
  a trailing empty `[workspace]` stanza.
- A committed `Cargo.lock`.
- `src/lib.rs` — a doc comment and nothing else.
- `tests/features/invalidation.feature` — one scenario whose `Then` step
  asserts a specific value.
- `tests/invalidation.rs` — `#[given]`/`#[when]`/`#[then]` steps plus a
  `#[scenario(path = "tests/features/invalidation.feature")]` test. The `Then`
  step must compare against a value taken from the Gherkin text (a step
  argument), so that editing the `.feature` file genuinely changes the
  expectation rather than merely changing prose.

**1b. The regression test itself.** Create
`crates/rstest-bdd/tests/feature_rebuild_invalidation.rs` with a supporting
module directory `crates/rstest-bdd/tests/feature_rebuild_invalidation/` if it
exceeds 400 lines. It must:

1. Copy the fixture crate to a stable scratch directory under the workspace
   `target/` — `target/tests/rebuild-invalidation/fixture` — so the checked-in
   tree is never mutated and the run is idempotent. Copy, do not symlink.
2. Set `CARGO_TARGET_DIR` to the workspace `target/` directory so
   `rstest-bdd` and its dependency tree are already warm, exactly as
   `crates/cargo-bdd/tests/cli.rs` does.
3. Run `cargo test` in the scratch copy and assert it **passes**.
4. Assert the emitted dep-info file for the fixture's test binary lists the
   `.feature` file. Locate it by globbing
   `<target>/debug/deps/invalidation-*.d`. This is the direct, cheap proof of
   the tracking contract and does not depend on rebuild timing at all.
5. Rewrite **only** `tests/features/invalidation.feature` in the scratch copy
   so the `Then` step expects a different value, then set its modification time
   to `SystemTime::now() + Duration::from_secs(2)` with
   `std::fs::File::set_modified` to defeat coarse mtime granularity.
6. Run `cargo test` again and assert it **fails**, and that the failure output
   names the *new* expectation. Asserting on the new expectation is what
   distinguishes a genuine rebuild from an incidental failure.

Mark the test `#[serial]` (from `serial_test`, already a workspace
dev-dependency). Add an override block to `.config/nextest.toml` putting
`binary_id(rstest-bdd::feature_rebuild_invalidation)` in the `cargo-spawning`
test-group with a 300-second slow timeout, alongside the existing
`trybuild`/`macro_compile` entry.

**1c. The behavioural specification.** Add
`crates/rstest-bdd/tests/features/rebuild_invalidation.feature` and drive it
with `#[scenario]` from the same test binary, so the contract is stated in
Gherkin as well as asserted in Rust:

```gherkin
Feature: Feature-file rebuild invalidation

  Scenario: A bound feature file is a tracked build dependency
    Given a scenario crate bound to a feature file
    When the crate is compiled
    Then the dep-info for the test binary lists the feature file

  Scenario: Editing only a feature file forces a rebuild and a fresh failure
    Given a scenario crate bound to a feature file that passes its test
    When only the feature file is edited to change the expectation
    Then the next test run recompiles the scenario binary
    And the test fails against the new expectation
```

**1d. The expansion assertion.** Add a unit test in `rstest-bdd-macros` that
runs the scenario codegen for a representative input and asserts on the emitted
`proc_macro2::TokenStream`: it contains `include_str!`, `concat!`, and
`env!("CARGO_MANIFEST_DIR")`, and it contains **no** absolute path literal.
This is the deterministic guard for Constraint 2; it does not depend on build
profile or debug-info settings, which the measurements showed make a raw
"grep the binary" assertion unreliable.

**Red evidence.** Run the two new binaries and capture the output. Both must
fail, and for the right reason: the regression test because the second
`cargo test` still *passes* (stale binary), and the expansion assertion because
no `include_str!` is emitted.

```bash
cargo nextest run -p rstest-bdd --test feature_rebuild_invalidation \
  2>&1 | tee /tmp/red-$(git branch --show-current).out
cargo nextest run -p rstest-bdd-macros scenario_emits_tracking \
  2>&1 | tee -a /tmp/red-$(git branch --show-current).out
```

Commit the red state.

### Milestone 2 — green for `#[scenario]`

Add a small, focused module — `crates/rstest-bdd-macros/src/codegen/scenario/tracking.rs`
is the natural home — exposing a single function that turns a user-supplied
`path =` argument into the tracking binding. The prescriptive signature is in
*Interfaces and dependencies* below.

The function must:

- Normalize the relative path: collapse `.` segments, keep `..` segments
  (they are legal and `rustc` resolves them), and emit `/` as the separator on
  every platform.
- Return `None` when the path is absolute (Decision D4), so the caller can emit
  the accompanying warning.
- Produce exactly this, and nothing else:

```rust
const _: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #rel));
```

Call it from `crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs`,
emitting the binding immediately beside the existing
`const __RSTEST_BDD_FEATURE_PATH` line inside the generated function body. A
function-body `const _` is proven to register in dep-info and to survive
`#![deny(warnings)]` (transcript C).

Order matters: the existing `validate_feature_file_exists` check runs *before*
codegen, so a missing file still produces exactly one diagnostic and no
`include_str!` is ever emitted for a file that does not exist. Verify this
rather than assuming it; a second, `include_str!`-generated "file not found"
error would be a regression in diagnostic quality.

Add unit tests in the new module using `rstest` table cases for the
normalization behaviour, and one `proptest` property: for any sequence of
non-empty, non-absolute path components, the produced relative literal never
begins with `/` or a Windows drive prefix, never contains `\`, and round-trips
to the same component sequence. `proptest` is already a dev-dependency of
`rstest-bdd-macros`.

Validation: the expansion assertion from 1d goes green; the regression test
from 1b goes green for the `#[scenario]` path.

### Milestone 3 — green for `scenarios!`, plus trybuild fixtures

Emit the same binding from the `scenarios!` code path. `ScenarioTestContext`
already carries a manifest-relative `rel_path` (from `strip_prefix` at
`crates/rstest-bdd-macros/src/macros/scenarios/mod.rs:99`), so it feeds the
same helper directly — no new path arithmetic is needed here. Do **not** touch
the re-absolutization at
`crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs:307` in this
milestone; that belongs to Milestone 5.

Add the two required `trybuild` fixtures under
`crates/rstest-bdd/tests/fixtures_macros/`:

- **Compile-pass**: `scenario_feature_tracking.rs`, binding a staged
  `.feature` file through `#[scenario]` and a directory through `scenarios!`,
  proving the emitted binding compiles cleanly in the staged trybuild crate
  root and does not collide when two scenarios bind the same file. Register it
  in `run_passing_macro_tests` in
  `crates/rstest-bdd/tests/trybuild_macros.rs`.
- **Compile-fail**: the existing `scenario_missing_file.rs` and its `.stderr`
  already pin the missing-`.feature` diagnostic. Confirm the `.stderr` is
  **unchanged** by this work. If it changes, the change has regressed the
  diagnostic and you should stop and investigate rather than re-blessing the
  file.

Also remove the now-redundant hand-written `const _: &str =
include_str!("basic.feature");` from at least one existing fixture (for
example `scenario_missing_name.rs`) and confirm the suite still behaves. That
is the cheapest honest evidence that the macro-emitted binding does the job the
manual line was doing.

If Decision D2 resolved to option (c), add the documented `build.rs` recipe to
`docs/users-guide.md` in Milestone 6 — not here.

### Milestone 4 — the redacted diagnostic snapshot

Add an `insta` snapshot test pinning the rendered missing-`.feature`
diagnostic. Follow the house redaction convention seen at
`crates/rstest-bdd-server/src/handlers/diagnostics/publish.rs:198` and
`crates/rstest-bdd-harness-gpui/tests/scenario_name_in_logs.rs:216`: build an
`insta::Settings` from `Settings::clone_current()`, `add_filter` over the
absolute path prefix, line and column numbers, and any rustc version string,
then `settings.bind(|| insta::assert_snapshot!(...))`.

Back the snapshot with explicit semantic assertions on the load-bearing
fragments — that the message contains `feature file not found`, names the
offending `.feature` file name, and points at the `#[scenario]` call site — so
the test fails loudly on a meaning change even where a reflow would let a
whole-text snapshot drift unnoticed. ADR-010's *Testing strategy* item 3
requires exactly this pairing.

`insta` is already a dev-dependency of `crates/rstest-bdd`; the `filters`
feature is enabled in `crates/rstest-bdd-server` and may need enabling for
`crates/rstest-bdd` too. Enabling an existing dependency's feature is not a new
dependency and does not trip the tolerance.

### Milestone 5 — remove the absolute path from the artefact (conditional on D3)

Two sites must change together, or the macros will disagree:

1. `crates/rstest-bdd-macros/src/macros/scenario/paths.rs` — add a
   manifest-relative accessor and feed *that* to `create_scenario_literals`.
   Do not repurpose `canonical_feature_path` itself: it is still needed for
   compile-time diagnostics (errors should name a path the user can click) and
   its memoization cache is keyed on the absolute form.
2. `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs:307` —
   drop the `ctx.manifest_dir.join(ctx.rel_path)` re-absolutization and pass
   `ctx.rel_path` straight through.

Normalize both to `/` separators so the emitted literal is identical on Windows
and POSIX.

Before editing, re-run the blast-radius survey: grep
`crates/**/tests/**/*.stderr`, every `**/snapshots/*.snap`, macro-crate unit
tests, and doctests for absolute feature paths. Planning found no checked-in
expectation carrying a real macro-emitted absolute path, but confirm that
before trusting it. Update whatever does change deliberately, one at a time,
reading each diff. Do not bulk-re-bless.

Update the doc comments on `ScenarioMetadata::feature_path`
(`crates/rstest-bdd/src/reporting/record.rs:8`),
`ExecutionError::MissingFixturesDetails::feature_path`
(`crates/rstest-bdd/src/execution/error.rs:208`), and the `ScenarioRecord`
accessor to state the contract explicitly: the path is relative to the
consuming crate's manifest directory. None of them says anything today, which
is how the two macros drifted apart in the first place.

Add an assertion that a **release-profile** build of a scenario test binary
contains no absolute manifest path. Use the release profile specifically:
measurement showed dev-profile binaries contain the manifest path in *every*
crate, tracked or not, purely from debug info, so a dev-profile assertion would
be vacuous or falsely red.

If D3 rules this out of scope, delete this milestone, record the ruling, and
instead add a note to `docs/known-issues.md` describing the residual absolute
path and pointing at a proposed follow-up roadmap item.

### Milestone 6 — documentation and roadmap

- `docs/adr-010-feature-file-change-detection.md`: status Proposed →
  Accepted. Record the chosen mechanism, and add the correction from Decision
  D0a distinguishing "macro writes an absolute literal" (rejected, correctly)
  from "macro emits `env!` and lets `rustc` build the path" (adopted), with the
  measured evidence. Record `tracked_path` as still unstable and still the
  long-term answer. Follow the `arch-decision-records` skill's format.
- `docs/rstest-bdd-design.md` §2.7.6.6: rewrite from "here is a foot-gun and
  two candidate mechanisms" to "here is what ships and why", including the
  residual `scenarios!` addition/deletion gap. Leave §3.2.2's OUT_DIR-caching
  discussion untouched except to keep its cross-reference accurate;
  invalidation and caching must stay distinct.
- `docs/v0-6-0-migration-guide.md`: delete the "Feature-file edits do not
  trigger a rebuild" section at line 714 — its own note says it can be removed
  once the fix ships — and, if that section is linked from elsewhere, fix the
  links. Run `scripts/check_users_guide_links.py` afterwards.
- `docs/users-guide.md`: state that editing a `.feature` file now rebuilds, and
  document the residual `scenarios!` gap plus (if D2 chose option (c)) the
  `build.rs` recipe. If you touch the `#[serial]` runner-behaviour table, keep
  it byte-identical with the copy in the design document or
  `scripts/check_serial_nextest_matrix.py` will fail.
- `docs/developers-guide.md`: document two internal conventions — the
  cargo-spawning fixture-crate test pattern (non-workspace fixture, shared
  `CARGO_TARGET_DIR`, `#[serial]`, `cargo-spawning` nextest group), and the
  invariant that macro-emitted token streams must contain no absolute path
  literal, with a pointer to the expansion assertion that enforces it.
- `docs/roadmap.md`: tick 10.3.3 to `[x]`. If D2 or D3 deferred anything, add
  the follow-up item in the same edit, using `mapsplice` for the structural
  change so numbering and `Requires` references stay consistent.
- Consider a `docs/CHANGELOG.md` entry via the `changelog` skill.

## Concrete steps

Run everything from the repository root:
`/home/leynos/.lody/repos/github---leynos---rstest-bdd/worktrees/2efe7d4e-4101-4859-8b92-9cefa53bc36f`.

Set up the log filename template once per shell:

```bash
LOGBASE="/tmp/$(git branch --show-current)"
```

Focused loops during development (fast, use these constantly):

```bash
cargo nextest run -p rstest-bdd-macros 2>&1 | tee "$LOGBASE-macros.out"
cargo nextest run -p rstest-bdd --test feature_rebuild_invalidation 2>&1 \
  | tee "$LOGBASE-invalidation.out"
cargo nextest run -p rstest-bdd --test trybuild_macros 2>&1 \
  | tee "$LOGBASE-trybuild.out"
```

Full gates at the end of every milestone. Run them **sequentially**, never in
parallel — this environment relies on build caching and on Cargo's package-cache
lock:

```bash
make check-fmt 2>&1 | tee "$LOGBASE-checkfmt.out"
make lint      2>&1 | tee "$LOGBASE-lint.out"
make test      2>&1 | tee "$LOGBASE-test.out"
```

Prefer delegating that full gate run to the `scrutineer` subagent, which runs
them sequentially, captures each log under `/tmp`, and returns a bounded
report. When it reports a failure, read the cited log rather than re-running
the gate.

Markdown after any documentation edit — note that `make fmt` can itself
introduce `MD013`/`MD039` violations, so lint after formatting, never before:

```bash
make fmt 2>&1 | tee "$LOGBASE-fmt.out"
make markdownlint 2>&1 | tee "$LOGBASE-mdlint.out"
```

Commit after each milestone with a file-based message (per the
`commit-message` skill; never use `-m`).

## Validation and acceptance

Acceptance is behavioural. Each item below is something a human can watch
happen.

**The headline behaviour.** In the fixture crate, `cargo test` passes. Change
only `tests/features/invalidation.feature` so its `Then` step expects a
different value. `cargo test` prints a `Compiling` line and the test fails,
naming the new expectation. Before the fix, the second run prints no
`Compiling` line and the test passes.

**Red-Green-Refactor evidence.** Record all three in *Artefacts and notes*.

- *Red*: `cargo nextest run -p rstest-bdd --test feature_rebuild_invalidation`
  fails, and the failure message says the second `cargo test` run succeeded
  when it should have failed. `cargo nextest run -p rstest-bdd-macros
  scenario_emits_tracking` fails because no `include_str!` is emitted.
- *Green*: both commands pass after Milestones 2 and 3, with no other test
  changed.
- *Refactor*: after tidying, the same two commands still pass and
  `make lint` is clean.

**Compile-time contract.** `cargo nextest run -p rstest-bdd --test
trybuild_macros` passes with the new compile-pass fixture registered, and
`crates/rstest-bdd/tests/fixtures_macros/scenario_missing_file.stderr` is
byte-identical to its pre-change content.

**Diagnostic wording.** The `insta` snapshot for the missing-`.feature`
diagnostic passes with its redaction filters, and the accompanying semantic
assertions pass. `cargo insta pending-snapshots` reports nothing outstanding.

**No absolute path.** The expansion assertion proves the emitted token stream
carries no absolute path literal. If Milestone 5 is in scope, a release-profile
scenario binary additionally contains no absolute manifest path.

**Quality criteria — what "done" means.**

- Tests: `make test` green, including `cargo test --doc` and
  `uv run pytest scripts/tests`.
- Lint and format: `make check-fmt` and `make lint` green, including all four
  Python structural checkers and the Whitaker Dylint suite. Note the known
  local `no_expect_outside_tests` false positive on two test-helper files,
  which is green on CI and should be ignored as environmental.
- Portability: the Windows CI legs pass. Because there is no macOS leg,
  say so plainly in the pull request rather than implying macOS coverage.
- Performance: `make test` total wall clock stays under the 5-minute nextest
  global timeout, with margin. Record the measured figure.
- Documentation: `scripts/check_users_guide_links.py` and
  `scripts/check_serial_nextest_matrix.py` pass; `make markdownlint` clean.

## Idempotence and recovery

Every step is safe to repeat.

The regression test never mutates the checked-in fixture. It copies the fixture
to `target/tests/rebuild-invalidation/fixture` and mutates the copy, restoring
the pristine `.feature` file at the start of each run rather than at the end,
so a killed test leaves nothing that breaks the next one. Deleting
`target/tests/rebuild-invalidation/` at any time is safe and merely costs one
cold compile.

Nothing in this plan is destructive. The riskiest edits are to checked-in
`.stderr` and `.snap` expectations in Milestone 5; those are under version
control, so `git checkout -- <path>` restores them. Never bulk-re-bless
snapshots — if more than a handful change, that is a signal to stop, not to
accept.

If a milestone goes wrong, `git reset --hard` to the previous milestone's
commit. That is why each milestone commits separately.

## Artefacts and notes

Three measurements taken during planning on this host with stable `rustc`.
They are the evidence behind Decision D0 and must be reproducible before
Milestone 1 starts. Each used a scratch crate under `target/plan-scratch/`,
removed afterwards.

**Transcript A — dep-info registration and rebuild triggering.** A crate whose
only reference to the feature file is
`const _: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", "features/x.feature"));`:

```text
=== dep-info .d ===
.../target/debug/libinv.rlib: .../features/x.feature .../src/lib.rs
=== now edit ONLY the .feature file ===
   Compiling inv v0.0.0 (...)
```

The `.feature` file appears in dep-info, and a `.feature`-only edit triggers
recompilation. This is the whole mechanism.

**Transcript B — what actually lands in the artefact.** A control binary with
no `include_str!` compared against one with it, in both profiles:

```text
--- control (dev profile, debuginfo on) ---
  abs path: PRESENT      feature text: absent
--- control (release profile) ---
  abs path: absent       feature text: absent
--- withinc (dev profile, debuginfo on) ---
  abs path: PRESENT      feature text: absent
--- withinc (release profile) ---
  abs path: absent       feature text: absent
```

Two conclusions. First, the mechanism adds neither the absolute path nor the
feature text to a binary — the unused `const _` is elided, so ADR-010's
"Medium" binary-size estimate for Option A does not apply to this form.
Second, the absolute path present in dev-profile binaries comes from debug
info and is present in the *control* too, which is why the plan asserts on the
emitted token stream (always meaningful) and on release binaries (meaningful),
not on dev binaries (vacuous).

A separate run against an `rlib` *did* show both the path and the text, because
rlib metadata retains source paths and constant values for downstream
inlining. Scenario tests compile to binaries, so this does not affect the
result — but do not be alarmed if you reproduce it.

**Transcript C — a function-body `const _` is enough.** With
`#![deny(warnings)]` and the binding inside a function body rather than at item
level:

```text
dep-info lists feature file?
1
```

It compiles clean and still registers. This matters because the binding is
emitted next to the existing `const __RSTEST_BDD_FEATURE_PATH` inside the
generated test function.

**External status check.** `proc_macro::tracked_path` — rust-lang/rust#99515,
"Tracking Issue for `proc_macro::{tracked_env, tracked_path}`" — is still open
and unstabilized, behind
`#![feature(proc_macro_tracked_env, proc_macro_tracked_path)]`. It remains the
right long-term mechanism and should be revisited when it stabilizes.

## Interfaces and dependencies

No new external dependency is required (subject to Decision D1). Everything
used is already in the workspace: `syn`, `quote`, `proc-macro2` in the macro
crate; `rstest`, `proptest`, `serial_test`, `insta`, `trybuild`, `tempfile` in
the test suites.

In `crates/rstest-bdd-macros/src/codegen/scenario/tracking.rs`, define:

```rust
/// A manifest-relative feature-file path, normalized for `include_str!`.
///
/// Always uses `/` separators, never begins with a separator or a drive
/// prefix, and never contains a backslash.
pub(crate) struct TrackedFeaturePath(String);

impl TrackedFeaturePath {
    /// Normalizes a user-supplied `path = ` argument.
    ///
    /// Returns `None` when `path` is absolute, because an absolute path
    /// cannot be expressed relative to `CARGO_MANIFEST_DIR` without baking
    /// the absolute path into the emitted tokens (ADR-010, binding
    /// constraint 1).
    pub(crate) fn from_manifest_relative(path: &std::path::Path) -> Option<Self>;
}

/// Emits the Cargo rebuild-dependency binding for a bound feature file.
///
/// Produces:
/// `const _: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", REL));`
///
/// Returns an empty token stream when `path` is `None`, so callers can emit
/// the accompanying "tracking skipped" warning themselves.
pub(crate) fn feature_tracking_binding(
    path: Option<&TrackedFeaturePath>,
) -> proc_macro2::TokenStream;
```

Call `feature_tracking_binding` from
`crates/rstest-bdd-macros/src/codegen/scenario/runtime.rs`, interpolating its
output beside the existing `const __RSTEST_BDD_FEATURE_PATH` line, and from the
`scenarios!` generation path in
`crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs`.

Keep `tracking.rs` well under the 400-line cap; if the normalization logic and
its tests grow past it, move the tests into a sibling `tracking/tests.rs`
module rather than adding an allowlist entry.

## Outcomes & retrospective

To be completed at milestone boundaries and at completion. Compare the result
against the *Purpose* section: can a user edit only a `.feature` file, run
`cargo test`, and see a rebuild and a fresh failure? Record the measured
`make test` wall clock, whether the Windows CI legs passed first time, and
whether the residual `scenarios!` addition gap caused any confusion in review.
