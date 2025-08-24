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

...