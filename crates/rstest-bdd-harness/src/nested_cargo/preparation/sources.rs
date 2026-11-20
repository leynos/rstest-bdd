//! What a caller tells a preparation, and what it reports back.
//!
//! [`Sources`] is the input side: a family name, a variant label and a hash of
//! the inputs it builds. [`Directories`] is where that family stamps and where
//! its nested build runs. [`PreparationSummary`] is the output side: what one
//! run of that family's preparation commands actually did, for the log line.
//! All three live beside [`Preparation`](super::Preparation) rather than inside
//! it because they are the ends of the same exchange and are never used
//! together by the same code. [`env_for_directory`] is here too: it is how a
//! caller turns the directory it stamps into the environment its child gets.

use std::{
    env,
    io,
    path::{Path, PathBuf},
};

use crate::nested_cargo::{NestedCargoEnv, ProfileDestination, env_from_vars};

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

/// Filter the inherited environment for a child that builds in `directory`.
///
/// `CARGO_TARGET_DIR` is dropped from the inherited values because the
/// directory being stamped is the authority: a stale inherited value would
/// point the build somewhere the stamp does not describe. Callers that only
/// need an environment for a nested build should reach for
/// [`build_child_env`](crate::nested_cargo::build_child_env) or [`env_from_vars`] instead: this
/// helper serves [`Preparation::for_fixture`](super::Preparation::for_fixture), because dropping
/// the inherited value is a decision about what a *stamp* describes rather than a general filtering
/// rule.
///
/// # Parameters
/// - `directory`: The directory being stamped, which supplies the `CARGO_TARGET_DIR` that replaces
///   the dropped value.
/// - `profile_destination`: Where the child's profile output goes, passed to [`env_from_vars`]
///   unchanged.
///
/// # Returns
/// The inherited environment less `CARGO_TARGET_DIR`, plus the resolved target
/// directory: what the nested build receives, not what this process holds.
///
/// # Errors
///
/// As [`env_from_vars`]. The dropped variable is re-supplied from `directory`,
/// so this reports only when `directory` itself is relative and the working
/// directory cannot be read.
pub fn env_for_directory(
    directory: &Path,
    profile_destination: ProfileDestination,
) -> io::Result<NestedCargoEnv> {
    env_from_vars(
        env::vars_os()
            .filter(|(key, _)| key != "CARGO_TARGET_DIR")
            .collect(),
        Some(directory),
        profile_destination,
    )
}
