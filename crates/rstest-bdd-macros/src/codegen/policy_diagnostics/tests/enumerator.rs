//! Exhaustive enumeration of the W001 and W002 state space.
//!
//! The properties here are stated over the classified slots rather than over
//! the clauses of `diagnose`, so they constrain the implementation instead of
//! restating it. The guard functions carry no wildcard arm: adding a variant
//! to `SlotClass` or `Framework` breaks compilation until the enumerator is
//! updated.

use rstest_bdd_policy::TestAttributeHint;

use super::super::{
    model::{
        Framework,
        PolicyFindings,
        PolicyInputs,
        SlotClass,
        diagnose,
        framework_of,
        has_evidence,
        implied_harness_family,
    },
    resolution::{emitted_attributes, selected_hint},
};

/// Every slot class the classifier can produce.
pub(super) const CLASSES: [SlotClass; 7] = [
    SlotClass::Absent,
    SlotClass::Recognized(TestAttributeHint::RstestOnly),
    SlotClass::Recognized(TestAttributeHint::RstestWithTokioCurrentThread),
    SlotClass::Recognized(TestAttributeHint::RstestWithGpuiTest),
    SlotClass::Evidenced(Framework::Tokio),
    SlotClass::Evidenced(Framework::Gpui),
    SlotClass::Foreign,
];

/// Every framework whose adapter paths can be spelled non-canonically.
const FRAMEWORKS: [Framework; 2] = [Framework::Tokio, Framework::Gpui];

/// Name one slot class for failure messages.
///
/// The match is exhaustive on purpose; see the module documentation.
pub(super) fn class_name(class: SlotClass) -> &'static str {
    match class {
        SlotClass::Absent => "absent",
        SlotClass::Recognized(hint) => match hint {
            TestAttributeHint::RstestOnly => "recognized-base",
            TestAttributeHint::RstestWithTokioCurrentThread => "recognized-tokio",
            TestAttributeHint::RstestWithGpuiTest => "recognized-gpui",
        },
        SlotClass::Evidenced(_) => "evidenced",
        SlotClass::Foreign => "foreign",
    }
}

/// Name one framework for failure messages.
///
/// The match is exhaustive on purpose; see the module documentation.
fn framework_name(framework: Framework) -> &'static str {
    match framework {
        Framework::Tokio => "tokio",
        Framework::Gpui => "gpui",
    }
}

/// Every classified `#[scenario]` input.
fn scenario_inputs() -> Vec<PolicyInputs> {
    CLASSES
        .iter()
        .flat_map(|&harness| {
            CLASSES.iter().flat_map(move |&attributes| {
                [false, true].into_iter().flat_map(move |is_async| {
                    [false, true]
                        .into_iter()
                        .flat_map(move |has_executor_attr| {
                            [false, true].into_iter().map(move |has_gpui_test_attr| {
                                PolicyInputs::Scenario {
                                    harness,
                                    attributes,
                                    is_async,
                                    has_executor_attr,
                                    has_gpui_test_attr,
                                }
                            })
                        })
                })
            })
        })
        .collect()
}

/// Every classified `scenarios!` input.
fn scenarios_inputs() -> Vec<PolicyInputs> {
    CLASSES
        .iter()
        .flat_map(|&harness| {
            CLASSES.iter().flat_map(move |&attributes| {
                [false, true]
                    .into_iter()
                    .map(move |runtime_alias| PolicyInputs::Scenarios {
                        harness,
                        attributes,
                        runtime_alias,
                    })
            })
        })
        .collect()
}

/// Every classified input, in a stable order.
pub(super) fn all_inputs() -> Vec<PolicyInputs> {
    let mut inputs = scenario_inputs();
    inputs.extend(scenarios_inputs());
    inputs
}

#[test]
fn the_model_covers_every_variant_at_least_once() {
    assert_eq!(
        CLASSES.len(),
        7,
        "every slot class must be enumerated exactly once"
    );
    assert_eq!(
        FRAMEWORKS.len(),
        2,
        "every evidenced framework must be enumerated exactly once"
    );
    for framework in FRAMEWORKS {
        assert!(
            CLASSES.contains(&SlotClass::Evidenced(framework)),
            "the enumerator must reach {}",
            framework_name(framework)
        );
    }
}

#[test]
fn the_state_space_holds_four_hundred_and_ninety_distinct_states() {
    let inputs = all_inputs();
    let mut distinct: Vec<PolicyInputs> = Vec::new();
    for input in &inputs {
        if !distinct.contains(input) {
            distinct.push(*input);
        }
    }

    assert_eq!(
        scenario_inputs().len(),
        392,
        "seven classes by seven classes by two signature shapes by two attribute axes"
    );
    assert_eq!(
        scenarios_inputs().len(),
        98,
        "seven classes by seven classes by two alias states"
    );
    assert_eq!(inputs.len(), 490, "the domain is the sum of both shapes");
    assert_eq!(
        distinct.len(),
        490,
        "the enumerator must yield 490 distinct states, not 490 yielded values"
    );
}

/// Whether the configuration is one the silence contract speaks about.
///
/// The antecedent is written from the classified slots, not from `diagnose`,
/// so it cannot restate the clauses it is meant to constrain.
fn expects_silence(inputs: PolicyInputs) -> bool {
    let conflict = implied_harness_family(inputs).is_some_and(|harness| {
        framework_of(inputs.attributes()).is_some_and(|attributes| harness != attributes)
    });
    let alias_error = match inputs {
        PolicyInputs::Scenarios { runtime_alias, .. } => {
            runtime_alias && !matches!(inputs.harness(), SlotClass::Absent)
        }
        PolicyInputs::Scenario { .. } => false,
    };
    !has_evidence(inputs.harness())
        && !has_evidence(inputs.attributes())
        && !conflict
        && !alias_error
        && !no_executor(inputs)
}

/// Whether the configuration is one `rstest` rejects for lacking an executor.
///
/// Stated through the precedence ladder rather than through the emitted set,
/// so it does not restate `has_missing_executor`.
fn no_executor(inputs: PolicyInputs) -> bool {
    let PolicyInputs::Scenario {
        is_async: true,
        has_executor_attr: false,
        ..
    } = inputs
    else {
        return false;
    };
    let selected = selected_hint(
        inputs.harness(),
        inputs.attributes(),
        inputs.attribute_runtime(),
    );
    matches!(inputs.harness(), SlotClass::Absent)
        && !matches!(inputs.attributes(), SlotClass::Absent)
        && selected == TestAttributeHint::RstestOnly
}

#[test]
fn no_evidenced_slot_and_no_framework_conflict_means_no_finding() {
    let silent: Vec<PolicyInputs> = all_inputs()
        .into_iter()
        .filter(|inputs| expects_silence(*inputs))
        .collect();

    assert!(
        !silent.is_empty(),
        "the no-evidence antecedent must be reachable"
    );
    for inputs in silent {
        let findings = diagnose(inputs);
        assert_eq!(
            findings,
            PolicyFindings::default(),
            "no evidenced slot may warn: {} / {} -> {findings:?}",
            class_name(inputs.harness()),
            class_name(inputs.attributes())
        );
    }
}

#[test]
fn each_slot_contributes_at_most_one_rendered_finding() {
    let mut none = 0_usize;
    let mut one = 0_usize;
    let mut two = 0_usize;
    for inputs in all_inputs() {
        let findings = diagnose(inputs);
        let harness_slot = findings.missing_attribute_policy || findings.slots.harness;
        let rendered = usize::from(harness_slot) + usize::from(findings.slots.attributes);
        match rendered {
            0 => none += 1,
            1 => one += 1,
            _ => two += 1,
        }
    }

    assert!(
        none > 0 && one > 0 && two > 0,
        "every rendered count must be reachable, got none={none}, one={one}, two={two}"
    );
}

/// W002 must be a *strict* addition, not a swap between frameworks.
#[test]
fn a_replaced_framework_attribute_is_not_a_missing_policy() {
    let inputs = PolicyInputs::Scenario {
        harness: SlotClass::Evidenced(Framework::Gpui),
        attributes: SlotClass::Absent,
        is_async: true,
        has_executor_attr: false,
        has_gpui_test_attr: false,
    };

    assert!(
        !diagnose(inputs).missing_attribute_policy,
        "an async GPUI harness replaces the Tokio attribute rather than losing one"
    );
    assert!(
        diagnose(inputs).slots.harness,
        "the non-canonical harness is still reported as W001"
    );
}

#[test]
fn a_user_gpui_attribute_suppresses_the_missing_policy_claim() {
    let base = PolicyInputs::Scenario {
        harness: SlotClass::Evidenced(Framework::Gpui),
        attributes: SlotClass::Absent,
        is_async: false,
        has_executor_attr: false,
        has_gpui_test_attr: false,
    };
    let with_gpui_attr = PolicyInputs::Scenario {
        harness: SlotClass::Evidenced(Framework::Gpui),
        attributes: SlotClass::Absent,
        is_async: false,
        has_executor_attr: true,
        has_gpui_test_attr: true,
    };
    let executor_only = PolicyInputs::Scenario {
        harness: SlotClass::Evidenced(Framework::Gpui),
        attributes: SlotClass::Absent,
        is_async: false,
        has_executor_attr: true,
        has_gpui_test_attr: false,
    };

    assert!(
        diagnose(base).missing_attribute_policy,
        "the base state loses the GPUI attribute"
    );
    assert!(
        !diagnose(with_gpui_attr).missing_attribute_policy,
        "a user-written `#[gpui::test]` is deduplicated, so the claim is false"
    );
    assert!(
        diagnose(executor_only).missing_attribute_policy,
        "an executor attribute that is not `#[gpui::test]` leaves the GPUI attribute absent"
    );
}

#[test]
fn evidence_alone_selects_the_runtime_fallback() {
    for framework in FRAMEWORKS {
        let hint = selected_hint(
            SlotClass::Evidenced(framework),
            SlotClass::Absent,
            rstest_bdd_policy::RuntimeMode::Sync,
        );

        assert_eq!(
            hint,
            TestAttributeHint::RstestOnly,
            "an evidenced {} harness selects no attribute of its own",
            framework_name(framework)
        );
    }
}

#[test]
fn a_missing_policy_always_means_a_strictly_larger_canonical_set() {
    for inputs in all_inputs() {
        if !diagnose(inputs).missing_attribute_policy {
            continue;
        }
        let SlotClass::Evidenced(framework) = inputs.harness() else {
            panic!("W002 requires an evidenced harness");
        };
        let canonical = inputs.with_harness(SlotClass::Recognized(framework.into()));

        assert!(
            emitted_attributes(canonical).is_strict_superset_of(emitted_attributes(inputs)),
            "W002 must hold only where canonicalizing strictly adds an attribute"
        );
    }
}
