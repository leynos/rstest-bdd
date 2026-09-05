//! Step-library identities and scenario scopes.
//!
//! This module models the closed vocabulary selected before a scenario starts.
//! It deliberately stores only stable Rust module identities: matching never
//! depends on registration order or on previously resolved steps.

/// Stable identity for a declared step library.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StepLibraryId(&'static str);

impl StepLibraryId {
    /// Create an identity from a compile-time module path.
    #[must_use]
    pub const fn new(value: &'static str) -> Self { Self(value) }

    /// Return the stable module-path representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str { self.0 }
}

/// Metadata submitted by `#[step_library]` declarations.
#[derive(Clone, Copy, Debug)]
pub struct StepLibrary {
    /// Stable identity of the library.
    pub id: StepLibraryId,
    /// Rust module name displayed in diagnostics.
    pub name: &'static str,
}

/// Closed set of libraries available to one scenario.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepScope {
    /// Library identities selected for the scenario.
    libraries: &'static [StepLibraryId],
}

impl StepScope {
    /// Create a scope containing exactly `libraries`.
    #[must_use]
    pub const fn new(libraries: &'static [StepLibraryId]) -> Self { Self { libraries } }

    /// Select the built-in global library.
    #[must_use]
    pub const fn global() -> Self { Self::new(&[GLOBAL_STEP_LIBRARY]) }

    /// Return the selected identities in declaration order.
    #[must_use]
    pub const fn libraries(self) -> &'static [StepLibraryId] { self.libraries }
}

/// Identity of the built-in library used by unannotated definitions.
pub const GLOBAL_STEP_LIBRARY: StepLibraryId = StepLibraryId::new("rstest_bdd::global");
