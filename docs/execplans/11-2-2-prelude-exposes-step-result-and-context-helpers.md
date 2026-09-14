# Expose a public prelude for common integration imports (roadmap 11.2.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must be
kept up to date as work proceeds.

Status: DRAFT

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
prelude plus their harness crate, and docs list the exported items."* Its stated
prerequisite, 11.2.1, is complete (`docs/roadmap.md:1023-1037`).

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

| Crate | Path | Role | Depends on (workspace crates only) |
| --- | --- | --- | --- |
| `rstest-bdd-policy` | `crates/rstest-bdd-policy` | Shared constants and policy enums | none |
| `rstest-bdd-patterns` | `crates/rstest-bdd-patterns` | Step-pattern parsing | none |
| `rstest-bdd-harness` | `crates/rstest-bdd-harness` | Harness adapter contracts | none |
| `rstest-bdd-macros` | `crates/rstest-bdd-macros` | The procedural macros | patterns, harness, policy |
| `rstest-bdd` | `crates/rstest-bdd` | Runtime: registry, `StepContext`, state | patterns, policy |
| `rstest-bdd-harness-tokio` | `crates/rstest-bdd-harness-tokio` | Tokio adapter | harness |
| `rstest-bdd-harness-gpui` | `crates/rstest-bdd-harness-gpui` | GPUI adapter | harness, **rstest-bdd** |

*Table: workspace crate dependency edges as they stand before this plan.*

The critical fact is the one that is **absent**: `rstest-bdd-macros` does not
depend on `rstest-bdd`, and `rstest-bdd` does not depend on `rstest-bdd-macros`
as a normal dependency. `rstest-bdd` lists it only under `[dev-dependencies]`
(`crates/rstest-bdd/Cargo.toml:48`, with `features = ["compile-time-validation"]`).

That separation is deliberate and documented. `docs/developers-guide.md:387-393`
states the policy:

> Use the existing dependency-load-bearing crates before introducing or widening
> edges. `rstest-bdd-patterns` owns shared pattern parsing, `rstest-bdd-policy`
> owns shared runtime and attribute-policy classification, and
> `rstest-bdd-harness` owns adapter contracts and test-staging helpers. The
> procedural macro crate may depend on those shared crates, but it must not
> depend on `rstest-bdd`; macro/runtime integration tests live under `rstest-bdd`
> instead.

`docs/adr-004-policy-crate.md` records why: the macro crate needed
`RuntimeMode` and `TestAttributeHint`, could not depend on the runtime crate,
and had been keeping a duplicate copy that drifted. The policy crate exists to
be the shared floor beneath both.

Note carefully what that policy forbids and what it does not. It forbids
`rstest-bdd-macros -> rstest-bdd`. It says nothing about
`rstest-bdd -> rstest-bdd-macros`, which points the other way and therefore
creates no cycle. This plan proposes that second edge. Because the policy text
is currently phrased as a single rule about the macro crate, the plan must
amend `docs/developers-guide.md` to state both directions explicitly, so the
next reader does not have to re-derive that the new edge is legal.

There is a second, subtler rule in the same section
(`docs/developers-guide.md:378-385`):

> Publishable first-party crates must keep their package-time dependency graph
> acyclic across normal, build, and development dependencies. `cargo package`
> resolves development dependencies while preparing a crate, so a dev-dependency
> cycle can block a live release even when the runtime dependency graph is
> acyclic.

`rstest-bdd` already dev-depends on `rstest-bdd-macros`, so the publication
order already places the macro crate first. Promoting that edge from a
dev-dependency to a normal dependency does not reorder anything. `make
publish-check` runs `lading publish`, which derives release order from the
graph, and is the gate that proves this.

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
marker written on a *parameter* of a function that is already being rewritten
by `#[given]`, `#[when]`, or `#[then]`. Those macros see it as raw attribute
syntax in their input token stream and strip it during expansion. The
classification code is
`crates/rstest-bdd-macros/src/codegen/wrapper/args/classify/harness_context/mod.rs:76`
(`classify_harness_context`), reached via `extract_flag_attribute(arg,
"harness_context")` at line 81.

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
is no `cargo-public-api` and no `cargo-semver-checks` anywhere in the `Makefile`
or `.github/workflows/`. Semver discipline here is currently a matter of
review, which is directly relevant to a change whose entire purpose is to add
public surface.

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
227/237, 253/263, 271/281). None of it compiles today: `crates/rstest-bdd/src/lib.rs`
contains no `pub use rstest_bdd_macros` anywhere. No README is doctested — no
crate does `#[doc = include_str!("../README.md")]` and no Makefile target checks
them — which is how fifteen wrong import lines have sat on the project's front
page unnoticed. This plan makes them true.

**The prelude is a thin, curated re-export of that root.** `rstest_bdd::prelude`
holds the subset a step-writing file actually needs. It never introduces a name
that is not also reachable at the crate root, so there are never two public APIs
to keep coherent.

### The dependency edge

`rstest-bdd` takes `rstest-bdd-macros` as a **normal, unconditional**
dependency. Not optional, and not behind a feature.

This is cycle-free, as established in "Context and orientation":
`rstest-bdd-macros` reaches only `rstest-bdd-patterns`, `rstest-bdd-harness`,
and `rstest-bdd-policy`, none of which reaches `rstest-bdd`. The edge already
exists in dev position, so the graph shape is proven. `lading.toml:4-12` already
lists `rstest-bdd-macros` before `rstest-bdd` in the publish order, so no
release-process change is needed.

The review's first instinct was a default-on `macros` feature, copying serde's
`derive`. That was rejected for four independent reasons, each of which is
sufficient on its own:

1. **It would break `make lint` on the first run.** Annotating a gated
   re-export requires `#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]`, and
   `doc(cfg)` is unstable. serde guards it with
   `#![cfg_attr(docsrs, feature(doc_cfg))]` and relies on docs.rs building on
   nightly. Here `rust-toolchain.toml` pins `channel = "stable"` and
   `Makefile:27` sets `RUSTDOC_FLAGS ?= --cfg docsrs -D warnings`
   unconditionally, so the `docsrs` cfg is **always on, on stable**. A grep for
   `doc_cfg` or `cfg_attr(docsrs` across `crates/**/*.rs` returns zero hits. The
   annotation would fail with E0658 immediately; omitting it would render the
   macros on docs.rs with no feature badge.
2. **Nothing in the repository can observe the feature switched off.**
   `Makefile:28` sets `CARGO_FLAGS ?= --workspace --all-targets --all-features`,
   and the Linux CI lane runs with `all-features: 'true'`. The only
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
   roadmap's stated goal of "one predictable module". `rstest_bdd::prelude::given`
   would mean different things in different configurations.

`rstest` itself settled this the same way: `rstest_macros` is a mandatory,
ungated dependency of `rstest`, re-exported unconditionally
(`pub use rstest_macros::fixture;`). `cucumber`, the closest competitor, does
gate its codegen crate, but puts it in `default` — and pays the `--no-default-features`
hazard that item 3 describes. The unconditional edge is simpler and strictly
safer.

The build-cost objection is weak here: `crates/rstest-bdd/Cargo.toml:19-38`
already pulls `tokio`, `i18n-embed`, `rust-embed`, `fluent`, `gherkin`, `regex`,
and `inventory` unconditionally, and `syn`, `quote`, and `proc-macro2` are
already resolved in `rstest-bdd`'s graph via its existing derive-using
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
cost. That is a worthwhile follow-up but it is not in scope here; it is filed in
"Follow-up work this plan deliberately does not do".

### What goes in the prelude

Founding membership is twelve items. Every one is either named by the roadmap
or required to make a named item usable.

| Item | Kind | Source | Why |
| --- | --- | --- | --- |
| `given` | attribute macro | `rstest-bdd-macros` | used by all four examples |
| `when` | attribute macro | `rstest-bdd-macros` | used by all four examples |
| `then` | attribute macro | `rstest-bdd-macros` | used by all four examples |
| `scenario` | attribute macro | `rstest-bdd-macros` | used by all four examples |
| `scenarios` | function-like macro | `rstest-bdd-macros` | the bulk-binding counterpart to `scenario` |
| `ScenarioState` | derive macro | `rstest-bdd-macros` | named by the roadmap |
| `ScenarioState` | trait | `rstest-bdd` (`state`) | named by the roadmap; supplies `.reset()` |
| `Slot` | struct | `rstest-bdd` (`state`) | named by the roadmap |
| `StepResult` | type alias | `rstest-bdd` | named by the roadmap |
| `StepError` | enum | `rstest-bdd` | `StepResult`'s default error parameter; unusable without it |
| `StepContext` | struct | `rstest-bdd` (`context`) | carries the five harness-context helper methods the roadmap names |
| `StepArgs` | derive macro + trait | both crates | trait/derive pair; see the pairing rule below |

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
  "custom step-execution plumbing outside the usual macros". Generic name, named
  by users approximately never.
- The reporting and skip-assertion macros (`skip!`, `assert_step_ok!`,
  `assert_step_skipped!`, and the `reporting` module). These are diagnostics
  surfaces, not step-authoring surfaces.
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
halves from `crate::` produces `error[E0252]: the name 'ScenarioState' is
defined multiple times`. The same applies to `StepArgs`, whose trait is bound at
`crates/rstest-bdd/src/lib.rs:112` and whose defining module is
`crate::step_args`.

### The marker attribute, and why it is not exported

Roadmap 11.2.2 asks the prelude to expose "marker attributes from 11.2.1".
As established in "Context and orientation", `#[harness_context]` is inert
syntax with no item behind it, so there is nothing to re-export. The review
considered promoting it to a real `#[proc_macro_attribute]` for discoverability
and rejected that decisively: the Rust Reference restricts attribute macros to
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
surfaces on hover. A note in the prelude's module documentation and a row in the
users-guide table reading "no import required" cover the remaining discovery
paths.

### The restated finish line

The roadmap's finish line — *"compile tests prove examples import only the
prelude plus their harness crate"* — cannot be met as literally worded, and not
because of anything this plan does. Five separate dependencies are structurally
unavoidable:

1. **`rstest`.** `rstest_macros` resolves its own runtime path with
   `proc_macro_crate::crate_name("rstest").expect("rstest is present in
   `Cargo.toml` qed")`. If `rstest` is absent from the *consuming* manifest,
   `#[fixture]` panics during macro expansion. Re-exporting `rstest::fixture`
   from the prelude would therefore not remove the manifest entry; it would only
   add `rstest` as a public dependency of `rstest-bdd`, making every `0.x` bump
   of a pre-1.0 crate a breaking change here.
2. **`rstest`, again, from the other direction.** Generated code emits
   `#[rstest::rstest]` as a bare relative path
   (`crates/rstest-bdd-macros/src/codegen/scenario/test_attrs.rs:160`), not
   through `proc_macro_crate`. `rstest` is already a de facto public dependency
   of this framework.
3. **`tokio` and `gpui`.** The same function emits `#[tokio::test(flavor =
   "current_thread")]` at line 161 and `#[gpui::test]` at line 164, also as bare
   paths. `examples/tokio-reminders` and `examples/gpui-counter` can never drop
   those.
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
snapshot would capture a hand-written list, which is precisely the artefact that
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
because adding is easy and refusing requires an argument. Six releases later the
prelude is a second crate root and the users-guide table is stale.

This plan writes the bar into `docs/developers-guide.md`:

> An item enters the prelude only if a step signature, a `#[derive]`, or an
> attribute in `examples/*/tests/*.rs` needs it, or if it is required to make
> such an item usable (a trait whose derive is exported, or an error type named
> in an exported alias). The prelude never introduces a name that is not also
> reachable at the `rstest_bdd` crate root. Removing or renaming a prelude item
> is a breaking change. Additions are permitted within a minor line but must be
> recorded in `docs/CHANGELOG.md` and noted as potentially requiring
> disambiguation for glob importers.

The founding twelve are grandfathered by the roadmap, which names `StepResult`,
`Slot`, and `ScenarioState` directly even though no example uses them yet; the
bar governs everything after.

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

This plan proposes one new decision record, **ADR-022**, covering the facade
dependency edge, the prelude admission bar, and the deliberate non-export of
`rstest`. Numbering note: the existing set runs to 021, but `005` is duplicated
(`adr-005-async-step-functions.md` and
`adr-005-harness-adapter-crates-for-framework-specific-test-integration.md`,
indexed in `docs/contents.md` as "ADR 005a"). 022 is the next free number.

Trace links:

```plaintext
RM-11.2.2 -> ADR-022 -> EP-M2 -> crates/rstest-bdd/tests/prelude_surface.rs
RM-11.2.2 -> ADR-022 -> EP-M4 -> examples/*/tests/*.rs compile without rstest-bdd-macros
RM-11.2.2 -> EP-M5 -> scripts/check_prelude_exports.py in `make lint`
RM-P11    -> EP-M1 -> `make publish-check` green; no public item removed
DD-2.7.6.4 -> EP-M6 -> docs/rstest-bdd-design.md §2.7.6.4 marks the prelude delivered
ADR-004   -> ADR-022 -> EP-M1 -> docs/developers-guide.md states both edge directions
ADR-007   -> EP-M3 -> given/when/then rustdoc documents #[harness_context]
```

**Proposed deviation, requiring approval before implementation.** RM-11.2.2's
finish line is not achievable as literally worded, for the five structural
reasons set out in "The restated finish line". This plan amends
`docs/roadmap.md:1040-1041` to the achievable property. Per the ExecPlan
convention this is an upstream change that must be accepted explicitly; it is
recorded in `Decision log` as DL-9 and is the reason this plan's status is
`DRAFT` rather than ready to execute.

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
- **No `doc(cfg)`.** `Makefile:27` sets `RUSTDOC_FLAGS ?= --cfg docsrs -D warnings`
  unconditionally and `rust-toolchain.toml` pins `channel = "stable"`. The
  `doc_cfg` feature is nightly-only. Do not introduce
  `#[cfg_attr(docsrs, doc(cfg(...)))]` or `#![cfg_attr(docsrs, feature(doc_cfg))]`
  anywhere; both fail `make lint` with E0658.
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

- **Scope:** more than 30 files changed, or more than 1,200 net added lines
  across the whole plan.
- **Interface:** any change to an existing public function or type signature.
  Adding new items and adding the `rstest-bdd -> rstest-bdd-macros` edge are
  pre-authorized; nothing else is.
- **Dependencies:** adding any workspace dependency beyond the
  `rstest-bdd -> rstest-bdd-macros` edge this plan authorizes. In particular, do
  not add `syn` to `rstest-bdd`'s dev-dependencies — the drift gate is a Python
  script by design, precisely to avoid that.
- **Build cost:** if `cargo build -p rstest-bdd` cold wall time exceeds 25
  seconds on the reference 24-core machine, or the normal-dependency count
  exceeds 135 crates, stop and escalate. The measured pre-change baseline is
  10.09 s and 99 crates; the predicted post-change figures are ~19.9 s and 129
  crates. See `Risks` R-2.
- **Gate churn:** if `make lint` reports Whitaker findings that cannot be fixed
  without restructuring code outside this plan's scope, stop and escalate.
- **Iterations:** a milestone's tests still failing after four attempts.
- **Roadmap amendment:** if the approver declines the finish-line amendment
  (DL-9), stop. The item cannot be completed honestly without it.
- **Ambiguity:** if the unconditional-versus-feature-gated decision (DL-2) is
  overruled at approval, stop and re-plan Milestones 1 and 4 before proceeding;
  a feature gate changes the CI feature lists, the docs.rs story, and the
  verification plan.

## Risks

- **R-1 — The `macros` gating decision is contested.**
  Severity: medium. Likelihood: medium.
  The design review split. Buzzy Bee measured the cost of the unconditional
  edge and argued for a default-off feature; Doggylump argued the feature's
  failure modes are worse than the cost; the roadmap's "predictable" wording and
  the unavailability of `doc(cfg)` on stable broke the tie toward unconditional.
  Mitigation: the decision is isolated to `crates/rstest-bdd/Cargo.toml` and the
  two `pub use` blocks, so reversing it is a small diff. DL-2 records the full
  argument and the numbers so the approver can overrule on the evidence rather
  than re-derive it.

- **R-2 — Build-cost regression for non-test consumers.**
  Severity: low. Likelihood: high (it is a certainty, the question is size).
  Measured: `rstest-bdd` gains 30 normal-dependency crates (99 → 129), about 40
  CPU-seconds, and roughly doubles cold wall time (10.09 s → 19.9 s) on a
  24-core machine. The new crates are the `cap-std`/`rustix` sandbox and the
  `cargo_metadata`/`toml_edit` manifest reader; `syn`, `quote`, `proc-macro2`,
  `gherkin`, `regex`, and `walkdir` were already present. External users who
  depend on `rstest-bdd` from `[dev-dependencies]` alongside `rstest-bdd-macros`
  pay **nothing**, because they already build the macro crate. The two payers
  are `cargo-bdd` and `rstest-bdd-harness-gpui`; for GPUI users the delta is +25
  crates against a 787-package graph, about 3%.
  Mitigation: accepted, recorded in ADR-022's Known Risks, with two follow-ups
  filed that would remove most of it (see "Follow-up work").

- **R-3 — `proc-macro2/span-locations` becomes unconditional downstream.**
  Severity: low. Likelihood: high.
  `crates/rstest-bdd-macros/Cargo.toml:42` pins
  `proc-macro2 = { features = ["span-locations"] }`. Under the new edge that
  feature unifies for every consumer of `rstest-bdd`, changing `-C metadata` for
  `serde_derive`, `thiserror-impl`, `derive_more`, and every other derive in the
  graph, and making all expansion pay per-token line/column tracking.
  Mitigation: none available without changing the macro crate's needs. Recorded
  so it is not rediscovered as a mystery rebuild. Note that today the feature
  flips on and off depending on whether the macro crate is in the build, which
  causes measurable rebuild thrash; after this change it is stably on, which is
  arguably an improvement.

- **R-4 — Seven committed lockfiles go stale.**
  Severity: medium. Likelihood: high.
  Cargo lockfiles record per-package dependency lists, so adding a normal
  dependency invalidates every lock that mentions `rstest-bdd`. All seven do:
  the root `Cargo.lock`, `crates/rstest-bdd/tests/ui_lints/`,
  `tests/fixtures/published-gpui-e2e/`, `tests/fixtures/published-gpui-0-2-2/`,
  `crates/rstest-bdd/tests/fixtures/feature_addition/`,
  `crates/rstest-bdd/tests/fixtures/rebuild_invalidation/`, and
  `crates/cargo-bdd/tests/fixtures/minimal/`. `make check-fixture-lockfiles`
  will fail until each is refreshed.
  Mitigation: Milestone 1 refreshes all seven as its final step and runs
  `make check-fixture-lockfiles` before committing. The *package set* is
  unchanged — all 30 crates already appear in every fixture lock via the
  dev-dependency path — so `make prefetch-fixture-deps` downloads nothing new
  and the offline mutation lane keeps working.

- **R-5 — The `-C metadata` twin-fixture invariant.**
  Severity: medium. Likelihood: low.
  `crates/rstest-bdd/tests/fixtures/rebuild_invalidation/Cargo.toml:6-19`
  documents a hard requirement that its dependency set stay byte-identical to
  `crates/cargo-bdd/tests/fixtures/minimal` so the two share compiled units,
  "doubling the cold-build cost on CI" otherwise. Both take `rstest-bdd` with
  implicit default features, so an unconditional edge keeps them matched — but
  the invariant is now more fragile.
  Mitigation: add a comment to both manifests naming the new edge, so a future
  contributor adding `default-features = false` to one sees the warning.

- **R-6 — The nested `cargo`-spawning test lane has only 120 seconds of slack.**
  Severity: medium. Likelihood: medium.
  `.config/nextest.toml:5-20` records explicit budget arithmetic —
  `180 + 600×4 + 600×3 = 4380 s` against a 75-minute global timeout. Those
  fixtures shell out to `cargo test --locked --offline`, and each such build now
  serialises the macro crate before the runtime crate. Scaled to the 2-vCPU CI
  runner that is roughly +19 s per cold invocation across about ten
  invocations — potentially +190 s against 120 s of slack on a cold or evicted
  sccache lane. No new compilation units are involved; the fixtures already
  build the macro crate through their own dev-dependencies.
  Mitigation: Milestone 1 measures a cold nested-fixture build before and after
  and records both figures. If the margin is breached, raise `global-timeout` in
  the same commit and say why in the nextest.toml comment block that already
  documents the arithmetic.

- **R-7 — The mutation lane multiplies R-6.**
  Severity: low. Likelihood: medium.
  `.github/workflows/mutation-testing.yml:47` runs `--all-features
  --test-workspace=true`, so the nested fixture builds rerun per mutant.
  Mitigation: watch the first post-merge mutation run; no pre-emptive action.

- **R-8 — E0252 when wiring the prelude.**
  Severity: low. Likelihood: medium.
  Importing both halves of `ScenarioState` or `StepArgs` from `crate::` rather
  than from their defining modules produces "the name is defined multiple
  times".
  Mitigation: stated explicitly in "The E0252 trap" and encoded in Milestone 2's
  red test. The tempting wrong fix is renaming the derive, which would be a
  breaking change; do not.

- **R-9 — Downstream glob ambiguity as the prelude grows.**
  Severity: low. Likelihood: low.
  Per the Cargo Book, adding a public item is a minor change that can
  nonetheless break glob importers. The exposure is a user who writes
  `use rstest_bdd::prelude::*;` alongside another glob binding the same name and
  then uses it; the error (E0659) fires at the use site, not the import.
  Mitigation: the written admission bar, the changelog requirement, and the
  subset invariant. No `prelude::v1`; see "The admission bar" for why.

- **R-10 — The examples become the only prelude consumers, losing coverage of
  the direct-import path.**
  Severity: low. Likelihood: medium.
  After Milestone 4 every example imports through the facade, so no
  user-vantage crate exercises `use rstest_bdd_macros::…` directly.
  Mitigation: Milestone 4 adds a fixture crate under
  `crates/rstest-bdd/tests/fixtures/` that imports the macros directly, keeping
  that entry point covered without weakening the examples.

## Verification plan

This change is a re-export and packaging change. It introduces no arithmetic,
no concurrency, no state machine, and no algorithm over a generated domain.
There is therefore **no invariant warranting property testing, bounded model
checking, or deductive proof**, and this plan deliberately proposes none. Saying
so explicitly is a requirement of the ExecPlan convention; the justification
follows, obligation by obligation.

What the change does introduce is a set of **structural propositions about the
build graph and the public surface** — statements that are either true or false
of the repository at a point in time, with no input domain to quantify over. The
right instrument for those is a compile-time or manifest-time assertion, not a
generator.

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
  control, a typo in the path or a parse returning an empty table would make the
  assertion pass for the wrong reason. This is the mutation the test must
  reject.

**V-2 — The facade resolves for a consumer that does not name the macro crate.**
A crate declaring only `rstest-bdd` (plus `rstest`) can write a complete
scenario through `rstest_bdd::prelude` and run it.

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
  where the partition is the twelve founding items and enumeration is not merely
  practical but total.
- Rationale: the set is small, closed, and fully known. Enumerating it is
  strictly stronger than sampling it.
- Artefact: a doctest in `crates/rstest-bdd/src/prelude.rs` that globs the
  prelude and then names each item in a way that forces resolution — a `let _:
  fn(...)` binding for types, an actual `#[derive]` and `#[given]` use for
  macros. Plus a sibling assertion that each name also resolves at
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
- Evidence: fails if `rstest-bdd-macros` reappears in any `examples/*/Cargo.toml`
  or if any `examples/**/*.rs` names `rstest_bdd_macros`.
- Non-vacuity: the accompanying pytest must include a fixture directory that
  *does* violate the rule and assert the script rejects it. This doubles as the
  negative control for V-2.

### Axioms

Assumed without verification, as third-party contracts:

- `proc_macro_crate::crate_name` reads the consuming crate's manifest and
  returns `FoundCrate::Itself` for the crate under compilation. Relied on by
  V-2. The repository's own handling is at
  `crates/rstest-bdd-macros/src/codegen/mod.rs:187-204`; that handling is
  repository-owned and is exercised by V-2 against a real manifest, which is the
  faithful-boundary requirement.
- `rstest_macros` requires `rstest` in the consuming manifest and panics
  otherwise (`crate_resolver.rs:9`). Relied on by the restated finish line. Not
  verified here; it is read directly from the vendored source and quoted in
  `Decision log` DL-5.
- Rust resolves a trait and a same-named macro in separate namespaces, and a
  single glob import delivers both. Relied on by the `ScenarioState` and
  `StepArgs` pairs. This is language behaviour, corroborated by `serde`'s
  shipped `Serialize` trait plus derive and by `bevy_ecs`'s `Component`. V-3
  exercises it directly, so a mistaken belief here fails the build rather than
  escaping.
- Cargo feature unification is additive within one resolution. Relied on by R-3.

### What is deliberately not verified

No `proptest` strategy, no `kani` harness, and no `verus` proof. There is no
quantified invariant: the prelude's membership is a fixed list of twelve names,
not a domain; the dependency edge is a single boolean fact about one manifest;
the documentation correspondence is an equality between two finite sets that
V-4 compares exhaustively. Generating random inputs would add cost and no
information, and a proof would restate the assumption. Recording this judgement
is itself the obligation; reviewers who disagree should say so at the approval
gate rather than after implementation.
