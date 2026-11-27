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

/// The text for two selectors that name different frameworks.
///
/// When the harness slot is non-canonical, removing `attributes` alone would
/// land in W002's antecedent, so the removal remedy says to apply the harness
/// fix as well (decision D30).
pub(crate) fn conflicting_selectors(
    harness: &str,
    attributes: &str,
    attributes_recognized: bool,
    matching_policy: &str,
    harness_needs_fixing: bool,
) -> String {
    let removal = if harness_needs_fixing {
        "Remove `attributes = ...` and apply the harness-path fix"
    } else {
        "Remove `attributes = ...`"
    };
    let selection = if attributes_recognized {
        format!("`attributes` selects {attributes}")
    } else {
        // A non-canonical policy is not recognized, so it overrides the harness
        // without adding its framework's attribute.
        format!("`attributes` names {attributes} but is not recognized")
    };
    format!(
        concat!(
            "rstest-bdd[W003]: `harness` selects {}, but {}; the explicit `attributes` takes ",
            "precedence. {}, or write `attributes = {}`."
        ),
        harness, selection, removal, matching_policy
    )
}

/// The text for the deprecated alias combined with an explicit harness.
///
/// A non-canonical harness also needs the canonical path here: errors suppress
/// warnings (decision D9), so the harness slot's own fix is never shown, and
/// clearing the alias alone would reveal a missing-policy warning (D41).
pub(crate) fn runtime_alias_with_harness(canonical_harness: Option<&str>) -> String {
    let harness_fix = canonical_harness.map_or_else(String::new, |path| {
        format!(", and write `harness = {path}`")
    });
    format!(
        concat!(
            "rstest-bdd[E002]: `runtime = \"tokio-current-thread\"` cannot be combined with ",
            "`harness = ...`; the harness already selects the runtime. Remove `runtime = ...`{}."
        ),
        harness_fix
    )
}

/// The text for an `async fn` scenario that has no executor.
///
/// The last sentence appears only for a third-party policy: the macros cannot
/// read its `test_attributes()` implementation, which is why it adds no
/// executor (ADR-0008).
pub(crate) fn missing_executor(tokio_policy: &str, third_party: bool) -> String {
    let limitation = if third_party {
        " rstest-bdd cannot read third-party attribute policies."
    } else {
        ""
    };
    format!(
        concat!(
            "rstest-bdd[E001]: this `async fn` scenario has no executor: `attributes = ...` adds ",
            "none, and no `harness` provides one. Remove `attributes = ...` (async scenarios ",
            "default to Tokio), write `attributes = {}`, or add an executor attribute such as ",
            "`#[tokio::test]`.{}"
        ),
        tokio_policy, limitation
    )
}
