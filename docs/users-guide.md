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

## Toolchain requirements

`rstest-bdd` targets Rust 1.85 or newer across every crate in the workspace.
Each `Cargo.toml` declares `rust-version = "1.85"`, so `cargo` will refuse to
compile the project on older compilers. The workspace uses the Rust 2024
edition.

The `rstest-bdd` crate currently requires the Rust nightly compiler because it
relies on auto traits and negative impls to normalise step return values. The
repository pins a nightly toolchain for development via `rust-toolchain.toml`.

Step definitions and writers remain synchronous functions; the framework no
longer depends on the `async-trait` crate to express async methods in traits.
Projects that previously relied on `#[async_trait]` in helper traits should
replace those methods with ordinary functions—`StepFn` continues to execute
synchronously and exposes results via `StepExecution`.

## The three amigos

| Role ("amigo")                     | Primary concerns                                                                                                                  | Features provided by `rstest‑bdd`                                                                                                                                                                                                                                                                                                                                                                         |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Business analyst/product owner** | Writing and reviewing business-readable specifications; ensuring that acceptance criteria are expressed clearly.                  | Gherkin `.feature` files are plain text and start with a `Feature` declaration; each `Scenario` describes a single behaviour. Steps are written using keywords `Given`, `When`, and `Then` ([syntax](gherkin-syntax.md#L72-L91)), producing living documentation that can be read by non-technical stakeholders.                                                                                          |
| **Developer**                      | Implementing step definitions in Rust and wiring them to the business specifications; using existing fixtures for setup/teardown. | Attribute macros `#[given]`, `#[when]` and `#[then]` register step functions and their pattern strings in a global step registry. A `#[scenario]` macro reads a feature file at compile time and generates a test that drives the registered steps. Fixtures whose parameter names match are injected automatically; use `#[from(name)]` only when a parameter name differs from the fixture.             |
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
data from an `Examples` table. A `Background` section defines steps that run
before each `Scenario` in a feature file, enabling shared setup across
scenarios. Advanced constructs such as data tables and Docstrings provide
structured or free‑form arguments to steps.

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

### Internationalised scenarios

`rstest-bdd` reads the optional `# language: <code>` directive that appears at
the top of a feature file. When a locale is specified, the parser uses that
language's keyword catalogue, enabling teams to collaborate in their native
language. The `examples/japanese-ledger` crate demonstrates the end-to-end
workflow for Japanese:

```gherkin
# language: ja
フィーチャ: 家計簿の残高を管理する
  シナリオ: 収入を記録する
    前提 残高は0である
    もし 残高に5を加える
    ならば 残高は5である
```

Step definitions use the same Unicode phrases:

```rust,no_run
use japanese_ledger::HouseholdLedger;
use rstest_bdd_macros::{given, when, then};

#[given("残高は{start:i32}である")]
fn starting_balance(ledger: &HouseholdLedger, start: i32) {
    ledger.set_balance(start);
}
```

Running `cargo test -p japanese-ledger` executes both Japanese scenarios. The
full source lives under `examples/japanese-ledger/` for teams that want to copy
the structure into their projects.

## Step definitions

Developers implement the behaviour described in a feature by writing step
definition functions in Rust. Each step definition is an ordinary function
annotated with one of the attribute macros `#[given]`, `#[when]` or `#[then]`.
The annotation takes a single string literal that must match the text of the
corresponding step in the feature file. Placeholders in the form `{name}` or
`{name:Type}` are supported. The framework extracts matching substrings and
converts them using `FromStr`; type hints constrain the match using specialized
regular expressions. If the step text does not supply a capture for a declared
argument, the wrapper panics with
`pattern '<pattern>' missing capture for argument '<name>'`, making the
mismatch explicit.

The procedural macro implementation expands the annotated function into two
parts: the original function and a wrapper function that registers the step in
a global registry. The wrapper captures the step keyword, pattern string and
associated fixtures and uses the `inventory` crate to publish them for later
lookup.

### Fixtures and implicit injection

`rstest‑bdd` builds on `rstest`’s fixture system rather than using a monolithic
“world” object. Fixtures are defined using `#[rstest::fixture]` in the usual
way. When a step function parameter does not correspond to a placeholder in the
step pattern, the macros treat it as a fixture and inject the value
automatically. The optional `#[from(name)]` attribute remains available when a
parameter name must differ from the fixture. Importing a symbol of the same
name is not required; do not alias a function or item just to satisfy the
compiler. Only the key stored in `StepContext` must match.

Internally, the step macros record the fixture names and generate wrapper code
that, at runtime, retrieves references from a `StepContext`. This context is a
key–value map of fixture names to type‑erased references. When a scenario runs,
the generated test inserts its arguments (the `rstest` fixtures) into the
`StepContext` before invoking each registered step.

### Mutable world fixtures

Each scenario owns its fixtures, so value fixtures are stored with exclusive
access. Step parameters declared as `&mut FixtureType` receive mutable
references, making “world” structs ergonomic without sprinkling `Cell` or
`RefCell` wrappers through the fields. Immutable references continue to work
exactly as before; mutability is an opt‑in convenience.

**When to use `&mut Fixture`**

- Prefer `&mut` when the world has straightforward owned fields and the steps
  mutate them directly.
- Prefer `Slot<T>` (from `rstest_bdd::Slot`) when state is optional, when a
  need to reset between steps, or when a step may override values conditionally
  without holding a mutable borrow.
- Combine both: keep the primary world mutable and store optional or
  late‑bound values in slots to avoid borrow checker churn inside complex
  scenarios.

#### Simple mutable world

```rust,no_run
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct CounterWorld {
    count: usize,
}

#[fixture]
fn world() -> CounterWorld {
    CounterWorld::default()
}

#[given("the world starts at {value}")]
fn seed(world: &mut CounterWorld, value: usize) {
    world.count = value;
}

#[when("the world increments")]
fn increment(world: &mut CounterWorld) {
    world.count += 1;
}

#[then("the world equals {expected}")]
fn check(world: &CounterWorld, expected: usize) {
    assert_eq!(world.count, expected);
}

#[scenario(path = "tests/features/mutable_world.feature", name = "Steps mutate shared state")]
fn mutable_world(world: CounterWorld) {
    assert_eq!(world.count, 3);
}
```

#### Slot‑based state (unchanged and still useful)

```rust,no_run
use rstest::fixture;
use rstest_bdd::{ScenarioState as _, Slot};
use rstest_bdd_macros::{given, scenario, then, when, ScenarioState};

#[derive(Default, ScenarioState)]
struct CartState {
    total: Slot<i32>,
}

#[fixture]
fn cart_state() -> CartState { CartState::default() }

#[when("I record {value:i32}")]
fn record(cart_state: &CartState, value: i32) { cart_state.total.set(value); }

#[then("the recorded value is {expected:i32}")]
fn check(cart_state: &CartState, expected: i32) {
    assert_eq!(cart_state.total.get(), Some(expected));
}

#[scenario(path = "tests/features/scenario_state.feature", name = "Recording a single value")]
fn keeps_value(cart_state: CartState) { let _ = cart_state; }
```

#### Mixed approach

```rust,no_run
#[derive(Default, ScenarioState)]
struct ReportWorld {
    total: usize,
    last_input: Slot<String>,
}

#[when("the total increases by {value:usize}")]
fn bump(world: &mut ReportWorld, value: usize) {
    world.total += value;
    world.last_input.set(format!("+{value}"));
}

#[then("the last input was recorded")]
fn last_input(world: &ReportWorld) {
    assert_eq!(world.last_input.get(), Some("+1".to_string()));
}
```

#### Best practices

- Keep world structs small and focused; extract helper methods when mutation
  requires validation or cross‑field consistency.
- Prefer immutable references in assertions to make read‑only intent obvious.
- Reserve `Slot<T>` for optional or resettable state; avoid mixing it in when a
  plain field would do.
- Add comments where mutation order matters between steps.

#### Troubleshooting

- A rustc internal compiler error (ICE) affected some nightly compilers when
  expanding macro‑driven scenarios with `&mut` fixtures. See
  `crates/rstest-bdd/tests/mutable_world_macro.rs` for a guarded example and
  `crates/rstest-bdd/tests/mutable_fixture.rs` for the context‑level regression
  test used until the upstream fix lands. Tracking details live in
  `docs/known-issues.md#rustc-ice-with-mutable-world-macro`.
- For advanced cases—custom fixture injection or manual borrowing—use
  `StepContext::insert_owned` and `StepContext::borrow_mut` directly; the
  examples above cover most scenarios.

### Step return values

`#[when]` steps may return a value. The scenario runner scans the available
fixtures for ones whose `TypeId` matches the returned value. When exactly one
fixture uses that type, the override is recorded under that fixture’s name and
subsequent steps receive the most recent value (last write wins). Ambiguous or
missing matches leave fixtures untouched, keeping scenarios predictable while
still allowing a functional style without mutable fixtures.

Steps may also return `Result<T, E>`. An `Err` aborts the scenario, while an
`Ok` value is injected as above. Type aliases to `Result` behave identically.
Returning `()` or `Ok(())` produces no stored value, so fixtures of `()` are
not overwritten.

```rust,no_run
use rstest::fixture;
use rstest_bdd_macros::{given, when, then, scenario};

#[fixture]
fn number() -> i32 { 1 }

#[when("it is incremented")]
fn increment(number: i32) -> i32 { number + 1 }

#[then("the result is 2")]
fn check(number: i32) { assert_eq!(number, 2); }

#[scenario(path = "tests/features/step_return.feature")]
fn returns_value(number: i32) { let _ = number; }
```

### Struct-based step arguments

When a step pattern contains several placeholders, the corresponding function
signatures quickly become unwieldy. Derive the `StepArgs` trait for a struct
whose fields mirror the placeholders and annotate the relevant parameter with
`#[step_args]` to request struct-based parsing. The attribute tells the macro
to consume every placeholder for that parameter, while fixtures and other
special arguments (`datatable`/`docstring`) continue to work as usual.

Fields must implement `FromStr`, and the derive macro enforces the bounds
automatically. Placeholders and struct fields must appear in the same order.
During expansion the macro inserts a compile-time check to ensure the field
count matches the pattern, producing a trait-bound error if the struct does not
implement `StepArgs`.

```rust,no_run
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when, StepArgs};

#[derive(StepArgs)]
struct CartInput {
    quantity: u32,
    item: String,
    price: f32,
}

#[derive(Default)]
struct Cart {
    quantity: u32,
    item: String,
    price: f32,
}

impl Cart {
    fn set(&mut self, details: &CartInput) {
        self.quantity = details.quantity;
        self.item = details.item.clone();
        self.price = details.price;
    }
}

#[fixture]
fn cart() -> Cart { Cart::default() }

#[given("a cart containing {quantity:u32} {item} at ${price:f32}")]
fn seed_cart(#[step_args] details: CartInput, cart: &mut Cart) {
    cart.set(&details);
}

#[then("the cart summary shows {quantity:u32} {item} at ${price:f32}")]
fn cart_summary(#[step_args] expected: CartInput, cart: &Cart) {
    assert_eq!(cart.quantity, expected.quantity);
    assert_eq!(cart.item, expected.item);
    assert!((cart.price - expected.price).abs() < f32::EPSILON);
}
```

`#[step_args]` must not be combined with `#[from]` and cannot target reference
types because the macro needs to own the struct to parse it. Attempting to use
multiple `#[step_args]` parameters in a single step yields a compile-time
error. The compiler also surfaces an error when the step pattern does not
declare any placeholders.

Example:

```rust,no_run
use rstest::fixture;
use rstest_bdd_macros::{given, when, then, scenario};

// A fixture used by multiple steps.
#[fixture]
fn basket() -> Basket {
    Basket::new()
}

#[given("an empty basket")]
fn empty_basket(basket: &mut Basket) {
    basket.clear();
}

#[when("the user adds a pumpkin")]
fn add_pumpkin(basket: &mut Basket) {
    basket.add(Item::Pumpkin, 1);
}

#[then("the basket contains one pumpkin")]
fn assert_pumpkins(basket: &Basket) {
    assert_eq!(basket.count(Item::Pumpkin), 1);
}

#[scenario(path = "tests/features/shopping.feature")]
fn test_add_to_basket(#[with(basket)] _: Basket) {
    // optional assertions after the steps
}
```

### Scenario state slots

Complex scenarios often need to capture intermediate results or share mutable
state between multiple steps. Historically, this required wrapping every field
in `RefCell<Option<T>>`, introducing noise and risking forgotten resets. The
`rstest-bdd` runtime now provides `Slot<T>`, a thin wrapper that exposes a
focused API for populating, reading, and clearing per-scenario values. Each
slot starts empty and supports helpers such as `set`, `replace`,
`get_or_insert_with`, `take`, and predicates `is_empty`/`is_filled`.

Define a state struct whose fields are `Slot<T>` and derive [`ScenarioState`].
The derive macro clears every slot by implementing `ScenarioState::reset` and
it automatically adds a [`Default`] implementation that leaves all slots empty.
**Do not** also derive or implement `Default`: Rust will report a
duplicate-implementation error because the macro already provides it. For
custom initialization, plan to use the future `#[scenario_state(no_default)]`
flag (or equivalent) to opt out of the generated `Default` and supply bespoke
logic.

```rust,no_run
use rstest::fixture;
use rstest_bdd::{ScenarioState, Slot};
use rstest_bdd_macros::{given, scenario, then, when, ScenarioState};

#[derive(ScenarioState)]
struct CliState {
    output: Slot<String>,
    exit_code: Slot<i32>,
}

#[fixture]
fn cli_state() -> CliState {
    CliState::default()
}

#[when("I run the CLI with {word}")]
fn run_command(cli_state: &CliState, argument: String) {
    let result = format!("ran {argument}");
    cli_state.output.set(result);
    cli_state.exit_code.set(0);
}

#[then("the CLI succeeded")]
fn cli_succeeded(cli_state: &CliState) {
    assert_eq!(cli_state.exit_code.get(), Some(0));
}

#[then("I can reset the state")]
fn reset_state(cli_state: &CliState) {
    cli_state.reset();
    assert!(cli_state.output.is_empty());
}

#[scenario(path = "tests/features/cli.feature")]
fn cli_behaviour(cli_state: CliState) {
    let _ = cli_state;
}
```

Slots are independent, so scenarios can freely mix eager `set` calls with lazy
`get_or_insert_with` initializers. Because slots live inside a regular fixture,
the state benefits from `rstest`’s usual lifecycle: a fresh struct is produced
for each scenario invocation, yet it remains trivial to clear and reuse the
contents when chaining multiple behaviours inside a single test body.

### Implicit fixture injection

Implicit fixtures such as `basket` must already be in scope in the test module;
`#[from(name)]` only renames a fixture and does not create one.

In this example, the step texts in the annotations must match the feature file
verbatim. The `#[scenario]` macro binds the test function to the first scenario
in the specified feature file and runs all registered steps before executing
the body of `test_add_to_basket`.

### Inferred step patterns

Step macros may omit the pattern string or provide a string literal containing
only whitespace. In either case, the macro derives a pattern from the function
name by replacing underscores with spaces.

```rust,no_run
use rstest_bdd_macros::given;

#[given]
fn user_logs_in() {
    // pattern "user logs in" is inferred
}
```

This reduces duplication between function names and patterns. A literal `""`
registers an empty pattern instead of inferring one.

> Note
> Inference preserves spaces derived from underscores:
>
> - Leading and trailing underscores become leading or trailing spaces.
> - Consecutive underscores become multiple spaces.
> - Letter case is preserved.

## Binding tests to scenarios

The `#[scenario]` macro is the entry point that ties a Rust test function to a
scenario defined in a `.feature` file. It accepts four arguments:

| Argument       | Purpose                                               | Status                                                                                    |
| -------------- | ----------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `path: &str`   | Relative path to the feature file (required).         | **Implemented**: resolved and parsed at compile time.                                     |
| `index: usize` | Optional zero-based scenario index (defaults to `0`). | **Implemented**: selects the scenario by position.                                        |
| `name: &str`   | Optional scenario title; resolves when unique.        | **Implemented**: errors when missing and directs duplicates to `index`.                   |
| `tags: &str`   | Optional tag-expression filter applied at expansion.  | **Implemented**: filters scenarios and outline example rows; errors when nothing matches. |

Tag filters run at macro-expansion time against the union of tags on the
feature, the matched scenario, and—when dealing with `Scenario Outline`—the
`Examples:` block that produced each generated row. Expressions support the
operators `and`, `or`, and `not` (case-insensitive) and respect the precedence
`not` > `and` > `or`; parentheses may be used to override this ordering. Tags
include the leading `@`. When the filter is combined with `index` or `name`,
the macro emits a compile error if the selected scenario does not satisfy the
expression.

```rust,no_run
# use rstest_bdd_macros::scenario;
#[scenario(path = "features/search.feature", tags = "@fast and not @wip")]
fn smoke_test() {}
```

If the feature file cannot be found or contains invalid Gherkin, the macro
emits a compile-time error with the offending path.

When `name` is provided, the macro matches the title case-sensitively. A
missing title triggers a diagnostic listing the available headings in the
feature. Duplicate titles yield a diagnostic that highlights the conflict and
lists the matching indexes and line numbers, enabling selection by the `index`
argument when required.

During macro expansion, the feature file is read and parsed. The macro
generates a new test function annotated with `#[rstest::rstest]` that performs
the following steps:

1. Build a `StepContext` and insert the test’s fixture arguments into it.

2. For each step in the scenario (according to the `Given‑When‑Then` sequence),
   look up a matching step function by `(keyword, pattern)` in the registry. A
   missing step causes the macro to emit a compile‑time error such as
   `No matching step definition found for: Given an undefined step`, allowing
   detection of incomplete implementations before tests run. Multiple matching
   definitions likewise produce an error.

3. Invoke the registered step function with the `StepContext` so that fixtures
   are available inside the step.

4. After executing all steps, run the original test body. This block can
   include extra assertions or cleanup logic beyond the behaviour described in
   the feature.

Because the generated code uses `#[rstest::rstest]`, it integrates seamlessly
with `rstest` features such as parameterized tests and asynchronous fixtures.
Tests are still discovered and executed by the standard Rust test runner, so
one may filter or run them in parallel as usual.

### Skipping scenarios

Steps or hooks may call `rstest_bdd::skip!` to stop executing the remaining
steps. The macro records a `Skipped` outcome and short-circuits the scenario so
the generated test returns before evaluating the annotated function body.
Invoke `skip!()` with no arguments to record a skipped outcome without a
message. Pass an optional string to describe the reason, and use the standard
`format!` syntax to interpolate values when needed. Set the
`RSTEST_BDD_FAIL_ON_SKIPPED` environment variable to `1`, or call
`rstest_bdd::config::set_fail_on_skipped(true)`, to escalate skipped scenarios
into test failures unless the feature or scenario carries an `@allow_skipped`
tag. (Example-level tags are not yet evaluated.)

The macro captures the current execution scope internally, so helper functions
may freely call `skip!` as long as they eventually run within a step or hook.
When code outside that context—for example, a unit test or module
initialization routine—invokes the macro, it panics with the message
`rstest_bdd::skip! may only be used inside a step or hook generated by rstest-bdd`.
 Each scope tracks the thread that entered it; issuing a skip from another
thread panics with
`rstest_bdd::skip! may only run on the thread executing the step ...`. Keep
skip requests on the thread that executes steps, so the runner can intercept
the panic payload.

```rust,no_run
# use rstest_bdd as bdd;
# use rstest_bdd_macros::{given, scenario};

#[given("a dependent service is unavailable")]
fn service_unavailable() {
    bdd::skip!("service still provisioning");
}

#[given("a maintenance window is scheduled")]
fn maintenance_window() {
    let component = "billing";
    bdd::skip!(
        "{component} maintenance in progress",
        component = component,
    );
}

#[scenario(path = "features/unhappy_path.feature")]
fn resilience_test() {
    panic!("scenario body is skipped");
}
```

When `fail_on_skipped` is enabled, apply `@allow_skipped` to the feature or the
scenario to document that the skip is expected:

```gherkin
@allow_skipped
Scenario: Behaviour pending external contract
  Given a dependent service is unavailable
```

### Asserting skipped outcomes

Tests that exercise skip-heavy flows no longer need to match on enums to verify
that a step or scenario stopped executing. Use
`rstest_bdd::assert_step_skipped!` to unwrap a `StepExecution::Skipped`
outcome, optionally constraining its message, and
`rstest_bdd::assert_scenario_skipped!` to inspect
[`ScenarioStatus`](crate::reporting::ScenarioStatus) records. Both macros
accept `message_absent = true` to assert that no message was provided and
substring matching to confirm that a message contains the expected reason.

```rust,no_run
use rstest_bdd::{assert_scenario_skipped, assert_step_skipped, StepExecution};
use rstest_bdd::reporting::{ScenarioMetadata, ScenarioRecord, ScenarioStatus, SkippedScenario};

let outcome = StepExecution::skipped(Some("maintenance pending".into()));
let message = assert_step_skipped!(outcome, message = "maintenance");
assert_eq!(message, Some("maintenance pending".into()));

let metadata = ScenarioMetadata::new(
    "features/unhappy.feature",
    "pending work",
    12,
    vec!["@allow_skipped".into()],
);
let record = ScenarioRecord::from_metadata(
    metadata,
    ScenarioStatus::Skipped(SkippedScenario::new(None, true, false)),
);
let details = assert_scenario_skipped!(
    record.status(),
    message_absent = true,
    allow_skipped = true,
    forced_failure = false,
);
assert!(details.allow_skipped());
```

## Autodiscovering scenarios

For large suites, it is tedious to bind each scenario manually. The
`scenarios!` macro scans a directory recursively for `.feature` files and
generates a module with a test for every `Scenario` found. Each test is named
after the feature file and scenario title. Identifiers are sanitized
(ASCII-only) and deduplicated by appending a numeric suffix when collisions
occur.

```rust,no_run
use rstest_bdd_macros::{given, then, when, scenarios};

#[given("a precondition")] fn precondition() {}
#[when("an action occurs")] fn action() {}
#[then("events are recorded")] fn events() {}

scenarios!("tests/features/auto");

// Only expand scenarios tagged @smoke and not marked @wip
scenarios!("tests/features/auto", tags = "@smoke and not @wip");
```

When `tags` is supplied the macro evaluates the expression against the same
union of feature, scenario, and example tags described above. Scenarios that do
not match simply do not generate a test, and outline examples drop unmatched
rows.

Generated tests cannot currently accept fixtures; use `#[scenario]` when
fixture injection or custom assertions are required.

## Running and maintaining tests

Once feature files and step definitions are in place, scenarios run via the
usual `cargo test` command. Test functions created by the `#[scenario]` macro
behave like other `rstest` tests; they honour `#[tokio::test]` or
`#[async_std::test]` attributes if applied to the original function. Each
scenario runs its steps sequentially in the order defined in the feature file.
By default, missing steps emit a compile‑time warning and are checked again at
runtime, so steps can live in other crates. Enabling the
`compile-time-validation` feature on `rstest-bdd-macros` registers steps and
performs compile‑time validation, emitting warnings for any that are missing.
The `strict-compile-time-validation` feature builds on this and turns those
warnings into `compile_error!`s when all step definitions are local. This
prevents behaviour specifications from silently drifting from the code while
still permitting cross‑crate step sharing.

To enable validation, pin a feature in the project's `dev-dependencies`:

```toml
[dev-dependencies]
rstest-bdd-macros = { version = "0.2.0", features = ["compile-time-validation"] }
```

For strict checking use:

```toml
[dev-dependencies]
rstest-bdd-macros = { version = "0.2.0", features = ["strict-compile-time-validation"] }
```

Steps are only validated when one of these features is enabled.

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
  `Infinity` (matched case-insensitively). Matching is anchored: the entire
  step text must match the pattern; partial matches do not succeed. Escape
  literal braces with `{{` and `}}`. Use
  `\` to match a single backslash. A trailing `\` or any other backslash escape
  is treated literally, so `\d` matches the two-character sequence `\d`. Nested
  braces inside placeholders are not supported. Braces are not allowed inside
  type hints. Placeholders use `{name}` or `{name:type}`; the type hint must
  not contain braces (for example, `{n:{u32}}` and `{n:Vec<{u32}>}` are
  rejected). To describe braces in the surrounding step text (for example,
  referring to `{u32}`), escape them as `{{` and `}}` rather than placing them
  inside `{name:type}`. The lexer closes the placeholder at the first `}` after
  the optional type hint; any characters between the `:type` and that first `}`
  are ignored (for example, `{n:u32 extra}` parses as `name = n`,
  `type = u32`). `name` must start with a letter or underscore and may contain
  letters, digits, or underscores (`[A-Za-z_][A-Za-z0-9_]*`). Whitespace within
  the type hint is ignored (for example, `{count: u32}` and `{count:u32}` are
  both accepted), but whitespace is not allowed between the name and the colon.
  Prefer the compact form `{count:u32}` in new code. When a pattern contains no
  placeholders, the step text must match exactly. Unknown type hints are
  treated as generic placeholders and capture any non-newline text using a
  non-greedy match.

## Data tables and Docstrings

Steps may supply structured or free-form data via a trailing argument. A data
table is received by including a parameter annotated with `#[datatable]` or
named `datatable`. The declared type must implement
`TryFrom<Vec<Vec<String>>>`, so the wrapper can convert the parsed cells.
Existing code can continue to accept a raw `Vec<Vec<String>>`, while the
`rstest_bdd::datatable` module offers strongly typed helpers.

`datatable::Rows<T>` wraps a vector of parsed rows and derives
`Deref<Target = [T]>`, `From<Vec<T>>`, and `IntoIterator`, enabling direct
consumption of the table. The `datatable::DataTableRow` trait describes how
each row should be interpreted for a given type. When `T::REQUIRES_HEADER` is
`true`, the first table row is treated as a header and exposed via the
`HeaderSpec` helpers.

```rust,no_run
use rstest_bdd::datatable::{
    self, DataTableError, DataTableRow, RowSpec, Rows,
};
# use rstest_bdd_macros::given;

#[derive(Debug, PartialEq, Eq)]
struct UserRow {
    name: String,
    email: String,
    active: bool,
}

impl DataTableRow for UserRow {
    const REQUIRES_HEADER: bool = true;

    fn parse_row(mut row: RowSpec<'_>) -> Result<Self, DataTableError> {
        let name = row.take_column("name")?;
        let email = row.take_column("email")?;
        let active = row.parse_column_with(
            "active",
            datatable::truthy_bool,
        )?;
        Ok(Self { name, email, active })
    }
}

#[given("the following users exist:")]
fn users_exist(#[datatable] rows: Rows<UserRow>) {
    for row in rows {
        assert!(row.active || row.name == "Bob");
    }
}
```

Projects that prefer to work with raw rows can declare the argument as
`Vec<Vec<String>>` and handle parsing manually. Both forms can co-exist within
the same project, allowing incremental adoption of typed tables.

### Performance and caching

Data tables are now converted once per distinct table literal and cached for
reuse. The generated wrappers key the cache by the table content, so repeated
executions of the same step with the same table reuse the stored
`Vec<Vec<String>>` without re-parsing cell text. The cache is scoped to the
step wrapper, preventing different steps or tables from sharing entries.

For zero-allocation access on subsequent executions, prefer the
`datatable::CachedTable` argument type. It borrows the cached rows via an
`Arc`, avoiding fresh string allocations when the step runs multiple times.
Existing `Vec<Vec<String>>` signatures remain supported; they clone the cached
rows when binding the argument, which still avoids re-parsing but retains the
ownership semantics of the older API. The cache lives for the lifetime of the
test process, so large tables remain available to later scenarios without
additional conversion overhead.

A derive macro removes the boilerplate when mapping headers to fields. Annotate
the struct with `#[derive(DataTableRow)]` and customize behaviour via field
attributes:

- `#[datatable(rename_all = "kebab-case")]` applies a casing rule to unnamed
  fields.
- `#[datatable(column = "Email address")]` targets a specific header.
- `#[datatable(optional)]` treats missing cells as `None`.
- `#[datatable(default)]` or `#[datatable(default = path::to_fn)]` supplies a
  fallback when the column is absent.
- `#[datatable(trim)]` trims whitespace before parsing.
- `#[datatable(truthy)]` parses tolerant booleans using
  `datatable::truthy_bool`.
- `#[datatable(parse_with = path::to_fn)]` calls a custom parser that returns a
  `Result<T, E>`.

```rust,no_run
use rstest_bdd::datatable::{self, DataTableError, Rows};
use rstest_bdd_macros::DataTableRow;

fn default_region() -> String { String::from("EMEA") }

fn parse_age(value: &str) -> Result<u8, std::num::ParseIntError> {
    value.trim().parse()
}

#[derive(Debug, PartialEq, Eq, DataTableRow)]
#[datatable(rename_all = "kebab-case")]
struct UserRow {
    given_name: String,
    #[datatable(column = "email address")]
    email: String,
    #[datatable(truthy)]
    active: bool,
    #[datatable(optional)]
    nickname: Option<String>,
    #[datatable(default = default_region)]
    region: String,
    #[datatable(parse_with = parse_age)]
    age: u8,
}

fn load_users(rows: Rows<UserRow>) -> Result<Vec<UserRow>, DataTableError> {
    Ok(rows.into_vec())
}
```

`#[derive(DataTableRow)]` enforces these attributes at compile time. Optional
fields must use `Option<T>`; applying `#[datatable(optional)]` to any other
type triggers an error. Optional fields also cannot declare defaults because
those behaviours are contradictory. Likewise, `#[datatable(truthy)]` and
`#[datatable(parse_with = …)]` are mutually exclusive, ensuring the macro can
select a single parsing strategy.

Tuple structs that wrap collections can derive `DataTable` to implement
`TryFrom<Vec<Vec<String>>>`. The macro accepts optional hooks to post-process
rows before exposing them to the step function:

- `#[datatable(map = path::to_fn)]` transforms the parsed rows.
- `#[datatable(try_map = path::to_fn)]` performs fallible conversion, returning
  a `DataTableError` on failure.

```rust,no_run
use rstest_bdd::datatable::{DataTableError, Rows};
use rstest_bdd_macros::{DataTable, DataTableRow};

#[derive(Debug, PartialEq, Eq, DataTableRow)]
struct UserRow {
    name: String,
    #[datatable(truthy)]
    active: bool,
}

#[derive(Debug, PartialEq, Eq, DataTable)]
#[datatable(row = UserRow, try_map = collect_active)] // Converts rows into a bespoke type.
struct ActiveUsers(Vec<String>);

fn collect_active(rows: Rows<UserRow>) -> Result<Vec<String>, DataTableError> {
    Ok(rows
        .into_iter()
        .filter(|row| row.active)
        .map(|row| row.name)
        .collect())
}
```

### Debugging data table errors

`Rows<T>` propagates [`DataTableError`] variants unchanged, making it easy to
surface context when something goes wrong. Matching on the error value enables
inspection of the row and column that triggered the failure:

```rust,no_run
# use rstest_bdd::datatable::{DataTableError, Rows};
# use rstest_bdd_macros::DataTableRow;
#
# #[derive(Debug, PartialEq, Eq, DataTableRow)]
# struct UserRow {
#     name: String,
#     #[datatable(truthy)]
#     active: bool,
# }

let table = vec![
    vec!["name".into(), "active".into()],
    vec!["Alice".into()],
];

let Err(DataTableError::MissingColumn { row_number, column }) =
    Rows::<UserRow>::try_from(table)
else {
    panic!("expected the table to be missing the 'active' column");
};

assert_eq!(row_number, 2);
assert_eq!(column, "active");
```

Custom parsers bubble their source error through `DataTableError::CellParse`.
Inspecting the formatted message shows the precise location of the failure,
including the human-readable column label:

```rust,no_run
# use rstest_bdd::datatable::{DataTableError, Rows};
# use rstest_bdd_macros::DataTableRow;
#
# #[derive(Debug, PartialEq, Eq, DataTableRow)]
# struct UserRow {
#     name: String,
#     #[datatable(truthy)]
#     active: bool,
# }

let result = Rows::<UserRow>::try_from(vec![
    vec!["name".into(), "active".into()],
    vec!["Alice".into(), "maybe".into()],
]);

let err = match result {
    Err(err) => err,
    Ok(_) => panic!("expected the 'maybe' flag to trigger a parse error"),
};

let DataTableError::CellParse {
    row_number,
    column_index,
    ..
} = err
else {
    panic!("unexpected error variant");
};
assert_eq!(row_number, 2);
assert_eq!(column_index, 2);
assert!(err
    .to_string()
    .contains("unrecognised boolean value 'maybe'"));
```

[`DataTableError`]: crate::datatable::DataTableError

A Gherkin Docstring is available through an argument named `docstring` of type
`String`. Both arguments must use these exact names and types to be detected by
the procedural macros. When both are declared, place `datatable` before
`docstring` at the end of the parameter list.

```gherkin
Scenario: capture table and docstring
  Given the following numbers:
    | a | b |
    | 1 | 2 |
  When I submit:
    """
    payload
    """
```

```rust,no_run
#[given("the following numbers:")]
fn capture_table(datatable: Vec<Vec<String>>) {
    // ...
}

#[when("I submit:")]
fn capture_docstring(docstring: String) {
    // ...
}

#[then("table and text:")]
fn capture_both(datatable: Vec<Vec<String>>, docstring: String) {
    // datatable must precede docstring
}
```

At runtime, the generated wrapper converts the table cells or copies the block
text and passes them to the step function. It panics if the step declares
`datatable` or `docstring` but the feature omits the content. Docstrings may be
delimited by triple double-quotes or triple backticks.

## Limitations and roadmap

The `rstest‑bdd` project is evolving. Several features described in the design
document and README remain unimplemented in the current codebase:

- **Wildcard keywords and** `*` **steps.** The asterisk (`*`) can replace any
  step keyword in Gherkin to improve readability, but step lookup is based
  strictly on the primary keyword. Using `*` in feature files will not match
  any registered step.

- **Restricted placeholder types.** Only placeholders that parse via
  `FromStr` are supported, and they must be well-formed and non-overlapping.

Consult the project’s roadmap or repository for updates. The addition of new
features may result in patterns or examples changing. Meanwhile, adopting
`rstest‑bdd` in its current form will be most effective when teams keep feature
files simple. Step definitions should remain explicit.

## Assertion macros

When step functions return `Result` values it is common to assert on their
outcome. The `rstest-bdd` crate exports two helper macros to streamline these
checks:

```rust,no_run
use rstest_bdd::{assert_step_err, assert_step_ok};

let ok: Result<(), &str> = Ok(());
assert_step_ok!(ok);

let err: Result<(), &str> = Err("boom");
let e = assert_step_err!(err, "boom");
assert_eq!(e, "boom");
```

`assert_step_ok!` unwraps an `Ok` value and panics with the error message when
the result is `Err`. `assert_step_err!` unwraps the error and optionally checks
that its display contains a substring. Both macros return their unwrapped
values, allowing further inspection when required.

## Internationalization and localization

### Writing feature files in other languages

Feature files can opt into any Gherkin localization. Add a `# language: <code>`
directive on the first line of the `.feature` file and keep the remainder of
the scenario in the target language:

```gherkin
# language: es
Característica: Control de stock
  Escenario: Añadir una calabaza
    Dado un inventario vacío
    Cuando registro una calabaza
    Entonces el inventario contiene una calabaza
```

The scenario parser reads the declaration and hands keyword matching to the
`gherkin` crate, so existing `#[given]`, `#[when]`, and `#[then]` definitions
continue to match without code changes. Omitting the directive keeps the
default English vocabulary to preserve backwards compatibility.

### Localizing runtime diagnostics

`rstest-bdd` now ships its user-facing diagnostics via Fluent translation
files. The crate bundles English strings by default and falls back to them when
no translation is available. Applications can opt into additional locales by
embedding the provided assets and selecting a language at runtime.

Localization tooling can be added to `Cargo.toml` as follows:

```toml
[dependencies]
rstest-bdd = "0.2.0"
i18n-embed = { version = "0.16", features = ["fluent-system", "desktop-requester"] }
unic-langid = "0.9"
```

The crate exposes the embedded assets via the [`Localizations`] helper. This
type implements `i18n_embed::I18nAssets`, allowing applications with existing
Fluent infrastructure to load resources into their own
[`FluentLanguageLoader`]. Libraries without a localization framework can rely
on the built-in loader and request a different language at runtime:

```rust,no_run
# fn scope_locale() -> Result<(), rstest_bdd::localization::LocalizationError> {
use rstest_bdd::select_localizations;
use unic_langid::langid;

select_localizations(&[langid!("fr")])?; // Switch diagnostics to French
# Ok(())
# }
```

The selection function preserves the caller-supplied order, so applications can
pass a list of preferred locales. The helper resolves to the best available
translation and continues to fall back to English when a requested locale is
not shipped with the crate. Procedural macro diagnostics remain in English so
compile-time output stays deterministic regardless of the host machine’s
language settings.

[`Localizations`]: https://docs.rs/rstest-bdd/latest/rstest_bdd/localization/
[`FluentLanguageLoader`]:
https://docs.rs/i18n-embed/latest/i18n_embed/fluent/struct.FluentLanguageLoader.html

## Diagnostic tooling

`rstest-bdd` bundles a small helper binary exposed as the cargo subcommand
`cargo bdd`.

Synopsis

- `cargo bdd steps`
- `cargo bdd steps --skipped`
- `cargo bdd unused`
- `cargo bdd duplicates`
- `cargo bdd skipped`

Examples

- `cargo bdd steps`
- `cargo bdd steps --skipped --json`
- `cargo bdd unused --quiet`
- `cargo bdd duplicates --json`
- `cargo bdd skipped --reasons`
- `cargo bdd steps --skipped --json` must be paired; using `--json` without
  `--skipped` is rejected by the CLI so invalid combinations fail fast.

The tool inspects the runtime step registry and offers four commands:

- `cargo bdd steps` prints every registered step with its source location and
  appends any skipped scenario outcomes using lowercase status labels whilst
  preserving long messages.
- `cargo bdd steps --skipped` limits the listing to step definitions that were
  bypassed after a scenario requested a skip, preserving the scenario context.
- `cargo bdd unused` lists steps that were never executed in the current
  process.
- `cargo bdd duplicates` groups step definitions that share the same keyword
  and pattern, helping to identify accidental copies.
- `cargo bdd skipped` lists skipped scenarios and supports `--reasons` to show
  file and line numbers alongside the explanatory message.

The subcommand builds each test target in the workspace and runs the resulting
binary with `RSTEST_BDD_DUMP_STEPS=1` and a private `--dump-steps` flag to
collect the registered steps and recently executed scenario outcomes as JSON.
Because usage tracking is process local, `unused` only reflects steps invoked
during that same execution. The merged output powers the commands above and the
skip status summary, helping to keep the step library tidy and discover dead
code early in the development cycle.

`steps --skipped` and `skipped` accept `--json` and emit objects that always
include `feature`, `scenario`, `line`, `tags`, and `reason` fields. The former
adds an embedded `step` object describing each bypassed definition (keyword,
pattern, file, and line) to help trace which definitions were sidelined by a
runtime skip.

### Scenario report writers

Projects that need to persist scenario results outside the CLI can rely on the
runtime reporting modules. `rstest_bdd::reporting::json` offers helper
functions that serialize the collector snapshot into a predictable schema with
lowercase status labels:

```rust,no_run
let mut buffer = Vec::new();
rstest_bdd::reporting::json::write_snapshot(&mut buffer)?;
```

The companion `rstest_bdd::reporting::junit` module renders the same snapshot
as JUnit XML. Each skipped scenario emits a `<skipped>` element with an
optional `message` attribute so continuous integration (CI) servers surface the
reason:

```rust,no_run
let mut xml = String::new();
rstest_bdd::reporting::junit::write_snapshot(&mut xml)?;
```

Both writers accept explicit `&[ScenarioRecord]` slices when callers want to
serialize a custom selection of outcomes rather than the full snapshot.

## Language server

The `rstest-bdd-server` crate provides a Language Server Protocol (LSP)
implementation that bridges Gherkin `.feature` files and Rust step definitions.
The binary is named `rstest-bdd-lsp` and communicates over stdin/stdout using
JSON-RPC, making it compatible with any editor supporting the LSP (VS Code,
Neovim, Zed, Helix, etc.).

### Installation

Build and install the language server from the workspace:

```bash
cargo install --path crates/rstest-bdd-server
```

The binary `rstest-bdd-lsp` is placed in the Cargo bin directory.

### Configuration

The server reads configuration from environment variables:

| Variable                     | Description                                         | Default |
| ---------------------------- | --------------------------------------------------- | ------- |
| `RSTEST_BDD_LSP_LOG_LEVEL`   | Logging verbosity (trace, debug, info, warn, error) | `info`  |
| `RSTEST_BDD_LSP_DEBOUNCE_MS` | Delay (ms) before processing file changes           | `300`   |

Example:

```bash
RSTEST_BDD_LSP_LOG_LEVEL=debug rstest-bdd-lsp
```

### Editor integration

#### VS Code

Add a configuration in the `settings.json` file or use an extension that allows
custom LSP servers. A minimal example using the
[LSP-client](https://marketplace.visualstudio.com/items?itemName=ACharLuk.easy-lsp-client)
 extension:

```json
{
  "easylsp.servers": [
    {
      "language": ["rust", "gherkin"],
      "command": "rstest-bdd-lsp"
    }
  ]
}
```

#### Neovim (nvim-lspconfig)

```lua
local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

if not configs.rstest_bdd then
  configs.rstest_bdd = {
    default_config = {
      cmd = { 'rstest-bdd-lsp' },
      filetypes = { 'rust', 'cucumber' },
      root_dir = lspconfig.util.root_pattern('Cargo.toml'),
    },
  }
end

lspconfig.rstest_bdd.setup({})
```

### Current capabilities

The initial release provides the LSP scaffolding:

- **Lifecycle handlers**: Responds to `initialize`, `initialized`, and
  `shutdown` requests per the LSP specification.
- **Workspace discovery**: Uses `cargo metadata` to locate the workspace root
  and enumerate packages.
- **Structured logging**: Configurable via environment variables; logs are
  written to stderr using the `tracing` framework.

Future releases will add indexing, navigation (go-to-definition,
go-to-implementation), and diagnostics as outlined in the
[Language Server Design Document](rstest-bdd-language-server-design.md).

## Summary

`rstest‑bdd` seeks to bring the collaborative clarity of Behaviour‑Driven
Development to Rust without sacrificing the ergonomics of `rstest` and the
convenience of `cargo test`. In its present form, the framework provides a core
workflow: write Gherkin scenarios, implement matching Rust functions with
`#[given]`, `#[when]` and `#[then]` annotations, rely on matching parameter
names for fixture injection (use `#[from]` when renaming), and bind tests to
scenarios with `#[scenario]`. Step definitions are discovered at link time via
the `inventory` crate, and scenarios execute all steps in sequence before
running any remaining test code. While advanced Gherkin constructs and
parameterization remain on the horizon, this foundation allows teams to
integrate acceptance criteria into their Rust test suites and to engage all
three amigos in the specification process.
