//! `LEM-FIX-CONVERGES`: a suggested fix never introduces a new finding.
//!
//! Every remedy a message offers is modelled as a transformation on states.
//! Working this lemma by hand falsified four design candidates while the plan
//! was drafted, so it constrains the design rather than restating it; two of
//! the survivors (W002's second fix leaving W001 in place, and D31's
//! user-attribute suppression) are only correct because the finding set keeps
//! the underlying W001.

use super::{
    super::model::{
        Framework,
        PolicyFindings,
        PolicyInputs,
        SlotClass,
        diagnose,
        framework_of,
        implied_harness_family,
    },
    enumerator::{all_inputs, class_name},
};

/// One finding kind the kernel can report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Finding {
    /// W001 on the harness slot.
    HarnessPath,
    /// W001 on the attributes slot.
    AttributesPath,
    /// W002, the missing attribute policy.
    MissingPolicy,
    /// W003, two selectors naming different frameworks.
    Conflict,
    /// E002, the deprecated alias combined with a harness.
    AliasWithHarness,
    /// E001, an async scenario with no executor.
    MissingExecutor,
}

impl Finding {
    /// Every finding kind, in a stable order.
    const ALL: [Self; 6] = [
        Self::HarnessPath,
        Self::AttributesPath,
        Self::MissingPolicy,
        Self::Conflict,
        Self::AliasWithHarness,
        Self::MissingExecutor,
    ];

    /// Whether this finding holds in `findings`.
    fn holds(self, findings: PolicyFindings) -> bool {
        match self {
            Self::HarnessPath => findings.slots.harness,
            Self::AttributesPath => findings.slots.attributes,
            Self::MissingPolicy => findings.missing_attribute_policy,
            Self::Conflict => findings.conflicting_frameworks.is_some(),
            Self::AliasWithHarness => findings.runtime_alias_with_harness,
            Self::MissingExecutor => findings.missing_executor,
        }
    }

    /// This finding's bit in a finding mask.
    fn bit(self) -> u8 {
        match self {
            Self::HarnessPath => 0b001,
            Self::AttributesPath => 0b010,
            Self::MissingPolicy => 0b100,
            Self::Conflict => 0b1000,
            Self::AliasWithHarness => 0b1_0000,
            Self::MissingExecutor => 0b10_0000,
        }
    }
}

/// The findings as a bit mask.
fn mask(findings: PolicyFindings) -> u8 {
    Finding::ALL
        .into_iter()
        .filter(|finding| finding.holds(findings))
        .fold(0, |mask, finding| mask | finding.bit())
}

/// The states the messages offer as fixes for one finding.
fn fixes(finding: Finding, inputs: PolicyInputs) -> Vec<PolicyInputs> {
    match finding {
        Finding::HarnessPath => canonicalize(inputs, Slot::Harness).into_iter().collect(),
        Finding::AttributesPath => canonicalize(inputs, Slot::Attributes).into_iter().collect(),
        Finding::MissingPolicy => missing_policy_fixes(inputs),
        Finding::Conflict => conflict_fixes(inputs),
        Finding::AliasWithHarness => alias_fixes(inputs),
        Finding::MissingExecutor => executor_fixes(inputs),
    }
}

/// Which slot a fix canonicalizes.
#[derive(Clone, Copy)]
enum Slot {
    /// The harness slot.
    Harness,
    /// The attributes slot.
    Attributes,
}

/// The state with one evidenced slot replaced by its canonical spelling.
fn canonicalize(inputs: PolicyInputs, slot: Slot) -> Option<PolicyInputs> {
    match slot {
        Slot::Harness => framework_of(inputs.harness())
            .map(|family| inputs.with_harness(SlotClass::Recognized(family.into()))),
        Slot::Attributes => framework_of(inputs.attributes())
            .map(|family| inputs.with_attributes(SlotClass::Recognized(family.into()))),
    }
}

/// W002's two fixes: the canonical harness, or the matching policy.
fn missing_policy_fixes(inputs: PolicyInputs) -> Vec<PolicyInputs> {
    let SlotClass::Evidenced(family) = inputs.harness() else {
        return Vec::new();
    };
    vec![
        inputs.with_harness(SlotClass::Recognized(family.into())),
        inputs.with_attributes(SlotClass::Recognized(family.into())),
    ]
}

/// W003's two fixes: the matching policy, or removing the override.
fn conflict_fixes(inputs: PolicyInputs) -> Vec<PolicyInputs> {
    let Some(harness) = implied_harness_family(inputs) else {
        return Vec::new();
    };
    let matching = inputs.with_attributes(SlotClass::Recognized(harness.into()));
    vec![remove_attributes(inputs), matching]
}

/// Remove the override, canonicalizing an evidenced harness as well.
///
/// Removing `attributes` alone would land in W002's antecedent, so the remedy
/// is the union of the two fixes the user is already being told to make
/// (decision D30).
fn remove_attributes(inputs: PolicyInputs) -> PolicyInputs {
    let removed = inputs.with_attributes(SlotClass::Absent);
    match removed.harness() {
        SlotClass::Evidenced(family) => removed.with_harness(SlotClass::Recognized(family.into())),
        SlotClass::Absent | SlotClass::Recognized(_) | SlotClass::Foreign => removed,
    }
}

/// E002's fix: clear the alias, canonicalizing an evidenced harness as well
/// (decision D41).
fn alias_fixes(inputs: PolicyInputs) -> Vec<PolicyInputs> {
    let cleared = inputs.without_runtime_alias();
    match cleared.harness() {
        SlotClass::Evidenced(family) => {
            vec![cleared.with_harness(SlotClass::Recognized(family.into()))]
        }
        SlotClass::Absent | SlotClass::Recognized(_) | SlotClass::Foreign => vec![cleared],
    }
}

/// E001's three fixes: drop the policy, adopt the Tokio policy, or write an
/// executor attribute.
fn executor_fixes(inputs: PolicyInputs) -> Vec<PolicyInputs> {
    vec![
        inputs.with_attributes(SlotClass::Absent),
        inputs.with_attributes(SlotClass::Recognized(Framework::Tokio.into())),
        inputs.with_user_test_attr(true),
    ]
}

/// How many `(state, finding, fix)` triples each finding kind has checked.
#[derive(Default)]
struct Tally {
    /// W001 on the harness slot.
    harness: usize,
    /// W001 on the attributes slot.
    attributes: usize,
    /// W002.
    policy: usize,
    /// W003.
    conflict: usize,
    /// E002.
    alias: usize,
    /// E001.
    executor: usize,
}

impl Tally {
    /// Count one checked fix for `finding`.
    fn record(&mut self, finding: Finding) { *self.slot(finding) += 1 }

    /// The counter belonging to `finding`.
    fn slot(&mut self, finding: Finding) -> &mut usize {
        match finding {
            Finding::HarnessPath => &mut self.harness,
            Finding::AttributesPath => &mut self.attributes,
            Finding::MissingPolicy => &mut self.policy,
            Finding::Conflict => &mut self.conflict,
            Finding::AliasWithHarness => &mut self.alias,
            Finding::MissingExecutor => &mut self.executor,
        }
    }

    /// Fail when a finding kind contributed nothing, which would make the
    /// lemma vacuous for it.
    fn assert_not_vacuous(&self) {
        assert!(
            self.harness > 0
                && self.attributes > 0
                && self.policy > 0
                && self.conflict > 0
                && self.alias > 0
                && self.executor > 0,
            "every finding kind must contribute checked fixes, got {}, {}, {}, {}, {}, {}",
            self.harness,
            self.attributes,
            self.policy,
            self.conflict,
            self.alias,
            self.executor
        );
    }
}

/// Check every fix one state's findings offer.
fn check_state(inputs: PolicyInputs, tally: &mut Tally) {
    let before = diagnose(inputs);
    for finding in Finding::ALL {
        if !finding.holds(before) {
            continue;
        }
        let offered = fixes(finding, inputs);
        assert!(
            !offered.is_empty(),
            "a reported {finding:?} must offer at least one fix"
        );
        for fixed in offered {
            tally.record(finding);
            assert_fix_converges(finding, inputs, before, fixed);
        }
    }
}

/// Assert one fix clears its own finding and introduces no other.
fn assert_fix_converges(
    finding: Finding,
    inputs: PolicyInputs,
    before: PolicyFindings,
    fixed: PolicyInputs,
) {
    let after = diagnose(fixed);
    assert!(
        !finding.holds(after),
        "{finding:?} must clear for {} / {}",
        class_name(inputs.harness()),
        class_name(inputs.attributes())
    );
    let allowed = mask(before) & !finding.bit();
    assert_eq!(
        mask(after) & !allowed,
        0,
        "{finding:?} fix on {} / {} introduced a finding: {after:?}",
        class_name(inputs.harness()),
        class_name(inputs.attributes())
    );
}

#[test]
fn every_suggested_fix_removes_its_finding_and_adds_none() {
    let mut tally = Tally::default();
    for inputs in all_inputs() {
        check_state(inputs, &mut tally);
    }

    tally.assert_not_vacuous();
}

#[test]
fn the_lemma_is_not_vacuous_for_any_finding_kind() {
    let mut tally = Tally::default();
    for inputs in all_inputs() {
        let findings = diagnose(inputs);
        for finding in Finding::ALL {
            if finding.holds(findings) {
                *tally.slot(finding) += fixes(finding, inputs).len();
            }
        }
    }

    tally.assert_not_vacuous();
}
