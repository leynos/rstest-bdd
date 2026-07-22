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
/// Assert that a unique fixture override can be replaced twice.
///
/// A macro rather than a helper function so that panic line numbers point at
/// the calling test.
macro_rules! assert_unique_fixture_can_be_overridden_twice {
    ($ctx:expr) => {{
        let first = $ctx.insert_value(Box::new(5u32));
        assert!(
            first.is_none(),
            "first override should have no previous value"
        );

        let Some(second) = $ctx.insert_value(Box::new(7u32)) else {
            panic!("expected previous override to be returned");
        };
        let Ok(previous) = second.downcast::<u32>() else {
            panic!("override should downcast to u32");
        };
        assert_eq!(*previous, 5);

        let Ok(current) = $ctx.try_borrow::<u32>("number") else {
            panic!("retrieved override should exist");
        };
        assert_eq!(*current, 7);
    }};
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
#[expect(
    clippy::expect_used,
    reason = "downcast must succeed for the typed fixture under test"
)]
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

/// Type-erased owned fixture cell, as accepted by `insert_owned`.
type OwnedCell = RefCell<Box<dyn Any>>;

/// Build a context with two owned fixtures and one shared fixture for
/// borrow-semantics tests. Returns the cells so they outlive the context.
fn owned_pair_cells() -> (OwnedCell, OwnedCell) {
    (
        StepContext::owned_cell(1_u32),
        StepContext::owned_cell(String::from("hi")),
    )
}

#[test]
#[expect(clippy::expect_used, reason = "tests require explicit panic messages")]
fn distinct_fixtures_can_be_borrowed_mutably_at_once() {
    let (first, second) = owned_pair_cells();
    let mut ctx = StepContext::default();
    ctx.insert_owned::<u32>("first", &first);
    ctx.insert_owned::<String>("second", &second);

    let mut a = ctx
        .try_borrow_mut::<u32>("first")
        .expect("first fixture borrows");
    let mut b = ctx
        .try_borrow_mut::<String>("second")
        .expect("second fixture borrows while first guard is alive");
    *a += 1;
    b.push('!');
    assert_eq!(*a, 2);
    assert_eq!(b.as_str(), "hi!");
}

#[test]
#[expect(clippy::expect_used, reason = "tests require explicit panic messages")]
fn conflicting_borrows_of_same_fixture_report_already_borrowed() {
    let (first, _second) = owned_pair_cells();
    let mut ctx = StepContext::default();
    ctx.insert_owned::<u32>("first", &first);

    let guard = ctx
        .try_borrow_mut::<u32>("first")
        .expect("initial mutable borrow succeeds");

    let mut_err = ctx
        .try_borrow_mut::<u32>("first")
        .expect_err("second mutable borrow conflicts");
    assert_eq!(
        mut_err,
        FixtureBorrowError::AlreadyBorrowed {
            name: "first".into()
        }
    );

    let shared_err = ctx
        .try_borrow::<u32>("first")
        .expect_err("shared borrow conflicts with live mutable guard");
    assert_eq!(
        shared_err,
        FixtureBorrowError::AlreadyBorrowed {
            name: "first".into()
        }
    );

    drop(guard);
    assert!(
        ctx.try_borrow_mut::<u32>("first").is_ok(),
        "borrow succeeds again after the guard is dropped"
    );
}

#[test]
#[expect(clippy::expect_used, reason = "tests require explicit panic messages")]
fn try_borrow_reports_not_found_type_mismatch_and_not_mutable() {
    let shared = 9_i32;
    let (first, _second) = owned_pair_cells();
    let mut ctx = StepContext::default();
    ctx.insert("shared", &shared);
    ctx.insert_owned::<u32>("first", &first);

    let missing = ctx
        .try_borrow::<u32>("absent")
        .expect_err("unknown fixture reports NotFound");
    assert_eq!(
        missing,
        FixtureBorrowError::NotFound {
            name: "absent".into()
        }
    );

    let mismatch = ctx
        .try_borrow::<String>("first")
        .expect_err("wrong type reports TypeMismatch");
    assert_eq!(
        mismatch,
        FixtureBorrowError::TypeMismatch {
            name: "first".into()
        }
    );

    let immutable = ctx
        .try_borrow_mut::<i32>("shared")
        .expect_err("shared fixtures cannot be borrowed mutably");
    assert_eq!(
        immutable,
        FixtureBorrowError::NotMutable {
            name: "shared".into()
        }
    );
}

#[test]
#[expect(clippy::expect_used, reason = "tests require explicit panic messages")]
fn override_values_participate_in_guard_borrowing() {
    let fixture = 1_u32;
    let mut ctx = StepContext::default();
    ctx.insert("number", &fixture);
    assert!(ctx.insert_value(Box::new(5_u32)).is_none());

    {
        let mut value = ctx
            .try_borrow_mut::<u32>("number")
            .expect("override borrows mutably through &self");
        *value += 1;
    }
    let value = ctx
        .try_borrow::<u32>("number")
        .expect("override readable after guard drop");
    assert_eq!(*value, 6);
}

#[test]
#[expect(clippy::expect_used, reason = "tests require explicit panic messages")]
fn multiple_shared_borrows_of_same_fixture_coexist() {
    let (first, _second) = owned_pair_cells();
    let mut ctx = StepContext::default();
    ctx.insert_owned::<u32>("first", &first);

    let a = ctx.try_borrow::<u32>("first").expect("first shared borrow");
    let b = ctx
        .try_borrow::<u32>("first")
        .expect("second shared borrow coexists");
    assert_eq!(*a, *b);

    let err = ctx
        .try_borrow_mut::<u32>("first")
        .expect_err("mutable borrow conflicts with shared guards");
    assert_eq!(
        err,
        FixtureBorrowError::AlreadyBorrowed {
            name: "first".into()
        }
    );
}
