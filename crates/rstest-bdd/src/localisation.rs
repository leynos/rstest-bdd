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

#[derive(Debug, Error)]
pub enum LocalisationError {
    #[error("localisation state is poisoned")]
    Poisoned,
    #[error("failed to load localisation resources: {0}")]
    Loader(#[from] I18nEmbedError),
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
    let guard = LANGUAGE_LOADER
        .read()
        .map_err(|_| LocalisationError::Poisoned)?;
    let selected = i18n_embed::select(&*guard, &Localisations, requested)?;
    Ok(selected)
}

/// Query the currently active localisations.
///
/// # Errors
///
/// Returns [`LocalisationError::Poisoned`] if the loader lock is poisoned.
pub fn current_languages() -> Result<Vec<LanguageIdentifier>, LocalisationError> {
    let guard = LANGUAGE_LOADER
        .read()
        .map_err(|_| LocalisationError::Poisoned)?;
    Ok(guard.current_languages())
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
    let guard = LANGUAGE_LOADER
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    callback(&guard)
}
