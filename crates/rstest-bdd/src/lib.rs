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
use std::collections::HashMap;
use std::sync::LazyLock;

type StepKey = (&'static str, &'static str);

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
#[derive(Debug)]
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
            $crate::Step {
                keyword: $keyword,
                pattern: $pattern,
                run: $handler,
                file: file!(),
                line: line!(),
            }
        }
    };
}

inventory::collect!(Step);

static STEP_MAP: LazyLock<HashMap<StepKey, fn()>> = LazyLock::new(|| {
    // Collect registered steps first so we can allocate the map with
    // an appropriate capacity. This avoids rehashing when many steps
    // are present.
    let steps: Vec<_> = iter::<Step>.into_iter().collect();
    let mut map = HashMap::with_capacity(steps.len());
    for step in steps {
        map.insert((step.keyword, step.pattern), step.run);
    }
    map
});

/// Look up a registered step by keyword and pattern.
///
/// # Examples
///
/// ```
/// use rstest_bdd::{step, lookup_step};
///
/// fn dummy() {}
/// step!("Given", "a thing", dummy);
///
/// let step_fn = lookup_step("Given", "a thing");
/// assert!(step_fn.is_some());
/// ```
#[must_use]
pub fn lookup_step(keyword: &str, pattern: &str) -> Option<fn()> {
    STEP_MAP.get(&(keyword, pattern)).copied()
}

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
