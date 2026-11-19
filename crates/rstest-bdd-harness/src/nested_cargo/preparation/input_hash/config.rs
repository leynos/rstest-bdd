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
    let mut present = Vec::new();
    for directory in config_search_dirs(workspace_root) {
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
/// `$CARGO_HOME` is appended when it is set to a non-empty path, because Cargo
/// reads it in addition to the walk rather than as part of it.
///
/// When `CARGO_HOME` is unset or empty, Cargo falls back to `$HOME/.cargo`;
/// [`cargo_home`] names that default here. Leaving it out would make the
/// fingerprint blind to a `~/.cargo/config.toml` that sets rustflags, a target
/// directory or a linker — a change the nested build would notice and the stamp
/// would not, which is the unsoundness this module exists to remove.
fn config_search_dirs(workspace_root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = workspace_root
        .ancestors()
        .map(|ancestor| ancestor.join(".cargo"))
        .collect();
    if let Some(cargo_home) = cargo_home(workspace_root) {
        dirs.push(cargo_home);
    }
    dirs
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
    if let Some(explicit) = std::env::var_os("CARGO_HOME") {
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
