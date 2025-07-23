//! Core library for `rstest-bdd`.
//! This crate exposes helper utilities used by behaviour tests. It also defines
//! the global step registry used to orchestrate behaviour-driven tests.

/// Returns a greeting for the library.
///
/// # Examples
///
/// ```
/// use rstest_bdd::greet;
///
/// assert_eq!(greet(), "Hello from rstest-bdd!");
/// ```
#[must_use]
pub fn greet() -> &'static str {
    "Hello from rstest-bdd!"
}

pub use inventory::{iter, submit};

/// Represents a single step definition registered with the framework.
///
/// Each step records its keyword, the pattern text used for matching, a
/// type-erased function pointer, and the source location where it was defined.
///
/// # Examples
///
/// ```
/// use rstest_bdd::{step, Step};
///
/// fn my_step() {}
///
/// step!("Given", "a step", my_step);
/// ```
#[derive(Copy, Clone, Debug)]
pub struct Step {
    /// The step keyword, e.g. `Given` or `When`.
    pub keyword: &'static str,
    /// Pattern text used to match a Gherkin step.
    pub pattern: &'static str,
    /// Function pointer executed when the step is invoked.
    pub run: fn(),
    /// Source file where the step is defined.
    pub file: &'static str,
    /// Line number within the source file.
    pub line: u32,
}

impl Step {
    /// Create a new `Step` instance.
    #[must_use]
    pub const fn new(
        keyword: &'static str,
        pattern: &'static str,
        run: fn(),
        file: &'static str,
        line: u32,
    ) -> Self {
        Self {
            keyword,
            pattern,
            run,
            file,
            line,
        }
    }
}

/// Register a step definition with the global registry.
///
/// This macro hides the underlying `inventory` call and captures
/// the source location automatically.
///
/// # Examples
///
/// ```
/// use rstest_bdd::{step, Step};
///
/// fn my_step() {}
///
/// step!("Given", "a pattern", my_step);
/// ```
#[macro_export]
macro_rules! step {
    ($keyword:expr, $pattern:expr, $handler:path) => {
        $crate::submit! {
            $crate::Step::new($keyword, $pattern, $handler, file!(), line!())
        }
    };
}

inventory::collect!(Step);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greet_returns_expected_text() {
        assert_eq!(greet(), "Hello from rstest-bdd!");
    }

    #[test]
    fn collects_registered_step() {
        fn sample() {}

        step!("Given", "a pattern", sample);

        let found = iter::<Step>
            .into_iter()
            .any(|step| step.pattern == "a pattern");

        assert!(found, "registered step was not found in the inventory");
    }
}
