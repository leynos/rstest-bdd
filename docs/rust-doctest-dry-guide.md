# A systematic guide to effective, ergonomic, and "don't repeat yourself" (DRY) doctests in Rust

## The `rustdoc` compilation model: a foundational perspective

To master the art of writing effective documentation tests in Rust, one must
first understand the foundational principles upon which the `rustdoc` tool
operates. Its behaviour, particularly its testing mechanism, is not an
arbitrary collection of features but a direct consequence of a deliberate
design philosophy. The core of this philosophy is that every doctest should
validate the public API of a crate from the perspective of an external user.
This single principle dictates the entire compilation model and explains both
the power and the inherent limitations of doctests.

### 1.1 The "separate crate" paradigm

At its heart, `rustdoc` treats each documentation test not as a snippet of code
running within the library's own context, but as an entirely separate,
temporary crate.[^1] When a developer executes

`cargo test --doc`, `rustdoc` initiates a multi-stage process for every code
block found in the documentation comments[^3]:

1. **Parsing and Extraction**: `rustdoc` first parses the source code of the
   library, resolving conditional compilation attributes (`#[cfg]`) to
   determine which items are active and should be documented for the current
   target.[^2] It then extracts all code examples enclosed in triple-backtick
   fences (\`\`\`).

2. **Code Generation**: For each extracted code block, `rustdoc` performs a
   textual transformation to create a complete, self-contained Rust program. If
   the block does not already contain a `fn main()`, the code is wrapped within
   one. Crucially, `rustdoc` also injects an `extern crate <mycrate>;`
   statement, where `<mycrate>` is the name of the library being documented.
   This makes the library under test available as an external dependency.[^3]

3. **Individual Compilation**: `rustdoc` then invokes the Rust compiler
   (`rustc`) separately for *each* of these newly generated miniature programs.
   Each one is compiled and linked against the already-compiled version of the
   main library.[^2]

4. **Execution and Verification**: Finally, if compilation succeeds, the
   resulting executable is run. The test is considered to have passed if the
   program runs to completion without panicking. The executable is then
   deleted.[^2]

The significance of this model cannot be overstated. It effectively transforms
every doctest into a true integration test.[^6] The test code does not have
special access to the library's internals; it interacts with the library's API
precisely as a downstream crate would, providing a powerful guarantee that the
public-facing examples are correct and functional.[^1]

### 1.2 First-order consequences of the model

This "separate crate" paradigm has two immediate and significant consequences
that shape all advanced doctesting patterns.

First, **API visibility is strictly limited to public items**. Because the
doctest is compiled as an external crate, it can only access functions,
structs, traits, and modules marked with the `pub` keyword. It has no access to
private items or even crate-level public items (e.g., `pub(crate)`). This is
not a bug or an oversight but a fundamental aspect of the design, enforcing the
perspective of an external consumer.[^1]

Second, the model has **profound performance implications**. The process of
invoking `rustc` to compile and link a new executable, for every single
doctest, is computationally expensive. For small projects, this overhead is
negligible. However, for large libraries with hundreds of doctests, the
cumulative compilation time can become a significant bottleneck in the
development and Continuous Integration (CI) and Continuous Deployment (CD)
cycle, a common pain point in the Rust community.[^2]

The architectural purity of the `rustdoc` model—its insistence on simulating an
external user—creates a fundamental trade-off. On one hand, it provides an
unparalleled guarantee that the public documentation is accurate and that the
examples work as advertised, creating true "living documentation".[^7] On the
other hand, this same purity prevents the use of doctests for verifying
documentation of internal, private APIs. This forces a bifurcation of
documentation strategy. Public-facing documentation can be tied directly to
working, tested code. Internal documentation for maintainers, which is equally
vital for a project's health, cannot be verified with the same tools. Examples
covering private functions must either be marked as

`ignore`, forgoing the test guarantee, or be duplicated in separate unit tests,
violating the "Don't Repeat Yourself" (DRY) principle.[^1] This reveals that
`rustdoc`'s design implicitly prioritizes the integrity of the public contract
over the convenience of a single, unified system for testable documentation of
both public and private code.

## Authoring effective doctests: from basics to best practices

With a solid understanding of the `rustdoc` compilation model, one can move on
to the practical craft of authoring doctests. An effective doctest is more than
just a block of code; it is a piece of technical communication that should be
clear, illustrative, and robust.

### 2.1 The anatomy of a doctest

Doctests reside within documentation comments. Rust recognizes two types:

- **Outer doc comments (`///`)**: These document the item that follows them
  (e.g., a function, struct, or module). This is the most common type.[^7]

- **Inner doc comments (`//!`)**: These document the item they are inside
  (e.g., a module or the crate itself). They are typically used at the top of
  `lib.rs` or `mod.rs` to provide crate- or module-level documentation.[^8]

<!-- markdownlint-disable MD013 --> Within these comments, a code block is
denoted by triple back-ticks (```). While `rustdoc` defaults to Rust syntax,
explicitly add the `rust` language specifier for clarity.[^3] A doctest
"passes" when it compiles and runs without panicking. To assert specific
outcomes, use the standard macros `assert!`, `assert_eq!`, and
`assert_ne!`.[^3] <!-- markdownlint-enable MD013 -->

### 2.2 The philosophy of a good example

The purpose of a documentation example extends beyond merely demonstrating
syntax. A reader can typically be expected to understand the mechanics of
calling a function or instantiating a struct. A truly valuable example
illustrates *why* and in *what context* an item should be used.[^9] It should
tell a small story or solve a miniature problem that illuminates the item's
purpose. For instance, an example for

`String::clone()` should not just show `hello.clone();`, but should demonstrate
a scenario where ownership rules necessitate creating a copy.[^9]

To achieve this, examples must be clear, concise, and purposeful. Any code that
directly relevant to the point being made—such as complex setup, boilerplate,
or unrelated logic—should be hidden to avoid distracting the reader.[^3]

### 2.3 Ergonomic error handling: taming the `?` operator

One of the most common ergonomic hurdles in writing doctests involves handling
functions that return a `Result`. The question mark (`?`) operator is the
idiomatic way to propagate errors in Rust, but it presents a challenge for
doctests. The implicit `fn main()` wrapper generated by `rustdoc` has a return
type of `()`, while the `?` operator can only be used in a function that
returns a `Result` or `Option`. This mismatch leads to a compilation error.[^3]

Using `.unwrap()` (or `.expect()`) in examples is strongly discouraged. It is
considered an anti-pattern because users often copy example code verbatim, and
encouraging panicking on errors is contrary to robust application design.[^9]
Instead, two canonical solutions exist.

#### Solution 1: The explicit main function

The most transparent, and recommended, approach is to manually write a main
function within the doctest that returns a Result. This leverages the
Termination trait, which is implemented for Result. The surrounding boilerplate
can then be hidden from the rendered documentation.

```rust,no_run
/// # Examples
///
/// ```
/// # use std::error::Error;
/// #
/// # fn main() -> Result<(), Box<dyn Error>> {
/// let config = "key=value".parse::<MyConfig>()?;
/// assert_eq!(config.get("key"), Some("value"));
/// #
/// # Ok(())
/// # }
/// ```
```

In this pattern, the reader only sees the core, fallible code, while the test
itself is a complete, well-behaved program.[^9]

#### Solution 2: The implicit Result-returning main

rustdoc provides a lesser-known but more concise shorthand for this exact
scenario. If a code block ends with the literal token (()), rustdoc will
automatically wrap the code in a main function that returns a Result.

```rust,no_run
/// # Examples
///
/// ```
/// let config = "key=value".parse::<MyConfig>()?;
/// assert_eq!(config.get("key"), Some("value"));
/// (()) // Note: No whitespace between parentheses
/// ```
```

This is functionally equivalent to the explicit `main` but requires less
boilerplate. However, it is critical that the `(())` be written as a single,
contiguous sequence of characters, as `rustdoc`'s detection mechanism is purely
textual and will not recognize `( () )`.[^3]

### 2.4 The power of hidden lines (`#`): creating clean examples

The mechanism that makes clean, focused examples possible is the "hidden line"
syntax. Any line in a doctest code block that begins with a `#` character
(optionally preceded by whitespace) will be compiled and executed as part of
the test, but it will be completely omitted from the final HTML documentation
rendered for the user.[^3]

This feature is essential for bridging the gap between what makes a good,
human-readable example, and what constitutes a complete, compilable program.
Its primary use cases include:

1. **Hiding** `main` **Wrappers**: As demonstrated in the error-handling
   examples, the entire `fn main() -> Result<…> { … }` and `Ok(())` scaffolding
   can be hidden, presenting the user with only the relevant code.[^9]

2. **Hiding Setup Code**: If an example requires some preliminary setup—like
   creating a temporary file, defining a helper struct for the test, or
   initializing a server—this logic can be hidden to keep the example focused
   on the API item being documented.[^3]

3. **Hiding** `use` **Statements**: While often useful to show which types are
   involved, `use` statements can sometimes be hidden to declutter simple
   examples.

The existence of features like hidden lines and the `(())` shorthand reveals a
core tension in `rustdoc`'s design. The compilation model is rigid: every test
must be a valid, standalone program.[^2] However, the ideal documentation
example is often just a small, illustrative snippet that is not a valid program
on its own.[^9] These ergonomic features are pragmatic "patches" designed to
resolve this conflict. They allow the developer to inject the necessary
boilerplate to satisfy the compiler without burdening the human reader with
irrelevant details. Understanding them as clever workarounds, rather than as
first-class language features, helps explain their sometimes quirky, text-based
behaviour.

## Advanced doctest control and attributes

Beyond basic pass/fail checks, `rustdoc` provides a suite of attributes to
control doctest behaviour with fine-grained precision. These attributes, placed
in the header of a code block (e.g., \`\`\`\`ignore\`), allow developers to
handle expected failures, non-executable examples, and other complex scenarios.

### 3.1 A comparative analysis of doctest attributes

Choosing the correct attribute is critical for communicating the intent of an
example and ensuring the test suite provides meaningful feedback. The following
table provides a comparative reference for the most common doctest attributes.

| Attribute    | Action                                                              | Test Outcome                                                   | Primary Use Case & Warnings                                                                                                                                                                                                           |
| ------------ | ------------------------------------------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ignore       | Skips both compilation and execution.                               | ignored                                                        | Use Case: For pseudocode, examples known to be broken, or to temporarily disable a test. Warning: Provides no guarantee that the code is even syntactically correct. Generally discouraged in favour of more specific attributes.[^3] |
| should_panic | Compiles and runs the code. The test passes if the code panics.     | OK on panic, failed if it does not panic.                      | Use Case: Demonstrating functions that are designed to panic on invalid input (e.g., indexing out of bounds).                                                                                                                         |
| compile_fail | Attempts to compile the code. The test passes if compilation fails. | OK on compilation failure, failed if it compiles successfully. | Use Case: Illustrating language rules, such as the borrow checker or type system constraints. Warning: Highly brittle. A future Rust version might make the code valid, causing the test to unexpectedly fail.[^4]                    |
| no_run       | Compiles the code but does not execute it.                          | OK if compilation succeeds.                                    | Use Case: Essential for examples with undesirable side effects in a test environment, such as network requests, filesystem input/output, or launching a GUI. Guarantees the example is valid Rust code without running it.[^5]        |
| edition20xx  | Compiles the code using the specified Rust edition's rules.         | OK on success.                                                 | Use Case: Demonstrating syntax alongside idioms that are specific to a particular Rust edition (e.g., edition2018, edition2021).[^4]                                                                                                  |

### 3.2 Detailed attribute breakdown

- `ignore`: This is the bluntest instrument in the toolbox. It tells `rustdoc`
  to do nothing with the code block. It is almost always better to either fix
  the example using hidden lines or use a more descriptive attribute like
  `no_run`.[^3] Its main legitimate use is for non-Rust code blocks or
  illustrative pseudocode.

- `should_panic`: This attribute inverts the normal test condition. It is used
  to document and verify behaviour that intentionally results in a panic. The
  test will fail if the code completes successfully or panics for a reason
  other than the one expected (if a specific panic message is asserted).[^3]

- `compile_fail`: This is a powerful tool for creating educational examples
  that demonstrate what *not* to do. It is frequently used in documentation
  about ownership, borrowing, and lifetimes to show code that the compiler will
  correctly reject. However, developers must be aware of its fragility. An
  evolution in the Rust language, or in the compiler, could make previously
  invalid code compile, which would break the test.[^4]

- `no_run`: This attribute strikes a crucial balance between test verification
  and practicality. For an example that demonstrates how to download a file
  from the internet, the example code must be syntactically correct and use the
  API properly, but it is undesirable for the Continuous Integration (CI)
  server to perform a network request during every test run. `no_run` provides
  this guarantee by compiling the code without executing it.[^5]

- `edition20xx`: This attribute allows an example to be tested against a
  specific Rust edition. This is important for crates that support multiple
  editions and need to demonstrate edition-specific features or migration
  paths.[^4]

## The DRY principle in doctests: managing shared and complex logic

The "Don't Repeat Yourself" (DRY) principle is a cornerstone of software
engineering, and it applies to test code as much as it does to production code.
As a project grows, it is common for multiple doctests to require the same
complex setup logic. Copying and pasting this setup into every doctest using
hidden lines is tedious, error-prone, and a clear violation of the DRY
principle.

### 4.1 The problem of shared setup

Consider a library for interacting with a database. Nearly every doctest might
need to perform the same initial steps: spin up a temporary database instance,
connect to it, and seed it with some initial data. Repeating this multi-line
setup in every single example is inefficient and makes maintenance difficult. A
change to the setup process would require updating dozens of doctests.

### 4.2 The `#[cfg(doctest)]` pattern for shared helpers

The canonical solution to this problem involves using a special configuration
flag provided by `rustdoc`: `doctest`. A common mistake is to try to place
shared test logic in a block guarded by `#[cfg(test)]`. This will not work,
because `rustdoc` does not enable the `test` configuration flag during its
compilation process; `#[cfg(test)]` is reserved for unit and integration tests
run directly by `cargo test`.[^11]

Instead, `rustdoc` sets its own unique `doctest` flag. By guarding a module or
function with `#[cfg(doctest)]`, developers can write helper code that is
compiled and available *only* when `cargo test --doc` is running. This code is
excluded from normal production builds and standard unit test runs, preventing
any pollution of the final binary or the public API.

The typical implementation pattern is to create a private helper module within
the library:

```rust,no_run
// In lib.rs or a submodule

/// A function that requires a complex environment to test.
///
/// # Examples
///
/// ```
/// # use crate::doctest_helpers::setup_test_environment;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut ctx = setup_test_environment()?;
/// let result = my_func_that_needs_env(&mut ctx);
/// assert!(result.is_ok());
/// # Ok(())
/// # }
/// ```
pub fn my_func_that_needs_env(ctx: &mut TestContext) -> Result<(), ()> {
    //… function logic…
    Ok(())
}

// This module and its contents are only compiled for doctests.
#[cfg(doctest)]
mod doctest_helpers {
    // Re-export any types needed for the function signatures above.
    pub use super::TestContext;
    use std::io::Result;

    pub struct TestContext {
        //… fields for the test context…
    }

    pub fn setup_test_environment() -> Result<TestContext> {
        // All the complex, shared setup logic lives here once.
        println!("Setting up test environment…");
        Ok(TestContext { /*… */ })
    }
}

// A struct that might be needed by the public function signature.
// It can be defined normally.
pub struct TestContext { /*… */ }
```

This pattern is the most effective way to achieve DRY doctests. It centralizes
setup logic, improves maintainability, and cleanly separates testing concerns
from production code.[^11]

### 4.3 Advanced DRY: programmatic doctest generation

For highly specialized use cases, such as authoring procedural macros, the DRY
principle can be taken a step further. A procedural macro generates code, and
it is often desirable to test that the generated code itself contains valid and
working documentation. Writing these doctests manually can be exceptionally
repetitive.

Crates like `quote-doctest` address this by allowing developers to
programmatically construct a doctest from a `TokenStream`. This enables the
generation of doctests from the same source of truth that generates the code
they are intended to test, representing the ultimate application of the DRY
principle in this domain.[^12]

## Conditional compilation strategies for doctests

Conditional compilation is a powerful feature of Rust, but it introduces
significant complexity when interacting with `rustdoc`. A common source of
confusion stems from the failure to distinguish between two separate goals: (1)
ensuring platform-specific or feature-gated items *appear* in the
documentation, and (2) ensuring doctests for those items *execute* only under
the correct conditions. These two goals are achieved with different mechanisms
that operate at different stages of the `rustdoc` pipeline.

### 5.1 Documenting conditionally compiled items: `#[cfg(doc)]`

**The Goal**: To ensure that an item, such as a `struct UnixSocket` that is
only available on Unix-like systems, is included in the documentation
regardless of which platform `rustdoc` is run on (e.g., when generating docs on
a Windows machine).

**The Mechanism**: `rustdoc` always invokes the compiler with the `--cfg doc`
flag set. By adding `doc` to an item's `#[cfg]` attribute, a developer can
instruct the compiler to include that item specifically for documentation
builds.[^13]

**The Pattern**:

```rust,no_run
/// A socket that is only available on Unix platforms.
#[cfg(any(target_os = "unix", doc))]
pub struct UnixSocket;
```

This `any` directive ensures the struct is compiled either when the target OS
is `unix` OR when `rustdoc` is running. This correctly makes the item visible
in the generated HTML. However, it is crucial to understand that this **does
not** make the doctest for `UnixSocket` pass on non-Unix platforms.

This distinction highlights the "cfg duality." The `#[cfg(doc)]` attribute
controls the *table of contents* of the documentation; it determines which
items are parsed and rendered. The actual compilation of a doctest, however,
happens in a separate, later stage. In that stage, the `doc` cfg is *not*
passed to the compiler.[^13] The compiler only sees the host `cfg` (e.g.,
`target_os = "windows"`), so the `UnixSocket` type is not available, and the
test fails to compile. `#[cfg(doc)]` affects what is documented, not what is
testable.

### 5.2 Executing doctests conditionally: feature flags

**The Goal**: To ensure a doctest that relies on an optional crate feature
(e.g., a feature named `"serde"`) is only executed when that feature is enabled
via `cargo test --doc --features "serde"`.

Two primary patterns exist to achieve this.

Pattern 1: #\[cfg\] Inside the Code Block

This pattern involves placing a #\[cfg\] attribute directly on the code within
the doctest itself.

```rust,no_run
/// This example only runs if the "serde" feature is enabled.
///
/// ```
/// # #[cfg(feature = "serde")]
/// # {
/// #   let my_struct = MyStruct::new();
/// #   let json = serde_json::to_string(&my_struct).unwrap();
/// #   assert_eq!(json, "{}");
/// # }
/// ```
```

When the `"serde"` feature is disabled, the code inside the block is compiled
out. The doctest becomes an empty program that runs, does nothing, and is
reported as `ok`. While simple to write, this can be misleading, because the
test suite reports a "pass" even though the test was effectively skipped.[^14]

Pattern 2: cfg_attr to Conditionally ignore the Test

A more explicit and accurate pattern uses the cfg_attr attribute to
conditionally add the ignore flag to the doctest's header. This is typically
done with inner doc comments (//!).

```rust,no_run
//! #![cfg_attr(not(feature = "serde"), doc = "```ignore")]
//! #![cfg_attr(feature = "serde", doc = "```")]
//! // Example code that requires the "serde" feature.
//! let my_struct = MyStruct::new();
//! let json = serde_json::to_string(&my_struct).unwrap();
//! assert_eq!(json, "{}");
//! ```
```

With this pattern, if the `"serde"` feature is disabled, the test is marked as
`ignored` in the test results, which more accurately reflects its status. If
the feature is enabled, the `ignore` is omitted, and the test runs normally.
This approach provides clearer feedback but is significantly more verbose and
less ergonomic, especially when applied to outer (`///`) doc comments, as the
`cfg_attr` must be applied to every single line of the comment.[^14]

### 5.3 Displaying feature requirements in docs: `#[doc(cfg(…))]`

To complement conditional execution, Rust provides a way to visually flag
feature-gated items in the generated documentation. This is achieved with the
`#[doc(cfg(…))]` attribute, which requires enabling the `#![feature(doc_cfg)]`
feature gate at the crate root.

```rust,no_run
// At the crate root (lib.rs)
#![feature(doc_cfg)]

// On the feature-gated item
#[cfg(feature = "serde")]
#[doc(cfg(feature = "serde"))]
pub fn function_requiring_serde() { /*… */ }
```

This will render a banner in the documentation for `function_requiring_serde`
that reads, "This is only available when the `serde` feature is enabled." This
attribute is purely for documentation generation, and it is independent of, but
often used alongside, the conditional test execution patterns.[^14]

## Doctests in the wider project ecosystem

Doctests are a powerful tool, but they are just one component of a
comprehensive testing strategy. Understanding their specific role, and their
limitations, is key to maintaining a healthy and well-tested Rust project.

### 6.1 Choosing the right test type: a decision framework

A robust testing strategy leverages three distinct types of tests, each with
its own purpose:

- **Doctests**: These are ideal for simple, "happy-path" examples of the
  public API. Their dual purpose is to provide clear documentation for users
  and to act as a basic sanity check that the examples remain correct over
  time. They should be easy to read and focused on illustrating a single
  concept.[^6]

- **Unit tests (`#[test]` in `src/`)**: These are for testing the
  nitty-gritty details of the implementation. They are placed in submodules
  within the source files (often `mod tests { … }`) and are compiled only with
  `#[cfg(test)]`. Because they live inside the crate, they can access private
  functions and modules, making them perfect for testing internal logic, edge
  cases, and specific error conditions.[^1]

- **Integration Tests (in the** `tests/` **directory)**: These test the crate
  from a completely external perspective, much like doctests. However, they are
  not constrained by the need to be readable documentation. They are suited for
  testing complex user workflows, interactions between multiple API entry
  points, and the overall behaviour of the library as a black box.[^6]

### 6.2 The unsolved problem: testing private APIs

As established, the `rustdoc` compilation model makes testing private items in
doctests impossible by design.[^1] The community has developed several
workarounds, but each comes with significant trade-offs[^1]:

1. `ignore` **the test**: This allows the example to exist in the documentation
   but sacrifices the guarantee of correctness. It is the least desirable
   option.

2. **Make items** `pub` **in a** `detail` **or** `internal` **module**: This
   compromises API design by polluting the public namespace and exposing
   implementation details that should be encapsulated. It can lead to misuse by
   users and makes future refactoring difficult.

3. **Use** `cfg_attr` **to conditionally make items public**: This involves
   adding an attribute like
   `#[cfg_attr(feature = "doctest-private", visibility::make(pub))]` to every
   private item that requires testing. While robust, it is highly invasive and
   adds significant boilerplate throughout the codebase.

The expert recommendation is to acknowledge this limitation and not fight the
tool. Do not compromise a clean API design for the sake of doctests. Use
doctests for their intended purpose—verifying public API examples—and rely on
dedicated unit tests for verifying private logic. The lack of a clean solution,
for test-verifying private documentation, is a known and accepted trade-off
within the Rust ecosystem.

### 6.3 Practical challenges and solutions

Beyond architectural considerations, developers face several practical,
real-world challenges when working with doctests.

- **The** `README.md` **Dilemma**: A project's `README.md` file serves multiple
  audiences. It needs to render cleanly on platforms like GitHub and
  [crates.io](http://crates.io), where hidden lines (`#…`) look like ugly,
  commented-out code. At the same time, it should contain testable examples,
  which often require hidden lines for setup.[^10] The best practice is to
  avoid maintaining the README manually. Instead, use a tool like

  `cargo-readme`. This tool generates a `README.md` file from the crate-level
  documentation (in `lib.rs`), automatically stripping out the hidden lines.
  This provides a single source of truth that is both fully testable via
  `cargo test --doc` and produces a clean, professional README for external
  sites.[^10]

- **Developer Ergonomics in IDEs**: Writing code inside documentation comments
  can be a subpar experience. IDEs and tools like `rust-analyzer` often provide
  limited or no autocompletion, real-time error checking, or refactoring
  support for code within a comment block.[^15] A common and effective workflow
  to mitigate this is to first write and debug the example as a standard

  `#[test]` function in a temporary file or test module. This allows the
  developer to leverage the full power of the IDE. Once the code is working
  correctly, it can be copied into the doc comment, and the necessary
  formatting (`///`, `#`, etc.) can be applied.[^15]

## Conclusion and recommendations

Rust's documentation testing framework is a uniquely powerful feature that
promotes the creation of high-quality, reliable "living documentation." By
deeply understanding its underlying compilation model, and the patterns that
have evolved to manage its constraints, developers can write doctests that are
effective, ergonomic, and maintainable. To summarize the key principles for
mastering doctests:

1. **Embrace the Model**: Treat a doctest as an external integration test
   compiled in a separate crate; this mental model explains nearly all of its
   behaviour.

2. **Prioritize Clarity**: Write examples that teach the *why*, not just the
   *how*. Use hidden lines (`#`) ruthlessly to eliminate boilerplate and focus
   the reader's attention on the relevant code.

3. **Handle Errors Gracefully**: For examples of fallible functions, always use
   the `fn main() -> Result<…>` pattern, hiding the boilerplate. Avoid
   `.unwrap()` to promote robust error-handling practices.

4. **Be DRY**: When setup logic is shared across multiple examples, centralize
   it in a helper module guarded by `#[cfg(doctest)]` to avoid repetition.

5. **Master** `cfg`: Use `#[cfg(doc)]` to control an item's *visibility* in the
   final documentation. Use `#[cfg(feature = "…")]` or other `cfg` flags
   *inside* the test block to control its conditional *execution*. Do not
   confuse the two.

6. **Know When to Stop**: A doctest is not the right tool for every job. When
   an example becomes overly complex, requires testing intricate error paths,
   or needs to access private implementation details, move it to a dedicated
   unit or integration test. Avoid compromising API design or test clarity by
   forcing a square peg into a round hole. Use the right tool for the job.

### **Works cited**

[^1]: Stack Overflow — Writing documentation tests for private modules,
accessed on July 15, 2025,
<https://stackoverflow.com/questions/70111757/how-can-i-write-documentation-tests-for-private-modules>
[^2]: Rustdoc doctests need fixing - Swatinem, accessed on July 15, 2025,
<https://swatinem.de/blog/fix-rustdoc/>
[^3]: Documentation tests - The rustdoc boOK - Rust Documentation, accessed on
July 15, 2025, <https://doc.rust-lang.org/rustdoc/documentation-tests.html>
[^4]: Documentation tests - - GitHub Pages, accessed on July 15, 2025,
<https://ebarnard.github.io/2019-06-03-rust-smaller-trait-implementers-docs/rustdoc/documentation-tests.html>
[^5]: Documentation tests - - MIT, accessed on July 15, 2025,
<http://web.mit.edu/rust-lang_v1.25/arch/amd64_ubuntu1404/share/doc/rust/html/rustdoc/documentation-tests.html>
[^6]: LogRocket Blog — Organizing Rust tests, accessed on July 15, 2025,
<https://blog.logrocket.com/how-to-organize-rust-tests/>
<https://www.reddit.com/r/rust/comments/qk77iu/best_way_to_organise_tests_in_rust/>
[^7]: Writing Rust documentation - Dev Community, accessed on July 15, 2025,
<https://dev.to/gritmax/writing-rust-documentation-5hn5>
[^8]: The rustdoc book, accessed on July 15, 2025,
<https://doc.rust-lang.org/rustdoc/what-is-rustdoc.html>
[^9]: Documentation - Rust API Guidelines, accessed on July 15, 2025,
<https://rust-lang.github.io/api-guidelines/documentation.html>
[^10]: Best practice for doc testing README - help - The Rust Programming
       Language Forum, accessed on July 15, 2025,
       <https://users.rust-lang.org/t/best-practice-for-doc-testing-readme/114862>
[^11]: Compile_fail doc test ignored in cfg(test) - help - The Rust Programming
Language Forum, accessed on July 15, 2025,
<https://users.rust-lang.org/t/compile-fail-doc-test-ignored-in-cfg-test/124927>
 Accessed on July 15, 2025,
<https://users.rust-lang.org/t/test-setup-for-doctests/50426>
[^12]: quote_doctest - Rust - [Docs.rs](http://Docs.rs), accessed on July 15,
2025, <https://docs.rs/quote-doctest>
[^13]: Advanced features - The rustdoc boOK - Rust Documentation, accessed on
       July 15, 2025, <https://doc.rust-lang.org/rustdoc/advanced-features.html>
[^14]: Stack Overflow — Conditionally executing a module-level doctest,
accessed on July 15, 2025,
<https://stackoverflow.com/questions/50312190/how-can-i-conditionally-execute-a-module-level-doctest-based-on-a-feature-flag>
 Stack Overflow — Conditional compilation with doctests, accessed on July 15,
2025,
<https://stackoverflow.com/questions/38292741/how-would-one-achieve-conditional-compilation-with-rust-projects-that-have-docte>
[^15]: Reddit — Preferred approaches for doc tests, accessed on July 15,
2025,
<https://www.reddit.com/r/rust/comments/ke438a/how_do_you_write_your_doc_tests/>
