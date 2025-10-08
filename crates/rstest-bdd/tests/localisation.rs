use i18n_embed::fluent::fluent_language_loader;
use rstest_bdd::localisation::{
    ScopedLocalisation, current_languages, install_localisation_loader, message, message_with_args,
    select_localisations, strip_directional_isolates,
};
use rstest_bdd::{Localisations, StepError};
use serial_test::serial;
use unic_langid::langid;

#[test]
fn scoped_localisation_overrides_current_thread() {
    let english_id = langid!("en-US");
    let base = ScopedLocalisation::new(std::slice::from_ref(&english_id))
        .unwrap_or_else(|error| panic!("failed to scope English locale: {error}"));

    let err = StepError::MissingFixture {
        name: "n".into(),
        ty: "u32".into(),
        step: "s".into(),
    };
    let baseline = strip_directional_isolates(&err.to_string());
    assert_eq!(
        baseline,
        "Missing fixture 'n' of type 'u32' for step function 's'"
    );

    {
        let french_id = langid!("fr");
        let french_guard = ScopedLocalisation::new(std::slice::from_ref(&french_id))
            .unwrap_or_else(|error| panic!("failed to scope French locale: {error}"));
        let french = strip_directional_isolates(&err.to_string());
        assert_eq!(
            french,
            "La fixture « n » de type « u32 » est introuvable pour la fonction « s »",
        );
        drop(french_guard);
    }

    let restored = strip_directional_isolates(&err.to_string());
    assert_eq!(restored, baseline);
    drop(base);
}

#[test]
fn select_localisations_respects_thread_override() {
    let guard = ScopedLocalisation::new(&[langid!("en-US")])
        .unwrap_or_else(|error| panic!("failed to scope English locale: {error}"));
    select_localisations(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to switch to French: {error}"));

    let err = StepError::PanicError {
        pattern: "p".into(),
        function: "f".into(),
        message: "boom".into(),
    };
    let display = strip_directional_isolates(&err.to_string());
    assert!(
        display.contains("Panique") || display.contains("panic"),
        "message should reflect locale switch, got: {display}",
    );

    drop(guard);
}

#[test]
fn current_languages_reports_thread_override() {
    let guard = ScopedLocalisation::new(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to scope French locale: {error}"));
    let active = current_languages()
        .unwrap_or_else(|error| panic!("failed to query current languages: {error}"));
    assert_eq!(active, vec![langid!("fr"), langid!("en-US")]);
    drop(guard);
}

#[test]
#[serial(localisation)]
fn install_localisation_loader_replaces_global_loader() {
    let replacement = {
        let loader = fluent_language_loader!();
        i18n_embed::select(&loader, &Localisations, &[langid!("fr")])
            .unwrap_or_else(|error| panic!("failed to prepare replacement loader: {error}"));
        loader
    };

    install_localisation_loader(replacement)
        .unwrap_or_else(|error| panic!("failed to install replacement loader: {error}"));

    let languages = current_languages()
        .unwrap_or_else(|error| panic!("failed to query languages after install: {error}"));
    assert_eq!(languages, vec![langid!("fr"), langid!("en-US")]);

    let restore = {
        let loader = fluent_language_loader!();
        i18n_embed::select(&loader, &Localisations, &[langid!("en-US")])
            .unwrap_or_else(|error| panic!("failed to prepare restoration loader: {error}"));
        loader
    };

    install_localisation_loader(restore)
        .unwrap_or_else(|error| panic!("failed to restore original loader: {error}"));
}

#[test]
fn select_localisations_falls_back_to_english() {
    let guard = ScopedLocalisation::new(&[langid!("en-US")])
        .unwrap_or_else(|error| panic!("failed to scope English locale: {error}"));
    let selected = select_localisations(&[langid!("zz")])
        .unwrap_or_else(|error| panic!("failed to select fallback locale: {error}"));
    assert_eq!(selected, vec![langid!("en-US")]);
    drop(guard);
}

#[test]
fn localisations_embed_resources() {
    let asset = Localisations::get("en-US/rstest-bdd.ftl")
        .unwrap_or_else(|| panic!("expected embedded English translations"));
    let contents = std::str::from_utf8(&asset.data)
        .unwrap_or_else(|error| panic!("embedded translations should be UTF-8: {error}"));
    assert!(
        contents.contains("step-error-missing-fixture"),
        "embedded catalogue should include step error messages"
    );
}

#[test]
fn message_helpers_use_active_locale() {
    let guard = ScopedLocalisation::new(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to scope French locale: {error}"));
    let plain = strip_directional_isolates(&message("assert-step-err-success"));
    assert!(plain.contains("réussi"));
    let detailed = strip_directional_isolates(&message_with_args(
        "assert-step-err-missing-substring",
        |args| {
            args.set("display", "boom".to_string());
            args.set("expected", "snap".to_string());
        },
    ));
    assert!(detailed.contains("boom"));
    drop(guard);
}
