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
