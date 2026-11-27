//! The pure diagnostic kernel for harness and attribute-policy paths.
//!
//! This module holds no `syn` types and emits nothing. A boundary classifies
//! the paths it was given, calls [`diagnose`], and renders the resulting
//! [`PolicyFindings`]. Keeping the decision a plain function over a finite
//! record is what lets the whole state space be enumerated in tests instead of
//! sampled through `trybuild` fixtures.

use rstest_bdd_policy::TestAttributeHint;

/// A first-party framework whose adapter types can be spelled non-canonically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Framework {
    /// The Tokio harness adapter crate.
    Tokio,
    /// The GPUI harness adapter crate.
    Gpui,
}

/// How one supplied `harness` or `attributes` path was classified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlotClass {
    /// The slot was not supplied.
    Absent,
    /// The path is the canonical spelling and selects `hint`.
    Recognized(TestAttributeHint),
    /// The path carries first-party evidence but is not the canonical
    /// spelling, so the macros fell back to base harness API types.
    Evidenced(Framework),
    /// Anything else: a custom adapter, a renamed import that leaves no
    /// evidence, or a policy the macros cannot evaluate.
    Foreign,
}

/// The explicit user arguments at one expansion boundary.
///
/// The harness field is the *explicit* `harness = ...` argument, never the
/// path that the deprecated `runtime` alias generates, because supplying the
/// alias is not supplying a harness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PolicyInputs {
    /// Classification of the explicit `harness = ...` argument.
    pub(crate) harness: SlotClass,
    /// Classification of the explicit `attributes = ...` argument.
    pub(crate) attributes: SlotClass,
}

/// Every finding for one expansion boundary.
///
/// A fixed-shape record makes a duplicated finding unrepresentable, which is
/// how the one-warning-per-slot contract is enforced structurally rather than
/// by counting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PolicyFindings {
    /// W001: the harness path has first-party evidence but is not canonical.
    pub(crate) harness_unrecognized_path: bool,
    /// W001: the attributes path has first-party evidence but is not
    /// canonical.
    pub(crate) attributes_unrecognized_path: bool,
}

/// Decide every finding for one boundary.
///
/// Pure: it reads only the classified slots, so the same inputs always give
/// the same findings and no diagnostic can depend on emission order.
///
/// # Examples
///
/// ```rust,ignore
/// let inputs = PolicyInputs {
///     harness: SlotClass::Evidenced(Framework::Tokio),
///     attributes: SlotClass::Absent,
/// };
/// assert!(diagnose(inputs).harness_unrecognized_path);
/// ```
pub(crate) fn diagnose(inputs: PolicyInputs) -> PolicyFindings {
    PolicyFindings {
        harness_unrecognized_path: has_evidence(inputs.harness),
        attributes_unrecognized_path: has_evidence(inputs.attributes),
    }
}

/// Whether a slot carries first-party evidence without being canonical.
pub(crate) fn has_evidence(class: SlotClass) -> bool { matches!(class, SlotClass::Evidenced(_)) }
