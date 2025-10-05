use rstest_bdd::{StepError, current_languages, select_localisations};
use serial_test::serial;
use unic_langid::langid;

struct ResetLocale;

fn strip_directional_isolates(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(*c, '\u{2066}' | '\u{2067}' | '\u{2068}' | '\u{2069}'))
        .collect()
}

impl Drop for ResetLocale {
    fn drop(&mut self) {
        let _ = select_localisations(&[langid!("en-US")]);
    }
}

#[test]
#[serial(localisation)]
fn select_localisations_switches_language() {
    let _guard = ResetLocale;
    let initial = current_languages()
        .unwrap_or_else(|error| panic!("failed to query current languages: {error}"));
    assert!(
        initial.contains(&langid!("en-US")),
        "English should be active by default"
    );

    select_localisations(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to apply French locale: {error}"));
    let err = StepError::MissingFixture {
        name: "n".into(),
        ty: "u32".into(),
        step: "s".into(),
    };
    assert_eq!(
        strip_directional_isolates(&err.to_string()),
        "La fixture « n » de type « u32 » est introuvable pour la fonction « s »",
    );

    select_localisations(&[langid!("en-US")])
        .unwrap_or_else(|error| panic!("failed to restore English locale: {error}"));
    assert_eq!(
        strip_directional_isolates(&err.to_string()),
        "Missing fixture 'n' of type 'u32' for step function 's'",
    );
}
