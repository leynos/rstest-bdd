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

- [x] **Advanced Gherkin Constructs**

  - [x] Implement support for `Background` steps, ensuring they are executed
    before each `Scenario`.

  - [x] Implement support for `Data Tables`, initially making the data
    available to the step function as a `Vec<Vec<String>>` (legacy baseline;
    typed support is planned below).

  - [x] Implement support for `Docstring`, making the content available as a
    `String` argument named `docstring`.

- [x] **Robust Error Handling**

  - [x] The `#[scenario]` macro must emit a `compile_error!` if the specified
    `.feature` file cannot be found or parsed.

  - [x] The `#[scenario]` macro must perform a compile-time check to ensure a
    matching step definition exists for every Gherkin step in the target
    scenario, emitting a `compile_error!` if any are missing.

- [x] **Typed Data Table Support**

  - [x] Add a `datatable` runtime module exposing `DataTableError`,
    `HeaderSpec`, `RowSpec`, `Rows<T>`, and convenience parsers such as
    `truthy_bool` and `trimmed<T: FromStr>`.

  - [x] Implement `TryFrom<Vec<Vec<String>>> for Rows<T>` (with `T:
    DataTableRow`) to split optional headers, build index maps, and surface row
    and column context on errors.

  - [ ] Provide `#[derive(DataTableRow)]` and `#[derive(DataTable)]` macros with
    field- and struct-level attributes for column mapping, optional or default
    cells, trimming, tolerant booleans, custom parsers, and row aggregation
    hooks.

  - [x] Update generated wrappers to forward conversion failures by formatting
    the `DataTableError` into the emitted `StepError`, ensuring diagnostics
    reach recorders.

  - [x] Extend documentation (users guide, design document) and add integration
    tests covering headered tables and tolerant boolean parsing.
  - [ ] Add compile-fail fixtures covering optional columns and invalid
    attribute combinations.

- [ ] **Tag Filtering**

  - [ ] Allow the `#[scenario]` macro to select scenarios by tag expression at
    macro-expansion time.

  - [ ] Extend the `scenarios!` macro to filter scenarios using the same tag
    syntax at macro-expansion time. (See: [design §1.3.4].)

  - [ ] Document tag-expression grammar and precedence (§1.3.4).

  - [ ] Filter at macro-expansion time and emit `compile_error!` diagnostics for
    invalid tag expressions (explicit empty string `""`, empty parentheses
    `()`, dangling operators). Omitting the `tags` argument applies no filter
    (`error: missing tag (allowed)`). Diagnostics include the byte offset and a
    short reason, e.g.:
    `error: empty tag string is not allowed (byte offset 42)` or
    `error: invalid tag expression at byte 7: expected tag or '(' after 'and'`.

  - [ ] Define tag scope and inheritance:
    - Scenarios inherit `Feature:` tags.
    - `Scenario Outline` cases inherit tags from the outline and their
      originating `Examples:` block.

  - [ ] Specify associativity (`and`/`or` left-associative; `not` unary-prefix)
    and reject unknown tokens (`&&`, `||`, `!`) at compile time.

  - [ ] Specify case rules and identifier grammar:
    - Tag identifiers are case-sensitive and match `[A-Za-z_][A-Za-z0-9_]*`.
    - Operator keywords (`and`, `or`, `not`) are case-insensitive and
      reserved; they cannot be used as identifiers.

  - [ ] Implement a single shared parser used by both macros to guarantee
    identical semantics.

  - [ ] Support an `@allow_skipped` tag and add a `fail_on_skipped`
    configuration option so skipped scenarios only fail when the flag is set
    and the tag is absent.

  - [ ] Add conformance tests for precedence, associativity, and scope:
    - Valid: `@a and not (@b or @c)`
    - Invalid: `@a && @b`, `""`, `()`, `@a and`, `(@a or @b`,
      `@a or and @b`

- [ ] **Rust 1.75 and Skipping Support**

  - [ ] Raise the minimum supported Rust version to 1.75 and remove the
    `async_trait` dependency from `World` and writer traits.
    - [ ] Set `rust-version = "1.75"` in all Cargo manifests.
    - [ ] Update `rust-toolchain.toml` and CI matrices to Rust 1.75.
    - [ ] Remove `async-trait` from dependencies and code imports.
    - [ ] Add a CI check that fails if `async-trait` reappears.

  - [ ] Provide a `skip!` macro that records a `Skipped` outcome and
    short-circuits remaining steps.

  - [ ] Expose skipped status through `cargo-bdd` and the JSON and JUnit
    writers. Emit a `<skipped>` child on each `<testcase>` element in JUnit
    output with an optional `message` attribute, and use lowercase `skipped`
    status strings in JSON and the CLI while preserving long messages and
    consistent casing.

  - [ ] Document the `skip!` macro, the `@allow_skipped` tag and migration
    guidance for adopting Rust 1.75.

[design §1.3.4]: ./rstest-bdd-design.md#134-filtering-scenarios-with-tags

- [x] **Boilerplate Reduction**

  - [x] Implement the `scenarios!("path/to/features/")` macro to automatically
    discover all `.feature` files in a directory and generate a test module
    containing a test function for every `Scenario` found.

  - [x] Harden the `#[scenario]` macro's existing `name` selector with
    compile-time diagnostics: emit an error when the requested title is absent
    so bindings stay robust to feature reordering, and fall back to the index
    only when duplicate titles exist.

## Phase 4: Internationalization and Localization

This phase introduces full internationalization (i18n) and localization (l10n)
support, enabling the use of non-English Gherkin and providing translated
diagnostic messages.

- [x] **Foundational Gherkin Internationalization**

  - [x] Implement language detection in the feature file parser by recognizing
    and respecting the `# language: <lang>` declaration.

  - [x] Refactor keyword parsing to be language-aware, relying on the
    `gherkin` crate's `StepType` rather than hardcoded English strings.

  - [x] Add a comprehensive test suite with `.feature` files in multiple
    languages (e.g., French, German, Spanish) to validate correct parsing and
    execution. These tests run in CI to maintain coverage as languages are
    added.

- [x] **Localization of Library Messages with Fluent**

  - [x] Integrate the `i18n-embed`, `rust-embed`, and `fluent` crates.
  - [x] Enable required features:
        `i18n-embed = { features = ["fluent-system", "desktop-requester"] }`.
  - [x] Pin minimum supported versions in `Cargo.toml`.
  - [x] Add a minimal `Cargo.toml` example to the docs.

  - [x] Create `.ftl` resource files under an `i18n/` directory for all
    user-facing diagnostic messages. If the macros crate also emits messages,
    maintain a separate `i18n/` in `rstest-bdd-macros` or introduce a shared
    `rstest-bdd-i18n` crate to host common assets.

  - [x] Use `rust-embed` to bundle the localization resources directly into the
    library binary.

  - [x] Missing translation keys or unsupported locales fall back to English.

  - [x] Implement the `I18nAssets` trait on a dedicated struct to make Fluent
    resources discoverable.

  - [x] Keep procedural macro diagnostics in English for deterministic builds.
    Localize user-facing runtime messages using a `FluentLanguageLoader` at
    runtime.

- [ ] **Documentation and User Guidance**

  - [ ] Update `README.md` and `docs/users-guide.md` with a new section
    detailing how to use the internationalization features.

  - [ ] Add a new example crate to demonstrate writing and running a BDD test
    suite using a non-English language.

  - [ ] Update `CONTRIBUTING.md` with guidelines for adding and maintaining
    translations for new diagnostic messages.

## Phase 5: Ergonomics and Developer Experience

This phase focuses on reducing boilerplate and improving the developer
experience by introducing more powerful and intuitive APIs.

- [ ] **Ergonomic Improvements**

  - [x] **Implicit Fixture Injection:** Automatically inject fixtures when a
      step function's parameter name matches a fixture name, removing the need
      for `#[from(...)]` in most cases.
      [User guide](users-guide.md#implicit-fixture-injection) ·
      [trybuild](../crates/rstest-bdd-macros/tests/ui/implicit_fixture_missing.rs)

  - [x] **Inferred Step Patterns:** Allow step definition macros (`#[given]`,
    etc.) to be used without an explicit pattern string. The pattern will be
    inferred from the function's name (e.g., `fn user_logs_in()` becomes "user
    logs in"). [User’s guide](users-guide.md#inferred-step-patterns)
  - [x] **Streamlined `Result` Assertions:** Introduce helper macros like
    `assert_step_ok!` and `assert_step_err!` to reduce boilerplate when testing
    `Result`-returning steps.
  - [ ] **Refined `skip!` Macro:** Polish the macro's syntax and emit
    compile-time diagnostics when misused. Coverage: disallow usage outside a
    step or hook, reject calls from non-test threads, verify short-circuit
    behaviour, and preserve the message in writer outputs.
  - [ ] **Skipped-Step Assertions:** Provide helper macros for verifying that
    steps or scenarios were skipped as expected.

- [ ] **State Management and Data Flow**

  - [x] **Step Return Values:** Allow `#[when]` steps to return values, which
      can then be automatically injected into subsequent `#[then]` steps,
      enabling a more functional style of testing. Returned values override
      fixtures of the same type.

  - [ ] **Scenario State Management:** Introduce a `#[scenario_state]` derive
    macro and a `Slot<T>` type to simplify the management of shared state
    across steps, reducing the need for manual `RefCell<Option<T>>` boilerplate.

- [ ] **Advanced Ergonomics**

  - [ ] **Struct-based Step Arguments:** Introduce a `#[step_args]` derive
    macro to allow multiple placeholders from a step pattern to be parsed
    directly into the fields of a struct, simplifying step function signatures.

## Phase 6: Extensions & Tooling

These tasks can be addressed after the core framework is stable and are aimed
at improving maintainability and IDE integration.

- [x] **Diagnostic Tooling**

  - [x] Create a helper binary or `cargo` subcommand (`cargo bdd`).

  - [x] Implement a `list-steps` command to print the entire registered step
     registry.

  - [x] Implement a `list-unused` command to report definitions never executed.

  - [x] Implement a `list-duplicates` command to group duplicate definitions.

  - [ ] Report skipped scenarios and their reasons.

    - Provide a `cargo bdd skipped --reasons` subcommand that lists each
      skipped scenario with its file, line and message.

    - Allow `cargo bdd steps --skipped` to filter the step registry for
      definitions bypassed at runtime.

    - Both commands accept `--json` and emit objects with fields `feature`,
      `scenario`, `line`, `tags` and `reason`:

      ```json
      {
        "feature": "path/to/file.feature",
        "scenario": "scenario title",
        "line": 42,
        "tags": ["@allow_skipped"],
        "reason": "explanatory message"
      }
      ```

- [ ] **IDE Integration**

  - [ ] Investigate creating a `rust-analyzer` procedural macro server to
    provide autocompletion and "Go to Definition" from `.feature` files.

  - [ ] Alternatively, develop a dedicated VS Code extension to provide this
    functionality.

  - [ ] Surface skipped scenario information in IDE plug-ins using the JSON
    fields `feature`, `scenario`, `line`, `tags` and `reason`.

- [ ] **Advanced Hooks**

  - [ ] Explore adding explicit teardown hooks that are guaranteed to run after
    a scenario, even in the case of a panic (e.g., `#[after_scenario]`).

- [ ] **Performance Optimization**

  - [ ] Implement caching for parsed Gherkin ASTs in the `OUT_DIR` to reduce
    compile-time overhead, only re-parsing files on modification.
