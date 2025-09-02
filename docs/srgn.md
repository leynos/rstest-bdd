# A Command-Line Wizard's Guide to Surgical Code Refactoring with srgn

## Part 1: Introduction - The Code Surgeon's Scalpel

### 1.1 Beyond Grep: The Need for Syntactic Precision

In the arsenal of any command-line proficient developer, tools like `grep`,
`sed`, and `ripgrep` are indispensable instruments for searching and
manipulating text. They are fast, powerful, and universally available. However,
they share a fundamental limitation: they perceive source code as a flat stream
of characters, oblivious to its intricate grammatical structure. This blindness
prevents them from reliably performing context-aware refactoring, where a
change in one syntactic location (e.g., a function signature) should not affect
another (e.g., a string literal).

This is the precise gap that `srgn`, the "code surgeon," is designed to
fill.[^1] It operates as a powerful hybrid, blending the regex-based pattern
matching of `grep`, the stream-editing capabilities of `tr` and `sed`, and the
syntactic intelligence of the `tree-sitter` parsing framework.[^1]

`srgn` complements traditional tools by operating on a different "dimension" of
code analysis.[^1] It is not a replacement for full-featured IDE refactoring
engines but a specialized scalpel for tasks that are too complex for a simple
regex and too specific for a generic IDE command.

The ideal use case for `srgn` emerges when a refactoring task requires
precision that text-based tools cannot provide, yet falls outside the scope of
standard IDE functions like "Rename All" or "Find All References." For example,
a global regex replacement to change a function call `foo()` to `bar()` might
incorrectly alter variable names like `my_foo` or text within comments.
Conversely, an IDE's rename function operates on a specific symbol's definition
and usages but cannot execute a rule-based transformation, such as "convert all
top-level `print()` calls to `logging.debug()`." `srgn` excels at this kind of
precise, rule-based, cross-file surgery, making it a strategic asset for
enforcing coding standards, executing targeted API migrations, and performing
complex cleanups.

### 1.2 Disambiguation: Identifying the Correct srgn

To ensure clarity, it is essential to acknowledge that the name "srgn" is
overloaded across different domains. This guide is exclusively dedicated to
`alexpovel/srgn`, the command-line code search and manipulation utility.[^1]
Other projects bearing a similar name are unrelated to the tool discussed here.
These include, but are not limited to, SRGAN, a Generative Adversarial Network
for image super-resolution 3; SRGN, a high-energy physics technique for
parameter estimation 4; and SRGN (SolRagon), a cryptocurrency token.[^6] This
report focuses solely on the code refactoring tool.

### 1.3 Core Philosophy: Scopes, Actions, and Intentional Simplicity

The design of `srgn` is built upon two foundational pillars: **Scopes** and
**Actions**.[^2] Scopes define

*where* in the code an operation should take place, while Actions define *what*
should be done to the text within that scope. This separation of concerns is
central to the tool's power and usability.

A core tenet of `srgn` is its intentional simplicity. The documentation states
its design goal clearly: "if you know regex and the basics of the language you
are working with, you are good to go".[^2] This philosophy distinguishes

`srgn` from other advanced code-querying tools. While tools like Semgrep use a
declarative, template-based syntax with metavariables (`$X`) and ellipses
(`...`) to find code that matches an abstract pattern 8,

`srgn` employs a more direct approach.

`srgn` does not use a proprietary structural pattern language. Instead, it
functions as a highly precise location filter. It answers the question, "Find
text matching this regex, but only at *this kind of location* (e.g., inside a
Python class definition)." This is fundamentally different from a tool that
answers, "Find code that *looks like this abstract pattern*." `srgn`'s power
derives from its compositional filtering model—layering predefined grammar
queries and user-supplied regular expressions—rather than from a complex,
abstract query language. This design choice makes its mechanisms transparent
and its learning curve gentle for anyone already comfortable with the command
line.

## Part 2: Getting Started - Installation and First Cuts

### 2.1 Installation: Preparing the Operating Theater

`srgn` can be installed across various platforms, catering to the diverse
environments of command-line users. The following methods are officially
supported 1:

- **Prebuilt Binaries**: The most straightforward method is to download a
  prebuilt binary for your specific architecture directly from the project's
  GitHub Releases page.[^1]

- `cargo-binstall`: For users with the Rust toolchain, this is the recommended
  installation method. It is significantly faster than compiling from source as
  it downloads prebuilt binaries when available. It is tested in the project's
  CI and serves as a reliable installation vector.[^1]

  Bash

  ```sh
  # Install the Rust toolchain if you haven't already
  # Then, install cargo-binstall
  cargo install cargo-binstall
  # Finally, install srgn
  cargo binstall srgn
  
  ```

- `cargo install`: The traditional method of compiling from source using Rust's
  package manager. This requires a C compiler to be present on the system
  (`gcc` on Linux, `clang` on macOS, or MSVC on Windows).[^1]

  Bash

  ```sh
  cargo install srgn
  
  ```

- **Package Managers**: `srgn` is available through several system package
  managers, offering convenient installation and updates 1:

  - **Homebrew (macOS/Linux):** `brew install srgn`

  - **Nix (NixOS/Linux/macOS):** `nix-shell -p srgn`

  - **Arch Linux:** Available via the AUR (Arch User Repository).

  - **MacPorts (macOS):** `sudo port install srgn`

For integration into automated workflows, a GitHub Action is available for
`cargo-binstall`, allowing for easy installation of `srgn` in CI/CD
pipelines.[^1]

### 2.2 The Anatomy of a srgn Command

The fundamental structure of a `srgn` command is analogous to familiar Unix
tools, making it intuitive for experienced users. The general syntax is:

`srgn '' -- ''`

Each component has a distinct role, as illustrated by the canonical `tr`-like
example from the documentation 1:

Bash

```sh
echo 'Hello World!' | srgn '[wW]orld' -- 'there'
# Output: Hello there!
```

- \`\`: These are flags that specify Actions (e.g., `--upper`, `--delete`) or
  language-aware grammar Scopes (e.g., `--python`, `--rust`).

- `''`: This is the mandatory, positional regular expression that defines the
  final layer of text to be matched. In the example, it's `'[wW]orld'`.

- \`\`: These are optional file or directory paths. If omitted, `srgn` reads
  from standard input (`stdin`). If a directory is provided, `srgn` performs a
  high-speed, recursive search for relevant files based on extensions and
  shebangs.[^1]

- `-- ''`: This is the optional replacement string. The `--` separator is a
  critical safety feature that disambiguates the replacement string from file
  paths or other arguments, especially when the replacement itself might
  resemble a flag.[^1]

If no replacement string or action flags are provided, `srgn` may enter its
"search mode," which transforms it into a powerful, syntax-aware search
tool.[^1]

### 2.3 Search Mode: ripgrep with Syntactic Superpowers

When a language flag (e.g., `--python` or its shorthand `--py` 9) is provided
without any accompanying actions or a replacement string,

`srgn` enters search mode.[^1] The documentation describes this mode as
"'ripgrep but with syntactical language elements'".[^2]

For instance, to find all class definitions in a Python project, one could run:

Bash

```sh
srgn --python 'class'.
```

The output mimics `grep` and `ripgrep`, prepending the file name and line
number to each match, making it easy to integrate into standard command-line
workflows.[^2]

This mode is not only precise but also exceptionally fast. A benchmark cited in
the documentation demonstrates its performance: `srgn` can find approximately
140,000 occurrences of a regex pattern within Go string literals across the
entire Kubernetes codebase (\~3 million lines of code) in under 3 seconds on a
modern multi-core machine.[^1] This combination of speed and syntactic
precision makes search mode a formidable tool for code exploration and auditing.

## Part 3: The Core Concept - Surgical Scoping

### 3.1 What srgn Means by 'Scope': Textual Regions, Not Semantic Namespaces

The term "scope" carries significant weight in programming, often referring to
semantic concepts of visibility and lifetime, such as Python's LEGB rule
(Local, Enclosing, Global, Built-in) or Rust's complex ownership and lifetime
scopes.[^10] A critical step in mastering

`srgn` is understanding that its use of the term is different.

In `srgn`, a "language grammar-aware scope" does not refer to a semantic
namespace but to a **textual region** of the source code that corresponds to a
specific node in its Abstract Syntax Tree (AST), as parsed by
`tree-sitter`.[^2] For example, the

`--python 'function'` scope selects the entire block of text that constitutes a
function definition, from the `def` keyword to the end of its body. It does not
understand which variables are accessible within that function.

This distinction is paramount. `srgn` operates on the code's grammatical
structure, not its compiled or interpreted meaning. It can identify all
comments, all string literals, or all function definitions, but it cannot
resolve a variable name to its declaration. This focus on syntactic structure
is the source of its speed and simplicity, but it also defines the boundaries
of its capabilities.

### 3.2 The Scoping Pipeline: Layering with Logical AND

The precision of `srgn` comes from its default mechanism of combining scopes: a
left-to-right, progressively narrowing filter that acts as a logical AND.[^2]
Each subsequent scope operates only on the text that was passed through by the
previous one.

Consider the following command:

Bash

```sh
# Find all occurrences of 'github.com' but only inside docstrings of Python classes.
srgn --python 'class' --python 'doc-strings' 'github\.com' my_project/
```

The execution pipeline for this command is as follows:

1. **Initial Scope**: `srgn` first parses all files in `my_project/` and
   identifies the textual regions of all `class` definitions. All other code is
   discarded from consideration.

2. **Intersection**: *Within the text of the class definitions only*, it then
   identifies all regions corresponding to `doc-strings`.

3. **Final Match**: Finally, *within the text of those docstrings only*, it
   applies the regex `'github\.com'` to find the ultimate matches.

This directional, filtering nature means the order of scopes is crucial. The
documentation provides a clear example of a nonsensical query,
`srgn --python 'doc-strings' --python 'class'`, which would attempt to find a
class definition *inside* a docstring and would almost certainly return no
results.[^1] This illustrates the power and predictability of the
intersectional pipeline.

### 3.3 Broadening the Search: Joining Scopes with Logical OR

While the default AND logic is excellent for drilling down, some tasks require
a broader search across different types of syntax. For this, `srgn` provides
the `--join-language-scopes` flag (or its shorthand, `-j`).[^2] This flag
alters the behavior for language scopes, changing the operation from
intersection (AND) to a union (OR).

A practical example from the release notes demonstrates its utility 9:

Bash

```sh
# Find all TODOs, whether they are in comments or docstrings.
srgn -j --python comments --python doc-strings 'TODO:' src/
```

Without the `-j` flag, this command would nonsensically search for docstrings
*inside* of comments. With `-j`, it creates a combined scope of all text that
is *either* a comment *or* a docstring, and then applies the `'TODO:'` regex to
that combined set. This is a common and powerful pattern for code maintenance
tasks.

### 3.4 The Two Fundamental Scope Types

To summarize, all `srgn` operations are built from two fundamental types of
scopes:

1. **Language Grammar Scopes**: These are the predefined syntactic elements
   specified with the `--<LANG> '<SCOPE_NAME>'` syntax (e.g.,
   `--python 'class'`, `--rust 'unsafe'`). They leverage `tree-sitter` to
   provide the foundational context awareness that sets `srgn` apart.[^1] A
   reference list of known scopes is provided in the Appendix.

2. **Regular Expression Scope**: This is the mandatory, positional argument
   that provides the final, fine-grained pattern matching. It is always the
   last filter applied in the pipeline, operating only on the text selected by
   the preceding language scopes.[^2]

## Part 4: Taking Action - Manipulation and Refactoring

### 4.1 Simple and Dynamic Replacement

The simplest action in `srgn` is replacement, specified with the
`-- 'replacement'` syntax. However, for any meaningful refactoring, dynamic
replacements are essential. `srgn` supports this through regex capture groups
(`$1`, `$2`, etc.), which substitute parts of the matched text into the
replacement string.[^2]

A rich example from the documentation showcases several advanced features at
once 2:

Bash

```sh
srgn --python 'doc-strings' '(?<!The )GNU ([a-z]+)' -- '$1: GNU 🐂 is not Unix'
```

This command deconstructs as follows:

- `--python 'doc-strings'`: The operation is scoped exclusively to Python
  docstrings.

- `'(?<!The )GNU ([a-z]+)'`: The regex scope uses a negative lookbehind
  `(?<!...)` to match the word "GNU" only when it is not preceded by "The ". It
  then captures the following lowercase word (e.g., "is") into group 1.

- `-- '$1: GNU 🐂 is not Unix'`: The replacement string uses `$1` to substitute
  the captured word. This example also demonstrates `srgn`'s full Unicode
  support.

### 4.2 Chaining Actions: A Multi-Stage Process

Beyond simple replacement, `srgn` offers a suite of built-in actions specified
via command-line flags. These actions are applied in a defined order *after*
the main replacement has occurred.[^2]

The command `srgn --upper '[wW]orld' -- 'you'` illustrates this two-stage
process. First, the regex match `World` is replaced with `you`. Second, the
`--upper` action is applied to that result, yielding the final output `YOU`.[^2]

Common built-in action flags include:

- `--upper`, `--lower`, `--titlecase`: For changing the case of matched
  text.[^2]

- `--delete`: Removes the matched text. As a safety measure, this action will
  produce an error if no scope is specified, preventing the accidental deletion
  of an entire file's content.[^1]

- `--squeeze`: Collapses sequences of whitespace. Like `--delete`, this
  requires an explicit scope.[^1]

- `--german`: A specialized action that correctly handles German orthography,
  such as converting "Ueberflieger" to "Überflieger," demonstrating the
  potential for domain-specific transformations.[^7]

### 4.3 In-place File Modification and Operational Safety

To apply changes directly to files on disk, one can provide a path to `srgn`
instead of piping from `stdin`. For more complex file selections, the `--glob`
option accepts a glob pattern.[^1]

It is crucial to heed the official documentation's warning: `srgn` is currently
in beta (major version 0). **Any in-place modifications should only be
performed on files that are safely under version control**.[^1]

To mitigate risk, the `--dry-run` flag is an indispensable safety feature.[^9]
When used,

`srgn` will print a `diff`-like output of the changes it *would* make without
modifying any files on disk. This allows for a complete preview of the
operation's impact before committing to the changes.

## Part 5: Real-World Recipes for Python Wizards

The following recipes demonstrate how to solve common Python refactoring
challenges by combining `srgn`'s scoping and action capabilities.

### 5.1 Simple Task: Renaming an Imported Module

- **Problem**: A core utility module, `old_utils`, has been renamed to
  `new_core_utils`. All `import old_utils` and `from old_utils import...`
  statements across the entire codebase must be updated.

- **Command**:

  Bash

  ```sh
  srgn --py 'module-names-in-imports' '^old_utils$' -- 'new_core_utils' src/
  
  ```

- **Explanation**: This command's precision comes from the
  `'module-names-in-imports'` grammar scope, a feature highlighted in the
  project's release notes.[^9] This scope surgically targets only the module
  names within

  `import` and `from... import` statements, completely avoiding the risk of
  altering variables or strings that happen to contain the text `old_utils`.
  The regex anchors (`^` and `$`) ensure that only the exact module name is
  replaced, preventing unintended changes to modules like `old_utils_extra`.

### 5.2 Intermediate Task: Converting `print` Calls to Structured Logging

- **Problem**: A legacy section of the codebase uses `print(f"...")` statements
  for debugging. These need to be converted to structured `logging.info(...)`
  calls to integrate with a centralized logging system.

- **Command**:

  Bash

  ```sh
  srgn --py 'call' '^print\((.*)\)$' -- 'logging.info($1)'. --dry-run
  
  ```

- **Explanation**: This recipe leverages the `'call'` grammar scope to identify
  function call expressions. The regex `^print\((.*)\)$` is designed to match
  the entire `print(...)` expression, capturing all of its arguments into the
  first capture group (`$1`). The replacement string then reconstructs the call
  using `logging.info($1)`, effectively swapping the function while preserving
  the arguments. The `--dry-run` flag is used to safely preview the widespread
  changes before applying them. It is important to recognize that this is a
  powerful *syntactic* transformation. It will not automatically add
  `import logging` to the top of files that lack it. This highlights `srgn`'s
  role as a surgical tool that often works in concert with other scripts or
  manual developer intervention.

### 5.3 Advanced Task: Finding Functions That Lack Docstrings

- **Problem**: As part of a new code quality initiative, all functions must
  have a docstring. The first step is to find every function definition that is
  not immediately followed by one.

- **Command**:

  Bash

  ```sh
  srgn --py 'function' 'def\s+\w+\(.*\):\n\s+[^"''#\s]'.
  
  ```

- **Explanation**: This sophisticated search-only operation, based on an
  example from the documentation 2, demonstrates the powerful synergy between
  grammar scopes and advanced regex.

  1. `--py 'function'`: The search is first narrowed to the complete text of
     all function definitions.

  2. `'def\s+\w+\(.*\):\n\s+[^"''#\s]'`: This multi-line regex is then applied.
     It looks for a `def` signature followed by a newline and indentation
     (`\n\s+`). The crucial part is the negative character class `[^"''#\s]`,
     which matches any character that is *not* a double quote, a single quote,
     a comment hash, or whitespace. If this pattern matches the first
     non-whitespace character after the function signature, it means the first
     statement in the body is code, not a docstring, and the function is
     flagged as a match.

## Part 6: Real-World Recipes for Rust Wizards

These recipes address refactoring tasks specific to the Rust ecosystem,
showcasing `srgn`'s versatility across different languages.

### 6.1 Simple Task: Upgrading Lint Attributes from `allow` to `expect`

- **Problem**: To improve code quality and prevent stale lint suppressions,
  temporary `#[allow(some_lint)]` attributes should be upgraded to
  `#[expect(some_lint)]`. This ensures that if the underlying code is fixed and
  no longer triggers the lint, the build will fail, forcing the removal of the
  now-unnecessary attribute. This exact use case is mentioned as an example in
  the `srgn` documentation.[^2]

- **Command**:

  Bash

  ```sh
  srgn --rs 'attribute' 'allow\((clippy::some_lint)\)' -- 'expect($1)' src/
  
  ```

- **Explanation**: This recipe uses the `'attribute'` scope to focus the search
  exclusively within `#[...]` blocks. The regex `allow\((clippy::some_lint)\)`
  matches the `allow` attribute for a specific lint and captures the lint's
  path into group 1. The replacement string then reuses this captured path with
  `expect($1)`, performing a precise and safe upgrade.

### 6.2 Intermediate Task: Auditing and Annotating `unsafe` Code

- **Problem**: A security audit requires that every `unsafe` block or function
  in the codebase be justified with a comment linking to a tracking ticket.

- **Command**:

  Bash

  ```sh
  srgn --rs 'unsafe' 'unsafe' -- '// TODO: Justify this unsafe block\nunsafe'.
  
  ```

- **Explanation**: This demonstrates a replacement that prepends text. The
  `'unsafe'` scope, a feature noted in a release update 9, correctly identifies
  both

  `unsafe fn` definitions and `unsafe {... }` blocks—a task that would be
  difficult and error-prone with a simple text search. The command finds every
  instance of the `unsafe` keyword within this scope and replaces it with a
  comment, a newline, and the original keyword, effectively annotating every
  unsafe usage point.

### 6.3 Advanced Task: Mass Crate Renaming in `use` Declarations

- **Problem**: A foundational crate within a large workspace, `old_api`, has
  been refactored and republished under the new name `new_api`. All `use`
  statements across dozens of member crates must be updated.

- **Command**:

  Bash

  ```sh
  srgn --rs 'names-in-uses-declarations' '^old_api' -- 'new_api'.
  
  ```

- **Explanation**: This operation's surgical precision is enabled by the
  `'names-in-uses-declarations'` scope, a powerful feature documented in the
  release notes.[^9] This scope targets

  *only* the paths inside `use...;` statements. It will correctly change
  `use old_api::prelude::*;` to `use new_api::prelude::*;` and
  `use old_api::{Foo, Bar};` to `use new_api::{Foo, Bar};` without any risk of
  incorrectly altering a local variable, struct, or comment that happens to
  contain the name `old_api`. This recipe is a clear demonstration of `srgn`'s
  core value proposition: providing syntactic context that regular expressions
  alone cannot.

## Part 7: The Next Level - srgn as a Rust Library

### 7.1 Programmatic Refactoring for Ultimate Control

For the most demanding refactoring tasks, `srgn` offers an escape hatch beyond
the command line. It is a dual-use tool, available not only as a binary but
also as a Rust library that can be added to a project with `cargo add srgn`.[^7]

This library interface provides the ultimate level of control for power users.
For extremely complex, multi-pass, or stateful refactoring scenarios where the
CLI's linear pipeline is insufficient, one can leverage `srgn`'s battle-tested
parsing and scoping engine directly within a custom Rust program. This opens
the door to building bespoke `cargo` subcommands, sophisticated build scripts,
or standalone code-mod utilities tailored to a project's specific needs. This
capability elevates `srgn` from a mere utility to a foundational component for
building higher-level developer tooling.

### 7.2 A Glimpse into the Library API

While a full library tutorial is beyond this guide's scope, a brief look at the
core API concepts reveals its ergonomic design. The official `docs.rs` page
provides several end-to-end examples that revolve around a few key types 7:

- `ScopedViewBuilder`: This is the entry point for all operations. It is
  initialized with the input source code: `ScopedViewBuilder::new(input)`.

- **Scopers**: Scopes are applied to the builder to narrow the view. This can
  be a regex scoper or a language grammar scoper built from a `PreparedQuery`:
  `builder.explode(&scoper)`.

- **Actions**: Actions, which implement the `Action` trait, are then mapped
  over the resulting view to perform the manipulation:
  `view.map_without_context(&action)`.

This programmatic interface allows for intricate logic, such as applying
different actions to different scopes within the same file or making decisions
based on the content of a match. For developers whose needs exceed the CLI, the
`srgn` library is the definitive path forward.

## Part 8: Conclusion - The Right Tool for the Right Cut

`srgn` is a specialized, high-precision instrument that fills a crucial niche
in the modern developer's command-line toolkit. By combining the familiarity of
regular expressions with the structural understanding of a language parser, it
enables a class of code search and refactoring tasks that are too nuanced for
`grep` and too specific for an IDE.

To effectively integrate `srgn` into a workflow, it is helpful to use the
following decision-making heuristic:

- **Use** `grep`**/**`ripgrep` **for**: Simple, context-free, read-only
  searching across files.

- **Use** `sed`**/**`awk` **for**: Simple, line-oriented, context-free
  replacements on text streams.

- **Use your IDE for**: Standard, semantic-aware refactorings that require full
  program understanding, such as renaming a variable and all its usages, or
  extracting a method.

- **Use** `srgn` **for**: Complex, rule-based, cross-file search and replace
  that requires syntactic context. This is the tool for tasks like, "replace
  `foo` with `bar`, but only inside function signatures and not inside string
  literals," or "find all `unsafe` blocks that are not preceded by a specific
  comment."

By understanding its unique position and capabilities, developers can wield
`srgn` as a surgical tool, performing precise, safe, and repeatable
modifications that would otherwise be tedious and error-prone.

---

## Appendix: Grammar Scope Reference

### A.[^1] A Note on This List

The following tables list the known language grammar scopes for Python and
Rust. This reference has been meticulously compiled from the official `srgn`
documentation, README examples, and GitHub release notes.[^2] As direct
inspection of the

`PreparedQuery` source enum was not possible during research 15, this list
should be considered comprehensive but potentially subject to change in future

`srgn` versions. Users can often discover available scopes by providing an
invalid one, as `srgn` will helpfully list the valid options.[^9]

### A.[^2] Table: Python Grammar Scopes (`--python <SCOPE>` or `--py <SCOPE>`)

| Scope Name              | Description                                                               | Example Command                               |
| ----------------------- | ------------------------------------------------------------------------- | --------------------------------------------- |
| class                   | Selects entire class definitions, from class to the end of the block.     | srgn --py 'class' 'MyClass'                   |
| function                | Selects entire function definitions, from def to the end of the block.    | srgn --py 'function' 'my_func'                |
| doc-strings             | Selects the content of docstrings ("""...""" or '''...''').               | srgn --py 'doc-strings' 'TODO'                |
| comments                | Selects the content of line comments (#...).                              | srgn --py 'comments' 'FIXME'                  |
| strings                 | Selects the content of all string literals.                               | srgn --py 'strings' 'hardcoded-secret'        |
| identifiers             | Selects language identifiers (variable names, function names, etc.).      | srgn --py 'identifiers' '^temp_\w+'           |
| module-names-in-imports | Selects only the module names in import and from... import statements.    | srgn --py 'module-names-in-imports' 'old_lib' |
| call                    | Selects entire function or method call expressions (e.g., foo(bar, baz)). | srgn --py 'call' '^print\('                   |

### A.[^3] Table: Rust Grammar Scopes (`--rust <SCOPE>` or `--rs <SCOPE>`)

| Scope Name                 | Description                                                    | Example Command                                        |
| -------------------------- | -------------------------------------------------------------- | ------------------------------------------------------ |
| unsafe                     | Selects unsafe blocks and unsafe function definitions.         | srgn --rs 'unsafe' '.'                                 |
| comments                   | Selects the content of line (//) and block (/*...*/) comments. | srgn --rs 'comments' 'HACK'                            |
| strings                    | Selects the content of all string literals.                    | srgn --rs 'strings' 'password'                         |
| attribute                  | Selects the content of attributes (#[...] and #![...]).        | srgn --rs 'attribute' 'deprecated'                     |
| names-in-uses-declarations | Selects only the crate/module paths within use statements.     | srgn --rs 'names-in-uses-declarations' 'old_crate'     |
| pub-enum                   | Selects public enum definitions.                               | srgn --rs 'pub-enum' 'MyEnum'                          |
| type-identifier            | Selects identifiers that refer to a type.                      | srgn --rs 'pub-enum' --rs 'type-identifier' 'Subgenre' |
| struct                     | Selects struct definitions.                                    | srgn --rs 'struct' 'RequestPayload'                    |
| impl                       | Selects impl blocks.                                           | srgn --rs 'impl' 'MyTrait for MyStruct'                |
| fn                         | Selects function definitions.                                  | srgn --rs 'fn' 'main'                                  |
| extern-crate               | Selects extern crate...; declarations.                         | srgn --rs 'extern-crate' 'libc'                        |

## Works Cited

 1. alexpovel/srgn: A grep-like tool which understands source code syntax and
    allows for manipulation in addition to search - GitHub, accessed on July
    11, 2025, <https://github.com/alexpovel/srgn>

 2. srgn/[README.md](http://README.md) at main · alexpovel/srgn · GitHub,
    accessed on July 11, 2025,
    <https://github.com/alexpovel/srgn/blob/main/README.md>

 3. Lornatang/SRGAN-PyTorch: A simple and complete implementation of
    super-resolution paper. - GitHub, accessed on July 11, 2025,
    <https://github.com/Lornatang/SRGAN-PyTorch>

 4. hep-lbdl/SRGN - GitHub, accessed on July 11, 2025,
    <https://github.com/hep-lbdl/SRGN>

 5. Security - hep-lbdl/SRGN - GitHub, accessed on July 11, 2025,
    <https://github.com/hep-lbdl/SRGN/security>

 6. How to Open and Manage Leveraged $SRGN (SolRagon) Trades on Hyperliquid: A
    Beginner's Tutorial · Issue #5 · synthesizearrayHSy/generatemonitorGhZ -
    GitHub, accessed on July 11, 2025,
    <https://github.com/synthesizearrayHSy/generatemonitorGhZ/issues/5>

 7. srgn - Rust - [Docs.rs](http://Docs.rs), accessed on July 11, 2025,
    <https://docs.rs/srgn>

 8. Pattern syntax - Semgrep, accessed on July 11, 2025,
    <https://semgrep.dev/docs/writing-rules/pattern-syntax>

 9. Releases · alexpovel/srgn - GitHub, accessed on July 11, 2025,
    <https://github.com/alexpovel/srgn/releases>

10. Python Scope & the LEGB Rule: Resolving Names in Your Code, accessed on
    July 11, 2025, <https://realpython.com/python-scope-legb-rule/>

11. Scopes - The Rust Reference, accessed on July 11, 2025,
    <https://doc.rust-lang.org/reference/names/scopes.html>

12. I can't understand the Rust "scope" definition (Rust Programming Language,
    2nd Ed. Klabnik & Nichols) - Stack Overflow, accessed on July 11, 2025,
    <https://stackoverflow.com/questions/77423163/i-cant-understand-the-rust-scope-definition-rust-programming-language-2nd-e>

13. betterletter/[README.md](http://README.md) at main · alexpovel/betterletter
    · GitHub, accessed on July 11, 2025,
    <https://github.com/alexpovel/betterletter/blob/main/README.md>

14. srgn - Rust Package Registry - [Crates.io](http://Crates.io), accessed on
    July 11, 2025, <https://crates.io/crates/srgn/>

15. accessed on January 1, 1970,
    <https://github.com/alexpovel/srgn/tree/main/src/scoping/langs>

16. accessed on January 1, 1970,
    <https://github.com/alexpovel/srgn/blob/main/src/scoping/langs/rust.rs>
