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
    stamp::{
        FINGERPRINT_EXACT,
        FINGERPRINT_PREFIXES,
        OUTPUTS_SEPARATOR,
        compose,
        is_fingerprint_variable,
        parse,
        render,
    },
};

/// A value for one build-relevant variable.
///
/// The alphabet carries the characters that could forge a field — the two line
/// breaks, `\`, and the `:` and `=` the fingerprint uses as separators — so a
/// generated case can land on the composition's own syntax rather than only on
/// well-behaved text. An empty value is included too: "unset" and "set to
/// nothing" reach the child differently for some variables, and the fingerprint
/// must not let them look the same.
///
/// Space is deliberately over-represented, and trailing whitespace in
/// particular is generated directly. A uniformly drawn value almost never ends
/// in a space, and a trailing space is *exactly* the character a parser that
/// normalizes with `trim_end` silently discards. With an even spread, a
/// mutation making the parser lenient survived every property in this module,
/// because no generated case ever had one.
fn fingerprint_value() -> impl Strategy<Value = String> {
    // Every arm yields `String`, including the regex one, so `prop_oneof!` sees
    // a single `Value` type. A `Just("\\")` arm would be an `&str` strategy and
    // the collection below would refuse to unify the two.
    let character = prop_oneof![
        4 => Just(String::from(" ")),
        1 => Just(String::from("\\")),
        1 => Just(String::from("\n")),
        1 => Just(String::from("\r")),
        1 => Just(String::from(":")),
        1 => Just(String::from("=")),
        // The stamp's own delimiter, reachable because `render` writes the
        // fingerprint verbatim ahead of the separator line. A parser that
        // split on the separator before matching the fingerprint found this
        // spelling inside a value and refused the stamp it had just written.
        1 => Just(String::from(OUTPUTS_SEPARATOR)),
        4 => "[a-z0-9]",
    ];
    // The suffix is drawn *with* the body rather than by mapping the same
    // strategy three times, which `prop_map` cannot do: it takes the strategy
    // by value. A trailing space or tab is not decoration. Whitespace a caller
    // can actually leave at the end of a value — an `inputs` hash built by
    // joining fields is the ordinary way it happens — is the only input that
    // distinguishes "strip the terminating line break" from "strip all
    // trailing whitespace", and that distinction is the contract of the round
    // trip below.
    (
        proptest::collection::vec(character, 0..=8).prop_map(|parts| parts.concat()),
        prop_oneof![
            Just(String::new()),
            Just(String::from(" ")),
            Just(String::from("\t")),
            Just(String::from("  ")),
        ],
    )
        .prop_map(|(value, suffix)| format!("{value}{suffix}"))
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
    )
    .expect("the fixture environment should resolve");
    Preparation::new(
        super::Sources::new("unit", variant, inputs),
        target,
        environment,
    )
    .expect("the fixture preparation should describe its artefacts")
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

    /// A written stamp admits exactly the fingerprint it was written for.
    ///
    /// This is the round trip the other two properties do not reach. They
    /// compare freshly composed texts, while a gate compares a composed text
    /// against one read back out of a file, and the two paths are different
    /// code: `render` writes the separator, `parse` splits on it. A parser that
    /// normalized the text on the way back — `trim_end`, for instance — would
    /// make a stamp written for `inputs: h` admit the build whose inputs are
    /// `h `, and both properties above would still pass because both sides of
    /// their comparison are unnormalized.
    #[test]
    fn a_written_fingerprint_reads_back_as_itself(
        name in fingerprint_name(),
        value in fingerprint_value(),
        toolchain in fingerprint_value(),
        variant in fingerprint_value(),
        inputs in fingerprint_value(),
    ) {
        let preparation = preparation_over(vec![variable(&name, &value)], &variant, &inputs);
        let fingerprint = compose(&preparation, &toolchain);
        let written = render(&fingerprint, &[]);
        prop_assert!(
            parse(written.as_str(), &fingerprint).is_some(),
            "a stamp written for a fingerprint must admit it: {:?}", written.as_str()
        );
    }

    /// Two fingerprints that differ differ after a write and a read back.
    ///
    /// The complement of the round trip, and the reason the parser must not be
    /// lenient: a *different* fingerprint must not be admitted by the stamp
    /// written for this one. Together the two properties say the gate compares
    /// the texts `compose` produced, not some normalization of them.
    ///
    /// The two sides differ in `inputs` rather than in an environment variable
    /// on purpose. `inputs` is the *last* field `compose` writes, so it is the
    /// only one whose trailing characters sit against the separator: a parser
    /// that made the newline before the separator optional accepted a stamp
    /// written for `h@@ recorded-artefacts` as one for `h`. Varying an
    /// environment variable hides that, because the field under test is then
    /// followed by `variant:` and `inputs:` rather than by the delimiter.
    ///
    /// The second value is *derived* from the first rather than drawn beside
    /// it. Two independent draws are almost never related as a value and one
    /// of its proper prefixes, so an independently drawn pair cannot reach the
    /// case at all — measured, not assumed: with both drawn, this property
    /// still passed under the optional-newline mutation. Appending to a drawn
    /// value makes the relationship certain, and the two alternatives below
    /// cover both ways the last field can run into the delimiter.
    #[test]
    fn a_written_fingerprint_rejects_a_different_one(
        name in fingerprint_name(),
        value in fingerprint_value(),
        inputs in fingerprint_value(),
        toolchain in fingerprint_value(),
        append in prop_oneof![
            Just(String::from(OUTPUTS_SEPARATOR)),
            Just(String::from(" ")),
        ],
    ) {
        let longer = format!("{inputs}{append}");
        prop_assume!(inputs != longer);
        // The stamp is written for the *longer* value and offered the shorter
        // one. Both directions matter, and this is the one that fails: after
        // consuming `inputs`, only the longer text still has the delimiter's
        // spelling inside the fingerprint, so an optional newline lets the
        // longer value's own text supply what the parser is looking for next.
        let first = preparation_over(vec![variable(&name, &value)], "v", &longer);
        let second = preparation_over(vec![variable(&name, &value)], "v", &inputs);
        let fingerprint = compose(&first, &toolchain);
        let other = compose(&second, &toolchain);
        prop_assume!(fingerprint != other);
        let written = render(&fingerprint, &[]);
        prop_assert!(
            parse(written.as_str(), &other).is_none(),
            "a stamp written for {longer:?} must not admit {inputs:?}: {:?}",
            written.as_str()
        );
        prop_assert!(
            parse(written.as_str(), &fingerprint).is_some(),
            "the same stamp must still admit the fingerprint it recorded: {:?}",
            written.as_str()
        );
    }

    /// A difference in the variant, the inputs or the toolchain is a
    /// difference in the fingerprint.
    ///
    /// The environment variable case above only ever varies one recorded
    /// variable. These three fields reach the composition through separate
    /// lines, so a composition that dropped or reordered one of them would
    /// stay invisible to a property that only ever moved a variable's value.
    #[test]
    fn distinct_variant_inputs_or_toolchain_compose_distinct_fingerprints(
        variant in fingerprint_value(),
        inputs in fingerprint_value(),
        toolchain in fingerprint_value(),
        other_variant in fingerprint_value(),
        other_inputs in fingerprint_value(),
        other_toolchain in fingerprint_value(),
    ) {
        // One alternative value per field, each assumed different from *its
        // own* field. A single `other` reused for both the variant and the
        // inputs would let an equal pair reach an assertion, and the case that
        // then fails is the property comparing one preparation with itself —
        // a failure in the test, not in the code.
        prop_assume!(other_variant != variant);
        prop_assume!(other_inputs != inputs);
        prop_assume!(other_toolchain != toolchain);

        let base = preparation_over(Vec::new(), &variant, &inputs);
        let changed_variant = preparation_over(Vec::new(), &other_variant, &inputs);
        let changed_inputs = preparation_over(Vec::new(), &variant, &other_inputs);
        prop_assert_ne!(compose(&base, &toolchain), compose(&changed_variant, &toolchain));
        prop_assert_ne!(compose(&base, &toolchain), compose(&changed_inputs, &toolchain));
        prop_assert_ne!(compose(&base, &toolchain), compose(&base, &other_toolchain));
    }
}
