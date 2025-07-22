//! Core library for `rstest-bdd`.
//!
//! This crate exposes helper utilities used by behaviour tests.

/// Returns a greeting for the library.
#[must_use]
pub fn greet() -> &'static str {
    "Hello from rstest-bdd!"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greet_returns_expected_text() {
        assert_eq!(greet(), "Hello from rstest-bdd!");
    }
}
