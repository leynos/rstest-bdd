//! Redirecting the file tier's discovery roots away from the host.
//!
//! The CLI fixture's child starts from the ambient environment, and the
//! binary's configuration loader merges *every* candidate file it finds. A
//! developer's `~/.cargo_bdd.toml` therefore reaches these smoke tests as
//! surely as an exported `CARGO_BDD_*` variable does, and can change what they
//! observe. Measured with a `HOME` dotfile setting `cmds.skipped.json = true`,
//! a plain listing test fails.
//!
//! This module owns the redirection so the shared child-environment helper
//! stays small, and so the roots it overrides can be unit-tested directly.
//! Its behavioural consequences are asserted in the sibling `environment`
//! module, which drives the helper every smoke test uses.

use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// Point every discovery root at an empty directory.
///
/// Each root is its own empty sibling directory under one process-wide parent,
/// so no root holds anything discovery looks for and none is nested inside
/// another. The parent is keyed by process id to keep concurrent test binaries
/// apart. It is scratch space under the system temporary directory and holds
/// three empty directories per test binary, well under a kilobyte; cleaning it
/// up is left to the host's temporary-directory housekeeping.
///
/// `HOME` and `USERPROFILE` cover the home dotfile, which both platforms read
/// and where Windows falls back from `HOME` to `USERPROFILE`.
/// `XDG_CONFIG_HOME` and `XDG_CONFIG_DIRS` cover the Unix bases; the latter
/// must be set rather than left absent, because an absent or wholly relative
/// value means `/etc/xdg`, which is a host root again.
///
/// The Windows native configuration directory comes from the Windows API
/// rather than a variable, so it is the one root this cannot redirect; see
/// `V5` in the execution plan for that residual.
pub(super) fn empty_configuration_home_variables() -> Vec<(OsString, OsString)> {
    static EMPTY_HOME: OnceLock<PathBuf> = OnceLock::new();
    let home = EMPTY_HOME.get_or_init(|| {
        let home = env::temp_dir().join(format!("cargo-bdd-cli-empty-home-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&home);
        home
    });
    let [home_root, xdg_config_home, xdg_config_dirs] =
        ["home", "xdg-config-home", "xdg-config-dirs"].map(|name| {
            let directory = home.join(name);
            let _ = std::fs::create_dir_all(&directory);
            directory.into_os_string()
        });
    let mut variables = vec![
        (OsString::from("HOME"), home_root.clone()),
        (OsString::from("USERPROFILE"), home_root),
        (OsString::from("XDG_CONFIG_HOME"), xdg_config_home),
        (OsString::from("XDG_CONFIG_DIRS"), xdg_config_dirs),
    ];
    variables.extend(ambient_cargo_home_variables());
    variables
}

/// Carry Cargo's ambient home through the redirection.
///
/// Cargo derives its registry from `CARGO_HOME` and its toolchain from
/// `RUSTUP_HOME`, each defaulting under the user home this policy overrides.
/// Redirecting the home without carrying these would point the nested build at
/// an empty registry. A variable the developer already set wins over the
/// user-home default, matching Cargo's own precedence.
fn ambient_cargo_home_variables() -> Vec<(OsString, OsString)> {
    let home = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"));
    [("CARGO_HOME", ".cargo"), ("RUSTUP_HOME", ".rustup")]
        .into_iter()
        .filter_map(|(key, directory)| {
            let ambient = env::var_os(key);
            let derived = home
                .as_ref()
                .map(|home| Path::new(home).join(directory))
                .map(PathBuf::into_os_string);
            let value = ambient.or(derived)?;
            Some((OsString::from(key), value))
        })
        .collect()
}
