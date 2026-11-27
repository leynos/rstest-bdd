//! `LEM-EVIDENCE`: the classifier's contract over an unbounded path space.
//!
//! The properties are membership predicates without recursion or arithmetic,
//! so class-directed property tests are proportionate. Each property is stated
//! as a consequence of `Evidenced`, never as a second recognition
//! implementation: a differential check of two copies of the same rule would
//! prove nothing.

use proc_macro2::{Ident, Span};
use proptest::prelude::*;
use rstest::rstest;
use rstest_bdd_policy::TestAttributeHint;

use super::super::{
    CrateResolver,
    SlotClass,
    classify_harness_path,
    classify_policy_path,
    model::Framework,
};

/// A resolver with a chosen Cargo answer.
struct TestResolver {
    /// The root identifier Cargo resolves the adapter crate to, if any.
    root: Option<Ident>,
    /// Whether syntactic evidence may treat the crate as present.
    dependency: bool,
}

impl TestResolver {
    /// A resolver that reports the crate as a resolvable dependency.
    fn resolvable() -> Self {
        Self {
            root: None,
            dependency: true,
        }
    }

    /// A resolver whose adapter crate is not a dependency at all.
    fn unresolvable() -> Self {
        Self {
            root: None,
            dependency: false,
        }
    }

    /// A resolver for a renamed dependency.
    fn renamed(root: &str) -> Self {
        Self {
            root: Some(Ident::new(root, Span::call_site())),
            dependency: true,
        }
    }
}

impl CrateResolver for TestResolver {
    fn resolved_root(&self, _: &str) -> Option<Ident> { self.root.clone() }

    fn is_direct_dependency(&self, _: &str) -> bool { self.dependency }
}

/// The canonical adapter type names per framework.
fn adapter_type_name(framework: Framework) -> &'static str {
    match framework {
        Framework::Tokio => "TokioHarness",
        Framework::Gpui => "GpuiHarness",
    }
}

/// The canonical crate identifier per framework.
fn crate_identifier(framework: Framework) -> &'static str {
    match framework {
        Framework::Tokio => "rstest_bdd_harness_tokio",
        Framework::Gpui => "rstest_bdd_harness_gpui",
    }
}

/// A path segment drawn from the names classification cares about.
fn segment() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(crate_identifier(Framework::Tokio).to_owned()),
        Just(crate_identifier(Framework::Gpui).to_owned()),
        Just(adapter_type_name(Framework::Tokio).to_owned()),
        Just(adapter_type_name(Framework::Gpui).to_owned()),
        // Prefixed so a generated name can never be a Rust keyword, which
        // `syn` would reject before the classifier ever saw the path.
        "[a-z][a-z0-9_]{0,7}".prop_map(|name| format!("segment_{name}")),
    ]
}

/// Parse a rendered path, failing the case rather than panicking.
fn parse(text: &str) -> Result<syn::Path, TestCaseError> {
    syn::parse_str::<syn::Path>(text).map_err(|error| TestCaseError::fail(error.to_string()))
}

proptest! {
    #[test]
    fn classification_obeys_the_evidence_contract(
        segments in prop::collection::vec(segment(), 1..=6),
        dependency in any::<bool>(),
        renamed in any::<bool>(),
    ) {
        let text = segments.join("::");
        let path = parse(&text)?;
        let resolver = TestResolver {
            root: renamed.then(|| Ident::new("tok", Span::call_site())),
            dependency,
        };
        let class = classify_harness_path(&path, &resolver);
        let Some(last) = segments.last() else {
            return Err(TestCaseError::fail(
                "the strategy must yield at least one segment",
            ));
        };

        if let SlotClass::Evidenced(framework) = class {
            prop_assert_eq!(
                last.as_str(),
                adapter_type_name(framework),
                "an evidenced path must end in that framework's adapter type"
            );
            prop_assert!(dependency, "an unresolvable crate carries no evidence");
            let crate_name = crate_identifier(framework);
            let below_root = segments
                .iter()
                .skip(1)
                .any(|segment| segment.as_str() == crate_name || (renamed && segment == "tok"));
            prop_assert!(
                below_root,
                "the crate identifier must appear below the root: {}",
                text
            );
        }

        if !dependency {
            prop_assert!(
                !matches!(class, SlotClass::Evidenced(_)),
                "an unresolvable adapter crate never yields evidence: {}",
                text
            );
        }
        if !segments.iter().any(|segment| {
            segment.as_str() == crate_identifier(Framework::Tokio)
                || segment.as_str() == crate_identifier(Framework::Gpui)
                // A renamed dependency's own root identifier is evidence too.
                || (renamed && segment == "tok")
        }) {
            prop_assert!(
                !matches!(class, SlotClass::Evidenced(_)),
                "a path with no first-party crate identifier is never evidence: {}",
                text
            );
        }

        let qualified = parse(&format!("::{text}"))?;
        prop_assert_eq!(
            classify_harness_path(&qualified, &resolver),
            class,
            "a leading `::` must not change the classification"
        );
    }
}

#[rstest]
#[case::custom_type("custom::TokioHarness", SlotClass::Foreign)]
#[case::two_level_alias(
    "alias::rstest_bdd_harness_tokio::TokioHarness",
    SlotClass::Evidenced(Framework::Tokio)
)]
#[case::deep_nesting(
    "a::rstest_bdd_harness_gpui::b::GpuiHarness",
    SlotClass::Evidenced(Framework::Gpui)
)]
#[case::crate_prelude(
    "rstest_bdd_harness_tokio::prelude::TokioHarness",
    SlotClass::Recognized(TestAttributeHint::RstestWithTokioCurrentThread)
)]
#[case::facade_without_crate("facade::TokioHarness", SlotClass::Foreign)]
fn named_classifications(#[case] text: &str, #[case] expected: SlotClass) {
    let path = match syn::parse_str::<syn::Path>(text) {
        Ok(path) => path,
        Err(error) => panic!("{text} should parse: {error}"),
    };

    assert_eq!(
        classify_harness_path(&path, &TestResolver::resolvable()),
        expected,
        "{text} must classify as expected"
    );
}

#[test]
fn an_unresolvable_adapter_never_evidences() {
    for text in [
        "alias::rstest_bdd_harness_tokio::TokioHarness",
        "a::rstest_bdd_harness_gpui::b::GpuiHarness",
    ] {
        let path = match syn::parse_str::<syn::Path>(text) {
            Ok(path) => path,
            Err(error) => panic!("{text} should parse: {error}"),
        };

        assert!(
            !matches!(
                classify_harness_path(&path, &TestResolver::unresolvable()),
                SlotClass::Evidenced(_)
            ),
            "{text} must not evidence an unresolvable crate"
        );
    }
}

#[test]
fn a_renamed_dependency_is_recognized_through_its_root() {
    let path = match syn::parse_str::<syn::Path>("tok::TokioHarness") {
        Ok(path) => path,
        Err(error) => panic!("the path should parse: {error}"),
    };

    assert_eq!(
        classify_harness_path(&path, &TestResolver::renamed("tok")),
        SlotClass::Recognized(TestAttributeHint::RstestWithTokioCurrentThread),
        "a renamed dependency's own root must be recognized"
    );
}

#[test]
fn the_policy_slot_uses_the_policy_table() {
    let path = match syn::parse_str::<syn::Path>("rstest_bdd_harness::DefaultAttributePolicy") {
        Ok(path) => path,
        Err(error) => panic!("the path should parse: {error}"),
    };

    assert_eq!(
        classify_policy_path(&path, &TestResolver::resolvable()),
        SlotClass::Recognized(TestAttributeHint::RstestOnly),
        "the policy table recognizes the default policy"
    );
}
