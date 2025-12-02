# Known issues

## rustc ICE with mutable world macro

- **Status:** Open; gated by the `mutable_world_macro` Cargo feature.
- **Affected toolchains:** nightly-2025-09-16 (the pinned toolchain in
  `rust-toolchain.toml`).
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
