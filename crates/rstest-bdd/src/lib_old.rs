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
use thiserror::Error;

//

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
#[derive(Debug, Error)]
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

/// Error conditions that may arise when extracting placeholders.
#[derive(Debug, Error)]
pub enum PlaceholderError {
    /// The supplied text did not match the step pattern.
    #[error("pattern mismatch")]
    PatternMismatch,
    /// The step pattern could not be compiled into a regular expression.
    #[error("invalid step pattern: {0}")]
    InvalidPattern(String),
    /// The step pattern was not compiled before use.
    #[error("uncompiled step pattern")]
    Uncompiled,
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
/// Literal braces may be escaped with `\{` or `\}`. Nested braces within
/// placeholders are honoured, preventing greedy captures. The returned vector
/// contains the raw substring for each placeholder in order of appearance.
///
/// # Errors
/// Returns [`PlaceholderError::PatternMismatch`] if the text does not satisfy
/// the pattern. The entire step text must match the pattern for a successful
/// extraction.
pub fn extract_placeholders(
    pattern: &StepPattern,
    text: StepText<'_>,
) -> Result<Vec<String>, PlaceholderError> {
    // Compile the pattern (caching the result) and map compile failures.
    pattern
        .compile()
        .map_err(|e| PlaceholderError::InvalidPattern(e.to_string()))?;
    // Retrieve the compiled regex without panicking if compilation was skipped.
    let re = pattern.regex.get().ok_or(PlaceholderError::Uncompiled)?;
    extract_captured_values(re, text.as_str()).ok_or(PlaceholderError::PatternMismatch)
}

//

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

//

//

#[cfg(any())]
#[expect(
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::string_slice,
    reason = "Scanner bounds are guarded; UTF-8 slices only occur over ASCII"
)]
fn build_regex_from_pattern(pat: &str) -> String {
    // Single-pass scanner that supports:
    // - Escaped braces via "\\{" and "\\}"
    // - Escaped literal braces via "{{" and "}}"
    // - Placeholders: `{name[:type]}` with nested braces allowed inside
    //   the placeholder content to determine the correct closing `}`.
    let bytes = pat.as_bytes();
    let mut i = 0usize;
    let len = bytes.len();
    let mut regex = String::with_capacity(pat.len().saturating_mul(2) + 2);
    regex.push('^');
    // Tracks unmatched literal opening braces that should block placeholders
    // until balanced again.
    let mut stray_depth: usize = 0;
    while i < len {
        if stray_depth > 0 {
            // Inside a literal-brace region: emit all characters literally and
            // maintain balance so placeholders remain disabled.
            if i + 1 < len && bytes[i] == b'{' && bytes[i + 1] == b'{' {
                regex.push_str(r"\{");
                i += 2;
                stray_depth += 1; // treat nested as additional literal brace
                continue;
            }
            if i + 1 < len && bytes[i] == b'}' && bytes[i + 1] == b'}' {
                regex.push_str(r"\}");
                i += 2;
                stray_depth = stray_depth.saturating_sub(1);
                continue;
            }
            if i + 1 < len && bytes[i] == b'\\' && (bytes[i + 1] == b'{' || bytes[i + 1] == b'}') {
                let ch = bytes[i + 1] as char;
                regex.push_str(&regex::escape(&ch.to_string()));
                i += 2;
                continue;
            }
            let ch = bytes[i] as char;
            if ch == '{' {
                stray_depth += 1;
            } else if ch == '}' {
                stray_depth = stray_depth.saturating_sub(1);
            }
            regex.push_str(&regex::escape(&ch.to_string()));
            i += 1;
            continue;
        }
        // Double-brace escapes for literal braces
        if i + 1 < len && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            regex.push_str(r"\{");
            i += 2;
            continue;
        }
        if i + 1 < len && bytes[i] == b'}' && bytes[i + 1] == b'}' {
            regex.push_str(r"\}");
            i += 2;
            continue;
        }

        // Backslash-escaped braces in the pattern become literal braces.
        if i + 1 < len && bytes[i] == b'\\' && (bytes[i + 1] == b'{' || bytes[i + 1] == b'}') {
            let ch = bytes[i + 1] as char;
            regex.push_str(&regex::escape(&ch.to_string()));
            i += 2;
            continue;
        }

        // Placeholder start: `{` followed by identifier start.
        if i + 1 < len
            && bytes[i] == b'{'
            && ((bytes[i + 1] as char).is_ascii_alphabetic() || bytes[i + 1] == b'_')
        {
            // Parse name
            let mut j = i + 2;
            while j < len {
                let c = bytes[j];
                if (c as char).is_ascii_alphanumeric() || c == b'_' {
                    j += 1;
                } else {
                    break;
                }
            }

            // Optional type hint: `:ty` up to the matching `}` (nesting allowed).
            let mut k = j;
            let mut type_hint: Option<String> = None;
            let mut nest = 0usize;
            // Whitespace immediately before ':' invalidates the placeholder.
            if k < len && (bytes[k] as char).is_ascii_whitespace() {
                let mut ws = k;
                while ws < len && (bytes[ws] as char).is_ascii_whitespace() {
                    ws += 1;
                }
                if ws < len && bytes[ws] == b':' {
                    // Malformed `name : ty` -> treat `{` as literal and block placeholders.
                    regex.push_str(r"\{");
                    i += 1;
                    stray_depth = stray_depth.saturating_add(1);
                    continue;
                }
                k = j; // no colon; fall through to scan for matching `}`
            }
            if k < len && bytes[k] == b':' {
                k += 1;
                let ty_start = k;
                let mut had_nested = false;
                while k < len {
                    match bytes[k] {
                        b'{' => {
                            had_nested = true;
                            nest += 1;
                            k += 1;
                        }
                        b'}' => {
                            if nest == 0 {
                                break;
                            }
                            nest -= 1;
                            k += 1;
                        }
                        _ => k += 1,
                    }
                }
                let ty = pat[ty_start..k].trim().to_string();
                if ty.is_empty() {
                    // Empty type hint -> treat as literal and block placeholders
                    regex.push_str(r"\{");
                    i += 1;
                    stray_depth = stray_depth.saturating_add(1);
                    continue;
                }
                type_hint = Some(ty);
                // If nested braces were present in the placeholder content,
                // honour a trailing literal '}' in the pattern to avoid
                // greedy consumption.
                if had_nested {
                    // Emit capture now, then a literal closing brace below
                }
            } else {
                // No explicit type hint; scan to matching `}` allowing nested.
                while k < len {
                    match bytes[k] {
                        b'{' => {
                            nest += 1;
                            k += 1;
                        }
                        b'}' => {
                            if nest == 0 {
                                break;
                            }
                            nest -= 1;
                            k += 1;
                        }
                        _ => k += 1,
                    }
                }
            }

            // If no closing brace found, treat `{` as literal.
            if k >= len || bytes[k] != b'}' {
                // Unclosed placeholder start -> treat `{` as literal and block placeholders
                regex.push_str(r"\{");
                i += 1;
                stray_depth = stray_depth.saturating_add(1);
                continue;
            }

            // Emit the capture group according to the type hint.
            regex.push('(');
            regex.push_str(get_type_pattern(type_hint.as_deref()));
            regex.push(')');
            // For placeholders with nested braces in the type section, require
            // an extra literal closing brace in the input.
            if let Some(ty) = &type_hint {
                if ty.contains('{') {
                    regex.push_str(r"\}");
                }
            }
            i = k + 1;
            continue;
        }

        // Default: emit literal char. A lone '{' opens a stray region that
        // disables placeholders until balanced.
        let ch = bytes[i] as char;
        if ch == '{' {
            stray_depth += 1;
        }
        regex.push_str(&regex::escape(&ch.to_string()));
        i += 1;
    }
    regex.push('$');
    regex
}

// Refactored scanner: state and helpers
struct RegexBuilder<'a> {
    pattern: &'a str,
    bytes: &'a [u8],
    position: usize,
    output: String,
    stray_depth: usize,
}

impl<'a> RegexBuilder<'a> {
    fn new(pattern: &'a str) -> Self {
        let mut output = String::with_capacity(pattern.len().saturating_mul(2) + 2);
        output.push('^');
        Self {
            pattern,
            bytes: pattern.as_bytes(),
            position: 0,
            output,
            stray_depth: 0,
        }
    }
    #[inline]
    fn has_more(&self) -> bool {
        self.position < self.bytes.len()
    }
    #[inline]
    fn advance(&mut self, n: usize) {
        self.position = self.position.saturating_add(n);
    }
    #[inline]
    fn push_literal_byte(&mut self, b: u8) {
        self.output
            .push_str(&regex::escape(&(b as char).to_string()));
    }
    #[inline]
    fn push_literal_brace(&mut self, brace: u8) {
        self.push_literal_byte(brace);
    }
    #[inline]
    fn push_capture_for_type(&mut self, ty: Option<&str>) {
        self.output.push('(');
        self.output.push_str(get_type_pattern(ty));
        self.output.push(')');
    }
}

#[inline]
fn is_escaped_brace(bytes: &[u8], pos: usize) -> bool {
    matches!(bytes.get(pos), Some(b'\\')) && matches!(bytes.get(pos + 1), Some(b'{' | b'}'))
}

#[inline]
fn is_double_brace(bytes: &[u8], pos: usize) -> bool {
    let first = match bytes.get(pos) {
        Some(b @ (b'{' | b'}')) => *b,
        _ => return false,
    };
    matches!(bytes.get(pos + 1), Some(b) if *b == first)
}

#[inline]
fn is_placeholder_start(bytes: &[u8], pos: usize) -> bool {
    matches!(bytes.get(pos), Some(b'{'))
        && matches!(bytes.get(pos + 1), Some(b) if (*b as char).is_ascii_alphabetic() || *b == b'_')
}

#[inline]
fn is_empty_type_hint(state: &RegexBuilder<'_>, name_end: usize) -> bool {
    if !matches!(state.bytes.get(name_end), Some(b':')) {
        return false;
    }
    let mut i = name_end + 1;
    while let Some(&b) = state.bytes.get(i) {
        if b == b'}' {
            return true;
        }
        if !(b as char).is_ascii_whitespace() {
            return false;
        }
        i += 1;
    }
    false
}

fn parse_escaped_brace(state: &mut RegexBuilder<'_>) {
    #[expect(clippy::indexing_slicing, reason = "predicate ensured bound")]
    let ch = state.bytes[state.position + 1];
    state.push_literal_brace(ch);
    state.advance(2);
}

fn parse_double_brace(state: &mut RegexBuilder<'_>) {
    #[expect(clippy::indexing_slicing, reason = "predicate ensured bound")]
    let brace = state.bytes[state.position];
    state.push_literal_brace(brace);
    if state.stray_depth > 0 {
        if brace == b'{' {
            state.stray_depth = state.stray_depth.saturating_add(1);
        }
        if brace == b'}' {
            state.stray_depth = state.stray_depth.saturating_sub(1);
        }
    }
    state.advance(2);
}

fn parse_literal(state: &mut RegexBuilder<'_>) {
    #[expect(clippy::indexing_slicing, reason = "caller ensured bound")]
    let ch = state.bytes[state.position];
    if ch == b'{' {
        state.stray_depth = state.stray_depth.saturating_add(1);
    }
    state.push_literal_byte(ch);
    state.advance(1);
}

fn parse_placeholder_name(state: &RegexBuilder<'_>, start: usize) -> (usize, String) {
    let mut i = start + 1;
    let mut name = String::new();
    while let Some(&b) = state.bytes.get(i) {
        if (b as char).is_ascii_alphanumeric() || b == b'_' {
            name.push(b as char);
            i += 1;
        } else {
            break;
        }
    }
    (i, name)
}

fn parse_type_hint(state: &RegexBuilder<'_>, start: usize) -> (usize, Option<String>) {
    let mut i = start;
    if !matches!(state.bytes.get(i), Some(b':')) {
        return (i, None);
    }
    i += 1;
    let ty_start = i;
    let mut nest = 0usize;
    while let Some(&b) = state.bytes.get(i) {
        match b {
            b'{' => {
                nest += 1;
                i += 1;
            }
            b'}' => {
                if nest == 0 {
                    break;
                }
                nest -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    #[expect(clippy::string_slice, reason = "ASCII region delimited by braces")]
    let ty = state.pattern[ty_start..i].trim().to_string();
    if ty.is_empty() {
        return (start, None);
    }
    (i, Some(ty))
}

fn parse_placeholder(state: &mut RegexBuilder<'_>) {
    let start = state.position;
    let (name_end, _name) = parse_placeholder_name(state, start + 1);
    if let Some(b) = state.bytes.get(name_end) {
        if (*b as char).is_ascii_whitespace() {
            let mut ws = name_end;
            while let Some(bw) = state.bytes.get(ws) {
                if !(*bw as char).is_ascii_whitespace() {
                    break;
                }
                ws += 1;
            }
            if matches!(state.bytes.get(ws), Some(b':')) {
                state.output.push_str(r"\{");
                state.advance(1);
                state.stray_depth = state.stray_depth.saturating_add(1);
                return;
            }
        }
    }
    if is_empty_type_hint(state, name_end) {
        state.output.push_str(r"\{");
        state.advance(1);
        state.stray_depth = state.stray_depth.saturating_add(1);
        return;
    }
    let (mut after, ty_opt) = parse_type_hint(state, name_end);
    if ty_opt.is_none() {
        // No explicit type hint; scan to matching '}' allowing nested.
        let mut k = name_end;
        let mut nest = 0usize;
        while let Some(&b) = state.bytes.get(k) {
            match b {
                b'{' => {
                    nest += 1;
                    k += 1;
                }
                b'}' => {
                    if nest == 0 {
                        break;
                    }
                    nest -= 1;
                    k += 1;
                }
                _ => k += 1,
            }
        }
        after = k;
    }
    if !matches!(state.bytes.get(after), Some(b'}')) {
        state.output.push_str(r"\{");
        state.advance(1);
        state.stray_depth = state.stray_depth.saturating_add(1);
        return;
    }
    state.push_capture_for_type(ty_opt.as_deref());
    if ty_opt.as_ref().is_some_and(|t| t.contains('{')) {
        state.output.push_str(r"\}");
    }
    after += 1;
    state.position = after;
}

fn build_regex_from_pattern(pat: &str) -> String {
    let mut st = RegexBuilder::new(pat);
    while st.has_more() {
        if st.stray_depth > 0 {
            if is_double_brace(st.bytes, st.position) {
                parse_double_brace(&mut st);
                continue;
            }
            if is_escaped_brace(st.bytes, st.position) {
                parse_escaped_brace(&mut st);
                continue;
            }
            #[expect(clippy::indexing_slicing, reason = "bounds checked by has_more")]
            let ch = st.bytes[st.position];
            if ch == b'{' {
                st.stray_depth = st.stray_depth.saturating_add(1);
            }
            if ch == b'}' {
                st.stray_depth = st.stray_depth.saturating_sub(1);
            }
            st.push_literal_byte(ch);
            st.advance(1);
            continue;
        }
        if is_double_brace(st.bytes, st.position) {
            parse_double_brace(&mut st);
            continue;
        }
        if is_escaped_brace(st.bytes, st.position) {
            parse_escaped_brace(&mut st);
            continue;
        }
        if is_placeholder_start(st.bytes, st.position) {
            parse_placeholder(&mut st);
            continue;
        }
        parse_literal(&mut st);
    }
    st.output.push('$');
    st.output
}

fn extract_captured_values(re: &Regex, text: &str) -> Option<Vec<String>> {
    let caps = re.captures(text)?;
    let mut values = Vec::new();
    for i in 1..caps.len() {
        values.push(caps[i].to_string());
    }
    Some(values)
}

#[cfg(test)]
mod internal_tests;

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
        if step.keyword == keyword && extract_placeholders(step.pattern, text).is_ok() {
            return Some(step.run);
        }
    }
    None
}
