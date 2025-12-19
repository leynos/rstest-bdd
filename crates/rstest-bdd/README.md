# rstest-bdd

*Where Rustaceans come for their gourd‑related puns.*

> **TL;DR**: Behaviour‑Driven Development (BDD), idiomatic to Rust. Keep your
> unit tests and your acceptance tests on the same vine, run everything with
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

  Migration (since 0.1.0-alpha2):

  ```rust,no_run
  // Before
  #[scenario(index = 0)]
  // After
  #[scenario(path = "tests/features/example.feature", index = 0)]
  ```

Think of it as *courgette‑driven* development: crisp, versatile, and it plays
nicely with everything else on your plate.

______________________________________________________________________

## Installation

Add the crates to your **dev‑dependencies**:

```toml
# Cargo.toml
[dev-dependencies]
rstest = "0.26.1"
rstest-bdd = "0.2.0"
```

Feature flags:

- `tokio` / `async-std` — choose your async test attribute (`#[tokio::test]`,
  etc.).

- `no-inventory` — fallback code‑gen registry for platforms where
  linker‑section collection is unwieldy.

- `compile-time-validation` — registers steps at compile time and reports
  missing or ambiguous steps with spans. (Disabled by default.)

- `strict-compile-time-validation` — fails compilation when steps are missing
  or ambiguous; implies `compile-time-validation`. (Disabled by default.)

Both features are disabled by default and apply only to the `rstest-bdd-macros`
crate. Enable them in your `Cargo.toml` with:

```toml
[dependencies]
rstest-bdd-macros = { version = "0.2.0", features = ["compile-time-validation"] }
```

Or via CLI:

```bash
cargo test --features "rstest-bdd-macros/compile-time-validation"
cargo test --features "rstest-bdd-macros/strict-compile-time-validation"
```

______________________________________________________________________

## Quick start (end‑to‑end “Web search”)

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

```rust,no_run
use rstest::fixture;
use rstest_bdd::{scenario, given, when, then};
// Browser automation example — pick your favourite WebDriver crate.
use thirtyfour::prelude::*;

#[fixture]
async fn browser() -> WebDriverResult<WebDriver> {
    let caps = DesiredCapabilities::firefox();
    Ok(WebDriver::new("http://localhost:4444", caps).await?)
}

// Bind this test to the named scenario from the feature file.
// The test body runs *after* all steps have passed.
#[scenario(path = "tests/features/web_search.feature", name = "Simple web search")]
#[tokio::test]
async fn test_simple_search(#[future] browser: WebDriver) {
    // Optional: final assertions / cleanup that aren't natural Gherkin steps.
}

#[given("the DuckDuckGo home page is displayed", result)]
async fn go_to_home(browser: &mut WebDriver) -> WebDriverResult<()> {
    browser.goto("https://duckduckgo.com/").await?;
    Ok(())
}

#[when("I search for \"(.*)\"", result)]
async fn search_for_phrase(browser: &mut WebDriver, phrase: String) -> WebDriverResult<()> {
    let form = browser.find(By::Id("search_form_input_homepage")).await?;
    form.send_keys(&phrase).await?;
    form.submit().await?;
    Ok(())
}

#[then("the search results page is displayed", result)]
async fn results_page_is_displayed(browser: &mut WebDriver) -> WebDriverResult<()> {
    browser.find(By::Id("links")).await?;
    Ok(())
}

#[then("the results contain \"(.*)\"", result)]
async fn results_contain_text(browser: &mut WebDriver, text: String) -> WebDriverResult<()> {
    let content = browser.source().await?;
    if content.contains(&text) { Ok(()) }
    else { Err(thirtyfour::error::WebDriverError::CustomError(
        format!("Result text not found: expected substring '{text}'")
    )) }
}
```

Run it:

```bash
cargo test -p your-crate -- --nocapture
```

Everything grows on the same trellis: your fixtures, your filters
(`cargo test search`), and your parallelism all continue to work as usual.

## Internationalisation in practice

Feature files can opt into another language by adding `# language: <code>` to
the first line. The Gherkin parser loads the appropriate keyword catalogue so
that teams can keep authoring steps in their preferred language. The
`examples/japanese-ledger` crate shows the full workflow in Japanese, including
Unicode step patterns, and a household ledger domain. Run it with
`cargo test -p japanese-ledger` to see two Japanese scenarios execute end to
end.

______________________________________________________________________

## Step definitions 101

Decorate plain Rust functions:

```rust,no_run
use rstest_bdd::{given, when, then};

#[given("an empty basket")]
fn empty_basket(basket: &mut Basket) {
    basket.clear();
}

#[when("I add {count:u32} pumpkins")]
fn add_pumpkins(basket: &mut Basket, count: u32) {
    basket.add(Item::Pumpkin, count);
}

#[then("the basket has {count:u32} pumpkins")]
fn assert_count(basket: &Basket, count: u32) {
    assert_eq!(basket.count(Item::Pumpkin), count);
}
```

Implicit fixtures such as `basket` must already be in scope in the test module;
`#[from(name)]` only renames a fixture and does not create one.

- Patterns accept **typed placeholders** like `{count:u32}`; values parse via
  `FromStr`.

- Fixtures are injected automatically when parameter names match fixtures;
  use `#[from(name)]` only to rename a parameter.

- Prefer readable step text first; compile‑time checks ensure you don’t forget
  an implementation.

______________________________________________________________________

## Scenario outline ≈ parametrised tests

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

```rust,no_run
#[scenario(path = "tests/features/login.feature", name = "Login with different credentials")]
#[tokio::test]
async fn test_login_scenarios(#[future] browser: WebDriver) {}

// Use typed placeholders to bind arguments by name.
#[when("I enter username {username} and password {password}")]
async fn enter_credentials(
    browser: &mut WebDriver,
    username: String,
    password: String,
) {
    // ...
}

#[then("I should see the message {message}")]
async fn see_message(browser: &mut WebDriver, message: String) {
    // ...
}
```

Under the rind, `#[scenario]` expands to an `rstest` parametrised test, so
cases show up individually in your runner.

______________________________________________________________________

## Background, tables, and docstrings

- **Background** runs before every scenario in the feature.
- **Data tables** arrive in a `datatable` parameter whose type implements
  `TryFrom<Vec<Vec<String>>>`. Continue using `Vec<Vec<String>>` or upgrade to
  `rstest_bdd::datatable::Rows<T>` for typed parsing.

- **Docstrings** arrive as a `String`.

```rust,no_run
#[given("the following users exist:")]
fn create_users(
    db: &mut DbConnection,
    #[datatable] datatable: rstest_bdd::datatable::Rows<UserRow>,
) {
    // `UserRow` implements `datatable::DataTableRow` elsewhere in the module.
    for row in datatable {
        db.insert_user(&row.name, &row.email);
    }
}
```

______________________________________________________________________

## Internationalization and localization

Write feature files in any language supported by Gherkin. Declare the locale at
the top of the `.feature` file and keep using the usual step macros:

```gherkin
# language: fr
Fonctionnalité: Panier
  Scénario: Ajouter un article
    Étant donné un panier vide
    Quand l'utilisateur ajoute une citrouille
    Alors le panier contient une citrouille
```

Keyword parsing is delegated to the `gherkin` crate, so `#[given]`, `#[when]`
and `#[then]` continue to match the translated keywords without additional
configuration.

Runtime diagnostics ship as Fluent translations bundled with the crate. English
messages are always available; call `select_localizations` to request another
locale before running scenarios:

```rust,no_run
use rstest_bdd::select_localizations;
use unic_langid::langid;

select_localizations(&[langid!("es")])?; // Switch diagnostics to Spanish
```

Missing translations fall back to English, and procedural macro diagnostics
remain in English so builds stay deterministic across environments.

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

- **Zero new runners**: keep Continuous Integration (CI) / Continuous Delivery
  (CD) and IDE behaviour unchanged.

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

If none is installed, install one, and re-run `make nixie`.

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
distribute this software for any purpose, with or without fee, and provided
that the copyright notice and this permission notice are included in all
copies. The software is provided “as is”, without warranty of any kind. See
`LICENSE` for the full text.

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
