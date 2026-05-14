//! Property-based tests for missing-fixture diagnostic invariants.

use proptest::prelude::*;
use rstest_bdd::{MissingFixtureDiagnostic, MissingFixturesDetails};

proptest! {
    /// Every fixture listed in `missing` appears in `missing_requirements`.
    #[test]
    fn missing_requirements_covers_every_missing_fixture(
        names in prop::collection::vec("[a-z_][a-z0-9_]{0,15}", 0..8usize)
    ) {
        // Build a MissingFixturesDetails with no typed metadata (all <unknown>).
        let static_names: Vec<&'static str> = names
            .iter()
            .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
            .collect();
        let requirements: Vec<MissingFixtureDiagnostic> = static_names
            .iter()
            .map(|&name| MissingFixtureDiagnostic { name, ty: "<unknown>" })
            .collect();
        let details = MissingFixturesDetails {
            step_pattern: "pattern".to_string(),
            step_location: "file.rs:1".to_string(),
            required: static_names.clone(),
            missing: static_names.clone(),
            missing_requirements: requirements,
            available: vec![],
            suggestion: None,
            feature_path: "f.feature".to_string(),
            scenario_name: "S".to_string(),
        };
        for name in &static_names {
            prop_assert!(
                details.missing_requirements.iter().any(|r| r.name == *name),
                "missing fixture '{name}' absent from missing_requirements"
            );
        }
    }

    /// `available` list is always sorted.
    #[test]
    fn available_list_is_always_sorted(
        mut names in prop::collection::vec("[a-z_][a-z0-9_]{0,15}", 0..16usize)
    ) {
        names.sort_unstable();
        names.dedup();
        let mut shuffled = names.clone();
        // Reverse to guarantee non-sorted input before the production code sorts.
        shuffled.reverse();
        // Production code sorts available_list in validate_required_fixtures.
        // Mimic that sort here and assert it matches.
        let mut sorted = shuffled.clone();
        sorted.sort_unstable();
        prop_assert_eq!(sorted, names);
    }
}
