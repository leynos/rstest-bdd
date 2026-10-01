//! Snapshot and nested-error formatting tests for execution errors.

use std::sync::Arc;

use i18n_embed::fluent::fluent_language_loader;
use rstest_bdd::{
    Localizations,
    execution::{MissingFixtureDiagnostic, MissingFixturesDetails},
    localization::{ScopedLocalization, strip_directional_isolates},
};
use unic_langid::langid;

use super::helpers::handler_failed;

#[test]
fn missing_fixtures_snapshot() {
    let _guard = match ScopedLocalization::new(&[langid!("en-US")]) {
        Ok(guard) => guard,
        Err(e) => panic!("en-US locale should always be available: {e}"),
    };
    let details = MissingFixturesDetails {
        step_pattern: "needs fixture".to_owned(),
        step_location: "src/steps.rs:42".to_owned(),
        required: vec!["db"],
        missing: vec!["db"],
        missing_requirements: vec![MissingFixtureDiagnostic {
            name: "db",
            ty: "DbPool",
        }],
        available: vec!["world".to_owned()],
        has_suggestion: true,
        feature_path: "features/example.feature".to_owned(),
        scenario_name: "Example scenario".to_owned(),
    };
    let error = rstest_bdd::execution::ExecutionError::MissingFixtures(Arc::new(details));
    insta::assert_snapshot!(format!("{error}"));
}

#[test]
fn execution_error_handler_failed_formats_nested_error_with_loader() {
    let loader = fluent_language_loader!();
    if let Err(e) = i18n_embed::select(&loader, &Localizations, &[langid!("pl")]) {
        panic!("failed to load Polish translations: {e}");
    }

    let error = handler_failed();
    let formatted = error.format_with_loader(&loader);
    let stripped = strip_directional_isolates(&formatted);

    // Verify Polish outer message
    assert!(
        stripped.contains("Krok zakończony błędem"),
        "expected Polish translation for outer error, got: {stripped}"
    );
    // Verify inner StepError is also formatted with the loader (Polish)
    assert!(
        stripped.contains("Błąd wykonywania kroku"),
        "expected Polish inner error message, got: {stripped}"
    );
    // Verify context fields
    assert!(
        stripped.contains("the user clicks submit"),
        "expected step text in message, got: {stripped}"
    );
    assert!(
        stripped.contains("button not found"),
        "expected inner error detail in message, got: {stripped}"
    );
}
