//! Public Rust-indexing entry points and internal scenario-binding results.

use std::path::{Path, PathBuf};

use super::super::{IndexedScenarioBinding, RustStepIndexError, RustStepIndexResult};

/// Rust step and scenario-binding indexes produced by one source traversal.
pub(crate) struct RustSourceIndexResult {
    /// Existing public step-index result.
    pub(crate) steps: RustStepIndexResult,
    /// Internal scenario bindings used by language-server lookups.
    pub(crate) scenario_bindings: Vec<IndexedScenarioBinding>,
}

/// Parse and index a Rust source file from disk.
///
/// # Errors
///
/// Returns an error when the file cannot be read or parsed as Rust source.
///
/// # Examples
///
/// ```
/// use rstest_bdd_server::indexing::index_rust_file;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let path = std::env::temp_dir().join(format!(
///     "rstest-bdd-server-index-rust-file-{}-{}.rs",
///     std::process::id(),
///     std::time::SystemTime::now()
///         .duration_since(std::time::UNIX_EPOCH)?
///         .as_nanos(),
/// ));
/// std::fs::write(&path, "#[given(\"a message\")]\nfn a_message() {}\n")?;
///
/// let index = index_rust_file(&path)?;
/// assert_eq!(index.index.path, path);
///
/// # std::fs::remove_file(&index.index.path).ok();
/// # Ok(())
/// # }
/// ```
pub fn index_rust_file(path: &Path) -> Result<RustStepIndexResult, RustStepIndexError> {
    index_rust_file_with_bindings(path).map(|result| result.steps)
}

/// Parse a Rust file together with its internal scenario-binding metadata.
pub(crate) fn index_rust_file_with_bindings(
    path: &Path,
) -> Result<RustSourceIndexResult, RustStepIndexError> {
    let source = std::fs::read_to_string(path)?;
    index_rust_source_with_bindings(path.to_path_buf(), &source)
}

/// Parse and index Rust step definitions from source text.
///
/// This is intended for language-server integrations that receive saved text
/// from the client and want to avoid a race with filesystem writes.
///
/// # Errors
///
/// Returns an error when the source cannot be parsed by `syn`.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
///
/// use rstest_bdd_server::indexing::index_rust_source;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let source = "#[when]\nfn do_the_thing() {}\n";
/// let index = index_rust_source(PathBuf::from("steps.rs"), source)?;
/// assert_eq!(index.index.step_definitions.len(), 1);
///
/// let step = index.index.step_definitions.first().expect("indexed step");
/// assert_eq!(step.pattern, "do the thing");
/// # Ok(())
/// # }
/// ```
pub fn index_rust_source(
    path: PathBuf,
    source: &str,
) -> Result<RustStepIndexResult, RustStepIndexError> {
    index_rust_source_with_bindings(path, source).map(|result| result.steps)
}

/// Parse Rust source together with its internal scenario-binding metadata.
pub(crate) fn index_rust_source_with_bindings(
    path: PathBuf,
    source: &str,
) -> Result<RustSourceIndexResult, RustStepIndexError> {
    super::parse_rust_source_with_bindings(path, source)
}
