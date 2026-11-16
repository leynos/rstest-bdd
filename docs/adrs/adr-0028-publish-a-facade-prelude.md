# Architectural decision record (ADR) 0028: Publish a prelude and a macros edge from the `rstest-bdd` facade

## Status

Accepted (2026-11-16): `crates/rstest-bdd` re-exports the procedural macros from
`rstest-bdd-macros` unconditionally and publishes `rstest_bdd::prelude`, a
documented subset of its own crate root.

## Date

2026-11-16.

## Context and problem statement

Roadmap entry 11.2.2 asks for a public prelude "so examples can import one
predictable module without hiding the underlying crates". Before this decision
the facade re-exported nothing from the macro crate: `rstest_bdd` exposed the
runtime types, the attributes lived only in `rstest_bdd_macros`, and every
consumer had to name both. Three READMEs documented
`use rstest_bdd::{given, when, then, …}`, and none of those lines compiled. The
documentation had been wrong for several releases because no gate compared it
with the crate root.

Two constraints shape any answer. First, `crates/rstest-bdd-macros` must never
depend on `crates/rstest-bdd`, in any dependency table including
dev-dependencies, because `cargo package` resolves development dependencies and
a cycle would block publication even when the runtime graph is acyclic. Second,
the v0.6.1 line is semver-compatible with v0.6.0, so the change must be
additive.

The question is how the facade exposes the macros and a prelude while keeping
ADR-004's layering, adding no dependency cycle, and not making `rstest` a
public dependency of this crate.

## Decision drivers

- A consumer writes one import for step writing: attributes, scenario macros,
  state, capture types, and the step context.
- The macro crate must not gain an edge back to the runtime crate.
- The prelude must not become a second public API that can drift from the
  crate root.
- Additions to a glob-imported module can break consumers, so the module needs
  a written admission bar and a changelog obligation.
- A consumer must keep ownership of its own `rstest` version.

## Requirements

### Functional requirements

- `use rstest_bdd::prelude::*;` supplies every name a step definition needs.
- Every prelude name also resolves at the `rstest_bdd` crate root.
- The prelude's contents are listed in `docs/users-guide.md` under a gate that
  fails when the list and the code disagree.
- A crate that names only `rstest-bdd` and `rstest` compiles and runs a
  scenario.

### Technical requirements

- No member of the dependency cycle prohibition may be relaxed.
- The change must build under the existing CI feature lists without editing
  either hand-maintained list.
- The documented build-cost increase must be measured, not estimated.

## Options considered

### Option A: a `prelude` module alone

Rejected. The READMEs would stay wrong, and the prelude would create names
reachable only through the module, which is exactly the "second API surface"
this decision tries to avoid.

### Option B: a crate-root re-export alone

Rejected. It leaves the roadmap's explicit "public prelude" unmet, and it would
need a roadmap amendment to be adopted at all.

### Option C: a feature-gated macros re-export

Rejected on four independent defects. `doc(cfg)` is nightly-only while the
`Makefile` passes `--cfg docsrs -D warnings` on a stable toolchain, so the
gated re-export could not be annotated and `make lint` fails with E0658. No
blocking gate ever builds with the feature off, because `CARGO_FLAGS` is
`--all-features` and the only no-default-features lane is Windows with
`continue-on-error: true`. The failure mode for a `default-features = false`
consumer is `cannot find attribute 'given'`, which mentions no feature. And a
gated prelude means different things in different builds, which contradicts
"one predictable module".

### Option D: unconditional re-export plus a prelude subset

Accepted.

| Topic                 | Option C (gated)                     | Option D (unconditional)   |
| --------------------- | ------------------------------------ | -------------------------- |
| Feature-off build     | attributes missing, misleading error | builds; attributes present |
| Documentation         | needs nightly `doc(cfg)` badge       | plain rustdoc              |
| CI feature lists      | two hand-maintained lists to edit    | no edit                    |
| Prelude meaning       | differs per build                    | one module in every build  |
| First-five-minutes UX | `cargo add` plus prelude fails       | works                      |

_Table 1: the gated and unconditional re-export compared._

## Decision outcome

`crates/rstest-bdd` takes an unconditional normal dependency on
`rstest-bdd-macros`, re-exports the step attributes and the scenario macros at
its crate root, and publishes `rstest_bdd::prelude` as a strict subset of that
root. The edge points from the runtime crate to the macro crate; the reverse
edge remains forbidden and a unit test reads the macro crate's manifest to
assert that it holds for normal, build, and development dependencies alike.

The founding thirteen namespace bindings are `given`, `when`, `then`,
`scenario`, `scenarios`, `ScenarioState` (derive and trait), `StepArgs` (derive
and trait), `StepContext`, `Slot`, `StepError`, and `StepResult`.
`ScenarioState` and `StepArgs` each name a trait and a derive macro, which Rust
keeps in separate namespaces, so one import brings both halves into scope.

An item enters the prelude only if a step signature, a `#[derive]`, or an
attribute in `examples/*/tests/*.rs` needs it, or if it is required to make
such an item usable (a trait whose derive is exported, or an error type named
in an exported alias). The prelude never introduces a name that is not also
reachable at the `rstest_bdd` crate root. Removing or renaming a prelude item
is a breaking change. Additions are permitted within a minor line but must be
recorded in `docs/CHANGELOG.md` and noted as potentially requiring
disambiguation for glob importers. The founding bindings are grandfathered by
the roadmap, which names `StepResult`, `Slot`, and `ScenarioState` directly
even though no example uses them yet; the bar governs everything after.

`rstest::fixture` and `rstest::rstest` are deliberately not re-exported. The
re-export would not work: `rstest_macros` resolves its runtime path through
`proc_macro_crate` and panics during expansion unless `rstest` is a direct
dependency of the consuming crate, so the manifest entry cannot be removed
either way. More decisively, a dependency on `rstest` would put the consumer's
`rstest` version under this crate's control, gating every consumer's `rstest`
upgrade on a release here; generated code emits `#[rstest::rstest]` against the
consumer's own dependency, so the consumer must own that version. `rstest` is
pre-1.0, so every minor bump would become a breaking change for this crate as
well.

The harness traits are not re-exported either. `rstest-bdd-harness` is only a
development dependency of the facade, so `HarnessAdapter` and `AttributePolicy`
are not reachable at the crate root and cannot enter a prelude that is a strict
subset of it. Adapters are chosen per scenario rather than imported for step
writing, and ADR-005a keeps the adapter contracts in their own crate.

Three layers keep the prelude from drifting from its documentation: a Python
comparator in `make lint` (`scripts/check_prelude_exports.py`) that reconciles
the `pub use` leaf names, the users-guide item table, and the READMEs' braced
imports; a doctest in `prelude.rs` that names every exported item explicitly
after a glob import; and an enforced documentation region that makes the
prelude section's fenced examples compiled examples rather than prose.

## Known risks and limitations

- The repository has no public-API gate: neither `cargo-public-api` nor
  `cargo-semver-checks` is installed, and `cargo-semver-checks` would not see
  this hazard anyway, because the Cargo Book classifies adding a public item as
  minor while noting that it can break glob importers. The admission bar and
  the changelog note are proportionate mitigation, not enforcement.
- The unconditional edge costs build time for every consumer of `rstest-bdd`,
  including those that never write a step. Measured on a 24-core machine: +30
  normal-dependency crates (99 to 129), +40 CPU-seconds, and roughly double the
  cold wall time (10.09 s to 19.9 s). For `rstest-bdd-harness-gpui` the delta
  is +25 crates against a 787-package graph, about 3%.
- A `default-features = false` build still carries the macros, because there is
  no feature to turn them off and no blocking lane that would notice their
  absence.
- Glob imports remain a forwards-compatibility hazard for every consumer, on
  the Cargo Book's own terms. Adding a prelude item can require an existing
  consumer to disambiguate a name.
- Because `rstest` stays the consumer's dependency, this release is built and
  tested against `rstest` `^0.26.1`, and the users' guide states that range. A
  consumer that renames its `rstest` dependency still fails with a bare
  `unresolved import rstest` from code it never wrote; resolving the generated
  path through `proc_macro_crate` is the fix, and it is a behavioural change
  deferred past v0.6.1.

## Architectural rationale

ADR-004 rejected moving the policy types into the macro crate because it
"inverts layering; ties runtime to macros". This decision does not move any
policy definition: `rstest-bdd-policy` remains the single owner, the macro
crate still imports the types directly from it, and the new edge runs from the
facade to the macros rather than back. The layering property ADR-004 protected
is that the macro crate never learns about the runtime crate, and that property
still holds and is now tested.

A facade that re-exports an implementation detail is only honest if the
re-export is total and flat. That is why the prelude is a strict subset of the
crate root rather than a curated parallel list: there is one public API to keep
coherent, and the drift gate compares documentation against that API rather
than against a second handwritten inventory.

## References

- [Roadmap 11.2.2](roadmap.md).
- [Users' guide: importing rstest-bdd](users-guide.md#importing-rstest-bdd).
- [ADR-004: policy crate](adr-004-policy-crate.md).
- [ADR-005a: harness adapter crates](adr-005a-harness-adapter-crates-for-framework-specific-test-integration.md).

## Amendments

### 2026-11-16: the facade re-export is total

The decision outcome requires a "total and flat" re-export, and the
implementation review found it was not: `step_library` was reachable only from
`rstest-bdd-macros`, so declaring a scoped vocabulary forced a consumer to name
the implementation crate. The crate root now re-exports it. It does not enter
the prelude: the entry rule above admits only an item that a step signature, a
`#[derive]`, or an attribute in the examples needs, and no example declares a
library yet.
