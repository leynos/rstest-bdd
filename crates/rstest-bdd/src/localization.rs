//! Localization utilities used by the public macros and runtime diagnostics.

use std::cell::RefCell;
use std::sync::RwLock;

use fluent::FluentArgs;
use i18n_embed::fluent::{fluent_language_loader, FluentLanguageLoader};
use i18n_embed::I18nEmbedError;
use once_cell::sync::Lazy;
use rust_embed::RustEmbed;
use thiserror::Error;
use unic_langid::LanguageIdentifier;

/// Embedded Fluent resources shipped with the crate.
///
/// The struct implements [`RustEmbed`], allowing callers to seed a
/// [`FluentLanguageLoader`] with the bundled locale files.
///
/// # Examples
/// ```
/// # use rstest_bdd::localization::Localizations;
/// # use i18n_embed::fluent::fluent_language_loader;
/// # use unic_langid::langid;
/// let mut loader = fluent_language_loader!();
/// let selected = i18n_embed::select(&loader, &Localizations, &[langid!("en-US")]).unwrap();
/// assert!(selected.contains(&langid!("en-US")));
/// ```
#[derive(RustEmbed)]
#[folder = "i18n"]
pub struct Localizations;

static LANGUAGE_LOADER: Lazy<RwLock<FluentLanguageLoader>> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    i18n_embed::select(&loader, &Localizations, &[unic_langid::langid!("en-US")])
        .unwrap_or_else(|error| panic!("failed to load default English translations: {error}"));
    RwLock::new(loader)
});

thread_local! {
    static OVERRIDE_LOADER: RefCell<Option<FluentLanguageLoader>> = const { RefCell::new(None) };
}

/// Errors from localization setup and queries.
#[derive(Debug, Error)]
pub enum LocalizationError {
    /// Global or thread-local localization state was poisoned.
    #[error("localization state is poisoned")]
    Poisoned,
    /// Loading or selecting Fluent resources failed.
    #[error("failed to load localization resources: {0}")]
    Loader(#[from] I18nEmbedError),
}

/// RAII guard that installs a thread-local localization loader for the
/// lifetime of the guard.
#[must_use]
pub struct ScopedLocalization {
    previous: Option<FluentLanguageLoader>,
}

impl ScopedLocalization {
    /// Load the requested locales into a dedicated loader and make it the
    /// active loader for the current thread.
    ///
    /// # Errors
    ///
    /// Returns [`LocalizationError::Loader`] if localization resources cannot
    /// be loaded for the requested languages.
    pub fn new(requested: &[LanguageIdentifier]) -> Result<Self, LocalizationError> {
        let loader = fluent_language_loader!();
        i18n_embed::select(&loader, &Localizations, requested)?;
        let previous = OVERRIDE_LOADER.with(|cell| cell.replace(Some(loader)));
        Ok(Self { previous })
    }
}

impl Drop for ScopedLocalization {
    fn drop(&mut self) {
        let previous = self.previous.take();
        OVERRIDE_LOADER.with(|cell| {
            *cell.borrow_mut() = previous;
        });
    }
}

/// Replace the global localization loader with a preconfigured instance.
///
/// # Errors
///
/// Returns [`LocalizationError::Poisoned`] when the global loader lock is poisoned.
pub fn install_localization_loader(loader: FluentLanguageLoader) -> Result<(), LocalizationError> {
    let mut guard = LANGUAGE_LOADER
        .write()
        .map_err(|_| LocalizationError::Poisoned)?;
    *guard = loader;
    Ok(())
}

/// Activate the best matching localizations for the provided language identifiers.
///
/// # Errors
///
/// Returns [`LocalizationError::Poisoned`] if the global loader lock is poisoned
/// or [`LocalizationError::Loader`] when resource selection fails.
pub fn select_localizations(
    requested: &[LanguageIdentifier],
) -> Result<Vec<LanguageIdentifier>, LocalizationError> {
    OVERRIDE_LOADER.with(|cell| -> Result<_, LocalizationError> {
        if let Some(loader) = cell.borrow_mut().as_mut() {
            let selected = i18n_embed::select(loader, &Localizations, requested)?;
            return Ok(selected);
        }
        let guard = LANGUAGE_LOADER
            .read()
            .map_err(|_| LocalizationError::Poisoned)?;
        let selected = i18n_embed::select(&*guard, &Localizations, requested)?;
        Ok(selected)
    })
}

/// Query the currently active localizations.
///
/// # Errors
///
/// Returns [`LocalizationError::Poisoned`] if the loader lock is poisoned.
pub fn current_languages() -> Result<Vec<LanguageIdentifier>, LocalizationError> {
    OVERRIDE_LOADER.with(|cell| -> Result<_, LocalizationError> {
        if let Some(loader) = cell.borrow().as_ref() {
            return Ok(loader.current_languages());
        }
        let guard = LANGUAGE_LOADER
            .read()
            .map_err(|_| LocalizationError::Poisoned)?;
        Ok(guard.current_languages())
    })
}

#[must_use]
/// Retrieve a localised string without interpolation arguments.
///
/// # Examples
/// ```
/// # use rstest_bdd::localization;
/// assert_eq!(
///     localization::message("placeholder-pattern-mismatch"),
///     "pattern mismatch"
/// );
/// ```
pub fn message(id: &str) -> String {
    with_loader(|loader| loader.get(id))
}

#[must_use]
/// Retrieve a localised string with Fluent arguments supplied via a closure.
///
/// # Examples
/// ```
/// # use rstest_bdd::localization;
/// let rendered = localization::message_with_args("panic-message-opaque-payload", |args| {
///     args.set("type", "Example".to_string());
/// });
/// assert!(rendered.contains("Example"));
/// ```
pub fn message_with_args<F>(id: &str, configure: F) -> String
where
    F: FnOnce(&mut FluentArgs<'static>),
{
    with_loader(|loader| message_with_loader(loader, id, configure))
}

pub(crate) fn message_with_loader<F>(
    loader: &FluentLanguageLoader,
    id: &str,
    configure: F,
) -> String
where
    F: FnOnce(&mut FluentArgs<'static>),
{
    let mut args: FluentArgs<'static> = FluentArgs::new();
    configure(&mut args);
    loader.get_args_fluent(id, Some(&args))
}

pub(crate) fn with_loader<R>(callback: impl FnOnce(&FluentLanguageLoader) -> R) -> R {
    OVERRIDE_LOADER.with(|cell| {
        let borrow = cell.borrow();
        if let Some(loader) = borrow.as_ref() {
            return callback(loader);
        }
        drop(borrow);
        let guard = LANGUAGE_LOADER
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        callback(&guard)
    })
}

/// Remove Unicode directional isolates inserted by Fluent during interpolation.
#[must_use]
pub fn strip_directional_isolates(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(*c, '\u{2066}' | '\u{2067}' | '\u{2068}' | '\u{2069}'))
        .collect()
}

/// Panic with a localized message resolved from a Fluent ID and key–value args.
#[macro_export]
macro_rules! panic_localized {
    ($id:expr $(, $key:ident = $value:expr )* $(,)?) => {{
        let message = $crate::localization::message_with_args($id, |args| {
            $( args.set(stringify!($key), $value.to_string()); )*
        });
        panic!("{message}");
    }};
}
