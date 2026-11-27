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

#[test]
fn every_suggested_fix_removes_its_finding_and_adds_none() {
    let (mut harness_checked, mut attributes_checked, mut policy_checked) =
        (0_usize, 0_usize, 0_usize);
    for inputs in all_inputs() {
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
                match finding {
                    Finding::HarnessPath => harness_checked += 1,
                    Finding::AttributesPath => attributes_checked += 1,
                    Finding::MissingPolicy => policy_checked += 1,
                }
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
        }
    }

    assert!(
        harness_checked > 0 && attributes_checked > 0 && policy_checked > 0,
        "every finding kind must contribute checked fixes, got {harness_checked}, {attributes_checked}, {policy_checked}"
    );
}

#[test]
fn the_lemma_is_not_vacuous_for_any_finding_kind() {
    let (mut harness_total, mut attributes_total, mut policy_total) = (0_usize, 0_usize, 0_usize);
    for inputs in all_inputs() {
        let findings = diagnose(inputs);
        for finding in Finding::ALL {
            if !finding.holds(findings) {
                continue;
            }
            let offered = fixes(finding, inputs).len();
            match finding {
                Finding::HarnessPath => harness_total += offered,
                Finding::AttributesPath => attributes_total += offered,
                Finding::MissingPolicy => policy_total += offered,
            }
        }
    }

    assert!(
        harness_total > 0 && attributes_total > 0 && policy_total > 0,
        "the lemma is vacuous for a finding kind with no fixes: \
         {harness_total}, {attributes_total}, {policy_total}"
    );
}
