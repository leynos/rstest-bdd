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
use std::str::FromStr;
use std::sync::{LazyLock, OnceLock};

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

/// Pattern text used to match a step at runtime.
///
/// The struct caches a compiled regular expression derived from the pattern so
/// placeholder extraction does not rebuild the regex on every invocation.
#[derive(Debug)]
pub struct StepPattern {
    raw: &'static str,
    regex: OnceLock<Regex>,
}

impl StepPattern {
    /// Create a new pattern wrapper from a string literal.
    #[must_use]
    pub const fn new(value: &'static str) -> Self {
        Self {
            raw: value,
            regex: OnceLock::new(),
        }
    }

    /// Access the underlying pattern string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.raw
    }

    fn regex(&self) -> &Regex {
        self.regex.get_or_init(|| {
            let src = build_regex_from_pattern(self.raw);
            Regex::new(&src).unwrap_or_else(|e| panic!("invalid step pattern: {e}"))
        })
    }

    /// Extract captured values from the provided text using the cached regex.
    #[must_use]
    pub fn captures(&self, text: StepText<'_>) -> Option<Vec<String>> {
        extract_captured_values(self.regex(), text.as_str())
    }
}

impl PartialEq for StepPattern {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Eq for StepPattern {}

impl std::hash::Hash for StepPattern {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
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
/// The pattern supports `format!`-style placeholders such as `{count:u32}` and
/// honours escaped braces (`{{` and `}}`). Any text outside placeholders must
/// match exactly. The returned vector contains the raw substring for each
/// placeholder in order of appearance.
#[must_use]
pub fn extract_placeholders(pattern: PatternStr<'_>, text: StepText<'_>) -> Option<Vec<String>> {
    let regex_source = build_regex_from_pattern(pattern.as_str());
    let re = Regex::new(&regex_source).ok()?;
    extract_captured_values(&re, text.as_str())
}

fn build_regex_from_pattern(pattern: &str) -> String {
    let mut regex_source = String::from("^");
    let mut chars = pattern.chars().peekable();
    loop {
        let Some(ch) = chars.next() else {
            break;
        };
        match ch {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    regex_source.push_str("\\{");
                } else {
                    let placeholder = consume_placeholder(&mut chars);
                    let ty = placeholder.split(':').nth(1);
                    let sub = type_subpattern(ty);
                    regex_source.push('(');
                    regex_source.push_str(sub);
                    regex_source.push(')');
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    regex_source.push_str("\\}");
                } else {
                    regex_source.push_str("\\}");
                }
            }
            _ => regex_source.push_str(&regex::escape(&ch.to_string())),
        }
    }
    regex_source.push('$');
    regex_source
}

fn consume_placeholder(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut out = String::new();
    let mut depth = 1;
    for c in chars.by_ref() {
        match c {
            '{' => {
                depth += 1;
                out.push(c);
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

fn type_subpattern(ty: Option<&str>) -> &'static str {
    match ty.unwrap_or("") {
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => "\\d+",
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => "-?\\d+",
        "f32" | "f64" => "-?\\d+(?:\\.\\d+)?",
        "bool" => "(?:true|false)",
        _ => "[^}]*",
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
    ($keyword:expr, $pattern:expr, $handler:path, $fixtures:expr) => {
        const _: () = {
            static PATTERN: $crate::StepPattern = $crate::StepPattern::new($pattern);
            $crate::submit! {
                $crate::Step {
                    keyword: $keyword,
                    pattern: &PATTERN,
                    run: $handler,
                    fixtures: $fixtures,
                    file: file!(),
                    line: line!(),
                }
            }
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
        if step.keyword == keyword && step.pattern.captures(text).is_some() {
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

        step!(StepKeyword::Given, "a pattern", wrapper, &[]);

        let found = iter::<Step>
            .into_iter()
            .any(|step| step.pattern.as_str() == "a pattern");

        assert!(found, "registered step was not found in the inventory");
    }
}
