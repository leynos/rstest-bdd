//! Column renaming strategies mirroring serde's `rename_all` semantics,
//! with an additional `Title Case` extension.
//!
//! The derive macros rely on these rules to translate struct field identifiers
//! into the column names that should be matched at runtime.

use convert_case::{Case, Casing};
use syn::LitStr;

/// Supported rename rules for datatable headers.
///
/// - `lowercase`: all letters are lowercase.
/// - `UPPERCASE`: all letters are uppercase.
/// - `snake_case`: words are separated by underscores.
/// - `SCREAMING_SNAKE_CASE`: words are separated by underscores and all letters
///   are uppercase.
/// - `kebab-case`: words are separated by hyphens.
/// - `SCREAMING-KEBAB-CASE`: words are separated by hyphens and all letters are
///   uppercase.
/// - `camelCase`: the first word is lowercase and subsequent words are
///   capitalised.
/// - `PascalCase`: each word is capitalised with no separators.
/// - `Title Case`: each word is capitalised and separated by spaces. This is
///   distinct from `PascalCase`, which concatenates words, and mirrors the
///   human-readable headers often used in Gherkin scenarios (for example
///   `Given Name`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenameRule {
    Lower,
    Upper,
    Snake,
    ScreamingSnake,
    Kebab,
    ScreamingKebab,
    Camel,
    Pascal,
    Title,
}

impl RenameRule {
    pub(crate) fn apply(self, ident: &str) -> String {
        match self {
            Self::Lower => ident.to_case(Case::Flat),
            Self::Upper => ident.to_case(Case::UpperFlat),
            Self::Snake => ident.to_case(Case::Snake),
            Self::ScreamingSnake => ident.to_case(Case::UpperSnake),
            Self::Kebab => ident.to_case(Case::Kebab),
            Self::ScreamingKebab => ident.to_case(Case::UpperKebab),
            Self::Camel => ident.to_case(Case::Camel),
            Self::Pascal => ident.to_case(Case::Pascal),
            Self::Title => ident.to_case(Case::Title),
        }
    }
}

impl TryFrom<&LitStr> for RenameRule {
    type Error = syn::Error;

    fn try_from(value: &LitStr) -> Result<Self, Self::Error> {
        match value.value().as_str() {
            "lowercase" => Ok(Self::Lower),
            "UPPERCASE" => Ok(Self::Upper),
            "snake_case" => Ok(Self::Snake),
            "SCREAMING_SNAKE_CASE" => Ok(Self::ScreamingSnake),
            "kebab-case" => Ok(Self::Kebab),
            "SCREAMING-KEBAB-CASE" => Ok(Self::ScreamingKebab),
            "camelCase" => Ok(Self::Camel),
            "PascalCase" => Ok(Self::Pascal),
            "Title Case" => Ok(Self::Title),
            other => Err(syn::Error::new(
                value.span(),
                format!("unsupported rename rule '{other}'"),
            )),
        }
    }
}
