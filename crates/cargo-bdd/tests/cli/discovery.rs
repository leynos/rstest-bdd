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
    io,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Point every discovery root at an empty directory.
///
/// Each root is its own empty sibling directory under one process-wide parent,
/// so no root holds anything discovery looks for and none is nested inside
/// another. The parent is keyed by process id to keep concurrent test binaries
/// apart. It is scratch space under the system temporary directory and holds
/// three empty directories per test binary, well under a kilobyte; cleaning it
/// up is left to the host's temporary-directory housekeeping.
///
/// `HOME` covers the home dotfile on every platform. On Windows it outranks
/// `USERPROFILE` in the discovery order, so redirecting it is enough and
/// `USERPROFILE` is deliberately left inherited: it is how the Windows
/// Known Folder API resolves a profile, and `sccache` — which CI installs as
/// `RUSTC_WRAPPER` — calls `directories::ProjectDirs` while resolving its own
/// configuration. With the profile unresolvable that call returns `None`,
/// `sccache` panics, and every nested cargo invocation in this suite dies with
/// `Unable to get config directory`. The earlier redirect covered
/// `USERPROFILE` as well and passed locally on Linux while turning both
/// Windows CI jobs red.
///
/// `XDG_CONFIG_HOME` and `XDG_CONFIG_DIRS` cover the Unix bases; the latter
/// must be set rather than left absent, because an absent or wholly relative
/// value means `/etc/xdg`, which is a host root again. Windows ignores both,
/// so they are set there only to keep one environment map for both platforms.
///
/// `CARGO_HOME` and `RUSTUP_HOME` hold no discovery candidate, so they are not
/// redirected but carried: see `ambient_cargo_home_variables`.
///
/// The Windows native configuration directory comes from the Windows API
/// rather than a variable, so it is the one root this cannot redirect; see
/// `V5` in the execution plan for that residual.
///
/// `requested` is the caller's own additions. A key it already sets is not
/// emitted here, because the caller supplies variables the helper has no way
/// to know about — a test may deliberately point `CARGO_HOME` at a custom
/// cache — and overwriting one would silently discard the intent it encodes.
///
/// Directory creation is fallible and its error is returned rather than
/// discarded: a redirect that names a directory which was never created would
/// otherwise surface as a missing-configuration failure in whichever smoke
/// test ran next, naming neither this module nor the cause.
pub(super) fn empty_configuration_home_variables(
    requested: &[(OsString, OsString)],
) -> io::Result<Vec<(OsString, OsString)>> {
    let home = empty_home_root()?;
    let home_root = create_sibling(&home, "home")?;
    let xdg_config_home = create_sibling(&home, "xdg-config-home")?;
    let xdg_config_dirs = create_sibling(&home, "xdg-config-dirs")?;
    let mut variables = vec![
        (OsString::from("HOME"), home_root),
        (OsString::from("XDG_CONFIG_HOME"), xdg_config_home),
        (OsString::from("XDG_CONFIG_DIRS"), xdg_config_dirs),
    ];
    variables.extend(ambient_cargo_home_variables(requested));
    Ok(variables)
}

/// The process-wide parent directory, created on first use.
///
/// The cached value is the path, not the outcome of creating it: a
/// `OnceLock` initializer cannot return an error, so caching a `Result` would
/// remember one transient failure for the whole test binary. Creating an
/// existing directory succeeds, so re-running the creation on each call is
/// both correct and cheap. The same argument is why the cached path stays a
/// `PathBuf` and is converted on every call: a `Utf8PathBuf` conversion can
/// fail on a non-UTF-8 temporary directory, and that failure must be reported
/// rather than cached for the process.
fn empty_home_root() -> io::Result<Utf8PathBuf> {
    static EMPTY_HOME: OnceLock<PathBuf> = OnceLock::new();
    let home = EMPTY_HOME
        .get_or_init(|| {
            env::temp_dir().join(format!("cargo-bdd-cli-empty-home-{}", std::process::id()))
        })
        .clone();
    let home = utf8_path(home)?;
    Dir::create_ambient_dir_all(&home, ambient_authority())?;
    Ok(home)
}

/// Convert a path for the UTF-8 directory API, reporting a non-UTF-8 path.
///
/// `cap_std::fs_utf8` is the fallible directory API this module uses; it takes
/// camino paths, so a path that is not valid UTF-8 has to be refused here
/// rather than silently replaced by a lossy encoding that names a different
/// directory.
fn utf8_path(path: PathBuf) -> io::Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path).map_err(|path| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "the empty configuration root is not valid UTF-8: {}",
                path.display()
            ),
        )
    })
}

/// Create one empty sibling of the shared parent and return its path.
///
/// `create_dir_all` on an existing directory succeeds, so a second call for
/// the same name is harmless.
fn create_sibling(parent: &Utf8Path, name: &str) -> io::Result<OsString> {
    let directory = parent.join(name);
    Dir::create_ambient_dir_all(&directory, ambient_authority())?;
    Ok(OsString::from(directory.into_string()))
}

/// The home Cargo and Rustup derive their defaults from.
///
/// Cargo resolves both homes through the `home` crate, and this mirrors it
/// rather than approximating it. On Unix that crate reads `HOME` and nothing
/// else. On Windows it reads `USERPROFILE` when that is set and non-empty, and
/// otherwise queries the profile Known Folder API; it never reads `HOME`.
///
/// `HOME` is therefore *not* a fallback on Windows. This file's own discovery
/// order does consult `HOME` first, which is why only `HOME` is redirected, but
/// that is the configuration file's rule, not Cargo's. Substituting `HOME` here
/// would hand the child a registry and toolchain under a root Cargo never
/// looks at, which a Windows shell with a Unix-style `HOME` arranged for Git or
/// MSYS makes reachable.
///
/// Returning `None` on Windows without `USERPROFILE` is deliberate: the caller
/// must not invent a home the way the platform would, because only the Windows
/// API can say where the profile is.
///
/// The empty value is skipped on both platforms because `home` skips it: an
/// empty `USERPROFILE` means "not set" to the Known Folder fallback there, and
/// an empty `HOME` would otherwise derive `.cargo` relative to the child's
/// working directory.
pub(super) fn toolchain_home() -> Option<OsString> {
    let names: &[&str] = if cfg!(windows) {
        &["USERPROFILE"]
    } else {
        &["HOME"]
    };
    names
        .iter()
        .find_map(|name| env::var_os(name).filter(|value| !value.is_empty()))
}

/// Carry Cargo's ambient home through the redirection.
///
/// Cargo derives its registry from `CARGO_HOME` and its toolchain from
/// `RUSTUP_HOME`, each defaulting under the user home this policy overrides.
/// Redirecting the home without carrying these would point the nested build at
/// an empty registry.
///
/// Three sources are considered in order of authority: what the caller
/// requested, then the ambient variable, then the [`toolchain_home`] default.
/// The caller's value is the most specific intent available and is passed
/// through untouched, which is what lets a test point the registry somewhere of
/// its own choosing. The ambient variable outranks the default for the same
/// reason, matching Cargo's own precedence.
///
/// `USERPROFILE` is never redirected into the child: see the module
/// documentation for why. It is consulted here only as the home Cargo itself
/// would derive a default from, which is a different question.
fn ambient_cargo_home_variables(requested: &[(OsString, OsString)]) -> Vec<(OsString, OsString)> {
    let home = toolchain_home();
    [("CARGO_HOME", ".cargo"), ("RUSTUP_HOME", ".rustup")]
        .into_iter()
        .filter_map(|(key, directory)| {
            if let Some((_, value)) = requested.iter().find(|(name, _)| name == key) {
                return Some((OsString::from(key), value.clone()));
            }
            let derived = home
                .as_ref()
                .map(|home| Path::new(home).join(directory))
                .map(PathBuf::into_os_string);
            let value = env::var_os(key).or(derived)?;
            Some((OsString::from(key), value))
        })
        .collect()
}
