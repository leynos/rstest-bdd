//! Loader and translated-diagnostic tests for execution errors.

use i18n_embed::fluent::fluent_language_loader;
use rstest::rstest;
use rstest_bdd::{
    Localizations,
    execution::ExecutionError,
    localization::{ScopedLocalization, strip_directional_isolates},
};
use unic_langid::{LanguageIdentifier, langid};

use super::helpers::{missing_fixtures, missing_harness_fixture, step_not_found};

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
    if let Err(e) = i18n_embed::select(&loader, &Localizations, std::slice::from_ref(&locale)) {
        panic!("failed to load {locale} translations: {e}");
    }

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
        let locale = match locale.parse::<LanguageIdentifier>() {
            Ok(locale) => locale,
            Err(e) => panic!("invalid locale {locale}: {e}"),
        };
        assert_non_english_missing_fixture_diagnostics_include_runtime_arguments(&locale);
    }
}

fn assert_non_english_missing_fixture_diagnostics_include_runtime_arguments(
    locale: &LanguageIdentifier,
) {
    let loader = fluent_language_loader!();
    if let Err(e) = i18n_embed::select(&loader, &Localizations, std::slice::from_ref(locale)) {
        panic!("failed to load {locale} translations: {e}");
    }

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
    let _guard = match ScopedLocalization::new(&[langid!("en-US")]) {
        Ok(guard) => guard,
        Err(e) => panic!("en-US locale should always be available: {e}"),
    };
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
