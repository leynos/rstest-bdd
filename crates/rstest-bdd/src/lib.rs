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

use gherkin::StepType;
pub use inventory::{iter, submit};
use regex::Regex;
use std::any::Any;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{LazyLock, OnceLock};

// Compile once: used by `build_regex_from_pattern` for splitting pattern text.
static PLACEHOLDER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{|}}|\{([A-Za-z_][A-Za-z0-9_]*)(?::([^}]+))?\}")
        .unwrap_or_else(|e| panic!("invalid placeholder regex: {e}"))
});

/// Wrapper for step pattern strings used in matching logic
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PatternStr<'a>(&'a str);

impl<'a> PatternStr<'a> {
    /// Construct a new `PatternStr` from a string slice.
    #[must_use]
    pub const fn new(s: &'a str) -> Self {
        Self(s)
    }

    /// Access the underlying string slice.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> From<&'a str> for PatternStr<'a> {
    fn from(s: &'a str) -> Self {
        Self::new(s)
    }
}

/// Wrapper for step text content from scenarios
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepText<'a>(&'a str);

impl<'a> StepText<'a> {
    /// Construct a new `StepText` from a string slice.
    #[must_use]
    pub const fn new(s: &'a str) -> Self {
        Self(s)
    }

    /// Access the underlying string slice.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> From<&'a str> for StepText<'a> {
    fn from(s: &'a str) -> Self {
        Self::new(s)
    }
}

/// Keyword used to categorize a step definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepKeyword {
    /// Setup preconditions for a scenario.
    Given,
    /// Perform an action when testing behaviour.
    When,
    /// Assert the expected outcome of a scenario.
    Then,
    /// Additional conditions that share context with the previous step.
    And,
    /// Negative or contrasting conditions.
    But,
}

impl StepKeyword {
    /// Return the keyword as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Given => "Given",
            Self::When => "When",
            Self::Then => "Then",
            Self::And => "And",
            Self::But => "But",
        }
    }
}

/// Error returned when parsing a `StepKeyword` from a string fails.
#[derive(Debug, thiserror::Error)]
#[error("invalid step keyword: {0}")]
pub struct StepKeywordParseError(pub String);

impl FromStr for StepKeyword {
    type Err = StepKeywordParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let kw = match value {
            "Given" => Self::Given,
            "When" => Self::When,
            "Then" => Self::Then,
            "And" => Self::And,
            "But" => Self::But,
            other => return Err(StepKeywordParseError(other.to_string())),
        };
        Ok(kw)
    }
}

impl From<&str> for StepKeyword {
    fn from(value: &str) -> Self {
        Self::from_str(value).unwrap_or_else(|_| panic!("invalid step keyword: {value}"))
    }
}

impl From<StepType> for StepKeyword {
    fn from(ty: StepType) -> Self {
        match ty {
            StepType::Given => Self::Given,
            StepType::When => Self::When,
            StepType::Then => Self::Then,
        }
    }
}

/// Pattern text used to match a step at runtime.
#[derive(Debug)]
pub struct StepPattern {
    text: &'static str,
    regex: OnceLock<Regex>,
}

impl StepPattern {
    /// Create a new pattern wrapper from a string literal.
    #[must_use]
    pub const fn new(value: &'static str) -> Self {
        Self {
            text: value,
            regex: OnceLock::new(),
        }
    }

    /// Access the underlying pattern string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.text
    }

    /// Compile the pattern into a regular expression, caching the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the pattern cannot be converted into a valid
    /// regular expression.
    pub fn compile(&self) -> Result<(), regex::Error> {
        let src = build_regex_from_pattern(self.text);
        let regex = Regex::new(&src)?;
        // Ignore result if already set; duplicate registration is benign.
        let _ = self.regex.set(regex);
        Ok(())
    }

    /// Return the cached regular expression.
    ///
    /// # Panics
    ///
    /// Panics if the pattern has not been compiled via [`compile`].
    #[must_use]
    pub fn regex(&self) -> &Regex {
        self.regex
            .get()
            .unwrap_or_else(|| panic!("step pattern regex must be precompiled"))
    }
}

impl From<&'static str> for StepPattern {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

type StepKey = (StepKeyword, &'static str);

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
/// Literal braces may be escaped by doubling them: `{{` or `}}`. Nested braces
/// inside placeholders are not supported. The returned vector contains the raw
/// substring for each placeholder in order of appearance. The entire step text
/// must match the pattern; otherwise this returns `None`.
#[must_use]
pub fn extract_placeholders(pattern: &StepPattern, text: StepText<'_>) -> Option<Vec<String>> {
    extract_captured_values(pattern.regex(), text.as_str())
}

fn build_regex_from_pattern(pat: &str) -> String {
    // Split the pattern into literal fragments and placeholders. A small depth
    // counter ensures that an unmatched `{` causes subsequent braces to be
    // treated literally rather than as placeholders.
    let ph_re = &PLACEHOLDER_RE;
    let mut regex = String::from("^");
    let mut last = 0;
    let mut depth = 0i32;
    for cap in ph_re.captures_iter(pat) {
        let m = cap.get(0).unwrap_or_else(|| panic!("capture missing"));
        if let Some(lit) = pat.get(last..m.start()) {
            for ch in lit.chars() {
                match ch {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            regex.push_str(&regex::escape(lit));
        }
        if depth == 0 {
            match m.as_str() {
                "{{" => regex.push_str(r"\{"),
                "}}" => regex.push_str(r"\}"),
                _ => {
                    let ty = cap.get(2).map(|m| m.as_str().trim());
                    regex.push('(');
                    regex.push_str(type_subpattern(ty));
                    regex.push(')');
                }
            }
        } else {
            for ch in m.as_str().chars() {
                match ch {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            regex.push_str(&regex::escape(m.as_str()));
        }
        last = m.end();
    }
    if let Some(tail) = pat.get(last..) {
        regex.push_str(&regex::escape(tail));
    }
    regex.push('$');
    regex
}

fn type_subpattern(ty: Option<&str>) -> &'static str {
    match ty {
        Some("u8" | "u16" | "u32" | "u64" | "u128" | "usize") => r"\d+",
        Some("i8" | "i16" | "i32" | "i64" | "i128" | "isize") => r"[+-]?\d+",
        // Accept integers, decimal forms with optional leading/trailing digits,
        // and scientific notation to match `FromStr` semantics.
        Some("f32" | "f64") => r"[+-]?(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?",
        _ => r".+?",
    }
}

fn extract_captured_values(re: &Regex, text: &str) -> Option<Vec<String>> {
    let caps = re.captures(text)?;
    let mut values = Vec::new();
    for i in 1..caps.len() {
        values.push(caps[i].to_string());
    }
    Some(values)
}

/// Type alias for the stored step function pointer.
pub type StepFn =
    for<'a> fn(&StepContext<'a>, &str, Option<&str>, Option<&[&[&str]]>) -> Result<(), String>;

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
    pub keyword: StepKeyword,
    /// Pattern text used to match a Gherkin step.
    pub pattern: &'static StepPattern,
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
    (@pattern $keyword:expr, $pattern:expr, $handler:path, $fixtures:expr) => {
        const _: () = {
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
    };

    ($keyword:expr, $pattern:expr, $handler:path, $fixtures:expr) => {
        const _: () = {
            static PATTERN: $crate::StepPattern = $crate::StepPattern::new($pattern);
    $crate::step!(@pattern $keyword, &PATTERN, $handler, $fixtures);
        };
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
        step.pattern.compile().unwrap_or_else(|e| {
            panic!(
                "invalid step pattern '{}' at {}:{}: {e}",
                step.pattern.as_str(),
                step.file,
                step.line
            )
        });
        map.insert((step.keyword, step.pattern.as_str()), step.run);
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
pub fn lookup_step(keyword: StepKeyword, pattern: PatternStr<'_>) -> Option<StepFn> {
    STEP_MAP.get(&(keyword, pattern.as_str())).copied()
}

/// Find a registered step whose pattern matches the provided text.
///
/// The search first attempts an exact match via `lookup_step` and then falls
/// back to evaluating each registered pattern for placeholders.
#[must_use]
pub fn find_step(keyword: StepKeyword, text: StepText<'_>) -> Option<StepFn> {
    if let Some(f) = lookup_step(keyword, text.as_str().into()) {
        return Some(f);
    }
    for step in iter::<Step> {
        if step.keyword == keyword && extract_placeholders(step.pattern, text).is_some() {
            return Some(step.run);
        }
    }
    None
}
