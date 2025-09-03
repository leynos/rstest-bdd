# Design Document: Phase 4 Ergonomics and Developer Experience

## 1. Introduction

With the core mechanics of `rstest-bdd` established in Phases 1-3, Phase 4
transitions the focus towards enhancing developer experience and reducing
boilerplate. The features outlined in this document are designed to make
writing and maintaining behaviour tests more intuitive, less verbose, and
better integrated into the development workflow. This document synthesises user
feedback and the high-level goals of the project roadmap into a concrete
technical design for implementation.

The guiding principles for this phase are:

- **Reduce ceremony:** Eliminate repetitive attributes and patterns where
  intent can be clearly inferred.
- **Improve data flow:** Provide more natural ways to share state and pass data
  between steps.
- **Enhance tooling:** Automate common, repetitive tasks to accelerate the
  test-writing process.

## 2. Reducing Boilerplate in Step Definitions

A significant portion of the feedback centres on the verbosity of step
definitions. The following proposals aim to address this by making the
framework's macros more intelligent and context-aware.

### 2.1. Implicit Fixture and Step Argument Injection

**Goal:** Eliminate the need for `#[from(fixture_name)]` on every fixture and
reduce noise in step function signatures. The current implementation requires
explicit annotation for every fixture, which is verbose, especially when a
fixture is used across many steps.

Proposed Design:

The argument classification logic in
crates/rstest-bdd-macros/src/codegen/wrapper/args.rs will be updated to
automatically distinguish between fixtures and step arguments based on the
context provided by the step pattern.

1. **Pattern-First Analysis:** When a step macro (`#[given]`, `#[when]`, etc.)
   is expanded, it will first parse its pattern string to identify all
   placeholders (e.g., `{count:u32}`).
2. **Argument Classification:** It will then iterate through the step
   function's parameters:

    - If a parameter's name matches a placeholder in the pattern, it will be
  classified as a **step argument**.
    - If a parameter's name does _not_ match any placeholder, it will be
      classified as a **fixture**.

3. **Conflict Resolution:**

    - The explicit `#[from(name)]` attribute will be retained to handle cases
      where a parameter name must differ from the fixture name.
    - If a parameter is not found in the pattern's placeholders and is not
  explicitly marked with `#[from]`, a compile-time error will be emitted if no
  fixture with that name is in scope, preventing ambiguity.

**User Experience:**

- **Before:**

```rust
#[when("I add {count:u32} pumpkins to the basket")]
fn add_pumpkins(#[from(basket)] b: &mut Basket, count: u32) {
    // ...
}
```

- **After:**

```rust
#[when("I add {count:u32} pumpkins to the basket")]
fn add_pumpkins(basket: &mut Basket, count: u32) {
    // `basket` is implicitly a fixture, `count` is a step argument.
}
```

### 2.2. Inferred Step Patterns

**Goal:** Remove the need for an explicit pattern string in the step attribute
when the function name clearly describes the step. This reduces duplication
where the pattern and function name are nearly identical.

Proposed Design:

The step attribute macros (#[given], #[when], #[then]) in
crates/rstest-bdd-macros/src/macros/mod.rs will be modified to make the pattern
string argument optional.

1. **Optional Argument:** The macros will be updated to accept `#[given]`,
   `#[given("a pattern")]`, `#[when]`, etc.
2. **Inference Logic:** If the pattern string is omitted:

    - The macro will take the identifier of the function it decorates.
    - It will convert the identifier from `snake_case` to a sentence-case string
  (e.g., `the_user_logs_in` becomes `"the user logs in"`).
    - Parameter names that are valid placeholders (e.g., `_var` or `var`) will
      be converted to `{var}` format within the inferred pattern.

3. **Doc Comment Fallback:** As a secondary mechanism, if no pattern is
   provided and the function name is ambiguous, the macro could fall back to
   using the function's Rustdoc summary line. However, inferring from the
   function name is more direct and less prone to parsing errors. The primary
   implementation will focus on name inference.

**User Experience:**

- **Before:**

```rust
#[given("the user is logged in")]
fn the_user_is_logged_in(user_session: &mut Session) {
    // ...
}
```

- **After:**

```rust
#[given]
fn the_user_is_logged_in(user_session: &mut Session) {
    // Pattern "the user is logged in" is inferred.
}
```

### 2.3. Struct-based Step Arguments

**Goal:** Condense step function signatures that have many placeholders by
binding them to the fields of a single struct. This improves readability and
centralises parsing logic.

Proposed Design:

A new derive macro, StepArgs, will be introduced in rstest-bdd-macros.

1. **`#[derive(StepArgs)]` Macro:**

    - This macro will be applied to a user-defined struct.
    - It will generate an implementation of `TryFrom<Vec<String>>` for the
      struct. The implementation will expect a vector of captured strings from
      the step pattern and attempt to parse each string into the corresponding
      struct field using `FromStr`. The order of fields will map to the order
      of captures.

2. **Step Macro Integration:**

    - The `extract_args` function will be updated to detect a single parameter
  whose type derives `StepArgs`.
    - If such a parameter is found, it will consume all available placeholders
      from the pattern. No other step arguments will be permitted. Fixture
      arguments will still be allowed.
    - The generated step wrapper will capture all placeholders into a
      `Vec<String>` and then call `try_into()` to populate the struct.

**User Experience:**

- **Before:**

```rust
#[when("a user named {name} with age {age:u32} and role {role} is created")]
fn create_user(name: String, age: u32, role: String) {
    // ...
}
```

- **After:**

```rust
use rstest_bdd_macros::StepArgs;

#[derive(StepArgs)]
struct NewUser {
    name: String,
    age: u32,
    role: String,
}

#[when("a user named {name} with age {age:u32} and role {role} is created")]
fn create_user(user_data: NewUser) {
    // access via user_data.name, user_data.age, etc.
}
```

## 3. Improving Data Flow and State Management

These features focus on making state management across steps more robust and
idiomatic, moving away from manual `RefCell` patterns towards
framework-provided solutions.

### 3.1. Passing Values Between Steps via Return Types

**Goal:** Allow a step (typically `#[when]`) to return a value that can be used
by a subsequent step (typically `#[then]`), promoting a more functional data
flow and reducing reliance on shared mutable state for simple value passing.

Proposed Design:

This requires changes to both the runtime and macro crates.

1. `StoreInContext` **Trait** (in `crates/rstest-bdd/src/types.rs`): A new
   trait will be defined to associate a return type with a unique key for
   storage in the `StepContext`. The key will likely be the `TypeId` of the
   return value.

    ```rust
    pub trait StoreInContext: 'static {
        // Provides a way to store and retrieve the value.
        // The StepContext will handle the implementation details.
    }
    
    impl<T: 'static> StoreInContext for T {}
    ```

2. `StepContext` **Enhancement** (in `crates/rstest-bdd/src/context.rs`): The
   `StepContext` will be extended with a type-indexed map (e.g.,
   `HashMap<TypeId, Box<dyn Any>>`) to store these returned values.
3. **Wrapper Codegen Update (in
   **`crates/rstest-bdd-macros/src/codegen/wrapper/emit.rs`**):**

    - The generated wrapper for a step function will inspect its return type.
    - If the return type is not `()` or `Result<(), E>`, the wrapper will
      capture the `Ok(value)` from the step function's result.
    - It will then insert this `value` into the `StepContext` using its
      `TypeId` as the key.

4. **Implicit Injection:** A parameter in a subsequent step that is not a
   fixture and not a step argument, but whose type matches a value stored in
   the context, will be implicitly injected.

**User Experience:**

```rust
#[when("the user searches for an item")]
fn search_for_item(api_client: &ApiClient) -> Result<Vec<SearchResult>, ApiError> {
    // This function now returns the search results directly.
    Ok(api_client.search("pumpkin"))
}

#[then("the results are displayed")]
fn results_are_displayed(results: Vec<SearchResult>) {
    // The `results` are injected directly from the previous step's return value.
    assert!(!results.is_empty());
}
```

### 3.2. Ergonomic Scenario State

**Goal:** Provide a structured, type-safe alternative to manually crafting
state objects with `RefCell<Option<T>>`. This is the most requested ergonomic
feature, aimed at creating a "world" or "state" object with less ceremony.

Proposed Design:

The design will adopt a balanced approach, providing an ergonomic `Slot<T>`
type and a helper derive macro, rather than a fully "magical" #[world]
attribute that hides the fixture mechanism. This aligns with rstest-bdd's
philosophy of augmenting, not obscuring, rstest.

1. `Slot<T>` **Type** (in `crates/rstest-bdd/src/state.rs`): A new public type
   will be introduced. It will be a thin wrapper around `RefCell<Option<T>>`,
   providing a clean API for state manipulation.

    ```rust
    pub struct Slot<T>(RefCell<Option<T>>);
    
    impl<T> Slot<T> {
      pub fn set(&self, value: T);
      pub fn get(&self) -> Option<T> where T: Clone; // Or returns a Ref<'_, T>
      pub fn take(&self) -> Option<T>;
      // ... and other helpers
    }
    ```

2. `#[derive(ScenarioState)]` **Macro** (in `rstest-bdd-macros`):

    - This derive macro will be applied to a user-defined state struct whose
      fields are of type `Slot<T>`.
    - It will automatically generate a `Default` implementation for the struct,
  which initialises each `Slot` to its empty state.
    - This encourages a pattern where the user defines their state struct,
      derives `ScenarioState`, and then provides it as a regular `rstest`
      fixture.

**User Experience:**

```rust
use rstest_bdd::{macros::{given, scenario, ScenarioState}, state::Slot};
use rstest::fixture;

// 1. Define the state struct with Slots
#[derive(ScenarioState, Default)]
struct CliState {
    command_output: Slot<String>,
    exit_code: Slot<i32>,
}

// 2. Provide it as a standard fixture
#[fixture]
fn cli_state() -> CliState {
    CliState::default()
}

// 3. Use it in steps
#[when("I run the command")]
fn run_command(cli_state: &CliState) {
    // No more RefCell<Option<...>> boilerplate
    cli_state.command_output.set("Success".to_string());
    cli_state.exit_code.set(0);
}

#[then("the exit code should be {int}")]
fn check_exit_code(cli_state: &CliState, code: i32) {
    assert_eq!(cli_state.exit_code.take(), Some(code));
}

// 4. The scenario ties it all together
#[scenario(path = "...")]
fn test_cli_command(cli_state: CliState) {
    // The fixture is injected here, making it available to all steps.
}
```

## 4. Developer Tooling and Utilities

### 4.1. Streamlined ,`Result`, Assertions

**Goal:** Simplify the common pattern of asserting that a step returning a
`Result` is either `Ok` or `Err`.

Proposed Design:

Two new macros will be added to the rstest-bdd crate and re-exported.

- `assert_step_ok!(result)`: This macro will take a `Result` value. If the
  result is `Err(e)`, it will panic with a formatted message including the
  error `e`.
- `assert_step_err!(result, expected_msg)`: This macro will assert that the
  `Result` is an `Err`. It can optionally take a second argument to assert that
  the error message contains a specific substring.

These will be simple declarative macros (`macro_rules!`) defined in
`rstest-bdd/src/lib.rs`.

### 4.2. Step Scaffolding

**Goal:** Automate the creation of skeleton step definition files from a
`.feature` file to reduce manual boilerplate.

Proposed Design:

A new binary crate, cargo-bdd, will be created.

1. **Command:** It will provide the subcommand
   `cargo bdd scaffold <path_to_feature>`.
2. **Functionality:**

    - The command will parse the specified `.feature` file, reusing the Gherkin
  parsing logic already present in `rstest-bdd-macros`.
    - It will iterate through all unique step strings in the feature.
    - It will generate a new Rust file (e.g., `tests/steps/my_feature_steps.rs`)
  containing skeleton step functions for each unique step.
    - Placeholders in step strings will be converted into function parameters
      with `String` types as a default.
    - The generated functions will have a `todo!()` macro in their body,
      prompting the developer to provide an implementation.

**Example Output:**

```rust
// Generated from: When I add {count:u32} pumpkins

use rstest_bdd_macros::when;

#[when("I add {count:u32} pumpkins")]
fn i_add_count_pumpkins(count: u32) {
    todo!("Implement this step");
}
```

## 5. Conclusion

The enhancements planned for Phase 4 represent a significant leap forward in
the usability and ergonomics of `rstest-bdd`. By focusing on reducing
boilerplate, improving state management, and providing intelligent tooling,
these changes will make the framework not only more powerful but also more
pleasant to use, encouraging the adoption of BDD practices by lowering the
barrier to writing clear, maintainable, and effective behaviour tests in Rust.
