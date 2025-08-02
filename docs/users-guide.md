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
| **Business analyst/product owner** | Writing and reviewing business-readable specifications; ensuring that acceptance criteria are expressed clearly.                  | Gherkin `.feature` files are plain text and start with a `Feature` declaration; each `Scenario` describes a single behaviour. Steps are written using keywords `Given`, `When`, and `Then` ([syntax](gherkin-syntax.md#L72-L91)), producing living documentation that can be read by non-technical stakeholders.      |
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
data from an `Examples` table. Other advanced constructs such as
**Background**, data tables and docstrings are not yet implemented.

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
The annotation takes a single string literal; this string must match the text
of the corresponding step in the feature file exactly. Unlike the aspirational
design, the current implementation does not parse placeholders or capture
groups from the pattern. Therefore, dynamic parameters cannot be extracted from
step text; each unique step text requires a definition.

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

- **Keep patterns exact.** Because the current implementation does not support
  placeholders or capturing groups, the pattern strings in `#[given]`,
  `#[when]` and `#[then]` must match the corresponding lines in the feature
  file character-for-character. Whitespace, capitalization, and punctuation must
  align.

## Limitations and roadmap

The `rstest‑bdd` project is evolving. Several features described in the design
document and README remain unimplemented in the current codebase:

- **Step argument parsing and typed placeholders.** The design envisages
  patterns like `{"{count:u32}"}` that extract values and convert them via
  `FromStr`, but the existing macros treat the pattern as a literal string and
  pass no captured arguments to the step function. Step functions may only
  accept fixtures as arguments.

- **Background sections, data tables and docstrings.** The design notes plans
  to support shared `Background` steps and to supply data tables and docstrings
  as arguments to step functions. These are currently not implemented.

- **Selecting scenarios by name.** The README hints at a `name` argument for
  the `#[scenario]` macro, but the macro only accepts `path` and optional
  `index`.

- **Wildcard keywords and** `*` **steps.** The asterisk (`*`) can replace any
  step keyword in Gherkin to improve readability, but step lookup is based
  strictly on the primary keyword. Using `*` in feature files will not match
  any registered step.

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
