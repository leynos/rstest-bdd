# Roadmap

This roadmap outlines the development plan for the `rstest-bdd` framework,
summarizes the provided design proposal and explains how work is staged into
phases. Phasing keeps implementation incremental and testing rigorous.

## 1. Core mechanics and proof of concept

The primary goal of this phase is to validate the core architectural decision:
using `inventory` for link-time collection of step definitions, which are then
discovered and executed by a procedural macro at runtime.

### 1.1. Project scaffolding

- [x] 1.1.1. Create a new Cargo workspace.
- [x] 1.1.2. Add the `rstest-bdd` library crate.
- [x] 1.1.3. Add the `rstest-bdd-macros` procedural macro crate.

### 1.2. Step registry implementation

- [x] 1.2.1. Define the `Step` struct within `rstest-bdd` to hold metadata
  (keyword, pattern, type-erased run function, source location).
- [x] 1.2.2. Use `inventory::collect!(Step)` to establish the global collection.

### 1.3. Step definition macros

- [x] 1.3.1. Implement the `#[given("...")]` attribute macro in
  `rstest-bdd-macros`.
- [x] 1.3.2. Implement the `#[when("...")]` attribute macro.
- [x] 1.3.3. Implement the `#[then("...")]` attribute macro.
- [x] 1.3.4. Ensure each macro generates an `inventory::submit!` block that
  constructs and registers a `Step` instance.

### 1.4. Scenario orchestrator macro (initial version)

- [x] 1.4.1. Implement a basic `#[scenario(path = "...")]` attribute macro.
- [x] 1.4.2. The macro must, at compile-time, read and parse the specified
  `.feature` file using the `gherkin` crate.
- [x] 1.4.3. The macro must generate a new test function annotated with
  `#[rstest]`.
- [x] 1.4.4. The body of the generated function must, at runtime, iterate
  through the scenario's Gherkin steps and find matching `Step` definitions
  from the `inventory::iter`.
- [x] 1.4.5. For this phase, only support exact, case-sensitive string matching
  with no argument parsing.

### 1.5. Validation

- [x] 1.5.1. Create a simple `web_search.feature` file.
- [x] 1.5.2. Create a `test_web_search.rs` file with corresponding step
  definitions.
- [x] 1.5.3. Create a test function annotated with `#[scenario]` that
  successfully runs the steps via `cargo test`.

## 2. Fixtures and parameterization

This phase focuses on integrating with `rstest`'s core features to manage state
and run data-driven tests, making the framework genuinely useful.

### 2.1. Fixture integration

- [x] 2.1.1. Enhance the step definition macros to inspect the signature of the
  attached function to identify requested fixtures.
- [x] 2.1.2. Modify the `#[scenario]` macro's code generation to correctly
  manage and pass fixtures to the step functions during execution.

### 2.2. Scenario outline support

- [x] 2.2.1. Extend the `#[scenario]` macro to detect `Scenario Outline` and its
  `Examples:` table in the parsed Gherkin Abstract Syntax Tree (AST).
- [x] 2.2.2. The macro generates a single, parameterized `#[rstest]` function.
- [x] 2.2.3. For each row in the `Examples:` table, the macro generates a
  corresponding `#[case(...)]` attribute.

### 2.3. Step argument parsing

- [x] 2.3.1. Implement a parser for `format!`-style placeholders (e.g.,
  `{count:u32}`).
- [x] 2.3.2. The runtime step-matching logic must extract values from the
  Gherkin step text based on these placeholders.
- [x] 2.3.3. Use the `FromStr` trait to convert the extracted string values into
  the types specified in the function signature.

## 3. Advanced Gherkin features and ergonomics

This phase aims for feature-parity with other mature Behaviour-Driven
Development (BDD) frameworks and improves the developer experience.

### 3.1. Advanced Gherkin constructs

- [x] 3.1.1. Implement support for `Background` sections, so their steps run
  before each `Scenario` in a feature file.
- [x] 3.1.2. Implement support for `Data Tables`, initially making the data
  available to the step function as a `Vec<Vec<String>>` (legacy baseline;
  typed support is planned below).
- [x] 3.1.3. Implement support for `Docstring`, making the content available as
  a `String` argument named `docstring`.

### 3.2. Robust error handling

- [x] 3.2.1. The `#[scenario]` macro must emit a `compile_error!` if the
  specified `.feature` file cannot be found or parsed.
- [x] 3.2.2. The `#[scenario]` macro must perform a compile-time check to ensure
  a matching step definition exists for every Gherkin step in the target
  scenario, emitting a `compile_error!` if any are missing.

### 3.3. Typed data table support

- [x] 3.3.1. Add a `datatable` runtime module exposing `DataTableError`,
  `HeaderSpec`, `RowSpec`, `Rows<T>`, and convenience parsers such as
  `truthy_bool` and `trimmed<T: FromStr>`.
- [x] 3.3.2. Implement `TryFrom<Vec<Vec<String>>> for Rows<T>` (with
  `T: DataTableRow`) to split optional headers, build index maps, and surface
  row and column context on errors.
- [x] 3.3.3. Provide `#[derive(DataTableRow)]` and `#[derive(DataTable)]` macros
  with field- and struct-level attributes for column mapping, optional or
  default cells, trimming, tolerant booleans, custom parsers, and row
  aggregation hooks.
- [x] 3.3.4. Update generated wrappers to forward conversion failures by
  formatting the `DataTableError` into the emitted `StepError`, ensuring
  diagnostics reach recorders.
- [x] 3.3.5. Extend documentation (user guide, design document) and add
  integration tests covering headered tables and tolerant boolean parsing.
- [x] 3.3.6. Add compile-fail fixtures covering optional columns and invalid
  attribute combinations.

### 3.4. Tag filtering

- [x] 3.4.1. Allow the `#[scenario]` macro to select scenarios by tag expression
  at macro-expansion time.
- [x] 3.4.2. Extend the `scenarios!` macro to filter scenarios using the same
  tag syntax at macro-expansion time. See [design §1.3.4].
- [x] 3.4.3. Document tag-expression grammar and precedence (§1.3.4).
- [x] 3.4.4. Filter at macro-expansion time and emit `compile_error!`
      diagnostics
  for invalid tag expressions (explicit empty string `""`, empty parentheses
  `()`, dangling operators). Omitting the `tags` argument applies no filter
  (`error: missing tag (allowed)`). Diagnostics include the byte offset and a
  short reason, e.g.: `error: empty tag string is not allowed (byte offset 42)`
  or `error: invalid tag expression at byte 7: expected tag or '(' after 'and'`.
- [x] 3.4.5. Define tag scope and inheritance:
  - Scenarios inherit `Feature:` tags.
  - `Scenario Outline` cases inherit tags from the outline and their originating
    `Examples:` block.
- [x] 3.4.6. Specify associativity (`and`/`or` left-associative; `not`
  unary-prefix) and reject unknown tokens (`&&`, `||`, `!`) at compile time.
- [x] 3.4.7. Specify case rules and identifier grammar:
  - Tag identifiers are case-sensitive and match `[A-Za-z_][A-Za-z0-9_]*`.
  - Operator keywords (`and`, `or`, `not`) are case-insensitive and reserved;
    they cannot be used as identifiers.
- [x] 3.4.8. Implement a single shared parser used by both macros to guarantee
  identical semantics.
- [x] 3.4.9. Support an `@allow_skipped` tag and add a `fail_on_skipped`
  configuration option. With this option, skipped scenarios only fail when the
  flag is set and the tag is absent.
- [x] 3.4.10. Add conformance tests for precedence, associativity, and scope:
  - Valid: `@a and not (@b or @c)`
  - Invalid: `@a && @b`, `""`, `()`, `@a and`, `(@a or @b`, `@a or and @b`

### 3.5. Rust 1.85 / Edition 2024 and skipping support

- [x] 3.5.1. Raise the minimum supported Rust version to 1.85 and remove the
  `async_trait` dependency from `World` and writer traits.
  - [x] Set `rust-version = "1.85"` in all Cargo manifests.
  - [x] Record that stable tooling supports Rust 2024 and that contributors
    should use the pinned toolchain for consistent formatting and linting.
  - [x] Remove `async-trait` from dependencies and code imports.
  - [x] Add a Continuous Integration (CI) check that fails if `async-trait`
    reappears.
- [x] 3.5.2. Provide a `skip!` macro that records a `Skipped` outcome and
  short-circuits remaining steps.
- [x] 3.5.3. Expose skipped status through `cargo-bdd` and the JSON and JUnit
  writers. Emit a `<skipped>` child on each `<testcase>` element in JUnit
  output with an optional `message` attribute, and use lowercase `skipped`
  status strings in JSON and the CLI while preserving long messages and
  consistent casing.
- [x] 3.5.4. Document the `skip!` macro, the `@allow_skipped` tag and migration
  guidance for adopting Rust 1.85 / edition 2024.

[design §1.3.4]: ./rstest-bdd-design.md#134-filtering-scenarios-with-tags
[implicit-fixture-guide]: users-guide.md#implicit-fixture-injection
[implicit-fixture-trybuild]:
../crates/rstest-bdd-macros/tests/ui/implicit_fixture_missing.rs

### 3.6. Boilerplate reduction

- [x] 3.6.1. Implement the `scenarios!("path/to/features/")` macro to
  automatically discover all `.feature` files in a directory and generate a
  test module containing a test function for every `Scenario` found.
- [x] 3.6.2. Harden the `#[scenario]` macro's existing `name` selector with
  compile-time diagnostics: emit an error when the requested title is absent so
  bindings stay robust to feature reordering, and fall back to the index only
  when duplicate titles exist.

## 4. Internationalization and localization

This phase introduces full internationalization (i18n) and localization (l10n)
support, enabling the use of non-English Gherkin, and providing translated
diagnostic messages.

### 4.1. Foundational Gherkin internationalization

- [x] 4.1.1. Implement language detection in the feature file parser by
  recognizing and respecting the `# language: <lang>` declaration.
- [x] 4.1.2. Refactor keyword parsing to be language-aware, relying on the
  `gherkin` crate's `StepType` rather than hardcoded English strings.
- [x] 4.1.3. Add a comprehensive test suite with `.feature` files in multiple
  languages (e.g., French, German, Spanish) to validate correct parsing and
  execution. These tests run in CI to maintain coverage as languages are added.

### 4.2. Localization of library messages with Fluent

- [x] 4.2.1. Integrate the `i18n-embed`, `rust-embed`, and `fluent` crates.
- [x] 4.2.2. Enable required features:
  `i18n-embed = { features = ["fluent-system", "desktop-requester"] }`.
- [x] 4.2.3. Pin minimum supported versions in `Cargo.toml`.
- [x] 4.2.4. Add a minimal `Cargo.toml` example to the docs.
- [x] 4.2.5. Create `.ftl` resource files under an `i18n/` directory for all
  user-facing diagnostic messages. If the macros crate also emits messages,
  maintain a separate `i18n/` in `rstest-bdd-macros` or introduce a shared
  `rstest-bdd-i18n` crate to host common assets.
- [x] 4.2.6. Use `rust-embed` to bundle the localization resources directly into
  the library binary.
- [x] 4.2.7. Missing translation keys or unsupported locales fall back to
  English.
- [x] 4.2.8. Implement the `I18nAssets` trait on a dedicated struct to make
  Fluent resources discoverable.
- [x] 4.2.9. Keep procedural macro diagnostics in English for deterministic
  builds. Localize user-facing runtime messages using a `FluentLanguageLoader`
  at runtime.

### 4.3. Documentation and user guidance

- [x] 4.3.1. Update `README.md` and `docs/users-guide.md` with a new section
  detailing how to use the internationalization features.
- [x] 4.3.2. Add a new example crate to demonstrate writing and running a BDD
  test suite using a non-English language.
- [x] 4.3.3. Update `CONTRIBUTING.md` with guidelines for adding and maintaining
  translations for new diagnostic messages.

## 5. Ergonomics and developer experience

This phase focuses on reducing boilerplate and improving the developer
experience by introducing more powerful and intuitive APIs.

### 5.1. Ergonomic improvements

- [x] 5.1.1. Implicit fixture injection: Automatically inject fixtures when a
  step function's parameter name matches a fixture name, removing the need for
  `#[from(...)]` in most cases. [user guide][implicit-fixture-guide] ·
  [trybuild][implicit-fixture-trybuild]
- [x] 5.1.2. Inferred step patterns: Allow step definition macros (`#[given]`,
  etc.) to be used without an explicit pattern string. The pattern will be
  inferred from the function's name (e.g., `fn user_logs_in()` becomes "user
  logs in"). [user guide](users-guide.md#inferred-step-patterns)
- [x] 5.1.3. Streamlined `Result` assertions: Introduce helper macros like
  `assert_step_ok!` and `assert_step_err!` to reduce boilerplate when testing
  `Result`-returning steps.
- [x] 5.1.4. Refined `skip!` macro: Polish the macro's syntax and surface clear
  diagnostics when misused. Coverage: disallow usage outside a step or hook
  (panic with a descriptive message), reject calls from non-test threads,
  verify short-circuit behaviour, and preserve the message in writer outputs.
- [x] 5.1.5. Skipped-step assertions: Provide helper macros for verifying that
  steps or scenarios were skipped as expected.
- [x] 5.1.6. Fallible scenario bodies: Allow `#[scenario]` functions to return
  `Result<(), E>` or `StepResult<(), E>`, returning `Ok(())` for skipped
  scenarios and ensuring `Err` outcomes do not record a pass.

### 5.2. State management and data flow

- [x] 5.2.1. Step return values: Allow `#[when]` steps to return values, which
  can then be automatically injected into subsequent `#[then]` steps, enabling
  a more functional style of testing. Returned values override fixtures of the
  same type.
- [x] 5.2.2. Scenario state management: Introduce a `#[derive(ScenarioState)]`
  macro and a `Slot<T>` type to simplify the management of shared state across
  steps, reducing the need for manual `RefCell<Option<T>>` boilerplate.

### 5.3. Advanced ergonomics

- [x] 5.3.1. Struct-based step arguments: Introduce a `#[step_args]` derive
  macro to allow multiple placeholders from a step pattern to be parsed
  directly into the fields of a struct, simplifying step function signatures.

## 6. Extensions and tooling

These tasks can be addressed after the core framework is stable and are aimed
at improving maintainability and IDE integration.

### 6.1. Diagnostic tooling

- [x] 6.1.1. Create a helper binary or `cargo` subcommand (`cargo bdd`).
- [x] 6.1.2. Implement a `list-steps` command to print the entire registered
  step registry.
- [x] 6.1.3. Implement a `list-unused` command to report definitions never
  executed.
- [x] 6.1.4. Implement a `list-duplicates` command to group duplicate
  definitions.
- [x] 6.1.5. Report skipped scenarios and their reasons.
  - Provide a `cargo bdd skipped --reasons` subcommand that lists each skipped
    scenario with its file, line, and message.
  - Allow `cargo bdd steps --skipped` to filter the step registry for
    definitions bypassed at runtime.
  - Both commands accept `--json` and emit objects with fields `feature`,
    `scenario`, `line`, `tags`, and `reason`:

    ```json
    {
      "feature": "path/to/file.feature",
      "scenario": "scenario title",
      "line": 42,
      "tags": ["@allow_skipped"],
      "reason": "explanatory message"
    }
    ```

### 6.2. IDE integration

- [ ] 6.2.1. Investigate creating a `rust-analyzer` procedural macro server to
  provide autocompletion and "Go to Definition" from `.feature` files.
- [ ] 6.2.2. Alternatively, develop a dedicated VS Code extension to provide
  this functionality.
- [ ] 6.2.3. Surface skipped scenario information in IDE plug-ins using the JSON
  fields `feature`, `scenario`, `line`, `tags` and `reason`.

### 6.3. Advanced hooks

- [ ] 6.3.1. Explore adding explicit teardown hooks that are guaranteed to run
  after a scenario, even in the case of a panic (e.g., `#[after_scenario]`).

### 6.4. Performance optimization

- [ ] 6.4.1. Implement caching for parsed Gherkin ASTs in the `OUT_DIR` to
  reduce compile-time overhead, only reparsing files on modification.

## 7. Language server foundations

This phase delivers the first `rstest-bdd-server` release, focused on
navigation between Rust step definitions and Gherkin features, plus on-save
consistency diagnostics. Real-time analysis and autocomplete remain out of
scope until the core workflow is stable.

### 7.1. Server scaffolding

- [x] 7.1.1. Add a new `rstest-bdd-server` crate (binary `rstest-bdd-lsp`) that
  depends on `async-lsp`, `gherkin`, and the shared pattern parser to align
  semantics with the macros.
- [x] 7.1.2. Implement Language Server Protocol (LSP) initialize/shutdown
  handlers, crate-root discovery via `cargo metadata`, and structured logging
  configurable through environment variables.

### 7.2. Indexing pipeline

- [x] 7.2.1. Parse `.feature` files with `gherkin` on save to capture steps, doc
  strings, tables, example columns, and byte offsets.
- [x] 7.2.2. Parse Rust files with `syn` to collect `#[given]`, `#[when]`, and
  `#[then]` functions, including pattern strings, keyword, parameter list, and
  expectations for tables or doc strings.
- [x] 7.2.3. Compile step patterns with `rstest-bdd-patterns` and populate an
  in-memory registry keyed by keyword, invalidated incrementally on file change
  notifications.

### 7.3. Navigation handlers

- [x] 7.3.1. Implement `textDocument/definition` to jump from a Rust step
  function to every matching feature step using keyword-aware regex matching.
- [x] 7.3.2. Implement `textDocument/implementation` to jump from a feature step
  to all matching Rust implementations, returning multiple locations when
  duplicates exist.

### 7.4. Diagnostics (on save)

- [x] 7.4.1. Emit diagnostics for unimplemented feature steps and unused step
  definitions by cross-referencing the registry.
- [x] 7.4.2. Validate placeholder counts, typed placeholders, and data table or
  docstring expectations against function signatures, emitting precise byte
  offsets in the source.
- [x] 7.4.3. Check scenario outline example columns against referenced
  parameters, flagging missing or surplus columns in either the feature or test
  binding.

### 7.5. Packaging and editor enablement

- [x] 7.5.1. Ship CLI options for log level, workspace root, and debounce
  interval; document VS Code, Zed, and Neovim launch examples in
  `docs/rstest-bdd-language-server-design.md` and the user guide. Finish line:
  `rstest-bdd-lsp --help` lists all three flags, and the user guide contains
  working editor snippets for VS Code, Neovim, and Zed. See
  `rstest-bdd-language-server-design.md` §7.5 "Packaging and editor enablement".
- [x] 7.5.2. Add smoke tests that start the server, answer a definition
  request, and emit diagnostics for one feature file; gate them in CI. Finish
  line: three smoke tests pass in `make test` (initialize/shutdown, definition
  navigation, diagnostic publication). See
  `rstest-bdd-language-server-design.md` §7.5 "Smoke tests".

## 8. Async step execution

This phase introduces Tokio-based asynchronous scenario execution, enabling
async test functions with proper fixture integration under the Tokio runtime.
Multi-thread mode remains out of scope until the fixture storage model is
redesigned. For the full architectural decision record, see
[ADR-001](adr-001-async-fixtures-and-test.md).

> **Note:** This phase includes native async step bodies. Async scenarios await
> `AsyncStepFn` step handlers sequentially, while synchronous steps run through
> the sync handler directly for minimal overhead. Async-only steps can still be
> executed from synchronous scenarios via a blocking fallback (with safeguards
> against nested Tokio runtimes).

### 8.1. Async step registry

- [x] 8.1.1. Define `StepFuture<'a>` type alias for the step wrapper return
  type.
- [x] 8.1.2. Implement `AsyncStepFn` wrapper type that returns `StepFuture`.
- [x] 8.1.3. Update `Step` struct to store async step wrappers alongside sync.
- [x] 8.1.4. Generate wrapper code that normalizes sync step definitions into
  the async interface, wrapping results in immediately ready futures.

### 8.2. Tokio current-thread integration

- [x] 8.2.1. Add `runtime` argument to `scenarios!` macro accepting
  `"tokio-current-thread"`.
- [x] 8.2.2. Generate `#[tokio::test(flavor = "current_thread")]` attribute for
  async scenario tests.
- [x] 8.2.3. Preserve `RefCell`-backed fixture model for mutable borrows across
  `.await` points.
- [x] 8.2.4. Support `#[scenario]` macro with explicit `#[tokio::test]`
  annotation for manual async scenario tests.

### 8.3. Unwind and skip handling

- [x] 8.3.1. Reuse sync unwind/skip handling by calling sync handler directly.
- [x] 8.3.2. Preserve `skip!` interception (sync path remains unchanged).
- [x] 8.3.3. Maintain panic context (step index, keyword, text,
  feature/scenario metadata) in async error reports.
- [x] 8.3.4. Support `async fn` step definitions by generating async wrappers
  that await the user step future.
- [x] 8.3.5. Update the async scenario executor to await `execute_step_async`
  for each step.
- [x] 8.3.6. Provide a blocking sync fallback for async-only steps in
  synchronous scenarios, with safeguards against nested Tokio runtimes.

### 8.4. Documentation and migration

- [x] 8.4.1. Document async scenario execution in the user guide (see
  [users-guide.md §Async scenario execution](users-guide.md#async-scenario-execution)).
- [x] 8.4.2. Document Tokio current-thread limitations (blocking operations,
  nested runtimes, `spawn_local` patterns) in the design document §2.5.
- [x] 8.4.3. Update design document §2.5 with implementation status.

## 9. Harness adapters and attribute plugins

This phase implements ADR-005 by introducing a harness adapter layer and an
attribute policy plugin interface, so Tokio and GPUI integrations live in
opt-in crates rather than the core runtime or macros.

### 9.1. Harness adapter core

- [x] 9.1.1. Add `rstest-bdd-harness` with the harness adapter trait and shared
  runner types.
- [x] 9.1.2. Provide `StdHarness` as the default synchronous implementation.
- [x] 9.1.3. Define the attribute policy plugin interface and a default policy
  that emits only `#[rstest::rstest]`.

### 9.2. Macro integration

- [ ] 9.2.1. Extend `#[scenario]` and `scenarios!` with
  `harness = path::ToHarness` and optional `attributes = path::ToPolicy`.
- [ ] 9.2.2. Delegate scenario execution to the selected harness adapter.
- [ ] 9.2.3. Treat `runtime = "tokio-current-thread"` as a compatibility alias
  for the Tokio harness adapter.

### 9.3. Tokio harness plugin crate

- [ ] 9.3.1. Create `rstest-bdd-harness-tokio`.
- [ ] 9.3.2. Move Tokio runtime wiring and async entry points into the adapter.
- [ ] 9.3.3. Provide a Tokio attribute policy plugin (current-thread flavour).

### 9.4. GPUI harness plugin crate

- [ ] 9.4.1. Create `rstest-bdd-harness-gpui`.
- [ ] 9.4.2. Execute scenarios inside the GPUI test harness and inject fixtures
  such as `TestAppContext`.
- [ ] 9.4.3. Provide the matching GPUI test attribute policy plugin.

### 9.5. Documentation and validation

- [ ] 9.5.1. Add a harness adapter chapter to the user guide and design docs.
- [ ] 9.5.2. Add integration tests covering harness selection and attribute
  policy resolution for Tokio and GPUI.
- [ ] 9.5.3. Document the extension point for future harness plugins (for
  example, `rstest-bdd-harness-bevy`).
