//! Render findings into the text a user reads, independent of the channel.
//!
//! Wording lives here rather than beside the emitters so the stable
//! deprecated-item note and the nightly native diagnostic cannot drift, and so
//! wording can be snapshot-tested without a compiler session. Every message
//! begins with a stable code, and every remedy removes the finding it is
//! attached to (decision D5): a workaround such as adding a direct
//! `rstest-bdd-harness` dependency belongs in the guide, not in the message.

/// The text for a harness or attribute-policy path that fell back to the base
/// harness crate.
///
/// The attribute clause appears only when the canonical spelling would emit an
/// attribute the actual expansion does not, so the message never claims a
/// consequence that did not occur.
///
/// # Examples
///
/// ```rust,ignore
/// let message = unrecognized_adapter_path(
///     "TokioHarness",
///     "rstest_bdd_harness_tokio::TokioHarness",
///     &[],
/// );
/// assert!(message.starts_with("rstest-bdd[W001]: "));
/// ```
pub(crate) fn unrecognized_adapter_path(
    written_type: &str,
    canonical_path: &str,
    dropped: &[&str],
) -> String {
    let clause = not_applied(dropped);
    format!(
        concat!(
            "rstest-bdd[W001]: this path was not recognized as the first-party `{}`, so base ",
            "harness API types were used{}. Write `{}`."
        ),
        written_type, clause, canonical_path
    )
}

/// The text for an evidenced harness path whose attribute policy is missing.
///
/// The second remedy is honest about its limit: adding the policy restores the
/// attribute but leaves the harness path's own W001 in place (decision D11).
pub(crate) fn missing_attribute_policy(
    canonical_harness: &str,
    canonical_policy: &str,
    dropped: &[&str],
) -> String {
    let clause = not_applied(dropped);
    format!(
        concat!(
            "rstest-bdd[W002]: no attribute policy could be inferred from this harness path{}. ",
            "Write `{}`, or add `attributes = {}` (which restores the attribute but leaves ",
            "warning W001 for the harness path)."
        ),
        clause, canonical_harness, canonical_policy
    )
}

/// The " and X was not applied" clause, empty when no attribute was lost.
fn not_applied(dropped: &[&str]) -> String {
    match dropped {
        [] => String::new(),
        [only] => format!(" and {only} was not applied"),
        [first, rest @ ..] => {
            let others = rest.join(", ");
            format!(" and {first}, {others} were not applied")
        }
    }
}
