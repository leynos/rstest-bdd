# rstest-bdd-patterns

Shared step-pattern compilation utilities used by both the
[`rstest-bdd`](https://crates.io/crates/rstest-bdd) runtime crate and the
[`rstest-bdd-macros`](https://crates.io/crates/rstest-bdd-macros) procedural
macros. The crate exposes the placeholder parsing and regex generation engine
so both dependants can validate and execute Behaviour-Driven Development steps
without duplicating logic.

The API is intentionally small: it provides helpers for turning annotated step
literals into regular expressions, along with supporting error types and
placeholder parsing utilities. Downstream crates wrap these primitives in
user-facing APIs tailored to their compile-time or runtime responsibilities.

## Usage

```rust
use regex::Regex;
use rstest_bdd_patterns::{build_regex_from_pattern, extract_captured_values};

let regex_src = build_regex_from_pattern("I have {count:u32}")
    .expect("example ensures fallible call succeeds");
let regex = Regex::new(&regex_src)
    .expect("example ensures fallible call succeeds");
let captures = extract_captured_values(&regex, "I have 3")
    .expect("example ensures fallible call succeeds");
assert_eq!(captures, vec!["3".to_string()]);
```
