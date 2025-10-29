//! Behavioural coverage for localisation helpers and diagnostics.

use i18n_embed::fluent::fluent_language_loader;
use rstest_bdd::localization::{
    current_languages, install_localization_loader, message, message_with_args,
    select_localizations, strip_directional_isolates, ScopedLocalization,
};
use rstest_bdd::{Localizations, StepError};
use serial_test::serial;
use unic_langid::langid;

#[test]
fn scoped_localization_overrides_current_thread() {
    let english_id = langid!("en-US");
    let base = ScopedLocalization::new(std::slice::from_ref(&english_id))
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
        let french_guard = ScopedLocalization::new(std::slice::from_ref(&french_id))
            .unwrap_or_else(|error| panic!("failed to scope French locale: {error}"));
        let french = strip_directional_isolates(&err.to_string());
        assert_eq!(
            french,
            "La fixture « n » de type « u32 » est introuvable pour la fonction « s »",
        );
        // Hold the scoped guard until the end of the block so the locale stays active.
        let _ = &french_guard;
    }

    // Keep the base guard alive until after the French scope finishes to restore English.
    let _ = &base;
    let restored = strip_directional_isolates(&err.to_string());
    assert_eq!(restored, baseline);
}

#[test]
fn select_localizations_respects_thread_override() {
    let guard = ScopedLocalization::new(&[langid!("en-US")])
        .unwrap_or_else(|error| panic!("failed to scope English locale: {error}"));
    select_localizations(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to switch to French: {error}"));

    let err = StepError::PanicError {
        pattern: "p".into(),
        function: "f".into(),
        message: "boom".into(),
    };
    let display = strip_directional_isolates(&err.to_string());
    let lowered = display.to_lowercase();
    assert!(
        display.contains("Panique") || lowered.contains("panic"),
        "message should reflect locale switch, got: {display}",
    );
    // Keep the scoped localization active for the lifetime of the assertion.
    let _ = &guard;
}

#[test]
fn current_languages_reports_thread_override() {
    let guard = ScopedLocalization::new(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to scope French locale: {error}"));
    let active = current_languages()
        .unwrap_or_else(|error| panic!("failed to query current languages: {error}"));
    assert_eq!(active, vec![langid!("fr"), langid!("en-US")]);
    // Keep the scoped localization active for the lifetime of the assertion.
    let _ = &guard;
}

#[test]
#[serial(localization)]
fn install_localization_loader_replaces_global_loader() {
    let replacement = {
        let loader = fluent_language_loader!();
        i18n_embed::select(&loader, &Localizations, &[langid!("fr")])
            .unwrap_or_else(|error| panic!("failed to prepare replacement loader: {error}"));
        loader
    };

    install_localization_loader(replacement)
        .unwrap_or_else(|error| panic!("failed to install replacement loader: {error}"));

    let languages = current_languages()
        .unwrap_or_else(|error| panic!("failed to query languages after install: {error}"));
    assert_eq!(languages, vec![langid!("fr"), langid!("en-US")]);

    let restore = {
        let loader = fluent_language_loader!();
        i18n_embed::select(&loader, &Localizations, &[langid!("en-US")])
            .unwrap_or_else(|error| panic!("failed to prepare restoration loader: {error}"));
        loader
    };

    install_localization_loader(restore)
        .unwrap_or_else(|error| panic!("failed to restore original loader: {error}"));
}

#[test]
fn select_localizations_falls_back_to_english() {
    let guard = ScopedLocalization::new(&[langid!("en-US")])
        .unwrap_or_else(|error| panic!("failed to scope English locale: {error}"));
    let selected = select_localizations(&[langid!("zz")])
        .unwrap_or_else(|error| panic!("failed to select fallback locale: {error}"));
    assert_eq!(selected, vec![langid!("en-US")]);
    // Keep the scoped localization active for the lifetime of the assertion.
    let _ = &guard;
}

#[test]
fn localizations_embed_resources() {
    let asset = Localizations::get("en-US/rstest-bdd.ftl")
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
    let guard = ScopedLocalization::new(&[langid!("fr")])
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
    // Keep the scoped localization active for the lifetime of the assertion.
    let _ = &guard;
}
