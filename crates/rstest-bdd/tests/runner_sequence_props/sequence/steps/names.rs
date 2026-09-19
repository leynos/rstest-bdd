//! The pattern text every generated invocation names.
//!
//! # Why these are duplicated from the registrations
//!
//! A step attribute takes a string literal, an `expr = "..."` literal, or a
//! return override — never a path. `step_attr_args.rs` parses exactly those
//! three forms and rejects anything else with "expected `result` or `value`",
//! so `#[given(names::PASSES)]` cannot compile and the literal must be written
//! at the registration site.
//!
//! Duplicating it here would be a silent hazard on its own: a constant that
//! drifted from its literal would make the generated invocation resolve to
//! nothing, and every case using that kind would quietly classify as
//! `Undefined` rather than as the terminal it was drawn to be. So the
//! duplication is not left to trust.
//! `runner_sequence_props::named_witnesses::each_terminal_kind_is_reached_by_its_own_witness`
//! runs a plan naming every kind and requires each to reach the status, the
//! terminal index, and the classification that `Kind` declares for it. A
//! constant that no longer names its registered step resolves to nothing,
//! yields `Undefined`, and fails there — the binding is checked by the runtime
//! rather than by a second copy of the same literal.

/// Resolves and does nothing.
pub(crate) const PASSES: &str = "a sequence probe step passes {index:usize}";

/// Resolves and returns a probe carrying its producing index.
pub(crate) const RETURN: &str = "a sequence probe step returns probe {index:usize}";

/// Resolves and returns a value no fixture can match by type.
pub(crate) const RETURN_UNMATCHED: &str =
    "a sequence probe step returns an unmatched value {index:usize}";

/// Resolves and records what the probe name resolves to.
pub(crate) const OBSERVE: &str = "a sequence probe step {observer:usize} observes the probe";

/// Resolves and asks to be skipped.
pub(crate) const SKIPS: &str = "a sequence probe step skips {index:usize}";

/// Resolves and returns an error.
pub(crate) const FAILS: &str = "a sequence probe step fails {index:usize}";

/// Resolves and panics.
pub(crate) const PANICS: &str = "a sequence probe step panics {index:usize}";

/// Resolves to a step declaring a fixture the context does not hold.
pub(crate) const MISSING_FIXTURE: &str = "a sequence probe step needs an absent fixture";

/// Resolves to nothing, because no step is registered for this text.
///
/// Deliberately absent from the registry, so the agreement test above must
/// exclude it rather than require it.
pub(crate) const UNREGISTERED: &str = "a sequence probe step nobody registered";

/// The name a lone probe fixture is registered under.
pub(crate) const PROBE: &str = "sequence probe";

/// The name a second probe fixture is registered under.
pub(crate) const OTHER_PROBE: &str = "sequence probe second";

/// Fill a pattern's `{name:type}` placeholder with `value`.
///
/// The generator spells each invocation's text through this, so a pattern's
/// placeholder is what lets a handler learn its own position without being
/// told. A pattern with no placeholder is returned unchanged.
///
/// Written with [`str::split_once`] rather than with byte offsets: the offsets
/// a `find` returns are byte positions, and slicing on one is only safe because
/// every brace in a pattern here happens to be ASCII. Splitting on the
/// delimiter cannot get that wrong for a pattern where one is not.
pub(crate) fn substitute(pattern: &str, value: usize) -> String {
    let Some((head, rest)) = pattern.split_once('{') else {
        return pattern.to_owned();
    };
    let Some((_, tail)) = rest.split_once('}') else {
        return pattern.to_owned();
    };
    format!("{head}{value}{tail}")
}
