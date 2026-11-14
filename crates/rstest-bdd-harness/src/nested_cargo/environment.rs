//! The filtered environment a nested `cargo` invocation runs with.
//!
//! The rules are one implementation, shared by every nested `cargo` caller:
//!
//! * `CARGO_MAKEFLAGS` is removed (stale jobserver).
//! * `CARGO_PKG_*` is removed (describes a different crate).
//! * `CARGO_LLVM_COV` and every `CARGO_LLVM_COV_*` variable is removed (outer coverage driver
//!   control).
//! * an existing `CARGO_TARGET_DIR` is preserved so nested builds reuse the outer build cache;
//! * when absent, the caller's fallback target directory is used;
//! * a relative `CARGO_TARGET_DIR` is resolved once, here, so every child and every stamp derived
//!   from it name the same directory;
//! * an inherited `LLVM_PROFILE_FILE` is kept by default, and replaced with a unique child-only
//!   scratch destination when the caller asks for [`ProfileDestination::ChildScratch`], so nested
//!   coverage never merges into the parent's gated profile;
//! * unrelated variables are preserved.
//!
//! Keeping the inherited profile is the default because `cargo llvm-cov` merges
//! the `.profraw` files named by the pattern it exported: replacing that
//! pattern silently drops the coverage of every process spawned with the
//! result, which then reads as untested code rather than as an uncollected
//! profile. The destination itself lives in the private `profile` module.

use std::{
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
    process::Command,
};

use super::{ProfileDestination, profile};

/// Build the deterministic nested-Cargo environment for this process.
///
/// Captures the current environment and applies the shared filtering rules;
/// `fallback_target_dir` is used when the parent has no `CARGO_TARGET_DIR`.
///
/// # Errors
///
/// As [`env_from_vars`].
pub fn build_child_env(
    fallback_target_dir: &Path,
    profile_destination: ProfileDestination,
) -> io::Result<NestedCargoEnv> {
    env_from_vars(
        env::vars_os().collect(),
        Some(fallback_target_dir),
        profile_destination,
    )
}

/// Build a nested-Cargo environment from an explicit variable snapshot.
///
/// Exposed for the workspace's own tests, which assert the filtering rules
/// against synthetic values without mutating the real process environment.
/// Production callers should use [`build_child_env`].
///
/// An inherited relative `CARGO_TARGET_DIR` is resolved against this process's
/// working directory, because the child resolves it against its own.
///
/// # Errors
///
/// Returns an error when `CARGO_TARGET_DIR` is relative and the working
/// directory cannot be read. The alternative — passing the relative value
/// through — would silently point the child at a different directory, so this
/// is reported rather than handed on.
///
/// # Panics
///
/// Panics when `vars` has no `CARGO_TARGET_DIR` and no fallback is supplied:
/// a nested Cargo invocation without a target directory would silently build
/// into the caller's manifest directory.
pub fn env_from_vars(
    vars: Vec<(OsString, OsString)>,
    fallback_target_dir: Option<&Path>,
    profile_destination: ProfileDestination,
) -> io::Result<NestedCargoEnv> {
    NestedCargoEnv::from_vars(vars, fallback_target_dir, profile_destination)
}

/// The filtered environment applied to every nested `cargo` invocation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NestedCargoEnv(BTreeMap<OsString, OsString>);

impl NestedCargoEnv {
    /// Build the filtered environment from raw variable pairs.
    ///
    /// Each rule is applied by one named helper below, so the shared policy
    /// reads as the list it documents rather than as nested conditionals.
    ///
    /// # Errors
    ///
    /// As [`absolute_target_directory`]: a relative `CARGO_TARGET_DIR` that
    /// cannot be resolved against the working directory is an error, not a
    /// relative value handed to the child.
    fn from_vars(
        mut vars: Vec<(OsString, OsString)>,
        fallback_target_dir: Option<&Path>,
        profile_destination: ProfileDestination,
    ) -> io::Result<Self> {
        strip_outer_build_vars(&mut vars);
        ensure_target_directory(&mut vars, fallback_target_dir);
        resolve_target_directory(&mut vars)?;
        redirect_profile_file(&mut vars, profile_destination);
        Ok(Self(vars.into_iter().collect()))
    }

    /// Whether `key` is set in the filtered environment.
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool { self.0.contains_key(OsStr::new(key)) }

    /// The value of `key` in the filtered environment, if present.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&OsStr> {
        self.0.get(OsStr::new(key)).map(OsString::as_os_str)
    }

    /// Iterate the filtered environment as sorted `(key, value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&OsString, &OsString)> { self.0.iter() }

    /// The build directory this environment resolves for Cargo's artefacts.
    ///
    /// Always absolute: [`env_from_vars`] resolves an inherited
    /// relative value once, against the current directory of the process that
    /// built the snapshot, so every nested command interprets it as the same
    /// place regardless of the working directory it is given. Callers that
    /// need to describe where artefacts live — a preparation stamp, a
    /// dep-info lookup — must derive the path from here rather than parse
    /// `CARGO_TARGET_DIR` themselves, or a relative value would be resolved a
    /// second time against a different directory.
    ///
    /// Returns `None` only for an environment built without the filtering
    /// constructor, such as [`Default`], and an error only when the value is
    /// relative and the working directory cannot be read — the same condition
    /// [`absolute_target_directory`] reports.
    ///
    /// # Errors
    ///
    /// As [`absolute_target_directory`].
    pub fn target_directory(&self) -> io::Result<Option<PathBuf>> {
        self.0
            .get(OsStr::new("CARGO_TARGET_DIR"))
            .map(|value| absolute_target_directory(value))
            .transpose()
    }

    /// Apply the environment to `cmd`, starting from an empty environment.
    pub fn apply_to(&self, cmd: &mut Command) {
        cmd.env_clear();
        for (key, value) in &self.0 {
            cmd.env(key, value);
        }
    }
}

/// Remove the variables that describe the *outer* build or crate.
///
/// `CARGO_MAKEFLAGS` names a jobserver a nested process cannot open, which
/// stalls on Windows; `CARGO_PKG_*` describes a different crate; and
/// `CARGO_LLVM_COV*` is the coverage driver's own control surface.
fn strip_outer_build_vars(vars: &mut Vec<(OsString, OsString)>) {
    vars.retain(|(key, _)| {
        let key = key.to_string_lossy();
        key != "CARGO_MAKEFLAGS"
            && !key.starts_with("CARGO_PKG_")
            && !key.starts_with("CARGO_LLVM_COV")
    });
}

/// Give the child a target directory when the parent named none.
///
/// An inherited value wins, so CI's coverage target directory keeps nested
/// builds warm; otherwise the caller's fallback applies. A caller that
/// supplies neither is a programming error: a nested Cargo invocation with no
/// target directory would build into the caller's manifest directory.
///
/// # Panics
///
/// Panics when `vars` has no `CARGO_TARGET_DIR` and no fallback is supplied.
fn ensure_target_directory(vars: &mut Vec<(OsString, OsString)>, fallback: Option<&Path>) {
    if vars
        .iter()
        .any(|(key, _)| key.to_string_lossy() == "CARGO_TARGET_DIR")
    {
        return;
    }
    let Some(fallback) = fallback else {
        panic!(concat!(
            "no CARGO_TARGET_DIR in the parent environment and no fallback target ",
            "directory provided"
        ));
    };
    vars.push((
        OsString::from("CARGO_TARGET_DIR"),
        fallback.as_os_str().to_os_string(),
    ));
}

/// Rewrite an inherited relative target directory to an absolute one.
///
/// Cargo reads the value relative to the working directory of the command
/// that consults it, and nested builds run in a fixture directory rather than
/// here. Resolving once, before the value reaches any child, is what keeps the
/// directory the child builds in and the directory a stamp is derived from the
/// same place.
///
/// A value that cannot be resolved is reported rather than passed on as a
/// relative path. A relative path in the child names the fixture directory
/// instead of the directory the caller meant, so the failure would surface
/// later as a cold rebuild or as a stamp that guards the wrong tree — neither
/// of which names the working directory as the cause.
fn resolve_target_directory(vars: &mut [(OsString, OsString)]) -> io::Result<()> {
    for (key, value) in vars {
        if key == OsStr::new("CARGO_TARGET_DIR") {
            *value = absolute_target_directory(value)?.into_os_string();
        }
    }
    Ok(())
}

/// Move a child's coverage output off the parent's gated profile.
///
/// A child writing into the parent's `.profraw` pattern would merge build-time
/// coverage into a measurement it does not belong to. Only
/// [`ProfileDestination::ChildScratch`] asks for this: the process under test
/// keeps the inherited pattern, because its coverage *is* the measurement and
/// a scratch path outside the coverage directory loses it silently.
fn redirect_profile_file(vars: &mut Vec<(OsString, OsString)>, destination: ProfileDestination) {
    if destination != ProfileDestination::ChildScratch {
        return;
    }
    if !vars
        .iter()
        .any(|(key, _)| key.to_string_lossy() == "LLVM_PROFILE_FILE")
    {
        return;
    }
    vars.retain(|(key, _)| key.to_string_lossy() != "LLVM_PROFILE_FILE");
    vars.push((
        OsString::from("LLVM_PROFILE_FILE"),
        profile::unique_profile_path().into_os_string(),
    ));
}

/// Resolve a `CARGO_TARGET_DIR` value to an absolute path.
///
/// Cargo resolves a relative `CARGO_TARGET_DIR` against the working directory
/// of the command that reads it, and nested builds here deliberately run in a
/// fixture directory rather than in the workspace. A relative value therefore
/// names one place for the test process and a different place for the nested
/// build, so a preparation stamp derived from the test's reading would guard a
/// tree the build never writes. Resolving once, before the value reaches any
/// child, keeps both looking at the same directory.
///
/// An already-absolute path is returned unchanged. A relative one is joined to
/// the working directory, and the lookup is the only step that can fail.
///
/// # Errors
///
/// Returns the error from reading the current directory. Returning the
/// relative path instead would hand the child a value that names the fixture
/// directory rather than the caller's, which is the silent misresolution this
/// helper exists to prevent.
pub fn absolute_target_directory(value: &OsStr) -> io::Result<PathBuf> {
    absolute_target_directory_with(value, env::current_dir)
}

/// [`absolute_target_directory`] with the working-directory lookup supplied.
///
/// The lookup is a parameter so the failure path is reachable from a test: no
/// portable fixture removes the process's own working directory, and deleting
/// the directory this process runs in would disturb every other test in the
/// binary. A test passes a closure that returns its own error instead.
fn absolute_target_directory_with<F>(value: &OsStr, lookup: F) -> io::Result<PathBuf>
where
    F: FnOnce() -> io::Result<PathBuf>,
{
    let path = Path::new(value);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    lookup().map(|cwd| cwd.join(path)).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "cannot resolve the relative target directory {} to an absolute path because the \
                 working directory cannot be read ({error}); a nested build would resolve it \
                 against its own working directory instead",
                path.display()
            ),
        )
    })
}

/// Describe the filtered environment for inclusion in failure messages.
#[must_use]
pub fn describe_env(env: &NestedCargoEnv) -> String {
    env.0
        .iter()
        .map(|(key, value)| format!("{}={}", key.to_string_lossy(), value.to_string_lossy()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    //! The target-directory resolution's failure path.
    //!
    //! The success paths are covered where they are used, through
    //! [`NestedCargoEnv`]; what cannot be reached from those is a working
    //! directory that cannot be read. No portable fixture removes the
    //! directory the test process is running in, so the lookup is injected.

    use std::{
        ffi::OsStr,
        io,
        path::{Path, PathBuf},
    };

    use super::absolute_target_directory_with;

    #[test]
    fn an_absolute_value_never_consults_the_working_directory() {
        // The lookup is not merely unused here, it must not be called: an
        // absolute path is already resolved, and a process whose working
        // directory has been deleted still names a perfectly good absolute
        // target. Panicking in the lookup states that rather than asserting
        // the result and hoping.
        let resolved = absolute_target_directory_with(Path::new("/abs/target").as_os_str(), || {
            panic!("an absolute target directory must not consult the working directory")
        })
        .expect("an absolute path is resolved without any lookup");
        assert_eq!(resolved, PathBuf::from("/abs/target"));
    }

    #[test]
    fn an_unreadable_working_directory_is_reported_rather_than_left_relative() {
        // The regression: this used to fall back to the relative path, which
        // then named one directory for this process and another for a child
        // running in a fixture directory. The error must name the value and
        // the cause, because the caller has no other way to learn that the
        // working directory is what went wrong.
        let error = absolute_target_directory_with(OsStr::new("relative-cache"), || {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "the working directory is gone",
            ))
        })
        .expect_err("an unreadable working directory must not yield a path");

        assert_eq!(
            error.kind(),
            io::ErrorKind::NotFound,
            "the cause's kind must survive: {error}"
        );
        let message = error.to_string();
        assert!(
            message.contains("relative-cache"),
            "the unresolved value must be named: {message}"
        );
        assert!(
            message.contains("the working directory is gone"),
            "the underlying cause must be named: {message}"
        );
    }

    #[test]
    fn a_relative_value_is_joined_to_the_read_working_directory() {
        let resolved = absolute_target_directory_with(OsStr::new("relative-cache"), || {
            Ok(PathBuf::from("/some/cwd"))
        })
        .expect("a readable working directory resolves a relative value");
        assert_eq!(resolved, PathBuf::from("/some/cwd/relative-cache"));
    }
}
