//! Opt-in tracing output for the `cargo-bdd` binary.
//!
//! The binary installs a subscriber only when `CARGO_BDD_LOG` is set, so the
//! default command output contract is unchanged: without the variable, spans
//! such as `cargo_bdd.configuration` are created but never rendered. Only that
//! variable selects the filter, so an ambient `RUST_LOG` in a host toolchain
//! or wrapper script cannot change this binary's output.

use std::{ffi::OsString, io};

use tracing_subscriber::EnvFilter;

/// Environment variable that opts the binary in to tracing output.
pub(crate) const VARIABLE: &str = "CARGO_BDD_LOG";

/// Install the stderr subscriber selected by `CARGO_BDD_LOG`.
///
/// Returns without doing anything when the variable is unset, or when another
/// subscriber already owns the global default.
pub(crate) fn install() {
    let Some(filter) = filter_from(std::env::var_os(VARIABLE)) else {
        return;
    };
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr);
    if builder.try_init().is_err() {
        // A subscriber installed by an embedding process keeps ownership.
    }
}

/// Build the filter for an environment value, or `None` for no opt-in.
fn filter_from(value: Option<OsString>) -> Option<EnvFilter> {
    value.map(|value| EnvFilter::new(value.to_string_lossy()))
}

#[cfg(test)]
mod tests;
