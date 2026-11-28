//! The single implementation of ADR-0008 attribute-policy precedence.
//!
//! Code generation and diagnostics both call [`selected_hint`]. Two encodings
//! of the ladder could drift, and a diagnostic that described a different
//! precedence from the one the generated test actually used would be worse
//! than no diagnostic at all.

use rstest_bdd_policy::{RuntimeMode, TestAttributeHint};

use super::model::{PolicyInputs, SlotClass};

/// Select the test-attribute hint for one expansion.
///
/// The ladder is ADR-0008's, unchanged: an explicit `attributes` path wins,
/// then a recognized first-party `harness` path, then the runtime fallback. A
/// non-canonical or third-party `attributes` path selects nothing rather than
/// deferring to the harness, which is exactly what the code generator has
/// always done.
///
/// # Examples
///
/// ```rust,ignore
/// let hint = selected_hint(
///     SlotClass::Recognized(TestAttributeHint::RstestWithGpuiTest),
///     SlotClass::Absent,
///     RuntimeMode::Sync,
/// );
/// assert_eq!(hint, TestAttributeHint::RstestWithGpuiTest);
/// ```
pub(crate) fn selected_hint(
    harness: SlotClass,
    attributes: SlotClass,
    attribute_runtime: RuntimeMode,
) -> TestAttributeHint {
    match attributes {
        SlotClass::Recognized(hint) => hint,
        SlotClass::Evidenced(_) | SlotClass::Foreign => TestAttributeHint::RstestOnly,
        SlotClass::Absent => match harness {
            SlotClass::Recognized(hint) => hint,
            SlotClass::Evidenced(_) | SlotClass::Foreign | SlotClass::Absent => {
                attribute_runtime.test_attribute_hint()
            }
        },
    }
}

/// The framework test attributes one expansion emits.
///
/// This is what the generated function carries, not what the policy selects:
/// the two differ because a Tokio attribute is dropped for a synchronous
/// signature while a GPUI attribute is not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AttributeSet {
    /// Whether `#[tokio::test(flavor = "current_thread")]` is emitted.
    pub(crate) tokio: bool,
    /// Whether `#[gpui::test]` is emitted.
    pub(crate) gpui: bool,
}

impl AttributeSet {
    /// Whether this set adds an attribute without losing one.
    ///
    /// Strictness matters: a configuration that *replaces* one framework
    /// attribute with another has not lost a policy, so it must not be
    /// reported as a missing one.
    pub(crate) fn is_strict_superset_of(self, other: Self) -> bool {
        self.covers(other) && (self.tokio != other.tokio || self.gpui != other.gpui)
    }

    /// The attribute names this set holds and `other` does not.
    pub(crate) fn difference(self, other: Self) -> Vec<&'static str> {
        let mut dropped = Vec::new();
        if self.tokio && !other.tokio {
            dropped.push("`#[tokio::test]`");
        }
        if self.gpui && !other.gpui {
            dropped.push("`#[gpui::test]`");
        }
        dropped
    }

    /// Whether every attribute in `other` is also in this set.
    fn covers(self, other: Self) -> bool {
        (self.tokio || !other.tokio) && (self.gpui || !other.gpui)
    }
}

/// The attributes one expansion emits.
pub(crate) fn emitted_attributes(inputs: PolicyInputs) -> AttributeSet {
    let hint = selected_hint(
        inputs.harness(),
        inputs.attributes(),
        inputs.attribute_runtime(),
    );
    match hint {
        TestAttributeHint::RstestWithTokioCurrentThread => AttributeSet {
            tokio: inputs.executes_async(),
            gpui: false,
        },
        TestAttributeHint::RstestWithGpuiTest => AttributeSet {
            tokio: false,
            gpui: true,
        },
        TestAttributeHint::RstestOnly => AttributeSet::default(),
    }
}
