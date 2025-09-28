//! Compile-time fixture that ensures missing steps emit diagnostics.
use rstest_bdd_macros::scenario;

/// This fixture is expected to fail because the feature file includes an
/// unmatched step.
/// The assertion clarifies the intent; remove it if runtime behaviour changes.
#[scenario(path = "../features/macros/unmatched.feature")]
fn missing_step() {
    assert!(false, "This test should fail due to an unmatched step definition.");
}

fn main() {}
