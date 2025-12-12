//! Structured logging with environment variable configuration.
//!
//! This module initialises the logging subsystem for the language server.
//! Logs are written to stderr to avoid interfering with JSON-RPC communication
//! on stdout.

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::config::ServerConfig;

/// Initialise the logging subsystem based on configuration.
///
/// Sets up tracing with the specified log level. Logs are written to stderr
/// to avoid interfering with the JSON-RPC communication on stdout.
///
/// # Environment Variables
///
/// - `RSTEST_BDD_LSP_LOG_LEVEL`: Sets the log level (trace, debug, info, warn,
///   error)
/// - `RUST_LOG`: Falls back to standard Rust logging convention if
///   LSP-specific variable is not set
///
/// # Note
///
/// If a global subscriber is already set, this function silently ignores
/// the error. This is expected behaviour in tests or when multiple
/// components attempt to initialise logging.
pub fn init_logging(config: &ServerConfig) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.log_level.as_filter_str()));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .finish();

    // Ignore error if a subscriber is already set (e.g., in tests).
    // The first subscriber wins, which is the expected behaviour.
    let _ = tracing::subscriber::set_global_default(subscriber);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LogLevel;

    #[test]
    fn log_level_converts_to_filter_string() {
        let config = ServerConfig::default().with_log_level(LogLevel::Debug);
        assert_eq!(config.log_level.as_filter_str(), "debug");
    }
}
