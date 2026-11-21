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

mod home;

// Re-exported because the export is the one place a relative value is
// re-rooted, and the preparation's own cases exercise that rule directly.
pub(crate) use home::resolve_cargo_home;
use home::{cargo_home, platform_home};

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

/// The configuration inputs when the walk and the command directory differ.
///
/// [`cargo_config_inputs`] uses one directory for both jobs, which is right
/// whenever the nested build runs inside the root being walked. It is wrong for
/// a family whose build runs *below* the stamped root: `trybuild` invokes Cargo
/// inside `<target>/tests/trybuild/<pkg>`, so that generated crate is the
/// directory a relative `CARGO_HOME` resolves against, while the walk must
/// still start at the workspace to find the configuration Cargo discovers by
/// climbing. Naming only one of the two would either miss that configuration or
/// resolve the home against a directory the build never uses.
///
/// Both ancestries are walked, not just the stamped root's. Cargo climbs from
/// the directory the command runs in, so everything between the generated crate
/// and the workspace — `<target>/tests/trybuild/.cargo`,
/// `<target>/tests/.cargo`, `<target>/.cargo` — is configuration it reads. None
/// of it is reachable from a walk that starts at the workspace, and none of it
/// is covered indirectly either: [`input_hash`] skips a `target` directory
/// during a walk, so an edit to a configuration file under one would leave the
/// stamp valid while the nested build picked up new rustflags.
///
/// The two walks overlap above the workspace, which is why the result is
/// deduplicated. Order is Cargo's — nearest first — and a path reached by both
/// walks keeps the position the generated crate's walk gave it, because that is
/// the ancestry the command itself climbs.
///
/// # Errors
///
/// As [`cargo_config_inputs`].
///
/// [`input_hash`]: super::input_hash
pub fn cargo_config_inputs_from(
    walk_root: &Path,
    working_directory: &Path,
) -> io::Result<Vec<PathBuf>> {
    cargo_config_inputs_from_with(
        walk_root,
        working_directory,
        std::env::var_os("CARGO_HOME").as_deref(),
    )
}

/// [`cargo_config_inputs_from`] with the environment supplied.
///
/// The value is what `CARGO_HOME` holds, not a resolved directory: a relative
/// one still has to be re-rooted against `working_directory`, which is the
/// rule this form exists to keep exercisable. A test that set the variable
/// around a call would change what every other thread in the same binary
/// reads, and the cases in this module run in parallel, so the value arrives
/// as a parameter instead.
pub(crate) fn cargo_config_inputs_from_with(
    walk_root: &Path,
    working_directory: &Path,
    cargo_home: Option<&OsStr>,
) -> io::Result<Vec<PathBuf>> {
    let cargo_home = resolve_cargo_home(working_directory, cargo_home, platform_home);
    let mut dirs = walk_dirs(working_directory);
    for directory in walk_dirs(walk_root) {
        if !dirs.contains(&directory) {
            dirs.push(directory);
        }
    }
    // The home is consulted after every walked directory rather than among
    // them, which is Cargo's order: it reads `$CARGO_HOME/config.toml` in
    // addition to the walk, and the walk wins a key both set.
    if let Some(cargo_home) = cargo_home {
        dirs.push(cargo_home);
    }
    present_configs(dirs)
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
    present_configs(config_search_dirs(workspace_root, cargo_home))
}

/// The configuration files present in `dirs`, in the order given.
///
/// Kept separate because two entry points search different directory lists —
/// one walk or two — and both then ask the same question of each directory.
/// Stating the presence rule twice would let one copy drift into treating an
/// unreadable file as absent, which is the distinction the rest of this module
/// depends on.
///
/// # Errors
///
/// As [`config_entry_is_present`].
fn present_configs(dirs: Vec<PathBuf>) -> io::Result<Vec<PathBuf>> {
    let mut present = Vec::new();
    for directory in dirs {
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
///
/// The walk's own directories are reported through the resolved root, so a
/// root named with `..` components contributes the same ancestry whether or
/// not the tree exists yet. See [`search_root`]. The Cargo home is reported as
/// the caller supplied it; see [`resolve_cargo_home`] for its spelling rule.
fn config_search_dirs(workspace_root: &Path, cargo_home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = walk_dirs(workspace_root);
    if let Some(cargo_home) = cargo_home {
        dirs.push(cargo_home.to_path_buf());
    }
    dirs
}

/// The `.cargo` directories the upward walk from `root` visits, nearest first.
///
/// Split from [`config_search_dirs`] because the two-walk entry point merges
/// two ancestries before appending the Cargo home once. Appending the home
/// inside each walk would place it between them, which is neither walk's order
/// and not Cargo's either.
fn walk_dirs(root: &Path) -> Vec<PathBuf> {
    search_root(root)
        .ancestors()
        .map(|ancestor| ancestor.join(".cargo"))
        .collect()
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
/// Resolution also has to be *stable across a tree that does not exist yet*,
/// which is why the longest existing prefix is resolved rather than the whole
/// path. The preparation walks this root before its own callback creates the
/// tree, and the scratch root is named through `crates/rstest-bdd/../..`. When
/// the whole path was resolved only if it existed, a cold run kept that
/// spelling and contributed four extra lexical ancestors — `crates`, then
/// `crates/rstest-bdd` and its `..` forms — while a warm run resolved it and
/// contributed none. The two walks therefore reported different `.cargo`
/// directories for the same tree, so a stamp written by the cold preparation
/// was refused by every later consumer with an empty evicted list. Resolving
/// the existing prefix collapses the `..` components in both cases and the
/// walk agrees.
fn search_root(workspace_root: &Path) -> PathBuf {
    let absolute = absolute_root(workspace_root);
    let Some((resolved, remainder)) = split_at_existing_prefix(&absolute) else {
        // No prefix of the path resolves; the absolute spelling is the best
        // available answer.
        return absolute;
    };
    reappend(resolved, &remainder)
}

/// The longest existing prefix of `absolute`, and the components after it.
///
/// Each step strips one trailing component and tries again; the components
/// stripped so far are returned in the order they were removed, which is why
/// [`reappend`] walks them back to front. `None` means nothing resolved at
/// all, not even the filesystem root.
fn split_at_existing_prefix(absolute: &Path) -> Option<(PathBuf, Vec<&OsStr>)> {
    let mut remainder: Vec<&OsStr> = Vec::new();
    let mut prefix = absolute;
    loop {
        if let Ok(resolved) = fs::canonicalize(prefix) {
            return Some((resolved, remainder));
        }
        let parent = prefix.parent()?;
        remainder.push(prefix.components().next_back()?.as_os_str());
        prefix = parent;
    }
}

/// Re-append the components stripped by [`split_at_existing_prefix`].
///
/// They were removed from the tail, so they are pushed back in reverse.
fn reappend(mut resolved: PathBuf, remainder: &[&OsStr]) -> PathBuf {
    for component in remainder.iter().rev() {
        resolved.push(component);
    }
    resolved
}

/// The walk's root when `workspace_root` could not be made absolute.
///
/// The directory may not exist yet — the preparation creates the tree it stamps
/// — and canonicalizing a relative path also needs it to exist beneath this
/// process's directory. Joining the process directory keeps the walk absolute,
/// which is the property that matters here; the fingerprint still covers the
/// tree through its other inputs once it exists.
fn absolute_root(workspace_root: &Path) -> PathBuf {
    if workspace_root.is_absolute() {
        return workspace_root.to_path_buf();
    }
    std::env::current_dir().map_or_else(
        |_| workspace_root.to_path_buf(),
        |directory| directory.join(workspace_root),
    )
}

#[cfg(test)]
mod tests;
