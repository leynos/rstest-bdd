# Tokio reminders example

This example crate demonstrates writing behaviour-driven development (BDD)
tests with the first-party `TokioHarness` from `rstest-bdd-harness-tokio`. The
scenarios rely on harness-led attribute-policy defaults, model a small reminder
service that queues local Tokio tasks, and leave explicit `flush().await`
coordination to the unit tests and doctest.

## Running the tests

Execute the example suite with:

```bash
cargo test -p tokio-reminders
```

The BDD scenarios live in `tests/features/reminders.feature`. Step definitions
in `tests/reminders.rs` demonstrate:

- Binding first-party Tokio scenarios with `harness = TokioHarness` alone.
- Relying on the macro to infer `TokioAttributePolicy` from the first-party
  harness path.
- Using immediate-ready `async fn` step definitions under `TokioHarness`.
- Observing queued reminder state without requiring a multi-poll async step.

Unit tests and the doctest for `ReminderService` live in `src/lib.rs`. They
show the explicit `flush().await` pattern needed when work must complete before
assertions run.
