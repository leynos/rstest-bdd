//! The single implementation of ADR-0008 attribute-policy precedence.
//!
//! Code generation and diagnostics both call [`selected_hint`]. Two encodings
//! of the ladder could drift, and a diagnostic that described a different
//! precedence from the one the generated test actually used would be worse
//! than no diagnostic at all.

use rstest_bdd_policy::{RuntimeMode, TestAttributeHint};

use super::model::SlotClass;

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
