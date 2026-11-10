//! Property tests for the fingerprint that decides stamp validity.
//!
//! The named tests in `tests::inputs` pin the composition against a chosen
//! toolchain identity and one forged line break. They are specifications of
//! individual cases; the relations below hold over arbitrary field values and
//! are what make the escaping trustworthy. [`super::stamp::escape`] is the only
//! structure the fingerprint has, so the two properties here are exactly its
//! contract:
//!
//! * any difference in a recorded field produces a different text, and
//! * no recorded value can add a line to it.
//!
//! Both are necessary. The first alone would pass for an implementation that
//! escaped nothing, because the *raw* values still differ; the second alone
//! would pass for one that composed every value as a constant.

use proptest::prelude::*;

use super::{
    Preparation,
    stamp::{FINGERPRINT_EXACT, FINGERPRINT_PREFIXES, compose, is_fingerprint_variable},
};

/// A value for one build-relevant variable.
///
/// The alphabet carries the characters that could forge a field — the two line
/// breaks, `\`, and the `:` and `=` the fingerprint uses as separators — so a
/// generated case can land on the composition's own syntax rather than only on
/// well-behaved text. An empty value is included too: "unset" and "set to
/// nothing" reach the child differently for some variables, and the fingerprint
/// must not let them look the same.
fn fingerprint_value() -> impl Strategy<Value = String> {
    // Every arm yields `String`, including the regex one, so `prop_oneof!` sees
    // a single `Value` type. A `Just("\\")` arm would be an `&str` strategy and
    // the collection below would refuse to unify the two.
    let character = prop_oneof![
        Just(String::from("\\")),
        Just(String::from("\n")),
        Just(String::from("\r")),
        Just(String::from(":")),
        Just(String::from("=")),
        Just(String::from(" ")),
        "[a-z0-9]",
    ];
    proptest::collection::vec(character, 0..=8).prop_map(|parts| parts.concat())
}

/// A variable name the fingerprint is supposed to record.
///
/// Drawn from the module's own two lists so the property cannot drift into
/// asserting about a name the implementation is right to ignore.
fn fingerprint_name() -> impl Strategy<Value = String> {
    let exact = proptest::sample::select(FINGERPRINT_EXACT.as_slice());
    let prefixed = (
        proptest::sample::select(FINGERPRINT_PREFIXES.as_slice()),
        "[a-z_]{0,6}",
        "[A-Z_]{0,6}",
    )
        .prop_map(|(prefix, middle, end)| format!("{prefix}{middle}{end}"));
    prop_oneof![exact.prop_map(String::from), prefixed]
}

/// One variable and its value, in the shape the environment constructor takes.
fn variable(name: &str, value: &str) -> (std::ffi::OsString, std::ffi::OsString) {
    (
        std::ffi::OsString::from(name),
        std::ffi::OsString::from(value),
    )
}

/// A scratch preparation over a synthetic but absolute target directory.
///
/// The tree is never consulted here, so it need not exist. The path is derived
/// from [`std::env::temp_dir`] so it is absolute on Windows too, where a literal
/// such as `/tmp/target` would be relative and would then be resolved by the
/// environment constructor.
fn preparation_over(
    variables: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    variant: &str,
    inputs: &str,
) -> Preparation {
    let target = std::env::temp_dir().join("target");
    let environment = crate::nested_cargo::env_from_vars(
        variables,
        Some(&target),
        crate::nested_cargo::ProfileDestination::Caller,
    );
    Preparation::new(
        super::Sources::new("unit", variant, inputs),
        target,
        environment,
    )
}

proptest! {
    /// Any difference in a recorded field produces a different fingerprint.
    ///
    /// This is the stamp's whole contract: two builds that differ in anything
    /// the fingerprint records must not share a stamp, because sharing one means
    /// the second build's timed assertion runs against the first's artefacts.
    #[test]
    fn distinct_field_values_compose_distinct_fingerprints(
        name in fingerprint_name(),
        left in fingerprint_value(),
        right in fingerprint_value(),
    ) {
        prop_assume!(left != right);
        let first = preparation_over(vec![variable(&name, &left)], "v", "hash");
        let second = preparation_over(vec![variable(&name, &right)], "v", "hash");
        prop_assert_ne!(
            compose(&first, "release: 1.88.0 host: x"),
            compose(&second, "release: 1.88.0 host: x"),
            "{} = {:?} and {:?} must not share a fingerprint", name, left, right
        );
    }

    /// No recorded value can add a line to the fingerprint.
    ///
    /// The fingerprint is compared as text, so its only structural defence is
    /// that no value can start a new line: an unescaped `\n` in `RUSTFLAGS`
    /// can spell `inputs: x` on the next line, and that is a field the
    /// composition never wrote. Counting lines turns that into a property. A
    /// fixed preparation composes exactly one line per recorded field, so the
    /// count must not move however hostile the values are; an unescaped break
    /// anywhere adds one.
    ///
    /// The expected count is derived from the preparation's own environment
    /// rather than hard-coded, because the environment constructor adds its own
    /// resolved `CARGO_TARGET_DIR` and that variable is recorded too.
    #[test]
    fn no_recorded_value_can_add_a_line_to_the_fingerprint(
        name in fingerprint_name(),
        value in fingerprint_value(),
        toolchain in fingerprint_value(),
        variant in fingerprint_value(),
        inputs in fingerprint_value(),
    ) {
        let preparation = preparation_over(vec![variable(&name, &value)], &variant, &inputs);
        let recorded = preparation
            .environment
            .iter()
            .filter(|(key, _)| is_fingerprint_variable(&key.to_string_lossy()))
            .count();
        // version, toolchain, one per recorded variable, variant, inputs.
        let expected = 2 + recorded + 2;
        let text = compose(&preparation, &toolchain);
        prop_assert_eq!(
            text.as_str().lines().count(),
            expected,
            "{} = {:?} must stay on its own line: {:?}", name, value, text.as_str()
        );
    }
}
