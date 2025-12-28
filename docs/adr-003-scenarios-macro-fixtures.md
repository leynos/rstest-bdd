# Architectural decision record (ADR) 003: `scenarios!` macro fixture injection

## Status

Accepted. 2025-12-27.

## Date

2025-12-27.

## Context and problem statement

The `scenarios!` macro auto-generates rstest-backed test functions from Gherkin
feature files. However, it lacks support for injecting rstest fixtures into
these generated tests. Users who want to share state (such as a "world" object)
across step definitions must work around this limitation by defining fixtures
at the step level or using other patterns.

The related `#[scenario]` attribute macro already supports fixtures via
function parameters, because the user provides a function whose signature can
include fixture arguments. The `scenarios!` macro generates functions
internally, so a different mechanism is needed.

Additionally, when fixtures are added to generated test function signatures,
the variables appear unused to the compiler because they are consumed via
`StepContext` insertion rather than direct reference in the test body. This
triggers `unused_variables` warnings, which become errors under strict linting
(e.g. `-D unused-variables`).

## Decision drivers

- Users should be able to inject rstest fixtures into all scenarios generated
  by `scenarios!`.
- The solution should integrate naturally with rstest's `#[fixture]` system.
- Generated code should compile cleanly under strict linting without forcing
  users to add workarounds at the macro call site.
- The implementation should follow existing patterns in the codebase.

## Decision outcome

Introduce a `fixtures = [name: Type, ...]` parameter to the `scenarios!` macro:

```rust,no_run
use rstest::fixture;
use rstest_bdd::scenarios;

struct TestWorld { /* ... */ }

#[fixture]
fn world() -> TestWorld {
    TestWorld::new()
}

scenarios!("tests/features", fixtures = [world: TestWorld]);
```

### Behaviour

1. Each generated test function includes the specified fixtures as parameters
   in its signature.
2. rstest resolves these parameters by calling the corresponding `#[fixture]`
   functions.
3. Fixture values are inserted into `StepContext` via the existing
   `extract_function_fixtures` utility, making them available to step
   definitions.
4. When fixtures are present, the generated test function is annotated with
   `#[expect(unused_variables)]` to suppress the lint warning.

### Lint suppression rationale

Fixture variables are consumed via `StepContext::insert()`, not referenced
directly in the generated test body. The `#[expect(unused_variables)]`
attribute:

- Documents that the apparent non-use is intentional.
- Allows compilation under `-D unused-variables`.
- Will emit a warning if the suppression becomes unnecessary (unlike
  `#[allow]`).

This follows the existing pattern in
`crates/rstest-bdd-macros/src/utils/ fixtures.rs` where `#[expect(unused_mut)]`
is used with a reason for similar generated code.

## Relationship to Cucumber

This is **not** a standard Cucumber feature. Cucumber frameworks typically use
world objects or dependency injection containers. The `fixtures` parameter is
an rstest-bdd specific extension that leverages rstest's fixture system to
provide similar functionality in a Rust-idiomatic way.

## Implementation details

### Parsing

The `fixtures = [name: Type, ...]` syntax is parsed in
`crates/rstest-bdd-macros/src/macros/scenarios/macro_args.rs`:

- `FixtureSpec` struct holds the identifier and type for each fixture.
- The parser accepts a bracketed, comma-separated list of `name: Type` pairs.

### Code generation

In `crates/rstest-bdd-macros/src/macros/scenarios/test_generation.rs`:

- Fixture parameters are added to the generated function signature before any
  scenario outline example parameters.
- `extract_function_fixtures` processes the signature to produce `StepContext`
  insertion code.
- The `#[expect(unused_variables)]` attribute is added when fixtures are
  present.

### Example generated code

For `scenarios!("tests/features", fixtures = [world: TestWorld])`:

For screen readers: The following Rust snippet illustrates the structure of
generated test code when fixtures are present.

```rust,no_run
#[expect(
    unused_variables,
    reason = "fixture variables are consumed via StepContext, \
              not referenced directly in the scenario test body"
)]
#[rstest::rstest]
fn feature_scenario_name(world: TestWorld) {
    // Fixture is inserted into StepContext:
    // let __cell = RefCell::new(Box::new(world));
    // ctx.insert_owned::<TestWorld>("world", &__cell);
    // ... step execution ...
}
```

## Known risks and limitations

- Fixture parameters appear before example parameters in scenario outlines.
  This ordering is consistent but not configurable.
- Users must ensure the fixture function name matches the parameter name, or
  use rstest's `#[from(...)]` attribute (which is already supported by
  `extract_function_fixtures`).
- The lint suppression applies to all fixture parameters. If a future change
  causes some fixtures to be directly referenced, the `#[expect]` may need
  adjustment.

## Alternatives considered

### Allow attribute at call site

Users could add `#[allow(unused_variables)]` at the macro invocation:

```rust,no_run
#[allow(unused_variables)]
scenarios!("tests/features", fixtures = [world: TestWorld]);
```

This was rejected because it pushes the workaround onto every consumer and
doesn't document the intent.

### Prefix fixture parameters with underscore

Generated fixture parameters could be prefixed with `_` to suppress the warning:

```rust,no_run
fn scenario(_world: TestWorld) { /* ... */ }
```

This was rejected because it would require changes to
`extract_function_fixtures` and would obscure the fixture's purpose in
diagnostics and generated code.
