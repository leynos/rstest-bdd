//! Source identity for a scenario plan and its steps.
//!
//! The path is opaque to the runtime: a `.feature` file, a Markdown document,
//! or any other identifier a frontend chooses. Nothing here knows what Gherkin
//! is, which is what lets a non-Gherkin frontend keep its own source paths and
//! line numbers all the way into the outcome.

use std::sync::Arc;

/// A source path that is `const`-constructible on the macro path and shared on
/// the dynamic path.
///
/// # Examples
///
/// ```
/// use rstest_bdd::runner::SourcePath;
///
/// let static_path = SourcePath::Static("notes/arithmetic.md");
/// assert_eq!(static_path.as_str(), "notes/arithmetic.md");
///
/// let shared: SourcePath = std::sync::Arc::<str>::from("spec/cases.toml").into();
/// assert_eq!(shared.as_str(), "spec/cases.toml");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SourcePath {
    /// A path known at compile time; [`Clone`] is a copy.
    Static(&'static str),
    /// A path parsed at run time and shared across a scenario's steps.
    Shared(Arc<str>),
}

impl SourcePath {
    /// Borrow the path as a string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::SourcePath;
    ///
    /// assert_eq!(SourcePath::Static("a.md").as_str(), "a.md");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Static(path) => path,
            Self::Shared(path) => path,
        }
    }
}

impl std::fmt::Display for SourcePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&'static str> for SourcePath {
    fn from(path: &'static str) -> Self { Self::Static(path) }
}

impl From<Arc<str>> for SourcePath {
    fn from(path: Arc<str>) -> Self { Self::Shared(path) }
}

impl From<String> for SourcePath {
    fn from(path: String) -> Self { Self::Shared(Arc::from(path)) }
}

/// A one-based position in a frontend's own source text.
///
/// The path is opaque to the runtime. Columns are measured in Unicode scalar
/// values. Unlike a step's location, a scenario's own line is optional, so it
/// is a plain accessor on
/// [`ScenarioPlan`](crate::runner::ScenarioPlan) rather than a second variant
/// here.
///
/// # Examples
///
/// ```
/// use rstest_bdd::runner::SourceLocation;
///
/// let location = SourceLocation::new_static("notes/arithmetic.md", 43, Some(5));
/// assert_eq!(location.path(), "notes/arithmetic.md");
/// assert_eq!(location.line(), 43);
/// assert_eq!(location.column(), Some(5));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceLocation {
    /// The frontend's own identifier for the document.
    path: SourcePath,
    /// One-based line number.
    line: u32,
    /// One-based column, when the frontend records columns.
    column: Option<u32>,
}

impl SourceLocation {
    /// Construct a location whose path is a compile-time literal.
    ///
    /// # Panics
    ///
    /// Debug builds panic when `line` is zero or when `column` is
    /// `Some(0)`, because both coordinates are one-based. Release builds store
    /// the values unchanged. Positions are only ever produced by a frontend's
    /// parser, so a zero means the parser reported a coordinate it cannot have
    /// observed, and failing loudly at the boundary beats carrying an
    /// unrepresentable position into an outcome.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::SourceLocation;
    ///
    /// const LOCATION: SourceLocation = SourceLocation::new_static("notes/arithmetic.md", 1, None);
    /// assert_eq!(LOCATION.line(), 1);
    /// ```
    #[must_use]
    pub const fn new_static(path: &'static str, line: u32, column: Option<u32>) -> Self {
        debug_assert!(line >= 1, "a source line is one-based");
        // `matches!` rather than `Option::is_none_or` or `Option::map_or`:
        // both are non-const on this toolchain (E0658), so a `const fn` cannot
        // call them. Expressing the rejection directly is also clearer than
        // asserting a negated predicate over a mapped value.
        debug_assert!(!matches!(column, Some(0)), "a source column is one-based");
        Self {
            path: SourcePath::Static(path),
            line,
            column,
        }
    }

    /// Construct a location from any convertible path.
    ///
    /// # Panics
    ///
    /// Debug builds panic when `line` is zero or when `column` is `Some(0)`,
    /// because both coordinates are one-based. Release builds store the values
    /// unchanged. See [`new_static`](Self::new_static) for why a zero is
    /// rejected rather than clamped.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::SourceLocation;
    ///
    /// let location = SourceLocation::new("notes/arithmetic.md", 7, None);
    /// assert_eq!(location.line(), 7);
    /// ```
    #[must_use]
    pub fn new(path: impl Into<SourcePath>, line: u32, column: Option<u32>) -> Self {
        debug_assert!(line >= 1, "a source line is one-based");
        // `matches!` rather than `Option::is_none_or` or `Option::map_or`:
        // both are non-const on this toolchain (E0658), so a `const fn` cannot
        // call them. Expressing the rejection directly is also clearer than
        // asserting a negated predicate over a mapped value.
        debug_assert!(!matches!(column, Some(0)), "a source column is one-based");
        Self {
            path: path.into(),
            line,
            column,
        }
    }

    /// Borrow the source path.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::SourceLocation;
    ///
    /// let location = SourceLocation::new_static("notes/a.md", 2, None);
    /// assert_eq!(location.path(), "notes/a.md");
    /// ```
    #[must_use]
    pub fn path(&self) -> &str { self.path.as_str() }

    /// Return the one-based source line.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::SourceLocation;
    ///
    /// let location = SourceLocation::new_static("notes/a.md", 2, None);
    /// assert_eq!(location.line(), 2);
    /// ```
    #[must_use]
    pub const fn line(&self) -> u32 { self.line }

    /// Return the one-based source column, when one was recorded.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::SourceLocation;
    ///
    /// let location = SourceLocation::new_static("notes/a.md", 2, Some(9));
    /// assert_eq!(location.column(), Some(9));
    /// ```
    #[must_use]
    pub const fn column(&self) -> Option<u32> { self.column }

    /// Borrow the path as a [`SourcePath`], for callers that share one path
    /// across steps.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::runner::{SourceLocation, SourcePath};
    ///
    /// let location = SourceLocation::new_static("notes/a.md", 2, None);
    /// assert_eq!(location.source_path(), &SourcePath::Static("notes/a.md"));
    /// ```
    #[must_use]
    pub const fn source_path(&self) -> &SourcePath { &self.path }
}
