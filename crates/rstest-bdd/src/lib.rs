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
    #[expect(
        clippy::expect_used,
        reason = "pattern is verified; invalid regex indicates programmer error"
    )]
    Regex::new(r"\{\{|\}\}|\{(?:[A-Za-z_][A-Za-z0-9_]*)(?::(?P<ty>[^}]+))?\}")
        .expect("invalid placeholder regex")
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
/// Placeholders follow `{name[:type]}`; `name` must start with a letter or
/// underscore and may contain letters, digits, or underscores. Whitespace
/// within the type hint is ignored (for example, `{n: f64}` and `{n:f64}` are
/// equivalent), but whitespace is not allowed between the name and the colon.
/// Literal braces may be escaped by doubling them: `{{` or `}}`. Nested braces
/// inside placeholders are not supported.
///
/// Type hints:
/// - Integers (`u*`/`i*`): decimal digits with an optional sign for signed
///   types.
/// - Floats (`f32`/`f64`): integers, decimals with optional leading or trailing
///   digits, optional scientific exponents, or `NaN`/`inf`/`Infinity`
///   (case-insensitive).
///
/// The returned vector contains the raw substring for each placeholder in order
/// of appearance. The entire step text must match the pattern; otherwise this
/// returns `None`.
#[must_use]
pub fn extract_placeholders(pattern: &StepPattern, text: StepText<'_>) -> Option<Vec<String>> {
    extract_captured_values(pattern.regex(), text.as_str())
}

/// Update unmatched brace depth by scanning ASCII brace bytes.
#[inline]
fn update_brace_depth(text: &str, mut depth: usize) -> usize {
    for b in text.bytes() {
        match b {
            b'{' => depth = depth.saturating_add(1),
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

/// Return the regex fragment for a placeholder type hint.
fn get_type_pattern(type_hint: Option<&str>) -> &'static str {
    match type_hint {
        Some("u8" | "u16" | "u32" | "u64" | "u128" | "usize") => r"\d+",
        Some("i8" | "i16" | "i32" | "i64" | "i128" | "isize") => r"[+-]?\d+",
        Some("f32" | "f64") => {
            r"(?i:(?:[+-]?(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?|nan|inf|infinity))"
        }
        _ => r".+?",
    }
}

/// Append a match segment and return the updated unmatched brace depth.
fn process_placeholder_match(
    match_text: &str,
    type_hint: Option<&str>,
    depth: usize,
    regex: &mut String,
) -> usize {
    if depth == 0 {
        match match_text {
            "{{" => regex.push_str(r"\{"),
            "}}" => regex.push_str(r"\}"),
            _ => {
                regex.push('(');
                regex.push_str(get_type_pattern(type_hint));
                regex.push(')');
            }
        }
        depth
    } else {
        regex.push_str(&regex::escape(match_text));
        // Adjust depth for braces inside unmatched regions.
        update_brace_depth(match_text, depth)
    }
}

fn build_regex_from_pattern(pat: &str) -> String {
    // Precompiled regex splits the pattern into literal fragments and
    // placeholders. `depth` tracks unmatched opening braces so a stray `{`
    // causes subsequent braces to be treated as literals.
    let ph_re = &PLACEHOLDER_RE;
    let mut regex = String::with_capacity(pat.len().saturating_mul(2) + 2);
    regex.push('^');
    let mut last = 0usize;
    let mut depth = 0usize;
    for cap in ph_re.captures_iter(pat) {
        #[expect(clippy::expect_used, reason = "placeholder regex guarantees a capture")]
        let m = cap.get(0).expect("placeholder capture missing");
        if let Some(literal) = pat.get(last..m.start()) {
            regex.push_str(&regex::escape(literal));
            depth = update_brace_depth(literal, depth);
        }

        let mat = m.as_str();
        let ty = cap.name("ty").map(|m| m.as_str().trim());
        depth = process_placeholder_match(mat, ty, depth, &mut regex);
        last = m.end();
    }
    if let Some(tail) = pat.get(last..) {
        regex.push_str(&regex::escape(tail));
    }
    regex.push('$');
    regex
}

fn extract_captured_values(re: &Regex, text: &str) -> Option<Vec<String>> {
    let caps = re.captures(text)?;
    let mut values = Vec::new();
    for i in 1..caps.len() {
        values.push(caps[i].to_string());
    }
    Some(values)
}

/// Error type produced by step wrappers.
///
/// The variants categorise the possible failure modes when invoking a step.
#[derive(Debug, thiserror::Error)]
pub enum StepError {
    /// Raised when a required fixture is absent from the [`StepContext`].
    #[error("Missing fixture '{name}' of type '{ty}' for step function '{step}'")]
    MissingFixture {
        name: String,
        ty: String,
        step: String,
    },

    /// Wraps generic execution failures.
    #[error("Step execution failed: {0}")]
    ExecutionError(String),

    /// Indicates a panic occurred inside the step function.
    #[error("Panic in step '{pattern}', function '{function}': {message}")]
    PanicError {
        pattern: String,
        function: String,
        message: String,
    },
}

/// Type alias for the stored step function pointer.
pub type StepFn =
    for<'a> fn(&StepContext<'a>, &str, Option<&str>, Option<&[&[&str]]>) -> Result<(), StepError>;

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
