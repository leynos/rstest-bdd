//! Property coverage for fallible workspace directory traversal.

use std::io;

use proptest::{prelude::*, test_runner::Config as ProptestConfig};

use super::{
    tests::{FailureSite, assert_io_error_kind, in_memory_workspace},
    *,
};

/// Generates non-optional I/O kinds that discovery must propagate.
fn required_error_kind_strategy() -> impl Strategy<Value = io::ErrorKind> {
    prop_oneof![
        Just(io::ErrorKind::PermissionDenied),
        Just(io::ErrorKind::Other),
    ]
}

/// Maps generated indices to the required, non-optional traversal operations.
fn required_failure_site(index: usize) -> FailureSite {
    match index {
        0 => FailureSite::WorkspaceReadDirectory,
        1 => FailureSite::WorkspaceDirectoryEntry,
        2 => FailureSite::CrateDirectoryMetadata,
        _ => FailureSite::CrateManifestMetadata,
    }
}

/// Maps generated indices to metadata probes where `NotFound` is optional.
fn optional_not_found_site(index: usize) -> FailureSite {
    match index {
        0 => FailureSite::OptionalTestsFeaturesMetadata,
        1 => FailureSite::OptionalWorkspaceFeaturesMetadata,
        _ => FailureSite::OptionalCrateManifestMetadata,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// Propagates each required directory operation's injected I/O failure.
    #[test]
    fn propagates_required_directory_operation_errors(
        nested_feature_depths in prop::collection::vec(1_u8..=3, 1..=3),
        entry_order in prop::collection::vec(any::<u8>(), 0..=8),
        failure_index in 0_usize..4,
        error_kind in required_error_kind_strategy(),
    ) {
        let workspace = in_memory_workspace(
            &nested_feature_depths,
            &entry_order,
            Some((required_failure_site(failure_index), error_kind)),
        );

        let error = find_feature_files_with(Path::new("workspace"), &workspace.reader)
            .expect_err("required directory-operation failures must be propagated");

        assert_io_error_kind(error, error_kind);
    }

    /// Ignores optional `NotFound` metadata probes while finding valid features.
    #[test]
    fn ignores_optional_not_found_metadata_probes(
        nested_feature_depths in prop::collection::vec(1_u8..=3, 1..=3),
        entry_order in prop::collection::vec(any::<u8>(), 0..=8),
        failure_index in 0_usize..3,
    ) {
        let workspace = in_memory_workspace(
            &nested_feature_depths,
            &entry_order,
            Some((optional_not_found_site(failure_index), io::ErrorKind::NotFound)),
        );

        let mut features = find_feature_files_with(Path::new("workspace"), &workspace.reader)
            .expect("optional missing paths must not prevent discovery");
        features.sort();

        prop_assert_eq!(features, workspace.expected_features);
    }
}
