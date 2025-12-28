//! Tests for step context and fixture management.

use super::*;
use std::sync::Once;

struct NoopLogger;

impl log::Log for NoopLogger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, _: &log::Record<'_>) {}
    fn flush(&self) {}
}

static LOGGER: NoopLogger = NoopLogger;
static INIT_LOGGER: Once = Once::new();

/// Fixture that initializes the logger for tests requiring log output.
///
/// Uses `Once` to ensure the logger is set exactly once across all tests.
/// Inject this fixture to ensure logging is available during test execution.
#[rstest::fixture]
fn logger() {
    INIT_LOGGER.call_once(|| {
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Warn);
    });
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "downcast must succeed for the typed fixture under test"
)]
fn borrow_mut_returns_mutable_fixture() {
    let cell: RefCell<Box<dyn Any>> = RefCell::new(Box::new(String::from("seed")));
    let mut ctx = StepContext::default();
    ctx.insert_owned::<String>("text", &cell);

    {
        let Some(mut value) = ctx.borrow_mut::<String>("text") else {
            panic!("mutable fixture should exist");
        };
        value.as_mut().push_str("ing");
    }
    drop(ctx);
    let value = cell
        .into_inner()
        .downcast::<String>()
        .expect("fixture should downcast to String");
    assert_eq!(*value, "seeding");
}

#[test]
fn borrow_mut_returns_none_for_shared_fixture() {
    let fixture = 5;
    let mut ctx = StepContext::default();
    ctx.insert("number", &fixture);
    assert!(ctx.borrow_mut::<i32>("number").is_none());
}

#[rstest::rstest]
#[expect(clippy::expect_used, reason = "tests require explicit panic messages")]
#[expect(
    clippy::used_underscore_binding,
    reason = "rstest fixture injection requires the parameter"
)]
fn insert_value_overrides_unique_fixture(_logger: ()) {
    let fixture = 1u32;
    let mut ctx = StepContext::default();
    ctx.insert("number", &fixture);

    let first = ctx.insert_value(Box::new(5u32));
    assert!(
        first.is_none(),
        "first override should have no previous value"
    );

    let second = ctx
        .insert_value(Box::new(7u32))
        .expect("expected previous override to be returned");
    let previous = second
        .downcast::<u32>()
        .expect("override should downcast to u32");
    assert_eq!(*previous, 5);

    let current = ctx
        .get::<u32>("number")
        .expect("retrieved fixture should exist");
    assert_eq!(*current, 7);
}

#[rstest::rstest]
#[expect(
    clippy::used_underscore_binding,
    reason = "rstest fixture injection requires the parameter"
)]
fn insert_value_returns_none_when_type_ambiguous(_logger: ()) {
    let first = 1u32;
    let second = 2u32;
    let mut ctx = StepContext::default();
    ctx.insert("one", &first);
    ctx.insert("two", &second);

    let result = ctx.insert_value(Box::new(5u32));
    assert!(result.is_none(), "ambiguous overrides must be ignored");
    assert_eq!(ctx.get::<u32>("one"), Some(&1));
    assert_eq!(ctx.get::<u32>("two"), Some(&2));
}

#[rstest::rstest]
#[expect(
    clippy::used_underscore_binding,
    reason = "rstest fixture injection requires the parameter"
)]
fn insert_value_returns_none_when_type_missing(_logger: ()) {
    let text = "fixture";
    let mut ctx = StepContext::default();
    ctx.insert("text", &text);

    let result = ctx.insert_value(Box::new(5u32));
    assert!(result.is_none(), "missing fixture should skip override");
    assert!(ctx.get::<u32>("text").is_none());
}

#[test]
fn available_fixtures_returns_inserted_names() {
    let value_a = 1u32;
    let value_b = "text";
    let mut ctx = StepContext::default();
    ctx.insert("fixture_a", &value_a);
    ctx.insert("fixture_b", &value_b);

    let names: Vec<_> = ctx.available_fixtures().collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"fixture_a"));
    assert!(names.contains(&"fixture_b"));
}

#[test]
fn available_fixtures_includes_owned_fixtures() {
    let value = 42u32;
    let cell: RefCell<Box<dyn Any>> = RefCell::new(Box::new(String::from("owned")));
    let mut ctx = StepContext::default();
    ctx.insert("shared", &value);
    ctx.insert_owned::<String>("owned", &cell);

    let names: Vec<_> = ctx.available_fixtures().collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"shared"));
    assert!(names.contains(&"owned"));
}

#[test]
fn available_fixtures_empty_when_no_fixtures() {
    let ctx = StepContext::default();
    let names: Vec<_> = ctx.available_fixtures().collect();
    assert!(names.is_empty());
}
