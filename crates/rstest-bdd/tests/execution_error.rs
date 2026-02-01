//! Unit tests for `ExecutionError` display and localization formatting.

use std::sync::Arc;

use i18n_embed::fluent::fluent_language_loader;
use rstest::rstest;
use rstest_bdd::execution::{ExecutionError, MissingFixturesDetails};
use rstest_bdd::localization::{ScopedLocalization, strip_directional_isolates};
use rstest_bdd::{Localizations, StepError, StepKeyword};
use unic_langid::{LanguageIdentifier, langid};

/// Helper to create a Skip error without message.
fn skip_without_message() -> ExecutionError {
    ExecutionError::Skip { message: None }
}

/// Helper to create a Skip error with message.
fn skip_with_message(msg: &str) -> ExecutionError {
    ExecutionError::Skip {
        message: Some(msg.into()),
    }
}

/// Helper to create a `StepNotFound` error.
fn step_not_found() -> ExecutionError {
    ExecutionError::StepNotFound {
        index: 3,
        keyword: StepKeyword::Given,
        text: "a user named Alice".into(),
        feature_path: "features/auth.feature".into(),
        scenario_name: "User login".into(),
    }
}

/// Helper to create a `MissingFixtures` error.
fn missing_fixtures() -> ExecutionError {
    ExecutionError::MissingFixtures(Arc::new(MissingFixturesDetails {
        step_pattern: "a database connection".into(),
        step_location: "tests/steps.rs:42".into(),
        required: vec!["db", "cache"],
        missing: vec!["db"],
        available: vec!["cache".into(), "config".into()],
        feature_path: "features/db.feature".into(),
        scenario_name: "Database query".into(),
    }))
}

/// Helper to create a `HandlerFailed` error.
fn handler_failed() -> ExecutionError {
    ExecutionError::HandlerFailed {
        index: 1,
        keyword: StepKeyword::When,
        text: "the user clicks submit".into(),
        error: Arc::new(StepError::ExecutionError {
            pattern: "the user clicks submit".into(),
            function: "click_submit".into(),
            message: "button not found".into(),
        }),
        feature_path: "features/form.feature".into(),
        scenario_name: "Form submission".into(),
    }
}

#[rstest]
#[case::skip_without_message(skip_without_message(), "Step skipped")]
#[case::skip_with_message(
    skip_with_message("not implemented yet"),
    "Step skipped: not implemented yet"
)]
#[case::step_not_found(
    step_not_found(),
    "Step not found at index 3: Given a user named Alice (feature: features/auth.feature, scenario: User login)"
)]
#[case::missing_fixtures(
    missing_fixtures(),
    "Step 'a database connection' (defined at tests/steps.rs:42) requires fixtures db, cache, but the following are missing: db. Available fixtures from scenario: cache, config (feature: features/db.feature, scenario: Database query)"
)]
#[case::handler_failed(
    handler_failed(),
    "Step failed at index 1: When the user clicks submit - Error executing step 'the user clicks submit' via function 'click_submit': button not found (feature: features/form.feature, scenario: Form submission)"
)]
fn execution_error_display_uses_localized_messages_and_context(
    #[case] error: ExecutionError,
    #[case] expected: &str,
) {
    // Scope to en-US to avoid environment-dependent output on non-English systems
    let _guard = ScopedLocalization::new(&[langid!("en-US")])
        .unwrap_or_else(|e| panic!("en-US locale should always be available: {e}"));
    assert_eq!(strip_directional_isolates(&error.to_string()), expected);
}

#[rstest]
#[case::skip_without_message(
    langid!("pl"),
    skip_without_message(),
    "Krok pominięty"
)]
#[case::skip_with_message(
    langid!("pl"),
    skip_with_message("jeszcze nie zaimplementowane"),
    "Krok pominięty: jeszcze nie zaimplementowane"
)]
#[case::step_not_found(
    langid!("pl"),
    step_not_found(),
    "Nie znaleziono kroku o indeksie 3: Given a user named Alice (feature: features/auth.feature, scenariusz: User login)"
)]
#[case::handler_failed(
    langid!("pl"),
    handler_failed(),
    "Krok zakończony błędem o indeksie 1: When the user clicks submit - Błąd wykonywania kroku « the user clicks submit » przez funkcję « click_submit »: button not found (feature: features/form.feature, scenariusz: Form submission)"
)]
fn execution_error_formats_in_locales(
    #[case] locale: LanguageIdentifier,
    #[case] error: ExecutionError,
    #[case] expected: &str,
) {
    let _guard = ScopedLocalization::new(std::slice::from_ref(&locale))
        .unwrap_or_else(|e| panic!("failed to scope locale {locale}: {e}"));
    assert_eq!(strip_directional_isolates(&error.to_string()), expected);
}

/// Asserts that the formatted string contains all expected substrings.
///
/// Panics with a descriptive message if any substring is missing.
fn assert_contains_all(formatted: &str, expected_substrings: &[(&str, &str)]) {
    let stripped = strip_directional_isolates(formatted);
    for (substring, description) in expected_substrings {
        assert!(
            stripped.contains(substring),
            "expected {description} in message, got: {stripped}"
        );
    }
}

#[rstest]
#[case::step_not_found_in_polish(
    langid!("pl"),
    step_not_found(),
    &[
        ("Nie znaleziono kroku", "Polish translation"),
        ("3", "index"),
        ("Given", "keyword"),
        ("a user named Alice", "text"),
        ("features/auth.feature", "feature_path"),
        ("User login", "scenario_name"),
    ]
)]
#[case::missing_fixtures_in_english(
    langid!("en-US"),
    missing_fixtures(),
    &[
        ("a database connection", "step_pattern"),
        ("tests/steps.rs:42", "step_location"),
        ("db", "missing fixture"),
        ("cache", "available fixture"),
        ("config", "available fixture"),
        ("features/db.feature", "feature_path"),
        ("Database query", "scenario_name"),
    ]
)]
fn execution_error_format_with_loader_wires_i18n_and_context(
    #[case] locale: LanguageIdentifier,
    #[case] error: ExecutionError,
    #[case] expected_substrings: &[(&str, &str)],
) {
    let loader = fluent_language_loader!();
    i18n_embed::select(&loader, &Localizations, std::slice::from_ref(&locale))
        .unwrap_or_else(|e| panic!("failed to load {locale} translations: {e}"));

    let formatted = error.format_with_loader(&loader);
    assert_contains_all(&formatted, expected_substrings);
}

#[test]
fn execution_error_handler_failed_formats_nested_error_with_loader() {
    let loader = fluent_language_loader!();
    i18n_embed::select(&loader, &Localizations, &[langid!("pl")])
        .unwrap_or_else(|e| panic!("failed to load Polish translations: {e}"));

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
