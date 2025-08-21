# `rstest-bdd` user's guide

## Introduction

Behaviour‑Driven Development (BDD) is a collaborative practice that emphasizes
a shared understanding of software behaviour across roles. The design of
`rstest‑bdd` integrates BDD concepts with the Rust testing ecosystem. BDD
encourages collaboration between developers, quality-assurance specialists, and
non-technical business participants by describing system behaviour in a
natural, domain‑specific language. `rstest‑bdd` achieves this without
introducing a bespoke test runner; instead, it builds on the `rstest` crate so
that unit tests and high‑level behaviour tests can co‑exist and run under
`cargo test`. The framework reuses `rstest` fixtures for dependency injection
and uses a procedural macro to bind tests to Gherkin scenarios, ensuring that
functional tests live alongside unit tests and benefit from the same tooling.

This guide explains how to consume `rstest‑bdd` at the current stage of
development. It relies on the implemented code rather than on aspirational
features described in the design documents. Where the design proposes advanced
behaviour, the implementation status is noted. Examples and explanations are
organized by the so‑called *three amigos* of BDD: the business analyst/product
owner, the developer, and the tester.

## The three amigos

| Role ("amigo")                     | Primary concerns                                                                                                                  | Features provided by `rstest‑bdd`                                                                                                                                                                                                                                                                                                                                                                         |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Business analyst/product owner** | Writing and reviewing business-readable specifications; ensuring that acceptance criteria are expressed clearly.                  | Gherkin `.feature` files are plain text and start with a `Feature` declaration; each `Scenario` describes a single behaviour. Steps are written using keywords `Given`, `When`, and `Then` ([syntax](gherkin-syntax.md#L72-L91)), producing living documentation that can be read by non-technical stakeholders.                                                                                          |
| **Developer**                      | Implementing step definitions in Rust and wiring them to the business specifications; using existing fixtures for setup/teardown. | Attribute macros `#[given]`, `#[when]` and `#[then]` register step functions and their pattern strings in a global step registry. A `#[scenario]` macro reads a feature file at compile time and generates a test that drives the registered steps. Fixture values from `rstest` can be injected into step functions via `#[from(fixture_name)]`.                                                         |
| **Tester/QA**                      | Executing behaviour tests, ensuring correct sequencing of steps and verifying outcomes observable by the user.                    | Scenarios are executed via the standard `cargo test` runner; test functions annotated with `#[scenario]` run each step in order and panic if a step is missing. Assertions belong in `Then` steps; guidelines discourage inspecting internal state and encourage verifying observable outcomes. Testers can use `cargo test` filters and parallelism because the generated tests are ordinary Rust tests. |

The following sections expand on these responsibilities and show how to use the
current API effectively.

## Gherkin feature files

Gherkin files describe behaviour in a structured, plain‑text format that can be
read by both technical and non‑technical stakeholders. Each `.feature` file
begins with a `Feature` declaration that provides a high‑level description of
the functionality. A feature contains one or more `Scenario` sections, each of
which documents a single example of system behaviour. Inside a scenario, the
behaviour is expressed through a sequence of steps starting with `Given`
(context), followed by `When` (action) and ending with `Then` (expected
outcome). Secondary keywords `And` and `But` may chain additional steps of the
same type for readability.

Scenarios follow the simple `Given‑When‑Then` pattern. Support for **Scenario
Outline** is available, enabling a single scenario to run with multiple sets of
data from an `Examples` table. A `Background` section may define steps that run
before each scenario. Advanced constructs such as data tables and Doc Strings
provide structured or free‑form arguments to steps.

### Example feature file

```gherkin
Feature: Shopping basket

  Scenario: Add item to basket
    Given an empty basket
    When the user adds a pumpkin
    Then the basket contains one pumpkin
```

The feature file lives within the crate (commonly under `tests/features/`). The
path to this file will be referenced by the `#[scenario]` macro in the test
code.

## Step definitions

Developers implement the behaviour described in a feature by writing step
definition functions in Rust. Each step definition is an ordinary function
annotated with one of the attribute macros `#[given]`, `#[when]` or `#[then]`.
The annotation takes a single string literal that must match the text of the
corresponding step in the feature file. Placeholders in the form `{name}` or
`{name:Type}` are supported. The framework extracts matching substrings and
converts them using `FromStr`; type hints constrain the match using specialised
regular expressions.

The procedural macro implementation expands the annotated function into two
parts: the original function and a wrapper function that registers the step in
a global registry. The wrapper captures the step keyword, pattern string and
associated fixtures and uses the `inventory` crate to publish them for later
lookup.

### Fixtures and the `#[from]` attribute

`rstest‑bdd` builds on `rstest`’s fixture system rather than using a monolithic
“world” object. Fixtures are defined using `#[rstest::fixture]` in the usual
way, and they can be injected into step definitions through the
`#[from(fixture_name)]` attribute. Internally, the step macros record the
fixture names and generate wrapper code that, at runtime, retrieves references
from a `StepContext`. This context is a key–value map of fixture names to
type‑erased references. When a scenario runs, the generated test inserts its
arguments (the `rstest` fixtures) into the `StepContext` before invoking each
registered step.

Example:

```rust
use rstest::fixture;
use rstest_bdd_macros::{given, when, then, scenario};

// A fixture used by multiple steps.
#[fixture]
fn basket() -> Basket {
    Basket::new()
}

#[given("an empty basket")]
fn empty_basket(#[from(basket)] b: &mut Basket) {
    b.clear();
}

#[when("the user adds a pumpkin")]
fn add_pumpkin(#[from(basket)] b: &mut Basket) {
    b.add(Item::Pumpkin, 1);
}

#[then("the basket contains one pumpkin")]
fn assert_pumpkins(#[from(basket)] b: &Basket) {
    assert_eq!(b.count(Item::Pumpkin), 1);
}

#[scenario(path = "tests/features/shopping.feature")]
fn test_add_to_basket(#[with(basket)] _: Basket) {
    // optional assertions after the steps
}
```

In this example, the step texts in the annotations must match the feature file
verbatim. The `#[scenario]` macro binds the test function to the first scenario
in the specified feature file and runs all registered steps before executing
the body of `test_add_to_basket`.

## Binding tests to scenarios

The `#[scenario]` macro is the entry point that ties a Rust test function to a
scenario defined in a `.feature` file. It accepts two arguments:

| Argument       | Purpose                                                                                                      | Status                                                                                                         |
| -------------- | ------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------- |
| `path: &str`   | Relative path to the feature file from the crate root. This is mandatory.                                    | **Implemented**: the macro resolves the path at compile time and parses the feature using the `gherkin` crate. |
| `index: usize` | Optional zero‑based index selecting a scenario when the file contains multiple scenarios. Defaults to `0`.   | **Implemented**: the macro uses the index to pick the scenario.                                                |

If the feature file cannot be found or contains invalid Gherkin, the macro
emits a compile-time error with the offending path.

The design document proposes a `name` argument to select scenarios by name, but
only `path` and `index` are currently accepted.

During macro expansion, the feature file is read and parsed. The macro
generates a new test function annotated with `#[rstest::rstest]` that performs
the following steps:

1. Build a `StepContext` and insert the test’s fixture arguments into it.

2. For each step in the scenario (according to the `Given‑When‑Then` sequence),
   look up a matching step function by `(keyword, pattern)` in the registry. A
   missing step triggers a panic with a clear error message, allowing
   developers to detect incomplete implementations.

3. Invoke the registered step function with the `StepContext` so that fixtures
   are available inside the step.

4. After executing all steps, run the original test body. This block can
   include extra assertions or cleanup logic beyond the behaviour described in
   the feature.

Because the generated code uses `#[rstest::rstest]`, it integrates seamlessly
with `rstest` features such as parameterized tests and asynchronous fixtures.
Tests are still discovered and executed by the standard Rust test runner, so
one may filter or run them in parallel as usual.

## Autodiscovering scenarios

For large suites, it is tedious to bind each scenario manually. The
`scenarios!` macro scans a directory recursively for `.feature` files and
generates a module with a test for every `Scenario` found. Each test is named
after the feature file and scenario title. Identifiers are sanitised
(ASCII-only) and deduplicated by appending a numeric suffix when collisions
occur.

```rust
use rstest_bdd_macros::{given, then, when, scenarios};

#[given("a precondition")] fn precondition() {}
#[when("an action occurs")] fn action() {}
#[then("events are recorded")] fn events() {}

scenarios!("tests/features/auto");
```

Generated tests cannot currently accept fixtures; use `#[scenario]` when
fixture injection or custom assertions are required.

## Running and maintaining tests

Once feature files and step definitions are in place, scenarios run via the
usual `cargo test` command. Test functions created by the `#[scenario]` macro
behave like other `rstest` tests; they honour `#[tokio::test]` or
`#[async_std::test]` attributes if applied to the original function. Each
scenario runs its steps sequentially in the order defined in the feature file.
When a step is not found, the test will panic and report which step is missing
and where it occurs in the feature. This strictness ensures that behaviour
specifications do not silently drift from the code.

Best practices for writing effective scenarios include:

- **Keep scenarios focused.** Each scenario should test a single behaviour and
  contain exactly one `When` step. If multiple actions need to be tested, break
  them into separate scenarios.

- **Make outcomes observable.** Assertions in `Then` steps should verify
  externally visible results such as UI messages or API responses, not internal
  state or database rows.

- **Avoid user interactions in** `Given` **steps.** `Given` steps establish
  context but should not perform actions.

- **Write feature files collaboratively.** The value of Gherkin lies in the
  conversation between the three amigos; ensure that business stakeholders read
  and contribute to the feature files.

- **Use placeholders for dynamic values.** Pattern strings may include
  `format!`-style placeholders such as `{count:u32}`. Type hints narrow the
  match. Numeric hints support all Rust primitives (`u8..u128`, `i8..i128`,
  `usize`, `isize`, `f32`, `f64`). Floating-point hints accept integers,
  decimal forms with optional leading or trailing digits, scientific notation
  (for example, `1e3`, `-1E-9`), and the special values `NaN`, `inf`, and
  `Infinity`. Matching is anchored: the entire step text must match the
  pattern; partial matches do not succeed. Escape literal braces with `{{` and
  `}}`. Nested braces inside placeholders are not supported. Placeholders
  follow `{name[:type]}`; `name` must start with a letter or underscore and may
  contain letters, digits, or underscores (`[A-Za-z_][A-Za-z0-9_]*`).
  Surrounding whitespace in the type hint is ignored (for example,
  `{count:u32}` and `{count: u32}` are both accepted). Prefer the compact form
  `{count:u32}` in new code. When a pattern contains no placeholders, the step
  text must match exactly.

## Data tables and Doc Strings

Steps may supply structured or free-form data via a trailing argument. A data
table is received by including an argument named `datatable` of type
`Vec<Vec<String>>`. A Gherkin Doc String is made available through an argument
named `docstring` of type `String`. Both arguments must use these exact names
and types to be detected by the procedural macros. When both are declared,
place `datatable` before `docstring` at the end of the parameter list. At
runtime, the generated wrapper converts the table cells or copies the block
text and passes them to the step function, panicking if the feature omits the
expected content. Doc Strings may be delimited by triple double-quotes or
triple backticks.

## Limitations and roadmap

The `rstest‑bdd` project is evolving. Several features described in the design
document and README remain unimplemented in the current codebase:

- **Selecting scenarios by name.** The README hints at a `name` argument for
  the `#[scenario]` macro, but the macro only accepts `path` and optional
  `index`.

- **Wildcard keywords and** `*` **steps.** The asterisk (`*`) can replace any
  step keyword in Gherkin to improve readability, but step lookup is based
  strictly on the primary keyword. Using `*` in feature files will not match
  any registered step.

- **Restricted placeholder types.** Only placeholders that parse via
  `FromStr` are supported, and they must be well-formed and non-overlapping.

Consult the project’s roadmap or repository for updates. When new features are
added, patterns and examples may change. Meanwhile, adopting `rstest‑bdd` in
its current form will be most effective when feature files remain simple and
step definitions are explicit.

## Summary

`rstest‑bdd` seeks to bring the collaborative clarity of Behaviour‑Driven
Development to Rust without sacrificing the ergonomics of `rstest` and the
convenience of `cargo test`. In its present form, the framework provides a core
workflow: write Gherkin scenarios, implement matching Rust functions with
`#[given]`, `#[when]` and `#[then]` annotations, inject fixtures via `#[from]`,
and bind tests to scenarios with `#[scenario]`. Step definitions are discovered
at link time via the `inventory` crate, and scenarios execute all steps in
sequence before running any remaining test code. While advanced Gherkin
constructs and parameterization remain on the horizon, this foundation allows
teams to integrate acceptance criteria into their Rust test suites and to
engage all three amigos in the specification process.
