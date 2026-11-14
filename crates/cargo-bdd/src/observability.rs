//! Opt-in tracing output for the `cargo-bdd` binary.
//!
//! The binary installs a subscriber only when `CARGO_BDD_LOG` is set, so the
//! default command output contract is unchanged: without the variable, spans
//! such as `cargo_bdd.configuration` are created but never rendered. Only that
//! variable selects the filter, so an ambient `RUST_LOG` in a host toolchain
//! or wrapper script cannot change this binary's output.
//!
//! The subscriber reports span *close* events rather than enter/exit pairs.
//! Configuration loading records no events of its own, so the close line shown
//! in the users' guide is what makes the documented timing observable at all.
//! One line per load also keeps the opt-in output readable when the filter
//! admits several commands. Colour is used only when stderr is a terminal, so a
//! redirected log stays plain text.

use std::{
    ffi::OsString,
    io::{self, IsTerminal},
};

use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

/// Environment variable that opts the binary in to tracing output.
pub(crate) const VARIABLE: &str = "CARGO_BDD_LOG";

/// Install the stderr subscriber selected by `CARGO_BDD_LOG`.
///
/// Span close events are enabled because the instrumented loaders record no
/// events of their own; without them the opt-in variable would produce no
/// output at all. The close event inherits the span's `INFO` level.
///
/// Returns without doing anything when the variable is unset, or when another
/// subscriber already owns the global default.
pub(crate) fn install() {
    let Some(filter) = filter_from(std::env::var_os(VARIABLE)) else {
        return;
    };
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        // The formatter colours unconditionally by default, which would write
        // escape sequences into redirected stderr and CI logs.
        .with_ansi(io::stderr().is_terminal())
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
