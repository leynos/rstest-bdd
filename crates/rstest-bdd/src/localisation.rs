//! Localisation utilities used by the public macros and runtime diagnostics.

use std::cell::RefCell;
use std::sync::{LazyLock, RwLock};

use fluent::FluentArgs;
use i18n_embed::I18nEmbedError;
use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use rust_embed::RustEmbed;
use thiserror::Error;
use unic_langid::LanguageIdentifier;

#[derive(RustEmbed)]
#[folder = "i18n"]
pub struct Localisations;

static LANGUAGE_LOADER: LazyLock<RwLock<FluentLanguageLoader>> = LazyLock::new(|| {
    let loader = fluent_language_loader!();
    i18n_embed::select(&loader, &Localisations, &[unic_langid::langid!("en-US")])
        .unwrap_or_else(|error| panic!("failed to load default English translations: {error}"));
    RwLock::new(loader)
});

thread_local! {
    static OVERRIDE_LOADER: RefCell<Option<FluentLanguageLoader>> = const { RefCell::new(None) };
}

/// Errors from localisation setup and queries.
#[derive(Debug, Error)]
pub enum LocalisationError {
    /// Global or thread-local localisation state was poisoned.
    #[error("localisation state is poisoned")]
    Poisoned,
    /// Loading or selecting Fluent resources failed.
    #[error("failed to load localisation resources: {0}")]
    Loader(#[from] I18nEmbedError),
}

/// RAII guard that installs a thread-local localisation loader for the
/// lifetime of the guard.
#[must_use]
pub struct ScopedLocalisation {
    previous: Option<FluentLanguageLoader>,
}

impl ScopedLocalisation {
    /// Load the requested locales into a dedicated loader and make it the
    /// active loader for the current thread.
    ///
    /// # Errors
    ///
    /// Returns [`LocalisationError::Loader`] if localisation resources cannot
    /// be loaded for the requested languages.
    pub fn new(requested: &[LanguageIdentifier]) -> Result<Self, LocalisationError> {
        let loader = fluent_language_loader!();
        i18n_embed::select(&loader, &Localisations, requested)?;
        let previous = OVERRIDE_LOADER.with(|cell| cell.replace(Some(loader)));
        Ok(Self { previous })
    }
}

impl Drop for ScopedLocalisation {
    fn drop(&mut self) {
        let previous = self.previous.take();
        OVERRIDE_LOADER.with(|cell| {
            *cell.borrow_mut() = previous;
        });
    }
}

/// Replace the global localisation loader with a preconfigured instance.
///
/// # Errors
///
/// Returns [`LocalisationError::Poisoned`] when the global loader lock is poisoned.
pub fn install_localisation_loader(loader: FluentLanguageLoader) -> Result<(), LocalisationError> {
    let mut guard = LANGUAGE_LOADER
        .write()
        .map_err(|_| LocalisationError::Poisoned)?;
    *guard = loader;
    Ok(())
}

/// Activate the best matching localisations for the provided language identifiers.
///
/// # Errors
///
/// Returns [`LocalisationError::Poisoned`] if the global loader lock is poisoned
/// or [`LocalisationError::Loader`] when resource selection fails.
pub fn select_localisations(
    requested: &[LanguageIdentifier],
) -> Result<Vec<LanguageIdentifier>, LocalisationError> {
    OVERRIDE_LOADER.with(|cell| -> Result<_, LocalisationError> {
        if let Some(loader) = cell.borrow_mut().as_mut() {
            let selected = i18n_embed::select(loader, &Localisations, requested)?;
            return Ok(selected);
        }
        let guard = LANGUAGE_LOADER
            .read()
            .map_err(|_| LocalisationError::Poisoned)?;
        let selected = i18n_embed::select(&*guard, &Localisations, requested)?;
        Ok(selected)
    })
}

/// Query the currently active localisations.
///
/// # Errors
///
/// Returns [`LocalisationError::Poisoned`] if the loader lock is poisoned.
pub fn current_languages() -> Result<Vec<LanguageIdentifier>, LocalisationError> {
    OVERRIDE_LOADER.with(|cell| -> Result<_, LocalisationError> {
        if let Some(loader) = cell.borrow().as_ref() {
            return Ok(loader.current_languages());
        }
        let guard = LANGUAGE_LOADER
            .read()
            .map_err(|_| LocalisationError::Poisoned)?;
        Ok(guard.current_languages())
    })
}

#[must_use]
pub fn message(id: &str) -> String {
    with_loader(|loader| loader.get(id))
}

#[must_use]
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

/// Panic with a localised message resolved from a Fluent ID and key–value args.
#[macro_export]
macro_rules! panic_localised {
    ($id:expr $(, $key:ident = $value:expr )* $(,)?) => {{
        let message = $crate::localisation::message_with_args($id, |args| {
            $( args.set(stringify!($key), $value.to_string()); )*
        });
        panic!("{message}");
    }};
}
