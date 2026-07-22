# Architectural decision record (ADR) 014: expose parser-neutral scenario execution

## Status

Proposed.

## Date

2026-07-13.

## Context and problem statement

rstest-bdd currently combines a reusable runtime with a Gherkin-specific
compile-time frontend. The runtime crate owns the step registry, placeholder
matching, fixture validation, synchronous and asynchronous step handlers,
returned-value propagation primitives, skip signals, and structured
step-execution errors. The procedural macro crate parses `.feature` files and
generates the remaining scenario orchestration code.

The generated scenario loop currently owns policy that is not intrinsically
Gherkin-specific. It executes steps in order, inserts step return values into
`StepContext`, stops after a failure or skip, records skipped work, and converts
terminal failures into panics for the Rust test harness. This policy exists as
quoted generated code rather than as a public runtime function.

Trymark proposes a standalone Markdown frontend for literate behavioural tests.
The stock Trymark executable must run in repositories that contain no
`Cargo.toml` and no user-authored Rust. It can still depend internally on
rstest-bdd and link a ready-made set of process, workspace, observation, and
oracle steps. Reusing only `execute_step` would force Trymark to reproduce the
scenario loop and its lifecycle policy. The two products would then own subtly
different definitions of skip handling, return propagation, cleanup, and
failure reporting.

Other non-Gherkin frontends could encounter the same boundary. A runtime API
that accepts an already-parsed scenario would let frontends supply steps
without translating them into temporary Gherkin or generated Rust source.

The decision is whether rstest-bdd should expose parser-neutral scenario
orchestration as a supported runtime API, and how to do so without weakening the
existing Gherkin and rstest integrations.

## Decision drivers

- Keep one canonical implementation of scenario order, skip, failure, returned
  value, and lifecycle policy.
- Let a standalone executable use rstest-bdd without invoking the Rust test
  harness or generating Rust code.
- Preserve the existing Gherkin macros and their `cargo test` behaviour.
- Keep Markdown syntax, process execution, snapshots, and Trymark reporting out
  of rstest-bdd.
- Return structured outcomes to callers instead of forcing every caller through
  a panic boundary.
- Retain link-time step registration and typed fixture extraction for linked
  Rust step packs.
- Coordinate the runner with ADR 012's guard-based `StepContext` borrowing and
  before- and after-scenario lifecycle direction.
- Keep source locations accurate for frontends whose scenarios do not originate
  in `.feature` files.
- Avoid a premature crate split or general workflow abstraction.

## Requirements

### Functional requirements

1. A caller can construct a scenario from a name, tags, source identity, and an
   ordered collection of step invocations.
2. Each step invocation carries a keyword, text, optional doc string, optional
   data table, and optional source location.
3. The runtime executes the same registered steps that Gherkin-generated tests
   execute today.
4. The synchronous and asynchronous runners apply equivalent policy.
5. A returned step value becomes available to later steps through the existing
   unambiguous type-matching rule.
6. A skip stops later steps and returns a structured skipped outcome with the
   step index and optional reason.
7. A failure stops later steps and returns a structured failed outcome with the
   step index and execution error.
8. Before-scenario and after-scenario lifecycle hooks run according to ADR 012,
   including after failure and skip.
9. Existing generated tests can translate the structured outcome into their
   current Rust test-harness and reporting behaviour.
10. External runners can render their own reports and choose their own aggregate
    process exit-code policy.

### Technical requirements

1. The parser-neutral plan contains no `gherkin` Abstract Syntax Tree (AST),
   Markdown, Trymark, Clap, process, snapshot, or reporter types.
2. The runtime runner does not panic for an ordinary scenario failure. It
   returns a typed outcome. Panics from step handlers remain represented through
   the existing step-error machinery.
3. The initial change is additive. Existing step macros, scenario macros,
   `execute_step`, and `execute_step_async` continue to work.
4. The procedural macro crate delegates canonical scenario-loop policy to the
   runtime after migration. It does not maintain a second implementation.
5. Link-time `inventory` registration remains the Rust extension mechanism.
   This ADR does not introduce runtime loading of Rust libraries.
6. The API permits frontends to retain richer source maps without forcing those
   maps into rstest-bdd's core types.
7. The public plan supports both statically generated Gherkin scenarios and
   dynamically parsed frontends without requiring avoidable copies at every
   step.
8. The runner and lifecycle API settle together. No Trymark-specific cleanup
   protocol may become a competing source of truth.

## Options considered

### Option A: implement an independent Trymark engine

Trymark could copy the step-matching concepts it needs or define its own
actuator, sensor, and oracle registry.

Rejected. This would duplicate the behaviour already owned by rstest-bdd and
would create two incompatible extension ecosystems. Skip, async, fixture, and
lifecycle semantics would drift even if the initial implementations looked
similar.

### Option B: lower Markdown into temporary Gherkin

Trymark could translate headings and fences into a generated `.feature` file,
then invoke the existing macro or parser path.

Rejected. A generated intermediate document would degrade source locations,
force fence metadata through Gherkin doc-string and table encodings, and expose
users to errors in text they never wrote. It would also retain a compile-time
Rust boundary that the standalone executable does not need.

### Option C: generate and compile a temporary Rust test crate

Trymark could generate scenario functions and use the existing procedural
macros unchanged.

Rejected for the stock runner. This would require Cargo in non-Rust
repositories, add compilation latency, and turn a language-neutral CLI test
into a Rust build. A custom linked Trymark runner remains a valid extension
path, but it must not define the default experience.

### Option D: expose a parser-neutral runtime plan and scenario runner

rstest-bdd exposes structured plan and outcome types plus synchronous and
asynchronous runner functions. Existing macros and Trymark both delegate to the
same implementation.

Accepted as the proposed direction. It reuses the current runtime architecture
and removes policy from generated code without making rstest-bdd aware of
Markdown.

### Option E: extract a new shared core crate immediately

The project could first move the registry, context, execution, plan, and runner
into a new `rstest-bdd-core` crate, with Gherkin and rstest integrations layered
above it.

Deferred. The conceptual split may become useful, particularly if the existing
runtime's `gherkin` dependency proves undesirable for standalone consumers.
Doing it before the public runner exists combines two architectural changes and
makes review harder. The first implementation should add the boundary inside
`rstest-bdd`; later evidence can justify extraction.

| Topic | Independent engine | Temporary Gherkin or Rust | Parser-neutral runner | Immediate core crate |
|---|---|---|---|---|
| One execution policy | No | Partly | Yes | Yes |
| Source fidelity | Yes | No | Yes | Yes |
| No Rust setup for stock Trymark | Yes | No | Yes | Yes |
| Change to rstest-bdd | None | Small frontend workaround | Focused runtime API | Large package refactor |
| Risk of semantic drift | High | Medium | Low | Low |
| Initial implementation cost | Medium | Medium | Medium | High |

_Table 1: Trade-offs between the considered execution architectures._

## Decision outcome and proposed direction

rstest-bdd will expose parser-neutral scenario execution from its runtime crate.
The Gherkin macros will become one frontend over that API. Trymark can become a
second frontend while shipping as an independent repository and executable.

### Scenario plan

The runtime will define a source-neutral execution view. The exact ownership
shape remains an implementation detail to settle through a focused API spike,
but it must support borrowed static data from generated tests and owned dynamic
data from external parsers.

An illustrative interface is:

```rust,no_run
pub struct ScenarioPlan<'a> {
    pub name: &'a str,
    pub tags: &'a [String],
    pub source: ScenarioSource<'a>,
    pub steps: &'a [StepInvocation<'a>],
    pub allow_skipped: bool,
}

pub struct StepInvocation<'a> {
    pub keyword: StepKeyword,
    pub text: &'a str,
    pub docstring: Option<&'a str>,
    pub table: Option<&'a [&'a [&'a str]]>,
    pub source: Option<SourceLocation<'a>>,
}

pub struct ScenarioSource<'a> {
    pub path: &'a str,
    pub line: Option<u32>,
}

pub struct SourceLocation<'a> {
    pub path: &'a str,
    pub line: u32,
    pub column: Option<u32>,
}
```

This example records the semantic contract, not final field names or lifetime
syntax. A frontend may own a richer plan and borrow it into this execution view.
rstest-bdd will not own Markdown byte ranges, inline-edit targets, or snapshot
metadata.

### Structured outcome

The runner will return a complete terminal outcome rather than panic:

```rust,no_run
pub enum ScenarioOutcome {
    Passed {
        steps: Vec<StepOutcome>,
    },
    Skipped {
        at: usize,
        message: Option<String>,
        steps: Vec<StepOutcome>,
    },
    Failed {
        at: usize,
        error: ExecutionError,
        steps: Vec<StepOutcome>,
    },
}
```

`StepOutcome` will retain enough information for a caller to report completed,
skipped, bypassed, and failed steps without re-running registry lookup. It will
not contain frontend-specific rendered diagnostics.

The runner surface will provide synchronous and asynchronous forms:

```rust,no_run
pub fn run_scenario(
    plan: &ScenarioPlan<'_>,
    context: &mut StepContext<'_>,
) -> ScenarioOutcome;

pub async fn run_scenario_async(
    plan: &ScenarioPlan<'_>,
    context: &mut StepContext<'_>,
) -> ScenarioOutcome;
```

### Canonical execution sequence

The runner owns the following sequence:

1. Enter the scenario lifecycle and run before-scenario hooks.
2. Resolve each invocation through the existing step registry.
3. Validate its fixture requirements.
4. Execute the selected synchronous or asynchronous handler.
5. Insert any returned value into `StepContext` under the existing unique-type
   rule.
6. Record the completed step outcome.
7. Stop after a skip or failure and mark later steps as bypassed where reporting
   requires it.
8. Run after-scenario cleanup on every terminal path.
9. Return `Passed`, `Skipped`, or `Failed` without panicking.

This sequence becomes normative. Macro code generation may construct static
arrays and adapters, but it must not carry an independent result-handling loop.

### Gherkin macro integration

The existing `#[scenario]` and `scenarios!` macros will continue to parse and
validate Gherkin at compile time. Their generated tests will:

1. construct or reference a parser-neutral plan;
2. build `StepContext` from rstest fixtures and harness context;
3. call the synchronous or asynchronous scenario runner;
4. preserve existing skip and bypassed-step reporting; and
5. translate a failed outcome into the Rust test harness's failure mechanism.

The generated test remains the boundary that may panic for a failed scenario.
The runtime runner itself does not.

### Source-neutral diagnostics

New plan and outcome types will use source-neutral terms such as `source` and
`source_path`. Existing public fields named `feature_path` remain available
during the additive migration. New reporting code should use source-neutral
accessors or wrapper types. A later pre-1.0 breaking release may rename legacy
fields if the migration cost is justified.

The new runner will pair an `ExecutionError` with the invocation's source
location through the structured outcome. A Markdown frontend can therefore
point at the heading or fence that produced a step without extending
`ExecutionError` with Markdown concepts.

### Extension boundary

The stock Trymark executable links standard step packs into the normal
rstest-bdd inventory. Custom Rust step packs require a custom linked runner,
because `inventory` is a link-time mechanism. Language-neutral extensions use
Trymark's external-driver protocol and do not alter rstest-bdd.

This ADR does not add runtime mutation of the global registry. If a future
consumer requires runtime step registration, that need requires a separate ADR
because it changes duplicate detection, determinism, and concurrency policy.

## Goals and non-goals

### Goals

- Make rstest-bdd's scenario policy reusable outside generated Rust tests.
- Keep Gherkin and Markdown frontends semantically aligned.
- Let Trymark ship a prebuilt standalone binary with standard linked steps.
- Improve rstest-bdd by moving policy out of procedural macro output.
- Give callers structured data for reports, exit codes, and source-aware
  diagnostics.

### Non-goals

- Add Markdown parsing or Trymark step packs to rstest-bdd.
- Standardize a universal Behaviour-Driven Development (BDD) interchange
  format.
- Replace rstest fixtures, harness adapters, or the existing Gherkin frontend.
- Add dynamic Rust plugin loading.
- Specify Trymark's document dialect, snapshot format, driver protocol, or CLI.
- Extract a new core crate in the first implementation.
- Stabilize exact ownership, allocation, or source-span representations in this
  ADR.

## Compatibility and migration

The change starts as an additive API. Existing users need not modify feature
files, step definitions, fixture declarations, or test commands.

The migration proceeds in four stages.

### Stage 1: add plan, outcome, and runner APIs

Implement the parser-neutral types and the canonical runner alongside the
existing generated loop. Add direct runtime tests for synchronous and
asynchronous plans.

### Stage 2: prove semantic parity

Run the existing macro-generated scenario corpus through both paths in a test
configuration. Compare terminal status, executed-step order, returned-value
propagation, skip position, bypassed steps, fixture diagnostics, and handler
failures. The two paths must agree before macro migration.

### Stage 3: switch generated scenarios to the runtime runner

Change macro output to build a plan and call the runtime API. Remove the old
result-handling loop after parity tests pass. Retain only thin generated
adapters for fixture setup, harness integration, outline values, and translation
to the Rust test harness.

### Stage 4: validate an external frontend

Use Trymark, or a small conformance frontend if Trymark is not yet available, to
parse scenarios at runtime and execute standard linked steps. This stage proves
that the API is genuinely parser-neutral rather than merely a rearranged
Gherkin implementation.

A later release may deprecate Gherkin-specific runtime field names or extract a
core crate. Those changes need explicit migration notes and do not form part of
the initial acceptance criteria.

## Verification strategy

The parser-neutral runner changes the semantic centre of the framework. Its
verification must cover policy, not merely API examples.

1. **Conformance tests.** Feed equivalent scenarios through existing Gherkin
   macro generation and the direct runtime API. Assert identical step order,
   fixture access, return propagation, skips, bypassed-step records, and errors.
2. **Synchronous and asynchronous equivalence.** For steps that support both
   modes, run the same plans through both functions and compare structured
   outcomes.
3. **Lifecycle tests.** In coordination with ADR 012, prove before-scenario and
   after-scenario hooks run exactly once on pass, failure, skip, missing step,
   missing fixture, and handler panic paths.
4. **Property-based sequence tests.** Generate bounded step sequences with pass,
   return, skip, and failure handlers. Assert that no step after a terminal
   event executes, values become visible only after their producing step, and
   cleanup always occurs.
5. **Source-location tests.** Execute plans whose source paths are not
   `.feature` files and prove outcomes preserve the supplied locations without
   Gherkin parsing.
6. **Compile and migration tests.** Keep the public macros' existing successful
   and compile-fail fixtures unchanged while their generated implementation
   moves to the runner.

The runner does not need formal verification at introduction. Property-based
sequence tests and dual-path conformance provide the stronger return for this
bounded state machine. A later model-checking spike remains available if
lifecycle interactions grow beyond those tests.

## Consequences

### Positive consequences

- rstest-bdd becomes a reusable behavioural runtime, not only a Gherkin macro
  implementation.
- Trymark can offer a standalone, language-neutral user experience without
  duplicating execution semantics.
- Procedural macro output shrinks and delegates more policy to ordinary,
  testable Rust functions.
- External frontends receive structured outcomes instead of parsing panic text.
- Source-aware reporters can use their native source maps while sharing the
  execution engine.

### Costs and trade-offs

- The public runtime surface grows and acquires compatibility obligations.
- The plan must balance static generated data against dynamically parsed owned
  data; a poor ownership design could create copies or awkward lifetimes.
- The macro migration temporarily maintains two execution paths for parity
  testing.
- ADR 012's lifecycle work and this runner cannot be designed independently.
- A prebuilt Trymark binary still cannot discover arbitrary Rust step crates at
  runtime; custom Rust extensions require a linked runner.

## Known risks and limitations

- A parser-neutral type can become a lowest-common-denominator interchange
  format. Keep it limited to information the execution engine needs.
- Exposing detailed per-step outcomes may accidentally freeze internal registry
  or error representations. Prefer opaque accessors where future change is
  plausible.
- An observer or event-listener interface may appear convenient for reporters,
  but callbacks can create re-entrancy and lifetime complexity. Structured
  terminal outcomes are sufficient for the first API.
- The existing runtime crate still depends on `gherkin` for compatibility
  types. This does not prevent Trymark adoption, but it may motivate a later
  crate split or optional dependency.
- Parallel scenario execution remains a caller concern. The runner executes one
  scenario; it does not schedule suites.

## Outstanding decisions

- Should the public execution view use borrowed slices, `Cow`, owned values, or
  separate owned and borrowed plan types?
- Which `StepOutcome` fields are stable public contract rather than reporter
  convenience?
- Should source columns use bytes, Unicode scalar values, or display columns?
- Does the first runner API include tags and `allow_skipped`, or should a
  separate policy object carry them?
- Which release contains the macro migration, given ADR 012's planned v0.7.0
  breaking work?
- Should the `gherkin` dependency become optional after the runner lands, or is
  a later `rstest-bdd-core` extraction cleaner?

These questions affect API shape, not the architectural decision to centralize
scenario execution in the runtime.

## Governs

- The parser-neutral scenario-plan and structured-outcome API in `rstest-bdd`.
- Migration of generated scenario-loop policy from `rstest-bdd-macros` into the
  runtime.
- Non-Gherkin linked frontends, including Trymark, that execute rstest-bdd step
  inventories.

This ADR does not govern Trymark's Markdown syntax, standard steps, process
model, snapshot format, external-driver protocol, or distribution.
