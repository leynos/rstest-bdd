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

The design is heavily modelled on `pytest-bdd`, a successful plugin for
Python's `pytest` framework.[^4]

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
  In order to find information, a user performs a web search.

  Scenario: Simple web search
    Given the DuckDuckGo home page is displayed
    When a user searches for "Rust programming language"
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
    // The fixture yields the browser to the test, and will handle cleanup after.
    Ok(WebDriver::new("http://localhost:4444", caps).await?)
}

// The #[scenario] macro binds this test function to a specific scenario.
// It will generate the necessary code to run the Gherkin steps.
// The test attribute (e.g., #[tokio::test]) would be configured via
// feature flags in Cargo.toml to support different async runtimes.
#[tokio::test]
async fn test_simple_search(#[future] browser: WebDriver) {
    // The body of this function runs *after* all Gherkin steps have passed.
    // It can be used for final assertions or complex cleanup.[6]
    // The example assumes the browser closes implicitly when the 'browser'
    // fixture goes out of scope.
}

// Step definitions are just decorated functions.
// The fixture is injected when the parameter name matches the fixture.
#[given("the DuckDuckGo home page is displayed")]
async fn go_to_home(browser: &mut WebDriver) -> WebDriverResult<()> {
    browser.goto("https://duckduckgo.com/").await?;
    Ok(())
}

// The framework will parse the quoted string and pass it as an argument.
#[when("I search for \"{phrase}\"")]
async fn search_for_phrase(browser: &mut WebDriver, phrase: String) -> WebDriverResult<()> {
    let form = browser.find(By::Id("search_form_input_homepage")).await?;
    form.send_keys(&phrase).await?;
    form.submit().await?;
    Ok(())
}

#[then("the search results page is displayed")]
async fn results_page_is_displayed(browser: &mut WebDriver) -> WebDriverResult<()> {
    browser.find(By::Id("links")).await?;
    Ok(())
}

#[then("the results contain \"(.*)\"")]
async fn results_contain_text(browser: &mut WebDriver, text: String) -> WebDriverResult<()> {
    let content = browser.source().await?;
    if content.contains(&text) { Ok(()) }
    else { Err(thirtyfour::error::WebDriverError::CustomError(
        format!("Result text not found: expected substring '{text}'")
    )) }
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
  Given the login page is displayed
  When a user enters username "<username>" and password "<password>"
  Then the message "<message>" is shown

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
<<<<<<< HEAD
#[when("a user enters username \"<username>\" and password \"<password>\"")]
||||||| parent of 4aac498 (Handle placeholder edge cases)
#[when("I enter username \"<username>\" and password \"<password>\"")]
=======
#[when("I enter username {username} and password {password}")]
>>>>>>> 4aac498 (Handle placeholder edge cases)
async fn enter_credentials(
    browser: &mut WebDriver,
    username: String,
    password: String,
) -> WebDriverResult<()> {
    //... implementation...
    Ok(())
}

<<<<<<< HEAD
<<<<<<< HEAD
#[then("the message \"<message>\" is shown")]
async fn see_message(#[from(browser)] driver: &mut WebDriver, message: String) {
||||||| parent of 2cd8b08 (Clarify implicit fixture docs)
#[then("I should see the message \"<message>\"")]
async fn see_message(#[from(browser)] driver: &mut WebDriver, message: String) {
=======
#[then("I should see the message \"<message>\"")]
async fn see_message(browser: &mut WebDriver, message: String) {
>>>>>>> 2cd8b08 (Clarify implicit fixture docs)
||||||| parent of 4aac498 (Handle placeholder edge cases)
#[then("I should see the message \"<message>\"")]
async fn see_message(browser: &mut WebDriver, message: String) {
=======
#[then("I should see the message {message}")]
async fn see_message(browser: &mut WebDriver, message: String) -> WebDriverResult<()> {
>>>>>>> 4aac498 (Handle placeholder edge cases)
    //... assert message is visible...
    Ok(())
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
// When 50 dollars are deposited

// Step definition in.rs file:
#[when("a user deposits {amount:u32} dollars")]
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

#### 1.3.4 Filtering Scenarios with Tags

Tags provide a convenient way to organize scenarios and control which tests
run. The `#[scenario]` macro will accept an optional `tags` argument containing
an expression such as `"@fast and not @wip"`. Only scenarios whose tags satisfy
this expression will expand into test functions. Filtering occurs at
macro-expansion time; unmatched scenarios do not generate tests (no runtime
skipping). The `scenarios!` macro will offer the same argument to filter an
entire directory of feature files.

Tag scope:

- Scenario tags inherit all tags declared at the `Feature:` level.
- For `Scenario Outline`, tags on the outline and on each `Examples:` block
  apply to the expanded cases produced from that block.
- Tag composition uses set union; duplicates are ignored. There is no implicit
  removal or override of inherited tags.

**Example:**

```rust
#[scenario(path = "search.feature", tags = "@fast and not @wip")]
fn search_fast() {}
```

The macro emits a test only when the matched scenario carries the `@fast` tag
and lacks the `@wip` tag.

Grammar and semantics:

- Tokens:
  - Tags are identifiers prefixed with `@` and match `[A-Za-z_][A-Za-z0-9_]*`.
  - Operators: `and`, `or`, `not`.
  - Parentheses `(` `)` group sub-expressions.
- Precedence: `not` > `and` > `or`. Parentheses override precedence.
- Associativity: `and` and `or` are left-associative; `not` is unary-prefix.
- Whitespace is ignored between tokens.
- Tag matching is case-sensitive; operator keywords are case-insensitive.
- Invalid expressions cause a `compile_error!` with a message that includes the
  byte offset of the failure and a short reason.
- Omitting the `tags` argument applies no filter; an explicit `""` or unknown
  tokens (e.g., `&&`, `||`, `!`) are invalid and emit `compile_error!`.
- Empty parentheses `()` and dangling operators (`@a and`, `or @b`, leading
  `and`/`or`) are invalid.
- Matching is set-membership only; tags do not carry values.

Both macros delegate tag-expression parsing to a shared module so that
`#[scenario]` and `scenarios!` share identical grammar and diagnostics.

EBNF:

```ebnf
expr      ::= or_expr
or_expr   ::= and_expr { "or" and_expr }
and_expr  ::= not_expr { "and" not_expr }
not_expr  ::= [ "not" ] primary
primary   ::= TAG | "(" expr ")"
TAG       ::= "@" IDENT
IDENT     ::= [A..Z | a..z | "_"] { A..Z | a..z | 0..9 | "_" }*
```

Example diagnostic:

```text
error: invalid tag expression at byte 7: expected tag or '(' after 'and'
```

`scenarios!` usage:

```rust
// Include smoke OR (critical AND not wip):
scenarios!("tests/features/", tags = "@smoke or (@critical and not @wip)");

// Exclude slow:
scenarios!("tests/features/", tags = "not @slow");

// Operator keywords are case-insensitive:
scenarios!("tests/features/", tags = "@SMOKE Or Not @Wip");
```

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

- Argument: An optional string literal representing the Gherkin step text. If
  omitted or containing only whitespace, the pattern is inferred from the
  function name by replacing underscores with spaces. A literal `""` registers
  an empty pattern. Inference preserves whitespace semantics: leading and
  trailing underscores become spaces, consecutive underscores become multiple
  spaces, and letter case is preserved. This avoids duplicating names while
  keeping the macros simple.

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
an isolated, stateless environment.[^20] This means that when the

`#[scenario]` macro is expanding, it has no direct way to discover the
functions that have been decorated with `#[given]`, `#[when]`, or `#[then]`. It
cannot scan the project's source code, reflect on other modules, or access a
shared compile-time state to build a map of available steps.[^22] This stands
in stark contrast to

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

To surface missing steps earlier, the macros crate now maintains a small,
compile‑time registry, and each `#[given]`, `#[when]`, and `#[then]` invocation
records its keyword and pattern there. When `#[scenario]` expands, it consults
this registry and emits a `compile_error!` for any Gherkin step that lacks a
unique definition. Because the registry only sees steps from the current
compilation unit, each entry stores the originating crate’s identifier to avoid
false positives from unrelated crates compiled in the same process. Scenarios
that reference steps in other crates would otherwise fail to compile, so the
crate defaults to a permissive mode that prints warnings for unknown steps.
Enabling the `strict-compile-time-validation` feature turns those warnings into
errors. The registry simply records metadata but reuses the runtime crate’s
pattern‑matching logic during validation, introducing a build-time dependency.
`inventory` is employed later for runtime, cross‑crate discovery and does not
power this compile‑time registry.

Step definitions are recorded per crate and grouped by keyword, enabling direct
lookups without scanning unrelated patterns. When the current crate has no
registered steps, non-strict validation emits a warning and continues, so that
definitions from other crates can satisfy the scenario.

The following sequence diagram illustrates the feature-gated step registration
and scenario validation flow:

```mermaid
sequenceDiagram
  autonumber
  actor Dev as Developer
  participant Attr as Step attribute macro
  participant Reg as validation::steps (gated)
  participant Scen as Scenario macro

  rect rgba(240,248,255,0.6)
    note over Attr: Step attribute expansion
    Dev->>Attr: define step fn(pattern)
    alt feature compile-time-validation
      Attr->>Reg: register_step(keyword, pattern)
      Reg-->>Attr: Ok
    else no feature
      Attr--xReg: registration code not compiled
    end
  end

  rect rgba(240,255,240,0.6)
    note over Scen: Scenario macro expansion & validation
    Dev->>Scen: #[scenario(...)]
    Scen->>Scen: parse steps → ParsedStep{keyword,text,docstring,table, span?}
    alt strict-compile-time-validation
      Scen->>Reg: validate(steps, strict=true)
      Reg-->>Scen: Err(missing with spans)
      Scen-->>Dev: compile_error at missing span
    else compile-time-validation only
      Scen->>Reg: validate(steps, strict=false)
      Reg-->>Scen: warnings (per-span)
      Scen-->>Dev: expanded scenario (with warnings)
    else no validation features
      Scen-->>Dev: expanded scenario (no validation)
    end
  end
```

Continuous integration verifies Markdown formatting and diagram rendering.
Every pull request runs `make fmt`, `make markdownlint`, and `make nixie`; the
job fails if formatting or Mermaid rendering errors are detected.

Because registration occurs as the compiler encounters each attribute, step
definitions must appear earlier in a module than any `#[scenario]` that uses
them. Declaring a scenario first would trigger validation before the step is
registered, producing a spurious "No matching step definition" error. A UI test
(`scenario_out_of_order`) documents this requirement.

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
    pub keyword: StepKeyword, // e.g., Given, When, Then, And or But
    pub pattern: &'static StepPattern, // The pattern string from the attribute,
                                       // e.g., "A user has {count} cucumbers"
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
step keywords are surfaced early. Matching ignores case and surrounding
whitespace. All five Gherkin keywords are recognized and `And`/`But` are
resolved to the preceding primary keyword during parsing.

The [`StepPattern`](../crates/rstest-bdd/src/pattern.rs) wrapper encapsulates
the pattern text so that step lookups cannot accidentally mix arbitrary strings
with registered patterns. Each pattern is compiled into a regular expression
when the step registry is initialized, surfacing invalid syntax immediately.
Equality and hashing rely solely on the pattern text. Transient fields like the
cached `Regex` are ignored to preserve identity-by-source-text semantics. The
global registry stores `(StepKeyword, &'static StepPattern)` keys in a
`hashbrown::HashMap` and uses the raw-entry API for constant-time lookups by
hashing the pattern text directly.

Placeholder parsing converts the pattern text into a regular expression using a
single-pass scanner. The current implementation relies on `pub(crate)` helpers
— `build_regex_from_pattern`, `try_parse_common_sequences`,
`parse_context_specific`, and `parse_placeholder`. The diagram below shows how
`compile` invokes the scanner and how malformed placeholders or unbalanced
braces surface as errors. This single-pass scanner is the current
implementation; issue #42 proposes replacing it with a simpler
`regex::Regex::replace_all` based approach.

```mermaid
sequenceDiagram
  autonumber
  actor Dev as Developer
  participant SP as StepPattern
  participant RB as build_regex_from_pattern (pub(crate))
  participant TC as try_parse_common_sequences (pub(crate))
  participant PC as parse_context_specific (pub(crate))
  participant PP as parse_placeholder (pub(crate))
  participant RX as regex::Regex

  Dev->>SP: compile()
  SP->>RB: build_regex_from_pattern(text)
  loop over pattern bytes
    RB->>TC: try_parse_common_sequences(...)
    alt recognized sequence
      TC-->>RB: consume
    else other character
      RB->>PC: parse_context_specific(...)
      alt placeholder start
        PC->>PP: parse_placeholder(...)
        alt OK
          PP-->>PC: Ok(())
        else Malformed/unbalanced
          PP-->>PC: Err(regex::Error)
          PC-->>RB: Err(regex::Error)
          RB-->>SP: Err(regex::Error)
          SP-->>Dev: Err
        end
      else stray/unmatched brace
        PC-->>RB: Err(regex::Error)
        RB-->>SP: Err(regex::Error)
        SP-->>Dev: Err
      end
    end
  end
  alt stray depth != 0
    RB-->>SP: Err(regex::Error)
    SP-->>Dev: Err
  else balanced
    RB-->>SP: Ok(src)
    SP->>RX: Regex::new(src)
    RX-->>SP: Ok(Regex)
    SP-->>Dev: Ok(())
  end
```

Figure: `compile` delegates to the internal single-pass scanner. At compile
time, `StepPattern::compile` returns a `Result<(), regex::Error>`, and
`extract_placeholders` wraps any compile error as
`PlaceholderError::InvalidPattern` at runtime.

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

#### Registry Interaction Diagrams

```mermaid
sequenceDiagram
  autonumber
  actor Dev as Macro User
  participant V as validate_steps_exist
  participant RS as Registry State
  participant VA as Validators
  participant D as Diagnostics

  Dev->>V: validate_steps_exist(steps, strict)
  V->>RS: validate_registry_state(defs, crate_id, strict)
  alt Definitions present
    V->>VA: validate_individual_steps(steps, defs)
    loop each step
      VA->>VA: find_step_matches(step, patterns)
      VA->>VA: validate_single_step(step, kw, matches)
      VA-->>D: Optional (span, message)
    end
    V-->>Dev: Emit per-step diagnostics (if any)
  else No definitions
    RS-->>D: Warning/Errors per strict mode
    V-->>Dev: Report outcome
  end
```

Figure: `validate_steps_exist` drives step validation and diagnostics.

```mermaid
sequenceDiagram
  autonumber
  participant U as Caller
  participant RSO as register_step
  participant RSI as register_step_inner
  participant REG as Registry

  U->>RSO: register_step(pattern, handler)
  RSO->>RSI: register_step_inner(current_crate_id(), ...)
  RSI->>RSI: normalise_crate_id -> Box<str>
  RSI->>REG: Insert step pattern under crate key
  REG-->>U: Registered
```

Figure: Step registration flows through a thin wrapper to the registry.

```mermaid
classDiagram
    class StepKeyword
    class StepPattern {
        +new(pattern: &str)
        +compile()
        +as_str()
    }
    class CrateDefs {
        +patterns(keyword: StepKeyword): &[StepPattern]
        +is_empty()
    }
    class ParsedStep {
        +text: String
    }
    class StepRegistry {
        +REGISTERED: Mutex<HashMap<String, CrateDefs>>
        +register_step_inner(keyword: StepKeyword,
                             pattern: &syn::LitStr,
                             crate_id: impl Into<String>)
        +register_step(keyword: StepKeyword, pattern: &syn::LitStr)
        +register_step_for_crate(keyword: StepKeyword,
                                 literal: &str,
                                 crate_id: &str)
        +validate_steps_exist(steps: &[ParsedStep],
                              strict: bool) -> Result<(), syn::Error>
    }
    StepRegistry --> CrateDefs
    CrateDefs --> StepPattern
    StepRegistry --> ParsedStep
    StepRegistry --> StepKeyword
```

Figure: Core types involved in registration and validation.

### 2.4 The Macro Expansion Process: A Compile-Time to Runtime Journey

The interaction between the user's code, the `rstest-bdd` macros, and the final
test execution can be broken down into a sequence of compile-time and runtime
events.

**1.** `#[given]` **Expansion (Compile-Time)**

- **Input Code:**

```rust

#[given("a user exists")]
fn given_i_am_a_user(mut user_context: UserContext) { /\*... \*/ }
```

- **Macro Action:** The `#[given]` proc-macro parses its attribute string
  (`"a user exists"`) and the function it's attached to. It then generates an
  `inventory::submit!` block. This block contains the static definition of a
  `Step` struct, where the `run` field is a type-erased pointer to a wrapper
  around the `given_i_am_a_user` function.

**2.** `#[scenario]` **Expansion (Compile-Time)**

- **Input Code:**

```rust
fn test_my_scenario(my_fixture: MyFixture) { /\* final assertion \*/ }
```

- **Macro Action:**

1. The `#[scenario]` proc-macro performs file I/O to read the contents of
   `f.feature`.
2. It uses a Gherkin parser crate (such as `gherkin` [^26]) to parse the feature
   file content into an Abstract Syntax Tree (AST).
3. It traverses the AST to find the `Scenario` with the name "My Scenario".
4. During compilation, the macro validates that each Gherkin step has a
   matching definition recorded by the step macros and emits `compile_error!`
   when one is missing. At runtime, the generated test still performs lookup
   via `inventory::iter::<Step>()` to resolve the concrete function and to
   perform placeholder matching and argument extraction.
5. Using the `quote!` macro [^28], it generates a completely new Rust function.
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

- Increase the minimum supported Rust version to 1.75 and remove the
  `async_trait` dependency from `World` and writer traits to simplify
  implementations and match Cucumber v0.21.

- Introduce a `skip!` macro that step or hook functions can invoke to record a
  `Skipped` outcome and halt the remaining steps. The macro accepts an optional
  message and integrates with the scenario orchestrator so the scenario is
  marked as skipped rather than failed.

- Extend tag filtering to recognize an `@allow_skipped` tag and provide a
  `fail_on_skipped` configuration flag. Scenarios bearing `@allow_skipped`
  bypass the failure check even when `fail_on_skipped` is enabled.

- Propagate skipped status through the `cargo-bdd` CLI and the JSON and JUnit
  writers. Emit a `<skipped>` child on each `<testcase>` element in JUnit
  output and use lowercase `skipped` status strings in JSON and the CLI while
  preserving long messages and consistent casing.

- Document the `skip!` macro, the `@allow_skipped` tag and the Rust 1.75
  migration with examples illustrating `fail_on_skipped` behaviour.

Subsequent phases refine these capabilities: Phase 5 will streamline the
macro’s syntax and add compile-time diagnostics, while Phase 6 will surface
skip details in diagnostic tooling and IDE integrations.

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

| Feature          | rstest-bdd (Proposed)                                                                                                        | cucumber                                                                          |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Test Runner      | Standard cargo test (via rstest expansion)                                                                                   | Custom runner invoked from a main function (World::run(…)) [^23]                  |
| State Management | rstest fixtures; dependency injection model [^1]                                                                             | Mandatory World struct; a central state object per scenario [^11]                 |
| Step Discovery   | Automatic via compile-time registration (inventory) and runtime matching                                                     | Explicit collection in the test runner setup (World::cucumber().steps(…)) [^37]   |
| Parameterization | Gherkin Scenario Outline maps to rstest's #[case] parameterization [^15]                                                     | Handled internally by the cucumber runner                                         |
| Async Support    | Runtime-agnostic via feature flags (e.g., tokio, async-std) which emit the appropriate test attribute (#[tokio::test], etc.) | Built-in; requires specifying an async runtime [^11]                              |
| Ecosystem        | Seamless integration with rstest and cargo features                                                                          | Self-contained framework; can use any Rust library within steps                   |
| Ergonomics       | pytest-bdd-like; explicit #[scenario] binding links test code to features [^6]                                               | cucumber-jvm/js-like; feature-driven, with a central test runner                  |
| Core Philosophy  | BDD as an extension of the existing rstest framework                                                                         | A native Rust implementation of the Cucumber framework standard                   |

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

  The implemented tool lives in a standalone `cargo-bdd` crate that acts as a
  cargo subcommand. It queries the runtime step registry and exposes three
  commands: `steps`, `unused`, and `duplicates`. Step usage is tracked in
  memory and appended to `<target-dir>/.rstest-bdd-usage.json`, allowing
  diagnostics to persist across binaries. Because `inventory` operates per
  binary, the subcommand compiles each test target and executes it with
  `RSTEST_BDD_DUMP_STEPS=1` and a private `--dump-steps` flag to stream the
  registry as JSON. The tool merges these dumps so diagnostics cover the entire
  workspace.

  The sequence below illustrates the diagnostic workflow:

```mermaid
sequenceDiagram
  autonumber
  actor Dev as Developer
  participant CB as cargo-bdd
  participant CM as cargo (metadata/build)
  participant TB as Test Binaries
  participant FS as target/.rstest-bdd-usage.json

  Dev->>CB: cargo bdd [steps|unused|duplicates]
  CB->>CM: cargo metadata (detect test targets)
  alt targets found
    CB->>CM: cargo test --no-run --message-format=json
    CM-->>CB: compiler-artifact JSON (paths to test binaries)
    loop per test binary
      CB->>TB: exec test-binary --dump-steps
      TB->>FS: append usage (on step lookups)
      TB-->>CB: stdout JSON (registered steps + usage flags)
    end
    CB->>CB: merge/aggregate steps
    opt unused
      CB->>CB: filter used==false
    end
    opt duplicates
      CB->>CB: group by (keyword, pattern) size>1
    end
    CB-->>Dev: print diagnostics
  else no targets
    CB-->>Dev: no output / empty
  end
```

  The usage file lives under the Cargo target directory and honours the
  `CARGO_TARGET_DIR` environment variable.

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
macros now parse the pattern's `{name}` placeholders up front and inspect the
parameters of the attached function. Parameters that match a placeholder become
step arguments; any remaining parameters are treated as fixture requests, with
an optional `#[from(name)]` attribute allowing the argument name to differ from
the fixture's. The macro generates a wrapper function taking a `StepContext`
and registers this wrapper in the step registry. The wrapper retrieves the
required fixtures from the context and calls the original step function.

```mermaid
sequenceDiagram
    participant MacroExpander
    participant StepFunction
    participant PatternParser
    participant ArgExtractor
    participant ErrorReporter
    MacroExpander->>PatternParser: Extract placeholder names from pattern
    PatternParser-->>MacroExpander: Return set of placeholders
    MacroExpander->>ArgExtractor: Pass function signature and placeholders
    ArgExtractor->>StepFunction: Inspect parameters
    ArgExtractor->>ArgExtractor: Classify parameters
    Note over ArgExtractor: If parameter matches placeholder, classify as step arg
    Note over ArgExtractor: If parameter does not match placeholder, classify as fixture
    ArgExtractor->>ErrorReporter: Report missing placeholders or fixture errors
    ErrorReporter-->>MacroExpander: Emit compile-time error if needed
    ArgExtractor-->>MacroExpander: Return argument classification
    MacroExpander->>StepFunction: Generate wrapper code with inferred fixtures
```

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
covers pattern mismatches and placeholder or regex compilation failures.

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
}
```

- Fields and metadata:
  - PatternMismatch: no fields; indicates the text did not satisfy the
    pattern. There is no separate “missing capture” error; a missing or extra
    capture manifests as a mismatch because the entire text must match the
    compiled regular expression for the pattern.
  - InvalidPlaceholder(String): the pattern contained malformed placeholder
    syntax and could not be parsed. The message includes the zero-based byte
    offset and, when available, the offending placeholder name.
  - InvalidPattern(String): carries the underlying `regex::Error` string coming
    from the regular expression engine during compilation of the pattern. No
    additional metadata (placeholder name, position, or line info) is captured.

- Example error strings (exact `Display` output):
  - Pattern mismatch: `"pattern mismatch"`
  - Invalid placeholder:

    ```text
    "invalid placeholder syntax: invalid placeholder in step pattern at byte 6 (zero-based) for placeholder `n`"
    ```

  - Invalid pattern: `"invalid step pattern: regex parse error: error message"`

  - Example JSON mapping (for consumers that serialize errors). Note: this is
    not emitted by the library; it suggests a shape for mapping the enum to
    JSON at an API boundary:

```json
{
  "code": "pattern_mismatch",
  "message": "pattern mismatch"
}

{
  "code": "invalid_placeholder",
  "message": "invalid placeholder syntax: invalid placeholder in step pattern at byte 6 (zero-based) for placeholder `n`"
}

{
  "code": "invalid_pattern",
  "message": "invalid step pattern: <regex_error>"
}
```

Note: `code` values are stable identifiers intended for programmatic use.

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

### 3.10 Implicit Fixture Injection Implementation

To streamline step definitions, the macro system now infers fixtures by
analysing the step pattern during expansion. Placeholder names are extracted
from the pattern string, and any function parameter whose identifier matches a
placeholder is treated as a typed step argument. Remaining parameters are
assumed to be fixtures and are looked up in the [`StepContext`] at runtime.

For early feedback, each inferred fixture name is referenced in the generated
wrapper. If no fixture with that name is in scope, the wrapper fails to
compile, surfacing the missing dependency before tests run. Conversely, if the
pattern declares placeholders without matching parameters, macro expansion
aborts with a clear diagnostic listing the missing arguments.

### 3.11 Runtime Module Layout (for Contributors)

To keep responsibilities cohesive the runtime is split into focused modules.
Public APIs are re‑exported from `lib.rs` so consumers continue to import from
`rstest_bdd::*` as before.

- `types.rs` — Core types and errors:
  - `PatternStr`
  - `StepText`
  - `StepKeyword`
  - `StepKeywordParseError`
  - `PlaceholderError`
  - `StepFn`

- `pattern.rs` — Step pattern wrapper:
  - `StepPattern::new`
  - `compile`
  - `regex`

- `placeholder.rs` — Placeholder extraction and scanner:
  - `extract_placeholders`
  - `build_regex_from_pattern`

- `context.rs` — Fixture context:
  - `StepContext`

- `registry.rs` — Registration and lookup:
  - `step!` macro
  - global registry map
  - `lookup_step`
  - `find_step`.

- `lib.rs` — Public API facade:
  - Re-exports public items
  - `greet` example function

All modules use en‑GB spelling and include `//!` module‑level documentation.

## Part 4: Internationalization and Localization Roadmap

### 4.1 Phase 1: Foundational Gherkin Internationalization (target v0.4)

- **Language detection:** Update the macro parser to honour the optional
  `# language: <lang>` declaration in feature files. The parser creates a
  language‑aware `gherkin::GherkinEnv` and defaults to English when the
  declaration is absent to preserve backwards compatibility.
- **Language‑aware keyword parsing:** Refactor `StepKeyword` parsing to rely on
  `gherkin::StepType`, allowing localized keywords such as `Étant donné` and
  `Gegeben sei` to map to the correct step types.
- **Testing and validation:** Introduce multilingual feature files, including
  French, German, and Spanish, to validate that `Given`, `When`, `Then`, `And`,
  and `But` are correctly recognized in each language. These scenarios will run
  in CI to maintain coverage as new languages are added.

### 4.2 Phase 2: Localization of Library Messages with Fluent (target v0.5)

- **Dependency integration:** Add `i18n-embed`, `rust-embed`, and `fluent` as
  dependencies to supply localization infrastructure.

  ```toml
  [dependencies]
  i18n-embed = { version = "0.16", features = ["fluent-system", "desktop-requester"] }
  rust-embed = "8"
  fluent = "0.17"
  ```

- **Localization resource creation:** Create an `i18n/<locale>/` hierarchy in
  the `rstest-bdd` crate containing Fluent translation files with identifiers
  such as `error-missing-step`. If the macros crate also emits messages,
  maintain a separate `i18n/` in `rstest-bdd-macros` or introduce a shared
  `rstest-bdd-i18n` crate to host common assets.
- **Resource embedding and loading:** Embed the `i18n` directory using
  `rust-embed` and expose it through a `Localizations` struct implementing
  `I18nAssets` so the Fluent loader can discover translations. Missing keys or
  unsupported locales fall back to English.
- **Refactor diagnostic messages:** Keep proc‑macro diagnostics stable and in
  English for deterministic builds. Localize user‑facing runtime messages in
  the `rstest-bdd` crate using `FluentLanguageLoader` and `i18n-embed`'s locale
  requesters. Avoid compile‑time locale switches in macros.

### 4.3 Phase 3: Documentation and User Guidance (target v0.6)

- **Update user documentation:** Extend `README.md` and `docs/users-guide.md`
  with guidance on writing non‑English feature files and selecting locales for
  runtime diagnostics.
- **Provide multilingual examples:** Add a new example test suite under
  `/examples` showcasing a non‑English Gherkin file and its localized
  diagnostics.
- **Update contributor guidelines:** Amend `CONTRIBUTING.md` with instructions
  for updating translations when new user‑facing messages are introduced.

## **Works cited**

[^1]: A Complete Guide to Behaviour-Driven Testing With Pytest BDD, accessed on
    20 July 2025, <https://pytest-with-eric.com/bdd/pytest-bdd/>.
[^2]: rstest - [crates.io](http://crates.io): Rust Package Registry, accessed on
    20 July 2025, <https://crates.io/crates/rstest/0.12.0>.
[^3]: Pytest-BDD: the BDD framework for pytest — pytest-bdd 8.1.0 documentation,
    accessed on 20 July 2025, <https://pytest-bdd.readthedocs.io/>.
[^4]: Behavior-Driven Python with pytest-bdd - Test Automation University -
    Applitools, accessed on 20 July 2025,
    <https://testautomationu.applitools.com/behaviour-driven-python-with-pytest-bdd/>.
[^5]: Python Testing 101: pytest-bdd - Automation Panda, accessed on 20 July
    2025,
    <https://automationpanda.com/2018/10/22/python-testing-101-pytest-bdd/>.
[^6]: Introduction - Cucumber Rust Book, accessed on 20 July 2025,
    <https://cucumber-rs.github.io/cucumber/main/>.
[^7]: Behavior-Driven Development: Python with Pytest BDD -
    [Testomat.io](http://Testomat.io), accessed on 20 July 2025,
    <https://testomat.io/blog/pytest-bdd/>.
[^8]: Scenario Outline in PyTest – BDD - QA Automation Expert, accessed on
    20 July 2025,
    <https://qaautomation.expert/2024/04/11/scenario-outline-in-pytest-bdd/>.
    See also chapter 5 of Behaviour-Driven Python with pytest-bdd,
    <https://testautomationu.applitools.com/behaviour-driven-python-with-pytest-bdd/chapter5.html>.
[^9]: How can I create parameterised tests in Rust? - Stack Overflow, accessed
    on 20 July 2025,
    <https://stackoverflow.com/questions/34662713/how-can-i-create-parameterized-tests-in-rust>.
[^10]: pytest-bdd - Read the Docs, accessed on 20 July 2025,
    <https://readthedocs.org/projects/pytest-bdd/downloads/pdf/latest/>.
[^11]: pytest-bdd - PyPI, accessed on 20 July 2025,
    <https://pypi.org/project/pytest-bdd/>. Discussion on sharing state between
    tests,
    <https://www.reddit.com/r/rust/comments/1hwx3tn/what_is_a_good_pattern_to_share_state_between/>.
     Rust issue tracking shared state,
    <https://github.com/rust-lang/rust/issues/44034>.
[^12]: inventory - Rust - [Docs.rs](http://Docs.rs), accessed on 20 July 2025,
    <https://docs.rs/inventory>. Shared steps and hooks with pytest-bdd,
    <https://www.luizdeaguiar.com.br/2022/08/shared-steps-and-hooks-with-pytest-bdd/>.
[^13]: Guide to Rust procedural macros | [developerlife.com], accessed on 20
    July 2025, <https://developerlife.com/2022/03/30/rust-proc-macro/>. The
    Rust macro system part 1, accessed on 20 July 2025,
    <https://medium.com/@alfred.weirich/the-rust-macro-system-part-1-an-introduction-to-attribute-macros-73c963fd63ea>.
     cucumber-rs repository, <https://github.com/cucumber-rs/cucumber>.
    Cucumber in Rust beginner's tutorial,
    <https://www.florianreinhard.de/cucumber-in-rust-beginners-tutorial/>.
[^14]: la10736/rstest: Fixture-based test framework for Rust - GitHub, accessed
    on 20 July 2025, <https://github.com/la10736/rstest>.

[^15]: rstest crate documentation for `#[case]` parameterisation, accessed on
    20 July 2025, <https://docs.rs/rstest/latest/rstest/attr.case.html>.
[^20]: Rust Reference: procedural macros operate without shared state, accessed
    on 20 July 2025,
    <https://doc.rust-lang.org/reference/procedural-macros.html>.
[^22]: Why macros cannot discover other macros, discussion on
    users.rust-lang.org, accessed on 20 July 2025,
    <https://users.rust-lang.org/t/why-cant-macros-discover-other-macros/3574>.
[^23]: cucumber crate documentation for `World::run`, accessed on 20 July 2025,
    <https://docs.rs/cucumber>.
[^26]: gherkin crate on crates.io, accessed on 20 July 2025,
    <https://crates.io/crates/gherkin>.
[^28]: quote crate macros, accessed on 20 July 2025,
    <https://docs.rs/quote>.
[^37]: cucumber crate step collection API, accessed on 20 July 2025,
    <https://docs.rs/cucumber/latest/cucumber/struct.World.html#method.steps>.
