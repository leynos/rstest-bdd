//! Unit tests for `StepError` display formatting

use i18n_embed::fluent::fluent_language_loader;
use rstest::rstest;
use rstest_bdd::{Localisations, StepError};
use serial_test::serial;
use unic_langid::langid;

fn strip_directional_isolates(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(*c, '\u{2066}' | '\u{2067}' | '\u{2068}' | '\u{2069}'))
        .collect()
}

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
#[serial(localisation)]
fn step_error_display_formats(#[case] err: StepError, #[case] expected: &str) {
    assert_eq!(strip_directional_isolates(&err.to_string()), expected);
}

#[test]
#[serial(localisation)]
fn step_error_formats_in_french() {
    let loader = fluent_language_loader!();
    i18n_embed::select(&loader, &Localisations, &[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to load French translations: {error}"));
    let err = StepError::MissingFixture {
        name: "n".into(),
        ty: "u32".into(),
        step: "s".into(),
    };
    assert_eq!(
        strip_directional_isolates(&err.format_with_loader(&loader)),
        "La fixture « n » de type « u32 » est introuvable pour la fonction « s »"
    );
    i18n_embed::select(&loader, &Localisations, &[langid!("en-US")])
        .unwrap_or_else(|error| panic!("failed to restore English translations: {error}"));
}
