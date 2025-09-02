# Assistant Instructions

## Code Style and Structure

- **Code is for humans.** Write your code with clarity and empathy—assume a
  tired teammate will need to debug it at 3 a.m.
- **Comment *why*, not *what*.** Explain assumptions, edge cases, trade-offs, or
  complexity. Don't echo the obvious.
- **Clarity over cleverness.** Be concise, but favour explicit over terse or
  obscure idioms. Prefer code that's easy to follow.
- **Use functions and composition.** Avoid repetition by extracting reusable
  logic. Prefer generators or comprehensions, and declarative code to
  imperative repetition when readable.
- **Small, meaningful functions.** Functions must be small, clear in purpose,
  single responsibility, and obey command/query segregation.
- **Clear commit messages.** Commit messages should be descriptive, explaining
  what was changed and why.
- **Name things precisely.** Use clear, descriptive variable and function names.
  For booleans, prefer names with `is`, `has`, or `should`.
- **Structure logically.** Each file should encapsulate a coherent module. Group
  related code (e.g., models + utilities + fixtures) close together.
- **Group by feature, not layer.** Colocate views, logic, fixtures, and helpers
  related to a domain concept rather than splitting by type.
- **Use consistent spelling and grammar.** Comments must use en-GB-oxendict
  ("-ize" / "-yse" / "-our") spelling and grammar, with the exception of
  references to external APIs.
- **Illustrate with clear examples.** Function documentation must include clear
  examples demonstrating the usage and outcome of the function. Test
  documentation should omit examples where the example serves only to reiterate
  the test logic.
- **Keep file size managable.** No single code file may be longer than 400
  lines. Long switch statements or dispatch tables should be broken up by
  feature and constituents colocated with targets. Large blocks of test data
  should be moved to external data files.

## Documentation Maintenance

- **Reference:** Use the markdown files within the `docs/` directory as a
  knowledge base and source of truth for project requirements, dependency
  choices, and architectural decisions.
- **Update:** When new decisions are made, requirements change, libraries are
  added/removed, or architectural patterns evolve, **proactively update** the
  relevant file(s) in the `docs/` directory to reflect the latest state.
  **Ensure the documentation remains accurate and current.**
- Documentation must use en-GB-oxendict ("-ize" / "-yse" / "-our") spelling
  and grammar. (EXCEPTION: the naming of the "LICENSE" file, which is to be
  left unchanged for community consistency.)

## Change Quality & Committing

- **Atomicity:** Aim for small, focused, atomic changes. Each change (and
  subsequent commit) should represent a single logical unit of work.
- **Quality Gates:** Before considering a change complete or proposing a commit,
  ensure it meets the following criteria:
  - New functionality or changes in behaviour are fully validated by relevant
    unittests and behavioural tests.
  - Where a bug is being fixed, a unittest has been provided demonstrating the
    behaviour being corrected both to validate the fix and to guard against
    regression.
  - Passes all relevant unit and behavioral tests according to the guidelines
    above. (Use `make test` to verify).
  - Passes lint checks. (Use `make lint` to verify).
  - Adheres to formatting standards tested using a formatting validator. (Use
    `make check-fmt` to verify).
- **Committing:**
  - Only changes that meet all the quality gates above should be committed.
  - Write clear, descriptive commit messages summarizing the change, following
    these formatting guidelines:
    - **Imperative Mood:** Use the imperative mood in the subject line (e.g.,
      "Fix bug", "Add feature" instead of "Fixed bug", "Added feature").
    - **Subject Line:** The first line should be a concise summary of the change
      (ideally 50 characters or less).
    - **Body:** Separate the subject from the body with a blank line. Subsequent
      lines should explain the *what* and *why* of the change in more detail,
      including rationale, goals, and scope. Wrap the body at 72 characters.
    - **Formatting:** Use Markdown for any formatted text (like bullet points or
      code snippets) within the commit message body.
  - Do not commit changes that fail any of the quality gates.

## Refactoring Heuristics & Workflow

- **Recognizing Refactoring Needs:** Regularly assess the codebase for potential
  refactoring opportunities. Consider refactoring when you observe:
  - **Long Methods/Functions:** Functions or methods that are excessively long
    or try to do too many things.
  - **Duplicated Code:** Identical or very similar code blocks appearing in
    multiple places.
  - **Complex Conditionals:** Deeply nested or overly complex `if`/`else` or
    `switch` statements (high cyclomatic complexity).
  - **Large Code Blocks for Single Values:** Significant chunks of logic
    dedicated solely to calculating or deriving a single value.
  - **Primitive Obsession / Data Clumps:** Groups of simple variables (strings,
    numbers, booleans) that are frequently passed around together, often
    indicating a missing class or object structure.
  - **Excessive Parameters:** Functions or methods requiring a very long list of
    parameters.
  - **Feature Envy:** Methods that seem more interested in the data of another
    class/object than their own.
  - **Shotgun Surgery:** A single change requiring small modifications in many
    different classes or functions.
- **Post-Commit Review:** After committing a functional change or bug fix (that
  meets all quality gates), review the changed code and surrounding areas using
  the heuristics above.
- **Separate Atomic Refactors:** If refactoring is deemed necessary:
  - Perform the refactoring as a **separate, atomic commit** *after* the
    functional change commit.
  - Ensure the refactoring adheres to the testing guidelines (behavioral tests
    pass before and after, unit tests added for new units).
  - Ensure the refactoring commit itself passes all quality gates.

## Rust Specific Guidance

This repository is written in Rust and uses Cargo for building and dependency
management. Contributors should follow these best practices when working on the
project:

- Run `make fmt`, `make lint`, and `make test` before committing. These targets
  wrap `cargo fmt`, `cargo clippy`, and `cargo test` with the appropriate flags.
- Clippy warnings MUST be disallowed.
- Fix any warnings emitted during tests in the code itself rather than
  silencing them.
- Where a function is too long, extract meaningfully named helper functions
  adhering to separation of concerns and CQRS.
- Where a function has too many parameters, group related parameters in
  meaningfully named structs.
- Where a function is returning a large error consider using `Arc` to reduce the
  amount of data returned.
- Every module **must** begin with a module level (`//!`) comment explaining the
  module's purpose and utility.
- Document public APIs using Rustdoc comments (`///`) so documentation can be
  generated with cargo doc.
- Prefer immutable data and avoid unnecessary `mut` bindings.
- Handle errors with the `Result` type instead of panicking where feasible.
- Use explicit version ranges in `Cargo.toml` and keep dependencies up-to-date.
- Avoid `unsafe` code unless absolutely necessary and document any usage
  clearly.
- Place function attributes **after** doc comments.
- Do not use `return` in single-line functions.
- Use predicate functions for conditional criteria with more than two branches.
- Lints must not be silenced except as a **last resort**.
- Lint rule suppressions must be tightly scoped and include a clear reason.
- Prefer `expect` over `allow`.
- Where a function is unused with specific features selected, use conditional
  compilation with `#[cfg]` or `#[cfg_attr]`.
- Prefer `.expect()` over `.unwrap()`.
- Use `concat!()` to combine long string literals rather than escaping newlines
  with a backslash.
- Prefer single line versions of functions where appropriate. I.e.,

  ```rust
  pub fn new(id: u64) -> Self { Self(id) }
  ```

  Instead of:

  ```rust
  pub fn new(id: u64) -> Self {
      Self(id)
  }
  ```

### Testing

- Write unit and behavioural tests for new functionality. Run both before and
  after making any change.
- Use `rstest` fixtures for shared setup.
- Replace duplicated tests with `#[rstest(...)]` parameterised cases.
- Prefer `mockall` for mocks/stubs.
- Mock non-deterministic dependencies (e.g., environment variables and the
  system clock) using dependency injection with the `mockable` crate (traits
  like `Env` and `Clock`) where appropriate. See
  `docs/reliable-testing-in-rust-via-dependency-injection.md` for guidance.

### Dependency Management

- **Mandate caret requirements for all dependencies.** All crate versions
  specified in `Cargo.toml` must use SemVer-compatible caret requirements
  (e.g., `some-crate = "1.2.3"`). This is Cargo's default and allows for safe,
  non-breaking updates to minor and patch versions while preventing breaking
  changes from new major versions. This approach is critical for ensuring build
  stability and reproducibility.
- **Prohibit unstable version specifiers.** The use of wildcard (`*`) or
  open-ended inequality (`>=`) version requirements is strictly forbidden, as
  they introduce unacceptable risk and unpredictability. Tilde requirements
  (`~`) should only be used where a dependency must be locked to patch-level
  updates for a specific, documented reason.

### Error Handling

- **Prefer semantic error enums**. Derive `std::error::Error` (via the
  `thiserror` crate) for any condition the caller might inspect, retry, or map
  to an HTTP status.
- **Use an *opaque* error only at the app boundary**. Use `eyre::Report` for
  human-readable logs; these should not be exposed in public APIs.
- **Never export the opaque type from a library**. Convert to domain enums at
  API boundaries, and to `eyre` only in the main `main()` entrypoint or
  top-level async task.

## Markdown Guidance

- Validate Markdown files using `make markdownlint`.
- Run `make fmt` after any documentation changes to format all Markdown
  files and fix table markup.
- Validate Mermaid diagrams in Markdown files by running `make nixie`.
- Markdown paragraphs and bullet points must be wrapped at 80 columns.
- Code blocks must be wrapped at 120 columns.
- Tables and headings must not be wrapped.
- Use dashes (`-`) for list bullets.
- Use GitHub-flavoured Markdown footnotes (`[^1]`) for references and
  footnotes.

## Additional tooling

The following tooling is available in this environment:

- `mbake` – A Makefile validator. Run using `mbake validate Makefile`.
- `strace` – Traces system calls and signals made by a process; useful for
  debugging runtime behaviour and syscalls.
- `gdb` – The GNU Debugger, for inspecting and controlling programs as they
  execute (or post-mortem via core dumps).
- `ripgrep` – Fast, recursive text search tool (`grep` alternative) that
  respects `.gitignore` files.
- `ltrace` – Traces calls to dynamic library functions made by a process.
- `valgrind` – Suite for detecting memory leaks, profiling, and debugging
  low-level memory errors.
- `bpftrace` – High-level tracing tool for eBPF, using a custom scripting
  language for kernel and application tracing.
- `lsof` – Lists open files and the processes using them.
- `htop` – Interactive process viewer (visual upgrade to `top`).
- `iotop` – Displays and monitors I/O usage by processes.
- `ncdu` – NCurses-based disk usage viewer for finding large files/folders.
- `tree` – Displays directory structure as a tree.
- `bat` – `cat` clone with syntax highlighting, Git integration, and paging.
- `delta` – Syntax-highlighted pager for Git and diff output.
- `tcpdump` – Captures and analyses network traffic at the packet level.
- `nmap` – Network scanner for host discovery, port scanning, and service
  identification.
- `lldb` – LLVM debugger, alternative to `gdb`.
- `eza` – Modern `ls` replacement with more features and better defaults.
- `fzf` – Interactive fuzzy finder for selecting files, commands, etc.
- `hyperfine` – Command-line benchmarking tool with statistical output.
- `shellcheck` – Linter for shell scripts, identifying errors and bad practices.
- `fd` – Fast, user-friendly `find` alternative with sensible defaults.
- `checkmake` – Linter for `Makefile`s, ensuring they follow best practices and
  conventions.
- `srgn` – [Structural grep](https://github.com/alexpovel/srgn), searches code
  and enables editing by syntax tree patterns (see `docs/srgn.md` for a
  complete guide).
- `difft` **(Difftastic)** – Semantic diff tool that compares code structure
  rather than just text differences.

## Key Takeaway

These practices help maintain a high-quality codebase and facilitate
collaboration.
