//! Render findings into the text a user reads, independent of the channel.
//!
//! Wording lives here rather than beside the emitters so the stable
//! deprecated-item note and the nightly native diagnostic cannot drift, and so
//! wording can be snapshot-tested without a compiler session.

use super::{classify::adapter_spec, model::Framework};

/// The text for a harness or attribute-policy path that fell back to the base
/// harness crate.
///
/// This is the pre-existing ADR-0017 wording, kept byte-identical while the
/// module is extracted so the pinned `.stderr` snapshots stay valid. A later
/// milestone replaces it with the code-prefixed message.
pub(crate) fn unrecognized_adapter_path(framework: Framework) -> String {
    let spec = adapter_spec(framework);
    format!(
        concat!(
            concat!(
                "rstest-bdd could not identify this harness or attribute-policy path as a first-party ",
                "adapter; ",
            ),
            "falling back to `rstest-bdd-harness` for base harness API types. ",
            "Use the canonical crate-root path, ensure `{}` is directly resolvable as `{}`, ",
            "or add `rstest-bdd-harness` as a direct dev-dependency."
        ),
        spec.package_name, spec.default_crate_name
    )
}
