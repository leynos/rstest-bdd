//! Guard-based fixture borrowing tests for `StepContext`.

use super::super::*;

/// Type-erased owned fixture cell, as accepted by `insert_owned`.
type OwnedCell = RefCell<Box<dyn Any>>;

/// Return two owned cells containing a `u32` and a `String`.
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
    assert!(ctx.insert_value(Box::new(5_u32)).is_inserted());

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
