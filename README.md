# rstest-bdd

*Where Rustaceans come for their gourd‑related puns.*

> **TL;DR**: Behaviour‑Driven Development, idiomatic to Rust. Keep your unit
> tests and your acceptance tests on the same vine, run everything with
> `cargo test`, and reuse your `rstest` fixtures.

______________________________________________________________________

## Why this crate?

`rstest-bdd` brings the collaborative clarity of BDD to Rust **without** asking
you to adopt a bespoke runner or a monolithic “world” object. Instead, it
builds on the excellent `rstest` fixture and parametrisation model:

- **One runner to rule them all**: execute scenarios with `cargo test`.

- **First‑class fixtures**: share `rstest` fixtures between unit, integration,
  and BDD tests.

- **Ergonomic step definitions**: `#[given]`, `#[when]`, `#[then]` with typed
  placeholders.

- **Feature parity**: Scenario Outlines, Background, data tables, and
  docstrings.

- **Pytest‑bdd vibes**: explicit `#[scenario]` binding from test code to a
  named scenario.

  The attribute now requires a `path` argument pointing to the `.feature` file;
  index-only usage is no longer supported.

Think of it as *courgette‑driven* development: crisp, versatile, and it plays
nicely with everything else on your plate.

______________________________________________________________________

## Installation

Add the crates to your **dev‑dependencies**:

```toml
# Cargo.toml
[dev-dependencies]
rstest = "0.25"
rstest-bdd = "0.1.0-alpha2"
```

Feature flags:

- `tokio` / `async-std` — choose your async test attribute (`#[tokio::test]`,
  etc.).

- `no-inventory` — fallback code‑gen registry for platforms where
  linker‑section collection is unwieldy.

- `compile-time-validation` — registers steps at compile time and reports
  missing or ambiguous steps with spans.

- `strict-compile-time-validation` — fails compilation when steps are missing
  or ambiguous; implies `compile-time-validation`.

Both features are disabled by default and apply only to the `rstest-bdd-macros`
crate. Enable them in your `Cargo.toml` with:

```toml
[dependencies]
rstest-bdd-macros = { version = "0.1.0-alpha2", features = ["compile-time-validation"] }
```

______________________________________________________________________

## Quick start (end‑to‑end “Web Search”)

**Feature file**: `tests/features/web_search.feature`

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

**Step definitions**: `tests/test_web_search.rs`

```rust
use rstest::fixture;
use rstest_bdd::{scenario, given, when, then};
// Browser automation example — pick your favourite WebDriver crate.
use thirtyfour::prelude::*;

#[fixture]
async fn browser() -> WebDriverResult<WebDriver> {
    let caps = DesiredCapabilities::firefox();
    let driver = WebDriver::new("http://localhost:4444", caps).await?;
    Ok(driver)
}

// Bind this test to the named scenario from the feature file.
// The test body runs *after* all steps have passed.
#[scenario(path = "tests/features/web_search.feature", name = "Simple web search")]
#[tokio::test]
async fn test_simple_search(#[future] browser: WebDriver) {
    // Optional: final assertions / cleanup that aren't natural Gherkin steps.
}

#[given("the DuckDuckGo home page is displayed")]
async fn go_to_home(#[from(browser)] driver: &mut WebDriver) {
    driver.goto("https://duckduckgo.com/").await.unwrap();
}

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

Run it:

```bash
cargo test -p your-crate -- --nocapture
```

Everything grows on the same trellis: your fixtures, your filters
(`cargo test search`), and your parallelism all continue to work as usual.

______________________________________________________________________

## Step definitions 101

Decorate plain Rust functions:

```rust
use rstest_bdd::{given, when, then};

#[given("an empty basket")]
fn empty_basket(#[from(basket)] b: &mut Basket) {
    b.clear();
}

#[when("I add {count:u32} pumpkins")]
fn add_pumpkins(#[from(basket)] b: &mut Basket, count: u32) {
    b.add(Item::Pumpkin, count);
}

#[then("the basket has {count:u32} pumpkins")]
fn assert_count(#[from(basket)] b: &Basket, count: u32) {
    assert_eq!(b.count(Item::Pumpkin), count);
}
```

- Patterns accept **typed placeholders** like `{count:u32}`; values parse via
  `FromStr`.

- Use `#[from(fixture_name)]` to inject any `rstest` fixture into a step.

- Prefer readable step text first; compile‑time checks ensure you don’t forget
  an implementation.

______________________________________________________________________

## Scenario Outline ≈ parametrised tests

Write once, test many:

**Feature**:

```gherkin
Scenario Outline: Login with different credentials
  Given I am on the login page
  When I enter username "<username>" and password "<password>"
  Then I should see the message "<message>"

  Examples:
    | username | password   | message                 |
    | user     | correctpass| Welcome, user!         |
    | user     | wrongpass  | Invalid credentials     |
    | admin    | adminpass  | Welcome, administrator! |
```

**Test**:

```rust
#[scenario(path = "tests/features/login.feature", name = "Login with different credentials")]
#[tokio::test]
async fn test_login_scenarios(#[future] browser: WebDriver) {}

// Placeholders from <angle brackets> arrive as typed arguments.
#[when("I enter username \"<username>\" and password \"<password>\"")]
async fn enter_credentials(
    #[from(browser)] driver: &mut WebDriver,
    username: String,
    password: String,
) {
    // ...
}

#[then("I should see the message \"<message>\"")]
async fn see_message(#[from(browser)] driver: &mut WebDriver, message: String) {
    // ...
}
```

Under the rind, `#[scenario]` expands to an `rstest` parametrised test, so
cases show up individually in your runner.

______________________________________________________________________

## Background, tables, and docstrings

- **Background** runs before every scenario in the feature.
- **Data tables** arrive in a `datatable` parameter of type
  `Vec<Vec<String>>`.

- **Docstrings** arrive as a `String`.

```rust
#[given("the following users exist:")]
fn create_users(
    #[from(db)] conn: &mut DbConnection,
    datatable: Vec<Vec<String>>,
) {
    // Assume the first row is a header: ["name", "email", ...]
    for row in datatable.into_iter().skip(1) {
        assert!(
            row.len() >= 2,
            "Expected at least two columns: name and email",
        );
        let name = &row[0];
        let email = &row[1];
        conn.insert_user(name, email);
    }
}
```

______________________________________________________________________

## How it works (the short tour)

- **Attribute macros**:

  - `#[given] / #[when] / #[then]` register step metadata.

  - `#[scenario]` binds a test function to a named scenario in a `.feature`
    file.

- **Discovery**: Steps are registered at compile time into a global registry
  (via linker‑section collection). At runtime, the generated test matches each
  Gherkin line against that registry and invokes the correct function.

- **Safety rails**: If a step in the feature has no matching implementation,
  you get a **compile error** with a helpful message, not a late test failure.

- **Fixtures**: Because the generated test is an `rstest`, your fixture
  dependency graph Just Works™.

If your target platform dislikes linker sections, enable the `no-inventory`
feature to switch to a build‑script registry.

______________________________________________________________________

## Design principles

- **Ecosystem, not empire**: reuse `rstest` instead of replacing it.

- **Readable first**: human‑centric step text, type‑safe argument parsing.

- **Fail fast**: resolve missing steps at compile time.

- **Zero new runners**: keep CI/CD and IDE behaviour unchanged.

______________________________________________________________________

## Limitations

- `.feature` files are processed at compile time; scenarios are static.

- Step definitions must be known at compile time (no dynamic registration).

- IDE navigation from Gherkin to Rust may require tooling support.

- Registry implementation relies on platform features; use `no-inventory` if
  needed.

______________________________________________________________________

## [Roadmap](docs/roadmap.md)

1. **Core mechanics**: step registry, `#[scenario]`, exact matching (done/PoC).

2. **Fixtures & parametrisation**: typed placeholders, Scenario Outline →
   `#[case]`.

3. **Feature parity & ergonomics**: Background, tables, docstrings,
   `scenarios!` macro, richer diagnostics.

4. **Developer tools**: `cargo bdd list-steps`, editor integrations.

We’re not here to replace `cucumber`; we’re here to offer a different trade‑off
for teams already invested in `rstest` and `cargo test`.

______________________________________________________________________

## Comparison at a glance

| Feature          | `rstest-bdd` (proposed)                     | `cucumber` (Rust)                 |
| ---------------- | ------------------------------------------- | --------------------------------- |
| Test runner      | `cargo test` (`rstest` under the hood)      | Custom runner (`World::run(...)`) |
| State management | `rstest` fixtures                           | `World` struct                    |
| Step discovery   | Compile‑time registration + runtime match   | Runner‑driven collection          |
| Scenario Outline | Maps to `rstest` parametrisation            | Built into runner                 |
| Async            | Runtime‑agnostic via features               | Built‑in with specified runtime   |
| Philosophy       | BDD as an **extension** of `rstest`         | Rust port of classic Cucumber     |

______________________________________________________________________

## Workspace layout

```text
rstest-bdd/             # Runtime crate (re-exports macros for convenience)
rstest-bdd-macros/      # Procedural macro crate
```

## Examples

An `examples` directory hosts standalone crates demonstrating `rstest-bdd`.
These crates are members of the repository's root workspace, so CI and
workspace commands include them. Each example can also be built and tested from
its directory. To build and test the `todo-cli` example:

```bash
cd examples/todo-cli
cargo test
# NOTE: The CLI stores tasks only in memory per invocation. Each `cargo run`
# starts with an empty list, so the 'list' command below will be empty.
cargo run -- add "Buy milk"   # adds in this process, then exits
cargo run -- list             # runs in a new process; prints an empty list
```

Dependencies for examples are captured in the repository's `Cargo.lock` to
ensure reproducible builds.

Note: `make nixie` renders Mermaid diagrams via `@mermaid-js/mermaid-cli`.
Ensure a supported runner is available (listed in preferred order):

- Bun: `bun x @mermaid-js/mermaid-cli`
- pnpm: `pnpm dlx @mermaid-js/mermaid-cli`
- Node.js: `npx --yes @mermaid-js/mermaid-cli`

If none is installed, install one and re-run `make nixie`.

______________________________________________________________________

## Prior art & acknowledgements

- Inspired by the ergonomics of **pytest‑bdd** and the fixture model of
  **rstest**.

- Uses a global step registry pattern popularised in Rust by the **inventory**
  crate.

- Tips the hat to **cucumber‑rs** for bringing Cucumber’s ideas to Rust.

______________________________________________________________________

## Contributing

Issues, ideas, and PRs are very welcome. Please include:

- A minimal repro (feature file + steps) when filing bugs.

- Before/after compiler output if you hit macro errors (the more precise, the
  better).

- Platform info if you suspect a registry/linker quirk.

Let’s **seed** a lovely ecosystem together.

______________________________________________________________________

## Licence

ISC Licence — because that’s how we roll. You’re free to use, copy, modify, and
distribute this software for any purpose, with or without fee, provided that
the copyright notice and this permission notice are included in all copies. The
software is provided “as is”, without warranty of any kind. See `LICENSE` for
the full text.

______________________________________________________________________

## Appendix: FAQ

**Does it work with stable Rust?**\
Yes; nothing here requires nightly.

**Can I mix BDD tests with unit tests in the same crate?**\
Absolutely. They run under the same `cargo test` umbrella and can share
fixtures.

**Will it slow my build?**\
There’s some compile‑time I/O to parse `.feature` files. For large suites,
caching parsed ASTs in `OUT_DIR` mitigates this (built in).

**Do I *have* to use regexes in step patterns?**\
No. Prefer typed placeholders like `{n:u32}`; fall back to regex groups only
when you really need them.

______________________________________________________________________

Happy testing — and may your scenarios be **gourd‑geous** and your failures
easy to **squash**. 🎃🧪🦀
