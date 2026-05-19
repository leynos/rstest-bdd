//! Unit tests for `ExecutionError` display and localization formatting.

use std::sync::Arc;

use i18n_embed::fluent::fluent_language_loader;
use rstest::rstest;
use rstest_bdd::execution::{ExecutionError, MissingFixtureDiagnostic, MissingFixturesDetails};
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

/// Private builder: constructs a `MissingFixtures` `ExecutionError` from
/// the provided field values, eliminating the repeated `Arc::new(...)` scaffold
/// shared by the two fixture-error factory helpers.
#[expect(
    clippy::too_many_arguments,
    reason = "test builder mirrors the diagnostic detail fields under test"
)]
fn make_missing_fixtures(
    step_pattern: &str,
    step_location: &str,
    required: Vec<&'static str>,
    missing: Vec<&'static str>,
    missing_requirements: Vec<MissingFixtureDiagnostic>,
    available: Vec<String>,
    has_suggestion: bool,
    feature_path: &str,
    scenario_name: &str,
) -> ExecutionError {
    ExecutionError::MissingFixtures(Arc::new(MissingFixturesDetails {
        step_pattern: step_pattern.into(),
        step_location: step_location.into(),
        required,
        missing,
        missing_requirements,
        available,
        has_suggestion,
        feature_path: feature_path.into(),
        scenario_name: scenario_name.into(),
    }))
}

/// Helper to create a `MissingFixtures` error.
fn missing_fixtures() -> ExecutionError {
    make_missing_fixtures(
        "a database connection",
        "tests/steps.rs:42",
        vec!["db", "cache"],
        vec!["db"],
        vec![MissingFixtureDiagnostic {
            name: "db",
            ty: "DbPool",
        }],
        vec!["cache".into(), "config".into()],
        false,
        "features/db.feature",
        "Database query",
    )
}

/// Helper to create a `MissingFixtures` error with harness guidance.
fn missing_harness_fixture() -> ExecutionError {
    make_missing_fixtures(
        "uses harness context",
        "tests/steps.rs:9",
        vec!["rstest_bdd_harness_context"],
        vec!["rstest_bdd_harness_context"],
        vec![MissingFixtureDiagnostic {
            name: "rstest_bdd_harness_context",
            ty: "AppContext",
        }],
        vec!["world".into()],
        true,
        "features/harness.feature",
        "Harness context",
    )
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
    "Step 'a database connection' (defined at tests/steps.rs:42) requires fixtures db, cache, but the following are missing: db. Requested fixture information: db: DbPool. Available fixtures from scenario: cache, config  (feature: features/db.feature, scenario: Database query)"
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
#[case::zh_hans_step_not_found(
    langid!("zh-Hans"),
    step_not_found(),
    "索引 3 处未找到步骤：Given a user named Alice（功能：features/auth.feature，场景：User login）"
)]
#[case::zh_hant_handler_failed(
    langid!("zh-Hant"),
    handler_failed(),
    "步驟在索引 1 失敗：When the user clicks submit - 透過函式「click_submit」執行步驟「the user clicks submit」時發生錯誤：button not found（功能：features/form.feature，情境：Form submission）"
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
        ("DbPool", "requested fixture type"),
        ("cache", "available fixture"),
        ("config", "available fixture"),
        ("features/db.feature", "feature_path"),
        ("Database query", "scenario_name"),
    ]
)]
#[case::missing_fixtures_in_simplified_chinese(
    langid!("zh-Hans"),
    missing_fixtures(),
    &[
        ("a database connection", "step_pattern"),
        ("tests/steps.rs:42", "step_location"),
        ("db", "missing fixture"),
        ("DbPool", "requested fixture type"),
        ("请求的夹具详情", "requested fixture details label"),
        ("cache", "available fixture"),
        ("config", "available fixture"),
        ("features/db.feature", "feature_path"),
        ("Database query", "scenario_name"),
    ]
)]
#[case::missing_fixtures_in_traditional_chinese(
    langid!("zh-Hant"),
    missing_fixtures(),
    &[
        ("a database connection", "step_pattern"),
        ("tests/steps.rs:42", "step_location"),
        ("db", "missing fixture"),
        ("DbPool", "requested fixture type"),
        ("請求的治具詳情", "requested fixture details label"),
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

const NON_ENGLISH_LOCALES: &[&str] = &[
    "ar", "cs", "da", "de", "el", "es-419", "fa", "fi", "fr", "he", "hi", "hu", "id", "it", "ja",
    "ko", "nb", "nl", "pl", "pt-BR", "pt-PT", "ro", "ru", "sv", "th", "tr", "uk", "vi", "zh-Hans",
    "zh-Hant",
];

#[test]
fn non_english_missing_fixture_diagnostics_include_runtime_arguments() {
    for locale in NON_ENGLISH_LOCALES {
        let locale = locale
            .parse::<LanguageIdentifier>()
            .unwrap_or_else(|e| panic!("invalid locale {locale}: {e}"));
        assert_non_english_missing_fixture_diagnostics_include_runtime_arguments(&locale);
    }
}

fn assert_non_english_missing_fixture_diagnostics_include_runtime_arguments(
    locale: &LanguageIdentifier,
) {
    let loader = fluent_language_loader!();
    i18n_embed::select(&loader, &Localizations, std::slice::from_ref(locale))
        .unwrap_or_else(|e| panic!("failed to load {locale} translations: {e}"));

    let formatted =
        strip_directional_isolates(&missing_harness_fixture().format_with_loader(&loader));

    assert!(
        !formatted.contains("Requested fixture details:"),
        "expected localized fixture-details label for {locale}, got: {formatted}"
    );
    assert!(
        !formatted.contains("Select a harness-backed scenario"),
        "expected localized harness suggestion for {locale}, got: {formatted}"
    );
    assert_contains_all(
        &formatted,
        &[
            ("rstest_bdd_harness_context", "requested fixture name"),
            ("AppContext", "requested fixture type"),
            ("world", "available fixture"),
        ],
    );
}

#[test]
fn missing_fixtures_format_includes_typed_request_details_and_suggestion() {
    let _guard = ScopedLocalization::new(&[langid!("en-US")])
        .unwrap_or_else(|e| panic!("en-US locale should always be available: {e}"));
    let error = missing_harness_fixture();

    assert_contains_all(
        &error.to_string(),
        &[
            ("rstest_bdd_harness_context", "requested fixture name"),
            ("AppContext", "requested fixture type"),
            ("world", "available fixture"),
            ("Select a harness-backed scenario", "harness suggestion"),
        ],
    );
}

#[test]
fn missing_fixtures_snapshot() {
    let _guard = ScopedLocalization::new(&[langid!("en-US")])
        .unwrap_or_else(|e| panic!("en-US locale should always be available: {e}"));
    let details = MissingFixturesDetails {
        step_pattern: "needs fixture".to_string(),
        step_location: "src/steps.rs:42".to_string(),
        required: vec!["db"],
        missing: vec!["db"],
        missing_requirements: vec![MissingFixtureDiagnostic {
            name: "db",
            ty: "DbPool",
        }],
        available: vec!["world".to_string()],
        has_suggestion: true,
        feature_path: "features/example.feature".to_string(),
        scenario_name: "Example scenario".to_string(),
    };
    let error = rstest_bdd::execution::ExecutionError::MissingFixtures(Arc::new(details));
    insta::assert_snapshot!(format!("{error}"));
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
