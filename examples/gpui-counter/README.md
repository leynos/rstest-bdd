# GPUI counter example

This example crate demonstrates writing behaviour-driven development (BDD)
tests with the first-party GPUI harness adapter from `rstest-bdd-harness-gpui`.
The scenarios rely on harness-led attribute-policy defaults, model a simple
counter application, and observe GPUI-injected `TestAppContext` details from
within step definitions.

## Running the tests

Execute the test suite with:

```bash
cargo test -p gpui-counter
```

The BDD scenarios live in `tests/features/counter.feature`. Step definitions in
`tests/counter.rs` demonstrate:

- Binding first-party GPUI scenarios with `harness = GpuiHarness` alone.
- Relying on the macro to infer `GpuiAttributePolicy` from the first-party
  harness path.
- Accessing the injected `gpui::TestAppContext` through the
  `#[from(rstest_bdd_harness_context)]` fixture key.
- Recording harness context observations (e.g. `TestAppContext` availability)
  in the example's own domain model.

Unit tests for the `CounterApp` domain model live in `src/lib.rs`.
