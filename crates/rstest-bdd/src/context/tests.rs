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

/// Describes which `insert_value` scenario to test.
#[derive(Debug, Clone, Copy)]
enum InsertValueScenario {
    /// One u32 fixture; `insert_value` twice expects first None, second returns previous.
    UniqueOverride,
    /// Two u32 fixtures; `insert_value` returns None because type is ambiguous.
    AmbiguousType,
    /// One &str fixture; `insert_value` with u32 returns None because type is missing.
    MissingType,
}

/// Verifies that a unique fixture can be overridden twice via `insert_value`.
///
/// First call returns None, second returns the previous override, final value is correct.
#[expect(clippy::expect_used, reason = "tests require explicit panic messages")]
fn assert_unique_fixture_can_be_overridden_twice(ctx: &mut StepContext<'_>) {
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
#[case::unique_override(InsertValueScenario::UniqueOverride)]
#[case::ambiguous_type(InsertValueScenario::AmbiguousType)]
#[case::missing_type(InsertValueScenario::MissingType)]
#[expect(
    clippy::used_underscore_binding,
    reason = "rstest fixture injection requires the parameter"
)]
fn insert_value_behavior(_logger: (), #[case] scenario: InsertValueScenario) {
    // Storage for fixtures must outlive the context
    let fixture_one: u32 = 1;
    let fixture_two: u32 = 2;
    let fixture_text: &str = "fixture";

    let mut ctx = StepContext::default();

    match scenario {
        InsertValueScenario::UniqueOverride => {
            ctx.insert("number", &fixture_one);
            assert_unique_fixture_can_be_overridden_twice(&mut ctx);
        }
        InsertValueScenario::AmbiguousType => {
            ctx.insert("one", &fixture_one);
            ctx.insert("two", &fixture_two);

            let result = ctx.insert_value(Box::new(5u32));
            assert!(result.is_none(), "ambiguous overrides must be ignored");
            assert_eq!(ctx.get::<u32>("one"), Some(&1));
            assert_eq!(ctx.get::<u32>("two"), Some(&2));
        }
        InsertValueScenario::MissingType => {
            ctx.insert("text", &fixture_text);

            let result = ctx.insert_value(Box::new(5u32));
            assert!(result.is_none(), "missing fixture should skip override");
            assert!(ctx.get::<u32>("text").is_none());
        }
    }
}

/// Describes which `available_fixtures` scenario to test.
#[derive(Debug, Clone, Copy)]
enum AvailableFixturesScenario {
    /// Two shared fixtures only.
    SharedOnly,
    /// One shared and one owned fixture.
    SharedAndOwned,
    /// No fixtures at all.
    Empty,
}

#[rstest::rstest]
#[case::shared_only(AvailableFixturesScenario::SharedOnly, &["fixture_a", "fixture_b"])]
#[case::shared_and_owned(AvailableFixturesScenario::SharedAndOwned, &["shared", "owned"])]
#[case::empty(AvailableFixturesScenario::Empty, &[])]
fn available_fixtures_behavior(
    #[case] scenario: AvailableFixturesScenario,
    #[case] expected: &[&str],
) {
    // Storage for fixtures must outlive the context
    let value_a: u32 = 1;
    let value_b: &str = "text";
    let shared_value: u32 = 42;
    let cell: RefCell<Box<dyn Any>> = RefCell::new(Box::new(String::from("owned")));

    let mut ctx = StepContext::default();

    match scenario {
        AvailableFixturesScenario::SharedOnly => {
            ctx.insert("fixture_a", &value_a);
            ctx.insert("fixture_b", &value_b);
        }
        AvailableFixturesScenario::SharedAndOwned => {
            ctx.insert("shared", &shared_value);
            ctx.insert_owned::<String>("owned", &cell);
        }
        AvailableFixturesScenario::Empty => {
            // No fixtures inserted
        }
    }

    let names: Vec<_> = ctx.available_fixtures().collect();
    assert_eq!(names.len(), expected.len());
    for name in expected {
        assert!(names.contains(name));
    }
}
