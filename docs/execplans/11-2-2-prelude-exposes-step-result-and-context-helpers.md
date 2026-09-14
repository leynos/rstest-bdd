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

<!-- Sections below are completed after the design review concludes. -->
