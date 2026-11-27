//! Exhaustive enumeration of the W001 state space.
//!
//! The properties here are stated over the classified slots rather than over
//! the clauses of `diagnose`, so they constrain the implementation instead of
//! restating it. The guard functions carry no wildcard arm: adding a variant
//! to `SlotClass` or `Framework` breaks compilation until the enumerator is
//! updated.

use rstest_bdd_policy::TestAttributeHint;

use super::super::{
    model::{Framework, PolicyInputs, SlotClass, diagnose, has_evidence},
    resolution::selected_hint,
};

/// Every slot class the classifier can produce.
const CLASSES: [SlotClass; 7] = [
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
fn class_name(class: SlotClass) -> &'static str {
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

/// Every classified input pair, in a stable order.
fn all_inputs() -> Vec<PolicyInputs> {
    CLASSES
        .iter()
        .flat_map(|&harness| {
            CLASSES.iter().map(move |&attributes| PolicyInputs {
                harness,
                attributes,
            })
        })
        .collect()
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
fn the_w001_state_space_holds_forty_nine_distinct_states() {
    let inputs = all_inputs();
    let mut distinct: Vec<PolicyInputs> = Vec::new();
    for input in &inputs {
        if !distinct.contains(input) {
            distinct.push(*input);
        }
    }

    assert_eq!(
        inputs.len(),
        49,
        "seven harness classes by seven attributes classes is 49 states"
    );
    assert_eq!(
        distinct.len(),
        49,
        "the enumerator must yield 49 distinct states, not 49 yielded values"
    );
}

#[test]
fn no_evidenced_slot_means_no_finding() {
    let silent: Vec<PolicyInputs> = all_inputs()
        .into_iter()
        .filter(|inputs| !has_evidence(inputs.harness) && !has_evidence(inputs.attributes))
        .collect();

    assert!(
        !silent.is_empty(),
        "the no-evidence antecedent must be reachable"
    );
    for inputs in silent {
        let findings = diagnose(inputs);
        assert!(
            !findings.harness_unrecognized_path && !findings.attributes_unrecognized_path,
            "no evidenced slot may warn: {} / {} -> {findings:?}",
            class_name(inputs.harness),
            class_name(inputs.attributes)
        );
    }
}

#[test]
fn each_slot_contributes_at_most_one_finding() {
    let mut counts = [0_usize; 3];
    for inputs in all_inputs() {
        let findings = diagnose(inputs);
        let count = usize::from(findings.harness_unrecognized_path)
            + usize::from(findings.attributes_unrecognized_path);
        assert!(
            count <= 2,
            "two slots cannot produce more than two slot findings: {} covered",
            count
        );
        counts[count] += 1;
    }

    assert!(
        counts.iter().all(|count| *count > 0),
        "every finding count must be reachable, got {counts:?}"
    );
}

#[test]
fn evidence_alone_selects_the_runtime_fallback() {
    for framework in FRAMEWORKS {
        let inputs = PolicyInputs {
            harness: SlotClass::Evidenced(framework),
            attributes: SlotClass::Absent,
        };
        let hint = selected_hint(
            inputs.harness,
            inputs.attributes,
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
