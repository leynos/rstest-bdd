# Expose a public prelude for common integration imports (roadmap 11.2.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IMPLEMENTATION COMPLETE — the user approved this plan on 2026-10-25 and
directed full implementation through PR completion on 2026-11-01. EP-M0 through
EP-M7 have landed on the branch: the dependency edge is in place, the crate
root re-exports the macro surface, `rstest_bdd::prelude` binds all thirteen
items across eleven spellings, the step attributes document
`#[harness_context]`, a compile-fail fixture pins the diagnostic for the marker
written without a step attribute, the examples reach the facade through the
prelude alone, the fixture crates prove a downstream manifest, the drift gate
holds the documentation to the code, ADR-027 records the decision, and
`docs/roadmap.md` carries the amended finish line. Pull request 773 is in
review against `main`; the branch was restacked onto `main` on 2026-11-23 after
the stacked base merged, and the record below states what moved. The corrected
package-identity count is 129 normal packages, within the approved ceiling of
135; see DL-14. DL-11 records the revised file-count tolerance; DL-12 adapts
EP-M0 to the shared build environment. The follow-up items the plan
deliberately left out are filed as issues #952-#959.

## Purpose / big picture

Writing a behavioural test with this framework currently means reaching into
two or three separate crates and knowing which item lives where. Every example
in the repository opens the same way:

```rust,ignore
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
```

A reader who then wants scenario state adds a third line from a fourth place:

```rust,ignore
use rstest_bdd::{ScenarioState as _, Slot};
use rstest_bdd_macros::{given, scenario, then, when, ScenarioState};
```

Nothing here is wrong, but the split is arbitrary from the outside. `given` and
`Slot` are one feature to the person using them; that they live in a procedural
macro crate and a runtime crate respectively is an implementation fact about
how Rust compiles procedural macros, not something a test author should have to
carry.

After this change, one import covers the common case:

```rust,ignore
use rstest_bdd::prelude::*;
```

The underlying crates stay exactly where they are and stay directly usable. The
prelude adds a path; it removes nothing and hides nothing. Roadmap 11.2.2 is
explicit about that balance: the prelude exists "so examples can import one
predictable module **without hiding the underlying crates**".

Success is observable in three ways:

1. The four example crates under `examples/` build and pass while importing
   only `rstest_bdd::prelude` and, where relevant, their harness crate. The
   compiler enforces this because `rstest-bdd-macros` is removed from those
   crates' manifests entirely — there is no longer a path by which they could
   reach it.
2. `docs/users-guide.md` carries a section listing every item the prelude
   exports, and a repository gate fails if that list and the code disagree.
3. A downstream crate that never mentions `rstest-bdd-macros` in its manifest
   can still write and run a complete scenario, proved by a fixture crate
   compiled during the test run.

Roadmap entry 11.2.2 in `docs/roadmap.md` (lines 1038-1043) is the requirement
of record. Its finish line is: *"compile tests prove examples import only the
prelude plus their harness crate, and docs list the exported items."* Its
stated prerequisite, 11.2.1, is complete (`docs/roadmap.md:1023-1037`).

## Context and orientation

Read this section before touching anything. It assumes no prior knowledge of
the repository.

### What this repository is

`rstest-bdd` is a behaviour-driven-development testing framework for Rust built
on top of the `rstest` fixture library. A user writes Gherkin `.feature` files
and annotates ordinary Rust functions with `#[given]`, `#[when]`, and `#[then]`
to bind them to Gherkin steps, then uses `#[scenario]` or `scenarios!` to
generate a `#[test]` function per scenario.

The workspace root is the directory containing this checkout's `Cargo.toml`.
All paths in this plan are relative to it.

### The crates, and which way the dependencies point

This matters more than anything else in the plan, so it is stated precisely.
Verified by reading every `crates/*/Cargo.toml`.

| Crate                      | Path                              | Role                                    | Depends on (workspace crates only) |
| -------------------------- | --------------------------------- | --------------------------------------- | ---------------------------------- |
| `rstest-bdd-policy`        | `crates/rstest-bdd-policy`        | Shared constants and policy enums       | none                               |
| `rstest-bdd-patterns`      | `crates/rstest-bdd-patterns`      | Step-pattern parsing                    | none                               |
| `rstest-bdd-harness`       | `crates/rstest-bdd-harness`       | Harness adapter contracts               | none                               |
| `rstest-bdd-macros`        | `crates/rstest-bdd-macros`        | The procedural macros                   | patterns, harness, policy          |
| `rstest-bdd`               | `crates/rstest-bdd`               | Runtime: registry, `StepContext`, state | patterns, policy                   |
| `rstest-bdd-harness-tokio` | `crates/rstest-bdd-harness-tokio` | Tokio adapter                           | harness                            |
| `rstest-bdd-harness-gpui`  | `crates/rstest-bdd-harness-gpui`  | GPUI adapter                            | harness, **rstest-bdd**            |

*Table: workspace crate dependency edges as they stand before this plan.*

The critical fact is the one that is **absent**: `rstest-bdd-macros` does not
depend on `rstest-bdd`, and `rstest-bdd` does not depend on `rstest-bdd-macros`
as a normal dependency. `rstest-bdd` lists it only under `[dev-dependencies]`
(`crates/rstest-bdd/Cargo.toml:48`, with
`features = ["compile-time-validation"]`).

That separation is deliberate and documented.
`docs/developers-guide.md:387-393` states the policy:

> Use the existing dependency-load-bearing crates before introducing or widening
> edges. `rstest-bdd-patterns` owns shared pattern parsing, `rstest-bdd-policy`
> owns shared runtime and attribute-policy classification, and
> `rstest-bdd-harness` owns adapter contracts and test-staging helpers. The
> procedural macro crate may depend on those shared crates, but it must not
> depend on `rstest-bdd`; macro/runtime integration tests live under
> `rstest-bdd`
> instead.

`docs/adr-004-policy-crate.md` records why: the macro crate needed
`RuntimeMode` and `TestAttributeHint`, could not depend on the runtime crate,
and had been keeping a duplicate copy that drifted. The policy crate exists to
be the shared floor beneath both.

Note carefully what that policy forbids and what it does not. It forbids
`rstest-bdd-macros -> rstest-bdd`. It says nothing about
`rstest-bdd -> rstest-bdd-macros`, which points the other way and therefore
creates no cycle. This plan proposes that second edge. Because the policy text
is currently phrased as a single rule about the macro crate, the plan must amend
`docs/developers-guide.md` to state both directions explicitly, so the next
reader does not have to re-derive that the new edge is legal.

There is a second, subtler rule in the same section
(`docs/developers-guide.md:378-385`):

> Publishable first-party crates must keep their package-time dependency graph
> acyclic across normal, build, and development dependencies. `cargo package`
> resolves development dependencies while preparing a crate, so a dev-dependency
> cycle can block a live release even when the runtime dependency graph is
> acyclic.

`rstest-bdd` already dev-depends on `rstest-bdd-macros`, so the publication
order already places the macro crate first. Promoting that edge from a
dev-dependency to a normal dependency does not reorder anything.
`make publish-check` runs `lading publish`, which derives release order from
the graph, and is the gate that proves this.

### Where the roadmap's named items live today

- `StepResult<T, E = StepError>` is a type alias declared directly in
  `crates/rstest-bdd/src/lib.rs:294`.
- `Slot<T>` is declared at `crates/rstest-bdd/src/state.rs:30` and re-exported
  at the crate root by `crates/rstest-bdd/src/lib.rs:111`.
- `ScenarioState` is a trait at `crates/rstest-bdd/src/state.rs:125`,
  re-exported by the same line 111. A **derive macro of the same name** lives
  in the macro crate at `crates/rstest-bdd-macros/src/lib.rs:164`. A trait and
  a derive macro occupy different namespaces, so one module may export both
  under one name; `serde` does exactly this with `Serialize`.
- The "harness-context helpers" are five inherent methods on `StepContext`, in
  `crates/rstest-bdd/src/context/mod.rs`: `insert_harness_context` (line 168),
  `insert_owned_harness_context` (179), `harness_context` (192),
  `borrow_harness_context` (202), and `borrow_harness_context_mut` (215).
  Inherent methods travel with their type, so exporting `StepContext` exports
  the helpers. There is nothing separate to re-export.
- `RSTEST_BDD_HARNESS_CONTEXT_FIXTURE` is declared at
  `crates/rstest-bdd/src/context/mod.rs:49` as an alias for
  `rstest_bdd_policy::HARNESS_CONTEXT_FIXTURE`
  (`crates/rstest-bdd-policy/src/lib.rs:114`).

### The marker attribute from 11.2.1, and why it is not an export

Roadmap 11.2.2 asks the prelude to expose "marker attributes from 11.2.1".
Taken literally this is not possible, and understanding why prevents a wasted
milestone.

`#[harness_context]` is **not** a `#[proc_macro_attribute]`. It is an inert
marker written on a *parameter* of a function that is already being rewritten by
`#[given]`, `#[when]`, or `#[then]`. Those macros see it as raw attribute
syntax in their input token stream and strip it during expansion. The
classification code is
`crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_context/mod.rs:76`
(`classify_harness_context`), reached via
`extract_flag_attribute(arg, "harness_context")` at line 81.

This is the same mechanism `rstest` itself uses for `#[from(...)]` and
`#[case]`: the attribute name never resolves to an item, so there is no item to
import and no item to re-export. A user writes `#[harness_context]` today
without importing anything, and will continue to.

The plan therefore treats this clause as a **documentation** obligation rather
than an export obligation, and says so in the prelude's own rustdoc so the next
reader is not left hunting for a missing symbol. Whether to promote the marker
into a real no-op attribute macro purely for tooling discoverability is a
separate question, addressed in the `Decision log`.

### How generated code finds the runtime crate

When `#[scenario]` expands, it must emit paths such as
`::rstest_bdd::StepContext`. It does not hardcode that name. It calls
`proc_macro_crate::crate_name("rstest-bdd")`, which reads the *consuming*
crate's `Cargo.toml` and returns the local name under which that crate is
declared, handling renames. The lookup table is at
`crates/rstest-bdd-macros/src/codegen/mod.rs:28-46`, the call at line 57, and
the resolution at lines 187-197, with `FoundCrate::Itself` handled for the
runtime crate's own tests.

The consequence for this plan is important and easy to get wrong: a user who
obtains the macros *through* `rstest_bdd::prelude` must still declare
`rstest-bdd` in their own manifest, because that is what the macro looks up.
They do not need to declare `rstest-bdd-macros`. Every example already declares
`rstest-bdd`, so this holds, but the plan verifies it explicitly with a fixture
crate rather than assuming it.

### What the examples import today

- `examples/todo-cli/tests/todo.rs:3-4` — `use rstest::fixture;` and
  `use rstest_bdd_macros::{given, scenario, then, when};`
- `examples/japanese-ledger/tests/ledger.rs:8-9` — the same two.
- `examples/gpui-counter/tests/counter.rs:8-9` — the same two, plus
  `harness = rstest_bdd_harness_gpui::GpuiHarness` written fully qualified
  inside the `#[scenario]` attribute at lines 56 and 63.
- `examples/tokio-reminders/tests/reminders.rs:6-8` — the same two plus
  `use rstest_bdd_harness_tokio::TokioTestContext;`, with
  `harness = rstest_bdd_harness_tokio::TokioHarness` at lines 99, 106, and 113.

Every example also carries
`#[rstest_bdd_test_macros::allow_fixture_expansion_lints]`, written fully
qualified, from the unpublished `rstest-bdd-test-macros` scaffolding crate.
That crate is `publish = false` and is test scaffolding, not part of the user
story; it is out of scope and stays.

No example imports `StepResult`, `Slot`, `ScenarioState`, or `StepContext`
today, which is why the roadmap's premise holds: there is no single predictable
module to point a newcomer at.

### The gates this repository runs

Prefer the Makefile targets over raw cargo invocations.

- `make check-fmt` — `cargo fmt --workspace -- --check`.
- `make lint` — clippy across the workspace with `-D warnings`, plus
  `cargo doc --workspace --no-deps`, the Whitaker Dylint suite
  (`make lint-whitaker`), the Python lints, and the structural checks in
  `scripts/`: `check_rs_file_lengths.py`, `check_unsafe_code_allows.py`,
  `check_users_guide_links.py`, `check_gpui_mapping_table.py`, and
  `check_serial_nextest_matrix.py`.
- `make test` — the workspace test suite via nextest, plus doctests.
- `make markdownlint` — Markdown lint; depends on `spelling`, which runs
  `typos`.
- `make nixie` — validates Mermaid diagrams.
- `make publish-check` — `lading publish`, packaging every crate in release
  order. This is the gate that proves a new dependency edge has not broken
  publication.

What the repository does **not** have is any automated public-API gate. There
is no `cargo-public-api` and no `cargo-semver-checks` anywhere in the
`Makefile` or `.github/workflows/`. Semver discipline here is currently a
matter of review, which is directly relevant to a change whose entire purpose
is to add public surface.

### Skills and documentation to load before starting

Load these before the first edit:

- `execplans` — the conventions this document follows.
- `rust-router` — routes to the smallest useful Rust skill; from it,
  `arch-crate-design` for the crate-boundary and feature-flag decisions and
  `rust-unit-testing` for the `rstest` and `googletest` assertion vocabulary.
- `arch-decision-records` — the Y-Statement format, if the `Decision log`
  concludes an ADR is warranted.
- `arch-supply-chain` — for the semver guardrail discussion.
- `addressing-whitaker-findings` — the Whitaker Dylint suite runs in
  `make lint` and has a per-lint remediation playbook.
- `commit-message` — this repository commits with file-based messages.
- `en-gb-oxendict` — all prose here is British English with Oxford spelling.

Read these documents:

- `docs/roadmap.md:995-1080` — phase 11 and the 11.2.2 entry itself.
- `docs/rstest-bdd-design.md` §2.7.6.4 (lines 2182-2196) — records the prelude
  as an unshipped v0.6.1 candidate; this plan must update it.
- `docs/developers-guide.md:368-393` — the workspace dependency policy quoted
  above.
- `docs/adr-004-policy-crate.md` — why the layering is the shape it is.
- `docs/adr-007-harness-context-injection.md` — the reserved fixture key and
  the 11.2.1 addendum describing `#[harness_context]`.
- `docs/execplans/11-2-1-annotate-parameters-with-harness-context.md` — the
  immediate prerequisite, and the house style this plan mirrors.
- `docs/documentation-style-guide.md` — the ADR template and the Markdown
  wrapping rules.
- `docs/users-guide.md` — the document that must gain the exported-items list.
- `docs/rust-doctest-dry-guide.md` — doctest conventions, relevant because the
  prelude's rustdoc will carry runnable examples.
- `docs/testing-strategy.md` and `docs/rust-testing-with-rstest-fixtures.md` —
  the testing vocabulary expected here.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` — the house view
  on file and function size, relevant to the 400-line ceiling.

## What this plan builds

The design below is the output of a six-lens design review whose findings are
recorded in `Decision log`. Several of the obvious first answers turned out to
be wrong, so read the rationale before changing any of it.

### The shape

Two surfaces, not one.

A reasonable reader will ask why, if the crate root gains the macros, the
prelude module is needed at all — `cucumber` ships the root re-export and no
prelude. There is a concrete answer beyond the roadmap's wording, and it is the
strongest argument for the module: **Clippy's `wildcard_imports` lint matches
on a path segment named `prelude`.** A downstream crate running
`#![warn(clippy::pedantic)]` can write `use rstest_bdd::prelude::*;` without a
finding, and cannot glob a crate root without one. `clippy.toml` does not set
`warn-on-all-wildcard-imports`, whose default is `false`, so the exemption
holds under this workspace's configuration too. A prelude therefore buys one
thing a root re-export cannot: a supported glob for users on pedantic Clippy.
EP-M0 re-probes this before any work depends on it.

**The crate root gains the macros.** `crates/rstest-bdd/src/lib.rs` re-exports
`given`, `when`, `then`, `scenario`, `scenarios`, and the derive macros from
`rstest-bdd-macros`. This is the load-bearing half of the change: it is what
makes `use rstest_bdd::{given, when, then};` compile.

That sentence is worth pausing on, because the repository already claims it
works. `README.md:116`, `crates/rstest-bdd/README.md:106`, and
`crates/rstest-bdd-macros/README.md:106` all open their worked example with:

```rust,ignore
use rstest_bdd::{scenario, given, when, then, StepResult};
```

and each of the three READMEs repeats the shape four more times (lines 188/198,
227/237, 253/263, 271/281). None of it compiles today:
`crates/rstest-bdd/src/lib.rs` contains no `pub use rstest_bdd_macros`
anywhere. No README is doctested — no crate does
`#[doc = include_str!("../README.md")]` and no Makefile target checks them —
which is how fifteen wrong import lines have sat on the project's front page
unnoticed. This plan makes them true.

**The prelude is a thin, curated re-export of that root.**
`rstest_bdd::prelude` holds the subset a step-writing file actually needs. It
never introduces a name that is not also reachable at the crate root, so there
are never two public APIs to keep coherent.

### The dependency edge

`rstest-bdd` takes `rstest-bdd-macros` as a **normal, unconditional**
dependency. Not optional, and not behind a feature.

This is cycle-free, as established in "Context and orientation":
`rstest-bdd-macros` reaches only `rstest-bdd-patterns`, `rstest-bdd-harness`,
and `rstest-bdd-policy`, none of which reaches `rstest-bdd`. The edge already
exists in dev position, so the graph shape is proven. `lading.toml:4-12`
already lists `rstest-bdd-macros` before `rstest-bdd` in the publish order, so
no release-process change is needed.

The review's first instinct was a default-on `macros` feature, reaching for
serde's `derive` as precedent. That citation does not survive contact with
serde: its `derive` feature is **opt-in**, as this repository's own
`serde = { version = "1.0", features = ["derive"], optional = true }`
(`crates/rstest-bdd/Cargo.toml:26`) demonstrates. serde is therefore precedent
for a default-*off* feature, not a default-on one, and it was withdrawn. The
gating idea was rejected on its own merits, for four independent reasons, each
sufficient on its own:

1. **It would break `make lint` on the first run.** Annotating a gated
   re-export requires `#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]`, and
   `doc(cfg)` is unstable. serde guards it with
   `#![cfg_attr(docsrs, feature(doc_cfg))]` and relies on docs.rs building on
   nightly. Here `rust-toolchain.toml` pins `channel = "stable"` and
   `Makefile:27` sets `RUSTDOC_FLAGS ?= --cfg docsrs -D warnings`
   unconditionally, so the `docsrs` cfg is **always on, on stable**. A grep for
   `doc_cfg` or `cfg_attr(docsrs` across `crates/**/*.rs` returns zero hits.
   The annotation would fail with E0658 immediately; omitting it would render
   the macros on docs.rs with no feature badge.
2. **Nothing in the repository can observe the feature switched off.**
   `Makefile:28` sets
   `CARGO_FLAGS ?= --workspace --all-targets --all-features`, and the Linux CI
   lane runs with `all-features: 'true'`. The only
   `with-default-features: false` lane is Windows, and its coverage step carries
   `continue-on-error: true`. A default-on feature that no blocking gate ever
   exercises with it off is an untested configuration by construction.
3. **The failure it produces is unattributable.** A consumer who writes
   `rstest-bdd = { version = "0.6.1", default-features = false }` to shed the
   `serde`/`serde_json` weight of `diagnostics` gets
   `error: cannot find attribute 'given' in this scope` on every step in their
   suite, with no mention of a feature, and the documented fix — re-add
   `rstest-bdd-macros` — is the exact thing the new guide told them to delete.
4. **It makes the prelude's contents vary by build,** which defeats the
   roadmap's stated goal of "one predictable module".
   `rstest_bdd::prelude::given` would mean different things in different
   configurations.

`rstest` itself settled this the same way: `rstest_macros` is a mandatory,
ungated dependency of `rstest`, re-exported unconditionally
(`pub use rstest_macros::fixture;`). `cucumber`, the closest competitor, does
gate its codegen crate, but puts it in `default` — and pays the
`--no-default-features` hazard that item 3 describes. The unconditional edge is
simpler and strictly safer.

The build-cost objection is weak here: `crates/rstest-bdd/Cargo.toml:19-38`
already pulls `tokio`, `i18n-embed`, `rust-embed`, `fluent`, `gherkin`,
`regex`, and `inventory` unconditionally, and `syn`, `quote`, and `proc-macro2`
are already resolved in `rstest-bdd`'s graph via its existing derive-using
dependencies. The marginal additions are the macro crate itself plus `walkdir`,
`cap-std`, `camino`, `convert_case`, `newt-hype`, and `proc-macro-error3`.

Two crates pay that cost without using the macros: `cargo-bdd`
(`crates/cargo-bdd/Cargo.toml:20`, a published binary) and
`rstest-bdd-harness-gpui` (`crates/rstest-bdd-harness-gpui/Cargo.toml:27`).
Neither can opt out under an unconditional edge. This is an accepted cost,
recorded in `Risks`. Note that `rstest-bdd-harness-gpui` uses **exactly one**
item from the runtime crate — `rstest_bdd::panic_message`, at
`crates/rstest-bdd-harness-gpui/src/gpui_harness/mod.rs:40`. Relocating that
helper into `rstest-bdd-harness` would sever the edge entirely and dissolve the
cost. That is a worthwhile follow-up but it is not in scope here; it is filed
in "Follow-up work this plan deliberately does not do".

### What goes in the prelude

Founding membership is thirteen namespace bindings across eleven distinct
spellings. Every binding is either named by the roadmap or required to make a
named item usable. `ScenarioState` and `StepArgs` each occupy both the type and
macro namespaces.

| Item            | Kind                | Source                     | Why                                                               |
| --------------- | ------------------- | -------------------------- | ----------------------------------------------------------------- |
| `given`         | attribute macro     | `rstest-bdd-macros`        | used by all four examples                                         |
| `when`          | attribute macro     | `rstest-bdd-macros`        | used by all four examples                                         |
| `then`          | attribute macro     | `rstest-bdd-macros`        | used by all four examples                                         |
| `scenario`      | attribute macro     | `rstest-bdd-macros`        | used by all four examples                                         |
| `scenarios`     | function-like macro | `rstest-bdd-macros`        | the bulk-binding counterpart to `scenario`                        |
| `ScenarioState` | derive macro        | `rstest-bdd-macros`        | named by the roadmap                                              |
| `ScenarioState` | trait               | `rstest-bdd` (`state`)     | named by the roadmap; supplies `.reset()`                         |
| `Slot`          | struct              | `rstest-bdd` (`state`)     | named by the roadmap                                              |
| `StepResult`    | type alias          | `rstest-bdd`               | named by the roadmap                                              |
| `StepError`     | enum                | `rstest-bdd`               | `StepResult`'s default error parameter; unusable without it       |
| `StepContext`   | struct              | `rstest-bdd` (`context`)   | carries the five harness-context helper methods the roadmap names |
| `StepArgs`      | derive macro        | `rstest-bdd-macros`        | paired with the trait below                                       |
| `StepArgs`      | trait               | `rstest-bdd` (`step_args`) | named by the derive; see the pairing rule below                   |

*Table: founding membership of `rstest_bdd::prelude`.*

**The pairing rule.** Where a name exists as both a trait and a derive macro,
the prelude ships both halves or neither. Shipping only the derive produces a
user who can write `#[derive(StepArgs)]` and then cannot name the trait they
just derived, and the resulting "no method named …" error does not point at the
prelude. This rule decides two cases: `ScenarioState` and `StepArgs` ship both;
`DataTable` and `DataTableRow` ship **neither**, because their companion types
`Rows<T>` and `DataTableError` live in `rstest_bdd::datatable` and the prelude
would otherwise give a user a derive with no way to write the parameter type.
The whole datatable story is deferred rather than shipped in half.

**Deliberate exclusions**, each of which someone will otherwise "fix" later:

- `RSTEST_BDD_HARNESS_CONTEXT_FIXTURE`. Roadmap 11.2.1 shipped
  `#[harness_context]` precisely so that users stop naming the reserved fixture
  key, and `docs/users-guide.md:180-183` tells readers to prefer the marker
  because it "names intent rather than the internal fixture key". Putting the
  constant one glob away from every user re-advertises what the immediately
  preceding roadmap item spent an entire ExecPlan hiding. It appears in no
  example.
- `FixtureBorrowError`. It appears only in the low-level plumbing sample at
  `docs/users-guide.md:804`, in a section whose own prose describes it as
  "custom step-execution plumbing outside the usual macros". Generic name,
  named by users approximately never.
- The reporting and skip-assertion macros (`skip!`, `assert_step_ok!`,
  `assert_step_err!`, the two skip assertions, and the `reporting` module).
  These are diagnostics surfaces, not step-authoring surfaces, and no example
  uses them. There is also a mechanical trap worth recording so that a later
  contributor does not assume they arrive for free: `#[macro_export]` places a
  macro at the **crate root** regardless of the module it is written in — the
  module documentation at `crates/rstest-bdd/src/macros.rs:4` says so in as
  many words — so a glob of `rstest_bdd::prelude` does not pick them up. Adding
  them later means an explicit `pub use crate::skip;` in `prelude.rs`, not a
  reorganization.
- Harness traits (`HarnessAdapter`, `AttributePolicy`, `ScenarioRunRequest`).
  These live in `rstest-bdd-harness`, which adapter authors must depend on
  directly — `docs/users-guide.md:942` already states that rule. Re-exporting
  them would also endanger the path-based policy recognition documented at
  `docs/users-guide.md:929-942`.

**The E0252 trap.** `crates/rstest-bdd/src/lib.rs:111` already binds
`ScenarioState` at the crate root in the type namespace. Once the root also
re-exports the derive, the root holds both namespaces under that name. The
prelude must therefore import the trait from its **defining module**,
`crate::state::ScenarioState`, not from `crate::ScenarioState`; importing both
halves from `crate::` produces
`error[E0252]: the name 'ScenarioState' is defined multiple times`. The same
applies to `StepArgs`, whose trait is bound at
`crates/rstest-bdd/src/lib.rs:112` and whose defining module is
`crate::step_args`.

### The marker attribute, and why it is not exported

Roadmap 11.2.2 asks the prelude to expose "marker attributes from 11.2.1". As
established in "Context and orientation", `#[harness_context]` is inert syntax
with no item behind it, so there is nothing to re-export. The review considered
promoting it to a real `#[proc_macro_attribute]` for discoverability and
rejected that decisively: the Rust Reference restricts attribute macros to
items, items in `extern` blocks, inherent and trait implementations, and trait
definitions. A function **parameter** is none of those, so a real attribute
macro could never legally be invoked where users write the marker. It would
publish a rustdoc page for something inapplicable everywhere a user would put
it, and degrade today's `cannot find attribute 'harness_context' in this scope`
into `expected non-macro attribute, found attribute macro`.

`rstest` settles this too: `rstest_macros` declares exactly two
`#[proc_macro_attribute]` items, `fixture` and `rstest`. Its `#[case]`,
`#[from]`, `#[with]`, `#[values]`, `#[future]`, `#[once]`, and `#[default]`
markers have no items at all and are documented inside the parent macro's
rustdoc. This repository is already following that precedent and should
continue to.

The obligation is therefore discharged as documentation, in the place where a
reader actually looks: the rustdoc of `given`, `when`, and `then`
(`crates/rstest-bdd-macros/src/lib.rs:71,95,119`), which is what an editor
surfaces on hover. A note in the prelude's module documentation and a row in
the users-guide table reading "no import required" cover the remaining
discovery paths.

### The restated finish line

The roadmap's finish line — *"compile tests prove examples import only the
prelude plus their harness crate"* — cannot be met as literally worded, and not
because of anything this plan does. Five separate dependencies are structurally
unavoidable:

1. **`rstest`.** `rstest_macros` resolves its own runtime path with
   `proc_macro_crate::crate_name("rstest").expect("rstest is present in`Cargo
   .toml`qed")`. If `rstest` is absent from the *consuming* manifest,
   `#[fixture]` panics during macro expansion. Re-exporting `rstest::fixture`
   from the prelude would therefore not remove the manifest entry; it would
   only add `rstest` as a public dependency of `rstest-bdd`, making every `0.x`
   bump of a pre-1.0 crate a breaking change here.
2. **`rstest`, again, from the other direction.** Generated code emits
   `#[rstest::rstest]` as a bare relative path
   (`crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs:160`), not
   through `proc_macro_crate`. `rstest` is already a de facto public dependency
   of this framework.
3. **`tokio` and `gpui`.** The same function emits
   `#[tokio::test(flavor = "current_thread")]` at line 161 and `#[gpui::test]`
   at line 164, also as bare paths. `examples/tokio-reminders` and
   `examples/gpui-counter` can never drop those.
4. **`rstest-bdd-test-macros`.** Every example carries
   `#[rstest_bdd_test_macros::allow_fixture_expansion_lints]`, fully qualified,
   in its test file and in three cases in `src/` too. It exists to silence
   `unused_braces` from `rstest`'s own fixture expansion under `-D warnings`,
   and the crate is `publish = false`.
5. **`rstest-bdd-harness`.** `examples/todo-cli/tests/cli.rs:7` uses
   `rstest_bdd_harness::binary_test_support::{BinaryName, locate_or_build_binary}`.

So the property is false today and would remain false under any prelude. The
achievable property — which is the one that actually matters, and which *is*
compiler-enforced — is:

> No example crate names any `rstest-bdd-*` crate that the `rstest-bdd` facade
> re-exports. Concretely, `rstest-bdd-macros` appears in no example manifest and
> in no example source file.

This plan amends `docs/roadmap.md:1038-1043` to say that, in the same change
that delivers it. An unachievable finish line does not stay unachieved; it gets
quietly fudged, and the next reader inherits a claim the evidence contradicts.

### How drift is prevented

"Docs list the exported items" needs a gate, because a list in prose next to a
list in code is the definition of a drift hazard — and this repository has
already shipped fifteen wrong import lines across three READMEs without
noticing.

The review rejected an `insta` snapshot of the prelude's contents. Rust has no
reflection over module items, so there is nothing for `insta` to serialize; the
snapshot would capture a handwritten list, which is precisely the artefact that
drifts. `cargo insta accept` then makes agreeing with a mistake a single
keystroke.

Three layers replace it, each catching what the others miss:

1. **`scripts/check_prelude_exports.py`**, wired into `make lint` beside the
   existing structural checks. It parses the `pub use` leaf names out of
   `crates/rstest-bdd/src/prelude.rs`, parses the item table from the new
   users-guide section, and fails on a mismatch in either direction. It also
   fails if any of the three READMEs names an item in a `use rstest_bdd::{…}`
   line that the crate root does not export — the rule that would have caught
   the existing breakage. This mirrors `scripts/check_gpui_mapping_table.py` and
   `scripts/check_serial_nextest_matrix.py`, which are the same program shape
   (find heading, parse table, compare sets) and which the maintainer has
   already reviewed twice.
2. **A doctest in `prelude.rs`** that names every exported item explicitly after
   a glob import. This catches renames and removals for free under `make test`,
   which the script alone does not do usefully, and doubles as the worked
   example.
3. **An `EnforcedRegion` entry** for the new users-guide section in
   `crates/rstest-bdd/tests/documentation_examples/tests/mod.rs`, so that the
   fenced code in that section must be a marked, compiled example. The enforced
   set is currently exactly one pair,
   `("docs/users-guide.md", "Feature file rebuild invalidation")`; everything
   else in the 2,800-line guide is unverified prose. The prelude section is
   exactly the kind of content that must not join that category.

### The admission bar

A prelude's real cost is not its file. It is the recurring judgement call, on
every future pull request that adds a public item, of whether the item belongs
in the prelude — a call that defaults to "yes" when no bar is written down,
because adding is easy and refusing requires an argument. Six releases later
the prelude is a second crate root and the users-guide table is stale.

This plan writes the bar into `docs/developers-guide.md`:

> An item enters the prelude only if a step signature, a `#[derive]`, or an
> attribute in `examples/*/tests/*.rs` needs it, or if it is required to make
> such an item usable (a trait whose derive is exported, or an error type named
> in an exported alias). The prelude never introduces a name that is not also
> reachable at the `rstest_bdd` crate root. Removing or renaming a prelude item
> is a breaking change. Additions are permitted within a minor line but must be
> recorded in `docs/CHANGELOG.md` and noted as potentially requiring
> disambiguation for glob importers.

The founding thirteen bindings are grandfathered by the roadmap, which names
`StepResult`, `Slot`, and `ScenarioState` directly even though no example uses
them yet; the bar governs everything after.

`prelude::v1`-style versioning was considered and rejected. `std::prelude::v1`
exists because `std` can never break anything; this crate is at `0.6.0` with a
documented policy permitting breaks at the next minor. Versioning buys
stability the project already has more cheaply, at the cost of a second module,
doubled rustdoc, and a migration story for a `v2` nobody can yet specify.

The Cargo Book classifies adding a public item as a **minor** change while
noting it "may cause compile-time errors due to glob imports", and adds that
"conventionally glob imports are a known forwards-compatibility hazard". The
bar plus the changelog note discharges that obligation proportionately.

## Conformance basis

There is no Terms of Reference document in this repository. The upstream
artefacts are:

- `docs/roadmap.md:1038-1043` — roadmap entry 11.2.2, the requirement of record.
  Referred to below as **RM-11.2.2**.
- `docs/roadmap.md:995-999` — the phase 11 preamble committing the v0.6.1 line
  to semver compatibility. **RM-P11**.
- `docs/rstest-bdd-design.md` §2.7.6.4, lines 2182-2196 — records "a prelude for
  common integration imports" as an unshipped v0.6.1 candidate. **DD-2.7.6.4**.
- `docs/adr-004-policy-crate.md` — the layering decision this plan amends.
  **ADR-004**.
- `docs/adr-007-harness-context-injection.md` and its 11.2.1 addendum — the
  reserved fixture key and the `#[harness_context]` marker. **ADR-007**.
- `docs/developers-guide.md:368-393` — the workspace dependency policy.
  **DG-DEP**.

This plan proposes one new decision record, **ADR-027**, covering the facade
dependency edge, the prelude admission bar, and the deliberate non-export of
`rstest`. Numbering note: when this plan was drafted the existing set ran to
023, because `005` is duplicated (`adr-005-async-step-functions.md` and
`adr-005-harness-adapter-crates-for-framework-specific-test-integration.md`,
indexed in `docs/contents.md` as "ADR 005a"), so 024 looked like the next free
number. Two records landed on the base branch while this plan waited:
[ADR-024](adr-024-structural-usage-tracking-in-registry-lookups.md) and
[ADR-025](adr-025-adopt-ortho-config-for-the-cargo-bdd-cli.md). ADR-027 is
therefore the number this plan's record takes, and every reference below uses
it.

Trace links:

```plaintext
RM-11.2.2 -> ADR-027 -> EP-M2 -> crates/rstest-bdd/tests/prelude_surface.rs
RM-11.2.2 -> ADR-027 -> EP-M4 -> examples/*/tests/*.rs compile without rstest-bdd-macros
RM-11.2.2 -> EP-M5 -> scripts/check_prelude_exports.py in `make lint`
RM-P11    -> EP-M1 -> `make publish-check` green; no public item removed
DD-2.7.6.4 -> EP-M6 -> docs/rstest-bdd-design.md §2.7.6.4 marks the prelude delivered
ADR-004   -> ADR-027 -> EP-M1 -> docs/developers-guide.md states both edge directions
ADR-007   -> EP-M3 -> given/when/then rustdoc documents #[harness_context]
```

**Accepted deviation (2026-09-18).** RM-11.2.2's finish line is not achievable
as literally worded, for the five structural reasons set out in "The restated
finish line". This plan amends the roadmap entry to a property that can
actually be enforced. The maintainer accepted the amendment on 2026-09-18; the
verbatim replacement text is fixed in DL-9 and applied by EP-M7, so the edit is
mechanical rather than a judgement call at the end of the work.

The user approved this plan on 2026-10-25. Implementation may proceed within
the accepted constraints and tolerances below.

## Constraints

Hard invariants. Violating one requires escalation, not a workaround.

- **Semver compatibility (RM-P11).** v0.6.1 must remain compatible with v0.6.0.
  Do not remove, rename, or change the signature of any existing public item.
  Every change in this plan is additive except the example manifests, which are
  `publish = false`.
- **No proc-macro dependency cycle.** `crates/rstest-bdd-macros` must never
  depend on `crates/rstest-bdd`, in any dependency table including
  dev-dependencies. `cargo package` resolves dev-dependencies, so a
  dev-dependency cycle blocks publication even when the runtime graph is
  acyclic (DG-DEP, `docs/developers-guide.md:378-385`). The new edge points the
  other way and is legal; the reverse remains forbidden and this plan adds a
  test asserting it.
- **`diagnostics` must remain in `default`.**
  `crates/rstest-bdd/Cargo.toml:63-71` already declares `[features]` with
  `default = ["diagnostics"]`, and `[[test]] dump_registry` at lines 73-76
  carries `required-features = ["diagnostics"]`. Any edit to that manifest must
  leave the default set intact. This is called out as a hard constraint because
  **no gate would catch its removal**: `Makefile:28` sets
  `CARGO_FLAGS ?= --workspace --all-targets --all-features`, so every workspace
  gate turns `diagnostics` on regardless of what `default` says, and a
  semver-visible regression would ship silently.
- **No `doc(cfg)`.** `Makefile:27` sets
  `RUSTDOC_FLAGS ?= --cfg docsrs -D warnings` unconditionally and
  `rust-toolchain.toml` pins `channel = "stable"`. The `doc_cfg` feature is
  nightly-only. Do not introduce `#[cfg_attr(docsrs, doc(cfg(...)))]` or
  `#![cfg_attr(docsrs, feature(doc_cfg))]` anywhere; both fail `make lint` with
  E0658.
- **No intra-doc link to `#[harness_context]`.** `Cargo.toml:122-123` sets
  `broken_intra_doc_links = "deny"` and `private_intra_doc_links = "deny"`, and
  `make lint` runs `cargo doc`. The marker has no item, so prose must use a
  plain code span, never a `[...]` link.
- **400-line file limit.** No Rust source file may exceed 400 lines, enforced by
  `scripts/check_rs_file_lengths.py` inside `make lint`. Do not add an entry to
  `scripts/rs-length-allowlist.txt`; its header records that every previously
  listed file has been decomposed.
- **Directory-based module roots.** A module with children lives in
  `<name>/mod.rs`, not `<name>.rs` beside a `<name>/` directory.
- **Do not edit `typos.toml` by hand.** It is generated by
  `scripts/generate_typos_config.py`.
- **en-GB-oxendict spelling** in all prose, comments, and documentation.
  Code identifiers keep upstream spelling.
- **Markdown wrapping:** paragraphs and bullets at 80 columns, fenced code at
  120 columns, tables and headings unwrapped.
- **No `unsafe`.** The workspace forbids it.
- **No `.expect()` or `.unwrap()` in production code.** Whitaker's
  `no_expect_outside_tests` and `no_unwrap_or_else_panic` enforce this in
  `make lint`. The exemption for tests does not reach helper functions outside
  `#[cfg(test)]` or `#[test]`.
- **The prelude is a strict subset of the crate root.** No name may be reachable
  through `rstest_bdd::prelude` that is not also reachable at `rstest_bdd::`.

## Tolerances (exception triggers)

Stop and escalate — do not improvise — when any of these is reached.

- **Scope:** more than 50 files changed, or more than 1,200 net added lines
  across the whole plan.
- **Interface:** any change to an existing public function or type signature.
  Adding new items and adding the `rstest-bdd -> rstest-bdd-macros` edge are
  pre-authorized; nothing else is.
- **Dependencies:** adding any workspace dependency beyond the
  `rstest-bdd -> rstest-bdd-macros` edge this plan authorizes. In particular,
  do not add `syn` to `rstest-bdd`'s dev-dependencies — the drift gate is a
  Python script by design, precisely to avoid that.
- **Build cost:** if `cargo build -p rstest-bdd` cold wall time exceeds 25
  seconds on the reference 24-core machine, or the normal-dependency count
  exceeds 135 crates, stop and escalate. On other hosts, record the CPU
  allocation and compare only measurements from the same host. The measured
  reference baseline is 10.09 s and 99 crates; the predicted post-change
  figures are ~19.9 s and 129 crates. See `Risks` R-2.
- **Gate churn:** if `make lint` reports Whitaker findings that cannot be fixed
  without restructuring code outside this plan's scope, stop and escalate.
- **Iterations:** a milestone's tests still failing after four attempts.
- **Roadmap amendment:** the finish-line amendment (DL-9) is accepted and its
  replacement text is fixed. If implementation evidence shows that even the
  amended property cannot be met — for example, if an example turns out to need
  `rstest-bdd-macros` for something the facade cannot re-export — stop and
  escalate rather than weakening the wording again.
- **Ambiguity:** if the unconditional-versus-feature-gated decision (DL-2) is
  overruled at approval, stop and re-plan Milestones 1 and 4 before proceeding;
  a feature gate changes the CI feature lists, the docs.rs story, and the
  verification plan.

## Risks

- **R-1 — The `macros` gating decision is contested.**
  Severity: medium. Likelihood: medium. The design review split. Buzzy Bee
  measured the cost of the unconditional edge and argued for a default-off
  feature; Doggylump argued the feature's failure modes are worse than the
  cost; the roadmap's "predictable" wording and the unavailability of
  `doc(cfg)` on stable broke the tie toward unconditional. Mitigation: the
  decision is isolated to `crates/rstest-bdd/Cargo.toml` and the two `pub use`
  blocks, so reversing it is a small diff. DL-2 records the full argument and
  the numbers so the approver can overrule on the evidence rather than
  re-derive it.

- **R-2 — Build-cost regression for non-test consumers.**
  Severity: low. Likelihood: high (it is a certainty, the question is size).
  Measured: `rstest-bdd` gains 30 normal-dependency crates (99 → 129), about 40
  CPU-seconds, and roughly doubles cold wall time (10.09 s → 19.9 s) on a
  24-core machine. The new crates are the `cap-std`/`rustix` sandbox and the
  `cargo_metadata`/`toml_edit` manifest reader; `syn`, `quote`, `proc-macro2`,
  `gherkin`, `regex`, and `walkdir` were already present. External users who
  depend on `rstest-bdd` from `[dev-dependencies]` alongside
  `rstest-bdd-macros` pay **nothing**, because they already build the macro
  crate. The two payers are `cargo-bdd` and `rstest-bdd-harness-gpui`; for GPUI
  users the delta is +25 crates against a 787-package graph, about 3%.
  Mitigation: accepted, recorded in ADR-027's Known Risks, with two follow-ups
  filed that would remove most of it (see "Follow-up work").

- **R-3 — `proc-macro2/span-locations` becomes unconditional downstream.**
  Severity: low. Likelihood: high. `crates/rstest-bdd-macros/Cargo.toml:42` pins
  `proc-macro2 = { features = ["span-locations"] }`. Under the new edge that
  feature unifies for every consumer of `rstest-bdd`, changing `-C metadata` for
  `serde_derive`, `thiserror-impl`, `derive_more`, and every other derive in
  the graph, and making all expansion pay per-token line/column tracking.
  Mitigation: none available without changing the macro crate's needs. Recorded
  so it is not rediscovered as a mystery rebuild. Note that today the feature
  flips on and off depending on whether the macro crate is in the build, which
  causes measurable rebuild thrash; after this change it is stably on, which is
  arguably an improvement.

- **R-4 — Seven committed lockfiles go stale.**
  Severity: medium. Likelihood: high. Cargo lockfiles record per-package
  dependency lists, so adding a normal dependency invalidates every lock that
  mentions `rstest-bdd`. All seven do: the root `Cargo.lock`,
  `crates/rstest-bdd/tests/ui_lints/`, `tests/fixtures/published-gpui-e2e/`,
  `tests/fixtures/published-gpui-0-2-2/`,
  `crates/rstest-bdd/tests/fixtures/feature_addition/`,
  `crates/rstest-bdd/tests/fixtures/rebuild_invalidation/`, and
  `crates/cargo-bdd/tests/fixtures/minimal/`. `make check-fixture-lockfiles`
  will fail until each is refreshed. Mitigation: Milestone 1 refreshes all
  seven as its final step and runs `make check-fixture-lockfiles` before
  committing. The *package set* is unchanged — all 30 crates already appear in
  every fixture lock via the dev-dependency path — so
  `make prefetch-fixture-deps` downloads nothing new and the offline mutation
  lane keeps working.

- **R-5 — The `-C metadata` twin-fixture invariant.**
  Severity: medium. Likelihood: low.
  `crates/rstest-bdd/tests/fixtures/rebuild_invalidation/Cargo.toml:6-19`
  documents a hard requirement that its dependency set stay byte-identical to
  `crates/cargo-bdd/tests/fixtures/minimal` so the two share compiled units,
  "doubling the cold-build cost on CI" otherwise. Both take `rstest-bdd` with
  implicit default features, so an unconditional edge keeps them matched — but
  the invariant is now more fragile. Mitigation: add a comment to both
  manifests naming the new edge, so a future contributor adding
  `default-features = false` to one sees the warning.

- **R-6 — The nested `cargo`-spawning test lane has only 120 seconds of slack.**
  Severity: medium. Likelihood: medium. `.config/nextest.toml:5-20` records
  explicit budget arithmetic — `180 + 600×4 + 600×3 = 4380 s` against a
  75-minute global timeout. Those fixtures shell out to
  `cargo test --locked --offline`, and each such build now serializes the macro
  crate before the runtime crate. Scaled to the 2-vCPU CI runner that is
  roughly +19 s per cold invocation across about ten invocations — potentially
  +190 s against 120 s of slack on a cold or evicted sccache lane. No new
  compilation units are involved; the fixtures already build the macro crate
  through their own dev-dependencies. Mitigation: Milestone 1 measures a cold
  nested-fixture build before and after and records both figures. If the margin
  is breached, raise `global-timeout` in the same commit and say why in the
  nextest.toml comment block that already documents the arithmetic. DL-23
  records six alternating paired builds; the measured compile-work delta is
  +2.237 s per cold invocation, so the margin was not breached and no bound
  changed.

- **R-7 — The mutation lane multiplies R-6.**
  Severity: low. Likelihood: medium.
  `.github/workflows/mutation-testing.yml:47` runs
  `--all-features --test-workspace=true`, so the nested fixture builds rerun
  per mutant. Mitigation: watch the first post-merge mutation run; no
  pre-emptive action.

- **R-8 — E0252 when wiring the prelude.**
  Severity: low. Likelihood: medium. Importing both halves of `ScenarioState` or
  `StepArgs` from `crate::` rather than from their defining modules produces
  "the name is defined multiple times". Mitigation: stated explicitly in "The
  E0252 trap" and encoded in Milestone 2's red test. The tempting wrong fix is
  renaming the derive, which would be a breaking change; do not.

- **R-9 — Downstream glob ambiguity as the prelude grows.**
  Severity: low. Likelihood: low. Per the Cargo Book, adding a public item is a
  minor change that can nonetheless break glob importers. The exposure is a
  user who writes `use rstest_bdd::prelude::*;` alongside another glob binding
  the same name and then uses it; the error (E0659) fires at the use site, not
  the import. Mitigation: the written admission bar, the changelog requirement,
  and the subset invariant. No `prelude::v1`; see "The admission bar" for why.

- **R-11 — `rstest` is required under exactly that name, unchecked.**
  Severity: low. Likelihood: low, but the prelude increases the population
  exposed to it. `DefaultAttributePolicy` renders the test attribute from a
  bare string literal: `crates/rstest-bdd-harness/src/policy.rs:89` declares
  `const DEFAULT_TEST_ATTRIBUTES: [TestAttribute; 1] = [TestAttribute::new("rstest::rstest")];`
  and the harness crate does not use `proc_macro_crate` anywhere. Unlike every
  first-party path, this one is never resolved against the consumer's manifest.
  So `rstest-bdd` carries an unwritten, unchecked requirement that the consumer
  declare a compatible `rstest` under exactly that name: a renamed dependency
  breaks with a bare `unresolved import rstest` from generated code the user
  never wrote. This predates the plan, but a prelude that tells users "one
  import is enough" makes it likelier they arrive here confused. Mitigation:
  EP-M6 states the supported `rstest` range in the users guide and in ADR-027's
  Known Risks. EP-M4's fixture cannot inherit the requirement:
  `crates/rstest-bdd/tests/fixtures/prelude_only` is a standalone workspace, so
  its manifest is read the way a consumer's is and `rstest.workspace = true`
  has no `[workspace.dependencies]` to resolve against. It therefore states the
  literal, and
  `prelude_only_fixture_takes_its_rstest_requirement_from_the_workspace`
  asserts that literal equals the workspace's own requirement, so a workspace
  bump that forgets the fixture fails a test rather than leaving two literals
  to drift. Resolving the path through `proc_macro_crate` so that a renamed or
  missing `rstest` yields an actionable diagnostic is the right fix and is
  filed as follow-up 8; it is behavioural change in the harness crate and does
  not belong in a v0.6.1 additive item.

- **R-10 — The examples become the only prelude consumers, losing coverage of
  the direct-import path.** Severity: low. Likelihood: medium. After Milestone
  4 every example imports through the facade, so no user-vantage crate exercises
  `use rstest_bdd_macros::…` directly. Mitigation: Milestone 4 adds a fixture
  crate under `crates/rstest-bdd/tests/fixtures/` that imports the macros
  directly, keeping that entry point covered without weakening the examples.

## Verification plan

This change is a re-export and packaging change. It introduces no arithmetic,
no concurrency, no state machine, and no algorithm over a generated domain.
There is therefore **no invariant warranting property testing, bounded model
checking, or deductive proof**, and this plan deliberately proposes none.
Saying so explicitly is a requirement of the ExecPlan convention; the
justification follows, obligation by obligation.

What the change does introduce is a set of **structural propositions about the
build graph and the public surface** — statements that are either true or false
of the repository at a point in time, with no input domain to quantify over.
The right instrument for those is a compile-time or manifest-time assertion,
not a generator.

### Obligations

**V-1 — Acyclicity.** `rstest-bdd-macros` depends on `rstest-bdd` in no
dependency table.

- Method: manifest assertion, executed as a Rust test.
- Rationale: this is a single predicate over one file. A property test would
  generate nothing meaningful; the domain has one element.
- Artefact: `crates/rstest-bdd/tests/prelude_surface.rs`, test
  `macros_crate_does_not_depend_on_runtime`. It parses
  `crates/rstest-bdd-macros/Cargo.toml` with the `toml` crate (already a
  dev-dependency of `rstest-bdd`, `Cargo.toml:60`) and asserts `rstest-bdd`
  appears in none of `dependencies`, `dev-dependencies`, `build-dependencies`.
- Evidence: fails if the key is added; passes now.
- Non-vacuity: the test must also assert that the manifest parsed and that
  `rstest-bdd-patterns` *is* found in `dependencies`. Without that positive
  control, a typo in the path or a parse returning an empty table would make
  the assertion pass for the wrong reason. This is the mutation the test must
  reject.

**V-2 — The facade resolves for a consumer that does not name the macro
crate.** A crate declaring only `rstest-bdd` (plus `rstest`) can write a
complete scenario through `rstest_bdd::prelude` and run it.

- Method: a compiled fixture crate, built and executed by the test suite.
- Rationale: this is the load-bearing behavioural claim of the whole item, and
  it depends on `proc_macro_crate::crate_name` reading the *consuming*
  manifest. Only a real separate crate with a real manifest exercises that; an
  in-crate test hits `FoundCrate::Itself` and proves nothing about the case
  users are in.
- Artefact: `crates/rstest-bdd/tests/fixtures/prelude_only/` — manifest,
  one `.feature` file, one test file importing `rstest_bdd::prelude::*`.
- Evidence: before Milestone 2 the fixture fails to compile with
  `cannot find attribute 'given' in this scope`. After, it compiles and the
  scenario passes.
- Non-vacuity: the fixture's manifest must be asserted to contain no
  `rstest-bdd-macros` entry, by the same test that builds it. Otherwise a
  well-meaning lockfile refresh could add the dependency and the fixture would
  pass while proving nothing. The negative control is the sibling fixture in
  V-5.

**V-3 — The prelude is a strict subset of the crate root, and every advertised
item resolves.**

- Method: an exhaustive compile-time naming test — the finite-partition case,
  where the partition is the thirteen founding namespace bindings and
  enumeration is not merely practical but total.
- Rationale: the set is small, closed, and fully known. Enumerating it is
  strictly stronger than sampling it.
- Artefact: a doctest in `crates/rstest-bdd/src/prelude.rs` that globs the
  prelude and then names each item in a way that forces resolution — a
  `let _: fn(...)` binding for types, an actual `#[derive]` and `#[given]` use
  for macros. Plus a sibling assertion that each name also resolves at
  `rstest_bdd::`.
- Evidence: `make test` runs doctests; a removal or rename fails it.
- Non-vacuity: merely importing a name does not prove it resolves to anything
  useful — an unused import is only a warning. Each item must be *used*, not
  just imported. The mutation this must reject is deleting one `pub use` line
  from `prelude.rs`; verify during Milestone 5 that doing so actually reddens
  the doctest, and record the transcript.

**V-4 — The documented item list matches the code.**

- Method: a source-and-prose comparator script, matching the five existing
  structural checks in `make lint`.
- Rationale: the proposition relates a Markdown table to a Rust file. No Rust
  test can observe a Markdown table, and no Rust test can enumerate a module's
  items without reflection the language does not have. A script is the only
  instrument that sees both sides.
- Artefact: `scripts/check_prelude_exports.py`, with
  `scripts/tests/test_check_prelude_exports.py`.
- Evidence: `make lint` fails when either side changes alone.
- Non-vacuity: the pytest suite must include a case where the table has an extra
  row, a case where `prelude.rs` has an extra `pub use`, and a case where both
  agree — proving the comparator can fail in both directions and can pass. A
  comparator that silently finds zero items on both sides would otherwise
  "pass". Assert a non-empty parse explicitly.

**V-5 — No example names a facade-re-exported crate.**

- Method: a manifest-and-source assertion script.
- Rationale: the compiler enforces this in one direction only. There is no
  `unused_crate_dependencies` lint configured anywhere in the workspace, so
  re-adding `rstest-bdd-macros` to an example manifest would leave the build
  green and the property silently gone. The compiler catches use without
  declaration; the script catches declaration without use.
- Artefact: `scripts/check_example_imports.py`, wired into `make lint`.
- Scope: the import allowlist applies to example source files containing at
  least one `#[scenario]`, `#[given]`, `#[when]`, or `#[then]` attribute.
  `examples/todo-cli/tests/cli.rs` is deliberately outside it: that file is an
  `assert_cmd` end-to-end driver with no BDD steps, and its
  `rstest_bdd_harness::binary_test_support` import is test-runner plumbing.
  Forcing it through the prelude would drag a binary locator into the
  framework's ergonomic surface for no user benefit. The manifest half of the
  check is unconditional — `rstest-bdd-macros` may appear in no example
  manifest at all.
- Evidence: fails if `rstest-bdd-macros` reappears in any
  `examples/*/Cargo.toml` or if any `examples/**/*.rs` names
  `rstest_bdd_macros`.
- Non-vacuity: the accompanying pytest must include a fixture directory that
  *does* violate the rule and assert the script rejects it. This doubles as the
  negative control for V-2.

### Axioms

Assumed without verification, as third-party contracts:

- `proc_macro_crate::crate_name` reads the consuming crate's manifest and
  returns `FoundCrate::Itself` for the crate under compilation. Relied on by
  V-2. The repository's own handling is at
  `crates/rstest-bdd-macros/src/codegen/mod.rs:187-204`; that handling is
  repository-owned and is exercised by V-2 against a real manifest, which is
  the faithful-boundary requirement.
- `rstest_macros` requires `rstest` in the consuming manifest and panics
  otherwise (`crate_resolver.rs:9`). Relied on by the restated finish line. Not
  verified here; it is read directly from the vendored source and quoted in
  `Decision log` DL-5.
- Rust resolves a trait and a same-named macro in separate namespaces, and a
  single glob import delivers both. Relied on by the `ScenarioState` and
  `StepArgs` pairs. **No longer an assumption:** a parallel review settled it
  empirically against the real crates, with a probe module re-exporting
  `rstest_bdd::{ScenarioState, Slot, StepContext, StepError, StepResult}` beside
  `rstest_bdd_macros::{ScenarioState, given, scenario, then, when}` and
  glob-importing it from a consumer; it compiled clean with
  `#[derive(ScenarioState)]`, a `T: ScenarioState` bound, `Slot`, `StepResult`,
  and `#[given]` all resolving. V-3 exercises it again, so a mistaken belief
  fails the build rather than escaping. EP-M2 should state this as fact and
  carry no contingency prose about renaming the derive.
- Clippy's `wildcard_imports` does not fire on a glob from a module named
  `prelude`. **Also settled empirically** by that review: a probe under
  `#![warn(clippy::pedantic)]` with `-D warnings` produced `dead_code` and
  `doc_markdown` findings but no `wildcard_imports`. EP-M0 re-runs the probe
  against this workspace before anything depends on it, because the design's
  ergonomic case for shipping a prelude module at all rests on it.
- Cargo feature unification is additive within one resolution. Relied on by R-3.

### What is deliberately not verified

No `proptest` strategy, no `kani` harness, and no `verus` proof. There is no
quantified invariant: the prelude's membership is a fixed list of thirteen
namespace bindings across eleven spellings, not a domain; the dependency edge
is a single boolean fact about one manifest; the documentation correspondence
is an equality between two finite sets that V-4 compares exhaustively.
Generating random inputs would add cost and no information, and a proof would
restate the assumption. Recording this judgement is itself the obligation;
reviewers who disagree should say so at the approval gate rather than after
implementation.

## Plan of work

Seven milestones. Each ends in a repository state that builds, passes the
gates, and is safe to stop at. Run gates sequentially at every milestone
boundary — never in parallel, because the build cache is shared and sequential
runs are what make it useful — and commit. For final merge validation, preserve
the user's requested order: `make check-fmt`, `make test`, `make typecheck`,
then `make lint`.

### Milestone 0 — orientation and green baseline

**Identifier:** EP-M0. **No code changes.**

Confirm the tree is clean and the gates pass before touching anything, so that
any later failure is attributable. Capture the two baseline measurements that
`Tolerances` and R-6 depend on:

```bash
cargo tree -p rstest-bdd --prefix none -e normal --no-dedupe \
  --format '{p}' | LC_ALL=C sort -u | wc -l
# historical reference: 99; record the count for the current base

CARGO_TARGET_DIR=target/ep-m0-baseline \
  /usr/bin/time -f '%e wall %U user %S sys' cargo build -p rstest-bdd
# record the elapsed time and available CPU allocation
# compare with ~10.1 s only on the reference 24-core machine
```

If `/usr/bin/time` is unavailable, use Bash's built-in `time` and record its
wall, user, and system values. Do not disable `RUSTC_WRAPPER` to manufacture a
cold build; record the available cache state.

Keep `RUSTC_WRAPPER` enabled so the shared compiler-admission pool accounts for
every Rust invocation. Keep ordinary target directories inside the repository;
Cargo continues using the shared default package cache. `make publish-check`
uses `TMPDIR` for both its throwaway target and test fixtures. Its server tests
need those fixture projects outside the checkout so Cargo does not discover
them beneath the workspace root. Set `TMPDIR` to an isolated user-cache
directory for that gate, never `/tmp`; this is the one target-location
exception. For the initial gates, set `CARGO_BUILD_JOBS=2` to reduce
process-spawn pressure after the previous typecheck attempt received OS error
11. This is an invocation-only limit, not a project configuration change.

Record both in `Progress`. They are the numbers the build-cost tolerance is
measured against.

The `-e normal` is load-bearing, not decoration. `cargo tree` walks
dev-dependency edges by default, so on the unmodified tree
`cargo tree -p rstest-bdd -i rstest-bdd-macros` already prints the package via
`[dev-dependencies]`, which would make a before-and-after comparison
meaningless. With `-e normal` the same query reports "nothing to print" until
EP-M1 lands. `--no-dedupe` and `--format '{p}'` keep Cargo's display-only `(*)`
marker out of the package identities counted by `sort -u`. Any edge or
acyclicity check written with `cargo tree` and without `-e normal` is vacuous;
that is why V-1 parses the manifest directly instead.

Then settle the assumption the whole ergonomic case rests on. Temporarily add a
documented `prelude` module to `crates/rstest-bdd/src/lib.rs` and
`use rstest_bdd::prelude::*;` to an existing integration test, run `make lint`,
observe it pass, and revert both. Use an inner `//!` module comment without an
outer `///` comment: Whitaker requires inner module documentation, and Clippy
rejects mixing the two styles. If `make lint` reports
`clippy::wildcard_imports` on that glob, **stop and escalate**: the choice
between setting `warn-on-all-wildcard-imports` explicitly, adding a scoped
`#[expect]`, and abandoning glob imports in favour of named prelude imports
belongs to the approver, not the implementer. Confirm the source probe is
reverted before proceeding; keep any generated configuration change separate
from feature implementation.

**Acceptance evidence:** `make check-fmt`, `make lint`, `make typecheck`,
`make test`, `make markdownlint`, and `make nixie` all green on an unmodified
code tree, with the log written to
`/tmp/baseline-rstest-bdd-$(git branch --show-current).out`. **Recovery:**
nothing to undo.

### Milestone 1 — the dependency edge and the crate-root re-export

**Identifier:** EP-M1. **Discharges:** RM-P11, part of RM-11.2.2, V-1.

This milestone makes `use rstest_bdd::{given, when, then};` compile — the thing
three READMEs have claimed for the life of the project.

1. In `crates/rstest-bdd/Cargo.toml`, move `rstest-bdd-macros` from
   `[dev-dependencies]` to `[dependencies]` as
   `rstest-bdd-macros.workspace = true`. Do **not** carry the
   `compile-time-validation` feature across: the runtime crate's own tests need
   it, so keep a `[dev-dependencies]` entry that adds only that feature. Cargo
   unifies the two.
2. Add `[package.metadata.docs.rs]` with `all-features = true`, mirroring
   `crates/rstest-bdd-macros/Cargo.toml:59-61`. Do **not** add `rustdoc-args`
   with `--cfg docsrs`, and do not use `doc(cfg)` — see `Constraints`.
3. In `crates/rstest-bdd/src/lib.rs`, beside the existing
   `pub use state::{ScenarioState, Slot};` at line 111, add the macro
   re-exports. Each needs a `///` doc comment because `missing_docs = "deny"`:

   ```rust
   pub use rstest_bdd_macros::{
       DataTable, DataTableRow, ScenarioState, StepArgs, given, scenario, scenarios, then, when,
   };
   ```

   Note this re-exports the `DataTable`/`DataTableRow` derives at the **root**
   even though they are excluded from the **prelude**. The root mirrors the
   macro crate; the prelude is the curated subset. That asymmetry is deliberate
   and is what lets the datatable story be completed later without a breaking
   change.
4. Add `crates/rstest-bdd/tests/prelude_surface.rs` with the V-1 test, including
   its positive control.
5. Add the R-5 comment to both
   `crates/rstest-bdd/tests/fixtures/rebuild_invalidation/Cargo.toml` and
   `crates/cargo-bdd/tests/fixtures/minimal/Cargo.toml`.
6. Refresh all seven lockfiles:

   ```bash
   cargo update -w
   make update-ui-lints-lock
   make update-published-gpui-0-2-2-lock
   make update-published-gpui-e2e-lock
   make update-feature-rebuild-fixtures-lock
   make check-fixture-lockfiles
   ```

   `crates/cargo-bdd/tests/fixtures/minimal/Cargo.lock` has no dedicated
   target. Refresh it directly:

   ```bash
   cargo metadata --format-version 1 \
     --manifest-path crates/cargo-bdd/tests/fixtures/minimal/Cargo.toml >/dev/null
   ```

7. Re-measure the two EP-M0 figures and compare against the tolerance.
8. Measure one cold nested-fixture build for R-6 and record it.

**Acceptance evidence:** `make check-fmt && make lint && make test` green;
`make check-fixture-lockfiles` green; `make publish-check` green, proving the
new edge has not broken release ordering; `cargo build -p rstest-bdd` cold wall
time and crate count within tolerance. **Conformance check:** no public item
removed or changed; the only new public items are re-exports;
`rstest-bdd-macros` still does not depend on `rstest-bdd` (V-1 asserts it).
**Recovery:** `git revert` the commit; the lockfiles revert with it.
**Remaining gaps:** no prelude module yet; examples unchanged. **Compatibility
decision:** none required. Every change is additive and the examples are
`publish = false`.

### Milestone 2 — the prelude module

**Identifier:** EP-M2. **Discharges:** RM-11.2.2, V-3.

Red first. Write the doctest before the module.

1. **Red.** Add `crates/rstest-bdd/src/prelude.rs` containing only the module
   documentation and its doctest, which globs the prelude and exercises all
   thirteen namespace bindings. Add `pub mod prelude;` to `lib.rs`. Run
   `cargo test -p rstest-bdd --doc prelude` and observe it fail with unresolved
   names. Record the transcript.
2. **Green.** Add the `pub use` statements. Import the trait halves from their
   **defining modules** — `crate::state::ScenarioState`,
   `crate::step_args::StepArgs` — not from `crate::`, or you get E0252 (R-8).
   Re-run the doctest.
3. **Refactor.** If the module approaches 350 lines, split it to
   `crates/rstest-bdd/src/prelude/mod.rs` now rather than later; the 400-line
   ceiling is hard and the allowlist is closed.
4. Extend `crates/rstest-bdd/tests/prelude_surface.rs` with the subset assertion
   from V-3: every prelude name also resolves at `rstest_bdd::`.

The module documentation must explain, in prose: that the prelude is a curated
subset of the crate root and never introduces a name not found there; that
`ScenarioState` and `StepArgs` each name both a trait and a derive macro, as
`serde::Serialize` does; and that `#[harness_context]` requires no import
because it is an inert parameter marker, written as a plain code span and never
as an intra-doc link (see `Constraints`).

**Acceptance evidence:** the doctest fails before step 2 and passes after;
`make check-fmt && make lint && make test` green. **Conformance check:**
prelude ⊆ root holds; no new name introduced. **Recovery:** delete `prelude.rs`
and the `pub mod` line. **Remaining gaps:** nothing consumes the prelude yet.

### Milestone 3 — the marker's documentation

**Identifier:** EP-M3. **Discharges:** ADR-007 follow-through, the RM-11.2.2
"marker attributes" clause.

1. Add a `# Parameter attributes` section to the rustdoc of `given`, `when`, and
   `then` at `crates/rstest-bdd-macros/src/lib.rs:71,95,119`, documenting
   `#[harness_context]` and stating that it needs no import. This is where an
   editor's hover lands, which is why it goes here rather than only in the
   prelude.
2. Add the missing compile-fail fixture. `crates/rstest-bdd/tests/ui_macros/`
   already covers `harness_context` with `_with_from`, `_with_datatable`,
   `_with_step_args`, `_on_placeholder`, `_duplicate`, and
   `_takes_no_arguments`. The gap is the marker written on a function that
   carries no step attribute — exactly what a user exploring the feature will
   try. Add `harness_context_without_step.rs` and pin its stderr.

**Acceptance evidence:** `make test` green with the new trybuild fixture
passing; `cargo doc` clean under `make lint`. **Conformance check:** no
behavioural change; documentation and one test only. **Recovery:** revert;
nothing depends on this milestone.

### Milestone 4 — migrate the examples

**Identifier:** EP-M4. **Discharges:** the restated RM-11.2.2 finish line, V-2,
V-5.

1. Add the V-2 fixture crate at
   `crates/rstest-bdd/tests/fixtures/prelude_only/` — a manifest naming only
   `rstest-bdd` and `rstest`, one `.feature` file, one test file using
   `use rstest_bdd::prelude::*;`. This is the proof that a downstream user
   never needs `rstest-bdd-macros`. Build and run it from a Rust test.
2. Add a sibling fixture that imports `rstest_bdd_macros` directly, preserving
   coverage of that entry point once the examples stop using it (R-10).
3. For each of `examples/todo-cli`, `examples/japanese-ledger`,
   `examples/gpui-counter`, `examples/tokio-reminders`: replace
   `use rstest_bdd_macros::{given, scenario, then, when};` with
   `use rstest_bdd::prelude::*;`, and delete
   `rstest-bdd-macros.workspace = true` from `[dev-dependencies]`.
4. Leave `use rstest::fixture;` in place in all four. It is required — see "The
   restated finish line" — and leaving it is what "without hiding the
   underlying crates" means in practice.
5. Add `scripts/check_example_imports.py` and
   `scripts/tests/test_check_example_imports.py`, and wire the script into
   `make lint` beside the existing structural checks. The compiler alone is not
   sufficient: there is no `unused_crate_dependencies` lint anywhere in the
   workspace, so nothing would stop a later contributor re-adding the manifest
   entry.

**Acceptance evidence:** all four example suites pass; the `prelude_only`
fixture compiles and its scenario passes; `grep -r rstest_bdd_macros examples/`
returns nothing; `make lint` runs the new script. **Conformance check:** the
achievable finish-line property holds and is gated in both directions.
**Recovery:** revert; the examples' previous imports are in git. **Remaining
gaps:** documentation not yet updated.

### Milestone 5 — the drift gate

**Identifier:** EP-M5. **Discharges:** V-4, the "docs list the exported items"
half of RM-11.2.2.

1. Add the "Importing `rstest-bdd`" section to `docs/users-guide.md`,
   immediately before `## Step definitions` at line 138. It contains the item
   table (item, kind, defining crate, canonical path), the same short example
   shown twice — once globbed, once fully spelled — and a row for
   `#[harness_context]` reading "no import required". `ScenarioState` and
   `StepArgs` each get **two rows**, trait and derive, so the glob is never
   opaque about what it delivered.
2. Add `scripts/check_prelude_exports.py` with its pytest suite, following the
   shape of `scripts/check_gpui_mapping_table.py` and
   `scripts/check_serial_nextest_matrix.py`. Reuse
   `scripts/markdown_references.py` for heading location. Wire it into
   `make lint`.
3. Extend the script to the READMEs: fail if `README.md`,
   `crates/rstest-bdd/README.md`, or `crates/rstest-bdd-macros/README.md` names
   an item in a `use rstest_bdd::{…}` line that the crate root does not export.
4. Register the new users-guide section as an `EnforcedRegion` in
   `crates/rstest-bdd/tests/documentation_examples/tests/mod.rs`, updating the
   `enforced_regions_list_is_what_we_think` assertion, so the section's fenced
   code must compile.
5. **Prove the gate bites.** Delete one `pub use` line from `prelude.rs` and
   confirm both the doctest and the script fail; delete one table row and
   confirm the script fails. Restore, and record both transcripts in
   `Artefacts and notes`. This is the V-3 and V-4 non-vacuity evidence and is
   not optional.

**Acceptance evidence:** `make lint` fails on a deliberately desynchronized
table and passes when synchronized; `make test` green including the enforced
region. **Conformance check:** documentation and code cannot now drift
silently. **Recovery:** revert; `make lint` returns to its previous checks.

### Milestone 6 — the remaining documentation

**Identifier:** EP-M6. **Discharges:** DD-2.7.6.4, ADR-027, DG-DEP.

1. **`README.md`, `crates/rstest-bdd/README.md`,
   `crates/rstest-bdd-macros/README.md`** — audit all five
   `use rstest_bdd::{…}` lines in each. They become true at Milestone 1; make
   sure they are *exactly* true, since the Milestone 5 gate now checks them.
2. **`docs/users-guide.md`** — migrate the eleven step-writing samples at lines
   126, 266, 435, 467, 517, 568, 626, 691, and the `scenarios!`/async blocks, to
   `use rstest_bdd::prelude::*;`. Deliberately **keep explicit** the harness
   cookbook (1039-1041, 1084-1085, 1122, 1150-1151), the harness adapter core
   API section, the published-gpui variants, the `StepContext` plumbing samples
   at 783 and 804, the manual async wrapper pattern, the datatable sections,
   and the localization and language-server sections. Rewrite lines 336-338 and
   566-568 to drop the `ScenarioState as _` idiom, which exists only because
   the trait and derive used to come from different crates. Add one sentence to
   the harness-policy section (889-942) stating that the prelude does not
   re-export harness traits and why.
3. **`docs/rstest-bdd-design.md` §2.7.6.4** — move "a prelude for common
   integration imports" out of "Remaining candidates, not yet shipped" into the
   delivered sentence, naming roadmap 11.2.2 and cross-linking ADR-027.
4. **`docs/developers-guide.md`** — amend the "Workspace dependency policy"
   section at 368-393 to state both edge directions explicitly; add the prelude
   admission bar; add a section describing `check_prelude_exports.py` and
   `check_example_imports.py`, mirroring the existing script sections.
5. **`docs/repository-layout.md`** — this is the workspace component map that
   `docs/contents.md` points at, and it presents `rstest-bdd` and
   `rstest-bdd-macros` as independent. Record the new edge.
6. **`docs/ergonomics-and-developer-experience.md`** — add the prelude to §2,
   "Reducing boilerplate in step definitions". That section is literally this
   change's subject.
7. **`docs/adr-027-*.md`** — write it, using the template at
   `docs/documentation-style-guide.md:418-425`. Cover the facade edge and its
   reconciliation with ADR-004 (whose options table at line 36 rejected an
   option on the grounds that it "ties runtime to macros"), the admission bar,
   the deliberate non-export of `rstest`, and Known Risks naming the absent
   public-API gate and the measured build cost.
8. **`docs/contents.md`** — index ADR-027.
9. **`docs/CHANGELOG.md`** — an additive entry on the v0.6.1 line.
10. Run `make markdownlint` and `make nixie`. Note that `make fmt` can itself
    introduce MD039/MD013 findings, so run the Markdown lint after formatting,
    not before.

**Acceptance evidence:** `make markdownlint`, `make nixie`, and the full gate
suite green; `make check-users-guide-links` green. **Conformance check:** every
upstream artefact in `Conformance basis` reflects the shipped state.
**Recovery:** documentation-only; revert freely.

### Milestone 7 — roadmap amendment, full gates, pull request

**Identifier:** EP-M7.

1. Replace `docs/roadmap.md:1038-1043` with the verbatim text fixed in DL-9
   under "The verbatim roadmap replacement", and tick 11.2.2 as done with a
   completion note recording what shipped and what was deliberately deferred.
   The amendment is already accepted, so this is a transcription step, not a
   decision. The replacement text was dry-run applied to `docs/roadmap.md` on
   2026-09-18 and both `make markdownlint` and `make -B spelling` passed with
   it in place, then the file was restored; the wrapping is therefore known to
   fit. Re-run both gates after transcribing anyway, since `make fmt` can
   reflow neighbouring entries in a file this size.
2. File the follow-up items listed below as roadmap entries or issues.
3. Run the complete gate suite sequentially, `tee`-ing each to
   `/tmp/$ACTION-rstest-bdd-$(git branch --show-current).out`: `make check-fmt`,
   `make lint`, `make test`, `make markdownlint`, `make nixie`,
   `make publish-check`.
4. Open the pull request.

**Acceptance evidence:** all gates green, with the log paths recorded in
`Artefacts and notes`.

## Follow-up work this plan deliberately does not do

Each of these was raised during the design review, judged out of scope, and
should be filed rather than forgotten:

1. **A public-API gate.** The repository has no `cargo-public-api` and no
   `cargo-semver-checks`. This item makes that gap materially worse by creating
   a surface whose *additions* can break glob importers. `cargo-public-api` is
   the tool that sees this hazard; `cargo-semver-checks` does not, because the
   Cargo Book classifies such additions as minor. This deserves its own roadmap
   entry. Filed as #952.
2. **Sever `rstest-bdd-harness-gpui -> rstest-bdd`.** That crate uses exactly
   one item from the runtime crate, `rstest_bdd::panic_message`
   (`crates/rstest-bdd-harness-gpui/src/gpui_harness/mod.rs:40`). Moving the
   helper into `rstest-bdd-harness` would remove the edge and eliminate most of
   R-2's cost for GPUI users. Filed as #953.
3. **Audit `cargo_metadata` in `rstest-bdd-harness`'s normal dependencies.** It
   drags `semver`, `camino`, `cargo-platform`, and `serde_json` into the macro
   crate's graph. If it is test-support-only, moving it behind a feature would
   cut a meaningful slice off R-2 independently of this work. Filed as #954.
4. **Complete the datatable story in the prelude.** Ship `DataTable` and
   `DataTableRow` together with `rstest_bdd::datatable::Rows` and
   `DataTableError`, or leave all four out. This plan leaves them out. Filed as
   #955.
5. **Make the examples copy-pasteable.** Every example carries
   `#[rstest_bdd_test_macros::allow_fixture_expansion_lints]` from an
   unpublished crate, so a user copying `examples/todo-cli/tests/todo.rs` off
   GitHub today gets
   `E0433: failed to resolve: use of undeclared crate rstest_bdd_test_macros`.
   That predates this plan but the prelude promotes the examples to the proof
   of the framework's ergonomics, which makes it more visible. Filed as #956.
6. **Add a `--no-default-features` gate.** The only lane that builds without
   default features is Windows, and its coverage step carries
   `continue-on-error: true`, so it cannot red the build. `diagnostics` remains
   a default feature after this change even though `macros` does not exist.
   Filed as #957.
7. **Enforce ADR number uniqueness.** `005` is duplicated because nothing
   checks. Filed as #958.
8. **Resolve the `rstest` attribute path through `proc_macro_crate`.**
   `crates/rstest-bdd-harness/src/policy.rs:89` hard-codes `"rstest::rstest"`,
   so a consumer who renames the dependency gets a bare
   `unresolved import rstest` from code they never wrote, and a consumer who
   omits it gets no actionable hint. Resolving it the way first-party paths are
   resolved would fix both. This is behavioural change in the harness crate and
   does not belong in a v0.6.1 additive item. See R-11. Filed as #959.

## Decision log

- **DL-23 — The CodeScene duplication finding was repaired by restructuring,
  and R-6's paired measurement was taken on this host.** Resolved on 2026-11-17
  against head `cde5c782`, whose assessment returned "substantially complete,
  but whole-plan completion remains unverified" and named a failed CodeScene
  check.

  The check failed at 9.39 on `crates/rstest-bdd/tests/prelude_surface.rs`: the
  renamed-dependency and target-specific-dependency guards had identical bodies
  and differed only in the manifest they parse. Suppressing the rule was not an
  option — the repository forbids silenced lints — and deleting either case
  would drop a regression. They became one `rstest` table, `case::renamed` and
  `case::target_specific`, each keeping its own assertion message, which is
  also what the repository's testing guidance prescribes for duplicated cases.
  `cs review` on the file then reported 10.0 with no findings, and both cases
  still fail if the guard stops walking nested tables or resolving `package`
  renames.

  R-6 asked Milestone 1 to measure a cold nested-fixture build before and after
  the edge. The plan carried a post-edge figure but no comparable baseline, so
  this round took six alternating pairs: the
  `crates/cargo-bdd/tests/fixtures/minimal` fixture, its own `target/` removed
  before each run, `cargo test --locked --offline`, at the base `7b617013` in a
  detached worktree and at `fd92a1b7` in this one. Wall time (min/median/max):
  base 26.764/43.748/143.783 s, head 18.583/40.721/83.444 s. Compile work, as
  `user` plus `sys` seconds, is the queue-independent figure: median 22.745 s
  at the base and 24.982 s at the head, a delta of +2.237 s (+9.8%) per cold
  invocation. Every run recorded `build-limits-status` first: the shared
  four-slot pool held between zero and four slots, and the wall readings track
  that contention, not the edge — the head was faster than the base in four of
  the six pairs. Scaled to the lane's roughly ten invocations, +2.2 s of
  compile work each is about +22 s against the 120 s of slack
  `.config/nextest.toml` documents, so no `global-timeout` change is warranted
  on this evidence and none was made. Two limits belong with these figures:
  they are this workstation's, where four admission slots and a six-core host
  hide ordering costs a 2-vCPU runner would expose, and the reference machine's
  25-second limit is not applied to an unlike host. CodeScene's other
  requirement — a maintainer's acceptance of the tolerance overage — is DL-16's
  escalation and remains outside this plan's authority.

  Date/Author: 2026-11-17, implementation agent.

- **DL-22 — The round-three assessment's two guard gaps were closed, and the
  example guard's manifest half moved into its own module.** Resolved on
  2026-11-17 against head `00c564ea`, whose verdict was "not yet complete
  against the whole approved plan" with two named gaps.

  The example guard matched a path into `rstest_bdd::` but not a facade alias.
  A step-bearing file could write `use rstest_bdd as bdd;`, satisfy the prelude
  half, and reach the crate root through `#[bdd::when(...)]`, which the path
  rule never saw. `FACADE_ALIAS_IMPORT` now finds `use rstest_bdd as bdd;` and
  `extern crate rstest_bdd as bdd;`, reports the import as a reach of its own,
  and reports any `<alias>::` path written through it. A declared-but-unused
  alias therefore still fails, and a used one is reported twice because it is
  two reaches. Two parameterized rows in `test_reports_a_facade_alias` assert
  both messages.

  Both dependency guards resolved a `package` rename from the declaring
  manifest alone, so a workspace alias —
  `macros = { package = "rstest-bdd-macros" }` in `[workspace.dependencies]`,
  inherited as `macros.workspace = true` — was read as its local key. The V-1
  guard in `crates/rstest-bdd/tests/prelude_surface.rs` now reads the workspace
  manifest once and resolves `workspace = true` entries through it, falling
  back to the key when no workspace entry names the entry rather than inventing
  a crate. The example guard does the same through `dependency_aliases(root)`.
  In both, `package` wins over inheritance, matching Cargo: an entry cannot
  both rename a crate and inherit it.

  That resolution and the alias rules pushed `scripts/check_example_imports.py`
  past the 400-line Python module budget the pylint gate enforces, so the
  manifest half moved to `scripts/example_dependencies.py`, with its own test
  module `scripts/tests/test_example_dependencies.py`. The split is by concern,
  not by size alone: the resolver answers a question about Cargo manifests and
  knows nothing about examples, and the example guard's own tests keep their
  fixtures.

  One portability note belongs here because it misled the first attempt. The
  new modules annotate with `Path` imported under `typing.TYPE_CHECKING`, which
  ruff requires of an annotation-only import and which works only because the
  project's floor is Python 3.14, whose annotations are lazily evaluated. The
  host's ambient `python3` is 3.12, under which the same module raises
  `NameError` at `def` time. Every gate runs these scripts through
  `$(PROJECT_PYTHON)`, which is `uv run --python 3.14 python`, so the gate is
  unaffected; the df12 lint also rejects `from __future__ import annotations`
  on this baseline, which is why the future import is absent rather than the
  `TYPE_CHECKING` import.

  Date/Author: 2026-11-17, implementation agent.

- **DL-21 — The round-two assessment's three gaps were closed by widening the
  guards and finishing the documentation conversion.** Resolved on 2026-11-16
  against head `f13a8f39`, whose verdict was "not yet complete against the
  whole approved plan". Each finding named a hole rather than a style
  preference, and each hole was one the branch's own claims made visible.

  The example guard matched text markers: a `use` line, or the bare spelling of
  an attribute. A step-bearing file could therefore write
  `#[rstest_bdd::given(...)]`, or call `rstest_bdd_macros::scenarios!(...)`,
  beside a compliant prelude import and pass. Reach is now matched by path
  (`STEP_ATTRIBUTE_PATH`, `CRATE_ROOT_PATH`, `MACROS_CRATE_PATH`) through one
  declarative `REACH_RULES` table, so the qualified attribute and the raw macro
  path are reported by the same code that reports the `use` forms they replace.
  Two negative tests cover them.

  The V-1 dependency guard read only the top-level dependency tables and only
  the key `rstest-bdd`. A renamed `package = "rstest-bdd"` entry and a
  `[target.'cfg(unix)'.dependencies]` entry both reached the runtime unseen.
  The guard now walks every nested table whose key names a dependency table and
  resolves each entry through its `package` rename, and two regression tests
  feed it inline manifests in exactly those two forms.

  The migration checklist told users to replace macro-crate imports with the
  prelude, which breaks the datatable derives: the prelude deliberately excludes
  `DataTable` and `DataTableRow` under ADR-027's entry rule. The guide now
  names three exclusions, lists the prelude's eleven names instead of linking
  to them, and routes the datatable derives to the crate root. The users' guide
  carried the same defect fifteen times: at the base it named
  `rstest_bdd_macros` in 31 places, and this branch had converted 16 of them
  while adding a sentence saying the import a user writes never names the macro
  crate. The remaining fifteen now use `rstest_bdd::prelude::*`, or
  `rstest_bdd::{DataTable, DataTableRow}` where a step receives a data table,
  so the sentence holds for the guide's own examples.

  Date/Author: 2026-11-16, implementation agent.

- **DL-20 — The CodeScene findings against the prelude gate were fixed by
  decomposition, not suppressed.** Resolved on 2026-11-16 after the hosted
  CodeScene check failed at `492b457f` on three of its gates. Every finding was
  against code this branch introduced, and each described a shape problem
  rather than a threshold: `dependency_crates` in
  `scripts/check_example_imports.py` reached five levels of conditional nesting
  across two branch bumps, `writes_step_attributes` and `names_macro_crate`
  were the same comprehension over different marker tuples, the two
  dependency-table tests differed only in a constant, and `support.rs` took
  fixture names as bare strings beside file and dependency names. The repairs
  are structural. The per-table walk moved into `dependency_names`, the two
  marker predicates collapsed into the single `is_in_scope` predicate that was
  their only caller, the two tests became one `pytest.mark.parametrize` case,
  and the fixture name became a `Fixture` newtype so a test-source file name
  can no longer be passed where a fixture is meant. All three files then score
  10.00 under `cs review`. Date/Author: 2026-11-16, implementation agent.

- **DL-19 — A compile-fail fixture proves the prelude's exclusions, so the
  guide's list is enforced by the compiler.** Resolved on 2026-11-16 while
  answering the review. The pin in DL-18 proves the guide's two prelude imports
  are the text the suite compiles, but nothing proved the complement: that an
  item the guide says the prelude leaves out really is unreachable through it.
  Without that, "the prelude omits the datatable items" was a sentence a future
  edit could falsify in silence, and a reader who ignored the guide would get a
  different, unrecorded verdict. `tests/ui_macros/prelude_omits_datatable.rs`
  imports `rstest_bdd::prelude::DataTable` and is registered in
  `run_failing_ui_tests` with a checked-in `.stderr`, so the exclusion fails to
  resolve for exactly the reason recorded. Its snapshot is trybuild's own
  preferred rendering: the fixture was compiled through the staged crate and
  then re-checked through trybuild before the `.stderr` was accepted, rather
  than written from memory of what rustc prints. Date/Author: 2026-11-16,
  implementation agent.

- **DL-18 — The example gate enforces the prelude alone, and the guide's
  imports are compiled copies.** Resolved on 2026-11-16 while answering the
  review. DL-9's accepted wording is that every example reaches its
  `rstest-bdd` surface "through `rstest_bdd::prelude` alone", but
  `scripts/check_example_imports.py` still accepted `use rstest_bdd::{…}` as a
  facade import, so a crate-root import passed a gate that the finish line
  forbids. The gate now requires the prelude marker and reports a crate-root
  import as a violation, while still accepting a braced
  `use rstest_bdd::prelude::{…}`. Separately, registering the users-guide
  section as an enforced region makes its markers mandatory but compiles
  nothing, so `prelude_examples/` now holds each import once where the compiler
  resolves it, and `prelude_examples_are_compiled_as_written` pins the guide's
  fence to that copy by text equality. Faithfulness beat convenience: the
  compiled copies had to lose their `//!` docs and lint allowance, and the
  guide's spelled example had to adopt rustfmt's own vertical layout, because
  the pin fails on any difference at all. Date/Author: 2026-11-16,
  implementation agent.

- **DL-17 — The facade re-export is completed at the crate root, not the
  prelude.** Resolved on 2026-11-16 while answering the review. ADR-027
  requires the facade's re-export to be "total and flat", and the review showed
  it was not: `step_library` remained reachable only from `rstest-bdd-macros`,
  so every consumer declaring a scoped vocabulary still named the
  implementation crate. The crate root now re-exports it, and
  `tests/prelude_surface.rs` names it so the compiler holds that. It does not
  enter the prelude: the ADR's entry rule admits an item only when a step
  signature, a `#[derive]`, or an attribute in the examples needs it, and no
  example declares a library. ADR-027 records the completion as an amendment
  rather than by editing the decision outcome, so the accepted decision and its
  later correction stay distinguishable. Date/Author: 2026-11-16,
  implementation agent.

- **DL-16 — The delivery exceeds the retained net-line tolerance; escalated,
  not self-approved.** Measured on 2026-11-16 at the first review-repair commit
  `8ba9759e`, against the merge base `7b617013` with the stacked PR's base
  `issue-901-adopt-ortho-config-v0-10-0-for-the-cargo-bdd-cli`. Including
  lockfiles and this plan, the branch changes 80 files for +9,628 and −453, a
  net of +9,175. Excluding the lockfiles and this plan, it changes 70 files for
  +4,184 and −449, a net of +3,735: 1,920 lines of Python gates and their tests
  across seven files, 1,250 lines of Rust across thirty-five, 471 lines of
  documentation across ten, 94 lines of manifests and fixture data across
  fourteen, and no net change across the four example files. DL-11 retained the
  1,200-net-added-line limit while adopting the 50-file ceiling, so the
  delivery is 2,535 net lines over a limit this agent may not waive. The
  overage is not padding: the drift gate, the example-import gate, the three
  documentation-extraction modules and their two test modules, the two fixture
  crates, and the users-guide and ADR documentation are each named by an
  approved milestone or verification obligation, and the alternative to writing
  them is shipping an unchecked re-export. Continuing is the user's direction
  under DL-11 ("continue through PR completion") and the exceeded limit is
  reported to the user in this round's summary; a reviewer who wants the
  overage formally waived should say so, and a reviewer who wants it reduced
  should say which milestone's verification to drop. Date/Author: 2026-11-16,
  implementation agent.

- **DL-15 — Keep the published GPUI fixture locks compatible with `xattr`.**
  Resolved on 2026-11-03 after PR CI run `41540661058` failed on Linux. Both
  published GPUI fixture lockfiles had moved `libc` from `0.2.189` on the
  rebased base to `0.2.191`, while retaining `xattr 0.2.3`, which refers to
  `libc::ENOATTR`. The upstream rationale for removing this obsolete Linux
  constant is in [libc issue #1356][libc-enoattr]. Keep `libc 0.2.189` in the
  documentation and E2E fixture lockfiles until their transitive `xattr`
  dependency is updated. Use Cargo's precise lockfile update rather than
  editing generated lock data. Validate `make check-published-gpui`,
  `make check-published-gpui-e2e-lock`, and `make check-fixture-lockfiles`.
  Date/Author: 2026-11-03, implementation agent.

[libc-enoattr]: <https://github.com/rust-lang/libc/issues/1356>

- **DL-14 — Correct the package-count measurement; retain the approved
  ceiling.** Resolved on 2026-11-02. The earlier command counted Cargo's
  display-only `(*)` marker as a separate row because it used `sort -u` on the
  deduplicated tree output. Re-running with `--no-dedupe --format '{p}'` and
  counting unique package identities gives 99 packages for `rstest-bdd`, 70 for
  `rstest-bdd-macros`, and 129 in their union; the macro edge adds 30. This is
  below the approved ceiling of 135, so no dependency reduction, tolerance
  change, or facade redesign is needed. Continue EP-M1 under the existing DL-2
  and DL-10 approvals. Date/Author: 2026-11-02, implementation agent, following
  the user's instruction to proceed with the corrected count.

- **DL-13 — Bound the initial gate job count after a spawn failure.** The
  previous `make typecheck` failed because the OS could not spawn a Rust build
  script (`EAGAIN`, OS error 11). Current Lody cgroup readings show no PID-cap
  or OOM event, so a persistent cgroup limit is not established. Run the
  initial Cargo-heavy gates with `CARGO_BUILD_JOBS=2` to reduce simultaneous
  process creation, while retaining the shared compiler wrapper and Cargo
  package cache. This setting is not committed to repository configuration.
  Date/Author: 2026-11-01, implementation agent.

- **DL-12 — Preserve shared compiler admission in EP-M0.** The original
  measurement command cleared `RUSTC_WRAPPER` and placed build artefacts under
  `/tmp`; both conflict with the current worktree instructions. EP-M0 now keeps
  the wrapper enabled and uses `target/ep-m0-baseline`. Record the CPU
  allocation because this host does not match the 24-core reference machine.
  Date/Author: 2026-11-01, implementation agent.

- **DL-11 — Extend the file-count tolerance for full delivery.** The initial
  30-file limit conflicts with the approved plan's estimate of at least 33
  files by EP-M4 and about 49 by EP-M7. After that conflict was presented, the
  user directed implementation to continue through PR completion. This adopts
  the offered 50-file ceiling; the 1,200-net-added-line limit remains.
  Date/Author: 2026-11-01, user direction interpreted by the implementation
  agent.

- **DL-10 — Approve the plan as written.** The user approved implementation on
  2026-10-25, including the unconditional `rstest-bdd` to `rstest-bdd-macros`
  dependency (DL-2) and the accepted RM-11.2.2 finish-line amendment (DL-9).
  The approved scope, constraints, tolerances, and verification obligations
  remain authoritative. Date/Author: 2026-10-25, user approval recorded by the
  implementation agent.

- **DL-1 — Ship both a crate-root re-export and a `prelude` module.**
  Rationale: the root re-export is required independently of the roadmap,
  because three READMEs already document
  `use rstest_bdd::{given, when, then, …}` in five places each and none of it
  compiles. The prelude module is required by RM-11.2.2's wording. Keeping the
  prelude a strict subset of the root means there is only ever one public API
  to keep coherent. Alternatives rejected: a prelude module alone (leaves the
  READMEs wrong, and creates names reachable only through `prelude::`); a root
  re-export alone (contradicts the roadmap's explicit "public prelude", and the
  reviewer who proposed it acknowledged it needs a roadmap amendment to adopt).
  Date/Author: 2026-09-14, planning agent, from the design review.

- **DL-2 — The `rstest-bdd -> rstest-bdd-macros` edge is unconditional, with no
  `macros` feature.** This is the plan's most contested decision and the one
  most worth overruling if the approver disagrees. Rationale: four independent
  defects rule out a feature gate. `doc(cfg)` is nightly-only while
  `Makefile:27` passes `--cfg docsrs -D warnings` on a stable toolchain, so a
  gated re-export cannot be annotated and would fail `make lint` with E0658. No
  blocking gate ever builds with the feature off, since `CARGO_FLAGS` is
  `--all-features` and the only `with-default-features: false` lane is Windows
  with `continue-on-error: true`. The failure mode for
  `default-features = false` consumers is `cannot find attribute 'given'` with
  no mention of a feature. And a gated prelude means different things in
  different builds, which is precisely what RM-11.2.2's "one predictable
  module" forbids. Unconditional is also the only option requiring **zero**
  edits to the two hand-maintained CI feature lists at `ci.yml:553` and
  `ci.yml:577`. `rstest` itself makes `rstest_macros` a mandatory, ungated
  dependency. Cost, measured: +30 normal-dependency crates (99 → 129), +40
  CPU-seconds, and roughly 2× cold wall time (10.09 s → 19.9 s) on a 24-core
  machine. The payers are `cargo-bdd` and `rstest-bdd-harness-gpui`; for GPUI
  users the delta is +25 crates against a 787-package graph, about 3%. External
  users who take `rstest-bdd` as a dev-dependency alongside `rstest-bdd-macros`
  pay nothing. Dissent recorded: one reviewer argued for a default-off feature
  on those numbers, noting that the population which benefits from default-on
  is exactly the population already compiling the macro crate. The counter is
  that default-off breaks `cargo add rstest-bdd` plus
  `use rstest_bdd::prelude::*` as a first-five-minutes experience, which is
  unacceptable for an ergonomics item. Follow-ups 2 and 3 would remove most of
  the cost. Date/Author: 2026-09-14, planning agent.

- **DL-3 — Do not re-export `rstest::fixture` or `rstest::rstest`.**
  Rationale: it would not work. `rstest_macros` resolves its runtime path with
  `proc_macro_crate::crate_name("rstest").expect("rstest is present in`Cargo
  .toml`qed")`, so `#[fixture]` panics during expansion unless `rstest` is a
  direct dependency of the consuming crate. The manifest entry cannot be
  removed, so the re-export would delete one `use` line at the cost of making
  `rstest` a public dependency — and `rstest` is `0.26.1`, pre-1.0, so every
  minor bump would become a breaking change here. Separately, generated code
  already emits `#[rstest::rstest]` as a bare path
  (`codegen/scenario/test_attrs.rs:160`), so `rstest` is a de facto public
  dependency already; formalizing it would widen, not describe, the exposure.
  "Without hiding the underlying crates" reads naturally as licensing exactly
  this. A parallel review supplied the argument this entry was missing, and it
  is the decisive one: **a dependency on `rstest` would put the consumer's
  `rstest` version under `rstest-bdd`'s control**, gating every consumer's
  rstest upgrade on an rstest-bdd release. Since generated code emits
  `#[rstest::rstest]` resolved against the consumer's own dependency, the
  consumer must own that version. That is the trade to write into ADR-027. The
  same review rejected as overstated the claim that two `rstest` copies would
  "silently mismatch" — two rstest majors produce a hard compile error on the
  fixture-resolution protocol, not silent misbehaviour — so this plan does not
  lean on it either. That review also considered rewriting the examples'
  `use rstest::fixture;` into fully-qualified `#[rstest::fixture]`, which would
  leave each example importing literally nothing but the prelude, and then
  argued against it. This plan adopts that conclusion. The rewrite is cheap
  only because the examples are toys: a real suite with ten fixtures would pay
  ten attribute rewrites to save one `use` line, and the examples would be
  modelling a style no user should copy. Worse, after such a migration the
  examples would name `rstest` nowhere while all four manifests still require
  it, which *hides* the underlying crate and inverts the roadmap's stated goal.
  The examples keep `use rstest::fixture;`. Date/Author: 2026-09-14, planning
  agent; extended 2026-09-18 after reconciling with PR #772.

- **DL-4 — `#[harness_context]` is not promoted to a `proc_macro_attribute`.**
  Rationale: the Rust Reference restricts attribute macros to items, items in
  `extern` blocks, inherent and trait implementations, and trait definitions. A
  function parameter is none of those, so a real attribute macro could never be
  invoked where users write the marker. It would publish a rustdoc page for
  something inapplicable and degrade the diagnostic from
  `cannot find attribute` to
  `expected non-macro attribute, found attribute macro`. `rstest` declares
  exactly two `proc_macro_attribute` items and documents `#[case]`, `#[from]`,
  and the rest inside the parent macro's rustdoc; this repository already
  follows that precedent. Date/Author: 2026-09-14, planning agent.

- **DL-5 — The drift gate is a Python comparator, not an `insta` snapshot.**
  Rationale: Rust has no reflection over module items, so there is nothing for
  `insta` to serialize. A snapshot would capture a handwritten list compared
  against itself, which detects nothing, and `cargo insta accept` makes
  agreeing with a mistake one keystroke. A snapshot also cannot observe a
  Markdown table, which is the artefact RM-11.2.2 actually names. The
  repository has five precedents for source-versus-prose comparator scripts in
  `make lint`. Alternative considered: a `macro_rules!` that emits both the
  `pub use` items and a `&[&str]` of their names, giving a single source of
  truth. Elegant, and it would make the snapshot meaningful — but it obscures
  the prelude's most important property, that it is a plainly readable list,
  and it still cannot see the users guide. Rejected on those grounds, not on
  cost. A parallel review reached the opposite conclusion and deserves a direct
  answer, because its reasoning is sound as far as it goes. It proposed that
  the export-identity test emit an `insta` snapshot and that the parity gate
  read *that* rather than parse `prelude.rs`, on the grounds that
  `.rustfmt.toml`'s `imports_granularity = "Crate"` reflows the `pub use` block
  and would defeat a line-oriented parser at precisely the moment the gate must
  be right. The reflow hazard is real, and this plan now records it explicitly
  under `Interfaces and dependencies`. The snapshot does not fix it, though. A
  snapshot emitted by a test that names each item by hand is derived from that
  handwritten list, so an item added to `prelude.rs` without touching the test
  changes neither the snapshot nor the guide, and the gate stays green. The
  snapshot catches removals and renames — which is exactly what the V-3 doctest
  already catches for free. Only reading `prelude.rs` itself catches additions,
  and additions are the direction the finish line cares about. The answer to
  the reflow hazard is a brace-aware parser plus a rustfmt-formatted test
  fixture, not a different source of truth. Date/Author: 2026-09-14, planning
  agent; extended 2026-09-18 after reconciling with PR #772.

- **DL-6 — `RSTEST_BDD_HARNESS_CONTEXT_FIXTURE` is excluded from the prelude.**
  Rationale: roadmap 11.2.1 shipped `#[harness_context]` specifically so users
  stop naming the reserved key, and `docs/users-guide.md:180-183` tells readers
  to prefer the marker. Putting the constant one glob away from every user
  re-advertises what the immediately preceding roadmap item spent an ExecPlan
  hiding. It appears in no example. Note this is a deliberate narrowing of
  RM-11.2.2's "harness-context helpers" phrase: the helpers are the five
  `StepContext` methods, which travel with `StepContext`, and the reserved key
  is not a helper. Date/Author: 2026-09-14, planning agent.

- **DL-7 — Trait/derive pairs ship both halves or neither.**
  Rationale: exporting a derive without its trait produces a user who can write
  `#[derive(StepArgs)]` and cannot name the trait, with a "no method named …"
  error that does not point at the prelude. This admits `ScenarioState` and
  `StepArgs` and excludes `DataTable`/`DataTableRow`, whose companion types
  `Rows<T>` and `DataTableError` live in `rstest_bdd::datatable`; shipping the
  derives alone would leave a user unable to write the parameter type.
  Date/Author: 2026-09-14, planning agent.

- **DL-8 — No `prelude::v1`.** Rationale: `std::prelude::v1` exists because
  `std` can never break anything. This crate is 0.6.0 with a documented policy
  permitting breaks at the next minor, so versioning buys stability the project
  already has, at the cost of a second module, doubled rustdoc, and a migration
  story for a `v2` nobody can specify. The written admission bar plus the
  changelog requirement discharges the Cargo Book's glob-import caveat
  proportionately. No crate in the surveyed prior art versions its prelude.
  Date/Author: 2026-09-14, planning agent.

- **DL-9 — ACCEPTED DEVIATION: amend RM-11.2.2's finish line.** The finish
  line "compile tests prove examples import only the prelude plus their harness
  crate" is unachievable for five structural reasons, none caused by this plan:
  `rstest` must be in the consuming manifest or `#[fixture]` panics; generated
  code emits bare `#[rstest::rstest]`, `#[tokio::test]`, and `#[gpui::test]`
  paths; every example carries
  `#[rstest_bdd_test_macros::allow_fixture_expansion_lints]` from an
  unpublished crate; and `examples/todo-cli/tests/cli.rs:7` needs
  `rstest_bdd_harness::binary_test_support`. Rationale for the replacement: the
  property worth having is the one a gate can hold, and "no example reaches
  past the facade for something the facade provides" is exactly that. It is
  compiler-enforced in one direction (an example that names `rstest_bdd_macros`
  without the manifest entry fails to build) and script-enforced in the other
  (V-5 catches a manifest entry re-added without a corresponding import). The
  crates that remain visible — `rstest`, the harness crate, the test-only
  scaffolding crate — are precisely the ones the roadmap's own "without hiding
  the underlying crates" clause says should stay visible, so the amendment
  sharpens the entry rather than retreating from it. Alternatives rejected:
  ship and record the shortfall in the completion note (leaves a finish line
  the evidence contradicts, which is how roadmap entries rot); or expand scope
  to strip `rstest-bdd-test-macros` from the examples, which is follow-up 5 and
  would still leave `rstest`, `tokio`, and `gpui` unaddressed. Impact:
  `docs/roadmap.md:1038-1043`. The verbatim replacement text is fixed below and
  applied by EP-M7. **Status: accepted by the maintainer, 2026-09-18.**
  Date/Author: proposed 2026-09-14, accepted 2026-09-18.

### The verbatim roadmap replacement

EP-M7 replaces `docs/roadmap.md:1038-1043` with exactly this. Reproduce it
character for character; the wrapping is already at the 80-column limit the
Markdown gate enforces, and the checkbox becomes `[x]` at completion.

```markdown
- [ ] 11.2.2. The public prelude exposes `StepResult`, `Slot`, `ScenarioState`,
  and `StepContext` with its harness-context helpers, so examples can import
  one predictable module without hiding the underlying crates. The
  `#[harness_context]` marker from 11.2.1 is inert parameter syntax with no
  importable item behind it, so documentation covers it rather than an export.
  Finish line: every example reaches its `rstest-bdd` surface through
  `rstest_bdd::prelude` alone, with `rstest-bdd-macros` absent from every
  example manifest and every example source file, enforced in both directions
  by the compiler and by a repository gate; a fixture crate naming only
  `rstest-bdd` and `rstest` compiles and runs a scenario; and
  `docs/users-guide.md` lists the exported items under a gate that fails when
  the list and the code disagree. Examples continue to name `rstest`, their
  harness crate, and the test-only scaffolding crate directly: those are not
  re-exported and are meant to stay visible. Prerequisite: 11.2.1. Design Doc:
  `docs/rstest-bdd-design.md` §2.7.6.4. (Dinolump)
```

Three substantive changes beyond the finish line itself. The item text now says
`StepContext` explicitly, because that is what carries the five harness-context
helper methods the original entry named (DL-6 declines to export the reserved
key constant, which is the other thing "harness-context helpers" could have
meant). The marker clause is restated so that a later reader cannot construe it
as requiring an export that cannot exist (DL-4). And the finish line gains the
fixture-crate clause, which is the observable form of the property that
actually matters to a downstream user: that they never need `rstest-bdd-macros`
in their own manifest.

## Progress

- [x] EP-M0 — orientation and green baseline.
- [x] EP-M1 — dependency edge, crate-root re-export, lockfile refresh.
- [x] EP-M2 — the prelude module.
- [x] EP-M3 — marker documentation and the missing trybuild fixture.
- [x] EP-M4 — example migration and the two fixture crates.
- [x] EP-M5 — the drift gate and the users-guide table.
- [x] EP-M6 — remaining documentation and ADR-027.
- [x] EP-M7 — roadmap amendment, full gates, pull request.

EP-M1 is committed as `7fbd9fee`: the macro edge, crate-root re-exports,
manifest acyclicity test, paired-fixture comments, and fixture lock refreshes.
Its sequential gates passed on the candidate tree based on HEAD
`5d7ca1e80e29c6fbe1ec600535cdae2145d3046a`: `make check-fmt`, `make test`,
`make typecheck`, `make lint`, `make markdownlint`, `make nixie`, and
`make check-fixture-lockfiles`. The test gate passed 2,084 tests with seven
skipped, the no-default-features leg passed 740 with seven skipped, and all 290
Python tests passed. Logs are under `/tmp/` with the `-2` suffix and the branch
and worktree identifier. `make publish-check` also passed after the package
edge and before the two comment-only fixture-manifest edits; those comments do
not alter package metadata. The complete transcript is
`/tmp/publish-check-ep-m1-isolated-scratch-11-2-2-public-prelude-execplan.out`.

EP-M2's test-first surface is implemented but not yet committed. The focused
red doctest failed with unresolved prelude names before the re-exports were
added, then passed after they were added. The sibling surface test also passed,
compiling every prelude name at both `rstest_bdd::prelude` and the crate root.
On this candidate, `make check-fmt`, `make typecheck`, `make markdownlint`, and
`make nixie` passed. `make test` timed out in the nested-Cargo test
`cargo-bdd::cli::list_steps_runs` after 2,002 passes; `make lint` was
terminated before producing a Clippy result. Both require a clean rerun with
`CARGO_BUILD_JOBS=2`. Transcripts:
`/tmp/ep-m2-prelude-red-11-2-2-public-prelude-execplan.out`,
`/tmp/ep-m2-prelude-green-11-2-2-public-prelude-execplan.out`, and
`/tmp/ep-m2-prelude-surface-11-2-2-public-prelude-execplan.out`. Hosted run
`41540661058` on head `55fb30ae` passed both Windows jobs and CodeScene but
failed Linux while compiling the published GPUI fixture. DL-15 records the
Cargo lock correction; its fixture checks and a new hosted run remain pending.
CodeRabbit CLI `0.9.2` reviewed all 12 committed paths in `7fbd9fee` against
`5d7ca1e80e29c6fbe1ec600535cdae2145d3046a`; the review completed with zero
findings. Its log is
`/tmp/coderabbit-2f060a99-dabc-4460-b43b-7ea2da17770f-11-2-2-public-prelude-execplan.out`.
The finish-line amendment (DL-9) remains accepted. The plan was rebased onto
`main` at `4d585b789dcc8f070c3c7b7cb141388a65afcbe5` without conflicts; all
seven plan commits matched in `git range-diff`.

EP-M4 is implemented and committed alongside this paragraph. On the tree it
certifies, all four gates passed: `make check-fmt` in 1 s, `make lint` in 20 s
with all nine steps reaching verdicts, `make typecheck` in 4 s, and `make test`
in 549 s across its four legs (2,393 nextest, 180 doc tests, 812
no-default-features, and 303 Python tests). The gates ran at HEAD
`c67cd87c11c08ae5693f347cb624232756beb6b4` with the dirty tree hashing to
`1d6a00e3a54cb64957dde9c37a572e343cfcc5173dba0d5f4b673093ec366705` both at
start and at end. Transcripts are
`/tmp/{check-fmt,lint,typecheck,test}-rstest-bdd-11-2-2-public-prelude-execplan-ep-m4b.out`.
This paragraph was added after that run; it changes no gate input those four
gates compile, so `make check-fmt` and `make markdownlint` were re-run over the
final tree and are recorded in the commit message.

The milestone delivered three things beyond the plan's steps. The
`prelude_only` fixture carries a doc-only `src/lib.rs` so the crate is a legal
library target while the claim lives in its `tests/`, and its manifest keeps
`rstest` with the reason written down. `macros_direct` is the negative control:
it declares `rstest-bdd-macros` and imports the attributes from there, so the
manifest assertion runs against a case where the crate is genuinely present,
and the direct entry point keeps a compiled consumer once no example names it.
And `scripts/check_example_imports.py` reports an empty scan as a violation in
both halves, because a structural gate whose search silently matches nothing
passes for the wrong reason.

EP-M5 is implemented and committed alongside this paragraph. The users guide
gains an "Importing `rstest-bdd`" section immediately before "Step
definitions", with the glob example, the spelled-out example, and the
fourteen-row item table: thirteen prelude bindings, with two rows each for
`ScenarioState` and `StepArgs`, plus the `#[harness_context]` row that reads
"no import required". `scripts/check_prelude_exports.py` reads `prelude.rs`,
the crate root, the guide's section, and the three READMEs, and fails when any
of them disagrees with the others.
`scripts/tests/test_check_prelude_exports.py` covers each disagreement against
a synthetic tree plus one live assertion over this repository. The script is
wired into `make lint`, and the section is registered as a second
`EnforcedRegion`, so its fences must stay marker-carrying and loadable. The
non-vacuity proofs are recorded under "Artefacts and notes".

The check is four modules rather than one, because the Python budget is the
same 400 lines pylint enforces for Rust (`C0302`): a single file that parsed
the Rust, read the Markdown, and compared them reached 631 lines.
`scripts/prelude_exports.py` reads the two Rust sources,
`scripts/prelude_docs.py` locates the section and reads its tables and fences,
`scripts/prelude_drift.py` turns the pairs into violations, and
`scripts/check_prelude_exports.py` is the CLI. The split follows the existing
`scripts/users_guide_links.py` and `scripts/check_users_guide_links.py` pair.
Two of this repository's own DF12 lints then shaped the code: the
constant-comparison chain in `split_top_level` became a `match`, and every test
assertion carries a failure message so a red case prints its violations.

EP-M6 is committed alongside this paragraph. It lands
`docs/adr-027-publish-a-facade-prelude.md` and the eight documentation updates
that cross-link it, as the "Artefacts and notes" entry lists. All four directed
gates passed on the tree it certifies: `make check-fmt` in 2 s, `make lint` in
14 s with every step reaching a verdict, `make typecheck` in 1 s, and
`make test` in 553 s (2,394 nextest passed with seven skipped, 813 on the
no-default-features leg with seven skipped, every doctest, and 313 pytest
checks). The gates ran at HEAD `35aa33e2` with the dirty tree hashing to
`0d068b2e2818dcf3af1ff8dd9ce6447bf1aca9356d94735a442ddc6efbf166ab` at both
start and end, so no gate input moved under them. Transcripts are
`/tmp/{check-fmt,lint,typecheck,test}-ep-m6-11-2-2-public-prelude-execplan.out`.
The three Markdown gates ran before that freeze and are recorded in the same
commit message: `make fmt` left 145 files unchanged, `make markdownlint`
reported zero errors over 144 files, and `make nixie` validated every diagram.
This paragraph and the checklist tick above were added after that run. Neither
is read by the four gates, so `make check-fmt` and `make markdownlint` were
re-run over the final tree and their verdicts are in the commit message.

EP-M7 is committed alongside this paragraph. It replaces the roadmap 11.2.2
entry with the DL-9 text, ticks it complete, records what shipped and what was
deliberately left out, and files the eight follow-up items as issues #952-#959.
The `Status` line and the `Outcomes & retrospective` section are updated to
match. Two repair commits precede it: `a3896359` splits the prelude drift
check's long functions and `60535421` lowers its branch count after the hosted
CodeScene advisory gate rejected the first attempt at 9.39.

All seven gates of EP-M7's step 3 passed on the tree this certifies, at HEAD
`60535421` with the dirty tree hashing to
`77ddaf5c534395e3bc0e5dfe52e9a395ade66e1783c3fd8282db8fcb80d1c54c` at both
start and end, so no gate input moved under them: `make check-fmt` in 2 s,
`make lint` in 19 s with all eleven steps reaching a verdict, `make typecheck`
in 4 s, `make test` in 570 s (2,394 nextest passed with seven skipped, 813 on
the no-default-features leg with seven skipped, every doctest, and 318 pytest
checks), `make markdownlint` in 20 s with zero errors over 144 files after
`typos.toml` regenerated byte-identically, `make nixie` in 1 s, and
`make publish-check` in 453 s over nine lockfiles. Transcripts are
`/tmp/{check-fmt,lint,typecheck,test,markdownlint,nixie,publish-check}-ep-m7-11-2-2-public-prelude-execplan.out`.
This paragraph was added after that run, and `make markdownlint` reads this
file, so it and `make check-fmt` were re-run over the final tree and their
verdicts are in the commit message.

On 2026-11-03, the sequential M2 recheck passed `make check-fmt` but
`CARGO_BUILD_JOBS=2 make test` again timed out in a nested Cargo fixture.
`feature_addition_triggers_rebuild` exceeded its 300-second bound while waiting
for compiler admission; the queue reached 441–442 requests ahead. The suite
reported 2,032 passed, one failed, seven skipped, and 51 not run. No assertion
failure was reported. The supervisor remained healthy, with automatic cache
recovery pending. `make typecheck`, `make lint`, `make markdownlint`, and
`make nixie` did not run because the scrutineer stopped after the failed test
gate. A separate post-format non-Cargo sequence then passed `make check-fmt`,
`make markdownlint`, and `make nixie`; typecheck and lint remain incomplete on
this candidate. The candidate source did not change during the recheck. Logs:
`/tmp/check-fmt-2f060a99-dabc-4460-b43b-7ea2da17770f-11-2-2-public-prelude-execplan-recheck.out`
and
`/tmp/test-2f060a99-dabc-4460-b43b-7ea2da17770f-11-2-2-public-prelude-execplan-recheck.out`.

After automatic recovery cleared the prior live-work fault, another sequential
full run again passed `make check-fmt` and failed `make test` at
`feature_addition_triggers_rebuild`. Its nested Cargo command exceeded 300.46
seconds while queued for compiler slots; queue snapshots included 435 requests
ahead. The suite again reported 2,032 passed, one failed, seven skipped, and 51
not run, with no assertion failure. A separate sequential diagnostic run then
passed `CARGO_BUILD_JOBS=2 make typecheck`. `CARGO_BUILD_JOBS=2 make lint` was
terminated with exit 143 before completion; its log records queue waits up to
493 requests ahead at 180 seconds and contains no lint diagnostic. Do not
report that lint run as a pass. The latest formatted candidate passed the
separate `make check-fmt`, `make markdownlint`, and `make nixie` sequence.
Fixture lock checks and `make check-published-gpui` remain unrun. Logs:
`/tmp/check-fmt-2f060a99-dabc-4460-b43b-7ea2da17770f-11-2-2-public-prelude-execplan-full1.out`,
`/tmp/test-2f060a99-dabc-4460-b43b-7ea2da17770f-11-2-2-public-prelude-execplan-full1.out`,
`/tmp/typecheck-2f060a99-dabc-4460-b43b-7ea2da17770f-11-2-2-public-prelude-execplan-cargoonly1.out`,
and
`/tmp/lint-2f060a99-dabc-4460-b43b-7ea2da17770f-11-2-2-public-prelude-execplan-cargoonly1.out`.

On 2026-11-15 the branch was rebased from `main` at
`4d585b789dcc8f070c3c7b7cb141388a65afcbe5` onto the `issue-901` branch at
`7b617013179030120fc077c9959e28d1e058152a`, so the pull request now targets
that branch. The replay used Git's built-in merge machinery with `zdiff3`.
Weave was bypassed deliberately: only the ambient global attributes file
selected it and the repository declares no tracked merge driver, which is the
unattended baseline for a multi-commit replay. The replay produced no textual
conflicts. One commit, `5d7ca1e8` ("Refresh generated spelling configuration"),
became empty because the target already carried the identical `typos.toml`
blob; it was skipped, not rewritten, and every other commit matched in
`git range-diff`. The six affected fixture lockfiles were reset to the target's
versions, and a minimal rebuild added only the `rstest-bdd-macros` dependency
edge that the new `rstest-bdd` dependency requires. The target's `libc 0.2.189`
pins are preserved, so DL-15's constraint continues to hold without a separate
precise update.

The rebased branch was then gated at `8972fb920cf2406f51509febfb3a2d351ced4672`.
`make test` passed with 2,386 of 2,386 nextest tests (seven skipped) in
618.591 s, 805 of 805 on the `--no-default-features` leg in 168.977 s, every
doctest, the `check-fixture-lockfiles` check, and 296 pytest checks.
`make typecheck` and `make lint` passed on the same revision in 77 s and 81 s
against a warm tree, and `make check-fmt` passed on the revision this section
is committed with. Transcripts are
`/tmp/gates-11-2-2-public-prelude/test-window.out`, `typecheck.out` and
`lint.out`.

Two host conditions shaped that evidence. Neither is a property of the branch,
and recording them keeps a later reader from mistaking either for one.

First, the host's `rust-cleanup.service` deleted this worktree's `target/` at
21:33 UTC on 2026-11-15, between gate runs. The filesystem is in that tool's
pressure band (`/home` at 85% used, ten-minute inactivity cutoff), and no build
was running at that instant. The deletion is journaled as a
`rust-cleanup-event` with `"deleted": 1`; it discarded a completed 935.8 s
trybuild preparation and forced a 30 m 42 s cold workspace rebuild. The warm
tree is now kept in use by the repository's own incremental build while a gate
window is awaited.

Second, the shared four-slot compiler pool carried between 100 and 236 queued
requests from other agents' workspaces for the whole period.
`step_macros_compile` compiles roughly a hundred fixtures one after another, so
its 1200 s nextest period is sensitive to that backlog: it timed out at depths
of 116 and 236 and passed in 418.788 s once the queue fell to 33. No bound was
raised and no test was skipped to obtain that pass. The same tree failed only
on wall-clock time, and `make prepare-nested-fixtures` was run before the suite
in the order `.config/nextest.toml` prescribes.

EP-M3 on 2026-11-16 added an identical `# Parameter attributes` section to
`given`, `when` and `then`, and added
`crates/rstest-bdd/tests/ui_macros/harness_context_without_step.rs` with its
pinned stderr — the marker on a function that carries no step attribute, which
rustc rejects because nothing consumes it. The candidate's gates ran
sequentially and all four passed: `make check-fmt`, `make test` with 2,386 of
2,386 nextest tests (seven skipped), `make typecheck`, and `make lint`, whose
nine steps ran to the end, so the Whitaker suite and the five structural checks
that follow it all carry a verdict. Transcripts are
`/tmp/check-fmt-11-2-2-public-prelude-execplan.out`,
`/tmp/test2-11-2-2-public-prelude-execplan.out`,
`/tmp/typecheck-11-2-2-public-prelude-execplan.out` and
`/tmp/lint-11-2-2-public-prelude-execplan.out`.

The first `make test` on this candidate was not a test failure. The agent's own
background wrapper reached its 30-minute budget and delivered SIGTERM to
nextest, which reported `step_macros_compile` as signal-terminated alongside
783 passes, no assertion failure, and 21 tests not run. The re-run without that
budget passed the same tree in 172 s. Do not read
`/tmp/test-11-2-2-public-prelude-execplan.out` as evidence about the branch.

EP-M0 passed on `162798e8a457921288ce510f89fe899e8254daf6` with
`CARGO_BUILD_JOBS=2`: `make check-fmt`, `make lint`, `make typecheck`,
`make test`, `make markdownlint`, and `make nixie` all passed. The test gate
passed 2,083 of 2,083 nextest tests (seven skipped), all doctests, and 290
pytest checks. Logs are under `/tmp/` with the branch and worktree identifier
in their names. The temporary wildcard-import probe passed `make lint` in 66
seconds, with no `clippy::wildcard_imports` diagnostic; its source edits were
reverted immediately afterwards.

The initial count was wrong: Cargo's default deduplicated display marks
repeated packages with `(*)`, and the earlier `sort -u` counted those marker
rows as packages. With
`cargo tree -p rstest-bdd --prefix none -e normal
--no-dedupe --format '{p}' | LC_ALL=C sort -u | wc -l`,
the rebased runtime graph contains 99 packages, the macro graph 70, and their
union 129 (30 added by the edge). This is within the approved ceiling of 135,
so no dependency reduction or tolerance change is needed. The baseline
`cargo build` used a fresh in-repository target directory and warm shared
`sccache`; Bash's built-in timer measured 60.567 seconds wall, 13.320 user, and
3.484 system with 24 online CPUs. A fresh post-edge package build measured
137.087 seconds wall, 19.192 user, and 5.012 system while the four-slot
compiler pool was full with healthy queued work, so its wall time includes
shared admission contention. An earlier post-edge run measured 544.006 seconds
wall with 22.829 user and 5.708 system. Do not treat either wall reading as a
direct cold-build comparison with the baseline. A separate fresh nested
`rebuild_invalidation` fixture build passed both tests in 85.347 seconds wall,
18.406 user, and 4.861 system; that fixture already names `rstest-bdd-macros`
as a dev-dependency, so this measurement does not isolate the new edge's cost.
Keep these readings and the reference-machine measurements distinct in R-2 and
R-6.

The worktree was clean at the start of resumed implementation, and the branch
matched both its upstream and PR #773 head.

`make publish-check` passed with `CARGO_BUILD_JOBS=2` and
`TMPDIR=$HOME/.cache/rstest-bdd-publish-preflight`. It validated all seven
tracked lockfiles, checked the workspace, passed its unit and binary tests, and
packaged all eight publishable crates in dry-run mode. The complete transcript
is
`/tmp/publish-check-ep-m1-isolated-scratch-11-2-2-public-prelude-execplan.out`.
The external cache directory keeps the preflight's temporary test workspaces
outside the checkout; Cargo still uses the shared package cache.

EP-M5's four Python modules, their test module, the Makefile wiring, the
users-guide table and this section are committed alongside this paragraph
(2026-11-16). Bringing the candidate through `make lint` uncovered a second
wave of Whitaker drift: the locally installed suite was rebuilt at 09:07 UTC
that day (library mtime 10:07:40 +0100, tip checkout `880939d1`), so findings
appeared in functions this branch does not change. The repairs are committed
separately as `4e2c80b5`. Thirteen named functions were refactored by helper
extraction — never by `#[allow]` — in four waves: four in `rstest-bdd-macros`
(`canonicalize_with_cap_std`, `resolve_scenario`, `extract_examples`,
`parse_primary`; `/tmp/lint-ep-m5d-20261116T120224Z.out`), then `try_from` in
`crates/rstest-bdd/src/datatable/rows.rs`, then a `--keep-going` sweep that
re-reported `try_from` beside eight new sites
(`/tmp/lint-whitaker-discovery-20261116T124914Z.out`), then three more after
two first-pass refactors left the nested clusters inside their new helpers and
one helper the first pass created was itself flagged
(`/tmp/lint-whitaker-b-20261116T130340Z.out`,
`/tmp/lint-whitaker-c-20261116T131101Z.out`). `text_document.rs` had grown to
407 lines against the 400-line module cap, so its Rust-index application moved
to a `rust_index` child module (312 + 132 lines). `make lint-whitaker` passed
on this tree in 3.13 s at 13:12 UTC
(`/tmp/lint-whitaker-d-20261116T131252Z.out`).

The 2026-11-16 review round (CodeRabbit's assessment of `b5671f9a`) is
committed as `8ba9759e`. Its five documented gaps and its scope finding were
each reproduced against the tree before anything changed. The repairs are DL-17
and DL-18 plus the documentation they imply: the crate root re-exports
`step_library`; `scenarios` is called a function-like macro in the table, the
drift gate's item kinds, and the facade doc; the guide's two prelude fences are
pinned to compiled copies; `check_example_imports.py` enforces the prelude
alone and sees renamed and target-specific macro-crate dependencies; the
`rstest` requirement equality is asserted; and the migration guide and ADR-027
carry the signposts the review asked for. The seven commit gates then ran once
each, sequentially, on that exact revision: `make check-fmt` (2 s), `make lint`
(626 s, all eleven recipe steps so no step is left without a verdict),
`make typecheck` (228 s), `make test` (1,847 s: 2,396 nextest tests with seven
skipped and none failing, doctests, the no-default-features leg's 815 tests,
seven fixture lockfiles fresh, and 323 pytest checks), `make markdownlint` (23
s), `make nixie` (under a second), and `make publish-check` (420 s).
`REV_START` and `REV_END` were both `8ba9759e` and `typos.toml` was unchanged,
so every log under `/tmp/gate-<target>-8ba9759e.out` is attributable to that
tree. Two bounds on what the green means are recorded rather than glossed:
`make publish-check` tolerated five of eight dry-run publishes through
`--allow-unpublished-workspace-deps`, so those sibling requirements were not
shown to resolve from the registry, and both nextest summaries report seven
skipped tests. DL-16 records the scope measurement and the escalation.

The review's third gap had a second half the repair commit left open: it asked
for "a negative test that proves invalid Rust fails", and DL-18 supplied only
the positive pin. DL-19 closes it in the following commit with
`tests/ui_macros/prelude_omits_datatable.rs` and its `.stderr`, registered in
`run_failing_ui_tests`. The snapshot was not written by hand. The new case was
compiled as a bin of the trybuild staging crate to capture rustc's rendering,
that rendering was cut down to what trybuild's normalization leaves, and the
result was then re-checked by running the case through trybuild itself before
the file was accepted; the first attempt failed on a missing final newline,
which is why the check exists. Generating the snapshot by running the whole
`step_macros_compile` suite under `TRYBUILD=overwrite` was tried first and
abandoned: it recompiles every registered fixture in one run and had to be
killed at its timeout, having produced no snapshot for this case.

DL-19 and its fixture are committed as `d440bade`, an amend of the first attempt
`11516ce6`: the `spelling` prerequisite of `make markdownlint` rejected the
hyphenated form of "written by hand" in this section, a defect the earlier
round had already corrected once and this paragraph reintroduced. The gates on
the fixture commit are worth recording in full, because `make test` failed
twice there and neither failure was the new fixture. At `11516ce6`,
`make check-fmt` (3 s), `make lint` (215 s, all eleven recipe steps),
`make typecheck` (49 s), `make nixie` (3 s) and `make publish-check` (824 s)
were green, while `make test` twice ended at
`rstest-bdd::trybuild_macros runner::step_macros_compile` with
`TIMEOUT [1200.005s]`: sixteen fixtures had completed in the first attempt and
forty-eight in the second, with the shared compiler pool holding four slots and
between 25 and 153 requests waiting on every sample. `build-limits-status`
reported `healthy` throughout, so the gate was queued, not broken, and the
second attempt was an authorized re-run rather than a fix.

At `d440bade` the same test passed in 684.647 s with every fixture, the new one
included, and again in 327.951 s in the no-default-features leg. The gate then
ran to its end for the first time: 2,396 nextest tests passed with seven
skipped, every crate's doctests passed, the no-default-features leg passed 815
with seven skipped, seven fixture lockfiles were up to date, and 323 pytest
checks passed — 4,447 s in total. `make check-fmt` (2 s), `make markdownlint`
(21 s, both steps, `typos.toml` unchanged) and `make nixie` (1 s) were re-run at
`d440bade`. `make lint`, `make typecheck` and `make publish-check` are the
`11516ce6` verdicts, carried forward because `d440bade` differs from `11516ce6`
by one sentence in this file and no step of those three gates reads it.
`REV_START` and `REV_END` were `d440bade` for the four re-run gates and
`11516ce6` for the three carried forward.

Pushing `492b457f` turned the hosted CodeScene check red on three of its gates,
all of them against files this branch adds. The findings and the structural
repair are DL-20; what belongs here is why the local gates had nothing to say
about them. CodeScene is not one of the seven commit targets, so the failure
was invisible until a push ran the hosted check — the same shape of gap that
`make lint` and CodeScene can disagree about. The repair changes
`scripts/check_example_imports.py`, its test module, and the `prelude_fixture`
test support, so it is a code change and needs the full gate set rather than
the Markdown-only subset the previous round could rely on.

The second whole-plan assessment, returned against `f13a8f39`, closed four of
the five earlier gaps and named three new ones: the example guard matched text
markers rather than paths, the V-1 dependency guard read only top-level tables
and literal keys, and the migration checklist claimed the prelude replaces
every macro-crate import, which breaks the datatable derives it excludes. All
three are recorded with their repairs as DL-21. Two of them are guard holes, so
each repair carries a negative case that fails on the old code: a qualified
attribute and a raw macro path in the example tests, a renamed `package` entry
and a target-specific table in two inline-manifest tests. The third is a
documentation defect that reached further than the report: the users' guide
named `rstest_bdd_macros` in fifteen places this branch had not converted,
beside a sentence it added saying that import never names the macro crate. All
fifteen now use the facade, so the sentence is true of the guide's own examples.

These edits touch Rust, Python and Markdown, so this round needs the full gate
set again rather than a subset.

The round-two gates ran at `00c564ea` and all seven passed: `make check-fmt` in
2 s, `make lint` in 39 s with all eleven recipe steps reaching a verdict,
`make typecheck` in 9 s, `make markdownlint` over 144 files with no resulting
`typos.toml` diff, `make nixie` over 147 files, and `make publish-check` in 359
s. `make test` needed a second attempt. The first failed in fixture setup, not
in an assertion: `feature_addition_triggers_rebuild` reported that the
`feature-addition` stamp vouched for build artefacts that no longer existed.
The cause was an ad-hoc `cargo test` run outside the gate's `RUSTFLAGS`. A
nested-fixture stamp is a function of the compiler flags and the feature
selection, so that run wrote stamps under a different fingerprint, the gate's
own preparation then rewrote them, and Cargo produced new hash directories
while the timed test read a stamp whose artefacts were gone.
`make prepare-nested-fixtures` passed 7/7 at the same revision and the re-run
passed in 549 s with 2,398 of 2,398 nextest tests in both legs, all doctests,
817 of 817 in the no-default-features leg, seven fixture lockfiles, and 325
pytest checks. Do not run a bare `cargo test` in this worktree while a gate
window is pending.

The third whole-plan assessment, returned against `00c564ea`, accepted those
three repairs and named two guard gaps: the example guard could not see a
facade alias, and both dependency guards read an inherited workspace alias as
its local key. Both repairs also required moving the manifest half of the
example guard into its own module to stay inside the 400-line Python budget.
The round touches Rust, Python and Markdown, so it needs the full gate set
again. The findings and their repairs are DL-22.

The fourth whole-plan assessment, returned against `cde5c782`, found the
implementation substantially complete and named three remaining items. Two are
actionable here: a failed CodeScene check on duplicated guard bodies, repaired
by consolidating them into an `rstest` table, and R-6's missing baseline, now
measured as six alternating paired builds. The third — a maintainer's
acceptance of the 84-file, 10,029-line tolerance overage — is the escalation
DL-16 recorded and is not this plan's to grant. Both repairs and the figures
are DL-23. The code change means this round needs the full gate set again.

## Surprises & discoveries

Recorded during planning and execution as evidence was confirmed.

- **Observation:** the shared Whitaker dylint suite changed underneath this
  branch mid-session, and two functions that had passed it earlier failed it.
  **Evidence:** `libwhitaker_suite@nightly-2026-05-28.so` was rebuilt at 04:56
  UTC on 2026-11-16 and the prior copy remains beside it as
  `*.tip-880939d1-backup`; the rebuilt suite reports the new
  `bumpy_road_function` lint on `ScenarioData::filter_by_tags` and on
  `gpui::run_test` in this workspace's own shim under `vendor/gpui`, the second
  reproducing standalone with `whitaker --lib whitaker_suite -- -p gpui`.
  **Impact:** `make lint` could not pass for any candidate until both were
  addressed. Both were split into named helpers rather than suppressed, and
  `dylint.toml` gained no exclusion: `bumpy_road_function` offers no
  excluded-crates key, and `vendor/gpui` is first-party code this workspace
  owns, not vendored upstream.

- **Observation:** `make test` can fail on this host for a reason unrelated to
  the candidate. **Evidence:** `target_directory_uses_running_test_executable`
  panicked with "integration tests should run beneath a target directory
  carrying Cargo's `CACHEDIR.TAG`" because this worktree's `target/` had no
  such tag; copying Cargo's standard tag from the nested probe target restored
  the test. **Impact:** check for the tag before reading that panic as a
  regression, and keep it in place when a cleanup tool empties `target/`.

- **Observation:** the Whitaker suite rejects `std::fs` even in a test helper,
  so the first EP-M4 gate run failed at `make lint`'s Whitaker step rather than
  at Clippy. **Evidence:** the step reported "std::fs operation
  `std::fs::read_to_string` bypasses the capability-based filesystem policy" at
  `crates/rstest-bdd/tests/prelude_fixture/support.rs:59` and `:85`, failing to
  compile the `prelude_fixture` test binary; `make check-fmt` had already
  failed separately on the same three new files being unformatted. **Impact:**
  a new helper that reads a fixture manifest or source must open the directory
  with `cap_std::fs::Dir::open_ambient_dir` and call `read_to_string` on the
  handle, as `crates/rstest-bdd/tests/nextest_config/support.rs` already does.
  Run `cargo +nightly-2026-08-07 fmt --all` over new files before the first
  gate run, since `rustfmt` re-wraps them and the check gate reads the result.

- **Observation:** the first wildcard probe attempts failed on module-document
  lint rules before reaching the wildcard decision. **Evidence:** Whitaker
  rejected a module without an inner doc comment, then Clippy rejected mixed
  inner and outer docs; the corrected inner-only probe passed `make lint` in 66
  seconds with no wildcard diagnostic. **Impact:** future probes must use the
  documented module shape above so they test only the intended lint question.

- **Observation:** `/usr/bin/time` is not installed in this worktree
  environment. **Evidence:** the requested command returned
  `/bin/bash: /usr/bin/time: No such file or directory`; the Cargo build then
  passed under Bash's built-in timer. **Impact:** the measured values remain
  useful for this host but are not a cold reference comparison.

- **Observation:** the initial rebased package count included Cargo's display
  marker as if it were a package. **Evidence:** using
  `cargo tree -p rstest-bdd --prefix none -e normal --no-dedupe --format '{p}'`
  produces 99 runtime packages, 70 macro packages, and a 129-package union; the
  initial deduplicated display command reported 121 runtime rows and
  incorrectly projected 154. **Impact:** the original 135-package ceiling
  remains satisfied and EP-M1 can proceed.

- **Observation:** shared compiler admission distorts fresh-target wall times.
  **Evidence:** a post-edge runtime build measured 137.087 seconds wall but
  24.204 seconds combined user and system CPU while the four-slot pool was full
  and queued work was healthy; a prior run measured 544.006 seconds wall with
  28.537 seconds CPU. The baseline measured 60.567 seconds wall and 16.804
  seconds CPU with a different queue state. **Impact:** retain these values
  with their admission context; do not infer a dependency reduction from wall
  time or claim a like-for-like comparison.

- **Observation:** Lading's publish preflight shares `TMPDIR` with its Cargo
  target and Rust test fixtures. **Evidence:** placing it under the checkout
  made `rstest-bdd-server` fixtures appear nested in the repository workspace;
  moving the temporary root to the user cache made the complete dry run pass.
  **Impact:** use the user cache for `TMPDIR` on this gate and keep `/tmp` for
  logs only.

- **Observation:** a `target/` directory can disappear between gate runs.
  **Evidence:** `rust-cleanup.timer` deleted this worktree's `target/` at 21:33
  UTC on 2026-11-15 while `/home` was 85% full, which is that tool's pressure
  band with a ten-minute inactivity cutoff; the journal records
  `rust-cleanup-event` with `"deleted": 1` and no build was running. The cost
  was a discarded 935.8 s trybuild preparation and a 30 m 42 s rebuild.
  **Impact:** a gate cycle interrupted between stages may be recovering from a
  host cleanup rather than from its own failure, so check whether `target/`
  still exists before diagnosing the next failure.

- **Observation:** `step_macros_compile`'s 1200 s period measures the shared
  admission queue as much as the compiler. **Evidence:** the test timed out at
  queue depths of 116 and 236 and passed in 418.788 s at depth 33, on an
  unchanged tree whose nested fixtures were already prepared. **Impact:** when
  this single test fails on time, re-run the suite in a shallow-queue window
  rather than changing the test, its period, or its fixture count.

- **Observation:** the compile-time export inventory has thirteen namespace
  bindings across eleven distinct spellings. **Evidence:** `ScenarioState` and
  `StepArgs` each occupy both the type and macro namespaces; the table now
  lists those bindings separately. **Impact:** V-3 and the prelude doctest must
  account for every binding without confusing spelling count with namespace
  membership.

- **Observation:** the next design decision record was ADR-024 when this plan
  was drafted. **Evidence:** the rebased main branch already contained ADR-023.
  **Impact:** all proposed ADR references in this plan used ADR-024 at drafting
  time. That reservation expired twice on the moving base: ADR-024 and ADR-025
  landed before this branch was rebased onto `main`, then the stacked base's
  own merge took 025 and moved OrthoConfig to 026, so the shipped record is
  ADR-027. The later observation recording the change carries the evidence.

- **Observation:** three READMEs document an import that has never compiled.
  **Evidence:** `README.md:116`, `crates/rstest-bdd/README.md:106`, and
  `crates/rstest-bdd-macros/README.md:106` all read
  `use rstest_bdd::{scenario, given, when, then, StepResult};`, and each
  repeats the shape at four further lines; `crates/rstest-bdd/src/lib.rs`
  contains no `pub use rstest_bdd_macros` anywhere. No README is doctested.
  **Impact:** turns this item from an ergonomic nicety into a correctness fix,
  and supplies the Milestone 5 gate with its most valuable rule.

- **Observation:** `doc(cfg)` cannot be used in this repository at all.
  **Evidence:** `Makefile:27` sets `RUSTDOC_FLAGS ?= --cfg docsrs -D warnings`
  unconditionally; `rust-toolchain.toml` pins `channel = "stable"`; a grep for
  `doc_cfg` or `cfg_attr(docsrs` across `crates/**/*.rs` returns zero hits.
  **Impact:** eliminated the feature-gated designs three reviewers had
  independently recommended, and is the single strongest argument for DL-2.

- **Observation:** `rstest` is already a public dependency of this framework.
  **Evidence:**
  `crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs:160` emits
  `#[rstest::rstest]` as a bare relative path, unlike the `rstest-bdd` path
  which goes through `proc_macro_crate`. Lines 161 and 164 do the same for
  `tokio` and `gpui`. **Impact:** reframes Q1 entirely. The question is not
  whether to take on `rstest` as a public dependency but whether to formalize
  one already present. DL-3 declines to widen it.

- **Observation:** the finish line was unachievable before this plan existed.
  **Evidence:** the five dependencies enumerated in DL-9. **Impact:** DL-9, and
  a reminder that finish lines should be checked against the tree when written,
  not when claimed.

- **Observation:** a second ExecPlan for this item was written concurrently and
  independently, and reached most of the same conclusions by different routes.
  **Evidence:** PR #772, branch
  `11-2-2-prelude-exposes-step-result-and-context-helpers`, 2,041 lines. Two
  Lody sessions were given the task without either being told about the other.
  **Impact:** its findings are folded into this plan and credited inline as "a
  parallel review". The two plans agree on every substantive decision except
  the drift-gate mechanism (see DL-5) and the `macros` feature (that plan gates
  it default-on; this one does not gate it at all, on the `doc(cfg)` evidence
  in DL-2). Where they agreed by different reasoning, the stronger argument has
  been adopted.

- **Observation:** `cargo tree` walks dev-dependency edges by default, which
  makes the obvious acyclicity check vacuous. **Evidence:** on the unmodified
  tree `cargo tree -p rstest-bdd -i rstest-bdd-macros` already prints the
  package via `[dev-dependencies]`; adding `-e normal` reports "nothing to
  print". **Impact:** every measurement in EP-M0 carries `-e normal`, and V-1
  parses the manifest directly rather than shelling out to `cargo tree`.

- **Observation:** dropping `diagnostics` from `default` would be invisible to
  every gate. **Evidence:** `crates/rstest-bdd/Cargo.toml:63-71` declares
  `default = ["diagnostics"]` and lines 73-76 gate the `dump_registry` test on
  `required-features = ["diagnostics"]`, while `Makefile:28` runs every gate
  with `--all-features`. **Impact:** a hard constraint, because the original
  draft of the parallel plan instructed the implementer to "add a `[features]`
  section (the crate has none today)", which would have silently shipped a
  semver-visible regression. Four of its six reviewers caught it independently.

- **Observation:** `#[macro_export]` macros are not reachable through a glob of
  a module. **Evidence:** `crates/rstest-bdd/src/macros.rs:4` states that the
  crate's assertion macros "remain available at the crate root via
  `#[macro_export]`". **Impact:** the assertion macros are excluded from the
  prelude on merit, but had they been wanted, a glob would not have delivered
  them; they would need an explicit `pub use crate::skip;`.

- **Observation:** the `rstest` attribute path is a bare, un-renameable string
  literal. **Evidence:** `crates/rstest-bdd-harness/src/policy.rs:89` declares
  `const DEFAULT_TEST_ATTRIBUTES: [TestAttribute; 1] = [TestAttribute::new("rstest::rstest")];`
  and a search for `proc_macro_crate` across `crates/rstest-bdd-harness/src/`
  returns nothing. Unlike every first-party path, this one is never resolved
  against the consumer's manifest. **Impact:** R-11, and follow-up 8.

- **Observation:** Clippy's `wildcard_imports` exempts paths containing
  `prelude`, and this workspace does not override that. **Evidence:**
  `clippy.toml` does not set `warn-on-all-wildcard-imports`, whose default is
  `false`; a parallel review confirmed the behaviour with a probe crate under
  `#![warn(clippy::pedantic)]`. **Impact:** this is the concrete reason the
  prelude module earns its place beside the crate-root re-export rather than
  being redundant with it, and EP-M0 now re-probes it before anything depends
  on it.

- **Observation:** `pub use` cannot say what kind of item a name is, so the
  drift gate needs its own kind table. **Evidence:**
  `crates/rstest-bdd/src/prelude.rs` states in prose that `ScenarioState` and
  `StepArgs` each name a trait and a derive macro; nothing in the two `pub use`
  statements records which is which, and the two halves live in different
  crates. **Impact:** `ITEM_KINDS` in `scripts/prelude_exports.py` is the only
  machine-readable statement of that fact. A prelude binding with no entry
  there is reported as a violation rather than assumed correct, so adding an
  export fails the gate until its documentation row is written.

- **Observation:** the "fully spelled" import must be spelled through the crate
  root, not through the defining modules. **Evidence:** the prelude's module
  documentation promises that it "adds no names that are unavailable at the
  root", and `crates/rstest-bdd/src/lib.rs:56-160` re-exports every one of the
  six runtime bindings. **Impact:** the users-guide example names
  `rstest_bdd::{…}` only, and the gate rejects any spelled example that names
  `rstest_bdd_macros`, which would have re-taught the very habit EP-M4 removed
  from the examples.

- **Observation:** the Python side of `make lint` is three gates with three
  separate opinions, and passing one says nothing about the others.
  **Evidence:** a single-file checker passed `ruff check` and its own tests,
  then failed pylint with `C0302 Too many lines in module (631/400)`; after the
  file was split into four cohesive modules, pylint passed at 10.00/10 but the
  DF12 plugin rejected a constant-comparison chain (`R9103`) and ten test
  assertions without failure messages (`C9102`). **Impact:** budget a Python
  helper like a Rust module — split before the first gate run — and read
  `make lint-python`'s whole recipe, not only the first red line, because each
  step stops the recipe before the next one has a verdict.

- **Observation:** the Whitaker suite is a rolling tip, so a green gate speaks
  for one `.so` file rather than for the branch alone. **Evidence:** on
  2026-11-16 the suite staged under
  `~/.cache/whitaker-tip/staging/nightly-2026-05-28/` was rebuilt at 09:07 UTC
  (library mtime 10:07:40 +0100; tip checkout `880939d1`), and the next
  `make lint` reported thirteen `bumpy_road_function` sites in functions that
  `origin/main` carries unchanged, among them
  `crates/rstest-bdd-macros/src/parsing/tags/parser.rs` and
  `crates/rstest-bdd/src/datatable/rows.rs`. **Impact:** attribute a red
  `lint-whitaker` finding before repairing it — check the library's mtime and
  the function's own history — and still repair it by helper extraction, since
  the shipped suite is what CI's pinned copy converges on. The same rebuild
  enforced the 400-line module cap on `text_document.rs`.

- **Observation:** the Whitaker gate stops at the first failing crate, so a fix
  loop costs one full rebuild per finding unless the checker is asked to keep
  going. **Evidence:** adding `--keep-going` to
  `whitaker --lib whitaker_suite -- --workspace --all-targets --all-features`
  enumerated all nine outstanding sites in one 5 m 29 s run while exiting 0, so
  the run is a survey rather than a gate. **Impact:** survey with
  `--keep-going`, then repair the set, then run `make lint-whitaker` for the
  verdict; do not read the survey's exit status as a gate result.

- **Observation:** this plan's decision record is ADR-027, not the ADR-024 the
  plan was drafted against, and not the ADR-026 an earlier rebase reserved.
  **Evidence:** while the plan waited on its stacked base, `main` gained
  `docs/adr-024-structural-usage-tracking-in-registry-lookups.md` and
  `docs/adr-025-adopt-ortho-config-for-the-cargo-bdd-cli.md`, which made 026
  free at the time. The stacked base then merged as PR #912 and landed its own
  records: `docs/adr-025-nested-cargo-preparation-gate-query-boundary.md` took
  025, moving the OrthoConfig record to
  `docs/adr-026-adopt-ortho-config-for-the-cargo-bdd-cli.md`, so 027 became the
  first free number. **Impact:** every reference in this plan and in the
  shipped documents names ADR-027, and the record is
  `docs/adr-027-publish-a-facade-prelude.md`. A plan that reserves a number and
  then waits for a rebase must re-check the reservation, because the number it
  chose is a fact about the base branch at drafting time, not about the plan.
  This record has now moved twice for that reason; the second move is the
  stronger evidence for the rule.

- **Observation:** the GPUI worked example keeps its explicit
  `rstest_bdd_macros` imports. **Evidence:** the three blocks in
  `docs/users-guide.md` that show a stateful GPUI window mirror
  `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs:18`, which still
  writes `use rstest_bdd_macros::{given, scenario, then, when};`; migrating the
  prose without the test would publish a shape the first-party test does not
  use. **Impact:** EP-M6 migrated fourteen step-writing samples and left the
  GPUI blocks, the harness cookbook, the published-GPUI variants, the datatable
  sections, `step_library`, and the localization and language-server sections
  explicit, as the milestone's keep-list requires. Moving the GPUI test and its
  prose together is follow-up work, not a documentation edit.

- **Observation:** CodeScene's advisory gate does enforce
  `Overall Code Complexity`, and that rule is the *mean* cyclomatic complexity
  across a file's functions, so satisfying `Complex Method` by extracting
  helpers makes it worse. **Evidence:** the hosted check on `a3896359` failed
  with "Enforce advisory code health rules (1 file with Overall Code
  Complexity)" at 9.39 for `scripts/prelude_drift.py`, a file whose Complex
  Method findings that commit had just removed; `cs review` names the rule's
  definition, and lowering the file's total branch count rather than splitting
  it again reached 10.0. **Impact:** re-measure a file after a split that clears
  `Complex Method`, because the extracted helpers join the mean the other rule
  scores.

- **Observation:** the documented-example harness enforces a region's *shape*,
  not that its fences compile. **Evidence:** registering "Importing
  `rstest-bdd`" as an enforced region made both prelude markers mandatory and
  would have failed an unmarked fence, but nothing in the harness resolves an
  import; EP-M5 step 4's "must compile" was unmet by the registration alone,
  and the guide's examples only became code when a module copied each one.
  **Impact:** a marked fence is documentation that a gate reads, not code that
  a compiler checks. `prelude_examples/` now holds each import once as a
  module, and `prelude_examples_are_compiled_as_written` compares the guide's
  fence with that copy line for line, so the two cannot diverge without a red
  test.

- **Observation:** the guide's handwritten import layout and rustfmt's are
  different fixed points, and a text-equality pin has to pick one. **Evidence:**
  `.rustfmt.toml` sets `imports_layout = "HorizontalVertical"` with a
  100-column width, so the compiled copy's spelled import became one name per
  line, while the guide's fence was packed to 80 columns; the pin failed until
  the guide adopted rustfmt's layout. **Impact:** pinning documentation to
  compiled code makes the formatter an author of the prose, which is the price
  of the guarantee. Write such a fence in the layout the compiler will
  reproduce rather than expecting the pin to tolerate cosmetic differences.

- **Observation:** the roadmap's accepted finish line was stronger than the
  gate written for it. **Evidence:** DL-9 says "through `rstest_bdd::prelude`
  alone", while `scripts/check_example_imports.py` accepted
  `use rstest_bdd::{…}` as a facade import until the review compared the two;
  every live example already complied, so no example changed. **Impact:** when
  a gate and its stated requirement are written at different times, compare
  them before trusting the gate's green; the gate was tightened rather than the
  requirement reworded.

## Outcomes & retrospective

All seven milestones landed. `rstest-bdd` now depends on `rstest-bdd-macros`,
re-exports that crate's surface at its root, and offers `rstest_bdd::prelude`
as a strict subset: eleven names, thirteen items, and nothing reachable only
through the prelude. The four examples import it without naming the macro crate,
`crates/rstest-bdd/tests/fixtures/prelude_only` proves a downstream manifest
needs only `rstest-bdd` and `rstest`, and `scripts/check_prelude_exports.py`
fails when the users' guide's item table, its spelled-out example, or the
README imports disagree with the code.

The three observable successes in the purpose section are met as follows. The
examples build and pass with no path to the macro crate (V-5). The users' guide
lists the exports under a gate that bites: EP-M5's two deliberate
desynchronizations each made the check exit non-zero, and both recordings are in
`Artefacts and notes`. The downstream fixture crate compiles and runs a
scenario. ADR-027 records the decision and the admission bar; the roadmap entry
carries the amended finish line.

Two requirements from the original entry could not be met as written, and the
plan says so rather than pretending otherwise. DL-9 replaced "examples import
only the prelude plus their harness crate" with "no example reaches past the
facade for something the facade provides", because `rstest` must stay in the
consuming manifest, generated code emits bare `rstest`, `tokio`, and `gpui`
paths, and the examples use an unpublished test-scaffolding crate. DL-2 made
the macro edge unconditional, because no available gate would protect a
feature-gated re-export. Both were approved before implementation; neither was
discovered late.

The lesson worth keeping is about CodeScene's advisory gate. Its
`Overall Code Complexity` rule is the **mean** cyclomatic complexity across a
file's functions, and it is enforced. Splitting a long function to satisfy
`Complex Method` therefore raises the mean, because every extracted helper
contributes its own branches to the average: the first repair commit on the
drift checker cleared four findings and still failed the gate at 9.39. What
worked was lowering the total: one lookup instead of three scans, one loop over
a symmetric difference instead of two generator expressions, and a header check
lifted out of a row walk. Both rules are satisfied now, at 10.0 per file.

A second lesson is about drift gates. A check that compares generated truth
with prose must report when it finds *nothing* to compare. An empty table, a
missing example block, or a README with no braced import all read as agreement
otherwise, and the gate passes for the wrong reason. Every comparison in the
three `scripts/prelude_*.py` modules emits a violation when its input is absent.

Residual risk, carried by issues rather than by this change: the prelude's glob
hazard has no public-API gate (#952), the macro edge costs thirty extra
packages and no gate measures that (#954), and the examples still need an
unpublished crate to be copy-pasteable (#956).

## Idempotence and recovery

Every milestone is a single revertible commit. The riskiest step is the
lockfile refresh in Milestone 1, which touches seven files; if it goes wrong,
`git checkout -- '**/Cargo.lock'` restores them and the `make update-*-lock`
targets regenerate them deterministically. Nothing in this plan deletes user
data, mutates state outside the working tree, or touches the network beyond
`cargo`'s normal registry access. `make prefetch-fixture-deps` downloads
nothing new, because the package set is unchanged.

Re-running any `make` target is safe. Re-running the Milestone 5 gate-proving
step is safe provided the deliberately broken line is restored; do that with
`git checkout --` rather than by retyping.

## Artefacts and notes

To be filled in as milestones land. Required transcripts:

- EP-M0: the two baseline measurements.
- EP-M1: `make publish-check` output; the post-change measurements.
- EP-M2: the red doctest failure and the green pass.
- EP-M5: the two deliberate-desynchronization failures proving the gate bites.
  Both were run on 2026-11-16 at HEAD
  `56251dff912a7c0c31c1c891a5025bae15327393` and restored afterwards.
  - `/tmp/nonvacuity-prelude-export.out`: with `then` removed from the macro
    `pub use` line (mutated file sha256 `170eb4b0…`), `python3
    scripts/check_prelude_exports.py` exited 1 reporting both "the table
    documents rstest-bdd-macros::then, which the prelude does not export" and
    "the spelled example names then, which the prelude does not export", and
    `cargo test -p rstest-bdd --doc` exited 101 with "cannot find attribute
    `then` in this scope" and one failing doctest.
  - `/tmp/nonvacuity-table-row.out`: with the `Slot` row deleted from the item
    table (mutated guide sha256 `3facc3ba…`), the same script exited 1 with
    "the table omits rstest-bdd::Slot". The restored tree exits 0.
- EP-M6: `docs/adr-027-publish-a-facade-prelude.md`, and the documentation
  edits it cross-links: the design document's delivered sentence, the
  developers' guide's both-directions policy, admission bar, and two gate
  sections, the repository layout's facade edge, the ergonomics document's
  §2.4, the contents index entry, and the changelog entry. The users guide also
  gained the supported `rstest` range, which R-11's mitigation requires. The
  four commit-gate transcripts are
  `/tmp/{check-fmt,lint,typecheck,test}-ep-m6-11-2-2-public-prelude-execplan.out`.
- EP-M7: `docs/roadmap.md`'s amended 11.2.2 entry with its completion note,
  issues #952-#959 for the follow-up items, and the seven gate transcripts
  `/tmp/{check-fmt,lint,typecheck,test,markdownlint,nixie,publish-check}-ep-m7-11-2-2-public-prelude-execplan.out`.

## Interfaces and dependencies

At the end of EP-M2, `crates/rstest-bdd/src/prelude.rs` must export exactly:

```rust
pub use crate::context::StepContext;
pub use crate::state::{ScenarioState, Slot};
pub use crate::step_args::StepArgs;
pub use crate::{StepError, StepResult};
pub use rstest_bdd_macros::{
    ScenarioState, StepArgs, given, scenario, scenarios, then, when,
};
```

Note the trait imports come from `crate::state` and `crate::step_args`, not
`crate::`, to avoid E0252 (R-8).

At the end of EP-M1, `crates/rstest-bdd/Cargo.toml` must contain
`rstest-bdd-macros.workspace = true` under `[dependencies]`, a
`[dev-dependencies]` entry adding only
`features = ["compile-time-validation"]`, and `[package.metadata.docs.rs]` with
`all-features = true` and no `rustdoc-args`.

New scripts, both following the shape of
`scripts/check_serial_nextest_matrix.py` and both wired into `make lint`:

- `scripts/check_prelude_exports.py`
- `scripts/check_example_imports.py`

**Implementation hazard for `check_prelude_exports.py`.** `.rustfmt.toml` sets
`imports_granularity = "Crate"` and `imports_layout = "HorizontalVertical"`, so
rustfmt merges and reflows the `pub use` block: the five statements shown above
become one `pub use crate::{...}` group plus one
`pub use rstest_bdd_macros::{...}` group, wrapped across lines. A line-oriented
parser that assumes one item per `pub use` line will therefore break the first
time `make fmt` runs. The script must parse the braced, reflowed form —
collecting leaf names from nested `{...}` groups — and its pytest suite must
include a rustfmt-formatted sample as a fixture. Verify this by running
`make fmt` immediately after Milestone 2 and confirming the script still passes.

New tests:

- `crates/rstest-bdd/tests/prelude_surface.rs`
- `crates/rstest-bdd/tests/prelude_fixture.rs` and its support modules
- `crates/rstest-bdd/tests/fixtures/prelude_only/`
- `crates/rstest-bdd/tests/ui_macros/harness_context_without_step.rs`
- `crates/rstest-bdd/tests/ui_macros/prelude_omits_datatable.rs`
- `crates/rstest-bdd/tests/documentation_examples/tests/prelude_examples/`
- `scripts/tests/test_check_prelude_exports.py`
- `scripts/tests/test_check_example_imports.py`

## Revision notes

- 2026-09-14, initial draft. Written after a six-lens design review whose
  findings overturned three of the six draft decisions: the macro re-export
  became unconditional rather than feature-gated (DL-2), re-exporting
  `rstest::fixture` was dropped as ineffective rather than merely costly
  (DL-3), and the `insta` snapshot was replaced by a Python comparator (DL-5).
  The plan was `BLOCKED` on DL-9, the finish-line amendment.

- 2026-09-18, reconciled with PR #772. A second ExecPlan for this item was
  written concurrently and independently; its findings are folded in here and
  credited inline as "a parallel review". Six discoveries were adopted: the
  `cargo tree` dev-edge trap, the `diagnostics`-in-`default` hazard that no
  gate would catch, the `#[macro_export]` reachability fact, the un-renameable
  `"rstest::rstest"` literal (now R-11 and follow-up 8), Clippy's `prelude`
  exemption for `wildcard_imports` (now the stated reason the prelude module
  earns its place, and an EP-M0 probe), and the import-allowlist scoping that
  keeps `examples/todo-cli/tests/cli.rs` out. Two arguments were corrected:
  serde's `derive` is opt-in and is therefore no precedent for a default-on
  feature, and the "two rstest copies silently mismatch" claim is overstated.
  Two disagreements are recorded rather than resolved away: the drift-gate
  mechanism (DL-5) and whether the macro re-export should be feature-gated
  (DL-2). No milestone was added or removed.

- 2026-09-18, DL-9 accepted. The maintainer approved amending RM-11.2.2's
  finish line, so the plan is no longer blocked and is `DRAFT` awaiting
  ordinary approval. DL-9 now carries the verbatim replacement text for
  `docs/roadmap.md:1038-1043`, which also restates the item's "harness-context
  helpers" and "marker attributes" clauses in line with DL-4 and DL-6 so that a
  later reader cannot construe either as requiring an export that cannot exist.
  EP-M7 step 1 applies that text mechanically. No other section changed: the
  design, milestones, and verification plan are unaffected by the amendment.

- 2026-11-16, EP-M3 complete. The marker documentation and the
  `harness_context_without_step` fixture are implemented and gated, and
  `Progress` and `Surprises & discoveries` record the candidate's verdicts. Two
  refactors were needed for the `bumpy_road_function` lint that arrived with a
  rebuilt Whitaker suite; neither changes behaviour. EP-M4 onwards remain.

- 2026-11-23, restacked onto `main`. The stacked base merged as PR #912 and
  landed as squash `4c21753d`, so the 35 commits this branch owns were replayed
  from the exclusive boundary `7b617013` onto `main`. Two commits were dropped
  because `main` had already made the same lint-driven refactors in a more
  developed form: `a2564b38` (two Whitaker `bumpy_road_function` sites) and
  `4e2c80b5` (thirteen sites plus a `text_document` module split). Every other
  commit replayed; the three that changed did so only where this branch's text
  met `main`'s: the `nextest_config` period paragraph keeps both, and the ADR
  renumbering below. The decision record moved from ADR-026 to ADR-027 because
  the merged base took 025 for the nested-Cargo preparation gate and moved
  OrthoConfig to 026; every link and reference now names
  `docs/adr-027-publish-a-facade-prelude.md`. One integration seam did not
  conflict but did not compile: the prelude fixture called `ensure_prepared`,
  which the base renamed to `ensure_prepared_building` and gave a runner
  parameter, and `Preparation::new` gained `Directories`; `a4801610` adapts the
  fixture to that API. At `a4801610`, `make check-fmt`, `make typecheck`,
  `make lint` (all eleven steps including Whitaker and both pylint runs at
  10.00/10), `make markdownlint` and `make nixie` are green. `make test` is not
  green on this host: the nested-Cargo preparation tests exceed their own
  in-test wall-clock bounds while the shared compile-admission pool holds
  roughly 300 requests from other repositories, so 2,385 of 2,574 nextest cases
  passed and the run stopped at the first preparation timeout. The hosted
  `build-test (linux, default features)` lane runs the same suite on a trusted
  runner and is the verdict being taken instead. That lane also ran for the
  first time on this branch: while it was stacked on a non-default base, no
  `pull_request` workflow fired at all, so nothing had ever exercised the
  Windows matrix. Its first run found a real defect in this plan's own gate —
  `read_sources` formatted a repository-relative `Path`, so Windows rendered
  `crates\rstest-bdd\src\prelude.rs` in a message whose test, documentation and
  every committed path use `/`. `bca7367c` renders those four diagnostic sites
  with `as_posix()`, which is what the other scripts already do, and the
  already-failing test pins the form on every platform. The restack is
  therefore the first revision at which the Windows lanes could report, and
  their verdict counts from `bca7367c` onward.

- 2026-11-23, hosted verdict at `cc73c888`. CI run `45163896903` reports
  `success` for all three lanes. The Linux lane ran the all-features,
  all-targets nextest suite with doctests and pytest, the standalone fixture
  lockfile check, the published-GPUI fixture and end-to-end scenario, the
  coverage measurement and the publish dry run; both Windows lanes passed their
  own feature sets. CodeScene and Gecko Security report success at the same
  head. The one part of `make test` that the hosted lane does not run is the
  separate `rstest-bdd --all-targets --no-default-features` leg. Three local
  attempts at it reached 735 of 736 started tests with every result green, and
  each stopped on the trybuild family: its first preparation mode finished (the
  third attempt reports 1053.3 s) and its second nested command then met the
  family's own 540 s wall-clock bound while the shared admission pool held 80
  to 370 requests from other repositories. The trybuild family is therefore the
  only suite without a green `--no-default-features` verdict. The configuration
  itself is not unexercised: the hosted Windows strict-compile-time-validation
  lane builds `with-default-features: false`, and the trybuild family passes
  there.

- 2026-11-23, scope decision recorded. The repository owner directed this
  candidate to be delivered through merge with the overage recorded rather than
  waived. DL-16 already states the measurement and the escalation; this entry
  records the direction that answers the reviewer's request for the plan
  approver's acceptance, and it is not a claim that the 50-file and 1,200-line
  tolerances were met.
