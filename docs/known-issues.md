# Known issues

## rustc internal compiler error (ICE) with mutable world macro

- **Status:** Open; gated by the `mutable_world_macro` Cargo feature.
- **Affected toolchains:** Rust toolchains that still trigger the ICE. This
  repository pins a toolchain in `rust-toolchain.toml` to keep contributor
  behaviour consistent.
- **Symptom:** Enabling `--features mutable_world_macro` and compiling the
  macro-driven test `tests/mutable_world_macro.rs` triggers a rustc internal
  compiler error (ICE) during macro expansion.
- **Reproduction:**

  ```bash
  cargo test -p rstest-bdd --features mutable_world_macro \
    -- tests::macro_world::mutable_world
  ```

- **Workaround:** The scenario is mirrored by the context-level regression
  test in `crates/rstest-bdd/tests/mutable_fixture.rs`, which avoids the macro
  path that currently triggers the compiler bug. The macro-driven test is
  guarded behind the feature flag until the upstream issue is resolved.
- **Next steps:** Once an upstream rustc issue is filed, update this section
  with the issue number and remove the feature gate when the fix ships.

## Async step functions in async scenarios

- **Status:** Resolved (2026-01-30); async step bodies run natively under async
  scenario runtimes.
- **Affected usage:** `scenarios!` with `runtime = "tokio-current-thread"` and
  `#[scenario]` combined with `#[tokio::test(flavor = "current_thread")]`.
- **Behaviour:** Step functions may now be `async fn`. Async scenarios execute
  each step by awaiting the registered async handler, keeping fixture borrows
  valid across `.await` points.
- **Notes:** Async step functions remain Tokio current-thread only. When an
  async-only step runs under a synchronous scenario, `rstest-bdd` falls back to
  a per-step Tokio runtime and will refuse to do so if a Tokio runtime is
  already running on the current thread.
