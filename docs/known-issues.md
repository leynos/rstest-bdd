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

- **Status:** Open; steps are synchronous even under async scenario runtimes.
- **Affected usage:** `scenarios!` with `runtime = "tokio-current-thread"` and
  `#[scenario]` combined with `#[tokio::test(flavor = "current_thread")]`.
- **Symptom:** Step functions cannot be `async fn`. Attempting to create a
  per-step Tokio runtime inside the scenario runtime can fail with nested
  runtime errors.
- **Workaround:** Keep steps synchronous, move async work into fixtures or the
  scenario test body, and only use per-step runtimes when the scenario itself
  is synchronous. See [ADR-005](adr-005-async-step-functions.md) for the
  current strategy.
- **Next steps:** Once the StreamEnd and CodecStateful migrations land, keep
  this section aligned with ADR-005 and record any migration learnings here.
