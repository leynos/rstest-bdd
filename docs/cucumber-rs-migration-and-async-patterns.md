# Cucumber-rs migration and async step patterns

## Purpose

This document provides the canonical migration and async execution guidance for
`rstest-bdd`. It consolidates the cucumber-rs compatibility notes and the
recommended async step strategy to reduce drift between documentation sources.

## Cucumber-rs migration patterns

The step macros accept cucumber-rs style `expr = "..."` attributes for easier
migration. The direct string literal form remains preferred for new code
because it is shorter and clearer.

```rust,no_run
use rstest_bdd::{given, when, then};

// cucumber-rs style (supported for migration):
#[given(expr = "an empty basket")]
fn empty_basket(basket: &mut Basket) {
    basket.clear();
}

// rstest-bdd style (preferred for new code):
#[given("an empty basket")]
fn empty_basket_alt(basket: &mut Basket) {
    basket.clear();
}
```

## Async step execution pattern

Async scenarios run on Tokio's current-thread runtime. Step functions remain
synchronous, so async work should be moved into fixtures or the scenario body.
Use per-step runtimes only in synchronous scenarios to avoid nested runtime
failures. ADR-005 captures the rationale and risk profile.

```rust,no_run
use rstest::fixture;
use rstest_bdd_macros::{given, scenarios, when};

struct StreamEnd;

impl StreamEnd {
    async fn connect() -> Self {
        StreamEnd
    }

    fn trigger(&self) {}
}

#[fixture]
async fn stream_end() -> StreamEnd {
    StreamEnd::connect().await
}

#[when("the stream ends")]
fn end_stream(stream_end: &StreamEnd) {
    stream_end.trigger();
}

scenarios!(
    "tests/features/streams.feature",
    runtime = "tokio-current-thread",
    fixtures = [stream_end]
);
```
