# Roadmap

This roadmap outlines the development plan for the `rstest-bdd` framework,
based on the provided design proposal. It is broken down into phases to allow
for incremental implementation, testing, and delivery of value.

## Phase 1: Core Mechanics & Proof of Concept

The primary goal of this phase is to validate the core architectural decision:
using `inventory` for link-time collection of step definitions, which are then
discovered and executed by a procedural macro at runtime.

- [x] **Project Scaffolding**

  - [x] Create a new Cargo workspace.

  - [x] Add the `rstest-bdd` library crate.

  - [x] Add the `rstest-bdd-macros` procedural macro crate.

- [x] **Step Registry Implementation**

  - [x] Define the `Step` struct within `rstest-bdd` to hold metadata (keyword,
    pattern, type-erased run function, source location).

  - [x] Use `inventory::collect!(Step)` to establish the global collection.

- [x] **Step Definition Macros**

  - [x] Implement the `#[given("...")]` attribute macro in `rstest-bdd-macros`.

  - [x] Implement the `#[when("...")]` attribute macro.

  - [x] Implement the `#[then("...")]` attribute macro.

  - [x] Ensure each macro generates an `inventory::submit!` block that
    constructs and registers a `Step` instance.

- [x] **Scenario Orchestrator Macro (Initial Version)**

  - [x] Implement a basic `#[scenario(path = "...")]` attribute macro.

  - [x] The macro must, at compile-time, read and parse the specified
    `.feature` file using the `gherkin` crate.

  - [x] The macro must generate a new test function annotated with `#[rstest]`.

  - [x] The body of the generated function must, at runtime, iterate through
    the scenario's Gherkin steps and find matching `Step` definitions from the
    `inventory::iter`.

  - [x] For this phase, only support exact, case-sensitive string matching with
    no argument parsing.

- [x] **Validation**

  - [x] Create a simple `web_search.feature` file.

  - [x] Create a `test_web_search.rs` file with corresponding step definitions.

  - [x] Create a test function annotated with `#[scenario]` that successfully
    runs the steps via `cargo test`.

## Phase 2: Fixtures and Parameterization

This phase focuses on integrating with `rstest`'s core features to manage state
and run data-driven tests, making the framework genuinely useful.

- [x] **Fixture Integration**

  - [x] Enhance the step definition macros to inspect the signature of the
    attached function to identify requested fixtures.

  - [x] Modify the `#[scenario]` macro's code generation to correctly manage
    and pass fixtures to the step functions during execution.

- [x] **Scenario Outline Support**

  - [x] Extend the `#[scenario]` macro to detect `Scenario Outline` and its
    `Examples:` table in the parsed Gherkin AST.

  - [x] The macro generates a single, parameterized `#[rstest]` function.

  - [x] For each row in the `Examples:` table, the macro generates a
    corresponding `#[case(...)]` attribute.

- [x] **Step Argument Parsing**

  - [x] Implement a parser for `format!`-style placeholders (e.g.,
    `{count:u32}`).

  - [x] The runtime step-matching logic must extract values from the Gherkin
    step text based on these placeholders.

  - [x] Use the `FromStr` trait to convert the extracted string values into the
    types specified in the function signature.

## Phase 3: Advanced Gherkin Features & Ergonomics

This phase aims for feature-parity with other mature BDD frameworks and
improves the developer experience.

- [ ] **Advanced Gherkin Constructs**

  - [x] Implement support for `Background` steps, ensuring they are executed
    before each `Scenario`.

  - [x] Implement support for `Data Tables`, making the data available to the
    step function as a `Vec<Vec<String>>`.

  - [ ] Implement support for `DocStrings`, making the content available as a
    `String` argument.

- [ ] **Robust Error Handling**

  - [ ] The `#[scenario]` macro must emit a `compile_error!` if the specified
    `.feature` file cannot be found or parsed.

  - [ ] The `#[scenario]` macro must perform a compile-time check to ensure a
    matching step definition exists for every Gherkin step in the target
    scenario, emitting a `compile_error!` if any are missing.

- [ ] **Boilerplate Reduction**

  - [ ] Implement the `scenarios!("path/to/features/")` macro to automatically
    discover all `.feature` files in a directory and generate a test module
    containing a test function for every `Scenario` found.

### Post-Core Implementation: Extensions & Tooling

These tasks can be addressed after the core framework is stable and are aimed
at improving maintainability and IDE integration.

- [ ] **Diagnostic Tooling**

  - [ ] Create a helper binary or `cargo` subcommand (`cargo bdd`).

  - [ ] Implement a `list-steps` command to print the entire registered step
    registry.

  - [ ] Implement commands to identify unused or duplicate step definitions.

- [ ] **IDE Integration**

  - [ ] Investigate creating a `rust-analyzer` procedural macro server to
    provide autocompletion and "Go to Definition" from `.feature` files.

  - [ ] Alternatively, develop a dedicated VS Code extension to provide this
    functionality.

- [ ] **Advanced Hooks**

  - [ ] Explore adding explicit teardown hooks that are guaranteed to run after
    a scenario, even in the case of a panic (e.g., `#[after_scenario]`).

- [ ] **Performance Optimisation**

  - [ ] Implement caching for parsed Gherkin ASTs in the `OUT_DIR` to reduce
    compile-time overhead, only re-parsing files on modification.
