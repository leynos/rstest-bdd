//! Behavioural coverage for explicit, lexical step-library selection.

use std::sync::atomic::{AtomicBool, Ordering};

use rstest_bdd_macros::{given, scenario, step_library};

static ACCOUNT_MATCHED: AtomicBool = AtomicBool::new(false);
static FILESYSTEM_MATCHED: AtomicBool = AtomicBool::new(false);

#[step_library]
mod accounts {
    //! Account-domain vocabulary used by the scoped-library scenarios.

    use super::{ACCOUNT_MATCHED, Ordering, given};

    #[given("the domain is empty")]
    fn domain_is_empty() { ACCOUNT_MATCHED.store(true, Ordering::SeqCst); }
}

#[step_library]
mod filesystem {
    //! Filesystem-domain vocabulary used by the scoped-library scenarios.

    use super::{FILESYSTEM_MATCHED, Ordering, given};

    #[given("the domain is empty")]
    fn domain_is_empty() { FILESYSTEM_MATCHED.store(true, Ordering::SeqCst); }
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
