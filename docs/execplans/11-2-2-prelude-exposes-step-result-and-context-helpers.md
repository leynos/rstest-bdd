# Expose a public prelude for integration imports (roadmap 11.2.2)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

Roadmap item: 11.2.2 (`docs/roadmap.md` lines 1038-1043).
Origin: the v0.6.1 early-life support programme recorded in
`docs/rstest-bdd-design.md` §2.7.6.4 (lines 2182-2196).

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
`#[then]`, and `#[scenario]` attributes, the `StepResult` alias, `Slot`,
the `ScenarioState` trait and its derive, `StepContext` and its
harness-context accessors — resolves. The underlying crates are not hidden:
they remain named in `Cargo.toml`, remain documented, and remain directly
importable by anyone who prefers explicit paths.

You can see this working three ways. First, every example test file in
`examples/` will contain exactly two framework imports (the prelude and, where
the example names a harness type, its harness adapter crate) and a machine
gate will fail if a fifth crate creeps back in. Second, a fixture crate that
lives outside the workspace and declares only `rstest-bdd`, `rstest`, and one
harness adapter in its manifest will compile a complete scenario using nothing
but prelude imports — it cannot compile if the prelude is missing an item.
Third, `docs/users-guide.md` will carry a table of every exported name, and a
gate wired into `make lint` will fail if that table and the actual `pub use`
list in `crates/rstest-bdd/src/prelude.rs` disagree.

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
  public item carries a `///` doc comment. `[workspace.lints.rust] missing_docs
  = "deny"` and the `cargo doc --workspace --no-deps` step inside `make lint`
  (`Makefile:112`, with `RUSTDOC_FLAGS ?= --cfg docsrs -D warnings`) enforce
  this.
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
  not a `#[proc_macro_attribute]`. There is no item to re-export.
  Severity: medium. Likelihood: certain (already observed).
  Mitigation: Decision D4 resolves this by documenting the marker as
  import-free and proving it with a compile test, rather than inventing a
  no-op attribute macro. The design document and the ADR-007 addendum are
  updated so the next reader is not misled.

- Risk: glob-importing a prelude is a semver hazard. Cargo's SemVer reference
  and the Rust API evolution RFC both note that adding a public item to a
  glob-imported module can break a downstream crate that already defines that
  name. Severity: medium. Likelihood: low for the initial release, rising with
  each later addition.
  Mitigation: the prelude is curated, not a mirror of the crate root; every
  name is framework-specific (`StepResult`, `ScenarioState`, `StepContext`)
  rather than generic (`Error`, `Result`, `Context`); and the docs/exports
  parity gate makes any future addition a visible, reviewed change rather than
  an incidental one. Record this policy in ADR-022 so later additions are
  weighed against it.

- Risk: `clippy::wildcard_imports` is a pedantic lint and the workspace runs
  `pedantic = { level = "warn", priority = -1 }` under `-D warnings`, so a
  glob import could fail `make lint`. Severity: high if true.
  Likelihood: low. Clippy's `warn-on-all-wildcard-imports` option defaults to
  `false`, and with that default the lint deliberately does not fire for
  imports from a module named `prelude`. `clippy.toml` does not set the option.
  Mitigation: EP-M0 proves this empirically with a throwaway glob import before
  any real work depends on it. If the assumption is wrong, that is an
  escalation, not a suppression.

- Risk: `rstest-bdd` gaining a normal dependency on `rstest-bdd-macros` changes
  feature unification. `rstest-bdd` already dev-depends on it with
  `features = ["compile-time-validation"]`
  (`crates/rstest-bdd/Cargo.toml:48`); adding a default-featured normal
  dependency means that within `rstest-bdd`'s own test build both edges unify
  and `compile-time-validation` stays on, as it is today. Downstream consumers
  get default features unless they ask otherwise.
  Severity: low. Likelihood: low.
  Mitigation: EP-M2 runs the full gates immediately after the manifest change,
  before any prelude content exists, so a unification surprise is isolated.

- Risk: adding `rstest-bdd-macros` to `rstest-bdd`'s normal dependencies
  lengthens the build chain for consumers who only want the runtime types —
  notably `rstest-bdd-harness-gpui`, which depends on `rstest-bdd`.
  Severity: low. Likelihood: certain.
  Mitigation: the macro re-exports sit behind a default-on `macros` feature
  (Decision D3), so `default-features = false` restores the old graph.

- Risk: the dependency-restricted compile fixture drifts out of step with the
  workspace, as fixture crates under `tests/fixtures/` have their own
  lockfiles. Severity: medium. Likelihood: medium.
  Mitigation: follow the established `tests/fixtures/published-gpui-e2e`
  pattern and register the fixture with the existing
  `scripts/check_fixture_lockfiles.py` machinery invoked by
  `make check-fixture-lockfiles` (`Makefile:103`).

- Risk: the docs/exports parity gate becomes a nuisance that people work around
  by editing the table without thinking. Severity: low. Likelihood: medium.
  Mitigation: make the failure message name the exact missing or surplus item
  and point at both files, following the error-message quality of
  `scripts/check_gpui_mapping_table.py`.

## Progress

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
  `rstest_bdd_harness_tokio::TokioTestContext`.
  Impact: the finish line's "the prelude plus their harness crate" is close to
  achievable, because the harness type in `#[scenario(harness = ...)]` is
  already written fully qualified (for example
  `examples/gpui-counter/tests/counter.rs:56`). Only `rstest::fixture` and the
  step attributes stand in the way.

- Observation: fully-qualified attribute paths are already house style in the
  examples. Evidence: `examples/gpui-counter/tests/counter.rs:11` writes
  `#[rstest_bdd_test_macros::allow_fixture_expansion_lints]` and line 56 writes
  `harness = rstest_bdd_harness_gpui::GpuiHarness`.
  Impact: migrating `#[fixture]` to `#[rstest::fixture]` (Decision D2) is
  consistent with existing practice rather than a novel imposition.

- Observation: `#[harness_context]` has no exportable item. Evidence: reading
  `crates/rstest-bdd-macros/src/lib.rs` in full shows only `given`, `when`,
  `then`, and `scenario` as `#[proc_macro_attribute]`; the marker is classified
  inside
  `crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_context/mod.rs`.
  The 11.2.1 plan's `Decision log` explicitly deferred "whether the marker
  should additionally be re-exported from a prelude" to this item.
  Impact: drives Decision D4.

- Observation: the repository has no public-API surface tooling. Evidence: a
  repository-wide search for `cargo-public-api`, `cargo-semver-checks`,
  `public-api.txt`, and `public_api` returns nothing.
  Impact: the "docs list the exported items" half of the finish line has to be
  discharged by a hand-written gate; there is no off-the-shelf backstop to
  lean on. This is what motivates the parity gate in EP-M7.

## Decision log

- Decision D1: the prelude lives at `rstest_bdd::prelude`, in a new file
  `crates/rstest-bdd/src/prelude.rs`, and re-exports a curated set of
  `rstest-bdd` runtime items plus the `rstest-bdd-macros` attribute and derive
  macros. It does not re-export whole crates and does not introduce any new
  type, trait, or function of its own.
  Rationale: `rstest_bdd` is the crate users believe they are using, so that is
  where the predictable module belongs. A curated list — rather than
  `pub use crate::*` — is what keeps the glob-import semver hazard bounded and
  what makes a docs parity table meaningful. Putting the prelude in
  `rstest-bdd-macros` is impossible: a `proc-macro` crate may only export
  macros. Creating a separate `rstest-bdd-prelude` crate was rejected because
  it adds a published crate and defeats "one predictable module".
  Date/Author: 2026-09-14, planning agent. **Pending approval.**

- Decision D2: the prelude does not re-export anything from `rstest`. Examples
  migrate `#[fixture]` to the fully-qualified `#[rstest::fixture]`, which needs
  no `use` statement, so their import list reduces to the prelude plus the
  harness crate as the finish line requires.
  Rationale: generated scenario code emits `#[rstest::rstest]` resolved against
  the consumer's own `rstest` dependency
  (`crates/rstest-bdd-harness/src/policy.rs:89`). Re-exporting `fixture` from
  `rstest-bdd` would create a second, independent path to `rstest` whose
  version is chosen by `rstest-bdd` rather than by the consumer. When those two
  requirements diverge across a semver-incompatible `rstest` release, Cargo
  builds two copies and the fixture protocol silently mismatches. Keeping
  `rstest` out of `rstest-bdd`'s manifest makes that failure impossible rather
  than merely unlikely, and it honours the roadmap's "without hiding the
  underlying crates" — `rstest` is the most important underlying crate of all.
  Alternative considered and rejected: re-export `fixture` and `rstest` with a
  caret requirement matching the workspace. Rejected because the mismatch is
  hard to test for and produces a confusing diagnostic when it occurs.
  Date/Author: 2026-09-14, planning agent. **Pending approval.**

- Decision D3: the macro re-exports sit behind a `macros` feature on
  `rstest-bdd`, enabled by default. `rstest-bdd` gains
  `rstest-bdd-macros = { workspace = true, optional = true }` and
  `[features] default = ["macros"]`, `macros = ["dep:rstest-bdd-macros"]`.
  The prelude module itself is unconditional; its macro re-export block is
  `#[cfg(feature = "macros")]`.
  Rationale: `rstest-bdd-harness-gpui` depends on `rstest-bdd` for runtime
  types only and should be able to opt out of building a proc-macro crate.
  A default-on feature keeps the ergonomic path zero-configuration while
  leaving the escape hatch. This mirrors the `serde`/`serde_derive` convention.
  Date/Author: 2026-09-14, planning agent. **Pending approval.**

- Decision D4: `#[harness_context]` is documented as needing no import, and
  that claim is proved by a compile test, rather than by adding a real no-op
  attribute macro.
  Rationale: the marker is inert and is stripped during `#[given]`/`#[when]`/
  `#[then]` expansion. Adding a `#[proc_macro_attribute] harness_context` whose
  only job is to be removed would create a second resolution path for the same
  syntax, risk the two disagreeing, and expand the public macro surface of a
  semver-compatible release for no user-visible gain. The roadmap's wording is
  imprecise rather than describing missing work; the honest fix is to correct
  the wording in the design document and prove the behaviour.
  Date/Author: 2026-09-14, planning agent. **Pending approval.**

- Decision D5: enforcement of the finish line uses three complementary
  artefacts — a dependency-restricted compile fixture (sufficiency), an
  import-allowlist contract test (actuality), and a docs/exports parity gate
  (documentation). An `insta` snapshot of the export list was considered and
  folded into the parity gate rather than duplicated, because the parity gate
  already makes every addition a reviewed diff in two places.
  Rationale: none of the three alone discharges the finish line. The compile
  fixture cannot prove the examples actually use the prelude; the allowlist
  test cannot prove the prelude is sufficient for a crate that does not already
  depend on the macro crate; neither touches documentation.
  Date/Author: 2026-09-14, planning agent. **Pending approval.**

- Decision D6: `examples/todo-cli/tests/cli.rs` is out of scope for the import
  allowlist. The allowlist applies to files that contain at least one
  `#[scenario]`, `#[given]`, `#[when]`, or `#[then]` attribute.
  Rationale: `cli.rs` is an `assert_cmd` end-to-end driver with no BDD steps;
  its `rstest_bdd_harness::binary_test_support` import is test-runner
  plumbing. Forcing it through the prelude would pull a binary locator into the
  framework's public ergonomic surface for no user benefit.
  Date/Author: 2026-09-14, planning agent. **Pending approval.**

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

| Item | Kind | Current path | Defined at |
| --- | --- | --- | --- |
| `StepResult<T, E>` | type alias | `rstest_bdd::StepResult` | `crates/rstest-bdd/src/lib.rs:294` |
| `StepError` | enum | `rstest_bdd::StepError` | `crates/rstest-bdd/src/lib.rs:194` |
| `Slot<T>` | struct | `rstest_bdd::Slot`, `rstest_bdd::state::Slot` | `crates/rstest-bdd/src/state.rs:30` |
| `ScenarioState` | trait | `rstest_bdd::ScenarioState` | `crates/rstest-bdd/src/state.rs:125` |
| `ScenarioState` | derive macro | `rstest_bdd_macros::ScenarioState` | `crates/rstest-bdd-macros/src/lib.rs:164` |
| `StepContext` | struct | `rstest_bdd::StepContext` | `crates/rstest-bdd/src/context/mod.rs` |
| `FixtureRef`, `FixtureRefMut` | structs | `rstest_bdd::FixtureRef`, `rstest_bdd::FixtureRefMut` | `crates/rstest-bdd/src/context/mod.rs` |
| `FixtureBorrowError` | enum | `rstest_bdd::FixtureBorrowError` | `crates/rstest-bdd/src/context/mod.rs` |
| `RSTEST_BDD_HARNESS_CONTEXT_FIXTURE` | const | `rstest_bdd::RSTEST_BDD_HARNESS_CONTEXT_FIXTURE` | `crates/rstest-bdd/src/context/mod.rs:49` |
| `given`, `when`, `then`, `scenario` | attribute macros | `rstest_bdd_macros::*` | `crates/rstest-bdd-macros/src/lib.rs` |
| `scenarios!` | function-like macro | `rstest_bdd_macros::scenarios` | `crates/rstest-bdd-macros/src/lib.rs` |
| `StepArgs`, `DataTable`, `DataTableRow` | derive macros | `rstest_bdd_macros::*` | `crates/rstest-bdd-macros/src/lib.rs` |

The "harness-context helpers" the roadmap names are the four methods on
`StepContext`, all already public: `insert_owned_harness_context`
(`crates/rstest-bdd/src/context/mod.rs:179`), `harness_context` (line 192),
`borrow_harness_context` (line 202), and `borrow_harness_context_mut`
(line 215). Methods are reached through their type, so exporting `StepContext`
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
qualified and needs no import (see
`examples/gpui-counter/tests/counter.rs:56`). `gpui::TestAppContext` in the
GPUI example is likewise already written fully qualified at the use site.

### Why the marker attribute cannot be re-exported

Roadmap 11.2.2 asks the prelude to expose "marker attributes from 11.2.1".
There is no such item. `crates/rstest-bdd-macros/src/lib.rs` declares exactly
four `#[proc_macro_attribute]` functions — `given`, `when`, `then`, and
`scenario` — and `#[harness_context]` is not among them. The marker is
recognized during argument classification in
`crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_context/mod.rs`
and stripped from the emitted function, so the compiler never resolves it as a
path.

This is not a gap in 11.2.1's delivery; it is how inert markers work, and it
is the same mechanism `rstest` uses for `#[from(...)]` and `#[case]`. The
11.2.1 ExecPlan's `Decision log` deferred the question of prelude exposure to
this item precisely so it could be settled here. Decision D4 settles it: the
prelude exposes `given`, `when`, and `then`, and those attributes understand
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
- `tests/fixtures/published-gpui-e2e/` — a crate deliberately outside the root
  workspace, with its own lockfile managed by
  `scripts/check_fixture_lockfiles.py`. This is the template for the
  dependency-restricted compile fixture.

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
  should stay semver-compatible… it must not remove the existing
  `StepContext`, harness, or macro surfaces."
- **STANDARDS:** `AGENTS.md`; `docs/documentation-style-guide.md`;
  `docs/rust-doctest-dry-guide.md`.

Trace links from requirement through milestone to evidence:

```plaintext
REQ-11.2.2 (prelude exposes the named items)
    -> DESIGN-2.7.6.4 (v0.6.1 prelude candidate)
        -> EP-M2 -> scripts/tests/test_rstest_bdd_macros_feature.py
        -> EP-M3 -> crates/rstest-bdd/tests/prelude_exports.rs
REQ-11.2.2-FL-a (compile tests prove examples import only prelude + harness)
    -> EP-M4 -> tests/fixtures/prelude-only/ compiles under `make test`
    -> EP-M5 -> examples/*/tests/*.rs import lists
    -> EP-M6 -> scripts/tests/test_example_prelude_imports.py
REQ-11.2.2-FL-b (docs list the exported items)
    -> EP-M7 -> docs/users-guide.md "Prelude" table
    -> EP-M7 -> scripts/check_prelude_exports.py wired into `make lint`
PHASE-CONSTRAINT-semver
    -> EP-M3 -> crates/rstest-bdd/tests/prelude_exports.rs::original_paths_still_resolve
ADR-007-addendum (marker needs no import)
    -> EP-M4 -> tests/fixtures/prelude-only/ marker usage compiles
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
- Clippy's `wildcard_imports` lint does not fire on a glob import from a module
  named `prelude` when `warn-on-all-wildcard-imports` is `false`. This is the
  documented default. EP-M0 exercises it empirically rather than trusting the
  documentation, because the entire ergonomic proposition collapses if it is
  wrong under this workspace's lint configuration.
- Cargo permits a dev-dependency cycle (`rstest-bdd` -> `rstest-bdd-macros`
  normal, `rstest-bdd-macros` -> `rstest` dev) and unifies a normal and a
  dev-dependency edge onto the same crate into one build with the union of
  their features. EP-M2 exercises this rather than assuming it.
- `trybuild` and `rstest` behave as documented. Their internals are not
  verified.

### Obligations

**INV-1 — Prelude sufficiency.**

- Obligation: a crate whose manifest names only `rstest-bdd`, `rstest`, and one
  first-party harness adapter, and whose source imports only
  `rstest_bdd::prelude::*` plus that adapter, can express a complete scenario:
  fixtures, all three step keywords, a fallible step returning `StepResult`, a
  `ScenarioState` implementation using `Slot`, a `#[harness_context]`
  parameter, and a `#[scenario]` binding.
- Method: a dependency-restricted compile fixture — a crate under
  `tests/fixtures/prelude-only/`, outside the root workspace, compiled by
  `cargo test` against its own manifest.
- Rationale: this is a statement about *name resolution under a restricted
  dependency set*. Only a real compile against a real restricted manifest can
  establish it; a test inside `crates/rstest-bdd` would inherit that crate's
  dev-dependencies and prove nothing.
- Domain: one crate exercising each category of exported item at least once.
  See the checklist in EP-M4.
- Artefact: `tests/fixtures/prelude-only/` plus the Makefile target that builds
  it.
- Evidence: before EP-M3, the fixture fails to compile with `unresolved import
  rstest_bdd::prelude`. After EP-M3 and EP-M4 it compiles and its scenario
  passes.
- Non-vacuity: the fixture must not merely compile — it must *run* a scenario
  that asserts something, so a fixture that silently bound zero steps would
  fail. The negative control is explicit: EP-M4 records a transcript from
  temporarily deleting one name (`Slot`) from the prelude's export list and
  observing the fixture fail to compile with `cannot find type Slot in this
  scope`. A fixture that still compiled after that deletion would prove the
  test was not exercising the export.

**INV-2 — Additive-only public surface.**

- Obligation: every path that resolved before this change still resolves
  afterwards, and resolves to the *same* item. In particular
  `rstest_bdd::Slot`, `rstest_bdd::state::Slot`, and
  `rstest_bdd::prelude::Slot` must all be one type, not three.
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
  the set of crate roots named by its `use` statements is a subset of
  {`rstest_bdd::prelude`, the example's own crate, the example's declared
  harness adapter crate, `std`, `core`}.
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

- Obligation: the set of item names in the users' guide prelude table is
  exactly the set of names re-exported by `crates/rstest-bdd/src/prelude.rs`.
- Method: a Python gate script comparing the two, wired into `make lint`,
  itself covered by `pytest` unit tests.
- Rationale: "docs list the exported items" is half the finish line, and
  hand-maintained lists drift. The repository has no public-API tooling to lean
  on (confirmed: no `cargo-public-api`, no `cargo-semver-checks`), so a
  bespoke gate is the only option. The `scripts/check_gpui_mapping_table.py`
  precedent shows this pattern is accepted here.
- Domain: the full export list; the extractor parses `pub use` statements,
  including brace-grouped and `#[cfg(feature = "macros")]`-gated blocks.
- Artefact: `scripts/check_prelude_exports.py`, tests in
  `scripts/tests/test_check_prelude_exports.py`, Makefile wiring at the end of
  the `lint` recipe.
- Evidence: running the script against a table missing one row exits 1 naming
  that row; against the committed pair it exits 0.
- Non-vacuity: the script must fail if the table is empty, if the `pub use`
  list is empty, or if the marker headings cannot be found — an extractor that
  parsed nothing would otherwise compare two empty sets and pass. The unit
  tests assert each of those failure modes explicitly, plus a missing-row case
  and a surplus-row case, so the gate is shown to reject in both directions.

**LEMMA-1 — Feature-gated macro re-exports do not change the default build.**

- Obligation: with default features, `rstest_bdd::prelude::given` resolves; with
  `--no-default-features`, the crate still compiles, the runtime items still
  resolve through the prelude, and `rstest-bdd-macros` is not in the dependency
  graph.
- Method: a manifest contract test plus two explicit `cargo check` invocations
  recorded in `Concrete steps`.
- Rationale: a feature flag's whole purpose is that both settings work; only
  building both settings establishes it. `make lint` and `make test` use
  `--all-features`, so the `--no-default-features` path would otherwise never be
  exercised.
- Domain: the two settings of the single `macros` feature.
- Artefact: `scripts/tests/test_rstest_bdd_macros_feature.py` for the manifest
  shape; the two `cargo` commands for the build reality.
- Evidence: `cargo check -p rstest-bdd --no-default-features` succeeds and
  `cargo tree -p rstest-bdd --no-default-features -i rstest-bdd-macros` reports
  that the package is not in the graph.
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
of thirteen re-exported names degenerates into the parameterized test already
specified in INV-2, and a bounded model check has no transition system to
explore. If implementation reveals a genuine invariant over a range — for
example, if the parity gate's extractor needs to handle arbitrary nested `cfg`
expressions — return to this section, state the invariant, and add a
`proptest` over generated `pub use` blocks before continuing.

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

Create `crates/rstest-bdd/tests/prelude_exports.rs`, holding the INV-2
identity assertions. Use `googletest`'s `#[gtest]` and `assert_that!` for value
assertions and `pretty_assertions` for structural comparisons, per the house
style at `docs/developers-guide.md` lines 2827-2839. The type-identity checks
are compile-time: for each exported type, a function whose parameter is typed
by the origin path and whose argument is typed by the prelude path, or the
equivalent `const _: fn(OriginPath) = |_| ();` shape. Prefer a small helper
macro over thirteen near-identical blocks, but keep the helper readable — this
file exists to be diffed by reviewers.

Create `scripts/tests/test_example_prelude_imports.py` with the INV-3 checker
and its unit tests, including the synthetic-tree negative control.

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

Edit `crates/rstest-bdd/Cargo.toml` only. Add to `[dependencies]`:

```toml
rstest-bdd-macros = { workspace = true, optional = true }
```

Add a `[features]` section (the crate has none today; place it after
`[lints]` and before `[dependencies]`, matching the layout in
`crates/rstest-bdd-macros/Cargo.toml`):

```toml
[features]
default = ["macros"]
# Re-export the step, scenario, and derive macros through `rstest_bdd::prelude`.
# Disable to depend on the runtime types alone without building the proc-macro
# crate; see ADR-022.
macros = ["dep:rstest-bdd-macros"]
```

Leave the existing dev-dependency on `rstest-bdd-macros` in place: it enables
`compile-time-validation`, which the normal dependency deliberately does not.

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
Explicitly say that `rstest`'s own `#[fixture]` and `#[rstest]` are *not*
re-exported, that users should write `#[rstest::fixture]` or import them from
`rstest` directly, and that this is so the consumer's `rstest` version is the
only one in play. A reader who wonders "why isn't `fixture` here?" must find
the answer in the module they are already looking at.

The export list, grouped and commented by purpose:

```rust
// Step outcomes and errors.
pub use crate::{StepError, StepResult};
// Scenario state.
pub use crate::{ScenarioState, Slot};
// The step-context store and its borrow guards, including the
// harness-context accessors.
pub use crate::{
    FixtureBorrowError, FixtureRef, FixtureRefMut, RSTEST_BDD_HARNESS_CONTEXT_FIXTURE, StepContext,
};
// Step and scenario macros.
#[cfg(feature = "macros")]
pub use rstest_bdd_macros::{
    DataTable, DataTableRow, ScenarioState, StepArgs, given, scenario, scenarios, then, when,
};
```

Note the deliberate collision: `ScenarioState` names both the trait
(`crate::state::ScenarioState`) and the derive macro
(`rstest_bdd_macros::ScenarioState`). Rust keeps traits and derive macros in
separate namespaces, so a single `use rstest_bdd::prelude::*;` brings both into
scope and `#[derive(ScenarioState)]` alongside `impl ScenarioState for …` both
work. Verify this compiles rather than assuming it; if the two cannot coexist
in one module, that is a design discovery to record in
`Surprises & discoveries` and resolve by renaming the derive re-export with an
explicit note in the users' guide — not by dropping one.

Do not export: the `__rstest_bdd_`-prefixed internals, `inventory::{iter,
submit}`, `FluentLanguageLoader`, the registry lookup functions, the
localization API, or `StepPattern`. These are framework-internal or
specialist; a prelude that carried them would be a mirror of the crate root and
would forfeit the bounded-hazard argument in `Risks`.

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

Create `tests/fixtures/prelude-only/` as a crate outside the root workspace.
Model its shape and lockfile handling on `tests/fixtures/published-gpui-e2e/`,
and register it with `scripts/check_fixture_lockfiles.py` so
`make check-fixture-lockfiles` covers it.

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

and exercises, at minimum: a `#[rstest::fixture]`; a `#[given]`, a `#[when]`,
and a `#[then]`; a step returning `StepResult<()>` that succeeds; a type
implementing the `ScenarioState` trait via `#[derive(ScenarioState)]` with a
`Slot<T>` field; a step taking `#[harness_context] ctx: &TokioTestContext`;
and a `#[scenario(path = …, harness = rstest_bdd_harness_tokio::TokioHarness)]`
binding that runs and asserts. The scenario must assert something real, so that
a build binding zero steps fails rather than passes.

Add a Makefile target that builds and runs it, and invoke that target from
`test`. Follow the naming and structure of the existing
`check-published-gpui` / `e2e-published-gpui` targets so the Makefile stays
legible, and validate with `mbake validate Makefile`.

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

Do not remove `rstest`, `rstest-bdd-macros`, or `rstest-bdd-test-macros` from
the example manifests. All three are still required: `rstest` because generated
code emits `#[rstest::rstest]` and the `src/` unit tests use it,
`rstest-bdd-macros` because `--all-features` builds of the workspace resolve
macro paths through it in other contexts, and `rstest-bdd-test-macros` for
`#[rstest_bdd_test_macros::allow_fixture_expansion_lints]`. Removing a manifest
entry is not what the finish line asks for; it asks about imports.

If it turns out that `rstest-bdd-macros` can be removed from an example's
`[dev-dependencies]` with everything still green, that is a bonus — verify it
per-crate with `cargo check` rather than assuming, and record the result in
`Surprises & discoveries`.

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
test in `scripts/tests/`, mirroring the
`scripts/users_guide_links.py` / `scripts/check_users_guide_links.py` split.

Validation: `pytest scripts/tests/test_example_prelude_imports.py` passes
including its negative control; `make lint` (which runs `ruff` and `pylint`)
green; `make test` green.

Commit.

### EP-M7 — documentation and the parity gate

Four documents change, plus one new ADR and one new gate.

`docs/users-guide.md`: add a "Prelude" subsection under "Harness adapter core
APIs" (around line 2326), carrying a table with one row per exported item
giving the name, its kind, and a one-line purpose — this is the artefact the
finish line calls "docs list the exported items". State the canonical two-line
import, state that `rstest`'s `#[fixture]` is deliberately absent and how to
write it instead, and state that `#[harness_context]` needs no import. Add a
cross-reference from "Scenario state slots" (line 546) and from the
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
must record: the glob-import semver hazard and why a curated list bounds it;
the `rstest` exclusion and the two-versions-of-`rstest` failure it prevents;
the `macros` feature; and the rule that any future addition requires an ADR
amendment. Cite the Cargo SemVer reference on glob imports and the tokio
prelude removal (tokio-rs/tokio#3257) as the prior art that informed the
curation policy. Add it to `docs/contents.md`.

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
  changed; §2.7.6.3's "adapter users need only the adapter crate" still holds
  because the new edge runs the other way. Recovery: revert the manifest hunk.
  Remaining gaps: no prelude exists. Compatibility decision: none — the feature
  is new, so nothing depends on its absence.

- **EP-M3.** Outcome: `rstest_bdd::prelude` exists, is documented, and is
  proved to name the same items as the original paths. Requirements:
  discharges INV-2, advances REQ-11.2.2. Acceptance evidence:
  `prelude_exports.rs` green plus the aliasing negative control. Conformance
  check: public surface grew additively only; every prior path still resolves;
  design §2.7.6.4 not yet updated (EP-M7). Recovery: delete `prelude.rs` and
  its `pub mod` line. Remaining gaps: nothing yet proves sufficiency or example
  adoption. Compatibility decision: none.

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
  evidence: the contract test green including its negative control.
  Conformance check: new test only; no interface change. Recovery: delete the
  test file. Remaining gaps: documentation. Compatibility decision: none.

- **EP-M7.** Outcome: the exported items are documented, the policy is recorded
  in an ADR, and drift between the two is gated. Requirements: discharges
  INV-4 and REQ-11.2.2-FL-b. Acceptance evidence: `make lint` runs the parity
  gate and passes; the gate's own unit tests pass; Markdown gates green.
  Conformance check: §2.7.6.4 now matches the implementation; ADR-007 addendum
  reconciled with D4; no undocumented public item remains. Recovery: revert the
  documentation commit and remove the Makefile line. Remaining gaps: the
  roadmap still shows the item open. Compatibility decision: none.

- **EP-M8.** Outcome: roadmap ticked, full gates green, pull request open.
  Requirements: closes REQ-11.2.2. Acceptance evidence: the `scrutineer` gate
  report. Conformance check: every trace link in `Conformance basis` resolves
  to a passing artefact; no upstream deviation left unrecorded. Recovery: the
  branch is not merged until review passes. Remaining gaps: none.
  Compatibility decision: none.

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
cargo tree -p rstest-bdd --no-default-features -i rstest-bdd-macros
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

1. `use rstest_bdd::prelude::*;` brings `StepResult`, `StepError`, `Slot`, the
   `ScenarioState` trait, the `ScenarioState` derive, `StepContext`,
   `FixtureRef`, `FixtureRefMut`, `FixtureBorrowError`,
   `RSTEST_BDD_HARNESS_CONTEXT_FIXTURE`, `given`, `when`, `then`, `scenario`,
   `scenarios`, `StepArgs`, `DataTable`, and `DataTableRow` into scope.
2. Every one of those names still resolves at its pre-existing path, to the
   same item.
3. `tests/fixtures/prelude-only/` compiles and its scenario passes, with a
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
- Security: not applicable; no new third-party dependency, no `unsafe`, no
  input parsing outside the two developer-facing gate scripts.

### Quality method

`scrutineer` runs the gates sequentially, tees each to a log under `/tmp`, and
returns a bounded report. On failure, read the cited log rather than re-running
the gate; re-run only after applying a fix.

## Idempotence and recovery

Every step is re-runnable. The Rust edits are ordinary source changes; the
Python gates are pure functions of the tree; the Makefile additions are
declarative.

The only step needing care is EP-M4's fixture lockfile. Follow the
`tests/fixtures/published-gpui-e2e` pattern exactly, and if
`make check-fixture-lockfiles` reports drift, regenerate rather than
hand-editing.

Each milestone is a separate commit, so recovery from any point is
`git revert` of that commit. Because the prelude is purely additive and the
examples are migrated in a single commit, no intermediate state requires a
compatibility shim to remain coherent.

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
    FixtureBorrowError, FixtureRef, FixtureRefMut, RSTEST_BDD_HARNESS_CONTEXT_FIXTURE,
    ScenarioState, Slot, StepContext, StepError, StepResult,
};
#[cfg(feature = "macros")]
pub use rstest_bdd_macros::{
    DataTable, DataTableRow, ScenarioState, StepArgs, given, scenario, scenarios, then, when,
};
```

New manifest surface on `crates/rstest-bdd/Cargo.toml`:

```toml
[features]
default = ["macros"]
macros = ["dep:rstest-bdd-macros"]
```

New repository artefacts:

- `crates/rstest-bdd/src/prelude.rs`
- `crates/rstest-bdd/tests/prelude_exports.rs`
- `tests/fixtures/prelude-only/` (crate, feature file, lockfile)
- `scripts/check_prelude_exports.py`
- `scripts/tests/test_check_prelude_exports.py`
- `scripts/tests/test_example_prelude_imports.py`
- `scripts/tests/test_rstest_bdd_macros_feature.py`
- `docs/adr-022-public-prelude-export-policy.md`

Modified: `crates/rstest-bdd/Cargo.toml`, `crates/rstest-bdd/src/lib.rs`,
`Makefile`, the four example test files, `docs/users-guide.md`,
`docs/developers-guide.md`, `docs/rstest-bdd-design.md`,
`docs/adr-007-harness-context-injection.md`, `docs/contents.md`,
`docs/roadmap.md`.

No new third-party dependency is introduced. The single new first-party
dependency edge is `rstest-bdd` -> `rstest-bdd-macros`, optional and default-on.

## Revision notes

- 2026-09-14: initial draft. Records the four scoping decisions the roadmap
  text leaves open — where the prelude lives (D1), the deliberate exclusion of
  `rstest` (D2), the `macros` feature (D3), the inert marker (D4) — together
  with the enforcement triple (D5) and the `cli.rs` exemption (D6). These are
  marked **Pending approval**; they define the deliverable, so confirming or
  changing one changes what "done" means.
