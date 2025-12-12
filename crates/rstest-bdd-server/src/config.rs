//! Server configuration parsed from environment variables.
//!
//! This module provides configuration types and parsing for the language
//! server. All settings can be overridden via environment variables prefixed
//! with `RSTEST_BDD_LSP_`.

use std::env;
use std::str::FromStr;

use crate::error::ServerError;

/// Log level enumeration matching tracing crate levels.
///
/// Defaults to `Info` when not specified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevel {
    /// Most verbose logging, includes all trace spans.
    Trace,
    /// Debug-level information for development.
    Debug,
    /// Standard informational messages.
    #[default]
    Info,
    /// Warning messages for potentially problematic situations.
    Warn,
    /// Error messages for failures.
    Error,
}

impl FromStr for LogLevel {
    type Err = ServerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" | "warning" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err(ServerError::InvalidConfig(format!(
                "unknown log level '{s}', expected one of: trace, debug, info, warn, error"
            ))),
        }
    }
}

impl LogLevel {
    /// Convert to a tracing filter directive string.
    #[must_use]
    pub fn as_filter_str(&self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// Default debounce interval in milliseconds.
const DEFAULT_DEBOUNCE_MS: u64 = 300;

/// Configuration for the language server.
///
/// All settings can be overridden via environment variables prefixed with
/// `RSTEST_BDD_LSP_`.
///
/// # Environment Variables
///
/// - `RSTEST_BDD_LSP_LOG_LEVEL`: Sets the log level (trace, debug, info, warn,
///   error)
/// - `RSTEST_BDD_LSP_DEBOUNCE_MS`: Delay before processing file changes
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Log level (trace, debug, info, warn, error).
    pub log_level: LogLevel,
    /// Debounce interval for file change events in milliseconds.
    pub debounce_ms: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            log_level: LogLevel::default(),
            debounce_ms: DEFAULT_DEBOUNCE_MS,
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables.
    ///
    /// Reads `RSTEST_BDD_LSP_LOG_LEVEL` and `RSTEST_BDD_LSP_DEBOUNCE_MS`.
    /// Falls back to defaults for missing values.
    ///
    /// # Errors
    ///
    /// Returns `ServerError::InvalidConfig` if an environment variable contains
    /// an invalid value.
    pub fn from_env() -> Result<Self, ServerError> {
        let log_level = match env::var("RSTEST_BDD_LSP_LOG_LEVEL") {
            Ok(val) => val.parse()?,
            Err(_) => LogLevel::default(),
        };

        let debounce_ms = match env::var("RSTEST_BDD_LSP_DEBOUNCE_MS") {
            Ok(val) => val.parse().map_err(|_| {
                ServerError::InvalidConfig(format!(
                    "invalid debounce value '{val}', expected a positive integer"
                ))
            })?,
            Err(_) => DEFAULT_DEBOUNCE_MS,
        };

        Ok(Self {
            log_level,
            debounce_ms,
        })
    }

    /// Apply optional overrides to an existing configuration.
    ///
    /// This is intended for CLI overrides that should take precedence over
    /// environment-based defaults.
    #[must_use]
    pub fn apply_overrides(
        mut self,
        log_level: Option<LogLevel>,
        debounce_ms: Option<u64>,
    ) -> Self {
        if let Some(level) = log_level {
            self.log_level = level;
        }

        if let Some(ms) = debounce_ms {
            self.debounce_ms = ms;
        }

        self
    }

    /// Create a new configuration with the specified log level.
    #[must_use]
    pub fn with_log_level(mut self, level: LogLevel) -> Self {
        self.log_level = level;
        self
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "tests require explicit panic messages for debugging failures"
)]
mod tests {
    use super::*;

    #[test]
    fn log_level_parses_valid_values() {
        assert_eq!("trace".parse::<LogLevel>().ok(), Some(LogLevel::Trace));
        assert_eq!("debug".parse::<LogLevel>().ok(), Some(LogLevel::Debug));
        assert_eq!("info".parse::<LogLevel>().ok(), Some(LogLevel::Info));
        assert_eq!("warn".parse::<LogLevel>().ok(), Some(LogLevel::Warn));
        assert_eq!("warning".parse::<LogLevel>().ok(), Some(LogLevel::Warn));
        assert_eq!("error".parse::<LogLevel>().ok(), Some(LogLevel::Error));
    }

    #[test]
    fn log_level_is_case_insensitive() {
        assert_eq!("TRACE".parse::<LogLevel>().ok(), Some(LogLevel::Trace));
        assert_eq!("Debug".parse::<LogLevel>().ok(), Some(LogLevel::Debug));
        assert_eq!("INFO".parse::<LogLevel>().ok(), Some(LogLevel::Info));
    }

    #[test]
    fn log_level_rejects_invalid_values() {
        let result = "invalid".parse::<LogLevel>();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("unknown log level"));
    }

    #[test]
    fn log_level_as_filter_str_returns_correct_strings() {
        assert_eq!(LogLevel::Trace.as_filter_str(), "trace");
        assert_eq!(LogLevel::Debug.as_filter_str(), "debug");
        assert_eq!(LogLevel::Info.as_filter_str(), "info");
        assert_eq!(LogLevel::Warn.as_filter_str(), "warn");
        assert_eq!(LogLevel::Error.as_filter_str(), "error");
    }

    #[test]
    fn server_config_default_values() {
        let config = ServerConfig::default();
        assert_eq!(config.log_level, LogLevel::Info);
        assert_eq!(config.debounce_ms, 300);
    }

    #[test]
    fn server_config_with_log_level_builder() {
        let config = ServerConfig::default().with_log_level(LogLevel::Debug);
        assert_eq!(config.log_level, LogLevel::Debug);
    }

    #[test]
    fn server_config_apply_overrides_updates_selected_fields() {
        let config = ServerConfig::default().apply_overrides(Some(LogLevel::Error), Some(42));
        assert_eq!(config.log_level, LogLevel::Error);
        assert_eq!(config.debounce_ms, 42);

        let config = ServerConfig::default().apply_overrides(None, None);
        assert_eq!(config.log_level, LogLevel::Info);
        assert_eq!(config.debounce_ms, 300);
    }
}
