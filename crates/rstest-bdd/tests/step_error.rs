//! Unit tests for `StepError` display formatting.

use i18n_embed::fluent::fluent_language_loader;
use rstest::rstest;
use rstest_bdd::localization::{ScopedLocalization, strip_directional_isolates};
use rstest_bdd::{Localizations, StepError};
use unic_langid::{LanguageIdentifier, langid};

#[rstest]
#[case(
    StepError::MissingFixture {
        name: "n".into(),
        ty: "u32".into(),
        step: "s".into(),
    },
    "Missing fixture 'n' of type 'u32' for step function 's'",
)]
#[case(
    StepError::ExecutionError {
        pattern: "p".into(),
        function: "f".into(),
        message: "m".into(),
    },
    "Error executing step 'p' via function 'f': m",
)]
#[case(
    StepError::PanicError {
        pattern: "p".into(),
        function: "f".into(),
        message: "boom".into(),
    },
    "Panic in step 'p', function 'f': boom",
)]
fn step_error_display_formats(#[case] err: StepError, #[case] expected: &str) {
    assert_eq!(strip_directional_isolates(&err.to_string()), expected);
}

#[rstest]
#[case::missing_fixture(
    langid!("fr"),
    StepError::MissingFixture {
        name: "n".into(),
        ty: "u32".into(),
        step: "s".into(),
    },
    "La fixture « n » de type « u32 » est introuvable pour la fonction « s »",
)]
#[case::execution(
    langid!("fr"),
    StepError::ExecutionError {
        pattern: "p".into(),
        function: "f".into(),
        message: "boom".into(),
    },
    "Erreur lors de l'exécution de l'étape « p » via la fonction « f » : boom",
)]
#[case::panic(
    langid!("fr"),
    StepError::PanicError {
        pattern: "p".into(),
        function: "f".into(),
        message: "boom".into(),
    },
    "Panique dans l'étape « p », fonction « f » : boom",
)]
fn step_error_formats_in_locales(
    #[case] locale: LanguageIdentifier,
    #[case] err: StepError,
    #[case] expected: &str,
) {
    let guard = ScopedLocalization::new(std::slice::from_ref(&locale))
        .unwrap_or_else(|error| panic!("failed to scope locale {locale}: {error}"));
    assert_eq!(strip_directional_isolates(&err.to_string()), expected);
    drop(guard);
}

#[test]
fn format_with_loader_uses_provided_loader() {
    let loader = fluent_language_loader!();
    i18n_embed::select(&loader, &Localizations, &[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to load French translations: {error}"));
    let err = StepError::ExecutionError {
        pattern: "p".into(),
        function: "f".into(),
        message: "boom".into(),
    };
    assert_eq!(
        strip_directional_isolates(&err.format_with_loader(&loader)),
        "Erreur lors de l'exécution de l'étape « p » via la fonction « f » : boom",
    );
}
