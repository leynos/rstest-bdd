//! Cargo's own configuration as a preparation input.
//!
//! Cargo discovers configuration by walking from a command's working directory
//! up to the filesystem root, and additionally reads `$CARGO_HOME/config.toml`.
//! Those files set development profile options, linker flags and, in
//! `CARGO_HOME`, the directories Cargo binds mounts from, and changing any of
//! them changes dependency artefacts without changing a single fixture source.
//! A stamp that ignored them would survive a configuration-only edit, and the
//! timed assertion would then compile the graph the preparation was meant to
//! have built.
//!
//! The search is a separate module from the hash it feeds because it is a
//! separate question — *which* files Cargo reads — with its own discovery walk,
//! its own resolution rule for a relative `CARGO_HOME`, and its own failure
//! modes. [`cargo_config_inputs`] answers that question; the caller hashes the
//! paths it returns alongside the fixture roots.

use std::{
    ffi::OsStr,
    fs,
    io,
    path::{Path, PathBuf},
};

/// Paths of the Cargo configuration files that apply at `workspace_root`.
///
/// Cargo discovers configuration by walking from a command's working directory
/// up to the filesystem root, and additionally reads `$CARGO_HOME/config.toml`.
/// A nested fixture build beneath the workspace therefore applies the workspace
/// `.cargo/config.toml` *and* any configuration above it. Those files set
/// development profile options, the Linux linker flags and, in `CARGO_HOME`,
/// the directories Cargo binds mounts from; changing any of them changes
/// dependency artefacts without changing a single fixture source. A stamp that
/// ignored them would survive a configuration-only edit, and the timed
/// assertion would then compile the graph the preparation was meant to have
/// built. The regressions are
/// `tests::cargo_config::a_cargo_home_setting_change_invalidates_the_prepared_tree`
/// for the `CARGO_HOME` case and
/// `tests::cargo_config::relocating_identical_configuration_invalidates_the_prepared_tree`
/// for the path-valued settings each file carries.
///
/// Every ancestor of the workspace root is walked, so a checkout nested inside
/// a larger tree picks up the configuration Cargo would read there, plus
/// `$CARGO_HOME/config.toml` when `CARGO_HOME` is set. Cargo accepts
/// `config.toml` or the legacy `config` name and a checkout may commit
/// neither, so a missing file is skipped rather than named.
///
/// A relative `CARGO_HOME` is resolved against `workspace_root` rather than
/// against this process's directory. Cargo resolves it against the working
/// directory of the command that reads it, and the nested fixture builds run
/// beneath the workspace, not in this process's directory. Left unresolved, a
/// `CARGO_HOME=.cargo` would have the probe read this process's tree while the
/// build read the fixture's, so editing the fixture's own Cargo home could
/// leave a warm stamp matching and move compilation back into the timed test.
/// An absolute value is used unchanged, which is also what Cargo does.
///
/// # Parameters
/// - `workspace_root`: The directory the nested fixture builds run beneath, from which the ancestor
///   walk starts and against which a relative `CARGO_HOME` is resolved.
///
/// # Returns
/// The paths of the configuration files that apply there, nearest first, in
/// the order Cargo consults them. A directory that holds neither accepted name
/// contributes nothing, so an empty result means no configuration applies.
///
/// # Errors
///
/// Returns the error when any path cannot be inspected. The probe is fallible
/// for the same reason [`input_hash`](super::input_hash) is: a path that exists but cannot be
/// examined — a permission denial or a symlink loop — must not be reported as
/// absent. That answer would leave the configuration out of the fingerprint, so
/// a stamp would survive a configuration change the build would notice, which is
/// the gap this function exists to close. A missing file is the only error
/// treated as "no configuration".
pub fn cargo_config_inputs(workspace_root: &Path) -> io::Result<Vec<PathBuf>> {
    let cargo_home = cargo_home(workspace_root);
    cargo_config_inputs_with(workspace_root, cargo_home.as_deref())
}

/// [`cargo_config_inputs`] with the Cargo home supplied rather than resolved.
///
/// The resolution reads process-wide environment, which makes it the one part
/// of the search a test cannot vary in place: [`std::env::set_var`] is unsafe
/// against concurrent readers, and this binary's cases run in parallel. Taking
/// the value as a parameter lets every rule about *which* files apply be
/// exercised for any Cargo home at all, including the absent one, without a
/// single case touching the environment.
pub(crate) fn cargo_config_inputs_with(
    workspace_root: &Path,
    cargo_home: Option<&Path>,
) -> io::Result<Vec<PathBuf>> {
    let mut present = Vec::new();
    for directory in config_search_dirs(workspace_root, cargo_home) {
        for entry in CONFIG_ENTRIES {
            let path = directory.join(entry);
            if config_entry_is_present(&path)? {
                present.push(path);
            }
        }
    }
    Ok(present)
}

/// The two names Cargo accepts for a configuration file in a `.cargo` directory.
const CONFIG_ENTRIES: [&str; 2] = ["config.toml", "config"];

/// Whether a configuration file exists, with absence kept distinct from failure.
///
/// The distinction is the whole point of this function: a missing file means
/// "no configuration here" and must be skipped, while a file that exists but
/// cannot be examined means the fingerprint cannot be trusted and must refuse
/// the preparation. Only [`io::ErrorKind::NotFound`] is the former.
///
/// # Errors
///
/// Returns every error other than [`io::ErrorKind::NotFound`], unchanged.
fn config_entry_is_present(path: &Path) -> io::Result<bool> {
    match fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// The `.cargo` directories Cargo searches, nearest first.
///
/// `workspace_root` and each of its ancestors contribute their `.cargo`
/// subdirectory, matching Cargo's walk from the working directory upwards.
/// `cargo_home` — `$CARGO_HOME` resolved by [`cargo_home`] — is appended when
/// it is known, because Cargo reads it in addition to the walk rather than as
/// part of it.
///
/// When `CARGO_HOME` is unset or empty, Cargo falls back to `$HOME/.cargo`;
/// [`cargo_home`] names that default here. Leaving it out would make the
/// fingerprint blind to a `~/.cargo/config.toml` that sets rustflags, a target
/// directory or a linker — a change the nested build would notice and the stamp
/// would not, which is the unsoundness this module exists to remove.
///
/// The Cargo home arrives as a value rather than being resolved here so that
/// the search over any given pair can be exercised without changing the
/// process's environment. A test that set `CARGO_HOME` around a call would be
/// changing what every other thread in the same binary reads, and this module's
/// own cases would then race each other for it.
fn config_search_dirs(workspace_root: &Path, cargo_home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = search_root(workspace_root)
        .ancestors()
        .map(|ancestor| ancestor.join(".cargo"))
        .collect();
    if let Some(cargo_home) = cargo_home {
        dirs.push(cargo_home.to_path_buf());
    }
    dirs
}

/// The absolute directory whose ancestors Cargo's walk visits.
///
/// [`Path::ancestors`] is a pure lexical operation, so a relative argument
/// yields a relative ancestry: `ancestors` of `target/tests/fixture` stops at
/// `target/tests/fixture`, `target/tests`, `target` and the empty path, and
/// never reaches the checkout's own `.cargo` or anything above it. Resolving
/// the directory against the current directory first makes the walk the same
/// one an absolute argument of the same directory produces.
///
/// This is not a cosmetic difference. The gate takes the family's scratch
/// directory as its root, and that value's shape depends on the caller rather
/// than on the tree, so a relative root silently dropped every configuration
/// file above it from the fingerprint — and a stamp written without them
/// survives an edit to one, which is the invalidation this module exists to
/// force.
///
/// A directory that cannot be resolved — it does not exist yet, or a component
/// cannot be inspected — is walked as given. The preparation creates the tree
/// it stamps, so on a first run the root may be absent; falling back keeps the
/// walk working, and the fingerprint's other inputs still cover the tree once
/// it exists.
fn search_root(workspace_root: &Path) -> PathBuf {
    fs::canonicalize(workspace_root).unwrap_or_else(|_| lexical_root(workspace_root))
}

/// The walk's root when `workspace_root` cannot be resolved.
///
/// The directory may not exist yet — the preparation creates the tree it stamps
/// — and canonicalizing a relative path also needs it to exist beneath this
/// process's directory. Joining the process directory keeps the walk absolute,
/// which is the property that matters here; the fingerprint still covers the
/// tree through its other inputs once it exists.
fn lexical_root(workspace_root: &Path) -> PathBuf {
    if workspace_root.is_absolute() {
        return workspace_root.to_path_buf();
    }
    std::env::current_dir().map_or_else(
        |_| workspace_root.to_path_buf(),
        |directory| directory.join(workspace_root),
    )
}

/// The Cargo home directory, honouring `CARGO_HOME` and Cargo's default.
///
/// An explicitly set `CARGO_HOME` wins, because that is what Cargo itself does
/// and the search must name the same directory the build reads. A relative
/// value is resolved against `working_directory`, matching Cargo's own rule
/// that it is interpreted relative to the command's working directory; the
/// nested builds run beneath the workspace, not in this process's directory, so
/// passing the value through unchanged would name two different trees.
/// Otherwise the default is the `home` directory's `.cargo`, from `HOME` on
/// Unix and `USERPROFILE` on Windows — the same pair Cargo's own default
/// resolves through. A host with neither variable set has no default to name,
/// and is left out rather than guessed at.
fn cargo_home(working_directory: &Path) -> Option<PathBuf> {
    resolve_cargo_home(working_directory, std::env::var_os("CARGO_HOME").as_deref())
}

/// The Cargo home for an explicitly resolved `CARGO_HOME`.
///
/// Split from [`cargo_home`] so the resolution rule — the one place a relative
/// value is re-rooted — can be exercised for a chosen value, without a case
/// writing to an environment variable every other thread in this binary reads.
///
/// An empty value is treated as unset, which is what Cargo does: it then falls
/// back to `$HOME/.cargo` (or `$USERPROFILE/.cargo` on Windows).
pub(crate) fn resolve_cargo_home(
    working_directory: &Path,
    explicit: Option<&OsStr>,
) -> Option<PathBuf> {
    if let Some(explicit) = explicit {
        let explicit = PathBuf::from(explicit);
        if !explicit.as_os_str().is_empty() {
            // Cargo interprets a relative `CARGO_HOME` against the working
            // directory of the command that reads it. The nested build's
            // working directory is the workspace (or a fixture beneath it),
            // not this process's, so the value has to be joined here or the
            // probe would read one tree while the build read another. An
            // absolute value is used unchanged, as Cargo does.
            return Some(if explicit.is_absolute() {
                explicit
            } else {
                working_directory.join(explicit)
            });
        }
    }
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|home| !home.is_empty()))?;
    Some(PathBuf::from(home).join(".cargo"))
}

#[cfg(test)]
mod tests {
    //! Resolution of the walk's root, for roots that exist and roots that do
    //! not.
    //!
    //! The production walk is a read of the tree, so its root is the one input
    //! a caller decides rather than one the checkout fixes. These cases pin
    //! that an absolute root is walked as given and that a relative one is
    //! resolved to an absolute walk, which is what the fingerprint depends on.

    use std::path::PathBuf;

    use super::{cargo_config_inputs_with, search_root};

    /// A checkout holding a `.cargo` directory, and a directory in it.
    fn checkout_with_nested_directory() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let checkout = tempfile::tempdir().expect("checkout");
        let nested = checkout.path().join("scratch/fixture");
        std::fs::create_dir_all(&nested).expect("nested directory");
        let cargo_dir = checkout.path().join(".cargo");
        std::fs::create_dir(&cargo_dir).expect("cargo dir");
        let config = cargo_dir.join("config.toml");
        std::fs::write(&config, "[build]\nrustflags = [\"-Copt-level=1\"]\n").expect("config");
        (checkout, nested, config, cargo_dir)
    }

    #[test]
    fn an_absolute_root_keeps_its_own_ancestry() {
        // An absolute root already names one tree, so rooting it must not move
        // it. Without this, a fix that always joined the current directory
        // would still pass the case below by re-rooting everything.
        let (_checkout, nested, _config, _cargo_dir) = checkout_with_nested_directory();
        assert_eq!(
            search_root(&nested),
            nested,
            "an absolute root must be walked as given"
        );
    }

    #[test]
    fn a_relative_root_resolves_to_an_absolute_walk() {
        // `Path::ancestors` is lexical. Walking `scratch/fixture` yields
        // `scratch/fixture`, `scratch` and the empty path, none of which is a
        // directory in the tree — so no configuration above the scratch root
        // could reach the fingerprint, and a stamp written from that result
        // survived an edit the nested build would notice.
        //
        // The relative value here names a real directory beneath this
        // process's own tree, so the case needs no current-directory change of
        // the kind the parallel suite must not make. What it pins is the rule:
        // whatever the process directory is, the walk starts from an absolute
        // path, and only an absolute ancestry can climb into the checkout.
        let relative = PathBuf::from("crates/rstest-bdd-harness/src");
        assert!(relative.is_relative(), "the case needs a relative root");

        let rooted = search_root(&relative);
        assert!(
            rooted.is_absolute(),
            "a walked root must be absolute, or its ancestry is not the tree's: {rooted:?}"
        );
        assert_eq!(
            rooted,
            std::env::current_dir()
                .expect("a process directory")
                .join(&relative),
            "the resolved root is the absolute spelling of the same directory"
        );
    }

    #[test]
    fn configuration_above_the_root_reaches_the_inputs() {
        // The defect stated as an outcome rather than as a shape: the nested
        // build reads `<checkout>/.cargo/config.toml`, so a probe given the
        // nested directory must report that file. Before the walk was rooted it
        // reported nothing above the nested path.
        let (_checkout, nested, config, cargo_dir) = checkout_with_nested_directory();
        let roots = cargo_config_inputs_with(&nested, None).expect("config inputs");
        assert!(
            roots.contains(&config),
            "configuration above the root must reach the fingerprint: {roots:?}"
        );
        assert!(
            roots.iter().any(|path| path.starts_with(&cargo_dir)),
            "the checkout's own `.cargo` is visited on the way up: {roots:?}"
        );
    }

    #[test]
    fn a_named_cargo_home_is_searched_last() {
        // Cargo reads `$CARGO_HOME/config.toml` in addition to the walk, and
        // the walk's own entries take precedence, so the home is consulted
        // after every ancestor rather than instead of them.
        let (_checkout, nested, config, _cargo_dir) = checkout_with_nested_directory();
        let home = tempfile::tempdir().expect("cargo home");
        let home_config = home.path().join("config.toml");
        std::fs::write(&home_config, "[build]\nrustflags = [\"-Copt-level=1\"]\n")
            .expect("home config");

        let roots = cargo_config_inputs_with(&nested, Some(home.path())).expect("config inputs");
        assert!(
            roots.contains(&config) && roots.contains(&home_config),
            "both the walk and the home contribute: {roots:?}"
        );
        assert_eq!(
            roots.last(),
            Some(&home_config),
            "the home is consulted after the walk, as Cargo does: {roots:?}"
        );
    }
}
