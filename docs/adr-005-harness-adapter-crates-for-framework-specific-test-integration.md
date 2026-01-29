# Architectural decision record (ADR) 005: Introduce harness adapter crates for framework-specific test integration

## Status

Accepted 2026-01-25: Introduce a small, framework-agnostic harness abstraction in
`rstest-bdd-harness`, and ship official harness adapters as separate crates:
`rstest-bdd-harness-tokio` and `rstest-bdd-harness-gpui`. This keeps Tokio and
GPUI out of `rstest-bdd`’s dependency graph by default (lighter SBOMs), while
enabling BDD-style tests to execute inside external test harnesses. A Bevy harness
will follow the same pattern.

## Date

2026-01-25.

## Context and problem statement

`rstest-bdd` needs to support BDD-style scenarios that run inside framework-
specific test harnesses, while still allowing users to write step functions that
receive harness-provided fixtures (for example, GPUI’s `TestAppContext`).

Two existing pressures make a “just add more features to the main crate” approach
increasingly unattractive:

- Tokio support introduces a large, visible dependency surface, even when many
  consumers do not need async tests.
- UI frameworks such as GPUI (and soon Bevy) bring heavy transitive dependencies,
  platform backends, and build-time costs that most `rstest-bdd` users should not
  pay unless they opt in.

We also need an architecture that scales beyond one-off integrations. GPUI is not
the last harness we will need, and Bevy is already on the near horizon.

The decision is whether to keep integrating harnesses directly into the primary
`rstest-bdd` crates, or to introduce a clean adapter boundary and publish harness
integrations as separate crates.

## Decision drivers

- Keep `rstest-bdd`’s default dependency footprint small for the majority of
  users.
- Avoid pulling Tokio into `rstest-bdd`’s dependency graph to reduce SBOM size
  and transitive supply-chain surface.
- Support multiple harnesses (Tokio, GPUI, Bevy, and others) without accumulating
  framework-specific conditionals in core code generation and runtime logic.
- Maintain a coherent, well-documented, and stable integration surface for
  harness authors.
- Preserve a smooth user experience for test authors, including clear, explicit
  opt-in behaviour.

## Options considered

### Option A: Monolithic integrations in `rstest-bdd`

Continue implementing Tokio, GPUI, and future harnesses directly in the existing
`rstest-bdd` crates, typically behind feature flags.

This keeps the crate count small, but moves complexity into conditional
compilation, increases maintenance cost in the core, and risks dependency creep.

### Option B: Official harness adapter crates (chosen)

Introduce a small “harness” crate that defines the adapter traits and types:

- `rstest-bdd-harness`: traits and shared types only (no Tokio, no GPUI).
- `rstest-bdd-harness-tokio`: Tokio-specific integration (depends on Tokio).
- `rstest-bdd-harness-gpui`: GPUI-specific integration (depends on GPUI).
- Future: `rstest-bdd-harness-bevy` (depends on Bevy).

Update `rstest-bdd` macros to generate calls into a selected harness adapter,
rather than hardcoding framework logic into core macro expansion.

### Option C: Third-party harness ecosystem only

Publish only the harness traits in a small crate, and leave Tokio, GPUI, and Bevy
integrations to the community.

This minimizes official maintenance, but risks fragmentation, inconsistent UX,
and duplicated work, especially for “obvious” harnesses like Tokio.

| Topic                             | Option A: Monolithic | Option B: Official harness crates | Option C: Third-party only |
| --------------------------------- | -------------------- | --------------------------------- | -------------------------- |
| Default dependency footprint      | Higher               | Low                               | Low                        |
| SBOM size for non-async users     | Worse (often)        | Better                            | Better                     |
| Scaling to multiple harnesses     | Poor                 | Good                              | Variable                   |
| Integration UX consistency        | High                 | High                              | Low                        |
| Maintenance burden in core crates | High                 | Medium                            | Low (but external churn)   |
| Release coordination complexity   | Low                  | Medium                            | Low (for core)             |
| Risk of ecosystem fragmentation   | Low                  | Low                               | High                       |

_Table 1: Trade-offs between integration strategies._

## Decision outcome / proposed direction

Adopt Option B.

We will introduce a harness abstraction layer and move framework-specific
integrations into dedicated crates:

- `rstest-bdd-harness` will define the stable adapter interface used by macro
  expansions and scenario execution.
- `rstest-bdd-harness-tokio` will provide Tokio async test support without
  bringing Tokio into `rstest-bdd`’s dependencies.
- `rstest-bdd-harness-gpui` will provide GPUI integration, including the ability
  to inject `TestAppContext` into BDD step execution.
- A Bevy harness crate will be added next, reusing the same adapter interface.

This approach keeps core crates lean, makes harness selection explicit, and gives
us a repeatable pattern for future harness adapters.

For screen readers: The following code block shows how a user opts into a harness
adapter in a scenario definition.

```rust,no_run
use rstest_bdd::scenario;

// Example harness adapters (names illustrative; the concrete API is defined
// by the harness crate).
use rstest_bdd_harness_tokio::TokioHarness;
use rstest_bdd_harness_gpui::GpuiHarness;

#[scenario(harness = TokioHarness, feature = "Async user flows")]
fn async_user_flow() {
    // Steps can be async; TokioHarness drives execution.
}

#[scenario(harness = GpuiHarness, feature = "UI behaviour")]
fn ui_behaviour() {
    // Steps can request GPUI fixtures (for example, `&TestAppContext`).
}
````

## Goals and non-goals

### Goals

- Keep `rstest-bdd`'s default dependency graph free of Tokio, GPUI, Bevy, and
  other heavyweight framework crates.
- Provide first-party harness adapters for Tokio and GPUI with consistent UX,
  documentation, and support guarantees.
- Make future harness integrations (for example, Bevy) incremental rather than
  architectural rewrites.
- Preserve (and ideally improve) current async support and step ergonomics.

### Non-goals

- Provide dynamic "plug-in discovery" at runtime or compile time. Harness
  selection remains explicit, by type path or configuration in macro attributes.
- Standardize every harness' fixture naming and semantics across frameworks.
  Harness crates may define their own fixture conventions, as long as they are
  documented and stable.
- Solve unrelated fixture design problems that are not driven by harness
  integration (for example, redesigning step parameter extraction).

## Migration plan

1. Introduce `rstest-bdd-harness`

   - Define the harness traits and minimal shared types.
   - Provide a default "std" harness implementation (no async runtime) used when
     no harness is specified.

2. Update macro expansion to target the harness abstraction

   - Generate scenario tests that call the selected harness adapter (defaulting
     to the std harness).
   - Keep the generated API source-compatible where feasible, and provide clear
     deprecation paths when not.

3. Extract Tokio integration into `rstest-bdd-harness-tokio`

   - Move Tokio-specific runtime mode logic and any Tokio-bound helpers into the
     new crate.
   - Ensure async steps and scenarios continue to work with Tokio when users opt
     in via the harness crate.

4. Implement `rstest-bdd-harness-gpui`

   - Wrap scenario execution inside the GPUI test harness.
   - Inject `TestAppContext` (and other GPUI test fixtures as needed) into the
     step execution context.

5. Document and stabilize

   - Add a "Harness adapters" chapter describing: selection, configuration,
     fixture injection, and portability expectations.
   - Provide cookbook examples for Tokio and GPUI, plus a "template" for future
     harness authors.

6. Add `rstest-bdd-harness-bevy`

   - Reuse the same traits, and validate that the abstraction fits another
     real-world harness without retrofitting.

## Known risks and limitations

- More crates increase publishing and release coordination overhead, especially
  when changes span macros, harness traits, and adapters.
- A too-opinionated harness trait can paint us into a corner; a too-generic trait
  can leak complexity into every adapter. We need to keep the trait surface small
  and evolve it cautiously.
- Users may find harness selection confusing without strong documentation and
  examples, particularly when troubleshooting async execution and fixture
  injection.
- Harness adapters can accidentally diverge in semantics (for example, panic
  handling, timeouts, or ordering guarantees). We need explicit behavioural
  contracts in each harness crate's documentation.

## Architectural rationale

This decision extends an existing design direction: keep the `rstest-bdd` core
small and stable, and push framework-specific logic to the edges. Harness adapters
formalize “the edge” as a public, testable interface.

Separating Tokio into `rstest-bdd-harness-tokio` reduces the default transitive
dependency surface and SBOM weight for users who do not need async tests, while
still allowing a polished, supported path for those who do.

Separating GPUI (and later Bevy) into dedicated harness crates avoids imposing UI
framework build and platform costs on consumers who are not doing UI testing, and
provides a clear home for framework-specific fixtures and execution semantics.

```text
::contentReference[oaicite:0]{index=0}
```
