//! Unit tests for the step registry index.

use super::*;
use crate::indexing::index_rust_source;

#[test]
fn replaces_file_entries_and_populates_keyword_registry() {
    let source = concat!(
        "use rstest_bdd_macros::{given, when};\n",
        "\n",
        "#[given(\"I have {n:u32}\")]\n",
        "fn have_number() {}\n",
        "\n",
        "#[when(\"I add 1\")]\n",
        "fn add_one() {}\n",
    );
    let index =
        index_rust_source(PathBuf::from("/tmp/steps.rs"), source).expect("index rust source");
    let mut registry = StepDefinitionRegistry::default();
    let errors = registry.replace_rust_file(&index);
    assert!(
        errors.is_empty(),
        "expected patterns to compile successfully: {errors:?}"
    );
    let given = registry.steps_for_keyword(StepType::Given);
    assert_eq!(given.len(), 1);
    assert!(
        given
            .first()
            .expect("compiled given matcher")
            .regex
            .is_match("I have 42")
    );
    let when = registry.steps_for_keyword(StepType::When);
    assert_eq!(when.len(), 1);
    assert!(
        when.first()
            .expect("compiled when matcher")
            .regex
            .is_match("I add 1")
    );
}

#[test]
fn filters_keyword_lookup_to_selected_libraries() {
    let source = concat!(
        "use rstest_bdd_macros::{given, step_library};\n",
        "#[step_library] mod accounts { use rstest_bdd_macros::given; ",
        "#[given(\"the domain is empty\")] fn empty() {} }\n",
        "#[step_library] mod filesystem { use rstest_bdd_macros::given; ",
        "#[given(\"the domain is empty\")] fn empty() {} }\n",
    );
    let index =
        index_rust_source(PathBuf::from("/tmp/steps.rs"), source).expect("index rust source");
    let mut registry = StepDefinitionRegistry::default();
    let errors = registry.replace_rust_file(&index);
    assert!(errors.is_empty(), "patterns should compile: {errors:?}");
    let accounts =
        registry.steps_for_keyword_in_scope(StepType::Given, &[String::from("accounts")]);
    assert_eq!(accounts.len(), 1);
    assert_eq!(
        accounts
            .first()
            .map(|definition| definition.library.as_str()),
        Some("accounts")
    );
}

#[test]
fn invalidates_entries_for_a_single_file_incrementally() {
    let path = PathBuf::from("/tmp/steps.rs");
    let first = "use rstest_bdd_macros::{given, when};\n#[given(\"a\")] fn step_a() \
                 {}\n#[when(\"b\")] fn step_b() {}\n";
    let second = "use rstest_bdd_macros::given;\n#[given(\"a\")] fn step_a() {}\n";
    let index_first = index_rust_source(path.clone(), first).expect("index first source");
    let index_second = index_rust_source(path.clone(), second).expect("index second source");
    let mut registry = StepDefinitionRegistry::default();
    registry.replace_rust_file(&index_first);
    assert_eq!(registry.steps_for_keyword(StepType::Given).len(), 1);
    assert_eq!(registry.steps_for_keyword(StepType::When).len(), 1);
    registry.replace_rust_file(&index_second);
    assert_eq!(registry.steps_for_keyword(StepType::Given).len(), 1);
    assert!(registry.steps_for_keyword(StepType::When).is_empty());
    assert_eq!(registry.steps_for_file(&path).len(), 1);
}

#[test]
fn invalidates_swapped_keyword_entries_by_their_updated_position() {
    let first_path = PathBuf::from("/tmp/first_steps.rs");
    let middle_path = PathBuf::from("/tmp/middle_steps.rs");
    let final_path = PathBuf::from("/tmp/final_steps.rs");
    let source = "use rstest_bdd_macros::given;\n#[given(\"a step\")] fn a_step() {}\n";
    let first = index_rust_source(first_path.clone(), source).expect("index first source");
    let middle = index_rust_source(middle_path.clone(), source).expect("index middle source");
    let final_index = index_rust_source(final_path.clone(), source).expect("index final source");
    let mut registry = StepDefinitionRegistry::default();
    registry.replace_rust_file(&first);
    registry.replace_rust_file(&middle);
    registry.replace_rust_file(&final_index);
    registry.invalidate_file(&middle_path);
    registry.invalidate_file(&final_path);
    assert_eq!(registry.steps_for_file(&first_path).len(), 1);
    assert_eq!(registry.steps_for_keyword(StepType::Given).len(), 1);
    assert!(registry.steps_for_file(&middle_path).is_empty());
    assert!(registry.steps_for_file(&final_path).is_empty());
}
