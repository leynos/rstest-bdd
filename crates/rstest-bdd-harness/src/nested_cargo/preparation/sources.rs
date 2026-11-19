//! What a caller tells a preparation, and what it reports back.
//!
//! [`Sources`] is the input side: a family name, a variant label and a hash of
//! the inputs it builds. [`Directories`] is where that family stamps and where
//! its nested build runs. [`PreparationSummary`] is the output side: what one
//! run of that family's preparation commands actually did, for the log line.
//! All three live beside [`Preparation`](super::Preparation) rather than inside
//! it because they are the ends of the same exchange and are never used
//! together by the same code.

use std::path::PathBuf;

/// What a preparation builds, as the caller describes it.
///
/// These three travel together: a family is named, it builds a variant, and
/// what it builds is identified by a hash of its inputs. Grouping them keeps
/// [`Preparation::new`](super::Preparation::new) readable and stops a caller
/// from pairing one family's name with another's input hash.
#[derive(Debug, Clone)]
pub struct Sources {
    /// Stable kebab-case family name, used in the stamp file name and logs.
    pub family: String,
    /// Caller-supplied label for the feature set or tool variant, such as
    /// `all-features` or the resolved Clippy version.
    pub variant: String,
    /// Caller-supplied hash of the fixture sources and lockfiles.
    pub inputs: String,
}

impl Sources {
    /// Name a family, its variant, and the hash of the inputs it builds.
    ///
    /// # Parameters
    /// - `family`: The stable kebab-case family name, used in the stamp file name and the log line.
    /// - `variant`: The caller's label for the feature set or tool variant, such as `all-features`
    ///   or a resolved Clippy version.
    /// - `inputs`: The hash of the fixture sources and lockfiles, as
    ///   [`input_hash`](super::input_hash) computes it.
    ///
    /// # Returns
    /// The three values grouped, unconverted beyond [`Into`], so a caller may
    /// pass borrowed strings without owning them first.
    #[must_use]
    pub fn new(
        family: impl Into<String>,
        variant: impl Into<String>,
        inputs: impl Into<String>,
    ) -> Self {
        Self {
            family: family.into(),
            variant: variant.into(),
            inputs: inputs.into(),
        }
    }
}

/// The two directories a preparation is described by.
///
/// They are grouped rather than passed one after another because the pair is
/// exactly what a caller gets wrong: the stamped root is where the stamp is
/// written and, for a fixture family, the child's target directory, while the
/// working directory is where the nested build actually runs. Naming them in a
/// struct keeps a caller from transposing two `PathBuf`s that have the same
/// type — a mistake the compiler cannot catch and a stamp would record.
#[derive(Debug, Clone)]
pub struct Directories {
    /// The stamped root, and the target directory the child receives.
    pub root: PathBuf,
    /// The directory the nested build runs in; see
    /// [`Preparation::working_directory`](super::Preparation::working_directory).
    pub working: PathBuf,
}

impl Directories {
    /// Name a stamped root and the directory its nested build runs in.
    ///
    /// # Parameters
    /// - `root`: The stamped root, which for a fixture family is also the child's target directory.
    /// - `working`: The directory the nested build runs in, which is the stamped root only when the
    ///   build runs there.
    ///
    /// # Returns
    /// The two grouped, unconverted beyond [`Into`], so a caller may pass
    /// borrowed paths.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, working: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            working: working.into(),
        }
    }
}

/// What one family's preparation did, for the log line.
#[derive(Debug, Default, Clone, Copy)]
pub struct PreparationSummary {
    /// Number of `Compiling` lines the nested builds emitted.
    pub compiled_units: usize,
}
