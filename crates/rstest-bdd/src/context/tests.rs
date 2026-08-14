//! Tests for step context and fixture management.

use super::*;
use std::sync::Once;

mod guard_borrowing;

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
    /// One u32 fixture; two inserts report the initial and displaced outcomes.
    UniqueOverride,
    /// Two u32 fixtures; `insert_value` reports an ambiguous type.
    AmbiguousType,
    /// One &str fixture; `insert_value` reports that the u32 type is missing.
    MissingType,
}

/// Assert that a unique fixture override can be replaced twice.
/// The first call returns None; the second returns the previous, correct value.
///
/// A macro rather than a helper function so that panic line numbers point at
/// the calling test.
macro_rules! assert_unique_fixture_can_be_overridden_twice {
    ($ctx:expr) => {{
        let first = $ctx.insert_value(Box::new(5u32));
        assert!(
            first.is_inserted(),
            "a recorded override should report is_inserted"
        );
        assert!(
            matches!(&first, InsertOutcome::Inserted(None)),
            "first override should insert with no previous value"
        );
        assert!(
            first.into_previous().is_none(),
            "an insert that displaced nothing should yield no previous override"
        );

        let second = $ctx.insert_value(Box::new(7u32));
        assert!(
            second.is_inserted(),
            "an override that displaced an earlier one should report is_inserted"
        );
        assert!(
            matches!(&second, InsertOutcome::Inserted(Some(_))),
            "second override should insert and carry the displaced value"
        );
        let Some(displaced) = second.into_previous() else {
            panic!("expected previous override to be returned");
        };
        let Ok(previous) = displaced.downcast::<u32>() else {
            panic!("override should downcast to u32");
        };
        assert_eq!(*previous, 5);

        let Ok(current) = $ctx.try_borrow::<u32>("number") else {
            panic!("retrieved override should exist");
        };
        assert_eq!(*current, 7);
    }};
}

#[test]
fn get_ignores_step_return_override() {
    let fixture = 1_u32;
    let mut ctx = StepContext::default();
    ctx.insert("number", &fixture);
    let _ = ctx.insert_value(Box::new(7_u32));
    let Ok(guard) = ctx.try_borrow::<u32>("number") else {
        panic!("inserted override should be readable");
    };
    assert_eq!(*guard, 7);
    drop(guard);
    assert_eq!(ctx.get::<u32>("number"), Some(&1));
}
#[rstest::rstest]
#[case::unique_override(InsertValueScenario::UniqueOverride)]
#[case::ambiguous_type(InsertValueScenario::AmbiguousType)]
#[case::missing_type(InsertValueScenario::MissingType)]
#[expect(
    clippy::used_underscore_binding,
    reason = "rstest fixture injection requires the parameter"
)]
fn insert_value_behaviour(_logger: (), #[case] scenario: InsertValueScenario) {
    // Storage for fixtures must outlive the context
    let fixture_one: u32 = 1;
    let fixture_two: u32 = 2;
    let fixture_text: &str = "fixture";

    let mut ctx = StepContext::default();

    match scenario {
        InsertValueScenario::UniqueOverride => {
            ctx.insert("number", &fixture_one);
            assert_unique_fixture_can_be_overridden_twice!(ctx);
        }
        InsertValueScenario::AmbiguousType => {
            ctx.insert("one", &fixture_one);
            ctx.insert("two", &fixture_two);

            let result = ctx.insert_value(Box::new(5u32));
            assert!(
                matches!(&result, InsertOutcome::AmbiguousIgnored),
                "ambiguous overrides must be reported as AmbiguousIgnored"
            );
            assert!(
                !result.is_inserted(),
                "a dropped value must not report is_inserted"
            );
            assert!(
                result.into_previous().is_none(),
                "a dropped value must not yield a previous override"
            );
            let Ok(one) = ctx.try_borrow::<u32>("one") else {
                panic!("first fixture should remain borrowable");
            };
            let Ok(two) = ctx.try_borrow::<u32>("two") else {
                panic!("second fixture should remain borrowable");
            };
            assert_eq!(*one, 1);
            assert_eq!(*two, 2);
        }
        InsertValueScenario::MissingType => {
            ctx.insert("text", &fixture_text);

            let result = ctx.insert_value(Box::new(5u32));
            assert!(
                matches!(&result, InsertOutcome::NoMatch),
                "missing fixture type must be reported as NoMatch"
            );
            assert!(
                !result.is_inserted(),
                "a dropped value must not report is_inserted"
            );
            assert!(
                result.into_previous().is_none(),
                "a dropped value must not yield a previous override"
            );
            let Err(mismatch) = ctx.try_borrow::<u32>("text") else {
                panic!("borrowing the fixture as u32 should report a type mismatch");
            };
            assert_eq!(
                mismatch,
                FixtureBorrowError::TypeMismatch {
                    name: "text".into()
                }
            );
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
fn available_fixtures_behaviour(
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

#[test]
fn insert_harness_context_exposes_shared_reference() {
    let context_value = 13usize;
    let mut ctx = StepContext::default();
    ctx.insert_harness_context(&context_value);

    assert_eq!(ctx.harness_context::<usize>(), Some(&13));
    assert_eq!(
        ctx.get::<usize>(RSTEST_BDD_HARNESS_CONTEXT_FIXTURE),
        Some(&13)
    );
}

#[test]
fn insert_owned_harness_context_supports_mutation() {
    let harness_cell: RefCell<Box<dyn Any>> = RefCell::new(Box::new(String::from("harness")));
    let mut ctx = StepContext::default();
    ctx.insert_owned_harness_context::<String>(&harness_cell);

    {
        let Some(mut value) = ctx.borrow_harness_context_mut::<String>() else {
            panic!("mutable harness context should exist");
        };
        value.as_mut().push_str("-updated");
    }

    {
        let Some(value) = ctx.borrow_harness_context::<String>() else {
            panic!("harness context should be borrowable");
        };
        assert_eq!(value.value(), "harness-updated");
    }

    drop(ctx);
    let stored = harness_cell
        .into_inner()
        .downcast::<String>()
        .expect("harness context should downcast to String");
    assert_eq!(*stored, "harness-updated");
}
