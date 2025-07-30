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
use regex::Regex;
use std::any::Any;
use std::collections::HashMap;
use std::sync::LazyLock;

type StepKey = (&'static str, &'static str);

/// Context passed to step functions containing references to requested fixtures.
///
/// This is constructed by the `#[scenario]` macro for each step invocation.
///
/// # Examples
///
/// ```
/// use rstest_bdd::StepContext;
///
/// let mut ctx = StepContext::default();
/// let value = 42;
/// ctx.insert("my_fixture", &value);
///
/// let retrieved: Option<&i32> = ctx.get("my_fixture");
/// assert_eq!(retrieved, Some(&42));
/// ```
#[derive(Default)]
pub struct StepContext<'a> {
    fixtures: HashMap<&'static str, &'a dyn Any>,
}

impl<'a> StepContext<'a> {
    /// Insert a fixture reference by name.
    pub fn insert<T: Any>(&mut self, name: &'static str, value: &'a T) {
        self.fixtures.insert(name, value);
    }

    /// Retrieve a fixture reference by name and type.
    #[must_use]
    pub fn get<T: Any>(&self, name: &str) -> Option<&'a T> {
        self.fixtures.get(name)?.downcast_ref::<T>()
    }
}

/// Extract placeholder values from a step string using a pattern.
///
/// The pattern supports `format!`-style placeholders such as `{count:u32}`.
/// Any text outside placeholders must match exactly. The returned vector
/// contains the raw substring for each placeholder in order of appearance.
#[must_use]
pub fn extract_placeholders(pattern: &str, text: &str) -> Option<Vec<String>> {
    let mut regex_source = String::from("^");
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
            }
            regex_source.push_str("(.+)");
        } else {
            regex_source.push_str(&regex::escape(&ch.to_string()));
        }
    }
    regex_source.push('$');
    let re = Regex::new(&regex_source).ok()?;
    let caps = re.captures(text)?;
    let mut values = Vec::new();
    for i in 1..caps.len() {
        values.push(caps[i].to_string());
    }
    Some(values)
}

/// Type alias for the stored step function pointer.
pub type StepFn = for<'a> fn(&StepContext<'a>, &str);

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
    pub run: StepFn,
    /// Names of fixtures this step requires.
    pub fixtures: &'static [&'static str],
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
    ($keyword:expr, $pattern:expr, $handler:path, $fixtures:expr) => {
        $crate::submit! {
            $crate::Step {
                keyword: $keyword,
                pattern: $pattern,
                run: $handler,
                fixtures: $fixtures,
                file: file!(),
                line: line!(),
            }
        }
    };
}

inventory::collect!(Step);

static STEP_MAP: LazyLock<HashMap<StepKey, StepFn>> = LazyLock::new(|| {
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
pub fn lookup_step(keyword: &str, pattern: &str) -> Option<StepFn> {
    STEP_MAP.get(&(keyword, pattern)).copied()
}

/// Find a registered step whose pattern matches the provided text.
///
/// The search first attempts an exact match via `lookup_step` and then falls
/// back to evaluating each registered pattern for placeholders.
#[must_use]
pub fn find_step(keyword: &str, text: &str) -> Option<StepFn> {
    if let Some(f) = lookup_step(keyword, text) {
        return Some(f);
    }
    for step in iter::<Step> {
        if step.keyword == keyword && extract_placeholders(step.pattern, text).is_some() {
            return Some(step.run);
        }
    }
    None
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
        fn wrapper(ctx: &StepContext<'_>, _text: &str) {
            let _ = ctx;
            sample();
        }

        step!("Given", "a pattern", wrapper, &[]);

        let found = iter::<Step>
            .into_iter()
            .any(|step| step.pattern == "a pattern");

        assert!(found, "registered step was not found in the inventory");
    }
}
