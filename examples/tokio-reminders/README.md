# Tokio reminders example

This example crate demonstrates writing behaviour-driven development (BDD)
tests that exercise `TokioHarness` and `TokioAttributePolicy` from
`rstest-bdd-harness-tokio`. The scenarios model a small reminder service that
queues local Tokio tasks, while the unit tests and doctest validate explicit
`flush().await` coordination.

## Running the tests

Execute the example suite with:

```bash
cargo test -p tokio-reminders
```

The BDD scenarios live in `tests/features/reminders.feature`. Step definitions
in `tests/reminders.rs` demonstrate:

- Binding scenarios with both `harness = TokioHarness` and
  `attributes = TokioAttributePolicy`.
- Using immediate-ready `async fn` step definitions under `TokioHarness`.
- Observing queued reminder state without requiring a multi-poll async step.

Unit tests and the doctest for `ReminderService` live in `src/lib.rs`. They
show the explicit `flush().await` pattern needed when work must complete before
assertions run.
