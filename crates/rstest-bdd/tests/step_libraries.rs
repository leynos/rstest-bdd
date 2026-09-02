//! Behavioural coverage for explicit, lexical step-library selection.

use std::sync::atomic::{AtomicBool, Ordering};

use rstest_bdd::{dump_registry, reporting};
use rstest_bdd_macros::{given, scenario, scenarios, step_library, then, when};
use serde_json::Value;
use serial_test::serial;

static ACCOUNT_MATCHED: AtomicBool = AtomicBool::new(false);
static FILESYSTEM_MATCHED: AtomicBool = AtomicBool::new(false);
static ASYNC_ACCOUNT_MATCHED: AtomicBool = AtomicBool::new(false);
static HARNESSED_ACCOUNT_MATCHED: AtomicBool = AtomicBool::new(false);

#[step_library]
mod accounts {
    //! Account-domain vocabulary used by the scoped-library scenarios.

    use super::{
        ACCOUNT_MATCHED,
        ASYNC_ACCOUNT_MATCHED,
        HARNESSED_ACCOUNT_MATCHED,
        Ordering,
        given,
        then,
        when,
    };

    #[given("the domain is empty")]
    fn domain_is_empty() { ACCOUNT_MATCHED.store(true, Ordering::SeqCst); }

    #[given("the scoped account scenario is skipped")]
    fn scoped_account_scenario_is_skipped() {
        rstest_bdd::skip!("account scope only");
    }

    #[then("the scoped trailing step is bypassed")]
    fn scoped_trailing_step_is_bypassed() {}

    #[when("the scoped account operation runs asynchronously")]
    async fn scoped_account_operation_runs_asynchronously() {
        tokio::task::yield_now().await;
        ASYNC_ACCOUNT_MATCHED.store(true, Ordering::SeqCst);
    }

    #[given("the harness runs the scoped account vocabulary")]
    fn harness_runs_scoped_account_vocabulary() {
        HARNESSED_ACCOUNT_MATCHED.store(true, Ordering::SeqCst);
    }

    #[given("the scenarios macro selects the account vocabulary")]
    fn scenarios_macro_selects_account_vocabulary() {}
}

#[step_library]
mod filesystem {
    //! Filesystem-domain vocabulary used by the scoped-library scenarios.

    use super::{FILESYSTEM_MATCHED, Ordering, given, then, when};

    #[given("the domain is empty")]
    fn domain_is_empty() { FILESYSTEM_MATCHED.store(true, Ordering::SeqCst); }

    #[then("the scoped trailing step is bypassed")]
    fn scoped_trailing_step_is_bypassed() {}

    #[when("the scoped account operation runs asynchronously")]
    fn scoped_account_operation_runs_asynchronously() {
        panic!("the async scenario must not use the unselected filesystem library");
    }

    #[given("the harness runs the scoped account vocabulary")]
    fn harness_runs_scoped_account_vocabulary() {
        panic!("the harness scenario must not use the unselected filesystem library");
    }

    #[given("the scenarios macro selects the account vocabulary")]
    fn scenarios_macro_selects_account_vocabulary() {
        panic!("scenarios! must not use the unselected filesystem library");
    }
}

#[scenario(
    path = "tests/features/step_libraries.feature",
    name = "Account vocabulary",
    libraries = [accounts]
)]
fn account_vocabulary() {
    assert!(ACCOUNT_MATCHED.load(Ordering::SeqCst));
}

#[scenario(
    path = "tests/features/step_libraries.feature",
    name = "Filesystem vocabulary",
    libraries = [filesystem]
)]
fn filesystem_vocabulary() {
    assert!(FILESYSTEM_MATCHED.load(Ordering::SeqCst));
}

#[scenario(
    path = "tests/features/step_libraries.feature",
    name = "Ambiguous vocabulary",
    libraries = [accounts, filesystem]
)]
#[should_panic(expected = "ambiguous Given 'the domain is empty'")]
fn ambiguous_vocabulary() {}

#[scenario(
    path = "tests/features/step_libraries.feature",
    name = "Reversed ambiguous vocabulary",
    libraries = [filesystem, accounts]
)]
#[should_panic(expected = "ambiguous Given 'the domain is empty'")]
fn reversed_ambiguous_vocabulary() {}

#[scenario(
    path = "tests/features/step_libraries.feature",
    name = "Async scoped vocabulary",
    libraries = [accounts]
)]
#[tokio::test(flavor = "current_thread")]
async fn async_scoped_vocabulary() {
    assert!(ASYNC_ACCOUNT_MATCHED.load(Ordering::SeqCst));
}

#[scenario(
    path = "tests/features/step_libraries.feature",
    name = "Harness scoped vocabulary",
    libraries = [accounts],
    harness = rstest_bdd_harness::StdHarness,
)]
fn harness_scoped_vocabulary() {
    assert!(HARNESSED_ACCOUNT_MATCHED.load(Ordering::SeqCst));
}

#[scenario(
    path = "tests/features/step_libraries.feature",
    name = "Scoped bypass vocabulary",
    libraries = [accounts]
)]
#[serial]
fn scoped_bypass_vocabulary() {}

#[test]
#[serial]
fn scoped_bypass_records_only_the_selected_library() {
    let _ = reporting::drain();
    scoped_bypass_vocabulary();
    let json = dump_registry().expect("registry dump should succeed");
    let dump: Value = serde_json::from_str(&json).expect("registry dump should be JSON");
    let bypassed_steps = dump
        .get("bypassed_steps")
        .and_then(Value::as_array)
        .expect("registry dump should contain bypassed steps");
    let bypassed = bypassed_steps
        .iter()
        .find(|step| {
            step.get("scenario_name") == Some(&Value::String("Scoped bypass vocabulary".into()))
        })
        .expect("scoped scenario should record its bypassed step");
    assert_eq!(
        bypassed.get("library"),
        Some(&Value::String("step_libraries::accounts".into()))
    );
    assert_eq!(
        bypassed.get("libraries"),
        Some(&serde_json::json!(["step_libraries::accounts"])),
    );
}

scenarios!(
    "tests/features/scoped_step_libraries",
    libraries = [accounts],
);
