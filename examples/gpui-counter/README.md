# GPUI counter example

This example crate demonstrates writing behaviour-driven tests that exercise
the GPUI harness adapter and attribute policy from `rstest-bdd-harness-gpui`.
The scenarios model a simple counter application while also observing
GPUI-injected `TestAppContext` details from within step definitions.

## Running the tests

Execute the test suite with:

```bash
cargo test -p gpui-counter
```

The BDD scenarios live in `tests/features/counter.feature`. Step definitions in
`tests/counter.rs` demonstrate:

- Binding scenarios with both `harness = GpuiHarness` and
  `attributes = GpuiAttributePolicy`.
- Accessing the injected `gpui::TestAppContext` through the
  `#[from(rstest_bdd_harness_context)]` fixture key.
- Recording harness context observations (e.g. dispatcher seed) in the
  example's own domain model.

Unit tests for the `CounterApp` domain model live in `src/lib.rs`.
