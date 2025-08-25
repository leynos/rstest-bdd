# Proposed Design for `rstest-bdd`: A BDD Framework for Rust

## Part 1: Vision and User-Facing Design

This part of the report details the user-facing aspects of the proposed
`rstest-bdd` framework. It outlines the core philosophy, provides a
comprehensive usage example, and explores advanced features, focusing on
creating an ergonomic and powerful BDD experience that is idiomatic to the Rust
ecosystem.

### 1.1 Introduction: A Synergistic Approach to BDD in Rust

Behaviour-Driven Development (BDD) is a software development process that
encourages collaboration between developers, quality assurance experts, and
non-technical business participants. It achieves this by using a natural,
domain-specific language to describe an application's behaviour from the user's
perspective.[^1] The proposed

`rstest-bdd` framework is designed to bring this collaborative power to Rust by
deeply integrating BDD principles with the `rstest` testing crate.

The core philosophy of `rstest-bdd` is to fuse the human-readable,
requirement-driven specifications of Gherkin 3 with the powerful,
developer-centric features of

`rstest`.[^2] The primary value proposition is the unification of high-level
functional and acceptance tests with low-level unit tests. Both test types
coexist within the same project, use the same fixture model for dependency
injection, and are executed by the standard `cargo test` command. This approach
eliminates the need for a separate test runner, reducing CI/CD configuration
complexity and lowering the barrier to adoption for teams already invested in
the Rust testing ecosystem.[^3]

The design is heavily modeled on `pytest-bdd`, a successful plugin for Python's
`pytest` framework.[^4]

`pytest-bdd`'s success stems from its ability to leverage the full power of its
host framework—including fixtures, parameterization, and a vast plugin
ecosystem—rather than replacing it.[^1] By emulating this model,

`rstest-bdd` will provide a familiar and robust BDD experience that feels
native to Rust developers who appreciate the capabilities of `rstest`.

### 1.2 A Complete Usage Example: The "Web Search" Scenario

To illustrate the intended workflow, this section presents a complete,
narrative example of testing a web search feature. This walkthrough mirrors the
structure of typical `pytest-bdd` tutorials, demonstrating the journey from a
plain-language specification to an executable test.[^1]

#### 1.2.1 Step 1: The Feature File

The process begins with a `.feature` file written in Gherkin. This file
describes the desired functionality in a way that can be understood and
validated by non-technical stakeholders.[^1]

**File:** `tests/features/web_search.feature`

```gherkin
Feature: Web Search
  As a user, I want to search for information,
  so that I can find what I'm looking for.

  Scenario: Simple web search
    Given the DuckDuckGo home page is displayed
    When I search for "Rust programming language"
    Then the search results page is displayed
    And the results contain "Rust Programming Language"
```

#### 1.2.2 Step 2: The Step Definition File

Next, developers create a corresponding Rust test file to implement the logic
for each step defined in the Gherkin scenario. This is where the core
`rstest-bdd` macros come into play.

A key design choice, inherited from `pytest-bdd`, is that the Rust test module
is the primary entry point, not the feature file.[^5] A test function is
explicitly bound to a Gherkin scenario using the

`#[scenario]` attribute macro, which is a direct parallel to `pytest-bdd`'s
`@scenario` decorator.[^3]

State is managed and passed between steps using `rstest`'s native fixture
system, a cornerstone of this design. This contrasts with other BDD frameworks
that often rely on a monolithic `World` object.[^6] By using fixtures,

`rstest-bdd` allows for the reuse of setup and teardown logic already written
for unit tests, promoting a Don't Repeat Yourself (DRY) approach.[^1]

**File:** `tests/test_web_search.rs`

```rust
use rstest::fixture;
use rstest_bdd::{scenario, given, when, then};
// Assume 'thirtyfour' or another WebDriver crate is used for browser automation.
use thirtyfour::prelude::*;

// An rstest fixture that provides the WebDriver instance for the test.
// This is standard rstest functionality.[4, 12]
#[fixture]
async fn browser() -> WebDriverResult<WebDriver> {
    let caps = DesiredCapabilities::firefox();
    let driver = WebDriver::new("http://localhost:4444", caps).await?;
    // The fixture yields the driver to the test, and will handle cleanup after.
    Ok(driver)
}

// The #[scenario] macro binds this test function to a specific scenario.
// It will generate the necessary code to run the Gherkin steps.
// The test attribute (e.g., #[tokio::test]) would be configured via
// feature flags in Cargo.toml to support different async runtimes.
#[tokio::test]
#
async fn test_simple_search(#[future] browser: WebDriver) {
    // The body of this function runs *after* all Gherkin steps have passed.
    // It can be used for final assertions or complex cleanup.[6]
    // The example assumes the browser closes implicitly when the 'browser'
    // fixture goes out of scope.
}

// Step definitions are just decorated functions.
// The #[from(fixture_name)] attribute injects the fixture into the step.
#
async fn go_to_home(#[from(browser)] driver: &mut WebDriver) {
    driver.goto("https://duckduckgo.com/").await.unwrap();
}

// The framework will parse the quoted string and pass it as an argument.
#[when("I search for \"(.*)\"")]
async fn search_for_phrase(#[from(browser)] driver: &mut WebDriver, phrase: String) {
    let form = driver.find(By::Id("search_form_input_homepage")).await.unwrap();
    form.send_keys(&phrase).await.unwrap();
    form.submit().await.unwrap();
}

#[then("the search results page is displayed")]
async fn results_page_is_displayed(#[from(browser)] driver: &mut WebDriver) {
    let results = driver.find(By::Id("links")).await;
    assert!(results.is_ok(), "Search results container not found.");
}

#[then("the results contain \"(.*)\"")]
async fn results_contain_text(#[from(browser)] driver: &mut WebDriver, text: String) {
    let content = driver.source().await.unwrap();
    assert!(content.contains(&text), "Result text not found in page source.");
}
```

#### 1.2.3 Step 3: Running the Tests

With the feature and step definition files in place, the user simply runs the
standard Rust test command:

```bash
cargo test
```

`rstest-bdd` ensures that `test_simple_search` is executed as a regular test.
`rstest` handles the `browser` fixture setup, and the code generated by
`#[scenario]` orchestrates the execution of the `given`, `when`, and `then`
steps in the correct order. This seamless integration means all standard
`cargo` and `rstest` features, such as test filtering and parallel execution,
work out of the box.[^7]

### 1.3 Advanced Usage Patterns

Beyond the basic workflow, `rstest-bdd` is designed to support the advanced
Gherkin features necessary for comprehensive testing.

#### 1.3.1 Parameterization with `Scenario Outline`

Gherkin's `Scenario Outline` allows a single scenario to be run with multiple
sets of data from an `Examples` table.[^8]

`rstest-bdd` will map this concept directly to `rstest`'s powerful
parameterization capabilities. The `#[scenario]` macro will detect a
`Scenario Outline` and generate code equivalent to a standard `rstest`
parameterized test using multiple `#[case]` attributes.[^9]

**Feature File (**`login.feature`**):**

```gherkin
Feature: User Login

  Scenario Outline: Login with different credentials
    Given I am on the login page
    When I enter username "<username>" and password "<password>"
    Then I should see the message "<message>"

    Examples:

   | username | password | message |
   | user | correctpass | "Welcome, user!" |
   | user | wrongpass | "Invalid credentials" |
   | admin | adminpass | "Welcome, administrator!" |
```

**Step Definition (**`test_login.rs`**):**

```rust
//...
#[scenario(path = "features/login.feature", index = 0)]
#[tokio::test]
async fn test_login_scenarios(#[future] browser: WebDriver) {}

// Placeholders from the 'Examples' table are passed as typed arguments to the step functions.
#[when("I enter username \"<username>\" and password \"<password>\"")]
async fn enter_credentials(
    #[from(browser)] driver: &mut WebDriver,
    username: String,
    password: String,
) {
    //... implementation...
}

#[then("I should see the message \"<message>\"")]
async fn see_message(#[from(browser)] driver: &mut WebDriver, message: String) {
    //... assert message is visible...
}
```

#### 1.3.2 Step Argument Parsing

To provide an ergonomic and type-safe way of extracting parameters from step
strings, `rstest-bdd` will support a `format!`-like syntax. This avoids the
need for raw regular expressions in most cases and leverages Rust's existing
`FromStr` trait for "magic conversion", a core feature of `rstest`.[^2] This is
directly analogous to the `parsers` module in `pytest-bdd`.1.

**Example:**

```rust
// Step in.feature file:
// When I deposit 50 dollars

// Step definition in.rs file:
#[when("I deposit {amount:u32} dollars")]
fn deposit_amount(#[from(account)] acc: &mut Account, amount: u32) {
    acc.deposit(amount);
}
```

The framework will parse the string "50", use `u32::from_str("50")` to convert
it, and pass the resulting `u32` value to the `deposit_amount` function.

#### 1.3.3 Using `Background`, Data Tables, and Docstrings

To achieve feature parity with modern BDD tools, the framework will support
other essential Gherkin constructs.

- Background: Steps defined in a `Background` section are executed
  before each `Scenario` in a feature file.[^10] The parser prepends these
  steps to the scenario's step list so the `#[scenario]` macro runs them first.

- Data Tables: A Gherkin data table provides a way to pass a structured
  block of data to a single step. Provide it to the step function via a single
  optional parameter annotated with `#[datatable]` or named `datatable` of type
  `Vec<Vec<String>>`, mirroring `pytest-bdd`'s `datatable` argument.[^11]
  During expansion, the `#[datatable]` marker is removed, but the declared
  parameter type is preserved and must implement `TryFrom<Vec<Vec<String>>>` to
  accept the converted cells. The annotated parameter must precede any Doc
  String argument and cannot combine `#[datatable]` with `#[from]`.

  **Feature File:**

```gherkin
  Given the following users exist:

  | name  | email              |
  | Alice | alice@example.com |
  | Bob   | bob@example.com   |
```

**Step definition (`tests/steps/create_users.rs`):**

```rust
#[given("the following users exist:")]
fn create_users(
    #[from(db)] conn: &mut DbConnection,
    datatable: Vec<Vec<String>>,
) {
    let headers = &datatable[0];
    let name_idx = headers
        .iter()
        .position(|h| h == "name")
        .expect("missing 'name' column");
    let email_idx = headers
        .iter()
        .position(|h| h == "email")
        .expect("missing 'email' column");

    for row in datatable.iter().skip(1) {
        assert!(
            row.len() > name_idx && row.len() > email_idx,
            "Expected 'name' and 'email' columns",
        );
        let name = &row[name_idx];
        let email = &row[email_idx];
        conn.insert_user(name, email);
    }
}
```

- Docstrings: A Gherkin docstring allows a larger block of multi-line text
  to be passed to a step. This will be provided as a `String` argument to the
  step function, again mirroring `pytest-bdd`.[^11]

## Part 2: Architectural and API Specification

This part transitions from the user's perspective to the technical
implementation, detailing the procedural macro API, the core architectural
challenges and solutions, and the end-to-end code generation process.

### 2.1 Procedural Macro API Design

The user-facing functionality is enabled by a suite of procedural macros. Each
macro has a distinct role in the compile-time orchestration of the BDD tests.

- `#[scenario("…")]` or `#[scenario(path = "…", index = N)]` – the primary
  entry point and orchestrator.

  - Arguments:

    - `path: &str`: A mandatory, relative path from the crate root to the
      `.feature` file containing the scenario. The path can be provided as a
      bare string literal or with the explicit `path =` form when other
      arguments are used.

    - `index: usize` (optional): Selects which scenario in the feature file to
      execute. Defaults to `0` when omitted.

  - Functionality: This macro is responsible for the heavy lifting. At
    compile time, it reads and parses the specified feature file, finds the
    matching scenario, and generates a complete, new test function annotated
    with `#[rstest]`. This generated function contains the runtime logic to
    execute the Gherkin steps.

- `#[given("...")]`, `#[when("...")]`, `#[then("...")]` - these macros attach to
  the step implementation functions.

  - Argument: A string literal representing the Gherkin step text. This
    string acts as a pattern and can include placeholders for argument parsing
    (e.g., "A user has {count:usize} cucumbers").

  - Functionality: These macros have a single, critical purpose: to
    register the decorated function and its associated metadata (the pattern
    string, keyword, and source location) into a global, discoverable registry.
    They do not generate any executable code on their own.

  The initial implementation delegates registration to the runtime crate's
  `step!` helper. Each macro expands to the original function followed by a
  call to `rstest_bdd::step!`, which internally uses `inventory::submit!` to
  add a `Step` to the registry.

- Data Tables: Step functions may include a single optional parameter
  declared in one of two ways: (a) annotated with `#[datatable]` and of any
  type `T` where `T: TryFrom<Vec<Vec<String>>>`, or (b) named `datatable` with
  concrete type `Vec<Vec<String>>`. When a feature step includes a data table,
  the wrapper converts the cells to `Vec<Vec<String>>` and, for (a), performs a
  `try_into()` to the declared type. The `#[datatable]` marker is removed
  during expansion. The data table parameter must precede any Doc String
  argument, must not be combined with `#[from]`, and the wrapper emits a
  runtime error if the table is missing.

- Docstrings: A multi-line text block immediately following a step is
  exposed to the step function through an optional `docstring` parameter of
  type `String`. The runner passes the raw block to the wrapper as
  `Option<&str>`, and the wrapper clones it into an owned `String` before
  calling the step function. As with data tables, the parameter must use this
  exact name and concrete type for detection. The wrapper fails at runtime if
  the docstring is absent. A data table must precede any docstring parameter,
  and feature files may delimit the block using either triple double-quotes or
  triple backticks.

### 2.2 The Core Architectural Challenge: Stateless Step Discovery

The most significant technical hurdle in this design is the inherent nature of
Rust's procedural macros. Each macro invocation is executed by the compiler in
an isolated, stateless environment.20. This means that when the

`#[scenario]` macro is expanding, it has no direct way to discover the
functions that have been decorated with `#[given]`, `#[when]`, or `#[then]`. It
cannot scan the project's source code, reflect on other modules, or access a
shared compile-time state to build a map of available steps.22. This stands in
stark contrast to

`pytest`, which provides a rich runtime plugin system that `pytest-bdd` hooks
into to discover tests and steps dynamically during a collection phase.[^7]

This fundamental constraint of the Rust compiler forces a specific
architectural choice. Several potential solutions exist, but only one aligns
with the project's core goals:

1. **Custom Test Runner:** The framework could provide its own test runner
   binary, similar to the `cucumber` crate which requires a `main` function to
   invoke `World::run(...)`.[^6] This runner would be responsible for
   discovering feature files and step definitions. However, this approach would
   completely bypass

   `rstest` and `cargo test`, violating the primary design goal of seamless
   integration. It would effectively be a reimplementation of `cucumber-rs`,
   not `rstest-bdd`.
2. `build.rs` **Code Generation:** A build script (`build.rs`) could be used to
   parse all `.rs` files in the `tests` directory before the main compilation.
   It could find all the step-definition attributes and generate a single,
   monolithic `steps.rs` file containing a registry of all steps. The
   `#[scenario]` macro could then `include!` this generated file. This approach
   is technically feasible but suffers from major drawbacks: it is complex to
   implement robustly, significantly slows down compilation, is notoriously
   brittle, and often provides a poor experience with IDE tools like
   `rust-analyzer` which may not be aware of the generated code.
3. **Link-Time Collection:** The ideal solution is a mechanism that allows each
   step-definition macro to emit metadata independently, with this metadata
   being collected into a single registry *after* all macros have run. This can
   be achieved by placing the metadata in a specific linker section of the
   compiled object file. At runtime, the application can read this linker
   section to discover all the registered items.

The third option, link-time collection, is the only one that satisfies all
design constraints. It preserves the standard `cargo test` workflow, avoids the
fragility of build scripts, and allows for fully decoupled step definitions.
This leads directly to the selection of the `inventory` crate as the
architectural cornerstone.

### 2.3 The `inventory` Solution: A Global Step Registry

The `inventory` crate provides a clean and powerful abstraction over the
link-time collection mechanism described above. It offers "typed distributed
plugin registration," allowing different parts of a program to submit items
into a collection that can be iterated over at runtime.[^12]

For `rstest-bdd`, this pattern is used to create a global registry of all step
definitions. First, a struct is defined to hold the metadata for each step.
This struct will contain a type-erased function pointer to the user's
implementation, the pattern string to match against Gherkin text, and source
location information for generating clear error messages.

**Definition of the** `Step` **struct (within the** `rstest-bdd` **crate):**

```rust
// A simplified representation of the step metadata.
#[derive(Debug)]
pub struct Step {
    pub keyword: StepKeyword, // e.g., Given, When or Then
    pub pattern: &'static StepPattern, // The pattern string from the attribute, e.g., "A user has {count} cucumbers"
    // A type-erased function pointer. Arguments will be wired up by the
    // scenario orchestrator in later phases.
    pub run: fn(),
    // Location info for better error messages.
    pub file: &'static str,
    pub line: u32,
}

// This macro call creates the global collection for 'Step' structs.
inventory::collect!(Step);
```

The [`StepKeyword`](../crates/rstest-bdd/src/types.rs) enum implements
`FromStr`. Parsing failures return a `StepKeywordParseError` to ensure invalid
step keywords are surfaced early.

The [`StepPattern`](../crates/rstest-bdd/src/pattern.rs) wrapper encapsulates
the pattern text so that step lookups cannot accidentally mix arbitrary strings
with registered patterns. Each pattern is compiled into a regular expression
when the step registry is initialised, surfacing invalid syntax immediately.
Equality and hashing rely solely on the pattern text. Transient fields like the
cached `Regex` are ignored to preserve identity-by-source-text semantics. The
global registry stores `(StepKeyword, &'static StepPattern)` keys in a
`hashbrown::HashMap` and uses the raw-entry API for constant-time lookups by
hashing the pattern text directly.

Duplicate step definitions are rejected when the registry is built. Attempting
to register the same keyword and pattern combination twice results in a panic
that points to the conflicting definition so that errors surface early during
test startup.

Placing the `Step` struct in the runtime crate avoids a circular dependency
between the procedural macros and the library. The macros will simply re-export
the type when they begin submitting steps to the registry.

A small convenience macro, `step!`, wraps `inventory::submit!` and directly
constructs a `Step`. It captures the file and line number automatically so that
users only provide the keyword, pattern and handler when registering a step.

The `#[given]`, `#[when]`, and `#[then]` macros will expand into an
`inventory::submit!` block. This macro call constructs an instance of the
`Step` struct at compile time and registers it for collection.[^12]

At runtime, the code generated by the `#[scenario]` macro can retrieve a
complete list of all step definitions across the entire application simply by
calling `inventory::iter::<Step>()`. This provides an iterator over all
registered `Step` instances, regardless of the file, module, or crate in which
they were defined.

The relationships among the core step types are shown below:

```mermaid
classDiagram
    class Step {
        + keyword: StepKeyword
        + pattern: &'static StepPattern
        + run: StepFn
    }
    class StepKeyword
    class StepFn
    class StepContext
    Step --> StepPattern : pattern
    Step --> StepKeyword : keyword
    Step --> StepFn : run
    class STEP_MAP {
        + (StepKeyword, &'static StepPattern) => StepFn
    }
    StepPattern : +as_str(&self) -> &'static str
    STEP_MAP --> StepFn : maps to
    StepContext --> Step : uses
```

```mermaid
classDiagram
    class StepContext {
    }
    class StepArg {
        pat: Ident
        ty: Type
    }
    class FixtureArg {
        pat: Ident
        name: Ident
        ty: Type
    }
    class Step {
        keyword: StepKeyword
        pattern: StepPattern
        run: StepFn
    }
    class StepFn
    class StepWrapper
    StepWrapper : extract_placeholders(pattern, text)
    StepWrapper : parse captures with FromStr
    StepWrapper : call StepFunction
    StepFn <|-- StepWrapper
    StepContext <.. StepWrapper
    StepArg <.. StepWrapper
    FixtureArg <.. StepWrapper
    Step o-- StepFn
```

### 2.4 The Macro Expansion Process: A Compile-Time to Runtime Journey

The interaction between the user's code, the `rstest-bdd` macros, and the final
test execution can be broken down into a sequence of compile-time and runtime
events.

**1.** `#[given]` **Expansion (Compile-Time)**

- **Input Code:**

```rust

#[given("I am a user")]
fn given_i_am_a_user(mut user_context: UserContext) { /\*... \*/ }
```

- **Macro Action:** The `#[given]` proc-macro parses its attribute string
  (`"I am a user"`) and the function it's attached to. It then generates an
  `inventory::submit!` block. This block contains the static definition of a
  `Step` struct, where the `run` field is a type-erased pointer to a wrapper
  around the `given_i_am_a_user` function.

**2.** `#[scenario]` **Expansion (Compile-Time)**

- **Input Code:**

```rust

#

fn test_my_scenario(my_fixture: MyFixture) { /\* final assertion \*/ }
```

- **Macro Action:**

1. The `#[scenario]` proc-macro performs file I/O to read the contents of
   `f.feature`.
2. It uses a Gherkin parser crate (such as `gherkin` 26) to parse the feature
   file content into an Abstract Syntax Tree (AST).
3. It traverses the AST to find the `Scenario` with the name "My Scenario".
4. It iterates through the global step registry (`inventory::iter`) *at compile
   time* to check if a matching step exists for every Gherkin step. If a step
   is missing, it emits a `compile_error!` with a helpful message, failing the
   build early.
5. Using the `quote!` macro 28, it generates a completely new Rust function.
   This generated function replaces the original

   `test_my_scenario` function.
6. The generated function is annotated with `#[rstest]`, and it preserves the
   original function's signature, including the `my_fixture: MyFixture`
   argument. This is critical for ensuring `rstest`'s dependency injection
   continues to work.
7. The body of this new, generated function contains the runtime logic for the
   BDD test:

   - It initializes a context or state object for the scenario.

   - It iterates through the steps of "My Scenario" as defined in the Gherkin
     AST.

   - For each Gherkin step, it iterates through the global step registry again
     (this time at runtime) by calling `inventory::iter::<Step>()`.

   - It finds the correct registered `Step` by matching the Gherkin step's text
     against the `pattern` field of each registered `Step`.

   - If a match is found, it parses any arguments from the Gherkin text.

   - It invokes the `run` function pointer from the matched `Step` struct,
     passing it the necessary context (which includes access to fixtures and
     step arguments).

   - After the step-execution loop, it includes the user's original code from
     the body of `test_my_scenario`.

### 3. Test Execution (Runtime)

1. The user runs `cargo test`.
2. The `rstest` test runner discovers the generated `test_my_scenario` function.
3. `rstest` first resolves and provides the `my_fixture` dependency.
4. `rstest` then executes the body of the generated function.
5. The generated code looks up each step using a map built from the global step
   registry. This map is initialized once via `LazyLock`, avoiding repeated
   iteration over `inventory::iter`. Fixtures like `my_fixture` are made
   available to the step functions through the context object passed to the
   call site.
6. If all steps pass, the original code from the user's `test_my_scenario`
   function body is executed.

This architecture successfully bridges the gap between the stateless
compile-time world of procedural macros and the stateful, ordered execution
required for a BDD scenario, all while remaining fully compatible with the
`rstest` framework.

## Part 3: Implementation and Strategic Analysis

This final part outlines a practical implementation strategy for `rstest-bdd`
and provides a critical analysis of the proposed design, including its
strengths, weaknesses, limitations, and a comparison to the existing `cucumber`
crate.

### 3.1 Phased Implementation Strategy

A phased approach is recommended to manage complexity and deliver value
incrementally.

- **Phase 1: Core Mechanics & Proof of Concept**

- Establish the two-crate workspace: `rstest-bdd` (the runtime library) and
  `rstest-bdd-macros` (the proc-macro implementation).

- Implement the `inventory`-based step registry. Define the `Step` struct and
  the `#[given]`, `#[when]`, and `#[then]` macros to populate the registry
  using `inventory::submit!`.

- Implement a basic `#[scenario]` macro. This includes compile-time Gherkin
  file parsing and a lookup map built at runtime from the step registry.
  Initially, this map supports exact string matching with no argument parsing.
  only exact string matching with no argument parsing.

- The goal of this phase is to validate the core architectural choice: that a
  `#[scenario]` macro can successfully find and execute steps registered by
  other macros at runtime.

- **Phase 2: Fixtures and Parameterization**

- Enhance the macro system to inspect the signatures of step functions and
  integrate with `rstest`'s fixture system. This allows steps to request
  fixtures directly.

- Implement support for `Scenario Outline`. The `#[scenario]` macro detects this
  Gherkin construct and generates the corresponding `#[rstest]` `#[case(…)]`
  attributes on the test function. This behaviour is now implemented and
  verified by the test suite.
- Introduced lightweight `ExampleTable` and `ScenarioData` structs in the
  macros crate. They encapsulate outline table rows and scenario metadata,
  replacing a complex tuple return and enabling clearer helper functions.
- Improved diagnostics when a `Scenario Outline` column does not match a test
  parameter. The macro lists available parameters, so mismatches can be
  resolved quickly.
- Errors for missing outline parameters use `syn::Error::new_spanned` for more
  precise diagnostics.

- Introduce the `{name:Type}` step argument parser, leveraging the `FromStr`
  trait for type conversion.

- **Phase 3: Advanced Gherkin Features & Ergonomics**

- Add support for Data Tables and Docstrings, passing them as special
  arguments to step functions.

- Implement robust compile-time error handling. The `#[scenario]` macro should
  emit clear compiler errors if a feature file cannot be parsed or if no
  matching step definition can be found for a Gherkin step. The macro now
  validates that the referenced feature file exists before invoking the Gherkin
  parser. Missing or malformed files cause `compile_error!` to be emitted,
  failing fast during compilation.

- Develop a `scenarios!` helper macro, analogous to the one in `pytest-bdd` 9,
  which can automatically bind all scenarios within one or more feature files,
  reducing boilerplate for the user.

### 3.2 Strengths and Weaknesses of the Proposed Architecture

The proposed design has a distinct set of advantages and disadvantages rooted
in its tight integration with the Rust compiler and `rstest`.

#### 3.2.1 Advantages

- **Seamless Ecosystem Integration:** The framework uses `cargo test` as its
  runner and `rstest` as its foundation. This means it works out-of-the-box
  with the entire Rust ecosystem, including CI/CD pipelines, code coverage
  tools, and test filtering mechanisms, without requiring a separate runner or
  special configuration.[^3]

- **Powerful Fixture Reuse:** By leveraging `rstest` fixtures for state
  management, developers can reuse existing setup/teardown logic from their
  unit tests for BDD scenarios. This promotes code reuse and consistency across
  the entire test suite.[^1]

- **Compile-Time Safety:** By validating that every Gherkin step has a
  corresponding implementation at compile time, the framework can fail fast
  with a clear `compile_error!`, preventing difficult-to-debug runtime panics
  from missing steps.

- **High Performance:** The test code is fully compiled Rust. While there is a
  small runtime overhead for matching Gherkin steps to functions, the core
  logic executes at native speed.

- **Decoupled and Reusable Steps:** Thanks to the `inventory`-based discovery,
  step definitions can be placed in any module (e.g., a central
  `tests/steps/common.rs` file, akin to `conftest.py` 9) and will be
  automatically available to any scenario, promoting modular and maintainable
  test code.

#### 3.2.2 Disadvantages

- **High Macro Complexity:** The implementation of the `#[scenario]` macro is
  non-trivial. It involves compile-time file I/O, parsing, and extensive code
  generation via `quote!`. Debugging and maintaining this macro will be a
  significant challenge.[^13]

- **Reliance on "Magic" and Portability:** The `inventory` crate's use of
  linker sections is powerful but potentially "magic." It abstracts away
  complex system-level behaviour that may be a conceptual hurdle for some users
  and has platform-specific considerations that may not be suitable for all
  targets, such as some embedded systems or Windows/PE environments.[^12]

**Mitigation:** For niche targets, a `no-inventory` feature flag could be
provided. This would trigger a fallback mechanism using a `build.rs` script to
scan for step definitions and generate a central registry file (e.g.,
`OUT_DIR/steps.rs`), which is then included via `include!` by the `#[scenario]`
macro.

- **Compile-Time Overhead:** The `#[scenario]` macro performs file I/O and
  parsing during compilation. For projects with many feature files, this could
  introduce a noticeable overhead to compile times. **Mitigation:** This can be
  significantly optimized by caching the parsed Gherkin ASTs in the `OUT_DIR`.
  The macro would only re-parse a `.feature` file if its modification time has
  changed, similar to how tools like `prost-build` handle `.proto` files.

- **Runtime Step Matching:** The connection between a Gherkin step and its
  implementing function is resolved at the beginning of each scenario's
  execution. This is a deliberate trade-off that enables decoupled steps, but
  it carries a minor performance cost compared to a fully pre-compiled approach
  where function calls are resolved at compile time.

### 3.3 Framework Limitations

The design choices lead to several inherent limitations that users should be
aware of.

- **Static Gherkin Files:** Feature files are dependencies that are read and
  processed at compile time. The framework cannot load or select `.feature`
  files dynamically at runtime.

- **Static Step Definitions:** All step definitions must be known at compile
  time so they can be registered. It is not possible to dynamically generate or
  register new step definitions at runtime.

- **IDE Support Challenges:** While test execution via `cargo test` will
  integrate perfectly with IDEs, more advanced features like "Go to Definition"
  (navigating from a Gherkin step in a `.feature` file directly to the
  implementing Rust function) will not work out-of-the-box. **Mitigation:**
  This functionality would require a dedicated IDE extension. Potential
  solutions include shipping a `rust-analyzer` proc-macro server stub that can
  surface the pattern-to-function mapping, or publishing a dedicated VS Code
  extension that generates virtual documents to bridge the gap between
  `.feature` files and Rust code.

### 3.4 Comparative Analysis: `rstest-bdd` vs. `cucumber`

The primary existing BDD framework in the Rust ecosystem is `cucumber`. A
comparison highlights the fundamental philosophical differences between the two
approaches.

The core distinction lies in their integration philosophy. `cucumber-rs`
provides a *Cucumber implementation in Rust*. It brings the established,
cross-language Cucumber ecosystem's concepts—such as a dedicated test runner, a
mandatory `World` state object, and built-in concurrency management—into a Rust
project.[^6] It aims for consistency with

`cucumber-jvm`, `cucumber-js`, etc.

In contrast, the proposed `rstest-bdd` provides a *BDD layer for the native
Rust testing ecosystem*. It adapts BDD principles to be idiomatic within the
existing paradigms of `cargo test` and `rstest`.[^14] This leads to a different
developer experience. A

`cucumber-rs` user asks, "How do I manage state in my `World` struct?" A
`rstest-bdd` user asks, "Which `rstest` fixture should I inject to provide this
state?" This makes `rstest-bdd` a potentially more natural fit for teams
already heavily invested in `rstest`, as they can leverage their existing
knowledge and fixtures directly. `cucumber-rs` is better suited for teams
seeking strict adherence to the global Cucumber standard or those who prefer a
hard separation between their BDD acceptance tests and their other
unit/integration tests.

The following table summarizes the key differences:

| Feature          | rstest-bdd (Proposed)                                                                                                        | cucumber                                                                       |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Test Runner      | Standard cargo test (via rstest expansion)                                                                                   | Custom runner invoked from a main function (World::run(…)) 23                  |
| State Management | rstest fixtures; dependency injection model 1                                                                                | Mandatory World struct; a central state object per scenario 11                 |
| Step Discovery   | Automatic via compile-time registration (inventory) and runtime matching                                                     | Explicit collection in the test runner setup (World::cucumber().steps(…)) 37   |
| Parameterization | Gherkin Scenario Outline maps to rstest's #[case] parameterization 15                                                        | Handled internally by the cucumber runner                                      |
| Async Support    | Runtime-agnostic via feature flags (e.g., tokio, async-std) which emit the appropriate test attribute (#[tokio::test], etc.) | Built-in; requires specifying an async runtime 11                              |
| Ecosystem        | Seamless integration with rstest and cargo features                                                                          | Self-contained framework; can use any Rust library within steps                |
| Ergonomics       | pytest-bdd-like; explicit #[scenario] binding links test code to features 6                                                  | cucumber-jvm/js-like; feature-driven, with a central test runner               |
| Core Philosophy  | BDD as an extension of the existing rstest framework                                                                         | A native Rust implementation of the Cucumber framework standard                |

### 3.5 Potential Extensions

Based on the successful patterns of `pytest-bdd` and the needs of a growing
Rust testing ecosystem, several extensions could be considered after the core
functionality is implemented:

- `scenarios!` **Macro:** Implemented to reduce boilerplate. The macro walks a
  directory recursively using the `walkdir` crate, discovers `.feature` files,
  and generates a module containing a test for each `Scenario`. Function names
  derive from the feature file stem and scenario title, sanitised and
  deduplicated. Generated tests do not currently accept fixtures.

  The following diagram summarizes the relationships between the macro and its
  helper modules:

  ```mermaid
  classDiagram
      class scenarios {
          +scenarios(input: TokenStream) TokenStream
      }
      class ScenarioConfig {
          +attrs: &Vec<syn::Attribute>
          +vis: &syn::Visibility
          +sig: &syn::Signature
          +block: &syn::Block
          +feature_path: String
          +scenario_name: String
          +steps: Vec<Step>
          +examples: Vec<Example>
      }
      class scenario {
          +generate_scenario_code(config: ScenarioConfig, iter: Iterator) proc_macro::TokenStream
      }
      class feature {
          +extract_scenario_steps(feature, idx: Option<usize>) -> Result<Data, Error>
          +parse_and_load_feature(path: &Path) -> Result<Feature, Error>
      }
      class errors {
          +error_to_tokens(err: &syn::Error) -> TokenStream
      }
      scenarios --> scenario : uses
      scenarios --> feature : uses
      scenarios --> errors : uses
      scenario <.. ScenarioConfig : uses
      feature <.. Step
      ScenarioConfig <.. Step
      ScenarioConfig <.. Example
  ```

  The following sequence diagram captures macro expansion and test execution:

  ```mermaid
  sequenceDiagram
      actor Dev as Developer
      participant RustC as Rust Compiler
      participant Macro as scenarios! (proc-macro)
      participant FS as Filesystem
      participant Parser as feature parser
      participant Gen as scenario::generate_scenario_code
      participant TestRunner as Test Runner

      Dev->>RustC: cargo test (compile)
      RustC->>Macro: expand scenarios!("path")
      Macro->>FS: list *.feature recursively
      loop per feature file
          Macro->>Parser: parse_and_load_feature(path)
          Parser-->>Macro: Feature with Scenarios
          loop per scenario
              Macro->>Parser: extract_scenario_steps(feature, idx)
              Macro->>Gen: generate_scenario_code(config)
              Gen-->>Macro: test item tokens
          end
      end
      Macro-->>RustC: emit module with generated tests
      RustC->>Dev: build complete
      Dev->>TestRunner: run tests
      TestRunner->>GeneratedTests: execute steps -> step functions
  ```

- **Diagnostic CLI:** A small helper utility, perhaps integrated as a cargo
  subcommand (`cargo bdd`), could provide diagnostic information. For example,
  `cargo bdd list-steps` could dump the entire registered step registry,
  helping developers find available steps and detect unused or duplicate
  definitions.

- **Teardown Hooks:** While `rstest` fixtures handle teardown via `Drop`, more
  explicit post-scenario cleanup, especially in the case of a step panic, could
  be valuable. A feature like `#[fixture(after)]` could be explored, either
  within `rstest-bdd` or as a proposal to `rstest` itself, to attach teardown
  logic that is guaranteed to run after a scenario completes, regardless of its
  outcome.

In conclusion, `rstest-bdd` is designed not to replace `cucumber` but to offer
a compelling alternative for a different audience: developers who prioritize
deep integration with Rust's native testing tools and want to unify their BDD
and unit testing workflows under the powerful `rstest` umbrella.

### 3.6 Workspace Layout Decisions

The project uses a Cargo workspace to keep the runtime and procedural macro
crates separate. The workspace contains two members:

- `rstest-bdd` — the runtime library.
- `rstest-bdd-macros` — the crate providing attribute macros.

This layout allows each crate to evolve independently while sharing common
configuration and lints at the workspace level.

### 3.7 Initial Scenario Macro Implementation

The first implementation of the `#[scenario]` macro kept the scope narrow to
validate the overall approach. It accepted only a `path` argument pointing to a
`*.feature` file and always executed the first `Scenario` found. The macro now
also accepts an optional `index` argument. When provided, the macro selects the
scenario at that zero-based position. If omitted, it defaults to `0`, matching
the behaviour of the earlier version. The `path` argument may be provided as a
bare string literal for convenience (e.g.
`#[scenario("tests/example.feature")]`) or using the explicit `path =` form
when combined with `index`. The generated test is annotated with `#[rstest]`
and at runtime iterates over the selected scenario's steps, finding matching
step definitions by exact string comparison. Argument parsing and fixture
handling remain unimplemented to minimize complexity while proving the
orchestration works.

### 3.8 Fixture Integration Implementation

The second phase extends the macro system to support fixtures. Step definition
macros now inspect the parameters of the attached function. Any argument is
treated as a fixture request, with an optional `#[from(name)]` attribute
allowing the argument name to differ from the fixture's. The macro generates a
wrapper function taking a `StepContext` and registers this wrapper in the step
registry. The wrapper retrieves the required fixtures from the context and
calls the original step function.

The `#[scenario]` macro populates a `StepContext` at runtime. It gathers all
fixtures provided to the generated test function and inserts references into
the context before executing each step via the registered wrapper. This
preserves `rstest`'s fixture injection semantics while enabling steps to share
state.

```mermaid
sequenceDiagram
    participant TestFunction
    participant ScenarioMacro
    participant StepContext
    participant StepWrapper
    TestFunction->>ScenarioMacro: Call generated test (with fixtures)
    ScenarioMacro->>StepContext: Insert fixture references
    loop For each step
        ScenarioMacro->>StepWrapper: Call step wrapper with StepContext
        StepWrapper->>StepContext: Retrieve fixtures by name/type
        StepWrapper->>StepFunction: Call original step function with fixtures
    end
```

Every wrapper function is given a unique symbol name derived from the source
function and an atomic counter. This avoids collisions when similarly named
steps appear in different modules. The macro also emits a compile-time array
length assertion to ensure the generated fixture list matches the wrapper
signature. Any mismatch is reported during compilation rather than at runtime.

### 3.9 Step-Argument Parsing Implementation

The third phase introduces typed placeholders to step patterns. The runtime
library exposes an `extract_placeholders` helper that converts a pattern with
`{name:Type}` segments into a regular expression and returns the captured
strings or a `PlaceholderError` detailing why extraction failed. This error
covers pattern mismatches as well as invalid or uncompiled step patterns.

PlaceholderError: API shape and examples

- Purpose: human‑readable diagnostics surfaced to callers and test failures.
- Stability: message text is intended for human display, not machine parsing.
  Programmes should branch on the enum variant rather than parsing strings.
- Shape: a Rust enum with the following variants and display formats:

```rust
enum PlaceholderError {
  // Display: "pattern mismatch"
  PatternMismatch,

  // Display: "invalid placeholder syntax: <reason>"
  InvalidPlaceholder(String),

  // Display: "invalid step pattern: <regex_error>"
  InvalidPattern(String),

  // Display: "uncompiled step pattern"
  Uncompiled,
}
```

- Fields and metadata:
  - PatternMismatch: no fields; indicates the text did not satisfy the
    pattern. There is no separate “missing capture” error; a missing or extra
    capture manifests as a mismatch because the entire text must match the
    compiled regular expression for the pattern.
  - InvalidPlaceholder(String): the pattern contained malformed placeholder
    syntax and could not be parsed. No additional metadata is captured.
  - InvalidPattern(String): carries the underlying `regex::Error` string coming
    from the regular expression engine during compilation of the pattern. No
    additional metadata (placeholder name, position, or line info) is captured.
  - Uncompiled: no fields; indicates the step pattern was queried before being
    compiled. This is a guard and should not occur in normal usage because
    patterns are compiled during step registration.

- Example error strings (exact `Display` output):
  - Pattern mismatch: `"pattern mismatch"`
  - Invalid placeholder: `"invalid placeholder syntax: reason"`
  - Invalid pattern: `"invalid step pattern: regex parse error: error message"`
  - Uncompiled: `"uncompiled step pattern"`

- Example JSON mapping (for consumers that serialise errors). Note: this is not
  emitted by the library; it is a suggested shape if you need to map the enum
  to JSON at an API boundary:

```json
// Pattern mismatch
{"code":"pattern_mismatch","message":"pattern mismatch"}

// Invalid placeholder
{"code":"invalid_placeholder","message":"invalid placeholder syntax: reason"}

// Invalid pattern
{"code":"invalid_pattern","message":"invalid step pattern: <regex_error>"}

// Uncompiled pattern
{"code":"uncompiled","message":"uncompiled step pattern"}
```

Step wrapper functions parse the returned strings and convert them with
`FromStr` before calling the original step. Scenario execution now searches the
step registry using `find_step`, which falls back to placeholder matching when
no exact pattern is present. This approach keeps the macros lightweight while
supporting type‑safe parameters in steps. The parser handles escaped braces,
nested brace pairs, and treats other backslash escapes literally, preventing
greedy captures while still requiring well‑formed placeholders.

The runner forwards the raw doc string as `Option<&str>` and the wrapper
converts it into an owned `String` before invoking the step function. The
sequence below summarizes how the runner locates and executes steps when
placeholders are present:

```mermaid
sequenceDiagram
    participant ScenarioRunner
    participant StepRegistry
    participant StepWrapper
    participant StepFunction

    ScenarioRunner->>StepRegistry: find_step(keyword, text)
    alt exact match
        StepRegistry-->>ScenarioRunner: StepFn
    else placeholder match
        StepRegistry->>StepRegistry: extract_placeholders(pattern, text)
        StepRegistry-->>ScenarioRunner: StepFn
    end
    ScenarioRunner->>StepWrapper: call StepFn(ctx, text, docstring: Option<&str>, table: Option<&[&[&str]]>)
    StepWrapper->>StepWrapper: extract_placeholders(pattern, text)
    StepWrapper->>StepWrapper: parse captures with FromStr
    StepWrapper->>StepFunction: call with typed args (docstring: String, datatable: Vec<Vec<String>>)
    StepFunction-->>StepWrapper: returns
    StepWrapper-->>ScenarioRunner: returns
```

### 3.10 Runtime Module Layout (for Contributors)

To keep responsibilities cohesive the runtime is split into focused modules.
Public APIs are re‑exported from `lib.rs` so consumers continue to import from
`rstest_bdd::*` as before.

- `types.rs`: Core types and errors.
  - `PatternStr`, `StepText`: light wrappers for pattern keys and step text.
  - `StepKeyword` (+ `FromStr`), `StepKeywordParseError`.
  - `PlaceholderError`: semantic error enum returned by parsing helpers.
  - `StepFn`: type alias for the step function pointer.
- `pattern.rs`: Step pattern wrapper.
  - `StepPattern::new`, `compile`, `regex` (plus `try_regex` for internal use).
- `placeholder.rs`: Placeholder extraction and scanner.
  - `extract_placeholders` (public) and the single‑pass scanner
    `build_regex_from_pattern` with small parsing predicates and helpers.
- `context.rs`: Fixture context.
  - `StepContext`: simple type‑indexed store used to pass fixtures into steps.
- `registry.rs`: Registration and lookup.
  - `Step` record, `step!` macro, global registry map, `lookup_step`,
    `find_step`.
- `lib.rs`: Public API facade.
  - Re‑exports public items and keeps the `greet()` example function.

All modules use en‑GB spelling and include `//!` module‑level documentation.

## **Works cited**

[^1]: A Complete Guide To Behavior-Driven Testing With Pytest BDD, accessed on
    July 20, 2025, <https://pytest-with-eric.com/bdd/pytest-bdd/>
[^2]: rstest - [crates.io](http://crates.io): Rust Package Registry, accessed on
    July 20, 2025, <https://crates.io/crates/rstest/0.12.0>
[^3]: Pytest-BDD: the BDD framework for pytest — pytest-bdd 8.1.0 documentation,
    accessed on July 20, 2025, <https://pytest-bdd.readthedocs.io/>
[^4]: Behavior-Driven Python with pytest-bdd - Test Automation University -
    Applitools, accessed on July 20, 2025,
    <https://testautomationu.applitools.com/behaviour-driven-python-with-pytest-bdd/>
[^5]: Python Testing 101: pytest-bdd - Automation Panda, accessed on July 20,
    2025,
    <https://automationpanda.com/2018/10/22/python-testing-101-pytest-bdd/>
[^6]: Introduction - Cucumber Rust Book, accessed on July 20, 2025,
    <https://cucumber-rs.github.io/cucumber/main/>
[^7]: Behavior-Driven Development: Python with Pytest BDD -
    [Testomat.io](http://Testomat.io), accessed on July 20, 2025,
    <https://testomat.io/blog/pytest-bdd/>
[^8]: Scenario Outline in PyTest – BDD - QA Automation Expert, accessed on July
    20, 2025,
    <https://qaautomation.expert/2024/04/11/scenario-outline-in-pytest-bdd/>
    <https://testautomationu.applitools.com/behaviour-driven-python-with-pytest-bdd/chapter5.html>
[^9]: How can I create parameterized tests in Rust? - Stack Overflow, accessed
       on July 20, 2025,
       <https://stackoverflow.com/questions/34662713/how-can-i-create-parameterized-tests-in-rust>
[^10]: pytest-bdd - Read the Docs, accessed on July 20, 2025,
    <https://readthedocs.org/projects/pytest-bdd/downloads/pdf/latest/>
[^11]: pytest-bdd - PyPI, accessed on July 20, 2025,
    <https://pypi.org/project/pytest-bdd/>
    <https://www.reddit.com/r/rust/comments/1hwx3tn/what_is_a_good_pattern_to_share_state_between/>
     GitHub, accessed on July 20, 2025,
    <https://github.com/rust-lang/rust/issues/44034>
[^12]: inventory - Rust - [Docs.rs](http://Docs.rs), accessed on July 20, 2025,
    <https://docs.rs/inventory>
    <https://www.luizdeaguiar.com.br/2022/08/shared-steps-and-hooks-with-pytest-bdd/>
[^13]: Guide to Rust procedural macros |
    [developerlife.com](http://developerlife.com), accessed on July 20, 2025,
    <https://developerlife.com/2022/03/30/rust-proc-macro/>
    <https://medium.com/@alfred.weirich/the-rust-macro-system-part-1-an-introduction-to-attribute-macros-73c963fd63ea>
     <https://github.com/cucumber-rs/cucumber>
    <https://www.florianreinhard.de/cucumber-in-rust-beginners-tutorial/>
[^14]: la10736/rstest: Fixture-based test framework for Rust - GitHub, accessed
       on July 20, 2025, <https://github.com/la10736/rstest>
