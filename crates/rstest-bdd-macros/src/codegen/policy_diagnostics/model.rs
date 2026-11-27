//! The pure diagnostic kernel for harness and attribute-policy paths.
//!
//! This module holds no `syn` types and emits nothing. A boundary classifies
//! the paths it was given, calls [`diagnose`], and renders the resulting
//! [`PolicyFindings`]. Keeping the decision a plain function over a finite
//! record is what lets the whole state space be enumerated in tests instead of
//! sampled through `trybuild` fixtures.

use rstest_bdd_policy::TestAttributeHint;

use super::resolution::emitted_attributes;

/// A first-party framework whose adapter types can be spelled non-canonically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Framework {
    /// The Tokio harness adapter crate.
    Tokio,
    /// The GPUI harness adapter crate.
    Gpui,
}

impl Framework {
    /// The framework's name as a message spells it.
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::Tokio => "Tokio",
            Self::Gpui => "GPUI",
        }
    }

    /// The attribute-policy type the first-party crate exports.
    pub(crate) fn attribute_policy_type(self) -> &'static str {
        match self {
            Self::Tokio => "TokioAttributePolicy",
            Self::Gpui => "GpuiAttributePolicy",
        }
    }
}

impl From<Framework> for TestAttributeHint {
    fn from(framework: Framework) -> Self {
        match framework {
            Framework::Tokio => Self::RstestWithTokioCurrentThread,
            Framework::Gpui => Self::RstestWithGpuiTest,
        }
    }
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

pub(crate) use super::inputs::PolicyInputs;

/// Two selectors that name different frameworks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameworkConflict {
    /// The framework the harness slot implies.
    pub(crate) harness: Framework,
    /// The framework the attributes slot selects.
    pub(crate) attributes: Framework,
}

/// Every finding for one expansion boundary.
///
/// A fixed-shape record makes a duplicated finding unrepresentable, which is
/// how the one-warning-per-slot contract is enforced structurally rather than
/// by counting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PolicyFindings {
    /// W001, per slot.
    pub(crate) slots: SlotWarnings,
    /// W002: the harness path has evidence and a policy that should have been
    /// inferred is missing.
    pub(crate) missing_attribute_policy: bool,
    /// W003: the two selectors name different frameworks.
    pub(crate) conflicting_frameworks: Option<FrameworkConflict>,
    /// E002: `scenarios!` combines the deprecated alias with a harness.
    pub(crate) runtime_alias_with_harness: bool,
    /// E001: an `async fn` scenario has no executor anywhere.
    pub(crate) missing_executor: bool,
}

/// The per-slot W001 findings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SlotWarnings {
    /// W001: the harness path has first-party evidence but is not canonical.
    pub(crate) harness: bool,
    /// W001: the attributes path has first-party evidence but is not
    /// canonical.
    pub(crate) attributes: bool,
}

/// Decide every finding for one boundary.
///
/// Pure: it reads only the classified slots and the shape flags, so the same
/// inputs always give the same findings and no diagnostic can depend on
/// emission order.
///
/// # Examples
///
/// ```rust,ignore
/// let inputs = PolicyInputs::Scenario {
///     harness: SlotClass::Evidenced(Framework::Tokio),
///     attributes: SlotClass::Absent,
///     is_async: false,
///     has_user_test_attr: false,
/// };
/// assert!(diagnose(inputs).harness_unrecognized_path);
/// ```
pub(crate) fn diagnose(inputs: PolicyInputs) -> PolicyFindings {
    PolicyFindings {
        slots: SlotWarnings {
            harness: has_evidence(inputs.harness()),
            attributes: has_evidence(inputs.attributes()),
        },
        missing_attribute_policy: has_missing_policy(inputs),
        conflicting_frameworks: framework_conflict(inputs),
        runtime_alias_with_harness: runtime_alias_conflict(inputs),
        missing_executor: has_missing_executor(inputs),
    }
}

/// E001: an `async fn` scenario whose expansion carries no executor.
///
/// `rstest` rejects such a function itself, so this error replaces one that
/// already fires; it only names the cause and the fixes. The rule mirrors
/// `rstest`'s: an explicit `test_attr`, or any attribute whose last segment is
/// `test`, satisfies it, and a user-written one is counted separately.
fn has_missing_executor(inputs: PolicyInputs) -> bool {
    let PolicyInputs::Scenario {
        is_async: true,
        has_user_test_attr: false,
        ..
    } = inputs
    else {
        return false;
    };
    if !matches!(inputs.harness(), SlotClass::Absent)
        || matches!(inputs.attributes(), SlotClass::Absent)
    {
        return false;
    }
    let emitted = emitted_attributes(inputs);
    !emitted.tokio && !emitted.gpui
}

/// The framework a class contributes to a conflict, if it contributes one.
///
/// `Recognized(RstestOnly)` is a deliberate downgrade rather than a framework,
/// so it never conflicts with anything.
pub(crate) fn framework_of(class: SlotClass) -> Option<Framework> {
    match class {
        SlotClass::Recognized(TestAttributeHint::RstestWithTokioCurrentThread) => {
            Some(Framework::Tokio)
        }
        SlotClass::Recognized(TestAttributeHint::RstestWithGpuiTest) => Some(Framework::Gpui),
        SlotClass::Evidenced(framework) => Some(framework),
        SlotClass::Recognized(TestAttributeHint::RstestOnly)
        | SlotClass::Absent
        | SlotClass::Foreign => None,
    }
}

/// The framework the harness selection implies, including the alias.
///
/// The alias is only an implied harness when no explicit harness was given;
/// with one, E002 reports the combination instead.
pub(crate) fn implied_harness_family(inputs: PolicyInputs) -> Option<Framework> {
    if let Some(family) = framework_of(inputs.harness()) {
        return Some(family);
    }
    match inputs {
        PolicyInputs::Scenarios {
            harness: SlotClass::Absent,
            runtime_alias: true,
            ..
        } => Some(Framework::Tokio),
        _ => None,
    }
}

/// W003: two selectors that name different frameworks.
fn framework_conflict(inputs: PolicyInputs) -> Option<FrameworkConflict> {
    let harness = implied_harness_family(inputs)?;
    let attributes = framework_of(inputs.attributes())?;
    (harness != attributes).then_some(FrameworkConflict {
        harness,
        attributes,
    })
}

/// E002: the deprecated alias cannot be combined with an explicit harness.
fn runtime_alias_conflict(inputs: PolicyInputs) -> bool {
    match inputs {
        PolicyInputs::Scenarios { runtime_alias, .. } => {
            runtime_alias && !matches!(inputs.harness(), SlotClass::Absent)
        }
        PolicyInputs::Scenario { .. } => false,
    }
}

/// Whether a slot carries first-party evidence without being canonical.
pub(crate) fn has_evidence(class: SlotClass) -> bool { matches!(class, SlotClass::Evidenced(_)) }

/// W002: evidence plus no policy, where the canonical spelling would emit an
/// attribute the actual expansion does not.
///
/// A user-written executor attribute suppresses the finding: the generator
/// deduplicates against it, so the attribute is present either way and the
/// warning's claim would be false (decision D31).
fn has_missing_policy(inputs: PolicyInputs) -> bool {
    let SlotClass::Evidenced(framework) = inputs.harness() else {
        return false;
    };
    if !matches!(inputs.attributes(), SlotClass::Absent) || inputs.has_user_test_attr() {
        return false;
    }
    let canonical = inputs.with_harness(SlotClass::Recognized(framework.into()));
    emitted_attributes(canonical).is_strict_superset_of(emitted_attributes(inputs))
}
