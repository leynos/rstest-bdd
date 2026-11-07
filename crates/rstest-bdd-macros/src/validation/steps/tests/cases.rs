//! Named inputs for the step-validation fixtures.
//!
//! Each struct here groups the strings one fixture needs into a single value.
//! The fixtures are all string-driven by nature — a step pattern, the text it
//! is matched against, the library that owns it — so passing them as loose
//! parameters made every call site a positional argument list in which two
//! adjacent `&str`s were interchangeable. Naming the group removes that
//! ambiguity: `PatternCase { pattern, text }` says which string is which, and a
//! fixture gains a field without every caller having to re-check its argument
//! order.
//!
//! The grouping also keeps the arguments self-documenting where the values are
//! inline literals, which is the usual shape in these tests.

/// A pattern and the step text expected to match it.
#[derive(Clone, Copy, Debug)]
pub(super) struct PatternCase<'a> {
    /// Registered pattern, which may contain placeholders.
    pub(super) pattern: &'a str,
    /// Step text a scenario would supply.
    pub(super) text: &'a str,
}

/// A step text, plus the foreign definition that must not satisfy it.
///
/// The definition is optional so one fixture covers both "nothing registered"
/// and "registered by another crate" without a second function.
#[derive(Clone, Copy, Debug)]
pub(super) struct StrictCase<'a> {
    /// Pattern and owning crate for a definition outside the test crate.
    pub(super) foreign: Option<(&'a str, &'a str)>,
    /// Step text under validation.
    pub(super) text: &'a str,
}

/// Two patterns that both accept the same step text.
#[derive(Clone, Copy, Debug)]
pub(super) struct AmbiguousCase<'a> {
    /// First registered pattern.
    pub(super) pattern_a: &'a str,
    /// Second registered pattern.
    pub(super) pattern_b: &'a str,
    /// Text both patterns match.
    pub(super) text: &'a str,
}

/// A crate identity and the library whose index an assertion inspects.
#[derive(Clone, Copy, Debug)]
pub(super) struct RegistrationCase<'a> {
    /// Crate whose registry entry is examined.
    pub(super) crate_id: &'a str,
    /// Library expected to receive the registration.
    pub(super) library: &'a str,
}

/// Definitions to register and the libraries the scenario selects.
#[derive(Clone, Copy, Debug)]
pub(super) struct ScopedCase<'a> {
    /// `(library, pattern)` pairs registered before validation.
    pub(super) definitions: &'a [(&'a str, &'a str)],
    /// Libraries the scenario names.
    pub(super) selected: &'a [&'a str],
}
