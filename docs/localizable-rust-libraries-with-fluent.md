# Architecting localizable Rust libraries with Fluent

When building a reusable Rust library (a crate), providing localized text for
elements such as error messages and UI components presents a unique challenge.
The library itself cannot and should not make assumptions about the end user's
language preference. The final application that consumes the library is the
sole authority on the current locale.

The solution is a robust architectural pattern based on a clear separation of
concerns and dependency injection. In this model, the library provides the
localizable _resources_ (the `.ftl` files), and the consuming application
provides the localization _context_ (the configured `LanguageLoader`). This
ensures that the application maintains full control over language negotiation
and resource loading, while the library remains agnostic and highly reusable.

This guide outlines the standard pattern for creating and consuming localizable
libraries in Rust using the Fluent ecosystem.

## Core principles

1. **The Application is the Authority:** The application is solely responsible
   for detecting the user's locale, creating and configuring a single,
   authoritative `LanguageLoader`, and managing the overall localization state.
2. **Libraries Provide Resources:** The library's role is to embed its `.ftl`
   translation files as assets and expose public functions that require a
   `LanguageLoader` to produce a translated string. The library defines _what_
   can be translated.
3. **Localization via Dependency Injection:** The application "injects" its
   configured `LanguageLoader` into the library's functions when it needs a
   localized message. The library never creates its own loader.
4. **Composability:** This pattern is highly composable. An application can
   aggregate translation assets from multiple independent libraries into one
   unified localization context, ensuring consistency across the entire program.

## Implementing the pattern: A two-crate workspace example

To illustrate this pattern, let's build a simple workspace containing an
application (`my-app`) that consumes a localizable library (`my-lib`).

### 1. Workspace setup

First, create the workspace structure.

```text
i18n-workspace/
├── Cargo.toml
├── my-app/
│   ├── Cargo.toml
│   └── src/main.rs
└── my-lib/
    ├── Cargo.toml
    ├── i18n/
    │   └── en-US/
    │       └── errors.ftl
    └── src/lib.rs

```

The root `Cargo.toml` defines the workspace members:

`i18n-workspace/Cargo.toml`

```ini,toml
[workspace]
members = ["my-app", "my-lib"]

```

### 2. The library crate (`my-lib`)

The library will contain its own Fluent Translation List (FTL) resources and
expose a function to retrieve localized messages.

`my-lib/Cargo.toml`

The library needs `i18n-embed` for the Fluent abstractions and `rust-embed` to
bundle the `.ftl` files into the binary.1

```ini,toml
[package]
name = "my-lib"
version = "0.1.0"
edition = "2024"

[dependencies]
i18n-embed = { version = "0.14", features = ["fluent-system"] }
rust-embed = "8.0"

```

`my-lib/i18n/en-US/errors.ftl`

This file contains the library's localizable strings.

```fluent
error-not-found = The requested item could not be found.
error-permission-denied = You do not have permission to perform this action.

```

`my-lib/src/lib.rs`

The library's code exposes its embedded assets and a function that accepts the
application's `LanguageLoader`.

```rust,no_run
use i18n_embed::fluent::FluentLanguageLoader;
use rust_embed::RustEmbed;

// 1. Embed the 'i18n' directory into the library binary.
// This makes the.ftl files available to the consuming application.
#
#[folder = "i18n/"]
pub struct MyLibLocalizations;

// 2. Define a public function that accepts a LanguageLoader via dependency injection.
// The library does not create its own loader; it uses the one provided by the application.
pub fn get_error_message(loader: &FluentLanguageLoader, error_id: &str) -> String {
    // 3. Use the provided loader to look up a message from the library's own resources.
    loader.lookup(error_id, None)
}

```

### 3. The application crate (`my-app`)

The application is responsible for setting up the localization context and
calling the library.

`my-app/Cargo.toml`

The application depends on the library and adds the `desktop-requester` feature
to `i18n-embed` to detect the system's locale.2

```ini,toml
[package]
name = "my-app"
version = "0.1.0"
edition = "2024"

[dependencies]
my-lib = { path = "../my-lib" }
i18n-embed = { version = "0.14", features = ["fluent-system", "desktop-requester"] }
rust-embed = "8.0"
unic-langid = "0.9"

```

`my-app/src/main.rs`

The application's `main` function orchestrates the entire process.

```rust,no_run
use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    DesktopLanguageRequester, I18nAssets,
};
use my_lib::{get_error_message, MyLibLocalizations};
use unic_langid::langid;

// The I18nAssets trait requires a struct to represent the assets.
// Here, we create a simple struct that will represent all assets,
// including those from the library.
struct AllLocalizations;

// Implement the I18nAssets trait to tell i18n-embed where to find
// the library's embedded resources.
impl I18nAssets for AllLocalizations {
    fn get_asset(path: &str) -> Option<std::borrow::Cow<'static, [u8]>> {
        MyLibLocalizations::get(path)
    }

    fn list_assets(path: &str) -> i18n_embed::rust_embed::Filenames {
        MyLibLocalizations::iter()
    }
}

fn main() {
    // 1. Create the application's single, authoritative LanguageLoader.
    let loader: FluentLanguageLoader = fluent_language_loader!();

    // 2. Determine the user's preferred language from the system.
    let requester = DesktopLanguageRequester::new();
    let requested_locales = requester.requested_languages();

    // 3. Perform language negotiation. The `select` function finds the best
    // matching language and loads all corresponding resources from the
    // library's assets into the application's loader.
    i18n_embed::select(&loader, &AllLocalizations, &requested_locales)
       .expect("Failed to select a language");

    // 4. Call the library's function, injecting the fully configured loader.
    let error_msg = get_error_message(&loader, "error-not-found");
    println!("Received from library: {}", error_msg);

    let perm_msg = get_error_message(&loader, "error-permission-denied");
    println!("Received from library: {}", perm_msg);
}

```

## Conclusion

This dependency injection pattern provides a clean, robust, and scalable
architecture for internationalization in a modular Rust ecosystem.1

- **For library authors:** The pattern enables shipping localizable components
  without imposing a specific localization strategy on consuming applications.
  The library remains focused on its core functionality, simply exposing its
  translatable resources.
- **For application teams:** The pattern preserves full control over the user
  experience. Teams can manage locales, provide fallbacks, and aggregate
  resources from any number of third-party crates into a single, consistent
  localization context.

By adhering to this separation of concerns, the Rust community can build a rich
ecosystem of composable, internationalized libraries that work together
seamlessly.

## Works cited

1. i18n_embed - Rust - [Docs.rs](http://Docs.rs), accessed on August 18, 2025,
   [https://docs.rs/i18n-embed](https://docs.rs/i18n-embed)

2. i18n_embed - Rust - [Docs.rs](http://Docs.rs), accessed on August 18, 2025,
   [https://docs.rs/i18n-embed/0.14.1/i18n_embed/](https://docs.rs/i18n-embed/0.14.1/i18n_embed/)
