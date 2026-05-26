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
- [x] 5.1.7. Normalize a single leading underscore consistently for implicit
  fixture keys derived from parameter names across `#[scenario]` fixture
  registration and step wrapper extraction, while keeping `#[from(...)]`
  authoritative. Reuse the existing `normalize_param_name()` helper so `_world`
  behaves like `world`, `__world` continues to mean `_world`, and the runtime
  missing-fixture diagnostics no longer diverge between scenario and step macro
  layers. Finish line: implicit fixture lookup follows one rule in both
  directions, coverage proves the scenario and step paths agree, and the user
  guide documents the rule and its `#[from(...)]` escape hatch. Design Doc:
  `docs/adr-009-consistent-implicit-fixture-name-normalization.md`.
  Prerequisite: ADR-009 accepted. (Telefono, Pandalump)

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

- [x] 9.2.1. Extend `#[scenario]` and `scenarios!` with
  `harness = path::ToHarness` and optional `attributes = path::ToPolicy`.
- [x] 9.2.2. Delegate scenario execution to the selected harness adapter.
- [x] 9.2.3. Treat `runtime = "tokio-current-thread"` as a compatibility alias
  for the Tokio harness adapter.
- [x] 9.2.4. Activate the `runtime = "tokio-current-thread"` compatibility
  alias so that `resolve_harness_path` resolves it to
  `rstest_bdd_harness_tokio::TokioHarness`. Update the doc comment on
  `resolve_harness_path` and the test
  `resolve_harness_path_runtime_alias_resolves_to_tokio_harness` to reflect the
  new resolved behaviour. Emit a deprecation warning, recommending
  `harness = rstest_bdd_harness_tokio::TokioHarness` as the canonical form.
  Delivered 2026-03-16. The alias now resolves to `TokioHarness` path and
  generates synchronous scenario test functions; the deprecated comment is
  updated; deprecation warning emitted via `emit_warning!`; behavioural tests
  (`runtime_compat_alias.rs`, `async_scenario.rs`) and documentation updated.
  Prerequisite: 9.3.2 delivered. Design Doc: §2.5.5, §2.7.3. (Doggylump,
  DevBoxer)

### 9.3. Tokio harness plugin crate

- [x] 9.3.1. Create `rstest-bdd-harness-tokio`. Finish line: crate exists as a
  workspace member, exports `TokioHarness` and `TokioAttributePolicy`, and all
  quality gates pass. Prerequisite: 9.2 complete. Design Doc: §2.7.4.
- [x] 9.3.2. Move Tokio runtime wiring and async entry points into the adapter.
  Finish line: `TokioHarness` implements `HarnessAdapter` using a
  current-thread runtime with `LocalSet`; unit and behavioural tests pass.
  Prerequisite: 9.3.1 scaffold. Design Doc: §2.7.4.
- [x] 9.3.3. Provide a Tokio attribute policy plugin (current-thread flavour).
  Finish line: `TokioAttributePolicy` emits `#[rstest::rstest]` and
  `#[tokio::test(flavor = "current_thread")]`; unit and behavioural tests pass.
  Prerequisite: 9.3.1 scaffold. Design Doc: §2.7.4.
- [x] 9.3.4. Wire `AttributePolicy::test_attributes()` into macro codegen. The
  macro currently ignores the attribute policy and always emits only
  `#[rstest::rstest]`. Update `assemble_test_tokens_with_harness` (and the
  non-harness path) to call `test_attributes()` on the resolved policy and emit
  the returned attributes on the generated test function. Finish line: a
  scenario using `attributes = TokioAttributePolicy` emits
  `#[tokio::test(flavor = "current_thread")]` in expanded output; existing
  tests continue to pass. Prerequisite: 9.3.3 delivered. Design Doc: §2.7.2,
  §2.7.3. (Pandalump)
- [x] 9.3.5. Document the `yield_now` single-tick drain limitation in
  `TokioHarness::run`. The current implementation yields once after
  `request.run()`, which is sufficient for single-poll `spawn_local` tasks but
  may not drive multi-poll futures to completion. Either strengthen the drain
  logic (e.g. loop until the `LocalSet` is idle) or add a doc comment and
  user-guide note explaining the constraint and recommending `.await`-based
  patterns inside steps for reliable completion. Finish line: limitation is
  documented or drain logic is hardened; behavioural test validates the chosen
  approach. Prerequisite: 9.3.2 delivered. Design Doc: §2.7.4. (Buzzy Bee)
- [x] 9.3.6. Add `StdHarness` behavioural tests for parity with `TokioHarness`
  coverage. `StdHarness` currently has no dedicated behavioural tests beyond
  being the implicit default. Add tests exercising metadata forwarding, closure
  execution, and panic propagation. Finish line: at least three behavioural
  tests for `StdHarness` pass in `make test`. Prerequisite: 9.1.2 delivered.
  Design Doc: §2.7.1. (Dinolump)
- [x] 9.3.7. Add a negative integration test for `async fn` step definitions
  combined with `harness = TokioHarness`. Verify that this combination produces
  the expected compile-time error via a `trybuild` compile-fail fixture. Finish
  line: `trybuild` test asserts the diagnostic message; `make test` passes.
  Prerequisite: 9.3.2 delivered. Design Doc: §2.7.3, §2.7.4. (Doggylump)
- [x] 9.3.8. Add a simple Tokio async demonstration application and BDD test
  suite under `examples/`, similar in scope to the existing demonstration
  applications. The example should exercise `TokioHarness` and
  `TokioAttributePolicy` in an end-to-end crate with asynchronous application
  behaviour and feature scenarios. Finish line: a new example crate and its BDD
  suite run successfully via `make test`, and the user guide links to it as the
  canonical Tokio harness example. Delivered via `examples/tokio-reminders`.
  Prerequisite: 9.3.4 delivered. Design Doc: §2.7.4. (Dinolump)

### 9.4. GPUI harness plugin crate

- [x] 9.4.1. Design the fixture injection mechanism for framework harnesses.
  The original `HarnessAdapter::run` signature wrapped a `FnOnce() -> T`
  closure that was opaque to the harness — the harness could not inject
  framework-specific resources (e.g. `TestAppContext`, `bevy::ecs::World`) into
  step functions. Produce an ADR evaluating approaches (thread-local
  convention, associated `Context` type on `HarnessAdapter`, or a `StepContext`
  extension trait) and select one that works for both GPUI and Bevy. Finish
  line: ADR is merged and the chosen approach is reflected in the
  `HarnessAdapter` trait (or documented as a convention). Delivered via
  `docs/adr-007-harness-context-injection.md`. Prerequisite: 9.3.4 delivered
  (attribute wiring unblocks full policy integration). (Telefono)
- [x] 9.4.2. Create `rstest-bdd-harness-gpui`.
- [x] 9.4.3. Execute scenarios inside the GPUI test harness and inject fixtures
  such as `TestAppContext`.
- [x] 9.4.4. Provide the matching GPUI test attribute policy plugin.
- [x] 9.4.5. Add a simple GPUI demonstration application and BDD test suite
  under `examples/`, similar in scope to the existing demonstration
  applications. The example should exercise `GpuiHarness` and
  `GpuiAttributePolicy` end-to-end and demonstrate step access to injected
  `TestAppContext`. Finish line: a new example crate and its BDD suite run
  successfully via `make test`, with any required native-library setup clearly
  documented in the user guide. Prerequisite: 9.4.4 delivered. Design Doc:
  §2.7.4. Delivered via `examples/gpui-counter`. (Dinolump)

### 9.5. Context injection mechanism

- [x] 9.5.1. Write ADR-007: Harness context injection. Evaluate three
  approaches — (a) thread-local convention, (b) associated `Context` type on
  `HarnessAdapter`, and (c) `StepContext` extension trait — and select one. The
  recommended direction is an associated `Context` type:

  ```rust
  pub trait HarnessAdapter {
      type Context: 'static;
      fn run<T>(
          &self,
          request: ScenarioRunRequest<'_, Self::Context, T>,
      ) -> T;
  }
  ```

  This is a breaking change to the `HarnessAdapter` trait but provides
  type-safe, per-harness fixture injection without thread-local indirection.
  The ADR must address migration of `StdHarness` (where `Context = ()`),
  `TokioHarness`, and the impact on macro codegen. Finish line: ADR-007 is
  merged. Delivered via `docs/adr-007-harness-context-injection.md`.
  Prerequisite: 9.4.1 design task complete. (Telefono, Pandalump)
- [x] 9.5.2. Implement `HarnessAdapter::Context` in `rstest-bdd-harness`.
  Update the trait, `StdHarness` (`Context = ()`), `ScenarioRunRequest`, and
  macro codegen to thread the context type through. Finish line: existing tests
  pass with `StdHarness` and `TokioHarness` updated; `Context` is available
  inside the runner closure. Delivered via `HarnessAdapter::Context`,
  `ScenarioRunRequest<'_, C, T>`, and macro harness codegen updates.
  Prerequisite: ADR-007 merged. (Pandalump)
- [x] 9.5.3. Update `TokioHarness` to use `Context` (e.g.
  `Context = tokio::runtime::Handle` or `()`) and validate that `spawn_local`
  patterns still work. Finish line: Tokio harness tests pass with the new trait
  surface. Delivered with `TokioHarness` implementing
  `HarnessAdapter<Context = ()>` and updated behavioural coverage.
  Prerequisite: 9.5.2. (Buzzy Bee)
- [x] 9.5.4. Make `HarnessAdapter::run` return
  `Result<T, HarnessError>` so harness initialization failures are propagated
  rather than panicked. Finish line: `HarnessError` type in
  `rstest-bdd-harness`, `HarnessResult` alias re-exported from crate root, all
  first-party harnesses and macro-generated delegation updated,
  `FailingHarness` integration test added, ADR-007 and user/developer guides
  updated. Closes `#443`. (Pandalump)

### 9.6. Documentation and validation

- [x] 9.6.1. Update the harness adapter chapter in the user guide and design
  docs to reflect delivered 9.3 outcomes, the attribute policy wiring (9.3.4),
  and the context injection mechanism (9.5). Finish line: `docs/users-guide.md`
  now leads with explicit harness and attribute-policy configuration, documents
  the Tokio compatibility alias as deprecated legacy syntax, records
  `rstest_bdd_harness_context`-based context injection, and clarifies the
  current first-party policy-resolution trust model for Tokio and GPUI.
  `docs/rstest-bdd-design.md` records the delivered architecture and validation
  surface, and `make check-fmt`, `make lint`, and `make test` pass.
  Prerequisite: 9.5.3. Delivered 2026-03-22. (Pandalump)
- [x] 9.6.2. Add integration tests covering attribute policy resolution for
  GPUI once 9.4 is delivered. Prerequisite: 9.4.3. (Pandalump)
- [x] 9.6.3. Add a third-party harness cookbook documenting how to write a
  custom `HarnessAdapter` (for example, `rstest-bdd-harness-bevy`), including
  the `Context` type, attribute policy, and `Cargo.toml` configuration. Finish
  line: cookbook section in the user guide with a working example. Delivered
  2026-05-08. (Dinolump)

### 9.7. Harness-led attribute-policy defaults

These items are gated on ADR-008 being accepted. While
`docs/adr-008-harness-led-attribute-policy-defaults.md` remains in `Proposed`
status, treat the tasks below as contingent planning items rather than active
implementation commitments.

- [x] 9.7.1. Extend first-party policy hint resolution so known harness paths
  can imply default test-attribute hints when `attributes = ...` is omitted.
  Add canonical mappings for `StdHarness`, `TokioHarness`, and `GpuiHarness`,
  and implement the precedence rules defined in ADR-008. Finish line: shared
  helpers resolve the same hint from either the first-party harness path or the
  first-party attribute-policy path, with unit tests for unknown third-party
  paths and precedence edge cases. Prerequisite: ADR-008 accepted; 9.3.4 and
  9.4.4 delivered. Design Doc:
  `docs/adr-008-harness-led-attribute-policy-defaults.md`,
  `docs/rstest-bdd-design.md` §2.7.3. Delivered 2026-05-08. The shared policy
  resolver now has regression coverage proving first-party harness paths and
  their matching attribute-policy paths resolve to the same hints, with exact
  matching negative cases for unknown and wrong-prefix harness paths.
  Delivered under maintainer authorization while ADR-008 remains in Proposed
  status; the prerequisite will be formally satisfied when ADR-008 is accepted.
  (Pandalump)
- [x] 9.7.2. Update `#[scenario]` and `scenarios!` code generation so
  first-party harnesses imply their default attribute policies when
  `attributes = ...` is omitted, while explicit `attributes = ...` remains
  authoritative. Preserve `attributes`-only configuration, harness-only
  configuration, current attribute de-duplication rules, and the ADR-008
  precedence order. Finish line: generated test attributes for `StdHarness`,
  `TokioHarness`, and `GpuiHarness` match their first-party defaults without
  requiring paired `attributes = ...`, and explicit override scenarios still
  expand correctly. Prerequisite: 9.7.1. Design Doc:
  `docs/adr-008-harness-led-attribute-policy-defaults.md`,
  `docs/rstest-bdd-design.md` §2.7.3. Delivered 2026-05-14. The codegen path
  already routed both `#[scenario]` and `scenarios!` through the ADR-008
  resolver; this item adds regression coverage for synchronous Tokio harness
  omission, first-party de-duplication, and harness-only Tokio `scenarios!`
  expansion with an async step. Delivered under maintainer authorization while
  ADR-008 remains in Proposed status; the prerequisite will be formally
  satisfied when ADR-008 is accepted. (Pandalump, Doggylump)
- [x] 9.7.3. Add unit, trybuild, and behavioural coverage for harness-led
  defaults and explicit overrides across the first-party harnesses. Cover
  harness-only scenarios, explicit override scenarios, attributes-only
  scenarios, and unknown third-party harness paths where relevant. Finish line:
  tests prove that harness-only Tokio and GPUI scenarios receive their
  first-party test attributes when the generated signature permits it, explicit
  overrides win, and `attributes`-only behaviour remains unchanged.
  Prerequisite: 9.7.2. Design Doc:
  `docs/adr-008-harness-led-attribute-policy-defaults.md`. Delivered
  2026-05-21 with unit coverage for precedence and unknown-path negatives,
  trybuild fixtures for first-party override and attributes-only expansion,
  and behavioural Tokio and GPUI scenario coverage. (Buzzy Bee)
- [x] 9.7.4. Update the user guide, design document, and first-party example
  prose to lead with harness-only configuration once the default-inference
  behaviour lands. Retain `attributes = ...` as an override pattern and keep
  the current third-party caveats explicit. Finish line: `docs/users-guide.md`
  and `docs/rstest-bdd-design.md` recommend harness-led defaults for the
  first-party integrations, examples no longer require both parameters by
  default, and `make markdownlint` passes. Prerequisite: 9.7.3. Design Doc:
  `docs/adr-008-harness-led-attribute-policy-defaults.md`. Delivered
  2026-05-26 with harness-only Tokio and GPUI example code, updated guide,
  design, and migration prose, focused example tests, full repository gates,
  and CodeRabbit review. (Dinolump)

## 10. First-cut beta feedback: v0.6.0-beta2 quick wins

The first downstream beta migration showed that the harness architecture is
usable, but that stateful GPUI adoption needs clearer defaults, diagnostics,
and examples before the final v0.6.0 release. This phase prioritizes small,
non-breaking changes that make the current model easier to adopt without
changing the public trait contracts.

### 10.1. Remove avoidable harness adoption friction

- [x] 10.1.1. Users of first-party adapters can depend only on the adapter
  crate in `Cargo.toml`; `rstest-bdd-harness` is required only for custom
  harness implementations or explicit use of the base harness API. Finish line:
  `docs/v0-6-0-migration-guide.md` contains the plain BDD, Tokio, GPUI, and
  custom harness dependency matrix, and fixture-generation tests or docs prove
  first-party adapters compile without a direct base-harness dependency. Design
  Doc: `docs/rstest-bdd-design.md` §2.7.6.3. (Dinolump)
- [x] 10.1.2. Provide detailed missing-fixture diagnostics that include the
  requested fixture name and type, the list of inserted fixtures from
  `StepContext::available_fixtures()`, and, when
  `rstest_bdd_harness_context` is absent, a suggested harness to select. Finish
  line: a regression test reproduces the missing-fixture failure and asserts
  the diagnostic contains the requested fixture name, requested type, inserted
  fixture list, and harness suggestion. Design Doc: `docs/rstest-bdd-design.md`
  §2.7.6.3. (Telefono)
- [x] 10.1.3. The feature-gated GPUI test suite provides realistic harness
  regression coverage beyond the counter example: it creates a window, persists
  durable entity/window handles, reconstructs visual context per step, resets
  scenario state before assignment, and documents the reset protocol in
  comments. Finish line: the automated GPUI suite passes in CI with a scenario
  that creates a window, carries handles across steps, reconstructs visual
  context, and includes reset-protocol comments. Prerequisite: 9.4.5. Design
  Doc: `docs/rstest-bdd-design.md` §2.7.6.2. (Doggylump)
- [ ] 10.1.4. Failing GPUI scenarios include the scenario name in logs where
  `GpuiHarness` and `gpui::TestAppContext` permit it, or the harness docs
  document the upstream limitation, so developers can quickly orientate failing
  scenarios. Finish line: a failing-harness regression asserts the scenario
  name appears in emitted diagnostics, or the GPUI harness docs state the
  upstream limitation and link the skipped test. Prerequisite: 9.4.3. Design
  Doc: `docs/rstest-bdd-design.md` §2.7.5. (Buzzy Bee)

### 10.2. Update adoption documentation before v0.6.0 final

- [ ] 10.2.1. Users can migrate a stateful GPUI test without reading macro
  expansion or GPUI harness source. The user guide and migration guide cover
  `GpuiHarness`, the reserved harness-context fixture key, durable
  entity/window handles, `VisualTestContext` reconstruction, and the explicit
  world-reset protocol. Finish line: `make markdownlint` passes and the user
  guide plus migration guide each include a GPUI playbook covering all listed
  topics. Design Doc: `docs/rstest-bdd-design.md` §2.7.6.2. (Dinolump)
- [ ] 10.2.2. The migration guide provides a troubleshooting entry explaining
  the `E0499`/`E0502` symptoms for two mutable `StepContext` fixtures, why the
  pattern fails, and recommended workarounds before downstream users reach
  compiler-error archaeology. Finish line: `docs/v0-6-0-migration-guide.md`
  contains the troubleshooting entry and links to the borrow-constraint design
  subsection. Design Doc: `docs/rstest-bdd-design.md` §2.7.6.1. (Telefono)
- [ ] 10.2.3. The v0.6.0 migration guide warns users to run downstream tests
  through the repository's CI-equivalent gate and to run feature-gated tests,
  such as `cargo test --all-features` or a project `make test`, before API
  diagnosis. Finish line: the migration checklist names both command shapes and
  `make markdownlint` passes. Design Doc: `docs/rstest-bdd-design.md` §2.7.6.3.
  (Doggylump)

## 11. Early life support: v0.6.1 additive hardening

The v0.6.1 line should stay semver-compatible. It can add helper APIs,
diagnostics, examples, and generated-wrapper improvements, but it must not
remove the existing `StepContext`, harness, or macro surfaces.

### 11.1. Add borrow and state helpers without breaking callers

- [ ] 11.1.1. `FixtureBorrowError` provides a structured error surface for
  generated and manual fixture extraction, with variants for missing fixture,
  type mismatch, immutable fixture requested mutably, and already-borrowed
  fixture cases, so generated wrappers produce targeted diagnostics instead of
  collapsing every extraction failure into `MissingFixture`. Finish line: unit
  tests cover every variant, and generated-wrapper tests assert each variant maps
  to the expected diagnostic. Design Doc: `docs/rstest-bdd-design.md` §2.7.6.4.
  (Telefono)
- [ ] 11.1.2. Generated code has an additive mutable-borrow helper that reduces
  unnecessary `&mut StepContext` contention where possible while preserving the
  existing `borrow_mut(&mut self, ...)` API. Regression tests cover mutable
  harness context plus scenario state, or docs explain precisely why the full
  fix must wait for v0.7.0. Finish line: generated-code tests pass for the
  helper without breaking the existing `borrow_mut` API, or the documented
  deferral includes a failing-shape test. Design Doc: `docs/rstest-bdd-design.md`
  §2.7.6.4. (Pandalump)
- [ ] 11.1.3. The scenario-local state helper exposes `set`, `with`,
  `with_mut`, `take`, and `reset` operations for complex adapters without
  per-scenario thread-local `RefCell` boilerplate. Unit tests cover the helper,
  and docs present it as an additive alternative to ad-hoc GPUI world state.
  Finish line: unit tests exercise all five operations, and docs show a GPUI
  world-state example using the helper. Design Doc: `docs/rstest-bdd-design.md`
  §2.7.6.4. (Dinolump)
- [ ] 11.1.4. Users can register per-scenario cleanup for stateful adapters, so
  scenarios can reset automatically without every `#[given]` implementation
  remembering the reset call. Prerequisite: 11.1.3. Design Doc:
  `docs/rstest-bdd-design.md` §2.7.6.4. Finish line: an integration test shows
  cleanup running after success and failure, and the docs state the required
  registration order. (Doggylump)

### 11.2. Smooth integration ergonomics

- [ ] 11.2.1. Developers can annotate parameters with `#[harness_context]`,
  with backwards-compatible support for `#[from(rstest_bdd_harness_context)]`.
  Examples use the readable marker, and generated code keeps the reserved
  fixture key internally. Finish line: macro tests cover both marker forms and
  examples compile using `#[harness_context]`. Design Doc:
  `docs/rstest-bdd-design.md` §2.7.6.4. (Dinolump)
- [ ] 11.2.2. The public prelude exposes `StepResult`, `Slot`, `ScenarioState`,
  harness-context helpers, and marker attributes from 11.2.1, so examples can
  import one predictable module without hiding the underlying crates.
  Finish line: compile tests prove examples import only the prelude plus their
  harness crate, and docs list the exported items. Prerequisite: 11.2.1. Design
  Doc: `docs/rstest-bdd-design.md` §2.7.6.4. (Dinolump)
- [ ] 11.2.3. Diagnostics detect non-canonical harness paths and missing or
  ambiguous attribute-policy annotations, with actionable guidance such as
  adding `attributes = ...` explicitly or using the canonical path. Finish
  line: trybuild tests cover non-canonical, missing, and ambiguous policy cases
  and assert the suggested fix text. Design Doc: `docs/rstest-bdd-design.md`
  §§2.7.3-2.7.6.4. (Telefono)
- [ ] 11.2.4. The test suite demonstrates v0.6.x compatibility for mutable
  world, fallible fixture, Tokio harness, GPUI harness with shared context, GPUI
  harness with mutable context and scenario state, and scenario outline shapes.
  Finish line: CI runs and passes one compatibility test for each listed shape.
  Design Doc: `docs/rstest-bdd-design.md` §2.7.6.4. (Buzzy Bee)

## 12. Pre-1.0.0 API consolidation: v0.7.0 ambitions

The v0.7.0 line is the last planned place for migration-guide-worthy API
cleanup before v1.0.0. This phase intentionally collects changes that would be
too disruptive for v0.6.x but would make the v1 surface smaller and more
predictable.

### 12.1. Redesign state and context borrowing

- [ ] 12.1.1. `StepContext` supports guard-based interior borrowing, so callers
  can concurrently borrow distinct mutable fixtures, including mutable harness
  context and mutable world state when fixture keys differ. Previous
  `Option`-based borrow APIs are replaced with `Result`-returning APIs carrying
  `FixtureBorrowError`, with generated-wrapper regression coverage. Finish
  line: runtime unit tests prove concurrent distinct mutable borrows succeed,
  same-fixture conflicts fail, and generated-wrapper tests cover harness context
  plus world state. Design Doc: `docs/rstest-bdd-design.md` §2.7.6.5.
  (Pandalump, Telefono)
- [ ] 12.1.2. `FixtureRefMut` exposes a stable, opaque public API that preserves
  value-accessor methods while hiding internal enum and representation details.
  Public callers retain value access methods, and internal variants are no
  longer part of the public surface. Prerequisite: 12.1.1. Design Doc:
  `docs/rstest-bdd-design.md` §2.7.6.5. Finish line: public API tests compile
  against accessor methods, and no downstream test can match internal variants.
  (Telefono)
- [ ] 12.1.3. A stable world lifecycle contract guarantees before-scenario
  reset, after-scenario cleanup, and cleanup on failure or skip, so users can
  model scenario state without thread-local reset conventions. The migration
  guide explains how v0.6 workarounds map to the v0.7 lifecycle. Prerequisite:
  12.1.1. Design Doc: `docs/rstest-bdd-design.md` §2.7.6.5. Finish line:
  lifecycle tests pass for success, assertion failure, and skip, and the
  migration guide includes the v0.6-to-v0.7 mapping. (Doggylump)

### 12.2. Simplify harness and generated-test APIs

- [ ] 12.2.1. Users can annotate steps with typed harness extractors such as
  `Harness<T>` or `HarnessMut<T>`, or with a stable attribute marker, so
  ordinary harness-backed steps receive harness fixtures automatically without
  spelling `rstest_bdd_harness_context`. Requires 11.2.1 or equivalent design
  validation. Design Doc: `docs/rstest-bdd-design.md` §2.7.6.5. Finish line:
  macro tests cover `Harness<T>`, `HarnessMut<T>`, and the marker path without
  user-visible reserved-key spelling. (Dinolump, Telefono)
- [ ] 12.2.2. Harnesses can supply a factory expression or equivalent
  configuration contract to instantiate configurable harnesses, so they no
  longer require zero-sized wrapper types solely for macro instantiation.
  Finish line: compile tests instantiate a configured harness through the new
  contract and reject an invalid factory with a targeted diagnostic. Design Doc:
  `docs/rstest-bdd-design.md` §§2.7.3, 2.7.6.5. (Pandalump)
- [ ] 12.2.3. A declarative extension model lets first-party and third-party
  harness crates participate through one explicit metadata mechanism instead of
  macro-local path tables. Finish line: one first-party and one example
  third-party harness use the metadata mechanism in tests, and docs describe
  the extension contract. Design Doc: `docs/rstest-bdd-design.md`
  §§2.7.3-2.7.6.5. (Telefono)
- [ ] 12.2.4. The generated-test model gives each `#[scenario]`, `scenarios!`,
  scenario, and outline row a readable Rust test name, isolated lifecycle, and
  failure reports that no longer depend on hidden loops over unrelated
  scenarios. Finish line: integration tests assert generated names, lifecycle
  isolation, and per-row failure reporting for `#[scenario]` and `scenarios!`.
  Design Doc: `docs/rstest-bdd-design.md` §2.7.6.5. (Doggylump)
- [ ] 12.2.5. The recorded async harness trait surface gives Tokio and future
  async adapters coherent migration, multi-poll, cancellation, and runtime
  ownership semantics, whether the v1 contract remains synchronous or moves to
  an async harness trait. Finish line: an ADR records the decision, Tokio tests
  cover the selected semantics, and migration docs explain the rejected path.
  Design Doc: `docs/rstest-bdd-design.md` §§2.5, 2.7.6.5. (Buzzy Bee)
- [ ] 12.2.6. The v1 packaging model records whether first-party integrations
  are feature-gated on `rstest-bdd` or remain explicit adapter crates, and the
  choice is captured in an ADR and migration guidance. Finish line: the ADR,
  migration guide, and publish/package tests all reflect the same packaging
  model. Design Doc: `docs/rstest-bdd-design.md` §2.7.6.5. (Pandalump)
