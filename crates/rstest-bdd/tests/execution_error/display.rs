//! Display and locale matrix tests for execution errors.

use rstest::rstest;
use rstest_bdd::{
    execution::ExecutionError,
    localization::{ScopedLocalization, strip_directional_isolates},
};
use unic_langid::{LanguageIdentifier, langid};

use super::helpers::{
    handler_failed,
    missing_fixtures,
    skip_with_message,
    skip_without_message,
    step_not_found,
};

#[rstest]
#[case::skip_without_message(skip_without_message(), "Step skipped")]
#[case::skip_with_message(
    skip_with_message("not implemented yet"),
    "Step skipped: not implemented yet"
)]
#[case::step_not_found(
    step_not_found(),
    "Step not found at index 3: Given a user named Alice (feature: features/auth.feature, \
     scenario: User login)"
)]
#[case::missing_fixtures(
    missing_fixtures(),
    "Step 'a database connection' (defined at tests/steps.rs:42) requires fixtures db, cache, but \
     the following are missing: db. Requested fixture information: db: DbPool. Available fixtures \
     from scenario: cache, config  (feature: features/db.feature, scenario: Database query)"
)]
#[case::handler_failed(
    handler_failed(),
    "Step failed at index 1: When the user clicks submit - Error executing step 'the user clicks \
     submit' via function 'click_submit': button not found (feature: features/form.feature, \
     scenario: Form submission)"
)]
fn execution_error_display_uses_localized_messages_and_context(
    #[case] error: ExecutionError,
    #[case] expected: &str,
) {
    // Scope to en-US to avoid environment-dependent output on non-English systems
    let _guard = match ScopedLocalization::new(&[langid!("en-US")]) {
        Ok(guard) => guard,
        Err(e) => panic!("en-US locale should always be available: {e}"),
    };
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
    let _guard = match ScopedLocalization::new(std::slice::from_ref(&locale)) {
        Ok(guard) => guard,
        Err(e) => panic!("failed to scope locale {locale}: {e}"),
    };
    assert_eq!(strip_directional_isolates(&error.to_string()), expected);
}
