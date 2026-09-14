# Expose a public prelude for common integration imports (roadmap 11.2.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

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

Founding membership is twelve items. Every one is either named by the roadmap
or required to make a named item usable.

| Item            | Kind                 | Source                   | Why                                                               |
| --------------- | -------------------- | ------------------------ | ----------------------------------------------------------------- |
| `given`         | attribute macro      | `rstest-bdd-macros`      | used by all four examples                                         |
| `when`          | attribute macro      | `rstest-bdd-macros`      | used by all four examples                                         |
| `then`          | attribute macro      | `rstest-bdd-macros`      | used by all four examples                                         |
| `scenario`      | attribute macro      | `rstest-bdd-macros`      | used by all four examples                                         |
| `scenarios`     | function-like macro  | `rstest-bdd-macros`      | the bulk-binding counterpart to `scenario`                        |
| `ScenarioState` | derive macro         | `rstest-bdd-macros`      | named by the roadmap                                              |
| `ScenarioState` | trait                | `rstest-bdd` (`state`)   | named by the roadmap; supplies `.reset()`                         |
| `Slot`          | struct               | `rstest-bdd` (`state`)   | named by the roadmap                                              |
| `StepResult`    | type alias           | `rstest-bdd`             | named by the roadmap                                              |
| `StepError`     | enum                 | `rstest-bdd`             | `StepResult`'s default error parameter; unusable without it       |
| `StepContext`   | struct               | `rstest-bdd` (`context`) | carries the five harness-context helper methods the roadmap names |
| `StepArgs`      | derive macro + trait | both crates              | trait/derive pair; see the pairing rule below                     |

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

- **Scope:** more than 30 files changed, or more than 1,200 net added lines
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
  Mitigation: accepted, recorded in ADR-022's Known Risks, with two follow-ups
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
  nextest.toml comment block that already documents the arithmetic.

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
  where the partition is the twelve founding items and enumeration is not
  merely practical but total.
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

## Plan of work

Seven milestones. Each ends in a repository state that builds, passes the
gates, and is safe to stop at. Run `make check-fmt && make lint && make test`
sequentially at every milestone boundary — never in parallel, because the build
cache is shared and sequential runs are what make it useful — and commit.

### Milestone 0 — orientation and green baseline

**Identifier:** EP-M0. **No code changes.**

Confirm the tree is clean and the gates pass before touching anything, so that
any later failure is attributable. Capture the two baseline measurements that
`Tolerances` and R-6 depend on:

```bash
cargo tree -p rstest-bdd --prefix none -e normal | sort -u | wc -l
# expect: 99

env -u RUSTC_WRAPPER CARGO_TARGET_DIR=$(mktemp -d) \
  /usr/bin/time -f '%e wall %U user %S sys' cargo build -p rstest-bdd
# expect: ~10.1 s wall on a 24-core machine
```

Record both in `Progress`. They are the numbers the build-cost tolerance is
measured against.

**Acceptance evidence:** `make check-fmt && make lint && make test` all green
on an unmodified tree, with the log written to
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
   documentation and its doctest, which globs the prelude and uses all twelve
   items. Add `pub mod prelude;` to `lib.rs`. Run
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

**Identifier:** EP-M6. **Discharges:** DD-2.7.6.4, ADR-022, DG-DEP.

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
   delivered sentence, naming roadmap 11.2.2 and cross-linking ADR-022.
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
7. **`docs/adr-022-*.md`** — write it, using the template at
   `docs/documentation-style-guide.md:418-425`. Cover the facade edge and its
   reconciliation with ADR-004 (whose options table at line 36 rejected an
   option on the grounds that it "ties runtime to macros"), the admission bar,
   the deliberate non-export of `rstest`, and Known Risks naming the absent
   public-API gate and the measured build cost.
8. **`docs/contents.md`** — index ADR-022.
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

1. Amend `docs/roadmap.md:1040-1041` to the achievable finish line, per DL-9,
   and tick 11.2.2 as done with a completion note recording what shipped and
   what was deliberately deferred.
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
   entry.
2. **Sever `rstest-bdd-harness-gpui -> rstest-bdd`.** That crate uses exactly
   one item from the runtime crate, `rstest_bdd::panic_message`
   (`crates/rstest-bdd-harness-gpui/src/gpui_harness/mod.rs:40`). Moving the
   helper into `rstest-bdd-harness` would remove the edge and eliminate most of
   R-2's cost for GPUI users.
3. **Audit `cargo_metadata` in `rstest-bdd-harness`'s normal dependencies.** It
   drags `semver`, `camino`, `cargo-platform`, and `serde_json` into the macro
   crate's graph. If it is test-support-only, moving it behind a feature would
   cut a meaningful slice off R-2 independently of this work.
4. **Complete the datatable story in the prelude.** Ship `DataTable` and
   `DataTableRow` together with `rstest_bdd::datatable::Rows` and
   `DataTableError`, or leave all four out. This plan leaves them out.
5. **Make the examples copy-pasteable.** Every example carries
   `#[rstest_bdd_test_macros::allow_fixture_expansion_lints]` from an
   unpublished crate, so a user copying `examples/todo-cli/tests/todo.rs` off
   GitHub today gets
   `E0433: failed to resolve: use of undeclared crate rstest_bdd_test_macros`.
   That predates this plan but the prelude promotes the examples to the proof
   of the framework's ergonomics, which makes it more visible.
6. **Add a `--no-default-features` gate.** The only lane that builds without
   default features is Windows, and its coverage step carries
   `continue-on-error: true`, so it cannot red the build. `diagnostics` remains
   a default feature after this change even though `macros` does not exist.
7. **Enforce ADR number uniqueness.** `005` is duplicated because nothing
   checks.

## Decision log

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
  this. Date/Author: 2026-09-14, planning agent.

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
  cost. Date/Author: 2026-09-14, planning agent.

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

- **DL-9 — PROPOSED DEVIATION, AWAITING APPROVAL: amend RM-11.2.2's finish
  line.** The finish line "compile tests prove examples import only the prelude
  plus their harness crate" is unachievable for five structural reasons, none
  caused by this plan: `rstest` must be in the consuming manifest or
  `#[fixture]` panics; generated code emits bare `#[rstest::rstest]`,
  `#[tokio::test]`, and `#[gpui::test]` paths; every example carries
  `#[rstest_bdd_test_macros::allow_fixture_expansion_lints]` from an
  unpublished crate; and `examples/todo-cli/tests/cli.rs:7` needs
  `rstest_bdd_harness::binary_test_support`. The proposed replacement is: *no
  example crate names any `rstest-bdd-*` crate that the `rstest-bdd` facade
  re-exports* — concretely, `rstest-bdd-macros` appears in no example manifest
  and no example source file. That property is compiler-enforced in one
  direction and script-enforced in the other. Impact:
  `docs/roadmap.md:1040-1041`. Options: amend (recommended); or ship and record
  the shortfall in the completion note; or expand scope to remove
  `rstest-bdd-test-macros` from the examples, which is follow-up 5 and does not
  address `rstest`, `tokio`, or `gpui` in any case. **Status: BLOCKED pending
  explicit acceptance.** Date/Author: 2026-09-14, planning agent.

## Progress

- [ ] EP-M0 — orientation and green baseline.
- [ ] EP-M1 — dependency edge, crate-root re-export, lockfile refresh.
- [ ] EP-M2 — the prelude module.
- [ ] EP-M3 — marker documentation and the missing trybuild fixture.
- [ ] EP-M4 — example migration and the two fixture crates.
- [ ] EP-M5 — the drift gate and the users-guide table.
- [ ] EP-M6 — remaining documentation and ADR-022.
- [ ] EP-M7 — roadmap amendment, full gates, pull request.

Nothing is implemented. This plan is `DRAFT` and additionally `BLOCKED` on
DL-9. Do not begin Milestone 1 until both the plan and the finish-line
amendment are approved.

## Surprises & discoveries

Recorded during planning, before any implementation.

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

## Outcomes & retrospective

Not yet started. To be completed at EP-M7.

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
- EP-M7: the full gate suite, with log paths under `/tmp/`.

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
- `crates/rstest-bdd/tests/fixtures/prelude_only/`
- `crates/rstest-bdd/tests/ui_macros/harness_context_without_step.rs`
- `scripts/tests/test_check_prelude_exports.py`
- `scripts/tests/test_check_example_imports.py`

## Revision notes

- 2026-09-14, initial draft. Written after a six-lens design review whose
  findings overturned three of the six draft decisions: the macro re-export
  became unconditional rather than feature-gated (DL-2), re-exporting
  `rstest::fixture` was dropped as ineffective rather than merely costly
  (DL-3), and the `insta` snapshot was replaced by a Python comparator (DL-5).
  The plan is `BLOCKED` on DL-9, the finish-line amendment, which must be
  accepted before Milestone 1 begins.
