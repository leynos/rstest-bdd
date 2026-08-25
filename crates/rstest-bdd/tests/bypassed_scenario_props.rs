//! Property tests for bypassed-scenario descriptors.
//!
//! `BypassedScenario` borrows generated scenario metadata, so these tests vary
//! every descriptor field and verify that its value-preserving builder methods
//! neither alter unrelated metadata nor share updates with prior descriptors.

use proptest::prelude::*;
use rstest_bdd::BypassedScenario;

/// Strategy for short scenario metadata segments that remain easy to diagnose.
fn metadata_segment() -> impl Strategy<Value = String> { "[a-z][a-z0-9_]{0,15}" }

proptest! {
    /// The base descriptor retains its identity and uses empty optional metadata.
    #[test]
    fn new_preserves_identity_and_initializes_defaults(
        feature_path in metadata_segment(),
        scenario_name in metadata_segment(),
        scenario_line in any::<u32>(),
    ) {
        let scenario = BypassedScenario::new(&feature_path, &scenario_name, scenario_line);

        prop_assert_eq!(scenario.feature_path, feature_path.as_str());
        prop_assert_eq!(scenario.scenario_name, scenario_name.as_str());
        prop_assert_eq!(scenario.scenario_line, scenario_line);
        prop_assert!(scenario.tags.is_empty());
        prop_assert_eq!(scenario.reason, None);
    }

    /// Fluent updates modify only their own field and chain without losing identity.
    #[test]
    fn fluent_updates_are_independent_and_chain_without_losing_metadata(
        feature_path in metadata_segment(),
        scenario_name in metadata_segment(),
        scenario_line in any::<u32>(),
        tags in prop::collection::vec(metadata_segment(), 0..8),
        reason in prop::option::of(metadata_segment()),
    ) {
        let base = BypassedScenario::new(&feature_path, &scenario_name, scenario_line);
        let with_tags = base.with_tags(&tags);
        let with_reason = base.with_reason(reason.as_deref());
        let chained = base.with_tags(&tags).with_reason(reason.as_deref());

        prop_assert!(base.tags.is_empty());
        prop_assert_eq!(base.reason, None);
        prop_assert_eq!(with_tags.tags, tags.as_slice());
        prop_assert_eq!(with_tags.reason, None);
        prop_assert!(with_reason.tags.is_empty());
        prop_assert_eq!(with_reason.reason, reason.as_deref());
        prop_assert_eq!(chained.feature_path, feature_path.as_str());
        prop_assert_eq!(chained.scenario_name, scenario_name.as_str());
        prop_assert_eq!(chained.scenario_line, scenario_line);
        prop_assert_eq!(chained.tags, tags.as_slice());
        prop_assert_eq!(chained.reason, reason.as_deref());
    }
}
