//! Tests for Rust step definition indexing.

use super::*;

#[expect(
    clippy::expect_used,
    reason = "tests use explicit failures for clarity"
)]
#[test]
fn indexes_step_definitions_and_infers_patterns() {
    let source = concat!(
        "use rstest_bdd_macros::{given, when, then};\n",
        "\n",
        "#[given(\"a message\")]\n",
        "fn has_pattern() {}\n",
        "\n",
        "#[when]\n",
        "fn inferred_from_name() {}\n",
        "\n",
        "#[then(\"   \")]\n",
        "fn inferred_from_whitespace() {}\n",
        "\n",
        "#[given(\"\")]\n",
        "fn empty_pattern() {}\n",
        "\n",
        "#[rstest_bdd_macros::when(\"qualified\")]\n",
        "fn qualified_attribute() {}\n",
    );

    let index = index_rust_source(PathBuf::from("steps.rs"), source).expect("index rust source");
    assert_eq!(index.step_definitions.len(), 5);

    let given = index
        .step_definitions
        .iter()
        .find(|step| step.function.name == "has_pattern")
        .expect("given step");
    assert_eq!(given.keyword, StepType::Given);
    assert_eq!(given.pattern, "a message");
    assert!(!given.pattern_inferred);

    let inferred = index
        .step_definitions
        .iter()
        .find(|step| step.function.name == "inferred_from_name")
        .expect("when step");
    assert_eq!(inferred.keyword, StepType::When);
    assert_eq!(inferred.pattern, "inferred from name");
    assert!(inferred.pattern_inferred);

    let inferred_whitespace = index
        .step_definitions
        .iter()
        .find(|step| step.function.name == "inferred_from_whitespace")
        .expect("then step");
    assert_eq!(inferred_whitespace.keyword, StepType::Then);
    assert_eq!(inferred_whitespace.pattern, "inferred from whitespace");
    assert!(inferred_whitespace.pattern_inferred);

    let empty_pattern = index
        .step_definitions
        .iter()
        .find(|step| step.function.name == "empty_pattern")
        .expect("empty pattern step");
    assert_eq!(empty_pattern.keyword, StepType::Given);
    assert_eq!(empty_pattern.pattern, "");
    assert!(!empty_pattern.pattern_inferred);

    let qualified = index
        .step_definitions
        .iter()
        .find(|step| step.function.name == "qualified_attribute")
        .expect("qualified attribute step");
    assert_eq!(qualified.keyword, StepType::When);
    assert_eq!(qualified.pattern, "qualified");
}

#[expect(
    clippy::expect_used,
    reason = "tests use explicit failures for clarity"
)]
#[test]
fn indexes_parameter_expectations_for_tables_and_docstrings() {
    let source = concat!(
        "use rstest_bdd_macros::when;\n",
        "\n",
        "#[when]\n",
        "fn uses_param_attrs(#[datatable] table: Vec<Vec<String>>, docstring: String) {}\n",
        "\n",
        "#[when]\n",
        "fn uses_param_names(datatable: Vec<Vec<String>>) {}\n",
        "\n",
        "#[when]\n",
        "fn docstring_wrong_type(docstring: &str) {}\n",
    );

    let index = index_rust_source(PathBuf::from("steps.rs"), source).expect("index rust source");

    let uses_param_attrs = index
        .step_definitions
        .iter()
        .find(|step| step.function.name == "uses_param_attrs")
        .expect("expected step");
    assert!(uses_param_attrs.expects_table);
    assert!(uses_param_attrs.expects_docstring);
    assert_eq!(uses_param_attrs.parameters.len(), 2);
    assert!(
        uses_param_attrs
            .parameters
            .iter()
            .any(|param| param.is_datatable)
    );
    assert!(
        uses_param_attrs
            .parameters
            .iter()
            .any(|param| param.is_docstring)
    );

    let uses_param_names = index
        .step_definitions
        .iter()
        .find(|step| step.function.name == "uses_param_names")
        .expect("expected step");
    assert!(uses_param_names.expects_table);
    assert!(!uses_param_names.expects_docstring);

    let docstring_wrong_type = index
        .step_definitions
        .iter()
        .find(|step| step.function.name == "docstring_wrong_type")
        .expect("expected step");
    assert!(!docstring_wrong_type.expects_docstring);
}

#[expect(
    clippy::expect_used,
    reason = "tests use explicit failures for clarity"
)]
#[test]
fn preserves_module_path_for_nested_definitions() {
    let source = concat!(
        "use rstest_bdd_macros::given;\n",
        "\n",
        "mod outer {\n",
        "    mod inner {\n",
        "        use rstest_bdd_macros::given;\n",
        "        #[given(\"nested\")]\n",
        "        fn nested_step() {}\n",
        "    }\n",
        "}\n",
    );

    let index = index_rust_source(PathBuf::from("steps.rs"), source).expect("index rust source");
    assert_eq!(index.step_definitions.len(), 1);
    let step = index
        .step_definitions
        .first()
        .expect("expected nested step");
    assert_eq!(
        step.function.module_path,
        vec!["outer".to_string(), "inner".to_string()]
    );
    assert_eq!(step.function.name, "nested_step");
}
