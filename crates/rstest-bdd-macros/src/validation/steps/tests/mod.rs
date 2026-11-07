//! Tests for missing, single, and ambiguous step-definition validation outcomes.
//! Add per-function `#[expect(...)]` entries rather than file-wide lint suppressions.
use rstest::rstest;
use serial_test::serial;

use super::{crate_id::normalize_crate_id, *};

mod cases;
mod identities;
mod support;
use self::{
    cases::{AmbiguousCase, PatternCase, RegistrationCase, ScopedCase, StrictCase},
    support::{assert_bullet_count, clear_registry, create_test_step},
};

#[rstest]
#[case::basic(PatternCase { pattern: "a step", text: "a step" })]
#[case::placeholder(PatternCase {
    pattern: "I have {item}",
    text: "I have apples",
})]
#[case::typed(PatternCase {
    pattern: "number {n:u32}",
    text: "number 42",
})]
#[serial]
fn validates_step_patterns(#[case] case: PatternCase<'_>) {
    clear_registry();
    register_step(
        StepKeyword::Given,
        &syn::LitStr::new(case.pattern, proc_macro2::Span::call_site()),
    );
    let steps = [create_test_step(StepKeyword::Given, case.text)];
    assert!(validate_steps_exist(&steps, true).is_ok());
    assert!(validate_steps_exist(&steps, false).is_ok());
}

#[rstest]
#[case::missing_step(StrictCase {
    foreign: None,
    text: "missing",
})]
#[case::foreign_crate_step(StrictCase {
    foreign: Some(("a step", "other")),
    text: "a step",
})]
#[serial]
fn validates_strict_mode_errors(#[case] case: StrictCase<'_>) {
    clear_registry();
    if let Some((pattern, crate_id)) = case.foreign {
        register_step_for_crate(StepKeyword::Given, pattern, crate_id);
    }
    let steps = [create_test_step(StepKeyword::Given, case.text)];
    assert!(validate_steps_exist(&steps, true).is_err());
    assert!(validate_steps_exist(&steps, false).is_ok());
}

#[rstest]
#[case::literal(AmbiguousCase {
    pattern_a: "a step",
    pattern_b: "a step",
    text: "a step",
})]
#[case::placeholder(AmbiguousCase {
    pattern_a: "I have {item}",
    pattern_b: "I have {n:u32}",
    text: "I have 1",
})]
#[serial]
fn errors_when_step_ambiguous(#[case] case: AmbiguousCase<'_>) {
    clear_registry();
    let lit_a = syn::LitStr::new(case.pattern_a, proc_macro2::Span::call_site());
    let lit_b = syn::LitStr::new(case.pattern_b, proc_macro2::Span::call_site());
    register_step(StepKeyword::Given, &lit_a);
    register_step(StepKeyword::Given, &lit_b);
    let steps = [create_test_step(StepKeyword::Given, case.text)];
    let err = match validate_steps_exist(&steps, false) {
        Err(e) => e.to_string(),
        Ok(()) => panic!("expected ambiguous step error"),
    };
    assert!(err.contains("Ambiguous step definition"));
    assert!(err.contains(case.pattern_a));
    assert!(err.contains(case.pattern_b));
    assert_bullet_count(&err, 2);
    assert!(validate_steps_exist(&steps, true).is_err());
}

#[rstest]
#[serial]
fn aborts_on_invalid_step_pattern() {
    clear_registry();
    // proc-macro-error3 panics outside macro contexts; assert expected message
    let Err(err) = std::panic::catch_unwind(|| {
        register_step(
            StepKeyword::Given,
            &syn::LitStr::new("unclosed {", proc_macro2::Span::call_site()),
        );
    }) else {
        panic!("expected invalid step pattern to abort");
    };
    let msg = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err.downcast_ref::<&str>().copied());
    let Some(msg) = msg else {
        panic!("panic payload must be a string");
    };
    assert!(
        msg.contains("proc-macro-error3 API cannot be used outside of `entry_point` invocation")
    );

    assert!(rstest_bdd_patterns::build_regex_from_pattern("unclosed {").is_err());
}

#[test]
#[serial]
fn errors_when_step_matches_three_definitions() {
    clear_registry();
    let lit_a = syn::LitStr::new("I have {item}", proc_macro2::Span::call_site());
    let lit_b = syn::LitStr::new("I have {n:u32}", proc_macro2::Span::call_site());
    let lit_c = syn::LitStr::new("I have 1", proc_macro2::Span::call_site());
    register_step(StepKeyword::Given, &lit_a);
    register_step(StepKeyword::Given, &lit_b);
    register_step(StepKeyword::Given, &lit_c);
    let steps = [create_test_step(StepKeyword::Given, "I have 1")];
    let err = match validate_steps_exist(&steps, false) {
        Err(e) => e.to_string(),
        Ok(()) => panic!("expected ambiguous step error"),
    };
    assert!(err.contains("Ambiguous step definition"));
    assert!(err.contains("I have {item}"));
    assert!(err.contains("I have {n:u32}"));
    assert!(err.contains("I have 1"));
    assert_bullet_count(&err, 3);
    assert!(validate_steps_exist(&steps, true).is_err());
}

#[test]
#[serial]
fn scoped_validation_uses_only_selected_libraries() {
    clear_registry();
    let account = syn::LitStr::new("the domain is empty", proc_macro2::Span::call_site());
    let filesystem = syn::LitStr::new("the domain is empty", proc_macro2::Span::call_site());
    register_step_in_library(StepKeyword::Given, &account, "accounts");
    register_step_in_library(StepKeyword::Given, &filesystem, "filesystem");
    let steps = [create_test_step(StepKeyword::Given, "the domain is empty")];
    let accounts = vec![Box::<str>::from("accounts")];

    assert!(validate_steps_exist_in_scope(&steps, &accounts, true).is_ok());

    let partly_external = vec![
        Box::<str>::from("accounts"),
        Box::<str>::from("external::shared"),
    ];
    let external_step = [create_test_step(
        StepKeyword::Given,
        "an externally defined step",
    )];
    assert!(validate_steps_exist_in_scope(&external_step, &partly_external, true).is_ok());
}

fn scoped_validation_error(case: ScopedCase<'_>) -> String {
    clear_registry();
    for (library, pattern) in case.definitions {
        let pattern = syn::LitStr::new(pattern, proc_macro2::Span::call_site());
        register_step_in_library(StepKeyword::Given, &pattern, library);
    }
    let steps = [create_test_step(StepKeyword::Given, "the domain is empty")];
    let libraries = case
        .selected
        .iter()
        .map(|library| Box::<str>::from(*library))
        .collect::<Vec<_>>();

    let Err(error) = validate_steps_exist_in_scope(&steps, &libraries, true) else {
        return String::from("scoped validation fixture unexpectedly succeeded");
    };
    error.to_string()
}

#[test]
#[serial]
fn scoped_validation_reports_unselected_library_hints() {
    let error = scoped_validation_error(ScopedCase {
        definitions: &[
            ("accounts", "a different account step"),
            ("filesystem", "the domain is empty"),
        ],
        selected: &["accounts"],
    });
    assert!(error.contains("Selected libraries: [accounts]"));
    assert!(error.contains("unselected libraries"));
    assert!(error.contains("filesystem"));
}

#[test]
#[serial]
fn scoped_validation_reports_equal_candidates_without_precedence() {
    let error = scoped_validation_error(ScopedCase {
        definitions: &[
            ("accounts", "the domain is empty"),
            ("filesystem", "the domain is empty"),
        ],
        selected: &["accounts", "filesystem"],
    });
    assert!(error.contains("Ambiguous step definition"));
    assert!(error.contains("Selected libraries: [accounts, filesystem]"));
    assert!(error.contains("accounts"));
    assert!(error.contains("filesystem"));
}

fn registration_index_counts(
    case: RegistrationCase<'_>,
    register: impl FnOnce(),
) -> Result<(usize, usize), String> {
    clear_registry();
    register();

    let registry = REGISTERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let crate_id = normalize_crate_id(case.crate_id);
    let defs = registry.get(crate_id.as_ref()).ok_or_else(|| {
        format!("registration should create a crate definition entry for {crate_id}")
    })?;
    let global_count = defs.patterns(StepKeyword::Given).len();
    let scoped_count = defs
        .scoped_by_kw
        .get(&(Box::<str>::from(case.library), StepKeyword::Given))
        .map_or(0, Vec::len);

    Ok((global_count, scoped_count))
}

#[test]
#[serial]
fn global_registration_populates_global_and_scoped_indexes() -> Result<(), String> {
    let crate_id = "global-registration-test";
    let counts = registration_index_counts(
        RegistrationCase {
            crate_id,
            library: "rstest_bdd::global",
        },
        || register_step_for_crate(StepKeyword::Given, "the global step", crate_id),
    )?;

    assert_eq!(counts, (1, 1));
    Ok(())
}

#[test]
#[serial]
fn named_library_registration_avoids_global_index() -> Result<(), String> {
    let crate_id = current_crate_id();
    let pattern = syn::LitStr::new("the named step", proc_macro2::Span::call_site());
    let counts = registration_index_counts(
        RegistrationCase {
            crate_id,
            library: "accounts",
        },
        || register_step_in_library(StepKeyword::Given, &pattern, "accounts"),
    )?;

    assert_eq!(counts, (0, 1));
    Ok(())
}
