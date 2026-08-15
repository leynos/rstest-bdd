# Make a `.feature`-only edit rebuild the scenario binary

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT — revised after a six-lens design review. One decision (D3)
remains open and needs a human ruling before Milestone 2.

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
   untouched. The one deliberate exception is the residual untrackable-path
   case in Decision D4, which is called out there and must be announced.
2. **No absolute path may be baked into the macro's emitted token stream.**
   This is binding constraint 1 of
   `docs/adr-010-feature-file-change-detection.md`. The macro must not write a
   literal such as `"/home/alice/project/tests/features/x.feature"` into the
   code it generates.
   *Scope of enforcement, stated precisely because the review found the loose
   wording misleading:* this constraint is enforced at the token-stream level
   and is verified by the Milestone 1d assertion. Artefact-level freedom from
   absolute paths is asserted for **binary and test targets only**; an `rlib`
   retains source paths and constant values in its metadata regardless of what
   this change does (see *Artefacts and notes*, transcript B). Say so wherever
   the claim is repeated; do not let it be read as a whole-artefact guarantee.
3. **Minimum supported Rust version (MSRV) stays at 1.85.** The workspace
   `Cargo.toml` declares `rust-version = "1.85"` and `edition = "2024"`. Any
   mechanism that needs a newer stable compiler is out of bounds.
4. **No nightly-only compiler feature, and no `nightly` cargo feature.**
   `proc_macro::tracked_path` remains unstable (rust-lang/rust#99515 is open
   and unstabilized). It cannot even hide behind an opt-in cargo feature,
   because `make lint` and `make test` both run `--all-features` on a stable
   toolchain and would enable it. See Decision D5.
5. **Invalidation is a tested contract, not advisory prose.** This is binding
   constraint 2 of ADR-010. A regression test must fail before the fix and pass
   after it.
6. **`make check-fmt` and `make lint` must pass at the end of every milestone;
   `make test` must pass at the end of every milestone except the red one.**
   Milestone 1 exists to produce failing tests, so requiring `make test` there
   is self-contradictory. Milestones 1 and 2 may land as separate commits but
   must be pushed together, with the captured red log recorded in *Artefacts
   and notes* as the evidence that the red stage happened.
7. **No new external dependency** without escalation. Enabling a feature on a
   dependency already in `[workspace.dependencies]`, or adding an
   already-in-workspace crate to another member's `[dev-dependencies]`, is not
   a new dependency.
8. **Do not use `/tmp` as a build target.** Scratch build output belongs under
   the repository's own `target/` directory. `/tmp` is for logs only. This is
   not merely a disk-space rule: `.cargo/config.toml` sets `[profile.dev]`
   options and `rust-toolchain.toml` pins the channel, and both are discovered
   by walking up from the child process's working directory. Keeping scratch
   inside the workspace is what makes a nested build fingerprint-compatible
   with the outer one. Do not "tidy" it into a `tempfile::TempDir`.

## Tolerances (exception triggers)

Stop and escalate — do not improvise — when any of these is reached.

- **Scope.** More than 25 files changed, or more than 900 net added lines
  across the whole change (documentation excluded).
- **New dependency.** Any addition to `[workspace.dependencies]`, or any
  dependency on a crate not already in the workspace. (Per Constraint 7, adding
  `insta` — already at `Cargo.toml:44` — to the macro crate's dev-dependencies
  does **not** trip this.)
- **New published crate.** Creating `rstest-bdd-build` or any other new
  workspace member that would be published changes the release surface.
- **Public API.** Any change to a `pub` item's signature in `crates/rstest-bdd`,
  `crates/rstest-bdd-macros`, `crates/rstest-bdd-harness`, or
  `crates/rstest-bdd-policy`.
- **Test-suite wall clock.** State the metric precisely, because "`make test`
  wall clock" conflates several things: `make test` also runs a `cargo build
  --bin`, `cargo test --doc`, and `uv run pytest`, and CI does not run
  `make test` for the Rust suite at all — it runs the `generate-coverage`
  action under instrumentation. Record three numbers: (i) nextest's own
  reported run duration from its summary line, (ii) the new test's individual
  duration, and (iii) the delta in (i) with and without the new test. Set the
  tolerance against (i), and separately record the CI coverage-leg duration
  before and after, because that is the leg that actually gates. Escalate if
  (i) approaches the configured `global-timeout`.
- **Compile-time cost of the emitted binding.** Measured during review at
  realistic scale and found to be inside noise (transcript E). The residual
  tolerance is for pathological suites: if a synthetic 100-file /
  500-scenario suite shows more than a 10% increase in `cargo build --timings`
  wall clock, stop and escalate.
- **File length.** `scripts/check_rs_file_lengths.py` caps every non-`target`
  `*.rs` file at 400 lines and refuses stale allowlist entries. If a file must
  exceed 400 lines, split it; do not add an allowlist entry without escalating.
- **Iterations.** If a milestone's tests still fail after five focused
  attempts, stop and escalate with the captured log.
- **Ambiguity.** If a choice materially changes the outcome and the plan does
  not already settle it, stop and present the options with trade-offs.

## Risks

- **Risk: the copied fixture crate's path dependencies do not resolve.**
  Severity: high. Likelihood: certain if unaddressed.
  The fixture lives at `crates/rstest-bdd/tests/fixtures/rebuild_invalidation/`
  and is copied to `target/tests/rebuild-invalidation/fixture/`. That is a
  change of directory depth, so relative `path = "../../../.."` dependencies
  overshoot or undershoot the workspace root and `cargo` fails with "failed to
  load manifest for dependency `rstest-bdd`". The precedent this plan copies,
  `crates/cargo-bdd/tests/fixtures/minimal/`, is run **in place** and therefore
  never hit this.
  Mitigation: rewrite the manifest's `path` values to absolute paths during the
  copy, derived from `env!("CARGO_MANIFEST_DIR")`, and assert afterwards that
  no `..` remains in any dependency path. The scratch manifest is not
  checked in, so an absolute path there breaches nothing — Constraint 2
  governs the macro's emitted tokens, not scratch build inputs.

- **Risk: the nested build collides with CI's coverage instrumentation — in
  two opposite directions.**
  Severity: high. Likelihood: high.
  Every CI leg runs under `cargo llvm-cov` via the shared `generate-coverage`
  action, which sets its own `CARGO_TARGET_DIR`, injects `-C instrument-coverage`
  into `RUSTFLAGS`/`CARGO_ENCODED_RUSTFLAGS`, and sets `LLVM_PROFILE_FILE`.
  Two failure modes pull in opposite directions, which is why this needs
  a stated principle rather than a rule of thumb:
  *Scrub too much* — override `CARGO_TARGET_DIR` to the plain workspace
  `target/`, or strip `RUSTFLAGS` — and the nested build's fingerprint matches
  nothing already built, so it cold-compiles the whole dependency tree twice
  on a two-core runner whose workflow already needs a "free disk space" step.
  *Propagate everything* and the child emits `.profraw` under the parent's
  `LLVM_PROFILE_FILE` pattern, polluting the merged profile — and the
  default-features Linux leg gates on the CodeScene coverage ratchet with
  `continue-on-error: false`.
  Mitigation, stated as a principle: **propagate everything that participates
  in the build fingerprint** (`CARGO_TARGET_DIR` when already set, `RUSTFLAGS`,
  `CARGO_ENCODED_RUSTFLAGS`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`) so the
  child build is warm; **redirect or strip only what causes cross-talk**
  (`LLVM_PROFILE_FILE` redirected to a scratch path under the scratch
  directory so nested coverage never merges into the parent's;
  `CARGO_MAKEFLAGS` and `CARGO_PKG_*` stripped). Measure warm and cold cost in
  Milestone 1 and record both. Note that
  `crates/cargo-bdd/tests/cli.rs` — the pattern this plan copies — does no
  environment scrubbing at all and has the same latent defect; fix it in the
  same change.

- **Risk: the fixture's feature set diverges from the existing fixture's,
  doubling the cold build.**
  Severity: medium. Likelihood: high if unaddressed.
  Cargo hashes the resolved feature set into `-C metadata`, so a fixture whose
  dependency set differs from `crates/cargo-bdd/tests/fixtures/minimal/`
  gets its own compiled units rather than sharing them. That was measured at
  ~14 s of extra first build even on a mostly-warm tree; on cold CI it is the
  whole subtree, on a runner where disk is already tight.
  Mitigation: make the new fixture's `[dependencies]` and `[dev-dependencies]`
  **byte-identical** to `minimal/Cargo.toml`'s — including
  `features = ["diagnostics", "test-support"]` — and commit the same lockfile.
  This is a constraint on Milestone 1a, not an incidental resemblance.

- **Risk: nested Cargo hangs, especially on Windows.**
  Severity: high. Likelihood: medium.
  `.github/workflows/ci.yml:43-45` disables nextest on the Windows legs
  specifically to avoid nested Cargo/nextest hangs, citing rust-lang/cargo#15744
  and nextest-rs/nextest#2463. This plan introduces a nested `cargo test` on
  **all four** legs, including the two Windows legs that workaround was written
  for. Under plain `cargo test` (the Windows configuration) the parent exports
  `CARGO_MAKEFLAGS`, so the child can inherit a jobserver whose descriptors it
  cannot open — the classic stall shape. An orphaned child holding
  `target/debug/.cargo-lock` poisons every later step in the job.
  Mitigation: strip cargo-injected variables from the child environment
  (`CARGO_MAKEFLAGS`, `CARGO_ENCODED_RUSTFLAGS` unless deliberately
  propagated, `RUSTC_WRAPPER`, `LLVM_PROFILE_FILE`, `CARGO_LLVM_COV*`,
  `CARGO_PKG_*`); invoke `env!("CARGO")` rather than a PATH lookup; impose the
  test's own wall-clock bound on the child and kill it with a clear message
  rather than relying on nextest's slow-timeout. **Pre-agreed fallback:** if
  the Windows legs hang, the expensive test gets `#[cfg_attr(windows, ignore)]`
  with the gap documented in `docs/known-issues.md` — the cheap dep-info test
  stays enabled everywhere. Do not disable anything broader.

- **Risk: nextest timeout arithmetic already does not close.**
  Severity: high. Likelihood: high.
  `.config/nextest.toml` sets `global-timeout = "5m"` (300 s) for the whole
  run. The `cargo-spawning` group has `max-threads = 1` and already carries
  four members budgeted at 300 s, 300 s, 300 s and 180 s. Adding a fifth makes
  the worst-case serialized budget roughly 780 s under a 300 s cap. A breach
  kills the *run*, not the test, so the CI failure names no test and reads as
  "CI is broken".
  Mitigation: decide this deliberately in Milestone 1 rather than discovering
  it. Either raise `global-timeout` with a measured justification recorded in
  the plan, or move the `cargo-spawning` members to `profile.long` (which
  already exists with a 15-minute global timeout). Raising a ceiling is not
  "silent" when it is measured and stated; leaving the arithmetic broken is
  the actual hazard.
  Be accurate about what the group buys, because the plan originally implied
  the wrong thing: Cargo releases the target-directory build lock *before*
  running tests (measured — an inner `cargo build` into the shared `target/`
  completed while the outer run's test binary was still alive), so correctness
  under a shared target directory comes from Cargo's own file locking, not
  from `max-threads = 1`. What the group prevents is *blocking*: a
  `Blocking waiting for file lock on build directory` stall counts fully
  against the slow-timeout. The group therefore only helps if **every**
  cargo-spawning binary is in it — and it does not exist on the Windows legs
  at all. Consider extending a Python structural checker (in the
  `scripts/check_serial_nextest_matrix.py` neighbourhood) to assert that any
  test binary spawning `cargo` is listed in the `cargo-spawning` filter, so
  the next person cannot silently omit one.

- **Risk: the trybuild staging environment resolves `CARGO_MANIFEST_DIR`
  differently from a real user crate.**
  Severity: medium. Likelihood: medium.
  `crates/rstest-bdd/tests/trybuild_macros/staging.rs` copies `.feature` files
  into `<target>/tests/trybuild/rstest-bdd/`, and `trybuild` compiles fixtures
  with that directory as the crate root.
  Mitigation: Milestone 4 adds a compile-pass fixture and asserts on its
  dep-info. The existing fixtures under
  `crates/rstest-bdd/tests/fixtures_macros/` already handwrite
  `const _: &str = include_str!("basic.feature");`, direct evidence the staged
  layout supports the construct.

- **Risk: Windows path handling — in the dep-info comparison, not the compile.**
  Severity: medium. Likelihood: medium.
  `concat!(env!("CARGO_MANIFEST_DIR"), "/", REL)` yields
  `C:\...\fixture/tests/features/x.feature` on Windows. `rustc` accepts it, but
  the `.d` file records that mixed form while a naively constructed expected
  path uses `\` throughout. `rustc` also writes make-style `.d` with
  backslash-escaped spaces, so a naive line split breaks on a path containing
  a space.
  Mitigation: normalize separators to `/` and case-fold before comparing;
  parse the `.d` with escape handling, or avoid parsing it by using
  `cargo test --no-run --message-format=json` to locate the artefact.

- **Risk: unpicking `canonical_feature_path` loses symlink resolution
  silently.**
  Severity: medium. Likelihood: medium.
  `crates/rstest-bdd-macros/src/macros/scenario/paths.rs` resolves symlinks
  through `cap-std` as a side effect of producing an absolute path, and its
  memoization cache is keyed on the absolute form.
  Mitigation: keep `canonical_feature_path` intact for diagnostics and add a
  separate relative-path accessor rather than mutating the existing function.

- **Risk: `scenarios!` cannot detect a newly *added* `.feature` file.**
  Severity: low. Likelihood: certain.
  Per-file registration tracks edits to files present when the macro last ran.
  It cannot see a file that did not exist then, because nothing references it.
  Mitigation: documented explicitly rather than hidden, in the users' guide and
  in `docs/known-issues.md`, with a follow-up roadmap item proposed. See
  Decision D2.

- **Risk: the nested fixture's `Cargo.lock` goes stale after a dependency
  bump.**
  Severity: low. Likelihood: medium.
  The repository already needed `make update-ui-lints-lock` (Makefile:139) for
  exactly this problem with the `ui_lints` nested crate. A stale lock plus
  `--locked` is a red build for an unrelated reason.
  Mitigation: add a Makefile target refreshing the fixture lock, or fold it
  into the existing one, and state whether the child runs `--locked`.

## Progress

Format: `- [x] (YYYY-MM-DDTHH:MMZ) description`. Timestamp every entry when it
is ticked so rates of progress and tolerance breaches are visible.

- [ ] Milestone 0: orientation and the D3 ruling (no code changes).
- [ ] Milestone 1: red — the two failing regression tests, the failing
      token-shape assertion, and the BDD feature specification.
- [ ] Milestone 2: green — emit the tracking item from `#[scenario]`.
- [ ] Milestone 3: green — emit the tracking items from `scenarios!`.
- [ ] Milestone 4: trybuild fixtures, including the dep-info assertion and the
      D4 diagnostic fixture.
- [ ] Milestone 5: the redacted `insta` snapshot with semantic assertions.
- [ ] Milestone 6: remove the absolute path from the embedded feature-path
      constant (conditional on D3).
- [ ] Milestone 7: documentation, ADR amendment, migration-guide caveat
      removal, roadmap tick.

## Surprises & discoveries

Recorded during planning and design review; keep appending during
implementation.

- Observation: **the `concat!(env!(…))` form embeds nothing into a compiled
  binary** — neither the absolute path nor the feature text — yet still
  registers the file in dep-info and still forces a rebuild.
  Evidence: transcripts A and B in *Artefacts and notes*. A control crate
  *without* the construct showed the same absolute-path presence in the
  dev-profile binary (it comes from debug info, not the mechanism) and both
  were clean in release.
  Impact: the load-bearing discovery. It makes the zero-friction path viable on
  MSRV 1.85 without the span gymnastics ADR-010 anticipated, and it requires
  ADR-010's rejection rationale to be amended rather than merely obeyed.

- Observation: **`include_str!` hard-errors on a non-UTF-8 file; `include_bytes!`
  does not, and registers dep-info identically.**
  Evidence: transcript D. `include_str!` produced
  ``error: `…/bad.feature` wasn't a utf-8 file`` — and note that the error text
  itself contains the absolute path. `include_bytes!` compiled clean under
  `#![deny(warnings)]` with one dep-info entry.
  Impact: the emitted binding uses `include_bytes!`. A `.feature` file that is
  not valid UTF-8 must produce the Gherkin parser's diagnostic, not a second,
  unrelated one pointing at line 0 of the feature file.

- Observation: **`proc_macro_error::emit_warning!` is nightly-only and is
  silently ignored on stable.** The workspace pins `proc-macro-error 1.0.4`
  (`crates/rstest-bdd-macros/Cargo.toml:39`, `Cargo.lock:1364`).
  Impact: the original Decision D4 ("skip tracking and warn") would have
  shipped as "skip tracking", silently — reintroducing the exact foot-gun this
  item exists to kill. D4 is rewritten accordingly. The pre-existing
  `emit_runtime_deprecation_warning` (`macros/scenarios/mod.rs:178`) has the
  same latent problem; out of scope here, worth a follow-up item.

- Observation: **`scenarios!` can parse a `.feature` file and generate zero
  tests from it**, when a `tags =` filter matches nothing
  (`macros/scenarios/mod.rs:161`, `check_empty_results`). Likewise the
  harness-delegated path replaces the generated function body entirely
  (`codegen/scenario/runtime/harness.rs`).
  Impact: a binding emitted *inside* a generated test body would track nothing
  for a filtered-out file, and would be at the mercy of whichever body
  assembler runs. The binding is therefore emitted at **item scope, once per
  bound feature file, independent of test generation**. This also removes the
  need to thread a new value through `ScenarioConfig` → `ScenarioMetadata` →
  `ScenarioLiterals`, and removes the risk of patching one of the two body
  assemblers and silently missing the other.

- Observation: **both macros already bake an absolute path into the artefact
  today**, by two different routes.
  `crates/rstest-bdd-macros/src/macros/scenario/paths.rs:113` returns an
  absolute, cap-std-canonicalized path for `#[scenario]`;
  `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs:307`
  computes a manifest-relative `rel_path` and then re-absolutizes it with
  `ctx.manifest_dir.join(ctx.rel_path)`. Both reach
  `const __RSTEST_BDD_FEATURE_PATH: &str`, emitted from **two** sites:
  `codegen/scenario/runtime.rs:261` and
  `codegen/scenario/runtime/harness.rs:38`. The latter destructures
  `ScenarioLiterals` with a `..` rest pattern, so adding a field there compiles
  clean and does nothing.
  Impact: the roadmap finish line "no absolute `CARGO_MANIFEST_DIR` path
  appears in the artefact" is not satisfiable by the invalidation work alone.
  Milestone 6 addresses it and must change both routes and both emission sites.

- Observation: **`rel_path` in `scenarios!` is not guaranteed relative.**
  `macros/scenarios/mod.rs:94-99` falls back to the absolute path when
  `strip_prefix` fails, which is reachable via symlinked feature directories
  and via the absolute directory arguments `path_resolution.rs` explicitly
  supports.
  Impact: no code may assume it is relative, and no doc comment may promise an
  unconditional relative contract.

- Observation: **nothing re-opens the feature file at runtime using the
  embedded path.** It flows into `ScenarioMetadata::new`, `ExecutionError`,
  `HarnessError::with_scenario_context`, the JSON and JUnit reporters, and
  `cargo-bdd`'s display formatting — all `Display` only. There is no
  `Path::new(feature_path)` or `PathBuf::from(feature_path)` anywhere.
  Impact: Milestone 6 is safe for runtime correctness. Its risk is entirely in
  the *serialized* surfaces.

- Observation: **the documented contract is already relative.**
  `docs/roadmap.md:316` documents the diagnostics JSON as
  `"feature": "path/to/file.feature"`. The implementation emits an absolute
  path, so it violates its own published example.
  Impact: Milestone 6 is a conformance fix, which is the cleanest
  justification for it.

- Observation: **eight `.expanded.rs` macrotest snapshots exist** under
  `crates/rstest-bdd-harness-tokio/tests/fixtures_macros/` and
  `crates/rstest-bdd-harness-gpui/tests/fixtures_macros/`, compared by
  `macrotest::expand_without_refresh` behind `RSTEST_BDD_RUN_MACROTEST`.
  Impact: they belong in the blast-radius survey, which originally covered only
  `.stderr` and `.snap`. Item-scope emission reduces but does not eliminate
  their exposure. The review also noted these snapshots appear to be curated
  excerpts already out of step with real expansion; confirm and record.

- Observation: **`googletest` and `pretty_assertions` are used nowhere in this
  workspace.** The house stack is `rstest`, `insta`, `proptest`, `serial_test`,
  `trybuild`, `macrotest`, `tempfile`, `temp-env`. See Decision D1.

## Decision log

- **Decision D0 (settled): emit, at item scope, once per bound feature file:**

  ```rust
  #[doc = "Registers the bound `.feature` file as a Cargo rebuild dependency \
           (ADR-010). Deleting this makes `.feature`-only edits silently skip \
           recompilation; see rstest-bdd::feature_rebuild_invalidation."]
  const _: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #rel));
  ```

  Rationale, point by point, because each element is load-bearing:
  - *Deferred path construction* (`concat!(env!(…))` rather than a literal)
    keeps the emitted token stream identical regardless of build directory and
    puts nothing absolute in it. Works on stable 1.85.
  - *`include_bytes!` rather than `include_str!`* avoids a second, unrelated
    compile error on a non-UTF-8 `.feature` file (transcript D). Dep-info
    registration is identical.
  - *Item scope rather than the generated function body* because `scenarios!`
    can parse a file and generate zero tests from it under a `tags =` filter,
    and because the harness path replaces the function body wholesale.
  - *Anonymous `const _`* because naming it would make the file contents
    reachable and therefore retained in the binary — the cost ADR-010 feared.
    This is fragile knowledge; hence the `#[doc]` attribute above.
  - *One binding per file, not per scenario*, to bound compile-time cost.

  Alternatives rejected: call-site-relative `include_str!` via
  `Span::local_file` needs Rust 1.88 and — more importantly — buys nothing,
  since both forms are relative and track identically, while returning `None`
  under `--remap-path-prefix` and computing the wrong offset under `#[path]`
  attributes. `proc_macro::tracked_path` is unstable (see D5). A mandatory
  build script requires every consumer to add `build.rs`.
  Date/Author: 2026-08-15, planning agent; revised after design review.

- **Decision D0a (settled): amend ADR-010 rather than silently diverge.**
  ADR-010 rejects "absolute-path `include_str!`" because it "embeds the full …
  path into the binary". Measurement shows that rationale applies to the
  variant where the *proc macro writes an absolute string literal into the
  token stream*, and not to the variant where the macro emits
  `env!("CARGO_MANIFEST_DIR")` and lets `rustc` construct the path. ADR-010's
  decision outcome explicitly defers the mechanism choice to this ExecPlan, so
  recording the correction is within remit.
  The amendment must **preserve the original rejection text verbatim** and add
  a dated adjacent subsection recording the measurement. A reader who sees the
  rejection silently vanish cannot tell whether the ADR was wrong or the
  implementation cheated. It must also correct Table 1's "Binary-size cost:
  A = Medium" row, which is measured-false for the adopted form, and note the
  rlib qualifier from Constraint 2.
  Date/Author: 2026-08-15, planning agent.

- **Decision D1 (settled): use the house testing stack; do not introduce
  `googletest` or `pretty_assertions`.**
  Neither appears anywhere in the workspace, roadmap 10.3.3's finish line says
  nothing about assertion libraries, and adding them for one feature would
  leave the suite inconsistent. Adopting them is a reasonable repository-wide
  decision, but it is a repository-wide decision, not a 10.3.3 decision. If the
  intent is a suite-wide migration, that is a separate roadmap item and should
  be raised as one.
  Date/Author: 2026-08-15, planning agent (ruled after design review).

- **Decision D2 (settled): ship per-file tracking now; document the residual
  addition/deletion gap; do not ship an untested `build.rs` recipe.**
  Per-file registration makes both macros notice *edits* to every `.feature`
  file they bound, which is the reported foot-gun and is exactly what the
  roadmap sentence asks for. It cannot notice a newly *added* file.
  A copy-paste `build.rs` recipe in the users' guide that nothing executes
  would rot within two releases, and its failure mode is silent stale tests —
  the same bug class this item exists to kill. Shipping uncompiled instructions
  for a *correctness* mechanism is worse than shipping none. The users' guide
  and `docs/known-issues.md` therefore state the gap plainly and point at a
  proposed follow-up roadmap item for a tested `rstest-bdd-build` helper.
  Note for that follow-up: ADR-010's claim that a build script "must emit one
  line per file discovered, not just the directory" is over-cautious — Cargo
  scans a `rerun-if-changed` directory recursively (rust-lang/cargo#8973).
  Correct this while amending the ADR so the follow-up is not built on a false
  premise.
  Date/Author: 2026-08-15, planning agent (ruled after design review).

- **Decision D3 (OPEN — needs a human ruling before Milestone 2): what
  `__RSTEST_BDD_FEATURE_PATH` should hold.**
  Three options, all now costed:
  1. **Manifest-relative** (the plan's default). Satisfies the roadmap finish
     line literally, and conforms to the already-documented JSON contract at
     `docs/roadmap.md:316`. Costs: a one-time discontinuity in JUnit
     `classname` (`crates/rstest-bdd/src/reporting/junit.rs:55`), which CI
     systems key test history, ownership and quarantine rules on; and a loss of
     workspace-wide uniqueness, so two crates in one workspace with the same
     conventional layout collide in merged `cargo bdd` output. The
     discontinuity is arguably owed anyway, since today's value is an absolute
     build-machine path that already differs between a laptop and CI.
  2. **Absolute, but deferred** — keep the constant absolute while building it
     the same way the tracking item does, with
     `concat!(env!("CARGO_MANIFEST_DIR"), "/", REL)` instead of a baked
     literal.
     Satisfies ADR-010's *driver* (nothing absolute in the emitted token
     stream, no build-directory divergence in source), keeps failure messages
     absolute and clickable, preserves workspace uniqueness, and needs no
     reporter churn. Fails the roadmap's *literal* finish line, because the
     absolute path still lands in the release binary.
  3. **Status quo.** Fails both the finish line and the ADR driver.
  Recommendation: option 1, on the strength of the documented-contract
  argument. **If option 1 is chosen it must ship in the same release as
  Milestones 2–4** — last in commit order, not in release order — so adopters
  absorb the churn once. It also requires a `docs/CHANGELOG.md` **Changed**
  entry naming JSON `feature_path`, JUnit `classname` and failure-message text,
  and a migration-guide note telling CI operators their JUnit history will
  discontinue once. Do not gate it behind a feature flag: two path formats
  means two permanent contracts for a `Display` string.
  Date/Author: 2026-08-15, planning agent. **Awaiting ruling.**

- **Decision D4 (settled): make the untrackable case nearly unreachable, and
  hard-error on the residue.**
  The original plan said "skip tracking and warn". That is unshippable: the
  crate's warning macro is nightly-only and silently ignored on stable, so it
  would have shipped as "skip tracking", silently.
  The fix is to remove almost all of the untrackable cases. `..` segments are
  legal in `include_bytes!`, so compute the component-wise relative offset from
  `CARGO_MANIFEST_DIR` to the target and emit
  `include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", "../../shared/x.feature"))`.
  That restores tracking for every path sharing a filesystem root — including
  a user-written absolute `path =`, and including the `scenarios!`
  `strip_prefix`-failure fallback, which is an *internal* absolutization and
  must never be reported as a user error.
  Only genuinely unrelatable roots remain: a different Windows drive letter, a
  UNC path, or a non-UTF-8 path that cannot be expressed as a `&str` literal.
  For those, emit `compile_error!` naming the path and explaining that the
  feature file cannot be registered as a rebuild dependency. Erroring is the
  right call because silent staleness is the precise hazard this item exists to
  remove, and because such a path is already non-portable.
  This is the one deliberate exception to Constraint 1. It is vanishingly rare,
  but it must be announced in the changelog and the migration guide, and pinned
  by a `trybuild` compile-fail fixture so the wording cannot drift.
  Date/Author: 2026-08-15, planning agent (rewritten after design review).

- **Decision D5 (settled): do not ship a `nightly` cargo feature for
  `proc_macro::tracked_path`, contrary to ADR-010 constraint 5.**
  Two independent reasons. First, `make lint` and `make test` both run
  `--all-features`, so the feature would be enabled on a stable toolchain by
  the project's own gates and break them immediately. Second, it buys nothing
  here: `tracked_path::path()` registers paths in dep-info, which
  `include_bytes!` already does, and it does not close the directory-addition
  gap either. Its only theoretical advantage — not loading file content into
  rustc's source map — was measured to have no artefact cost.
  Amend ADR-010 constraint 5 from "usable behind a `nightly` feature gate" to
  "not adoptable behind a feature gate while the gates run `--all-features`;
  revisit on stabilization".
  Date/Author: 2026-08-15, planning agent (added after design review).

## Context and orientation

Everything below is in the repository at the root of this working tree. You
need no other checkout.

### The crates

`crates/` holds the workspace members. Four matter here.

`crates/rstest-bdd-macros` is the procedural-macro crate — the code that runs
*inside the compiler* and generates tests. Almost all of the change lives here.
Its entry points are in `crates/rstest-bdd-macros/src/lib.rs`: the `scenario`
attribute macro at around line 150, delegating to `macros::scenario`
(`crates/rstest-bdd-macros/src/macros/scenario/mod.rs:64`), and the `scenarios!`
function-like macro at around line 207, delegating to `macros::scenarios`
(`crates/rstest-bdd-macros/src/macros/scenarios/mod.rs:199`).

`crates/rstest-bdd` is the runtime library the generated code calls into. It
owns the compile-time test suites: `trybuild` fixtures, `.feature` files for
the project's own behavioural tests, and `insta` snapshots.

`crates/rstest-bdd-harness-tokio` and `crates/rstest-bdd-harness-gpui` own the
eight `.expanded.rs` macrotest snapshots that the blast-radius survey must
cover.

`crates/cargo-bdd` is a diagnostic command-line tool. It matters as *precedent*
(`crates/cargo-bdd/tests/cli.rs` is the existing cargo-spawning integration
test) and as a *consumer* of the embedded feature path via `--dump-steps` JSON.

### How a feature file is found today

Both macros resolve the user's `path =` argument against `CARGO_MANIFEST_DIR`,
which Cargo sets to the directory containing the consuming crate's
`Cargo.toml`. `crates/rstest-bdd-macros/src/parsing/feature/mod.rs:150` does the
join:

```rust
let feature_path = std::env::var("CARGO_MANIFEST_DIR")
    .map_or_else(|_| PathBuf::from(path), |dir| PathBuf::from(dir).join(path));
```

`crates/rstest-bdd-macros/src/macros/scenarios/mod.rs:56`
(`resolve_manifest_directory`) is the companion for `scenarios!`, erroring with
`"CARGO_MANIFEST_DIR is not set. This macro must run within Cargo."` when the
variable is missing.

The file is read by `gherkin::Feature::parse_path`
(`parsing/feature/mod.rs:175`) and, on the error-recovery path, by a
`std::fs::read_to_string` at line 178. Neither is visible to Cargo. Before any
read, `validate_feature_file_exists` (`parsing/feature/mod.rs:121`) produces:

```text
feature file not found: {path}
feature path is not a file: {path}
failed to access feature file ({path}): {err}
```

There is **no `build.rs` anywhere in the workspace**, no
`cargo::rerun-if-changed` directive in any source file, and no use of
`proc_macro::tracked_path`. The only `include_str!` calls naming a `.feature`
file are handwritten lines in trybuild fixtures under
`crates/rstest-bdd/tests/fixtures_macros/`.

### How the generated code carries the feature path

`crates/rstest-bdd-macros/src/macros/scenario/paths.rs:113`
(`canonical_feature_path`) joins `CARGO_MANIFEST_DIR`, canonicalizes through
`cap-std`, caches the result, and returns an **absolute** `String`.
`create_scenario_literals` (`codegen/scenario/runtime.rs:143`) wraps it in a
`syn::LitStr` without touching the path.

The constant is then emitted from **two** places, and this is the trap:

- `codegen/scenario/runtime.rs:261` (`assemble_test_tokens`)
- `codegen/scenario/runtime/harness.rs:38`
  (`assemble_test_tokens_with_harness`, selected at `runtime.rs:326` whenever
  `harness = …` is set)

`harness.rs:24-29` destructures `ScenarioLiterals` with a `..` rest pattern, so
adding a field there compiles clean and silently does nothing. Item-scope
emission (Decision D0) sidesteps this entirely — but Milestone 6 still has to
touch both sites.

`scenarios!` reaches the same constant by its own route:
`process_feature_file` (`macros/scenarios/mod.rs:94`) computes `rel_path` with
`strip_prefix`, falling back to the **absolute** path on failure, and
`generate_scenario_test` (`macros/scenarios/test_generation.rs:307`)
re-absolutizes it with `ctx.manifest_dir.join(ctx.rel_path)`.

### The gates

`make check-fmt` runs `cargo fmt --all -- --check` and `ruff format --check`.

`make lint` runs `cargo clippy --workspace --all-targets --all-features --
-D warnings`, `cargo doc --workspace --no-deps` with
`RUSTDOCFLAGS="--cfg docsrs -D warnings"`, `make lint-whitaker`,
`make lint-python`, and four Python structural checkers:
`scripts/check_rs_file_lengths.py` (400-line cap, with
`scripts/rs-length-allowlist.txt`), `scripts/check_users_guide_links.py`,
`scripts/check_gpui_mapping_table.py`, and
`scripts/check_serial_nextest_matrix.py`.

`make test` builds the `cargo-bdd` and `todo-cli` binaries, runs
`cargo nextest run --workspace --all-targets --all-features` (falling back to
`cargo test`), then `cargo test --doc`, then `uv run pytest scripts/tests`, all
with `RUSTFLAGS="-D warnings"`.

`make markdownlint` runs `markdownlint-cli2` plus the en-GB spelling and phrase
gates. `make nixie` validates Mermaid diagrams. `make publish-check` packages
the crates in release order.

`.config/nextest.toml` sets a 60-second per-test slow timeout, a **5-minute
global timeout for the entire run**, and a `cargo-spawning` test-group with
`max-threads = 1` carrying `binary_id(cargo-bdd::cli)` at 180 s and the three
`trybuild`/`macro_compile` binaries at 300 s.

`.github/workflows/ci.yml` runs four legs — two `ubuntu-latest` (nextest plus
lint tooling) and two `windows-latest` (plain `cargo test`, no nextest, most
tool steps skipped). **Every leg sets `coverage: true`** and runs through the
shared `generate-coverage` action, i.e. `cargo llvm-cov`. There is no macOS
leg. The Windows legs disable nextest citing rust-lang/cargo#15744 and
nextest-rs/nextest#2463 — nested Cargo hangs.

### The existing cargo-spawning test pattern

`crates/cargo-bdd/tests/cli.rs:35` is the shape to copy, with the caveats in
*Risks*:

```rust
let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimal");
// Reuse the workspace target directory so path dependencies (rstest-bdd and
// rstest-bdd-macros) are already compiled before invoking `cargo bdd`.
let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
```

Its fixture, `crates/cargo-bdd/tests/fixtures/minimal/`, is kept out of the
workspace by a trailing empty `[workspace]` stanza, depends on `rstest-bdd` by
relative path, and has a committed `Cargo.lock`. Crucially it is run **in
place**, so it never hits the copy-depth problem this plan does. Every test in
`cli.rs` is `#[serial]`.

Note that `#[serial]` alone buys nothing under nextest: `serial_test = "2"` is
declared without the `file_locks` feature, so its lock is in-process only and
nextest runs each test in its own process. The repository's own table at
`docs/users-guide.md:1676` says exactly this. The `cargo-spawning` test-group
is what actually serializes. Under plain `cargo test` — the Windows legs —
neither applies, and the only protection is that Cargo runs test binaries
sequentially.

### The trybuild environment

`crates/rstest-bdd/tests/trybuild_macros.rs` drives the compile tests;
`step_macros_compile` is skipped only under nextest on Windows.
`crates/rstest-bdd/tests/trybuild_macros/staging.rs` copies `.feature` files
into `<target>/tests/trybuild/features` and `<target>/tests/trybuild/rstest-bdd`.
When `trybuild` compiles a fixture, `CARGO_MANIFEST_DIR` is
`<target>/tests/trybuild/rstest-bdd`, as the checked-in
`scenario_missing_file.stderr` confirms:

```text
error: feature file not found: $WORKSPACE/target/tests/trybuild/rstest-bdd/tests/features/does_not_exist.feature
```

Fixtures are registered in `run_passing_macro_tests` (pass list) or
`run_failing_macro_tests` (fail list) in `trybuild_macros.rs`. Only pass-list
fixtures actually reach codegen — a compile-fail fixture aborts before the
macro emits anything, so it can never demonstrate emitted tokens.

### Skills and documents to load before starting

- `leta` — semantic code navigation. `leta workspace add <repo root>` first,
  then `leta show <symbol>` instead of reading whole files and `leta refs
  <symbol>` instead of grepping for usages.
- `rust-router` — routes to the smallest useful Rust skill; from it,
  `rust-unit-testing` and `arch-decision-records`.
- `nextest` — for the test-group and timeout changes.
- `proptest` — for the path-normalization property test.
- `execplans` — this document's own conventions; re-read before revising it.
- `commit-message` — file-based commit messages, never `-m`.
- `en-gb-oxendict` — British English with Oxford `-ize` spelling, which the
  existing documentation follows ("canonicalization", "artefact"). Note the
  repository is internally inconsistent on the possessive: `AGENTS.md` says
  "users' guide", `docs/documentation-style-guide.md` says "user's guide". This
  plan follows `AGENTS.md`; do not "correct" it.

Read: `AGENTS.md`; `docs/adr-010-feature-file-change-detection.md` (especially
its *Testing strategy*); `docs/rstest-bdd-design.md` §2.7.6.6 (line 2153) and
§3.2.2 (line 2453, the orthogonal OUT_DIR-caching concern that must **not** be
conflated with invalidation); `docs/v0-6-0-migration-guide.md` line 714;
`docs/documentation-style-guide.md`; `docs/developers-guide.md`;
`docs/testing-strategy.md`; `docs/repository-layout.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`;
`docs/rust-testing-with-rstest-fixtures.md`; `docs/rust-doctest-dry-guide.md`;
`docs/gherkin-syntax.md`.

## Plan of work

### Milestone 0 — orientation and the D3 ruling (no code changes)

Read the documents above. Reproduce transcripts A–D on your own machine so you
trust them. Obtain the ruling on Decision D3 and record it in the *Decision
Log* with its date. D1, D2, D4 and D5 are settled; do not reopen them without
new evidence.

Go/no-go: do not start Milestone 1 until D3 is recorded.

### Milestone 1 — red

Three artefacts, in this order.

**1a. The regression fixture crate.** Create
`crates/rstest-bdd/tests/fixtures/rebuild_invalidation/` as a self-contained,
non-workspace crate: `edition = "2024"`, a trailing empty `[workspace]` stanza,
a committed `Cargo.lock`, a doc-comment-only `src/lib.rs`,
`tests/features/invalidation.feature`, and `tests/invalidation.rs` with the
step functions and one `#[scenario]` test.

Its `[dependencies]` and `[dev-dependencies]` must be **byte-identical** to
`crates/cargo-bdd/tests/fixtures/minimal/Cargo.toml`'s, including
`features = ["diagnostics", "test-support"]` on `rstest-bdd`, with the same
committed lockfile. This is a hard requirement, not a resemblance: Cargo hashes
the resolved feature set into `-C metadata`, so a divergent set gets its own
compiled units and CI pays the cold cost twice. See the corresponding entry in
*Risks*.

The `Then` step must compare against a value **captured from the Gherkin text
as a step argument**, so editing the `.feature` file genuinely changes the
expectation. Put a comment in the `.feature` file warning that the edit
performed by the test changes only that captured value, never step keyword or
pattern text — two CI legs build with `strict-compile-time-validation`, under
which a pattern-text change would fail as a *compile* error and the "output
names the new expectation" assertion would not hold.

Confirm `make publish-check` stays green with the new fixture present:
`crates/rstest-bdd`'s manifest has no `include`/`exclude`, so the fixture is
packaged into the `.crate`. Do this in Milestone 1, not at the end.

**1b. Two separate regression tests**, in
`crates/rstest-bdd/tests/feature_rebuild_invalidation.rs` (with a supporting
module directory if it approaches 400 lines). They are split so each goes red
for its own stated reason and so the cheap one survives if the expensive one is
ever `#[ignore]`d:

*Test 1 — the dep-info contract (cheap).* Build the fixture once and assert the
emitted dep-info lists the `.feature` file. Locate the artefact deterministically
with `cargo test --no-run --message-format=json` and read the `executable`
field; do **not** glob `<target>/debug/deps/invalidation-*.d`. That directory
already holds over a thousand `.d` files and two distinct hashes of
`librstest_bdd`; a glob matches stale artefacts from earlier runs, including
pre-fix ones, so the assertion can pass on a `.d` unrelated to the current
build or fail nondeterministically depending on which match it picks. Note in
a comment that Cargo's own
`target/debug/.fingerprint/<pkg>-<hash>/dep-test-<name>` is the real input and
the rustc `.d` is a proxy for it.

Assert the feature file appears in the dep-info **exactly once**. `rustc` and
Cargo deduplicate the entry today even when a file is included many times
(measured: 500 occurrences of one file produced one `.d` entry), so
`assert_eq!(count, 1)` is a stable contract — and it pins the one-binding-per-
file property from Decision D0 so a future refactor cannot quietly regress to
one binding per scenario.

*Test 2 — the rebuild behaviour (expensive).* Run `cargo test`, assert it
passes; rewrite **only** the `.feature` file's captured value; set its
modification time to `SystemTime::now() + Duration::from_secs(2)` with
`std::fs::File::set_modified` (stable since 1.75; on Windows, write the content,
drop the handle, then reopen with write access to set the time); run
`cargo test` again and assert it **fails and the output names the new
expectation**. Asserting on the new expectation is the load-bearing proof —
that string exists only in the new Gherkin text, so if the binary reports it,
the binary was recompiled from that text. Also assert the second run's stderr
contains `Compiling <fixture-name>`, but as corroboration only; `Compiling`
appears for any dirty dependency and is a weak proxy on its own.

Both tests share one `Command`-building helper so the two invocations use
byte-identical environments — the single most likely spurious-red cause is a
rebuild triggered by an environment difference between run 1 and run 2. Capture
the resolved child environment in the failure message.

The shared harness must:

1. **Rewrite the copied manifest's path dependencies to absolute paths**
   derived from `env!("CARGO_MANIFEST_DIR")`, and assert no `..` remains in any
   dependency path. See the first entry in *Risks*.
2. **Propagate everything that participates in the build fingerprint** so the
   child build is warm: `CARGO_TARGET_DIR` when already set (falling back to
   the workspace `target/` only when it is not), `RUSTFLAGS`,
   `CARGO_ENCODED_RUSTFLAGS`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`.
3. **Redirect or strip only what causes cross-talk.** Redirect
   `LLVM_PROFILE_FILE` to a path under the scratch directory so nested
   coverage output never merges into the parent's gated profile; strip
   `CARGO_MAKEFLAGS` (the jobserver-inheritance stall) and `CARGO_PKG_*`.
   Invoke `env!("CARGO")`, not a PATH lookup. Pass `--offline --locked`.
4. **Impose its own wall-clock bound** on the child and kill it with a clear
   message rather than relying on nextest's slow-timeout.
5. **Use the stamp-file protocol** described in *Idempotence and recovery*.

Mark both `#[serial]` and add a `.config/nextest.toml` override placing
`binary_id(rstest-bdd::feature_rebuild_invalidation)` in the `cargo-spawning`
group. **Before choosing the timeout, resolve the arithmetic**: the group has
`max-threads = 1` and already budgets ~1080 s across four members under a 300 s
`global-timeout`. Either raise `global-timeout` with a measured justification
recorded here, or move the `cargo-spawning` members to `profile.long`. Record
the decision in the *Decision Log*.

**1c. The behavioural specification.** Add
`crates/rstest-bdd/tests/features/rebuild_invalidation.feature`:

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

This must **share one execution** with 1b, not duplicate it — the plan cannot
afford to pay the nested-cargo cost twice. Make the `#[scenario]` tests *be*
the regression tests, with the step functions reading a `OnceLock`-cached
result produced by the shared harness. Name the step signatures in the code.
Record the dogfooding hazard explicitly: a bug in the macro could mask the very
regression this scenario proves, which is why Test 1's dep-info assertion (a
direct filesystem check, not a macro-mediated one) is the primary contract.

**1d. The token-shape assertion.** A unit test in `rstest-bdd-macros` asserting
on the emitted `proc_macro2::TokenStream` for a representative input. Substring
presence is not enough — `include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
"/", "the/wrong/file.feature"))` would satisfy it. Assert **exact equality** of
the emitted relative literal against a table of `(input path, expected literal)`
cases, plus one `insta` snapshot of the normalized `TokenStream::to_string()`.

Scope this assertion **to the tracking item only**, not the whole expansion.
`__RSTEST_BDD_FEATURE_PATH` is an absolute literal today, so a whole-expansion
"no absolute path" assertion cannot pass until Milestone 6 and would leave a
novice unable to tell a bug from the plan. The whole-expansion assertion is
added in Milestone 6.

**Red evidence.** Capture the output; both must fail for their stated reason.
Test 1 fails because the dep-info does not list the feature file; Test 2 fails
because the second `cargo test` passes when it should fail; 1d fails because no
tracking item is emitted.

```bash
cargo nextest run -p rstest-bdd --test feature_rebuild_invalidation \
  2>&1 | tee /tmp/red-$(git branch --show-current).out
cargo nextest run -p rstest-bdd-macros tracking 2>&1 \
  | tee -a /tmp/red-$(git branch --show-current).out
```

Commit the red state. Per Constraint 6, `make test` is not expected to pass at
this milestone; `make check-fmt` and `make lint` are.

### Milestone 2 — green for `#[scenario]`

Create `crates/rstest-bdd-macros/src/codegen/tracking.rs`. Its module-level
`//!` doc comment must state the mechanism, cite ADR-010, and name the
regression test that guards it — this is the one place the *why* can live
permanently, and `AGENTS.md` already mandates a `//!` on every module.

Emit the tracking item from `crates/rstest-bdd-macros/src/macros/scenario/mod.rs`,
at the point where the resolved feature path is known, as a **sibling item** to
the generated test function. Do not touch the codegen layer, and do not thread
a new value through `ScenarioConfig` → `ScenarioMetadata` → `ScenarioLiterals`
— item-scope emission makes that unnecessary and avoids the `harness.rs` rest
pattern trap entirely.

Ordering matters: `validate_feature_file_exists` runs *before* codegen, so a
missing file produces exactly one diagnostic and no tracking item. Verify this
rather than assuming it. Also decide deliberately what happens when a file
present at build N is deleted before build N+1 — dep-info triggers the rebuild,
and the user then gets the macro's `feature file not found` *plus* a second
`couldn't read …` from the emitted binding. Add that case to the regression
test and record the ruling.

**Normalization rules**, which the review found underspecified. The tracked
literal is derived from the exact string handed to `parse_and_load_feature` —
never from `paths.rs::normalize`, which is a cache-key helper whose `..`
collapsing would name a different file through a symlink. Reuse would be a
silent correctness bug; say so in a code comment.

| Input | Emitted literal | Note |
| --- | --- | --- |
| `tests/features/x.feature` | `tests/features/x.feature` | unchanged |
| `./tests/x.feature` | `tests/x.feature` | leading `./` stripped |
| `a/./b/x.feature` | `a/b/x.feature` | `.` segments collapsed |
| `a/../b/x.feature` | `a/../b/x.feature` | `..` **retained** |
| `a\b\x.feature` | `a/b/x.feature` | **Windows only**; a backslash is a legal filename character on POSIX and must be preserved there |
| absolute, same root | `../../shared/x.feature` | component-wise offset from `CARGO_MANIFEST_DIR` (D4) |
| absolute, different root | — | `compile_error!` (D4) |
| non-UTF-8 | — | `compile_error!`; never lossy-convert (D4) |
| empty | — | `compile_error!` |

*Table 1: Feature-path normalization rules for the emitted tracking literal.*

Add `rstest` table tests for every row, and one `proptest` property: for any
sequence of non-empty relative components, the emitted literal round-trips to
the same component sequence and never begins with a separator or a drive
prefix. Make the backslash invariant platform-conditional — the original
unconditional form encoded a bug.

**Confirm the compile-time cost** against transcript E, which measured it as
inside noise at 100 files / 500 scenarios. Re-measure only if your suite shape
differs materially (very large individual `.feature` files are the pathological
case: cost is linear in Σ(occurrences × file size), and `rustc` does not
deduplicate the reads even though it deduplicates the dep-info entry).

Watch two file-length hazards: `codegen/scenario/runtime.rs` is 397 lines and
`codegen/scenario.rs` is 399, against a 400-line cap. Item-scope emission
should keep you out of both, but if plumbing is needed, put it in
`codegen/scenario/runtime/types.rs` (101 lines) rather than escalating on a
foreseeable, mechanical problem.

### Milestone 3 — green for `scenarios!`

Emit one tracking item per **discovered file**, at the top of the `scenarios!`
expansion, from `crates/rstest-bdd-macros/src/macros/scenarios/mod.rs` — before
scenario codegen and **independent of whether any test is generated**. A
`tags =` filter that matches nothing still parses the file
(`check_empty_results`, `mod.rs:161`), and that file must still be tracked; a
body-scoped or per-test binding would leave it untracked and reopen the
foot-gun in exactly the macro this plan calls the harder case.

Route the path through the same D4 helper. `rel_path` is **not** guaranteed
relative (`mod.rs:94-99` falls back to the absolute path when `strip_prefix`
fails, reachable via symlinked feature directories and via the absolute
directory arguments `path_resolution.rs` supports), so the offset computation
handles it. When it does, the diagnostic must never blame the user for a path
they did not write.

Add a regression case: a `scenarios!` directory with a `tags =` filter that
excludes every scenario in one file, asserting that file still appears in
dep-info.

Do **not** touch the re-absolutization at `test_generation.rs:307` here; that
belongs to Milestone 6.

Run the macrotest snapshots deliberately and refresh any that move:

```bash
RSTEST_BDD_RUN_MACROTEST=1 cargo nextest run \
  -p rstest-bdd-harness-tokio -p rstest-bdd-harness-gpui --test macro_compile
```

While there, confirm or refute the review's observation that the eight
`.expanded.rs` files look like curated excerpts already out of step with real
expansion — an ostensibly-live gate that is actually dead is worth recording in
*Surprises & Discoveries* either way.

### Milestone 4 — trybuild fixtures

Three fixtures, all registered in `crates/rstest-bdd/tests/trybuild_macros.rs`:

- **Compile-pass** `scenario_feature_tracking.rs` in
  `crates/rstest-bdd/tests/fixtures_macros/`, registered in
  `run_passing_macro_tests`. It binds a staged `.feature` through `#[scenario]`
  and a directory through `scenarios!`, and proves the emitted item compiles in
  the staged trybuild crate root without colliding when two scenarios bind the
  same file. Give it crate-level `#![deny(warnings)]`,
  `#![deny(clippy::pedantic)]` and `#![deny(clippy::missing_docs_in_private_items)]`
  so the lint-cleanliness assumption becomes a fixture rather than a hope.
- **Compile-fail** for the D4 unrelatable-path `compile_error!`, with a
  checked-in `.stderr`. This is the *new* compile-time behaviour and the
  roadmap's compile-fail clause is properly satisfied by pinning it. Reusing
  `scenario_missing_file.rs` alone is a regression guard, not a fixture for
  this change.
- **Unchanged** `scenario_missing_file.stderr` — confirm it is byte-identical.
  If it moves, the change has regressed the diagnostic; stop and investigate
  rather than re-blessing.

**Add a dep-info assertion on the compile-pass fixture.** After the trybuild
build you have already paid for, assert the fixture's dep-info lists the staged
`.feature` file. This costs almost nothing, is a genuine end-to-end check of the
macro-emitted binding in a real compilation, and is the mid-tier signal that
catches a future codegen refactor silently dropping the binding — without
depending on the expensive nested-cargo test that is most likely to be
`#[ignore]`d.

Finally, remove the now-redundant handwritten
`const _: &str = include_str!("basic.feature");` from a **pass-list** fixture —
`scenario_single_match.rs` (`trybuild_macros.rs:83`) or, better,
`scenario_harness_params.rs` (`:87`), which also exercises the harness path.
Do **not** use `scenario_missing_name.rs`: it is on the fail list, the macro
aborts before codegen, so no binding is emitted and the removal proves nothing
while shifting every line number in a `.stderr` that must stay stable. Treat
this as a one-time confirmation, not a standing regression signal — if codegen
later stops emitting the binding, that fixture still compiles cleanly.

### Milestone 5 — the redacted diagnostic snapshot

The roadmap's clause is "a redacted `insta` snapshot with semantic assertions
pins **any touched** diagnostic wording". The missing-`.feature` diagnostic is
explicitly *not* touched (Milestone 4 requires its `.stderr` unchanged), so
snapshotting it would satisfy the letter and miss the point. Target the
diagnostics this change actually creates or moves:

1. The **D4 `compile_error!`** for an unrelatable feature path — new wording.
2. If D3 rules Milestone 6 in scope, the **runtime failure text and reporter
   output** that flip from absolute to relative.

Host the snapshot in `crates/rstest-bdd-macros`, which is where the diagnostic
is produced, and add `insta = { workspace = true, features = ["filters"] }` to
that crate's `[dev-dependencies]`. `insta` is already at `Cargo.toml:44` in
`[workspace.dependencies]`, so per Constraint 7 this is not a new dependency.
Snapshot the rendered `syn::Error` from a direct call to the macro's internal
entry point rather than shelling out.

Follow the house redaction convention
(`crates/rstest-bdd-server/src/handlers/diagnostics/publish.rs:198`,
`crates/rstest-bdd-harness-gpui/tests/scenario_name_in_logs.rs:216`): build
`insta::Settings::clone_current()`, `add_filter` over absolute path prefixes,
line and column numbers and any rustc version string, then
`settings.bind(|| insta::assert_snapshot!(...))`. Back it with explicit
substring assertions on the load-bearing fragments so a meaning change fails
loudly even where a reflow would let a whole-text snapshot drift.

### Milestone 6 — remove the absolute path from the artefact (conditional on D3)

Only if D3 ruled option 1. Two sites must change together:

1. `crates/rstest-bdd-macros/src/macros/scenario/paths.rs` — add a
   manifest-relative accessor and feed *that* to `create_scenario_literals`. Do
   not repurpose `canonical_feature_path`: it is still needed for compile-time
   diagnostics and its memoization cache is keyed on the absolute form.
2. `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs:307` —
   drop `ctx.manifest_dir.join(ctx.rel_path)` and pass `ctx.rel_path` through.

Both emission sites are affected: `codegen/scenario/runtime.rs:261` and
`codegen/scenario/runtime/harness.rs:38`. **Delete the `..` rest pattern in the
`harness.rs` destructuring** so a future field addition there is a hard error
rather than a silent omission.

Because `rel_path` is not guaranteed relative, `__RSTEST_BDD_FEATURE_PATH` must
have a stated fallback. Document it as: *relative to the consuming crate's
manifest directory when the feature file lies within it; otherwise absolute.*
Do not ship an unconditional guarantee. Update the doc comments on
`ScenarioMetadata::feature_path` (`crates/rstest-bdd/src/reporting/record.rs:11`),
`ExecutionError::MissingFixturesDetails::feature_path`
(`crates/rstest-bdd/src/execution/error.rs:208`), and the `ScenarioRecord`
accessor — none of them says anything today, which is how the two macros
drifted apart.

**Rule on workspace uniqueness.** Absolute paths made `feature_path` globally
unique; `tests/features/login.feature` does not. `cargo-bdd` merges dumps
across workspace members, so two crates with the same conventional layout now
collide in merged output and in `format_location`
(`crates/cargo-bdd/src/output.rs:150`). Minimum: document the loss in the
users' guide and the `feature_path` doc comment. Better: have `cargo-bdd`
qualify merged entries with the package name it already has from
`cargo metadata`, pinned by a test. State whichever you choose.

Re-run the blast-radius survey before editing, now covering `.stderr`, `.snap`,
**`**/*.expanded.rs`**, macro-crate unit tests, and doctests. Planning found no
checked-in expectation carrying a real macro-emitted absolute path, but confirm
it. Update whatever changes one at a time, reading each diff. Never
bulk-re-bless; if more than a handful move, that is a signal to stop.

Add the **whole-expansion** "no absolute path literal" assertion here, deferred
from 1d.

Do **not** add a release-profile artefact check, despite the earlier draft
proposing one. `target/` in this repository contains only `debug/`, and nothing
in CI builds in release except `make publish-check`, which already compiles
into a separate target directory and already contributes to the disk pressure
the workflow's free-disk step works around. A release assertion would mean a
third full dependency-tree build inside a test, in minutes and gigabytes, on a
disk-constrained runner — a silent breach of this plan's own wall-clock
tolerance. The token-stream assertion is deterministic, free, and a strictly
stronger contract; lean on it. (A dev-profile artefact check would be worse
than useless: dev binaries contain the manifest path in every crate, tracked or
not, purely from debug info, so the assertion is vacuous — see transcript B.)

Finally, `ScenarioTestContext.manifest_dir` (`test_generation.rs:34`) may become
dead once its only consumer at line 307 goes, and would then trip `dead_code`
under `-D warnings`. Remove it or state why it is retained.

### Milestone 7 — documentation and roadmap

- `docs/adr-010-feature-file-change-detection.md`: Status `Proposed` →
  `Accepted`, with the date and a brief summary of what was decided, per
  `docs/documentation-style-guide.md`; update the `Date` field to the amendment
  date. **Preserve the "Absolute-path `include_str!` variant — rejected"
  subsection verbatim** and add a dated adjacent subsection recording the
  measurement and the distinction (D0a). Correct Table 1's
  "Binary-size cost: A = Medium" row. Correct the Option B claim about
  per-file `rerun-if-changed` lines (D2). Amend constraint 5 per D5. Record the
  `include_bytes!` choice and the anonymous-`const` rationale.
- `docs/rstest-bdd-design.md` §2.7.6.6: rewrite from "here is a foot-gun and two
  candidate mechanisms" to "here is what ships and why", including the residual
  `scenarios!` addition/deletion gap. Leave §3.2.2's OUT_DIR-caching discussion
  alone except to keep its cross-reference accurate; invalidation and caching
  must stay distinct.
- `docs/v0-6-0-migration-guide.md`: delete the "Feature-file edits do not
  trigger a rebuild" section at line 714 — its own note says it can go once the
  fix ships. Add, in the same edit: the D4 `compile_error!` announcement; if D3
  ruled option 1, the JUnit `classname` discontinuity note for CI operators;
  and a paragraph for consumers on hermetic build systems (Bazel, Buck2, Nix
  sandboxes) that parse dep-info and require every listed input to be declared
   — they will need to add `.feature` files to their input sets. That is
  correct behaviour and the point of the change, but it is a migration action.
- `docs/users-guide.md`: two passages assert the old behaviour and must both
  change — line 1523 ("…foot-gun … until roadmap item 10.3.3 lands") and line
  1660 ("Editing only a `.feature` file does not trigger a rebuild … touch a
  binding `.rs` file"). Add the residual `scenarios!` gap. **Add a short *Cost*
  subsection**, because this change makes a previously free operation cost
  something and users will feel it as "my edit loop got slower": editing one
  `.feature` file now rebuilds the whole test binary containing that
  `scenarios!` invocation, which re-parses every feature file in the bound
  directory. Measured at 5.4 s for 100 files × 5 scenarios (transcript E). The
  mitigation is to split large feature directories across several test
  binaries. This is a fair trade for correctness, but it should be stated
  rather than discovered. Two gates guard this
  file: `scripts/check_users_guide_links.py` validates that absolute GitHub
  reference links resolve *and* that each URL fragment still matches a heading
  in the target, so renaming the design document's §2.7.6.6 heading breaks the
  build unless the guide's link changes in the same commit; and if you touch
  the `#[serial]` runner-behaviour table, keep it byte-identical with the design
  document's copy or `scripts/check_serial_nextest_matrix.py` fails.
- `docs/developers-guide.md`: the cargo-spawning fixture-crate *convention*
  (non-workspace fixture, inherited `CARGO_TARGET_DIR`, child-environment
  hygiene, `cargo-spawning` nextest group) and the invariant that
  macro-emitted token streams carry no absolute path literal, with a pointer to
  the assertion that enforces it and to `codegen/tracking.rs`. Per the style
  guide this document must **not** embed repository-layout guidance.
- `docs/repository-layout.md`: the new paths —
  `crates/rstest-bdd/tests/fixtures/rebuild_invalidation/` and the
  `target/tests/rebuild-invalidation/` scratch area. This is the canonical
  document for path responsibility.
- `docs/testing-strategy.md`: this change introduces a new *class* of test — a
  cargo-spawning fixture crate with its own nextest group and its own
  environment-hygiene rules. That is a testing-strategy change, not just an
  implementation detail.
- `docs/known-issues.md`: the residual `scenarios!` addition/deletion gap
  (unconditionally, per D2), and the Windows `#[ignore]` fallback if it was
  taken.
- `docs/CHANGELOG.md`: **required**, not optional. This retires a published
  migration caveat and, under D3 option 1, changes serialized reporter output.
  Use the `changelog` skill.
- `docs/contents.md`: no change needed — it lists `execplans/` generically.
  Stated so the implementer does not wonder.
- `docs/roadmap.md`: tick 10.3.3 to `[x]` **only if** the finish line is
  actually met. If D3 ruled Milestone 6 out of scope, the artefact still
  carries an absolute path and the clause is unmet: either leave the item `[ ]`
  or amend the clause in the same edit with recorded rationale. Add the D2
  follow-up item for the `rstest-bdd-build` helper, and the
  `emit_runtime_deprecation_warning` follow-up, using `mapsplice` so numbering
  and `Requires` references stay consistent.

## Concrete steps

Run everything from the repository root:
`/home/leynos/.lody/repos/github---leynos---rstest-bdd/worktrees/2efe7d4e-4101-4859-8b92-9cefa53bc36f`.

```bash
LOGBASE="/tmp/$(git branch --show-current)"
```

Focused loops during development:

```bash
cargo nextest run -p rstest-bdd-macros 2>&1 | tee "$LOGBASE-macros.out"
cargo nextest run -p rstest-bdd --test feature_rebuild_invalidation 2>&1 \
  | tee "$LOGBASE-invalidation.out"
cargo nextest run -p rstest-bdd --test trybuild_macros 2>&1 \
  | tee "$LOGBASE-trybuild.out"
RSTEST_BDD_RUN_MACROTEST=1 cargo nextest run \
  -p rstest-bdd-harness-tokio -p rstest-bdd-harness-gpui --test macro_compile \
  2>&1 | tee "$LOGBASE-macrotest.out"
```

Full gates at the end of every milestone, run **sequentially** — this
environment relies on build caching and on Cargo's package-cache lock:

```bash
make check-fmt 2>&1 | tee "$LOGBASE-checkfmt.out"
make lint      2>&1 | tee "$LOGBASE-lint.out"
make test      2>&1 | tee "$LOGBASE-test.out"
make publish-check 2>&1 | tee "$LOGBASE-publish.out"
```

Prefer delegating that run to the `scrutineer` subagent, which runs the gates
sequentially, captures each log under `/tmp`, and returns a bounded report.
When it reports a failure, read the cited log rather than re-running the gate.

Markdown after any documentation edit — `make fmt` can itself introduce
`MD013`/`MD039` violations, so lint *after* formatting:

```bash
make fmt 2>&1 | tee "$LOGBASE-fmt.out"
make markdownlint nixie 2>&1 | tee "$LOGBASE-mdlint.out"
```

Commit after each milestone with a file-based message (`commit-message` skill;
never `-m`).

## Validation and acceptance

**The headline behaviour.** In the fixture crate, `cargo test` passes. Change
only the captured value in `tests/features/invalidation.feature`. `cargo test`
prints a `Compiling` line and the test fails, naming the new expectation.
Before the fix, the second run prints no `Compiling` line and the test passes.

**Red-Green-Refactor evidence.** Record all three in *Artefacts and notes*.

- *Red*: Test 1 fails because the dep-info does not list the feature file;
  Test 2 fails because the second `cargo test` succeeded when it should have
  failed; the 1d token-shape assertion fails because no tracking item is
  emitted. Each must fail for its own reason — that is why 1b is split.
- *Green*: all three pass after Milestones 2 and 3, with no other test changed.
- *Refactor*: the same commands still pass and `make lint` is clean.

**Compile-time contract.** `cargo nextest run -p rstest-bdd --test
trybuild_macros` passes with the new compile-pass fixture and the new D4
compile-fail fixture registered; the compile-pass fixture's dep-info lists the
staged `.feature` file; and `scenario_missing_file.stderr` is byte-identical to
its pre-change content.

**Diagnostic wording.** The `insta` snapshot for the D4 diagnostic passes with
its redaction filters and its semantic assertions. `cargo insta
pending-snapshots` reports nothing outstanding.

**No absolute path.** The Milestone 1d assertion proves the *tracking item*
carries no absolute path literal. If Milestone 6 lands, the whole-expansion
assertion extends that to the constant, subject to the rlib qualifier in
Constraint 2. There is deliberately no artefact-grep assertion; see Milestone 6
for why.

**Quality criteria — what "done" means.**

- Tests: `make test` green, including `cargo test --doc` and
  `uv run pytest scripts/tests`. `make publish-check` green.
- Lint and format: `make check-fmt` and `make lint` green, including all four
  Python structural checkers and the Whitaker Dylint suite. Note the known
  local `no_expect_outside_tests` false positive on two test-helper files,
  which is green on CI and should be ignored as environmental.
- Portability: both Windows CI legs pass. There is no macOS leg — say so
  plainly in the pull request rather than implying macOS coverage.
- Performance: the `cargo nextest` step stays comfortably inside its
  `global-timeout` on the slowest (coverage-instrumented) CI leg. Record
  nextest's reported run duration with and without the new test, the new
  test's own duration, and the CI coverage-leg duration before and after.
- Documentation: `scripts/check_users_guide_links.py` and
  `scripts/check_serial_nextest_matrix.py` pass; `make markdownlint nixie`
  clean.

## Idempotence and recovery

Every step is safe to repeat.

The regression tests never mutate the checked-in fixture. They copy it to
`target/tests/rebuild-invalidation/fixture` and mutate the copy. Restoring only
the `.feature` file is **not** sufficient — a scratch directory left over from
an older fixture version would keep a stale `tests/invalidation.rs`,
`Cargo.toml` and `Cargo.lock`, and the test would either fail inexplicably or
pass against the wrong sources. A test killed mid-copy leaves the same
wreckage.

Use a stamp-file protocol instead: compute a hash of the source fixture tree;
on entry, if `target/tests/rebuild-invalidation/.stamp` is absent or differs,
`remove_dir_all` the scratch directory and re-copy wholesale; write the stamp
**last**, so a partial copy can never be mistaken for a complete one.

Manual recovery: `rm -rf target/tests/rebuild-invalidation`. That is always
safe and costs one cold compile.

Note that setting the `.feature` file's mtime into the future leaves the
scratch tree permanently "ahead" of wall clock. That is intentional and
harmless here; leave the comment explaining it so nobody "fixes" it.

Nothing in this plan is destructive. The riskiest edits are to checked-in
`.stderr`, `.snap` and `.expanded.rs` expectations in Milestone 6; those are
under version control, so `git checkout -- <path>` restores them. If a
milestone goes wrong, `git reset --hard` to the previous milestone's commit —
which is why each milestone commits separately.

## Artefacts and notes

Four measurements taken during planning on this host with stable `rustc`. Each
used a scratch crate under `target/plan-scratch*/`, removed afterwards.
Reproduce them in Milestone 0 before trusting them.

**Transcript A — dep-info registration and rebuild triggering.** A crate whose
only reference to the feature file is the tracking binding:

```text
=== dep-info .d ===
.../target/debug/libinv.rlib: .../features/x.feature .../src/lib.rs
=== now edit ONLY the .feature file ===
   Compiling inv v0.0.0 (...)
```

**Transcript B — what actually lands in the artefact.** A control binary with
no binding, compared against one with it, in both profiles:

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

Two conclusions. The mechanism adds neither the absolute path nor the feature
text to a binary — the unused anonymous `const` is elided, so ADR-010's
"Medium" binary-size estimate does not apply to this form. And the absolute
path present in dev-profile binaries comes from debug info and is present in
the *control* too, which is why the plan asserts on the emitted token stream
(always meaningful) and on release binaries (meaningful), not on dev binaries
(vacuous).

A separate run against an `rlib` *did* show both the path and the text, because
rlib metadata retains source paths and constant values for downstream
inlining. Scenario tests compile to binaries, so this does not affect the
result — but it is why Constraint 2 is scoped to binary and test targets, and
the qualifier must survive into the ADR amendment.

**Transcript C — an anonymous `const` in a function body is enough.** With
`#![deny(warnings)]`, dep-info listed the feature file. Retained as evidence
that the construct is lint-clean; the shipping form uses item scope for the
reasons in D0.

**Transcript D — `include_str!` versus `include_bytes!` on a non-UTF-8 file.**

```text
--- include_str! on non-UTF-8 ---
error: `.../f/bad.feature` wasn't a utf-8 file
 --> src/lib.rs:2:17
--- include_bytes! on non-UTF-8 ---
COMPILED CLEAN
dep-info entry?
1
```

`include_str!` fails hard, and its error text contains the absolute path.
`include_bytes!` compiles clean under `#![deny(warnings)]` and registers
dep-info identically. This is why D0 specifies `include_bytes!`.

**Transcript E — scaling measurements.** Taken during design review on a
24-core host with the workspace dev profile.

```text
Emission cardinality        O(scenarios) if emitted per test; O(files) as specified in D0
rustc read dedupe           none: 500 include_* of one file -> 500 openat(); ~120 MB/s
Cost vs control, 500 occurrences   4 KB feature +0.03 s | 40 KB +0.19 s | 400 KB +1.67 s
dep-info dedupe             yes: 500 occurrences of one file -> 1 entry
No-op cargo build, 500 tracked paths   41.8 ms -> 45.8 ms (+4 ms)
Real macros, scenarios! over 100 files x 5 scenarios (500 tests, 4 KB each):
    without bindings 5.66 s | with 500 bindings 5.67 s  (inside noise)
scenarios! expansion alone  5 tests 0.38 s | 125 tests 1.57 s | 500 tests 5.40 s
New test, warm              fixture no-op cargo test 0.15 s; rebuild+relink+run 0.31 s
New test, cold fixture      ~14 s on an otherwise-warm tree
Target-dir lock             released before tests run; inner cargo build succeeded
                            while the outer test binary was still alive
```

Three conclusions. The mechanism's compile-time cost is negligible at realistic
scale, for a reason worth stating: the proc macro *already reads every feature
file* through `std::fs`, so the emitted binding at worst doubles a read that
already happens. Dep-info and fingerprint costs are noise. And the real new
cost is not the binding at all — it is that a `.feature` edit now triggers a
full re-expansion of the enclosing `scenarios!` invocation, which is the
dominant term for large suites and which Milestone 7 must document.

**External status checks.** `proc_macro::tracked_path` — rust-lang/rust#99515,
"Tracking Issue for `proc_macro::{tracked_env, tracked_path}`" — is open and
unstabilized, behind
`#![feature(proc_macro_tracked_env, proc_macro_tracked_path)]`.
`proc_macro_error 1.0.4`'s `emit_warning!` is documented as nightly-only and
silently ignored on stable. Cargo scans a `rerun-if-changed` **directory**
recursively (rust-lang/cargo#8973), contrary to ADR-010's Option B con.

## Interfaces and dependencies

No new external dependency (D1, Constraint 7). `insta` moves from
`[workspace.dependencies]` into `crates/rstest-bdd-macros`'s
`[dev-dependencies]` in Milestone 5, which is not a new dependency.

In `crates/rstest-bdd-macros/src/codegen/tracking.rs`:

```rust
/// Why a feature file cannot be registered as a Cargo rebuild dependency.
pub(crate) enum Untrackable {
    /// The path shares no filesystem root with `CARGO_MANIFEST_DIR`
    /// (different Windows drive, UNC prefix).
    UnrelatableRoot(std::path::PathBuf),
    /// The path is not valid UTF-8 and cannot be written as a string literal.
    NonUtf8(std::path::PathBuf),
    /// The path is empty.
    Empty,
}

/// A feature-file path expressed relative to `CARGO_MANIFEST_DIR`.
///
/// Always uses `/` separators, never begins with a separator or a drive
/// prefix, and may contain `..` segments (see Table 1 in the ExecPlan).
pub(crate) struct TrackedFeaturePath(String);

impl TrackedFeaturePath {
    /// Expresses `path` relative to `CARGO_MANIFEST_DIR`, computing a
    /// component-wise `..` offset when `path` is absolute.
    pub(crate) fn try_new(path: &std::path::Path) -> Result<Self, Untrackable>;

    /// Emits the Cargo rebuild-dependency item for this feature file.
    pub(crate) fn binding(&self) -> proc_macro2::TokenStream;
}

/// Resolves `path` and emits either the tracking item or a `compile_error!`.
///
/// This is the only place that decides what happens on failure, so the
/// decision cannot be forgotten at a call site.
pub(crate) fn feature_tracking_item(
    path: &std::path::Path,
    span: proc_macro2::Span,
) -> proc_macro2::TokenStream;
```

The `Result`-returning constructor and the single `feature_tracking_item` entry
point are deliberate: the earlier `Option`-in / empty-tokens-out shape pushed
the failure-reporting obligation onto every call site, with silence as the
failure mode. There must be exactly one place where "untrackable" turns into a
diagnostic.

Call `feature_tracking_item` from `crates/rstest-bdd-macros/src/macros/scenario/mod.rs`
(once, for the bound file) and from
`crates/rstest-bdd-macros/src/macros/scenarios/mod.rs` (once per discovered
file, independent of test generation). The codegen layer is not modified in
Milestones 2–3.

Keep `tracking.rs` well under the 400-line cap; if the normalization logic and
its tests grow past it, move the tests into a sibling `tracking/tests.rs`.

## Outcomes & retrospective

To be completed at milestone boundaries and at completion. Compare against
*Purpose*: can a user edit only a `.feature` file, run `cargo test`, and see a
rebuild and a fresh failure? Record the CI-measured nextest wall clock, the
synthetic-suite compile-time delta, whether both Windows legs passed first
time, whether the `.expanded.rs` gate turned out to be live or dead, and
whether the residual `scenarios!` addition gap caused confusion in review.

## Revision notes

- **2026-08-15, revision 1.** Initial draft.
- **2026-08-15, revision 2.** Revised after a six-lens design review
  (structure, alternatives, scaling, contracts, failure modes, viability).
  What changed: the emitted binding moved from `include_str!` in the generated
  function body to `include_bytes!` at item scope, once per bound feature file,
  after measurement showed `include_str!` hard-errors on non-UTF-8 files and
  after review found that `scenarios!` can parse a file and generate zero tests
  from it; Decision D4 was rewritten because the crate's warning macro is a
  no-op on stable, which would have shipped a silent skip; D1, D2 and D5 were
  ruled rather than left open; D3 gained a third option and a
  same-release requirement; the regression test gained manifest rewriting,
  `CARGO_TARGET_DIR` inheritance, child-environment hygiene, deterministic
  artefact lookup and a stamp-file protocol, and was split in two; a dep-info
  assertion on the trybuild fixture was added as a cheap standing signal; the
  nextest timeout arithmetic, the compile-time cost tolerance, the
  `.expanded.rs` blast radius, and four further documents were added.
  Why: the review found four blocking issues that would each have surfaced as
  a mid-implementation stop, and two that would have shipped the original
  foot-gun in a corner.
  Effect on remaining work: Milestone 2 is simpler than before (no type
  threading), Milestone 1 is materially more demanding, and only D3 still
  blocks a start.
- **2026-08-15, revision 3.** Folded in the scaling review's measurements
  (transcript E). What changed: the compile-time cost is now measured rather
  than merely toleranced, and found negligible — the macro already reads every
  feature file, so the binding at worst doubles an existing read; the
  coverage-environment mitigation was rewritten from "scrub" to a stated
  principle (propagate what participates in the fingerprint, redirect only what
  cross-talks), because scrubbing and propagating fail in opposite directions;
  the fixture's dependency set is now required to be byte-identical to the
  existing fixture's so the two share compiled units; the release-profile
  artefact assertion was **deleted** as an unbudgeted third full build on a
  disk-constrained runner; the dep-info assertion gained an exactly-once check;
  the wall-clock tolerance was made measurable; and the users' guide gained a
  required *Cost* subsection, because a `.feature` edit now re-expands the
  whole enclosing `scenarios!` invocation.
  Why: the review measured the mechanism as cheap and the *test harness* as
  the real cost risk, which inverts where the plan was spending its caution.
  Effect on remaining work: no milestone gains scope; Milestone 6 loses one
  deliverable.
