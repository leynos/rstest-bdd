//! `LEM-FIX-CONVERGES`: a suggested fix never introduces a new finding.
//!
//! Every remedy a message offers is modelled as a transformation on states.
//! Working this lemma by hand falsified four design candidates while the plan
//! was drafted, so it constrains the design rather than restating it; two of
//! the survivors (W002's second fix leaving W001 in place, and D31's
//! user-attribute suppression) are only correct because the finding set keeps
//! the underlying W001.

use super::{
    super::model::{Framework, PolicyFindings, PolicyInputs, SlotClass, diagnose},
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
}

impl Finding {
    /// Every finding kind, in a stable order.
    const ALL: [Self; 3] = [Self::HarnessPath, Self::AttributesPath, Self::MissingPolicy];

    /// Whether this finding holds in `findings`.
    fn holds(self, findings: PolicyFindings) -> bool {
        match self {
            Self::HarnessPath => findings.harness_unrecognized_path,
            Self::AttributesPath => findings.attributes_unrecognized_path,
            Self::MissingPolicy => findings.missing_attribute_policy,
        }
    }

    /// This finding's bit in a finding mask.
    fn bit(self) -> u8 {
        match self {
            Self::HarnessPath => 0b001,
            Self::AttributesPath => 0b010,
            Self::MissingPolicy => 0b100,
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

/// The framework a slot evidences, if it evidences one.
fn framework(class: SlotClass) -> Option<Framework> {
    match class {
        SlotClass::Evidenced(framework) => Some(framework),
        SlotClass::Absent | SlotClass::Recognized(_) | SlotClass::Foreign => None,
    }
}

/// The states the messages offer as fixes for one finding.
fn fixes(finding: Finding, inputs: PolicyInputs) -> Vec<PolicyInputs> {
    match finding {
        Finding::HarnessPath => framework(inputs.harness())
            .map(|family| vec![inputs.with_harness(SlotClass::Recognized(family.into()))])
            .unwrap_or_default(),
        Finding::AttributesPath => framework(inputs.attributes())
            .map(|family| vec![inputs.with_attributes(SlotClass::Recognized(family.into()))])
            .unwrap_or_default(),
        Finding::MissingPolicy => {
            let SlotClass::Evidenced(family) = inputs.harness() else {
                return Vec::new();
            };
            vec![
                inputs.with_harness(SlotClass::Recognized(family.into())),
                inputs.with_attributes(SlotClass::Recognized(family.into())),
            ]
        }
    }
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
        }
    }

    /// Fail when a finding kind contributed nothing, which would make the
    /// lemma vacuous for it.
    fn assert_not_vacuous(&self) {
        assert!(
            self.harness > 0 && self.attributes > 0 && self.policy > 0,
            "every finding kind must contribute checked fixes, got {}, {}, {}",
            self.harness,
            self.attributes,
            self.policy
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
