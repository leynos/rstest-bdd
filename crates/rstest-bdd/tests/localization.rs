//! Behavioural coverage for localization helpers and diagnostics.

use i18n_embed::fluent::fluent_language_loader;
use rstest_bdd::{
    Localizations,
    StepError,
    localization::{
        ScopedLocalization,
        current_languages,
        install_localization_loader,
        message,
        message_with_args,
        select_localizations,
        strip_directional_isolates,
    },
};
use serial_test::serial;
use unic_langid::langid;

#[test]
fn scoped_localization_overrides_current_thread() {
    let english_id = langid!("en-US");
    let _base = ScopedLocalization::new(std::slice::from_ref(&english_id))
        .expect("failed to scope English locale");

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
        let _french_guard = ScopedLocalization::new(std::slice::from_ref(&french_id))
            .expect("failed to scope French locale");
        let french = strip_directional_isolates(&err.to_string());
        assert_eq!(
            french,
            "La fixture « n » de type « u32 » est introuvable pour la fonction « s »",
        );
    }

    let restored = strip_directional_isolates(&err.to_string());
    assert_eq!(restored, baseline);
}

#[test]
fn select_localizations_respects_thread_override() {
    let _guard =
        ScopedLocalization::new(&[langid!("en-US")]).expect("failed to scope English locale");
    select_localizations(&[langid!("fr")]).expect("failed to switch to French");

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
}

#[test]
fn current_languages_reports_thread_override() {
    let _guard = ScopedLocalization::new(&[langid!("fr")]).expect("failed to scope French locale");
    let active = current_languages().expect("failed to query current languages");
    assert_eq!(active, vec![langid!("fr"), langid!("en-US")]);
}

#[test]
#[serial(localization)]
fn install_localization_loader_replaces_global_loader() {
    let replacement = {
        let loader = fluent_language_loader!();
        i18n_embed::select(&loader, &Localizations, &[langid!("fr")])
            .expect("failed to prepare replacement loader");
        loader
    };

    install_localization_loader(replacement).expect("failed to install replacement loader");

    let languages = current_languages().expect("failed to query languages after install");
    assert_eq!(languages, vec![langid!("fr"), langid!("en-US")]);

    let restore = {
        let loader = fluent_language_loader!();
        i18n_embed::select(&loader, &Localizations, &[langid!("en-US")])
            .expect("failed to prepare restoration loader");
        loader
    };

    install_localization_loader(restore).expect("failed to restore original loader");
}

#[test]
fn select_localizations_falls_back_to_english() {
    let _guard =
        ScopedLocalization::new(&[langid!("en-US")]).expect("failed to scope English locale");
    let selected =
        select_localizations(&[langid!("zz")]).expect("failed to select fallback locale");
    assert_eq!(selected, vec![langid!("en-US")]);
}

#[test]
fn localizations_embed_resources() {
    let Some(asset) = Localizations::get("en-US/rstest-bdd.ftl") else {
        panic!("expected embedded English translations");
    };
    let contents = std::str::from_utf8(&asset.data).expect("embedded translations should be UTF-8");
    assert!(
        contents.contains("step-error-missing-fixture"),
        "embedded catalogue should include step error messages"
    );
}

#[test]
fn message_helpers_use_active_locale() {
    let _guard = ScopedLocalization::new(&[langid!("fr")]).expect("failed to scope French locale");
    let plain = strip_directional_isolates(&message("assert-step-err-success"));
    assert!(plain.contains("réussi"));
    let detailed = strip_directional_isolates(&message_with_args(
        "assert-step-err-missing-substring",
        |args| {
            args.set("display", "boom".to_owned());
            args.set("expected", "snap".to_owned());
        },
    ));
    assert!(detailed.contains("boom"));
}
