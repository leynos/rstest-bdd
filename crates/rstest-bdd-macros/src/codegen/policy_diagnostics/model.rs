//! The pure diagnostic kernel for harness and attribute-policy paths.
//!
//! This module holds no `syn` types and emits nothing. A boundary classifies
//! the paths it was given, calls [`diagnose`], and renders the resulting
//! [`PolicyFindings`]. Keeping the decision a plain function over a finite
//! record is what lets the whole state space be enumerated in tests instead of
//! sampled through `trybuild` fixtures.

use rstest_bdd_policy::{RuntimeMode, TestAttributeHint};

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

/// The explicit user arguments at one expansion boundary.
///
/// The harness field is the *explicit* `harness = ...` argument, never the
/// path that the deprecated `runtime` alias generates, because supplying the
/// alias is not supplying a harness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PolicyInputs {
    /// One `#[scenario]` expansion.
    Scenario {
        /// Classification of the explicit `harness = ...` argument.
        harness: SlotClass,
        /// Classification of the explicit `attributes = ...` argument.
        attributes: SlotClass,
        /// Whether the annotated function is `async`.
        is_async: bool,
        /// Whether the function already carries an executor attribute.
        has_user_test_attr: bool,
    },
    /// One `scenarios!` expansion.
    Scenarios {
        /// Classification of the explicit `harness = ...` argument.
        harness: SlotClass,
        /// Classification of the explicit `attributes = ...` argument.
        attributes: SlotClass,
        /// Whether the deprecated `runtime` alias was supplied.
        runtime_alias: bool,
    },
}

impl PolicyInputs {
    /// Which explicit path the harness slot holds.
    pub(crate) fn harness(self) -> SlotClass {
        match self {
            Self::Scenario { harness, .. } | Self::Scenarios { harness, .. } => harness,
        }
    }

    /// Which explicit path the attributes slot holds.
    pub(crate) fn attributes(self) -> SlotClass {
        match self {
            Self::Scenario { attributes, .. } | Self::Scenarios { attributes, .. } => attributes,
        }
    }

    /// The runtime the selected hint falls back to.
    pub(crate) fn attribute_runtime(self) -> RuntimeMode {
        if self.implies_tokio_runtime() {
            RuntimeMode::TokioCurrentThread
        } else {
            RuntimeMode::Sync
        }
    }

    /// Whether the caller asked for a Tokio runtime by any spelling.
    ///
    /// `#[scenario]` asks with an `async fn`; `scenarios!` asks with the
    /// deprecated alias. Reading the question once, rather than nesting a
    /// conditional inside each shape, keeps the two spellings visibly the same
    /// policy.
    fn implies_tokio_runtime(self) -> bool {
        match self {
            Self::Scenario { is_async, .. } => is_async,
            Self::Scenarios { runtime_alias, .. } => runtime_alias,
        }
    }

    /// Whether the generated test function is `async`.
    ///
    /// `scenarios!` reproduces `resolve_effective_runtime`: the alias forces a
    /// synchronous body unless an explicit harness is also present.
    pub(crate) fn executes_async(self) -> bool {
        match self {
            Self::Scenario { is_async, .. } => is_async,
            Self::Scenarios {
                harness,
                runtime_alias,
                ..
            } => runtime_alias && !matches!(harness, SlotClass::Absent),
        }
    }

    /// Whether a user-written attribute already satisfies `rstest`'s rule.
    ///
    /// `scenarios!` generates its functions, so a user cannot write one there.
    pub(crate) fn has_user_test_attr(self) -> bool {
        match self {
            Self::Scenario {
                has_user_test_attr, ..
            } => has_user_test_attr,
            Self::Scenarios { .. } => false,
        }
    }

    /// The same configuration with the harness slot replaced.
    pub(crate) fn with_harness(self, class: SlotClass) -> Self {
        match self {
            Self::Scenario {
                attributes,
                is_async,
                has_user_test_attr,
                ..
            } => Self::Scenario {
                harness: class,
                attributes,
                is_async,
                has_user_test_attr,
            },
            Self::Scenarios {
                attributes,
                runtime_alias,
                ..
            } => Self::Scenarios {
                harness: class,
                attributes,
                runtime_alias,
            },
        }
    }

    /// The same configuration with the attributes slot replaced.
    pub(crate) fn with_attributes(self, class: SlotClass) -> Self {
        match self {
            Self::Scenario {
                harness,
                is_async,
                has_user_test_attr,
                ..
            } => Self::Scenario {
                harness,
                attributes: class,
                is_async,
                has_user_test_attr,
            },
            Self::Scenarios {
                harness,
                runtime_alias,
                ..
            } => Self::Scenarios {
                harness,
                attributes: class,
                runtime_alias,
            },
        }
    }
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
    /// W002: the harness path has evidence and a policy that should have been
    /// inferred is missing.
    pub(crate) missing_attribute_policy: bool,
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
        harness_unrecognized_path: has_evidence(inputs.harness()),
        attributes_unrecognized_path: has_evidence(inputs.attributes()),
        missing_attribute_policy: has_missing_policy(inputs),
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
