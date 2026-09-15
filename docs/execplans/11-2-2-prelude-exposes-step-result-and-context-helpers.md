# Expose a public prelude for integration imports (roadmap 11.2.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

Roadmap item: 11.2.2 (`docs/roadmap.md` lines 1038-1043). Origin: the v0.6.1
early-life support programme recorded in `docs/rstest-bdd-design.md` §2.7.6.4
(lines 2182-2196).

## Purpose / big picture

Today a person writing their first `rstest-bdd` acceptance test has to know
which of four crates each name lives in. The example at
`examples/tokio-reminders/tests/reminders.rs` opens with four `use` lines that
between them name `rstest`, `rstest_bdd_harness_tokio`, `rstest_bdd_macros`,
and the example's own crate — and never mentions `rstest_bdd` at all, even
though `rstest_bdd` is the crate people are told they are using. That split is
an accident of how the workspace grew, not a design anyone would choose.

After this change a newcomer writes one import to get the framework:

```rust
use rstest_bdd::prelude::*;
use rstest_bdd_harness_tokio::TokioTestContext;
```

and everything the framework asks them to name — the `#[given]`, `#[when]`,
`#[then]`, and `#[scenario]` attributes, the `StepResult` alias, `Slot`, the
`ScenarioState` trait and its derive, `StepContext` and its harness-context
accessors — resolves. The underlying crates are not hidden: they remain named in
`Cargo.toml`, remain documented, and remain directly importable by anyone who
prefers explicit paths.

You can see this working three ways. First, every example test file in
`examples/` will contain exactly two framework imports (the prelude and, where
the example names a harness type, its harness adapter crate) and a machine gate
will fail if a fifth crate creeps back in. Second, a fixture crate that lives
outside the workspace and declares only `rstest-bdd`, `rstest`, and one harness
adapter in its manifest will compile a complete scenario using nothing but
prelude imports — it cannot compile if the prelude is missing an item. Third,
`docs/users-guide.md` will carry a table of every exported name, and a gate
wired into `make lint` will fail if that table and the actual `pub use` list in
`crates/rstest-bdd/src/prelude.rs` disagree.

## Constraints

These are hard invariants. If satisfying the objective would require breaking
one, stop, record the conflict in `Decision log`, and escalate.

- The v0.6.1 line is semver-compatible. Nothing in this plan may remove,
  rename, or change the signature of any existing public item in `rstest-bdd`,
  `rstest-bdd-macros`, `rstest-bdd-harness`, `rstest-bdd-harness-tokio`,
  `rstest-bdd-harness-gpui`, or `rstest-bdd-policy`. The prelude is purely
  additive; every item it exports must remain reachable at its present path.
- `rstest-bdd` must not gain a dependency on `rstest` (the crate). Generated
  scenario code emits the fully-qualified attribute `#[rstest::rstest]`
  (`crates/rstest-bdd-harness/src/policy.rs:89`), which resolves against the
  *consumer's* `rstest` dependency. If `rstest-bdd` also depended on `rstest`
  and the two requirements ever resolved to different semver-incompatible
  versions, a fixture declared with a prelude-re-exported `#[fixture]` and a
  test generated with the consumer's `#[rstest::rstest]` would belong to two
  different proc-macro crates. That failure is silent at the manifest level and
  confusing at the call site. See Decision D2.
- No dependency cycle may be introduced. `rstest-bdd` gaining a normal
  dependency on `rstest-bdd-macros` is permitted and verified acyclic:
  `rstest-bdd-macros` depends only on `rstest-bdd-patterns`,
  `rstest-bdd-harness`, `rstest-bdd-policy`, and third-party crates
  (`crates/rstest-bdd-macros/Cargo.toml` lines 27-44), none of which depend on
  `rstest-bdd`. The existing dev-dependency edge from `rstest-bdd-macros` back
  to `rstest` is unaffected; Cargo permits dev-dependency cycles.
- No file may exceed 400 lines (`AGENTS.md`, "Keep file size manageable",
  enforced by `scripts/check_rs_file_lengths.py` and by the Whitaker
  `module_max_lines` lint). The prelude module and any new gate script must be
  written to fit, not added to `scripts/rs-length-allowlist.txt`.
- Every new module begins with a `//!` module-level doc comment, and every new
  public item carries a `///` doc comment.
  `[workspace.lints.rust] missing_docs = "deny"` and the
  `cargo doc --workspace --no-deps` step inside `make lint` (`Makefile:112`,
  with `RUSTDOC_FLAGS ?= --cfg docsrs -D warnings`) enforce this.
- Lint suppressions are a last resort, must be `#[expect(...)]` rather than
  `#[allow(...)]` (`[workspace.lints.clippy] allow_attributes = "deny"`), and
  must carry a `reason = "..."`.
- Documentation is en-GB with Oxford spelling, prose wrapped at 80 columns and
  code blocks at 120 columns (`AGENTS.md`, "Markdown guidance").
- `examples/*` are ordinary workspace members (`Cargo.toml` lines 14-17) and
  are therefore already covered by `make check-fmt`, `make lint`, and
  `make test`, all of which pass `--workspace --all-targets --all-features`.
  Anything this plan does to the examples must keep those three green.

## Tolerances (exception triggers)

Stop and escalate rather than improvizing when any of these is reached.

- Scope: if the change touches more than 30 files, or the net Rust diff exceeds
  900 lines excluding generated snapshots, stop and escalate.
- Interface: if delivering the finish line appears to require changing an
  existing public signature — as opposed to adding new items — stop and
  escalate. This is a semver-compatible release line.
- Dependencies: the only new inter-crate dependency this plan authorizes is
  `rstest-bdd` -> `rstest-bdd-macros`. Any other new dependency, first-party or
  third-party, is an escalation.
- Feature flags: if the macro re-export cannot be made to work without adding a
  second feature axis beyond the single `macros` feature described in Decision
  D3, stop and escalate.
- Iterations: if a gate still fails after four focused attempts on the same
  root cause, stop, write what was tried into `Surprises & discoveries`, and
  escalate.
- Time: if any milestone exceeds four hours of wall-clock work, stop and
  escalate with a partial-progress note in `Progress`.
- Ambiguity: if the expert review or implementation evidence contradicts
  Decision D1, D2, or D3, stop — those decisions define the deliverable shape
  and changing one changes what "done" means.

## Risks

- Risk: the roadmap phrase "marker attributes from 11.2.1" has no referent.
  `#[harness_context]` is an inert parameter marker consumed inside the
  `#[given]`/`#[when]`/`#[then]` expansion
  (`crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_context/mod.rs`),
  not a `#[proc_macro_attribute]`. There is no item to re-export. Severity:
  medium. Likelihood: certain (already observed). Mitigation: Decision D4
  resolves this by documenting the marker as import-free and proving it with a
  compile test, rather than inventing a no-op attribute macro. The design
  document and the ADR-007 addendum are updated so the next reader is not
  misled.

- Risk: glob-importing a prelude is a semver hazard. Cargo's SemVer reference
  and the Rust API evolution RFC both note that adding a public item to a
  glob-imported module can break a downstream crate that already defines that
  name. Severity: medium. Likelihood: low for the initial release, rising with
  each later addition. Mitigation: the prelude is curated, not a mirror of the
  crate root; every name is framework-specific (`StepResult`, `ScenarioState`,
  `StepContext`) rather than generic (`Error`, `Result`, `Context`); and the
  docs/exports parity gate makes any future addition a visible, reviewed change
  rather than an incidental one. Record this policy in ADR-022 so later
  additions are weighed against it.

- Risk: `clippy::wildcard_imports` is a pedantic lint and the workspace runs
  `pedantic = { level = "warn", priority = -1 }` under `-D warnings`, so a glob
  import could fail `make lint`. Severity: high if true. Likelihood: low.
  Clippy's `warn-on-all-wildcard-imports` option defaults to `false`, and with
  that default the lint deliberately does not fire for imports from a module
  named `prelude`. `clippy.toml` does not set the option. Mitigation: EP-M0
  proves this empirically with a throwaway glob import before any real work
  depends on it. If the assumption is wrong, that is an escalation, not a
  suppression.

- Risk: `rstest-bdd` gaining a normal dependency on `rstest-bdd-macros` changes
  feature unification. `rstest-bdd` already dev-depends on it with
  `features = ["compile-time-validation"]` (`crates/rstest-bdd/Cargo.toml:48`);
  adding a default-featured normal dependency means that within `rstest-bdd`'s
  own test build both edges unify and `compile-time-validation` stays on, as it
  is today. Downstream consumers get default features unless they ask
  otherwise. Severity: low. Likelihood: low. Mitigation: EP-M2 runs the full
  gates immediately after the manifest change, before any prelude content
  exists, so a unification surprise is isolated.

- Risk: adding `rstest-bdd-macros` to `rstest-bdd`'s normal dependencies
  lengthens the build chain for consumers who only want the runtime types —
  notably `rstest-bdd-harness-gpui`, which depends on `rstest-bdd`. Severity:
  low. Likelihood: certain. Mitigation: the macro re-exports sit behind a
  default-on `macros` feature (Decision D3), so `default-features = false`
  restores the old graph.

- Risk: a `#[cfg(feature = "macros")]` block inside a glob-imported module
  makes the prelude's *name set* a function of other crates' manifests. Cargo
  features are additive and unify across the whole graph, so a library that sets
  `default-features = false` and writes `use rstest_bdd::prelude::*;` gets a
  small prelude — until some unrelated dependency elsewhere enables `macros`,
  at which point nine further names appear in its scope and may collide with
  its own. A cfg-gated glob prelude is the one shape where feature unification
  can break a build that was previously fine. Severity: medium. Likelihood:
  low, rising with adoption. Mitigation, to settle at approval: either make the
  dependency non-optional and delete the gate, or split the module so the
  unconditional names live in `prelude` and the macro names in a separate
  `prelude::macros`, making each glob's contents independent of feature
  resolution. Do not ship a cfg-gated glob without choosing one of these.

- Risk: a published prelude cannot be withdrawn. EP-M3's stated recovery
  ("delete `prelude.rs`") is true before merge and false after
  `rstest-bdd 0.6.1` ships; yanking does not remove the API from existing
  lockfiles. The draft's `Risks` covered future *additions* to the glob but
  never the module's own irreversibility. Severity: medium. Likelihood: certain
  if the design turns out wrong. Mitigation: `#[doc(hidden)]` is the wrong
  hedge — it forfeits the entire ergonomic benefit and still does not make
  removal semver-legal. A non-default `prelude` feature makes removal cheap but
  breaks "one predictable module" and means the migrated examples do not
  compile for a default consumer. The workable hedge is **scope**: adding a
  name later is semver-safe, removing one is not, so ship only what the
  examples and the EP-M4 fixture demonstrably need. That is in direct tension
  with the panel's other finding that the list must be *coherent* — a derive
  without its trait sends users straight back to a second import. The plan
  resolves the tension by shipping **complete trait/derive groups for the
  step-author surface and nothing else**: no harness-author surface, no
  localization, no registry, no `StepPattern`. Every group in the list is
  exercised by the EP-M4 fixture.

- Risk: the dependency-restricted compile fixture drifts out of step with the
  workspace, as fixture crates under `tests/fixtures/` have their own
  lockfiles. Severity: medium. Likelihood: medium. Mitigation: follow
  `crates/rstest-bdd/tests/fixtures/rebuild_invalidation/`, whose lockfile is
  seeded from `crates/cargo-bdd/tests/fixtures/minimal/`.
  `scripts/check_fixture_lockfiles.py` discovers such fixtures automatically,
  so the real work is the four wiring points listed in EP-M4 — dependabot, `fmt`
  /`check-fmt`, the mutation-lane prefetch, and the shared target directory.

- Risk: the docs/exports parity gate becomes a nuisance that people work around
  by editing the table without thinking. Severity: low. Likelihood: medium.
  Mitigation: make the failure message name the exact missing or surplus item
  and point at both files, following the error-message quality of
  `scripts/check_gpui_mapping_table.py`.

## Progress

- [x] (2026-09-14) Draft written; six-expert design review completed;
      revisions applied. Two load-bearing assumptions verified empirically with
      throwaway probe crates. Six decisions remain **pending approval**, D0
      first.
- [ ] EP-M0: orientation, green baseline, and the wildcard-import probe.
- [ ] EP-M1: red — failing tests and gates that specify the prelude.
- [ ] EP-M2: manifest change wiring `rstest-bdd-macros` into `rstest-bdd`
      behind the default-on `macros` feature, with gates green and no prelude
      content yet.
- [ ] EP-M3: green — `crates/rstest-bdd/src/prelude.rs` with the curated
      export list.
- [ ] EP-M4: the dependency-restricted compile fixture.
- [ ] EP-M5: migrate the four example crates to prelude-only imports.
- [ ] EP-M6: the import-allowlist contract test.
- [ ] EP-M7: documentation — users' guide table, developers' guide, design doc
      §2.7.6.4, ADR-022 — and the docs/exports parity gate.
- [ ] EP-M8: roadmap tick, full gates, pull request.

## Surprises & discoveries

- Observation: no example test file imports `rstest_bdd` at all today.
  Evidence: an exhaustive grep over `examples/` finds no `use rstest_bdd::`;
  the four BDD test files import only `rstest::fixture`,
  `rstest_bdd_macros::{given, scenario, then, when}`, the example's own crate,
  and (in `examples/tokio-reminders/tests/reminders.rs:7` only)
  `rstest_bdd_harness_tokio::TokioTestContext`. Impact: the finish line's "the
  prelude plus their harness crate" is close to achievable, because the harness
  type in `#[scenario(harness = ...)]` is already written fully qualified (for
  example `examples/gpui-counter/tests/counter.rs:56`). Only `rstest::fixture`
  and the step attributes stand in the way.

- Observation: fully-qualified attribute paths are already house style in the
  examples. Evidence: `examples/gpui-counter/tests/counter.rs:11` writes
  `#[rstest_bdd_test_macros::allow_fixture_expansion_lints]` and line 56 writes
  `harness = rstest_bdd_harness_gpui::GpuiHarness`. Impact: migrating
  `#[fixture]` to `#[rstest::fixture]` (Decision D2) is consistent with
  existing practice rather than a novel imposition.

- Observation: `#[harness_context]` has no exportable item. Evidence: reading
  `crates/rstest-bdd-macros/src/lib.rs` in full shows only `given`, `when`,
  `then`, and `scenario` as `#[proc_macro_attribute]`; the marker is classified
  inside
  `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_context/mod.rs`.
  The 11.2.1 plan's `Decision log` explicitly deferred "whether the marker
  should additionally be re-exported from a prelude" to this item. Impact:
  drives Decision D4.

- Observation: the repository has no public-API surface tooling. Evidence: a
  repository-wide search for `cargo-public-api`, `cargo-semver-checks`,
  `public-api.txt`, and `public_api` returns nothing. Impact: the "docs list
  the exported items" half of the finish line has to be discharged by a
  handwritten gate; there is no off-the-shelf backstop to lean on. This is what
  motivates the parity gate in EP-M7.

- Observation: `crates/rstest-bdd/Cargo.toml` already has a `[features]`
  section. Evidence: lines 63-71 declare `default = ["diagnostics"]` plus
  `diagnostics`, `mutable_world_macro`, and `test-support`, and lines 73-76 gate
  `[[test]] dump_registry` on `required-features = ["diagnostics"]`. Impact:
  the draft's EP-M2 instruction to "add a `[features]` section (the crate has
  none today)" would have dropped `diagnostics` from the default set — a
  semver-visible regression that **no gate would catch**, because every
  workspace gate runs `--all-features` (`Makefile:28`). Four of six reviewers
  found this independently. Corrected, and LEMMA-1 now asserts
  `diagnostics ∈ default`.

- Observation: `cargo tree` walks dev-dependency edges by default, so the
  draft's LEMMA-1 control could never have worked. Evidence: on the unmodified
  tree, `cargo tree -p rstest-bdd -i rstest-bdd-macros` prints the package via
  `[dev-dependencies]`; with `-e normal` it reports "nothing to print". Impact:
  `-e normal` added throughout.

- Observation: `.rustfmt.toml` sets `imports_granularity = "Crate"` and
  `imports_layout = "HorizontalVertical"`. Impact: rustfmt merges the draft's
  three purpose-commented `pub use crate::{…}` statements into one and deletes
  the comments, so the plan contradicted itself between EP-M3 and
  `Interfaces and dependencies`, and a line-oriented parity parser would break
  on the first export addition. Resolved by generating the export list as an
  `insta` snapshot instead of parsing it.

- Observation: the draft export list was incoherent. Evidence: `StepArgs`
  (`crates/rstest-bdd/src/step_args.rs:60`) and `DataTableRow`
  (`crates/rstest-bdd/src/datatable/rows.rs:10`) are both traits *and* derive
  names, like `ScenarioState`; the draft exported the derive for both and the
  trait for neither. The five `#[macro_export]` assertion macros
  (`assert_step_ok!`, `assert_step_err!`, `skip!`, and two skip assertions) sit
  at the crate root and are not picked up by a glob of `prelude`. Impact: list
  corrected from 13/18 inconsistent names to 21 distinct names in complete
  trait/derive groups, and the EP-M4 fixture must exercise every group.

- Observation: the measured cost of the `rstest-bdd` -> `rstest-bdd-macros`
  edge is +30 packages on the normal graph — including the whole `cap-std`/
  `rustix` closure — plus a rebuild of `syn` with `extra-traits`. But every
  documented BDD usage already declares `rstest-bdd-macros` directly
  (`docs/users-guide.md:126, 266, 338, 435, 467`), so for a step author the
  marginal cost is zero. Impact: default-on survives, but `cargo-bdd` and
  `rstest-bdd-harness-gpui` must opt out in the same commit, and the plan's "no
  new third-party dependency" claim was corrected to distinguish declared from
  transitive.

- Observation: publish order does not change. Evidence: `lading.toml` already
  orders `rstest-bdd-macros` before `rstest-bdd`, because
  `crates/rstest-bdd/Cargo.toml:48` already dev-depends on it with a version.
  Impact: recorded so a future reader does not "fix" a non-problem.

- Observation: a crate-root re-export was never considered. Impact: recorded as
  Decision D0, the headline question for approval.

## Decision log

- **Decision D0 — the headline question the panel raised, and the one that most
  changes this deliverable. The draft never considered a crate-root
  re-export.** D1 weighed only three options: a prelude in `rstest-bdd`, a
  prelude in `rstest-bdd-macros` (impossible — a `proc-macro` crate can export
  nothing but macros), and a separate `rstest-bdd-prelude` crate. The obvious
  fourth is `pub use rstest_bdd_macros::{given, when, then, scenario, …};` at
  the `crates/rstest-bdd/src/lib.rs` root with `#[doc(inline)]`, and no
  `prelude` module at all. This is what `cucumber` does. An example would then
  open:

  ```rust
  use rstest_bdd::{given, scenario, then, when, StepResult};
  use rstest_bdd_harness_tokio::TokioTestContext;
  ```

  which is still two framework imports and still passes INV-3, because INV-3
  reduces each `use` to its crate root. "Docs list the exported items" is then
  discharged by rustdoc's own re-export list plus one users-guide paragraph.
  The collisions are already proven safe: the root carries
  `pub use state::{ScenarioState, Slot}` (`lib.rs:111`) and
  `pub use step_args::{StepArgs, StepArgsError}` (`lib.rs:112`), the derives of
  those names live in the macro namespace, and `DataTable`/`DataTableRow`
  traits are not at the root at all.

  |                                       | D1: `prelude` module + glob                                                                 | D0: crate-root re-export                                                                                |
  | ------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
  | Import ergonomics                     | one line, no names                                                                          | one line, names spelled out                                                                             |
  | Downstream `clippy::wildcard_imports` | silenced, because the lint skips paths containing `prelude`                                 | not applicable                                                                                          |
  | SemVer glob hazard                    | real; needs ADR-022 and a curation policy                                                   | none — consumers name what they take                                                                    |
  | New artefacts                         | module, feature, fixture crate, two gate scripts, four tests, ADR, five doc edits, Makefile | roughly ten lines in `lib.rs` plus doc edits                                                            |
  | Adding an export later                | additive and invisible to consumers                                                         | additive, but consumers must edit their `use` to take it                                                |
  | Cost of being wrong                   | a published prelude is permanent for the 0.6 line                                           | adding a prelude later is itself additive                                                               |
  | Prior art in the testing niche        | `proptest`, `googletest`, `assert_cmd`, `predicates`                                        | `rstest`, `cucumber`, `insta`, `mockall`, `test-case`, `serial_test`, `quickcheck`, `pretty_assertions` |

  The prior-art split is the most interesting finding. The testing crates that
  ship a prelude are those whose API is *extension traits and matcher
  functions*, where method syntax fails silently without the trait in scope —
  the one case corrode.dev's "Don't Use Preludes And Globs" concedes is
  legitimate, and the reason `rayon::prelude` earns its keep. `rstest-bdd`'s
  user-facing API is attribute macros, which are named explicitly at the use
  site anyway, so the ergonomic case for a prelude is weakest in exactly this
  shape. Against that, a prelude buys one thing a root glob cannot: Clippy's
  `wildcard_imports` matches on a path segment containing `prelude`, so
  downstream users running pedantic Clippy can glob-import a prelude and cannot
  glob-import a crate root.

  **Recommendation: keep D1 (the prelude module).** The requirement of record
  says "The public prelude exposes…" — the roadmap author asked for a prelude
  by name, and D0 would need a roadmap amendment, not just an ExecPlan one. But
  D0 is a genuinely better default on the merits, and if the word "prelude" in
  the roadmap was inferred rather than intended, it should be adopted: it
  deletes the feature flag, the parity gate, the ADR, the glob hazard, and most
  of the fixture work. **This is the first thing to settle at approval.**
  Date/Author: 2026-09-14, panel review. **Pending approval.**

- Decision D1: the prelude lives at `rstest_bdd::prelude`, in a new file
  `crates/rstest-bdd/src/prelude.rs`, and re-exports a curated set of
  `rstest-bdd` runtime items plus the `rstest-bdd-macros` attribute and derive
  macros. It does not re-export whole crates and does not introduce any new
  type, trait, or function of its own. Rationale: `rstest_bdd` is the crate
  users believe they are using, so that is where the predictable module
  belongs. A curated list — rather than `pub use crate::*` — is what keeps the
  glob-import semver hazard bounded and what makes a docs parity table
  meaningful. Putting the prelude in `rstest-bdd-macros` is impossible: a
  `proc-macro` crate may only export macros. Creating a separate
  `rstest-bdd-prelude` crate was rejected because it adds a published crate and
  defeats "one predictable module". Date/Author: 2026-09-14, planning agent.
  **Pending approval.**

- Decision D2: the prelude does not re-export anything from `rstest`. Examples
  migrate `#[fixture]` to the fully-qualified `#[rstest::fixture]`, which needs
  no `use` statement, so their import list reduces to the prelude plus the
  harness crate as the finish line requires. Rationale, corrected after review.
  The draft argued that two `rstest` copies would "silently mismatch"; that is
  overstated — two rstest majors produce a hard compile error on the
  fixture-resolution protocol, not silent misbehaviour. The decisive argument
  is the one the draft failed to make: **a dependency on `rstest` would put the
  consumer's `rstest` version under `rstest-bdd`'s control**, gating every
  consumer's rstest upgrade on an rstest-bdd release. Generated scenario code
  already emits `#[rstest::rstest]` resolved against the consumer's own
  dependency (`crates/rstest-bdd-harness/src/policy.rs:89`), so the consumer
  must own that version. That is the trade to write into ADR-022, and it
  honours the roadmap's "without hiding the underlying crates" — `rstest` is
  the most important underlying crate of all.

  The exclusion leaves an **implicit contract that must be made explicit**.
  `DefaultAttributePolicy` hard-codes the literal string `"rstest::rstest"`
  (`crates/rstest-bdd-harness/src/policy.rs:89`) and, unlike every first-party
  path, does not resolve it through `proc_macro_crate`. So `rstest-bdd` carries
  an unwritten, unchecked, un-renameable requirement that the consumer declare
  a compatible `rstest` under exactly that name. Three cheap remedies, all in
  scope for EP-M7: state the supported `rstest` range in the users' guide and
  README under ADR-022's ownership; resolve `rstest` through `proc_macro_crate`
  in the default policy so a renamed dependency works and a *missing* one
  yields "add `rstest` to your dev-dependencies" rather than a bare
  `unresolved import rstest`; and derive the EP-M4 fixture's `rstest`
  requirement from `[workspace.dependencies]` rather than duplicating the
  literal `"0.26.1"`. A `compile_error!` version check is not worth it — a proc
  macro can see whether `rstest` is declared, not its resolved version.

  Alternative considered and rejected: re-export `fixture` and `rstest` with a
  caret requirement matching the workspace. Rejected on the version-control
  argument above, not on the mismatch argument.

  **Open for approval — the style tax is separable from the constraint.** The
  constraint (`rstest-bdd` must not depend on `rstest`) is sound and unanimous.
  What does *not* follow is forbidding `use rstest::fixture;` in the examples.
  Each BDD example has exactly one fixture today, so the rewrite is nearly free
  — but it is cheap precisely because the examples are toys. A real suite has
  ten fixtures, where one `use rstest::fixture;` beats ten `#[rstest::fixture]`
  rewrites, and the examples would be modelling a style no user should copy.
  Worse, after migration the examples name `rstest` nowhere while all four
  manifests still require it, which *hides* the underlying crate and inverts
  the roadmap's stated goal. The finish line is already being read to admit the
  example's own crate; admitting `rstest` on the same basis keeps the full
  ergonomic win (four imports down to three, three down to two) with no style
  tax. **Recommendation: adopt D2-prime — keep the dependency exclusion, add
  `rstest` to INV-3's allowed roots, and leave `#[fixture]` alone.** D2 as
  drafted remains the fallback if a strict reading of the finish line is
  preferred. Date/Author: 2026-09-14, planning agent; revised after panel
  review. **Pending approval.**

- Decision D3: the macro re-exports sit behind a `macros` feature on
  `rstest-bdd`, enabled by default, **and the two in-workspace runtime-only
  consumers opt out in the same commit**. `rstest-bdd` gains
  `rstest-bdd-macros = { workspace = true, optional = true }`,
  `default = ["diagnostics", "macros"]`, and
  `macros = ["dep:rstest-bdd-macros"]`. The prelude module itself is
  unconditional; its macro re-export block is `#[cfg(feature = "macros")]`.
  `crates/cargo-bdd/Cargo.toml` and `crates/rstest-bdd-harness-gpui/Cargo.toml`
  both change to `default-features = false, features = ["diagnostics"]`.
  Rationale: the measured cost of the new edge is +30 packages on
  `rstest-bdd`'s normal graph — including the whole `cap-std`/`rustix` closure
  — plus a rebuild of `syn` with `extra-traits`, the most expensive feature
  `syn` offers. That cost is borne entirely by consumers who do not write
  steps. But every documented BDD usage already declares `rstest-bdd-macros`
  directly (`docs/users-guide.md:126, 266, 338, 435, 467`;
  `crates/rstest-bdd/README.md:42`), so for an actual step author those 30
  packages are *already* in the graph and the marginal cost of default-on is
  zero. The only genuine losers are `cargo-bdd` — a shipped CLI binary that
  cannot invoke `#[given]` at all — and `rstest-bdd-harness-gpui`, which wants
  runtime types only. Both are in this workspace, so both are fixed here rather
  than left as a theoretical escape hatch nobody takes. Note that
  `default-features = false` also drops `diagnostics`, which is why both
  opt-outs must name it explicitly. Correction: the draft cited "the `serde`/
  `serde_derive` convention" in support of default-on. That citation was wrong
  and is withdrawn — serde's `derive` feature is opt-in, as this repository's
  own `serde = { version = "1.0", features = ["derive"], optional = true }`
  (`crates/rstest-bdd/Cargo.toml:26`) demonstrates. The argument above stands
  on the measured graph, not on a precedent. Alternative considered: `macros`
  off by default. Rejected because a new user who writes `rstest-bdd = "0.6.1"`
  and `use rstest_bdd::prelude::*;` would get an unresolved-import error for
  the crate's flagship ergonomic feature, which defeats the item's purpose.
  Date/Author: 2026-09-14, planning agent; revised after panel review.
  **Pending approval.**

- Decision D4: `#[harness_context]` is documented as needing no import, and
  that claim is proved by a compile test, rather than by adding a real no-op
  attribute macro. Rationale: the marker is inert and is stripped during
  `#[given]`/`#[when]`/ `#[then]` expansion. Adding a
  `#[proc_macro_attribute] harness_context` whose only job is to be removed
  would create a second resolution path for the same syntax, risk the two
  disagreeing, and expand the public macro surface of a semver-compatible
  release for no user-visible gain. The roadmap's wording is imprecise rather
  than describing missing work; the honest fix is to correct the wording in the
  design document and prove the behaviour. Date/Author: 2026-09-14, planning
  agent. **Pending approval.**

- Decision D5, revised after review: enforcement uses three complementary
  artefacts — a dependency-restricted compile fixture (sufficiency), an
  import-allowlist contract test (actuality), and a docs/exports parity gate
  (documentation) — with the parity gate reading an `insta` snapshot emitted by
  the export-identity test rather than parsing `prelude.rs`. The draft folded
  the snapshot away as duplication; that was backwards. The snapshot is what
  makes the gate robust, because `.rustfmt.toml`'s
  `imports_granularity = "Crate"` reflows the `pub use` block on every addition
  and would defeat a line-oriented parser at precisely the moment the gate must
  be right. Rationale: none of the three alone discharges the finish line. The
  compile fixture cannot prove the examples actually use the prelude; the
  allowlist test cannot prove the prelude is sufficient for a crate that does
  not already depend on the macro crate; neither touches documentation.
  Date/Author: 2026-09-14, planning agent. **Pending approval.**

- Decision D6: `examples/todo-cli/tests/cli.rs` is out of scope for the import
  allowlist. The allowlist applies to files that contain at least one
  `#[scenario]`, `#[given]`, `#[when]`, or `#[then]` attribute. Rationale:
  `cli.rs` is an `assert_cmd` end-to-end driver with no BDD steps; its
  `rstest_bdd_harness::binary_test_support` import is test-runner plumbing.
  Forcing it through the prelude would pull a binary locator into the
  framework's public ergonomic surface for no user benefit. Date/Author:
  2026-09-14, planning agent. **Pending approval.**

## Outcomes & retrospective

To be completed at EP-M8. Before setting this plan to `COMPLETE`, reconcile
every implementation discovery with the artefacts named in `Conformance basis`:
update `docs/rstest-bdd-design.md` §2.7.6.4 to move "a prelude for common
integration imports" from the not-yet-shipped list to delivered, confirm
ADR-022 records the final export policy, and confirm the ADR-007 addendum
carries the D4 clarification. A purely mechanical difference may instead be
recorded in `Decision log`.

## Context and orientation

### What this repository is

`rstest-bdd` is a behaviour-driven-development (BDD) testing framework for
Rust, layered on top of the `rstest` fixture-injection crate. A user writes a
Gherkin `.feature` file, annotates ordinary Rust functions with `#[given]`,
`#[when]`, and `#[then]` to bind them to the steps in that file, and then
annotates one function per scenario with `#[scenario(path = "...")]`. The
`#[scenario]` macro expands to an `rstest` test that looks each step up in a
global registry and runs it.

The workspace root is `Cargo.toml`. The crates that matter for this plan are:

- `crates/rstest-bdd` — the runtime library. Defines the step registry, the
  `StepContext` fixture store, the error and state types, and the localization
  machinery. This is the crate that will gain the prelude.
- `crates/rstest-bdd-macros` — a `proc-macro` crate. Exports the `given`,
  `when`, `then`, and `scenario` attribute macros, the `scenarios!`
  function-like macro, and the `ScenarioState`, `StepArgs`, `DataTableRow`, and
  `DataTable` derive macros. A `proc-macro` crate can export nothing else, so
  the prelude cannot live here.
- `crates/rstest-bdd-harness` — the harness abstraction: `HarnessAdapter`,
  `AttributePolicy`, `TestAttribute`, and the scenario-runner traits. Both
  first-party adapter crates re-export its whole public surface verbatim.
- `crates/rstest-bdd-harness-tokio` and `crates/rstest-bdd-harness-gpui` —
  the first-party adapters. These are the "harness crate" the roadmap item
  refers to.
- `crates/rstest-bdd-policy` — a tiny dependency-free crate holding shared
  constants, notably `HARNESS_CONTEXT_FIXTURE`
  (`crates/rstest-bdd-policy/src/lib.rs:114`), which exists so the macro crate
  and the runtime crate can agree on the reserved fixture key without one
  depending on the other.

`examples/todo-cli`, `examples/japanese-ledger`, `examples/gpui-counter`, and
`examples/tokio-reminders` are ordinary workspace members (`Cargo.toml` lines
14-17), so they are compiled, linted, and tested by the normal gates.

### Terms of art used in this plan

- **Prelude.** A module, conventionally named `prelude`, whose only content is
  re-exports of the names a user most often needs, intended to be glob-imported
  with `use some_crate::prelude::*;`. `std` has one; `rayon`, `pyo3`, `bevy`,
  and `proptest` have one. `tokio` had one and removed it at 1.0
  (tokio-rs/tokio#3257) on the grounds that it did not carry its weight.
- **Glob import / wildcard import.** `use path::*;` — brings every public item
  of `path` into scope at once.
- **Re-export.** `pub use other::Item;` — makes `Item` reachable through this
  module as well as its original path. It does not move or copy the item; the
  original path keeps working, and the two paths name the same item.
- **Inert attribute / marker attribute.** An attribute that is not itself a
  macro. It is written in the source purely so an enclosing macro can see it
  and act on it, and the enclosing macro removes it before the compiler ever
  tries to resolve it. `#[harness_context]` is one of these. Because it is
  never resolved as a path, it never needs importing — and, symmetrically, it
  cannot be re-exported, because there is no item to export.
- **Attribute policy.** The mechanism by which a harness decides which test
  attribute to stamp onto the generated test function. The default policy emits
  `#[rstest::rstest]` (`crates/rstest-bdd-harness/src/policy.rs:89`); the Tokio
  policy emits a Tokio-flavoured variant.
- **Trybuild fixture.** A `.rs` file compiled in isolation by the `trybuild`
  crate, either expected to compile (a pass fixture) or expected to fail with a
  recorded `.stderr` (a compile-fail fixture).
- **Finish line.** The roadmap's own acceptance sentence for an item. For
  11.2.2 it is: "compile tests prove examples import only the prelude plus
  their harness crate, and docs list the exported items."

### Where the candidate items live today

Every item the roadmap names already exists and is already public. This plan
adds a second path to each, never a new definition.

| Item                                    | Kind                | Current path                                          | Defined at                                |
| --------------------------------------- | ------------------- | ----------------------------------------------------- | ----------------------------------------- |
| `StepResult<T, E>`                      | type alias          | `rstest_bdd::StepResult`                              | `crates/rstest-bdd/src/lib.rs:294`        |
| `StepError`                             | enum                | `rstest_bdd::StepError`                               | `crates/rstest-bdd/src/lib.rs:194`        |
| `Slot<T>`                               | struct              | `rstest_bdd::Slot`, `rstest_bdd::state::Slot`         | `crates/rstest-bdd/src/state.rs:30`       |
| `ScenarioState`                         | trait               | `rstest_bdd::ScenarioState`                           | `crates/rstest-bdd/src/state.rs:125`      |
| `ScenarioState`                         | derive macro        | `rstest_bdd_macros::ScenarioState`                    | `crates/rstest-bdd-macros/src/lib.rs:164` |
| `StepContext`                           | struct              | `rstest_bdd::StepContext`                             | `crates/rstest-bdd/src/context/mod.rs`    |
| `FixtureRef`, `FixtureRefMut`           | structs             | `rstest_bdd::FixtureRef`, `rstest_bdd::FixtureRefMut` | `crates/rstest-bdd/src/context/mod.rs`    |
| `FixtureBorrowError`                    | enum                | `rstest_bdd::FixtureBorrowError`                      | `crates/rstest-bdd/src/context/mod.rs`    |
| `RSTEST_BDD_HARNESS_CONTEXT_FIXTURE`    | const               | `rstest_bdd::RSTEST_BDD_HARNESS_CONTEXT_FIXTURE`      | `crates/rstest-bdd/src/context/mod.rs:49` |
| `given`, `when`, `then`, `scenario`     | attribute macros    | `rstest_bdd_macros::*`                                | `crates/rstest-bdd-macros/src/lib.rs`     |
| `scenarios!`                            | function-like macro | `rstest_bdd_macros::scenarios`                        | `crates/rstest-bdd-macros/src/lib.rs`     |
| `StepArgs`, `DataTable`, `DataTableRow` | derive macros       | `rstest_bdd_macros::*`                                | `crates/rstest-bdd-macros/src/lib.rs`     |

The "harness-context helpers" the roadmap names are the four methods on
`StepContext`, all already public: `insert_owned_harness_context`
(`crates/rstest-bdd/src/context/mod.rs:179`), `harness_context` (line 192),
`borrow_harness_context` (line 202), and `borrow_harness_context_mut` (line
215). Methods are reached through their type, so exporting `StepContext`
exports all four; there is nothing further to list.

`crates/rstest-bdd/src/lib.rs` currently has no `prelude` module, and neither
does any other crate in the workspace. Note that `lib.rs:28-29` already
re-exports third-party items bare (`FluentLanguageLoader` from `i18n_embed`,
`iter` and `submit` from `inventory`), so re-exporting from another crate is
not a new practice here.

### What the examples import today

```plaintext
examples/todo-cli/tests/todo.rs:3-5
    use rstest::fixture;
    use rstest_bdd_macros::{given, scenario, then, when};
    use todo_cli::TodoList;

examples/japanese-ledger/tests/ledger.rs:7-9
    use japanese_ledger::HouseholdLedger;
    use rstest::fixture;
    use rstest_bdd_macros::{given, scenario, then, when};

examples/gpui-counter/tests/counter.rs:7-9
    use gpui_counter::CounterApp;
    use rstest::fixture;
    use rstest_bdd_macros::{given, scenario, then, when};

examples/tokio-reminders/tests/reminders.rs:6-9
    use rstest::fixture;
    use rstest_bdd_harness_tokio::TokioTestContext;
    use rstest_bdd_macros::{given, scenario, then, when};
    use tokio_reminders::ReminderService;
```

After EP-M5 each of these becomes the prelude, the example's own crate, and —
only where a harness type is named in a signature — the harness adapter:

```plaintext
examples/tokio-reminders/tests/reminders.rs
    use rstest_bdd::prelude::*;
    use rstest_bdd_harness_tokio::TokioTestContext;
    use tokio_reminders::ReminderService;
```

The harness named in `#[scenario(harness = ...)]` is already written fully
qualified and needs no import (see `examples/gpui-counter/tests/counter.rs:56`).
`gpui::TestAppContext` in the GPUI example is likewise already written fully
qualified at the use site.

### Why the marker attribute cannot be re-exported

Roadmap 11.2.2 asks the prelude to expose "marker attributes from 11.2.1".
There is no such item. `crates/rstest-bdd-macros/src/lib.rs` declares exactly
four `#[proc_macro_attribute]` functions — `given`, `when`, `then`, and
`scenario` — and `#[harness_context]` is not among them. The marker is
recognized during argument classification in
`crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_context/mod.rs`
and stripped from the emitted function, so the compiler never resolves it as a
path.

This is not a gap in 11.2.1's delivery; it is how inert markers work, and it is
the same mechanism `rstest` uses for `#[from(...)]` and `#[case]`. The 11.2.1
ExecPlan's `Decision log` deferred the question of prelude exposure to this
item precisely so it could be settled here. Decision D4 settles it: the prelude
exposes `given`, `when`, and `then`, and those attributes understand
`#[harness_context]` without any further import. EP-M4's compile fixture makes
that a tested claim rather than an assertion.

### Existing gate machinery this plan reuses

- `make check-fmt` (`Makefile:202-206`) — `cargo fmt --all -- --check` plus the
  two out-of-workspace GPUI fixture manifests plus `ruff format --check`.
- `make lint` (`Makefile:110-119`) — Clippy over
  `--workspace --all-targets --all-features` with `-D warnings`; then
  `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS` denying warnings; then
  the Whitaker Dylint suite; then the Python linters; then five Python gate
  scripts. **The new docs/exports parity gate is added to this list.**
- `make test` (`Makefile:95-105`) — builds the binaries, runs
  `cargo nextest run --workspace --all-targets --all-features`, runs
  `cargo test --doc --workspace --all-features` (this is where new prelude
  doctests execute), runs `make check-fixture-lockfiles`, and finally runs
  `pytest scripts/tests`. **The new import-allowlist contract test lands in
  `scripts/tests/` and is picked up by that last step.**
- `make markdownlint` (`Makefile:208-209`) — markdownlint-cli2 preceded by the
  `spelling` gate.
- `make fmt` (`Makefile:193-200`) — the write side, including `mdformat-all`.
  Per project experience, `make fmt` on Markdown can itself introduce MD039 or
  MD013 findings, so always re-run `make markdownlint` afterwards.

Two precedents are worth reading before writing the new gates:

- `scripts/check_gpui_mapping_table.py` with its test
  `scripts/tests/test_check_gpui_mapping_table.py` — a doc-versus-doc table
  parity gate wired into `make lint` at `Makefile:118`. This is the template
  for the docs/exports parity gate.
- `scripts/tests/test_example_workspace_lints.py` — a `pytest` that parses all
  four example manifests with `tomllib` and asserts a structural property. This
  is the template for the import-allowlist contract test.
- `crates/rstest-bdd/tests/fixtures/rebuild_invalidation/` — a crate outside
  the root workspace, using plain `path =` dependencies, with a committed
  lockfile that `scripts/check_fixture_lockfiles.py` discovers automatically.
  This, *not* `tests/fixtures/published-gpui-e2e/`, is the template for the
  dependency-restricted compile fixture; the published-GPUI fixtures use the
  heavier staged pattern, which does not apply here.

### Skills and documentation to load before starting

Load these before writing code, not after:

- `rust-router`, then `arch-crate-design` (crate boundaries, public versus
  internal API, feature flags) and `rust-unit-testing` (fixtures, table tests,
  `googletest` and `pretty_assertions` vocabulary).
- `execplans` — this document's own format rules, including the requirement to
  keep the living sections current.
- `arch-decision-records` — the Y-Statement format required for ADR-022.
- `codegraph-mcp` for structural questions (callers, impact) in preference to
  grep.
- `python-router` and `ruff-016` before touching `scripts/`.

Read, in this order:

- `AGENTS.md` — the binding style, testing, dependency, and documentation
  rules for this repository.
- `docs/roadmap.md` lines 1038-1043 — the requirement of record.
- `docs/rstest-bdd-design.md` §2.7.6.3 (lines 2155-2180) for the harness
  dependency-matrix rationale and §2.7.6.4 (lines 2182-2196) for the v0.6.1
  helper programme this item belongs to.
- `docs/execplans/11-2-1-annotate-parameters-with-harness-context.md` — the
  immediately preceding item, whose `Decision log` (lines 363-372) defers the
  marker-attribute question here.
- `docs/adr-007-harness-context-injection.md` including its 11.2.1 addendum.
- `docs/users-guide.md` — "Scenario state slots" (lines 546-609, containing the
  existing `use rstest_bdd::{ScenarioState, Slot};` at line 567), "Mutable
  world fixtures" (the import at line 337), the `#[harness_context]` narrative
  (lines 1076-1113), "Fixture key versus parameter name" (lines 1656-1668), and
  "Harness adapter core APIs" (lines 2326-2441).
- `docs/developers-guide.md` — "Rust documentation policy and gates" (lines
  469-491), "Shared policy crate (rstest-bdd-policy)" (lines 1802-1862, the
  closest existing precedent for documenting a re-export boundary), and the
  "`googletest` and `pretty_assertions` house style" section (lines 2827-2839).
- `docs/rust-doctest-dry-guide.md` — in particular that doctests compile
  against the public API only (§1.1), that `.unwrap()`/`.expect()` are banned
  in examples in favour of the `Result`-returning `main` or the `(())`
  shorthand (§2.3), that hidden `#` lines must not hide the point of the
  example (§2.4), and the repository-specific note at lines 239-243 about
  keeping fixture-naming rules visible.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` before writing
  the gate scripts, whose cyclomatic budget is 12 (`clippy.toml`).

## Conformance basis

There is no separate Terms of Reference document for this work. The upstream
artefacts of record are:

- **REQ (requirement of record):** `docs/roadmap.md` lines 1038-1043, item
  11.2.2. Finish line, verbatim: *"compile tests prove examples import only the
  prelude plus their harness crate, and docs list the exported items."*
  Prerequisite: 11.2.1 (complete, 2026-08-17).
- **DESIGN:** `docs/rstest-bdd-design.md` §2.7.6.4 (lines 2182-2196), which
  lists "a prelude for common integration imports" among the v0.6.1 candidates
  not yet shipped. §2.7.6.3 (lines 2155-2180) constrains the outcome:
  first-party adapter users should be able to depend only on the selected
  adapter crate, and the prelude must not undermine that.
- **ADR:** `docs/adr-007-harness-context-injection.md` and its 11.2.1 addendum
  own the harness-context marker semantics. This plan adds ADR-022 for the
  prelude export policy and amends the ADR-007 addendum with the D4
  clarification.
- **PHASE CONSTRAINT:** `docs/roadmap.md` lines 995-999 — "The v0.6.1 line
  should stay semver-compatible… it must not remove the existing `StepContext`,
  harness, or macro surfaces."
- **STANDARDS:** `AGENTS.md`; `docs/documentation-style-guide.md`;
  `docs/rust-doctest-dry-guide.md`.

Trace links from requirement through milestone to evidence:

```plaintext
REQ-11.2.2 (prelude exposes the named items)
    -> DESIGN-2.7.6.4 (v0.6.1 prelude candidate)
        -> EP-M2 -> scripts/tests/test_rstest_bdd_macros_feature.py
        -> EP-M3 -> crates/rstest-bdd/tests/prelude_exports.rs
REQ-11.2.2-FL-a (compile tests prove examples import only prelude + harness)
    -> EP-M4 -> crates/rstest-bdd/tests/fixtures/prelude_only/ compiles
    -> EP-M5 -> examples/*/tests/*.rs import lists
    -> EP-M6 -> scripts/tests/test_example_prelude_imports.py
REQ-11.2.2-FL-b (docs list the exported items)
    -> EP-M7 -> docs/users-guide.md "Prelude" table
    -> EP-M7 -> scripts/check_prelude_exports.py wired into `make lint`
PHASE-CONSTRAINT-semver
    -> EP-M3 -> crates/rstest-bdd/tests/prelude_exports.rs::original_paths_still_resolve
ADR-007-addendum (marker needs no import)
    -> EP-M4 -> prelude_only fixture: marker usage compiles
```

## Verification plan

This change is mostly a re-export surface, so most obligations are structural
rather than algorithmic. The section states each one honestly, including the
two that are genuinely non-trivial, and says plainly where an obligation is
discharged by a compile-time fact rather than a runtime assertion.

### Axioms (assumed, not verified here)

- `rustc` resolves `pub use` re-exports to the same item as the original path,
  for types, traits, constants, function-like macros, derive macros, and
  attribute macros alike. This is the language definition; this plan does not
  attempt to verify it. It is *relied upon* by the whole design, so EP-M3
  includes a witness test that would fail if it were false in some
  configuration this repository actually uses.
- `trybuild` and `rstest` behave as documented. Their internals are not
  verified.

Two further assumptions were carried as axioms in the draft. Both have since
been **verified empirically** during planning, with throwaway probe crates that
were deleted afterwards, so neither is an axiom any longer:

- *Clippy's `wildcard_imports` does not fire on a glob from a module named
  `prelude`.* Confirmed: a probe crate under `#![warn(clippy::pedantic)]` with
  `-D warnings` and a `use crate::prelude::*;` produced `dead_code` and
  `doc_markdown` findings but no `wildcard_imports`. `clippy.toml` does not set
  `warn-on-all-wildcard-imports`, whose default is `false`. EP-M0 re-runs the
  probe against the real workspace configuration before any work depends on it.
- *A trait and a same-named derive macro coexist through one glob import.*
  Confirmed against the real crates: a probe declaring
  `pub use rstest_bdd::{ScenarioState, Slot, StepContext, StepError, StepResult};`
  and
  `pub use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};` in
  one module, glob-imported by a consumer, compiled clean with
  `#[derive(ScenarioState)]`, a `T: ScenarioState` bound, `Slot`, `StepResult`,
  and `#[given]` all resolving. Type and macro namespaces are genuinely
  separate; `serde`'s `pub use serde_derive::Serialize` beside
  `pub trait Serialize` is the same shape. EP-M3 should state this as fact and
  drop the draft's contingency prose about renaming the derive.

The draft also carried an axiom that "Cargo permits a dev-dependency cycle
(`rstest-bdd` -> `rstest-bdd-macros` normal, `rstest-bdd-macros` -> `rstest`
dev)". That was confused — `rstest` is not `rstest-bdd`, `rstest-bdd-macros`
has no dev edge back to `rstest-bdd`, and there is no cycle of any kind. The
axiom is withdrawn; EP-M2 was de-risking something that does not exist.

Publish order is likewise unaffected, contrary to what a reader might expect:
`lading.toml` already orders `rstest-bdd-macros` before `rstest-bdd`, because
`crates/rstest-bdd/Cargo.toml:48` already dev-depends on it *with a version*.
Promoting that edge from dev to normal changes nothing, and no tooling encodes
a stale order.

### Obligations

**INV-1 — Prelude sufficiency.**

- Obligation: a crate whose manifest names only `rstest-bdd`, `rstest`, and one
  first-party harness adapter, and whose source imports only
  `rstest_bdd::prelude::*` plus that adapter, can express a complete scenario:
  fixtures, all three step keywords, a fallible step returning `StepResult`, a
  `ScenarioState` implementation using `Slot`, a `#[harness_context]`
  parameter, and a `#[scenario]` binding.
- Method: a dependency-restricted compile fixture — a crate under
  `crates/rstest-bdd/tests/fixtures/prelude_only/`, outside the root workspace,
  compiled by `cargo test` against its own manifest.
- Rationale: this is a statement about *name resolution under a restricted
  dependency set*. Only a real compile against a real restricted manifest can
  establish it; a test inside `crates/rstest-bdd` would inherit that crate's
  dev-dependencies and prove nothing.
- Domain: one crate exercising each category of exported item at least once.
  See the checklist in EP-M4.
- Artefact: `crates/rstest-bdd/tests/fixtures/prelude_only/` plus the Makefile
  target that builds
  it.
- Evidence: before EP-M3, the fixture fails to compile with
  `unresolved import rstest_bdd::prelude`. After EP-M3 and EP-M4 it compiles
  and its scenario passes.
- Non-vacuity: the fixture must not merely compile — it must *run* a scenario
  that asserts something, so a fixture that silently bound zero steps would
  fail. The negative control is explicit: EP-M4 records a transcript from
  temporarily deleting one name (`Slot`) from the prelude's export list and
  observing the fixture fail to compile with
  `cannot find type Slot in this scope`. A fixture that still compiled after
  that deletion would prove the test was not exercising the export.

**INV-2 — Additive-only public surface.**

- Obligation: every path that resolved before this change still resolves
  afterwards, and resolves to the *same* item. In particular `rstest_bdd::Slot`,
  `rstest_bdd::state::Slot`, and `rstest_bdd::prelude::Slot` must all be one
  type, not three.
- Method: parameterized compile-time identity assertions in a new integration
  test, in the style of the existing
  `crates/rstest-bdd/tests/fixtures_macros/execution_policy_reexports.rs`
  type-identity fixture.
- Rationale: type identity across re-export paths is a compile-time property;
  asserting it at compile time is both stronger and cheaper than any runtime
  check. For the macro re-exports, identity is demonstrated by using the
  prelude-pathed and origin-pathed attribute on equivalent functions and
  observing equivalent behaviour, since a proc-macro has no value to compare.
- Domain: every name in the prelude export list, enumerated exhaustively. The
  parity gate (INV-4) is what keeps "exhaustively" true as the list changes.
- Artefact: `crates/rstest-bdd/tests/prelude_exports.rs`.
- Evidence: `cargo nextest run -p rstest-bdd prelude_exports` passes; it fails
  to compile if a name is dropped from either path.
- Non-vacuity: the identity assertions are written so that they fail if the two
  paths name different types. The negative control is a transcript from
  temporarily pointing one prelude entry at a deliberately different type
  (`pub use crate::StepText as Slot;`) and observing the test fail to compile
  with a type-mismatch diagnostic, not merely a name error.

**INV-3 — Example import closure.**

- Obligation: for every file under `examples/*/tests/` and `examples/*/src/`
  that contains a `#[scenario]`, `#[given]`, `#[when]`, or `#[then]` attribute,
  the set of crate roots named by its `use` statements is a subset of {
  `rstest_bdd::prelude`, the example's own crate, the example's declared
  harness adapter crate, `std`, `core`} — plus `rstest` if D2-prime is adopted.
- **Honesty caveat, raised by two reviewers and required in the users' guide.**
  This obligation measures `use` lines, not crates. Every example applies
  `#[rstest_bdd_test_macros::allow_fixture_expansion_lints]` to its fixture
  (`examples/gpui-counter/tests/counter.rs:11`,
  `examples/todo-cli/tests/todo.rs:9`, and the other two), and names its
  harness fully qualified in `#[scenario(harness = …)]`. After EP-M5 each file
  still *references* three or four crates; it just names most of them in
  attribute position rather than in a `use`. Do not let the green gate oversell
  the outcome: state the goal in the users' guide and ADR-022 as **"one `use`
  line for the framework"**, which is true and valuable, rather than "only the
  prelude plus the harness crate", which is true only of import statements. If
  a stronger claim is wanted, the checker must also count crate roots in
  attribute and type position — that is a larger job and should be a separate
  roadmap item.
- Method: a `pytest` contract test parsing each file's `use` statements and
  each example's manifest.
- Rationale: this is a property of the repository's source text over a small,
  enumerable file set. A parameterized structural test is exactly proportionate
  — a property test over generated inputs would have no meaningful generator,
  and a model checker has no state machine to explore.
- Domain: all four example crates; currently five files, of which four are in
  scope and `examples/todo-cli/tests/cli.rs` is excluded per Decision D6. The
  test discovers files rather than hard-coding the list, so a new example is
  covered automatically.
- Artefact: `scripts/tests/test_example_prelude_imports.py`.
- Evidence: the test fails before EP-M5 (four files import
  `rstest_bdd_macros`), passes after.
- Non-vacuity: the test must assert that it actually found at least four
  in-scope files and at least one `use` statement per file; a discovery bug
  that silently matched nothing would otherwise pass. The negative control is a
  unit test within the same file that runs the checker over a synthetic
  temporary tree containing a deliberately non-compliant file and asserts the
  checker reports it — this exercises the rejection path without mutating the
  real examples.

**INV-4 — Documentation parity.**

- Obligation: the set of **(name, kind, source path)** rows in the users'
  guide prelude table is exactly the set of items re-exported by
  `rstest_bdd::prelude`. Rows, not bare names: `ScenarioState`, `StepArgs`, and
  `DataTableRow` are each exported twice — the trait from `crate::`, the derive
  from `rstest_bdd_macros::` — so a name-set comparison would silently collapse
  three pairs and let a half-missing pair pass. This is also why the snapshot,
  not a `pub use` parser, must be the source of truth: a `pub use` block does
  not record item kind, but the export-identity test necessarily knows it.
- Method: **an `insta` snapshot as the single source of truth**, plus a Python
  gate comparing the users-guide table against that snapshot. The snapshot is
  emitted by the INV-2 test, which already has to enumerate every export.
- Rationale, revised after review. The draft proposed a line-oriented parser
  reading `pub use` statements out of `prelude.rs`. That is the plan's most
  fragile artefact and it would break at exactly the wrong moment.
  `.rustfmt.toml` sets `imports_granularity = "Crate"` and
  `imports_layout = "HorizontalVertical"`, so adding one export reflows the
  whole brace group and merges statements — no line's content is stable. The
  predictable outcome is a red CI on an unrelated pull request, a contributor
  editing the table until it goes green, and a genuinely undocumented export
  landing through the gate that exists to prevent exactly that. Comparing two
  Markdown/text artefacts, with the Rust side generated rather than parsed,
  removes the parser entirely and leaves one source of truth. "Docs list the
  exported items" remains half the finish line and handwritten lists still
  drift; the repository has no public-API tooling to lean on (confirmed: no
  `cargo-public-api`, no `cargo-semver-checks`). Considered and rejected:
  dropping the gate and linking to the rustdoc page instead. Rustdoc's
  generated list cannot go stale and is genuinely better documentation — but it
  lives on docs.rs, not in the repository, so it cannot be diffed in review and
  cannot fail a gate. Do both: link the rustdoc page from the users' guide
  *and* keep the in-repo table gated.
- Domain: the full export list of 21 distinct names.
- Artefact: the snapshot at
  `crates/rstest-bdd/tests/snapshots/prelude_exports__surface.snap`, the gate
  `scripts/check_prelude_exports.py`, its tests in
  `scripts/tests/test_check_prelude_exports.py`, and Makefile wiring at the end
  of the `lint` recipe.
- Evidence: running the gate against a table missing one row exits 1 naming
  that row; against the committed pair it exits 0. Adding an export without
  updating the table fails `make lint`; adding one without updating the
  snapshot fails `make test` with an `insta` diff.
- Non-vacuity: the gate must fail if the table is empty, if the snapshot is
  empty, or if the marker headings cannot be found — an extractor that parsed
  nothing would otherwise compare two empty sets and pass. The unit tests
  assert each of those failure modes explicitly, plus a missing-row case and a
  surplus-row case, so the gate is shown to reject in both directions. The
  snapshot's own non-vacuity is INV-2: a name absent from the prelude fails to
  compile before it can reach the snapshot.

**LEMMA-1 — Feature-gated macro re-exports do not change the default build.**

- Obligation: with default features, `rstest_bdd::prelude::given` resolves; with
  `--no-default-features`, the crate still compiles, the runtime items still
  resolve through the prelude, and `rstest-bdd-macros` is not in the dependency
  graph.
- Method: a manifest contract test plus two explicit `cargo check` invocations
  recorded in `Concrete steps`.
- Rationale: a feature flag's whole purpose is that both settings work; only
  building both settings establishes it. `make lint` and `make test` use
  `--all-features`, so the `--no-default-features` path would otherwise never
  be exercised.
- Domain: the two settings of the single `macros` feature.
- Artefact: `scripts/tests/test_rstest_bdd_macros_feature.py` for the manifest
  shape; the two `cargo` commands for the build reality.
- Evidence: `cargo check -p rstest-bdd --no-default-features` succeeds, and
  `cargo tree -p rstest-bdd --no-default-features -e normal --prefix none`
  output does **not** contain `rstest-bdd-macros`. The `-e normal` filter is
  load-bearing: without it `cargo tree` also walks dev-dependency edges, and
  `crates/rstest-bdd/Cargo.toml:48` dev-depends on `rstest-bdd-macros`
  unconditionally, so the query would succeed both before and after the change
  and prove nothing. Prefer this positive absence check over the draft's
  `-i rstest-bdd-macros` "did not match any packages" oracle, which also fires
  for a mistyped package name.
- **Additional obligation, from the panel: `diagnostics` must stay in
  `default`.** Every workspace gate runs `--all-features` (`Makefile:28`), so
  the default feature set is otherwise never exercised and a regression there
  would reach consumers unseen.
  `scripts/tests/test_rstest_bdd_macros_feature.py` asserts that `default`
  contains both `diagnostics` and `macros`, and `Concrete steps` adds a plain
  `cargo check -p rstest-bdd` plus one run of the `dump_registry` test target
  under default features.
- Non-vacuity: the `cargo tree` check is the control — if the feature gate were
  ineffective, `rstest-bdd-macros` would appear and the step would fail. The
  manifest test additionally asserts the feature is listed in `default`, so a
  refactor that quietly dropped it from `default` is caught.

### Obligations deliberately not taken

There is no invariant here that ranges over an unbounded input domain, no
ordering or concurrency property, and no arithmetic or memory-safety question.
Consequently this plan uses no `proptest` property test, no `kani` harness, and
no `verus` proof, and that is a considered conclusion rather than an omission.
Introducing one would be vacuous: a property test over a fixed, enumerable list
of twenty-one re-exported names degenerates into the parameterized test already
specified in INV-2, and a bounded model check has no transition system to
explore. If implementation reveals a genuine invariant over a range — for
example, if the parity gate's extractor needs to handle arbitrary nested `cfg`
expressions — return to this section, state the invariant, and add a `proptest`
over generated `pub use` blocks before continuing.

## Plan of work

Each milestone ends in a state the repository could sit in indefinitely. No
milestone introduces a shim, alias, or facade that a later milestone removes;
the prelude is additive from the first commit, and the examples move in one
step once the prelude is complete.

### EP-M0 — orientation, green baseline, and the wildcard-import probe

No production changes. Establish that the tree is green before touching it, and
settle the one assumption the whole design rests on.

Run the three gates and record the result. Then write a throwaway glob import
of an existing prelude-shaped module into a scratch file and confirm Clippy
does not object under this workspace's configuration. The cheapest honest probe
is to add, temporarily, a `pub mod prelude { pub use crate::StepResult; }` to
`crates/rstest-bdd/src/lib.rs` and a `use rstest_bdd::prelude::*;` to an
existing integration test, run `make lint`, observe it pass, and revert both.

If `make lint` reports `clippy::wildcard_imports` on that glob, stop. The
design's ergonomic premise is false under this configuration and the choice
between setting `warn-on-all-wildcard-imports` explicitly, adding a scoped
`#[expect]`, and abandoning glob imports in favour of named prelude imports is
the user's, not the implementer's.

Validation: `make check-fmt && make lint && make test` all green, working tree
clean, probe reverted.

### EP-M1 — red: failing tests that specify the prelude

Write the tests before the module exists. Each must fail for the intended
reason, and the reason must be recorded.

Create `crates/rstest-bdd/tests/prelude_exports.rs`, holding the INV-2 identity
assertions. Use `googletest`'s `#[gtest]` and `assert_that!` for value
assertions and `pretty_assertions` for structural comparisons, per the house
style at `docs/developers-guide.md` lines 2827-2839. The type-identity checks
are compile-time. The draft proposed `const _: fn(OriginPath) = |_| ();`; that
shape proves nothing, because the closure's parameter type is *inferred* and
will happily unify with whatever it is given. Use a typed identity function
instead, which fails to compile whenever the two paths name different types:

```rust
fn _slot_paths_agree(x: rstest_bdd::state::Slot<u8>) -> rstest_bdd::prelude::Slot<u8> { x }
```

Read `crates/rstest-bdd/tests/fixtures_macros/execution_policy_reexports.rs`
and follow its actual shape rather than inventing one; it already solves this
problem for `RuntimeMode` and `TestAttributeHint`. Macros have no value to
compare, so for the macro re-exports demonstrate equivalence by applying the
prelude-pathed and origin-pathed attribute to equivalent functions and
asserting equivalent behaviour. Prefer a small helper macro over twenty-one
near-identical blocks, but keep the helper readable — this file exists to be
diffed by reviewers.

Create `scripts/tests/test_example_prelude_imports.py` with its unit tests and
the synthetic-tree negative control, but **only a stub checker** — EP-M6 writes
the checker. The draft specified the checker in both milestones; keep the
specification in EP-M6 and let EP-M1 own only the failing expectations.

Create `scripts/tests/test_check_prelude_exports.py` with the INV-4 gate's unit
tests, including the empty-table, empty-export-list, missing-heading,
missing-row, and surplus-row cases.

Create `scripts/tests/test_rstest_bdd_macros_feature.py` with the LEMMA-1
manifest assertions.

Expected red state, recorded as a transcript in `Artefacts and notes`:

- `cargo nextest run -p rstest-bdd prelude_exports` fails to compile with
  `unresolved import rstest_bdd::prelude`.
- `pytest scripts/tests/test_example_prelude_imports.py` fails, naming the four
  example files that import `rstest_bdd_macros`.
- `pytest scripts/tests/test_check_prelude_exports.py` fails with
  `ModuleNotFoundError: scripts.check_prelude_exports`.
- `pytest scripts/tests/test_rstest_bdd_macros_feature.py` fails because
  `rstest-bdd` has no `macros` feature.

Do not proceed until each failure message matches the expectation. A test that
fails for the wrong reason is not a red test.

Validation: the four commands above fail as described; `make check-fmt` is
green.

### EP-M2 — manifest: wire `rstest-bdd-macros` into `rstest-bdd`

**Panel note on this boundary.** As drafted, EP-M2 commits a default-on feature
enabling a dependency that no code references — a state the plan's own
`Milestones and plateaus` says the repository must be able to "sit in
indefinitely", which it should not if published. Either fold EP-M2 into EP-M3
and keep the feature-matrix checks as a named validation step within it, or
keep the split and accept that this plateau is a build-graph checkpoint rather
than a shippable state. The plan keeps the split, because the panel's measured
findings — +30 packages, a `syn` rebuild with `extra-traits`, and two consumers
needing opt-outs — are exactly the kind of surprise worth isolating in one
revertible commit. Say so rather than claiming it is a shippable plateau.

Edit `crates/rstest-bdd/Cargo.toml`, `crates/cargo-bdd/Cargo.toml`, and
`crates/rstest-bdd-harness-gpui/Cargo.toml`. Add to `rstest-bdd`'s
`[dependencies]`:

```toml
rstest-bdd-macros = { workspace = true, optional = true }
```

Amend the **existing** `[features]` section. `crates/rstest-bdd/Cargo.toml`
lines 63-71 already declare `default = ["diagnostics"]` alongside `diagnostics`,
`mutable_world_macro`, and `test-support`, and the `[[test]] dump_registry`
target at lines 73-76 carries `required-features = ["diagnostics"]`. Adding
`macros` to `default` — rather than replacing it — is mandatory; dropping
`diagnostics` would silently turn off `serde`, `serde_json`, and
`dump_registry` for every consumer, which is precisely the semver-visible
regression the first `Constraint` forbids.

```toml
[features]
default = ["diagnostics", "macros"]
diagnostics = ["serde", "serde_json"]
# Re-export the step, scenario, and derive macros through `rstest_bdd::prelude`.
# Disable to depend on the runtime types alone without building the proc-macro
# crate; see ADR-022. Note that `default-features = false` also drops
# `diagnostics`, so an opting-out consumer must re-add it explicitly.
macros = ["dep:rstest-bdd-macros"]
mutable_world_macro = []
test-support = []
```

Leave the existing dev-dependency on `rstest-bdd-macros` in place: it enables
`compile-time-validation`, which the normal dependency deliberately does not.

Then change the `rstest-bdd` entry in both `crates/cargo-bdd/Cargo.toml` and
`crates/rstest-bdd-harness-gpui/Cargo.toml` to
`{ workspace = true, default-features = false, features = ["diagnostics"] }`,
per Decision D3. `cargo-bdd` is a shipped CLI binary that cannot invoke
`#[given]` at all, and the GPUI adapter wants runtime types only; without these
edits the `macros` feature is an escape hatch with no user, which the panel
identified as its weakest point. `default-features = false` also drops
`diagnostics`, which is why both must name it.

No prelude content yet. This milestone exists so that if feature unification or
the build graph misbehaves, it does so in isolation.

Validation: `make check-fmt && make lint && make test` green;
`pytest scripts/tests/test_rstest_bdd_macros_feature.py` now passes;
`cargo check -p rstest-bdd --no-default-features` succeeds; and
`cargo tree -p rstest-bdd --no-default-features -i rstest-bdd-macros` reports
the package is absent from the graph.

Commit.

### EP-M3 — green: the prelude module

Create `crates/rstest-bdd/src/prelude.rs` and declare it from
`crates/rstest-bdd/src/lib.rs` with `pub mod prelude;`, placed with the other
public module declarations around lines 31-43.

The module opens with a `//!` comment that states what the prelude is for, what
it deliberately excludes and why, and how to opt out of the macro re-exports.

It must also state the **rule of membership**, because without one the prelude
and the crate root will drift and nobody will know which is authoritative.
Twelve of the twenty-one names are already at the crate root
(`crates/rstest-bdd/src/lib.rs:48-55, 111-112`), so the only things
`prelude::*` gives that `rstest_bdd::*` does not are the macro re-exports and
immunity from `clippy::wildcard_imports`. The rule to write down and give
ADR-022 to own: *the prelude is the curated subset of the crate root that a
step author needs, plus the macro crate's attributes and derives. Harness
authors, framework internals, and specialist surface stay at the root.*
Explicitly say that `rstest`'s own `#[fixture]` and `#[rstest]` are *not*
re-exported, that users should write `#[rstest::fixture]` or import them from
`rstest` directly, and that this is so the consumer's `rstest` version is the
only one in play. A reader who wonders "why isn't `fixture` here?" must find
the answer in the module they are already looking at.

The export list, grouped and commented by purpose:

`.rustfmt.toml` sets `imports_granularity = "Crate"` and
`imports_layout = "HorizontalVertical"`, so rustfmt merges every
`pub use crate::{…}` statement into one and discards comments interleaved
between them. Write the block the way rustfmt will leave it, and put the
per-group explanation in the module doc rather than in inline comments that the
formatter will delete:

```rust
pub use crate::{
    DataTableRow, FixtureBorrowError, InsertOutcome, ScenarioState, Slot, StepArgs, StepContext,
    StepError, StepResult, assert_step_err, assert_step_ok, skip,
};
pub use crate::datatable::{DataTableError, Rows};
#[cfg(feature = "macros")]
pub use rstest_bdd_macros::{
    DataTable, DataTableRow, ScenarioState, StepArgs, given, scenario, scenarios, then, when,
};
```

Three corrections to the draft list, each found by the review panel:

- **Trait/derive symmetry.** `StepArgs`
  (`crates/rstest-bdd/src/step_args.rs:60`)
  and `DataTableRow` (`crates/rstest-bdd/src/datatable/rows.rs:10`) are *both*
  traits and derive-macro names, exactly like `ScenarioState`. The draft
  exported the derive for both and the trait for neither, so
  `#[derive(DataTableRow)]` followed by a trait-method call would have needed a
  second import — the precise failure this module exists to prevent. Export
  both halves of all three.
- **The assertion macros.** `assert_step_ok!`, `assert_step_err!`, and `skip!`
  (`crates/rstest-bdd/src/macros.rs`) are the crate's most-typed names;
  `docs/users-guide.md:2725` documents
  `use rstest_bdd::{assert_step_err, assert_step_ok};` as the house idiom for
  fallible steps. Because they are `#[macro_export]`ed they sit at the crate
  root and a glob of `prelude` does *not* pick them up transitively, so they
  must be re-exported explicitly. A prelude carrying `StepResult` but not
  `assert_step_ok!` sends the user straight back to a second `use` line.
- **`InsertOutcome`** is the return type of
  `StepContext::insert_owned_harness_context`, so a prelude-only user who
  inserts a harness context cannot otherwise name what they get back.

Excluded, and the module doc must say why: `Rows` and `DataTableError` are
included because every data-table step signature names them
(`docs/users-guide.md:2606, 2652, 2682`), but the registry lookup functions,
the localization API, `StepPattern`, `StepExecution`/`StepFuture`, the
`inventory` and `FluentLanguageLoader` re-exports, and the whole
`rstest-bdd-harness` surface are harness-author or framework-internal surface,
not step-author surface.

Add `#[doc(no_inline)]` to the `rstest_bdd_macros` group. Cross-crate `pub use`
is *inlined* by rustdoc by default, which would duplicate every macro's full
documentation onto the prelude page and present a second canonical-looking path
for `given`; same-crate re-exports render as a link list. `no_inline` makes
both groups render as one uniform, scannable list of links. Record the choice
in ADR-022.

Note the deliberate collision: `ScenarioState` names both the trait
(`crate::state::ScenarioState`) and the derive macro
(`rstest_bdd_macros::ScenarioState`). Rust keeps traits and derive macros in
separate namespaces, so a single `use rstest_bdd::prelude::*;` brings both into
scope and `#[derive(ScenarioState)]` alongside `impl ScenarioState for …` both
work. This was verified empirically during planning against the real crates, and
`serde`'s `pub use serde_derive::Serialize` beside `pub trait Serialize` is
the same shape, so state it as fact rather than hedging. The three affected
pairs are `ScenarioState`, `StepArgs`, and `DataTableRow`. Document each in the
users' guide table with its kind populated: the failure mode when someone later
adds a same-named *type* is a baffling diagnostic, and the kind column is what
makes that reviewable.

Do not export: the `__rstest_bdd_`-prefixed internals,
`inventory::{iter, submit}`, `FluentLanguageLoader`, the registry lookup
functions, the localization API, or `StepPattern`. These are framework-internal
or specialist; a prelude that carried them would be a mirror of the crate root
and would forfeit the bounded-hazard argument in `Risks`.

Add a module-level doctest showing the canonical import pair. Follow
`docs/rust-doctest-dry-guide.md`: no `.unwrap()`, hidden `#` lines only for
noise, and keep the import lines visible since they are the point.

Validation: `cargo nextest run -p rstest-bdd prelude_exports` passes;
`cargo test --doc -p rstest-bdd` passes;
`make check-fmt && make lint && make test` green. Record the INV-2 non-vacuity
transcript (temporarily aliasing one entry to the wrong type, observing the
compile failure, reverting).

Commit.

### EP-M4 — the dependency-restricted compile fixture

Create `crates/rstest-bdd/tests/fixtures/prelude_only/` as a crate outside the
root workspace.

Do **not** model it on `tests/fixtures/published-gpui-e2e/`, as the draft said.
That is the *staged* pattern: it patches crates.io names onto `target/`
artefacts, so it needs a staging target, a `STAGED_FIXTURES` entry in
`scripts/fixture_lockfile_discovery.py`, its own
`check-published-gpui-e2e-lock` target, explicit `check-fmt` lines, and a CI
discard entry. None of that applies to a fixture using plain `path =`
dependencies. The correct template is
`crates/rstest-bdd/tests/fixtures/rebuild_invalidation/`, whose lockfile is
seeded from `crates/cargo-bdd/tests/fixtures/minimal/Cargo.lock` by
`make update-feature-rebuild-fixtures-lock` (`Makefile:268-273`).

There is likewise nothing to "register" with
`scripts/check_fixture_lockfiles.py`: `discover_fixture_manifests`
auto-discovers every workspace opt-out manifest that has local path
dependencies and a sibling `Cargo.lock` (`Makefile:275-280`). Four wiring
points the draft missed *do* need doing:

- add the fixture directory to the `cargo` `directories:` list in
  `.github/dependabot.yml` (lines 22-29), or its lockfile rots silently — and
  the failure mode is an *absence* of noise, surfacing much later as an
  unexplained `--locked` break;
- `cargo fmt --all` does not reach out-of-workspace fixtures, so add a
  `--manifest-path` line to both `fmt` and `check-fmt` (`Makefile:193-206`),
  alongside the two existing GPUI entries;
- `make prefetch-fixture-deps` will now warm an extra `rstest` plus `tokio`
  closure on every mutation-lane run;
- set `CARGO_TARGET_DIR` to the workspace `target/` when building it. The GPUI
  fixtures need their own target tree because they pin a nightly toolchain and
  consume staged packages; this fixture has neither constraint, so sharing the
  target directory reuses roughly 140 already-built dependency artefacts.

Cost, measured: the union graph is 144 packages, identical to
`examples/tokio-reminders`, so with a shared target directory the marginal
build is the fixture crate itself.

Its manifest declares exactly:

```toml
[dev-dependencies]
rstest-bdd = { path = "../../../crates/rstest-bdd" }
rstest = "0.26.1"
rstest-bdd-harness-tokio = { path = "../../../crates/rstest-bdd-harness-tokio" }
tokio = { version = "1", features = ["rt", "macros"] }
```

`rstest` is present because generated code emits `#[rstest::rstest]`; `tokio`
is present because the Tokio harness needs a runtime. Neither is imported by a
`use` statement in the fixture's source — that is the point.

Its single test file imports exactly:

```rust
use rstest_bdd::prelude::*;
use rstest_bdd_harness_tokio::TokioTestContext;
```

and exercises **every group in the export list**, so that no name ships without
a sufficiency proof. At minimum: a `#[rstest::fixture]`; a `#[given]`, a
`#[when]`, and a `#[then]`; a step returning `StepResult<()>` that succeeds and
one asserted with `assert_step_ok!`/`assert_step_err!`; a `skip!`; a type
implementing the `ScenarioState` trait via `#[derive(ScenarioState)]` with a
`Slot<T>` field; a data-table step using `#[derive(DataTableRow)]` with `Rows`
and `DataTableError` in its signature; a `#[derive(StepArgs)]` struct argument;
a `scenarios!` invocation; a step taking
`#[harness_context] ctx: &TokioTestContext` and one calling
`insert_owned_harness_context` so `InsertOutcome` is named; and a
`#[scenario(path = …, harness = rstest_bdd_harness_tokio::TokioHarness)]`
binding that runs and asserts. The scenario must assert something real, so that
a build binding zero steps fails rather than passes.

The draft's checklist covered six of the twenty-one names, which would have let
the fixture go green while the prelude was incoherent — precisely the
trait-without-derive gap the panel found in the draft export list. Treat the
checklist and the export list as one artefact: adding a name to either without
the other is the failure this milestone exists to prevent.

Add one further fixture for Doggylump's collision scenario: a compile test in
which the consumer defines its own `StepResult` beside
`use rstest_bdd::prelude::*;`, asserting that the resulting diagnostic is
comprehensible. Nothing else in the plan exercises glob-import collision with a
crate that has its own names, and a path-dependency fixture unifies everything
so it can never reproduce it incidentally.

Add a Makefile target that builds and runs it, and invoke that target from
`test`. Follow the naming and structure of the existing `check-published-gpui` /
`e2e-published-gpui` targets so the Makefile stays legible, and validate with
`mbake validate Makefile`.

Record the INV-1 non-vacuity transcript: delete `Slot` from the prelude export
list, run the fixture, observe `cannot find type Slot in this scope`, restore.

Validation: the fixture target passes; `make test` green; the negative-control
transcript is in `Artefacts and notes`.

Commit.

### EP-M5 — migrate the examples

For each of `examples/todo-cli/tests/todo.rs`,
`examples/japanese-ledger/tests/ledger.rs`,
`examples/gpui-counter/tests/counter.rs`, and
`examples/tokio-reminders/tests/reminders.rs`:

- replace the `rstest::fixture` and `rstest_bdd_macros::{…}` imports with a
  single `use rstest_bdd::prelude::*;`,
- change each `#[fixture]` to `#[rstest::fixture]`,
- keep the example's own-crate import and, in `reminders.rs`, the
  `rstest_bdd_harness_tokio::TokioTestContext` import,
- leave `examples/todo-cli/tests/cli.rs` alone (Decision D6),
- leave the nested `#[cfg(test)]` unit-test modules inside `examples/*/src/`
  alone: those are plain `rstest` unit tests with no BDD steps, so they are out
  of scope for INV-3 and should keep importing `rstest` directly.

`rstest` and `rstest-bdd-test-macros` must stay in the example manifests:
`rstest` because generated code emits `#[rstest::rstest]` and the `src/` unit
tests use it, and `rstest-bdd-test-macros` for
`#[rstest_bdd_test_macros::allow_fixture_expansion_lints]`.

`rstest-bdd-macros` is the open case. The draft asserted it must stay because
"`--all-features` builds of the workspace resolve macro paths through it in
other contexts". **That claim is unsubstantiated and was withdrawn under
review** — `crates/rstest-bdd-macros/src/codegen/mod.rs` resolves the *runtime*
crate through `proc_macro_crate`, not the macro crate. Determine it empirically
per crate: remove the entry, run `cargo check -p <example> --all-targets`, and
keep the removal if green. All four examples already declare `rstest-bdd`, so
no manifest addition is needed either way. Record the outcome in
`Surprises & discoveries`; if the entry *is* removable, say so in the users'
guide, because leaving it in place undercuts the "one import" story at the
manifest level.

Validation: `make check-fmt && make lint && make test` green; the four example
test suites still pass with the same test names.

Commit.

### EP-M6 — the import-allowlist contract test

Make `scripts/tests/test_example_prelude_imports.py` pass. The checker should:

- discover every `.rs` file under `examples/*/tests/` and `examples/*/src/`,
- keep only files containing at least one of `#[scenario`, `#[given`,
  `#[when`, `#[then`,
- parse each retained file's top-level `use` statements — a line-oriented
  parser is sufficient and more legible here than pulling in a Rust parser;
  handle brace groups and leading `::`,
- reduce each to its crate root,
- assert the set is a subset of the allowed roots for that example, where the
  harness adapter is read from that example's `Cargo.toml` rather than
  hard-coded,
- assert it examined at least one `use` statement in at least four files.

Follow `docs/scripting-standards.md` and the `.rules/python-*` guides. Keep the
module under 400 lines; if the checker and its tests together push past that,
extract the checker into `scripts/example_prelude_imports.py` and leave the
test in `scripts/tests/`, mirroring the `scripts/users_guide_links.py` /
`scripts/check_users_guide_links.py` split.

Validation: `pytest scripts/tests/test_example_prelude_imports.py` passes
including its negative control; `make lint` (which runs `ruff` and `pylint`)
green; `make test` green.

Commit.

### EP-M7 — documentation and the parity gate

Four documents change, plus one new ADR and one new gate.

`docs/users-guide.md`: add a "Prelude" subsection under "Harness adapter core
APIs" (around line 2326), carrying a table with one row per exported item
giving the name, its **kind**, its source path, and a one-line purpose — this
is the artefact the finish line calls "docs list the exported items", and the
kind column is load-bearing for INV-4. Link the rustdoc prelude page alongside
it, since the generated list cannot go stale. State the canonical two-line
import, state that `rstest`'s `#[fixture]` is deliberately absent and how to
write it instead (or, under D2-prime, that `use rstest::fixture;` remains the
recommended form), and state that `#[harness_context]` needs no import. State
the goal as "one `use` line for the framework" rather than overselling it, per
the INV-3 honesty caveat.

**Scope the panel added: migrate the guide's own snippets.** The guide contains
roughly twenty code blocks teaching `use rstest_bdd_macros::{given, …}` — lines
126, 266, 338, 435, 467, 517, 568, 626, 691, 1085, 1150, 1578, 1614 and others.
Adding one prelude table while leaving every snippet on the old idiom means a
newcomer meets the old form nineteen times and the new form once, and the
documentation contradicts itself. Convert the snippets, starting with the
`use rstest_bdd::{ScenarioState, Slot};` example at line 567, which is where a
reader actually meets the imports. Some are `<!-- tested-example -->` blocks
executed by `crates/rstest-bdd/tests/documentation_examples/`; run `make test`
after converting rather than assuming they are inert prose.

Add a cross-reference from "Scenario state slots" (line 546) and from the
`#[harness_context]` narrative (lines 1076-1113) so a reader arriving at either
finds the prelude.

`docs/developers-guide.md`: add a "Public prelude module" section modelled on
the existing "Shared policy crate (rstest-bdd-policy)" section (lines
1802-1862). Record what the prelude may and may not contain, that additions are
a reviewed decision because of the glob-import hazard, where the `pub use`
block lives, which tests and gates guard it, and that ADR-022 owns the policy.

`docs/rstest-bdd-design.md` §2.7.6.4: move "a prelude for common integration
imports" out of the not-yet-shipped list into a delivered paragraph, naming the
module path, the `macros` feature, the D2 exclusion of `rstest`, and ADR-022.
Correct the "marker attributes" expectation per Decision D4.

`docs/adr-007-harness-context-injection.md`: extend the 11.2.1 addendum with
one paragraph recording that the marker is inert and therefore needs no import
and cannot be re-exported, and that this is proved by the prelude-only compile
fixture.

Create `docs/adr-022-public-prelude-export-policy.md` in Y-Statement form, per
the `arch-decision-records` skill and `docs/documentation-style-guide.md`. It
must record:

- the glob-import semver hazard and why a curated list bounds it;
- the rule that **no name is added to the prelude within a semver-compatible
  line**; additions land at the next breaking release. The panel preferred this
  to the draft's "additions require an ADR amendment", because the draft's rule
  permits additions precisely when the hazard bites and leaves the judgement to
  a reviewer. The stronger rule is mechanical — and, together with the fact
  that removal is *also* not semver-legal, it is why the initial export list
  must be right rather than topped up later;
- the `rstest` exclusion, argued on version control of the consumer's
  dependency rather than on the weaker two-copies story, plus the implicit
  `"rstest::rstest"` string contract in `DefaultAttributePolicy` and the
  supported `rstest` range;
- **the lockstep rule for `rstest-bdd` and `rstest-bdd-macros`.** The new edge
  carries `version = "0.6.0"`, i.e. `^0.6`, over a large hidden protocol —
  roughly thirty `__rstest_bdd_`-prefixed symbols — with no version or protocol
  guard in either crate. Today the consumer picks both versions and the
  lockfile keeps them aligned; afterwards `rstest-bdd` picks the macro version,
  making `rstest-bdd 0.6.1` with `rstest-bdd-macros 0.6.9` resolvable. This is
  the same two-copies hazard D2 rejects for `rstest`, reintroduced for the
  macro crate, and the draft did not analyse it. `docs/releasing-crates.md`
  already requires the two to move in lockstep; record it here and use an `=`
  pin on the prelude edge unless a reason not to is written down;
- the `macros` feature, the two mandated opt-outs, and the measured +30-package
  transitive cost;
- `#[doc(no_inline)]` on the macro group and why;
- **as an explicit non-goal:** a layered prelude in each harness adapter crate
  (`rstest_bdd_harness_tokio::prelude` re-exporting `rstest_bdd::prelude::*`
  plus the adapter's own types), which would give an adapter user exactly one
  framework import and would satisfy §2.7.6.3 directly. Both adapters already
  re-export the base harness surface verbatim, so the pattern is established
  here. It is out of scope because `rstest-bdd-harness-tokio` only
  *dev*-depends on `rstest-bdd` and would need a new normal edge, which
  `Tolerances` forbids, and because `todo-cli` and `japanese-ledger` use the
  default harness and would still need the base prelude. Record the rationale
  so the next reviewer does not re-open it;
- **the departure from §2.7.6.3** recorded in EP-M2's conformance check.

Cite the Cargo SemVer reference on glob imports, the tokio prelude removal
(tokio-rs/tokio#3257), and corrode.dev's "Don't Use Preludes And Globs" as the
prior art that informed the curation policy, together with the testing-niche
split recorded in Decision D0. Add it to `docs/contents.md`.

Create `scripts/check_prelude_exports.py` implementing INV-4, modelled on
`scripts/check_gpui_mapping_table.py`, with the same quality of error message.
Wire it into the `lint` recipe in `Makefile` after
`check_serial_nextest_matrix.py`. Validate the Makefile with
`mbake validate Makefile`.

Update `docs/roadmap.md` only in EP-M8.

Validation: `make fmt` then `make markdownlint` (in that order — `make fmt` can
itself introduce MD039 or MD013 findings), `make nixie`,
`make check-fmt && make lint && make test` all green.

Commit.

### EP-M8 — roadmap tick, full gates, pull request

Mark `docs/roadmap.md` item 11.2.2 as `[x]` with a completion note in the style
of 11.2.1's: the date, the delivered module path, the `macros` feature, the D2
and D4 decisions, the three enforcement artefacts, and a pointer to this
ExecPlan and to ADR-022.

Run the full gate set sequentially, capturing each to a log under `/tmp`.
Delegate this to the `scrutineer` subagent rather than running it inline.

Open the pull request. Update `Progress` and `Outcomes & retrospective`, and
set the plan's status to `COMPLETE`.

## Milestones and plateaus

- **EP-M0.** Outcome: verified green baseline and a settled wildcard-import
  assumption. Requirements: none discharged; de-risks all later work.
  Acceptance evidence: three green gate transcripts plus the probe transcript.
  Conformance check: no interface, dependency, or format change. Recovery:
  nothing to revert. Remaining gaps: all. Compatibility decision: none.

- **EP-M1.** Outcome: four failing specifications, each failing for its stated
  reason. Requirements: specifies REQ-11.2.2 and both finish-line halves.
  Acceptance evidence: the four recorded red transcripts. Conformance check: no
  production code touched; no public interface changed. Recovery: delete the
  four new test files. Remaining gaps: everything green. Compatibility
  decision: none.

- **EP-M2.** Outcome: `rstest-bdd` builds with and without the new `macros`
  feature; the dependency graph is as intended. Requirements: discharges
  LEMMA-1. Acceptance evidence: `test_rstest_bdd_macros_feature.py` green, the
  `cargo tree` transcript, three green gates. Conformance check: one new
  first-party dependency, authorized by `Tolerances`; no public item added or
  changed. **Conformance deviation to be accepted at approval:** the draft
  claimed §2.7.6.3 "still holds because the new edge runs the other way". That
  was a non-sequitur. §2.7.6.3 is about what appears in the *user's* manifest —
  "lets first-party adapter users depend only on the selected adapter crate" —
  not about internal edge direction. A prelude in `rstest-bdd` makes it a crate
  every user must name directly, and EP-M4's own fixture manifest lists four.
  This item therefore moves the workspace *away* from §2.7.6.3's goal. Record
  that plainly in ADR-022 rather than claiming compliance, and see the layered
  adapter-prelude option below, which is the design that would satisfy both. On
  ADR-004: the new edge is **orthogonal, not a violation** — ADR-004 forbids
  macros-to-runtime (`docs/adr-004-policy-crate.md:36`); this is the opposite
  arrow and the graph stays acyclic. The direction still states in one
  sentence: *the runtime crate is the user-facing facade over the macro crate,
  which sits over the shared leaf crates.* Recovery: revert the manifest hunk.
  Remaining gaps: no prelude exists. Compatibility decision: none — the feature
  is new, so nothing depends on its absence.

- **EP-M3.** Outcome: `rstest_bdd::prelude` exists, is documented, and is
  proved to name the same items as the original paths. Requirements: discharges
  INV-2, advances REQ-11.2.2. Acceptance evidence: `prelude_exports.rs` green
  plus the aliasing negative control. Conformance check: public surface grew
  additively only; every prior path still resolves; design §2.7.6.4 not yet
  updated (EP-M7). Recovery: delete `prelude.rs` and its `pub mod` line.
  Remaining gaps: nothing yet proves sufficiency or example adoption.
  Compatibility decision: none.

- **EP-M4.** Outcome: an out-of-workspace crate compiles and runs a full
  scenario from prelude imports alone. Requirements: discharges INV-1 and the
  ADR-007-addendum trace. Acceptance evidence: the fixture target passes plus
  the `Slot`-deletion negative control. Conformance check: new fixture is
  outside the root workspace and lockfile-managed, matching the established
  pattern; no public interface change. Recovery: delete the fixture directory
  and the Makefile target. Remaining gaps: the real examples are unchanged.
  Compatibility decision: none.

- **EP-M5.** Outcome: all four examples import the prelude. Requirements:
  advances REQ-11.2.2-FL-a. Acceptance evidence: green gates and unchanged test
  names. Conformance check: examples are still ordinary workspace members; no
  manifest entry removed; `cli.rs` untouched per D6. Recovery: `git revert` the
  commit; the prelude keeps working either way. Remaining gaps: nothing
  prevents regression. Compatibility decision: none — examples have no external
  consumers.

- **EP-M6.** Outcome: regression to multi-crate imports is mechanically
  blocked. Requirements: discharges INV-3 and REQ-11.2.2-FL-a. Acceptance
  evidence: the contract test green including its negative control. Conformance
  check: new test only; no interface change. Recovery: delete the test file.
  Remaining gaps: documentation. Compatibility decision: none.

- **EP-M7.** Outcome: the exported items are documented, the policy is recorded
  in an ADR, and drift between the two is gated. Requirements: discharges INV-4
  and REQ-11.2.2-FL-b. Acceptance evidence: `make lint` runs the parity gate
  and passes; the gate's own unit tests pass; Markdown gates green. Conformance
  check: §2.7.6.4 now matches the implementation; ADR-007 addendum reconciled
  with D4; no undocumented public item remains. Recovery: revert the
  documentation commit and remove the Makefile line. Remaining gaps: the
  roadmap still shows the item open. Compatibility decision: none.

- **EP-M8.** Outcome: roadmap ticked, full gates green, pull request open.
  Requirements: closes REQ-11.2.2. Acceptance evidence: the `scrutineer` gate
  report. Conformance check: every trace link in `Conformance basis` resolves
  to a passing artefact; no upstream deviation left unrecorded. Recovery: the
  branch is not merged until review passes. Remaining gaps: none. Compatibility
  decision: none.

## Concrete steps

Run everything from the repository root,
`/home/leynos/.lody/repos/github---leynos---rstest-bdd/worktrees/1f3a9fe1-1108-4dde-8668-77a0461f188a`.
Prefer Makefile targets over raw `cargo` invocations. Never run gates in
parallel — this environment relies on build caching, and sequential execution
is what benefits from it.

Capture long output for later review:

```bash
make lint 2>&1 | tee "/tmp/lint-rstest-bdd-$(git branch --show-current).out"
```

EP-M0:

```bash
git branch --show-current            # 11-2-2-prelude-exposes-step-result-and-context-helpers
make check-fmt && make lint && make test
```

EP-M1 red checks:

```bash
cargo nextest run -p rstest-bdd prelude_exports   # expect: unresolved import
uv run pytest scripts/tests/test_example_prelude_imports.py
uv run pytest scripts/tests/test_check_prelude_exports.py
uv run pytest scripts/tests/test_rstest_bdd_macros_feature.py
```

EP-M2 feature checks:

```bash
cargo check -p rstest-bdd --no-default-features
cargo tree -p rstest-bdd --no-default-features -e normal -i rstest-bdd-macros
```

Expected transcript for the second command:

```plaintext
error: package ID specification `rstest-bdd-macros` did not match any packages
```

That error is the success condition: the package is absent from the graph.

EP-M3 focused check:

```bash
cargo nextest run -p rstest-bdd prelude_exports
cargo test --doc -p rstest-bdd
```

EP-M4:

```bash
make e2e-prelude-only          # exact target name chosen in EP-M4
```

EP-M6 and EP-M7:

```bash
uv run pytest scripts/tests
uv run python scripts/check_prelude_exports.py
```

Before each commit:

```bash
make check-fmt && make lint && make test
```

After any Markdown edit:

```bash
make fmt && make markdownlint && make nixie
```

Commit with a file-based message, per the `commit-message` skill; do not pass
the message with `-m`.

## Validation and acceptance

### Red-green-refactor evidence

- **Red.** EP-M1 leaves four specifications failing, each with a recorded
  failure message matching its stated expectation. No production code exists
  for any of them.
- **Green.** EP-M2 turns the feature manifest test green; EP-M3 turns
  `prelude_exports.rs` green; EP-M5 turns the import-allowlist test green;
  EP-M7 turns the parity gate green. Each is the smallest change that does so.
- **Refactor.** EP-M6 and EP-M7 extract checker logic into its own module if
  the 400-line budget requires it, rerunning the focused test and then the full
  gates.

No expected-failure markers are left in the tree. Where the red state is a
compile error rather than an assertion failure — which is the case for
`prelude_exports.rs` and for the compile fixture — that is the strictest
available form of red, and the failure text is recorded verbatim rather than
paraphrased.

### Behavioural specification

There is no new Gherkin feature file. The user-visible behaviour here is
compile-time name resolution, which a runtime `.feature` scenario cannot
observe — a scenario can only run once the imports already resolve. The
behavioural artefact is instead the prelude-only compile fixture of EP-M4,
which *is* a scenario: it binds real steps from a real `.feature` file and
asserts a real outcome, under the restricted dependency set that is the
property under test. Its feature file is the specification:

```gherkin
Feature: Importing the framework through the prelude

  Scenario: A prelude-only crate runs a scenario with harness context
    Given a counter starting at 3
    When I increment the counter by 4
    And I record the harness context
    Then the counter reads 7
    And the harness context was available
```

### Acceptance criteria

The work is done when all of the following hold.

1. `use rstest_bdd::prelude::*;` brings all 21 names into scope: `StepResult`,
   `StepError`, `Slot`, `InsertOutcome`, `StepContext`, `FixtureBorrowError`,
   `Rows`, `DataTableError`, the `ScenarioState`, `StepArgs`, and
   `DataTableRow` traits *and* their same-named derives, the `DataTable`
   derive, the `given`, `when`, `then`, and `scenario` attributes, the
   `scenarios!` macro, and the `assert_step_ok!`, `assert_step_err!`, and
   `skip!` macros. The `insta` snapshot is the authoritative list; this
   sentence must be kept in step with it.
2. Every one of those names still resolves at its pre-existing path, to the
   same item.
3. `crates/rstest-bdd/tests/fixtures/prelude_only/` compiles and its scenario
   passes, with a
   manifest naming only `rstest-bdd`, `rstest`, `rstest-bdd-harness-tokio`, and
   `tokio`, and source importing only `rstest_bdd::prelude::*` and
   `rstest_bdd_harness_tokio::TokioTestContext`.
4. Every BDD example file under `examples/` imports only the prelude, its own
   crate, and where needed its harness adapter; the contract test enforces it
   and fails on a synthetic violation.
5. `docs/users-guide.md` carries a table of every exported item, and
   `scripts/check_prelude_exports.py` — running inside `make lint` — fails if
   that table and `prelude.rs` disagree in either direction.
6. `cargo check -p rstest-bdd --no-default-features` succeeds and
   `rstest-bdd-macros` is absent from that graph.
7. `docs/rstest-bdd-design.md` §2.7.6.4, `docs/developers-guide.md`, the
   ADR-007 addendum, and the new ADR-022 all describe what was actually built.
8. `docs/roadmap.md` item 11.2.2 is `[x]` with a completion note.

### Quality criteria

- Tests: `make test` green, including the doctests, the new fixture target, and
  `pytest scripts/tests`.
- Verification: INV-1 through INV-4 and LEMMA-1 discharged, each with its
  non-vacuity evidence recorded in `Artefacts and notes`.
- Lint and typecheck: `make check-fmt`, `make lint` (Clippy, rustdoc, Whitaker,
  `ruff`, `pylint`, and all six gate scripts), and `make typecheck` green.
- Markdown: `make markdownlint` and `make nixie` green.
- Makefile: `mbake validate Makefile` green.
- Performance: not applicable; no runtime code path changes.
- Security: no new *declared* third-party dependency and no `unsafe`. Be
  precise about this in ADR-022: the new first-party edge pulls 30 further
  packages into `rstest-bdd`'s normal graph transitively, including the
  `cap-std`/`rustix` capability-filesystem closure. The claim "no new
  dependency" is true of `Cargo.toml` and false of the build; record the latter.

### Quality method

`scrutineer` runs the gates sequentially, tees each to a log under `/tmp`, and
returns a bounded report. On failure, read the cited log rather than re-running
the gate; re-run only after applying a fix.

## Idempotence and recovery

Every step is re-runnable. The Rust edits are ordinary source changes; the
Python gates are pure functions of the tree; the Makefile additions are
declarative.

The only step needing care is EP-M4's fixture lockfile. Follow the
`crates/rstest-bdd/tests/fixtures/rebuild_invalidation` pattern, and if
`make check-fixture-lockfiles` reports drift, regenerate with
`make update-fixture-lockfiles` rather than editing by hand.

Each milestone is a separate commit, so recovery from any point is `git revert`
of that commit. Because the prelude is purely additive and the examples are
migrated in a single commit, no intermediate state requires a compatibility
shim to remain coherent.

Clean up after EP-M0: the probe module and probe import must not survive into
any commit. Verify with `git status` before committing EP-M1.

## Artefacts and notes

Populate during implementation. The following are required, not optional:

- The EP-M0 wildcard-import probe transcript, showing `make lint` green with a
  glob import of a `prelude` module in place.
- The four EP-M1 red transcripts, verbatim.
- The EP-M2 `cargo tree --no-default-features` transcript.
- The INV-2 negative control: the diff that aliased one prelude entry to the
  wrong type, and the resulting compile error.
- The INV-1 negative control: the diff that removed `Slot` from the export
  list, and the resulting `cannot find type Slot in this scope` from the
  prelude-only fixture.
- The INV-3 negative control: the synthetic-tree test output showing the
  checker rejecting a non-compliant file.
- The INV-4 negative controls: gate output for a missing row and for a surplus
  row.
- The final `scrutineer` gate report.

## Interfaces and dependencies

At the end of EP-M3, `crates/rstest-bdd/src/prelude.rs` must exist and must
export exactly the following, and `crates/rstest-bdd/src/lib.rs` must declare
`pub mod prelude;`:

```rust
//! Common imports for writing `rstest-bdd` acceptance tests.
//!
//! Glob-import this module to get the step and scenario macros together with
//! the runtime types they need:
//!
//! ```
//! use rstest_bdd::prelude::*;
//! ```
//!
//! `rstest`'s own `#[fixture]` and `#[rstest]` are deliberately absent; write
//! `#[rstest::fixture]`, or import them from `rstest` directly. See ADR-022.

pub use crate::{
    DataTableRow, FixtureBorrowError, InsertOutcome, ScenarioState, Slot, StepArgs, StepContext,
    StepError, StepResult, assert_step_err, assert_step_ok, skip,
};
pub use crate::datatable::{DataTableError, Rows};
#[cfg(feature = "macros")]
#[doc(no_inline)]
pub use rstest_bdd_macros::{
    DataTable, DataTableRow, ScenarioState, StepArgs, given, scenario, scenarios, then, when,
};
```

That is **21 distinct names** across 23 `pub use` entries (`ScenarioState`,
`StepArgs`, and `DataTableRow` each appear twice, once per namespace). Fix this
count here and let the export-identity test own it; the draft quoted "thirteen"
in the `Verification plan`, which was the row count of the survey table, not
the export list.

Amended manifest surface on `crates/rstest-bdd/Cargo.toml` — note this **edits
the existing `[features]` table at lines 63-71**, it does not create one:

```toml
[features]
default = ["diagnostics", "macros"]
macros = ["dep:rstest-bdd-macros"]
```

`crates/cargo-bdd/Cargo.toml` and `crates/rstest-bdd-harness-gpui/Cargo.toml`
both change their `rstest-bdd` entry to
`{ workspace = true, default-features = false, features = ["diagnostics"] }`.

New repository artefacts:

- `crates/rstest-bdd/src/prelude.rs`
- `crates/rstest-bdd/tests/prelude_exports.rs`
- `crates/rstest-bdd/tests/fixtures/prelude_only/` (crate, feature file,
  lockfile)
- `scripts/check_prelude_exports.py`
- `scripts/tests/test_check_prelude_exports.py`
- `scripts/tests/test_example_prelude_imports.py`
- `scripts/tests/test_rstest_bdd_macros_feature.py`
- `docs/adr-022-public-prelude-export-policy.md`

Modified: `crates/rstest-bdd/Cargo.toml`, `crates/rstest-bdd/src/lib.rs`,
`crates/cargo-bdd/Cargo.toml`, `crates/rstest-bdd-harness-gpui/Cargo.toml`,
`.github/dependabot.yml`, `Makefile`, the four example test files,
`docs/users-guide.md`, `docs/developers-guide.md`, `docs/rstest-bdd-design.md`,
`docs/adr-007-harness-context-injection.md`, `docs/contents.md`,
`docs/roadmap.md`.

No new third-party dependency is *declared*. The single new first-party edge is
`rstest-bdd` -> `rstest-bdd-macros`, optional and default-on, which pulls 30
further packages into `rstest-bdd`'s normal graph transitively and rebuilds
`syn` with `extra-traits`. See Decision D3 for why that cost is zero for step
authors and how it is removed for the two runtime-only consumers.

## Revision notes

- 2026-09-14: initial draft. Records the four scoping decisions the roadmap
  text leaves open — where the prelude lives (D1), the deliberate exclusion of
  `rstest` (D2), the `macros` feature (D3), the inert marker (D4) — together
  with the enforcement triple (D5) and the `cli.rs` exemption (D6). These are
  marked **Pending approval**; they define the deliverable, so confirming or
  changing one changes what "done" means.

- 2026-09-14: revised after a six-expert design review (structural integrity,
  alternatives, cost, contracts, failure modes, developer experience). What
  changed:

  - **Added Decision D0**, the crate-root re-export alternative the draft never
    considered. It is the headline approval question and would delete much of
    the plan if adopted.
  - **Corrected a factual error that would have broken the build**: the
    `[features]` section already exists, and the draft's instruction would have
    dropped `diagnostics` from `default` with no gate to catch it.
  - **Corrected the export list** from an incoherent 13/18 to 21 names in
    complete trait/derive groups, adding the assertion macros, the data-table
    types, and `InsertOutcome`.
  - **Replaced the fragile parity parser** with an `insta` snapshot as single
    source of truth, after rustfmt's `imports_granularity` was found to defeat
    line-oriented parsing.
  - **Fixed three verification defects**: LEMMA-1's control could never have
    passed (`cargo tree` walks dev edges), INV-2's identity technique proved
    nothing (inferred closure parameter), and INV-4 compared bare names where
    three names are exported twice.
  - **Corrected the fixture template**, its four missing wiring points
    (dependabot, `fmt`/`check-fmt`, mutation-lane prefetch, shared target
    directory), and its coverage, which reached six of twenty-one names.
  - **Withdrew two unsound arguments**: the `serde` precedent for default-on
    (serde's `derive` is opt-in) and the §2.7.6.3 conformance claim (a
    non-sequitur — this item moves *away* from that goal, which is now recorded
    as an accepted deviation).
  - **Added four risks** the draft missed: feature unification changing a
    glob's name set, the irreversibility of a published prelude, the unpinned
    `rstest-bdd`/`rstest-bdd-macros` protocol, and glob-import collision with a
    consumer's own names.
  - **Added D2-prime**, separating D2's sound dependency constraint from its
    unnecessary `#[rstest::fixture]` style tax, and an honesty caveat so a green
    INV-3 is not oversold.

  Effect on remaining work: EP-M2 gains two manifest opt-outs, EP-M4 gains
  substantial fixture coverage and four wiring points, EP-M7 gains the
  migration of roughly twenty users-guide snippets, and ADR-022 gains five
  further clauses. Nothing is descoped, because D0 and D2-prime are the
  descoping options and both await approval.
