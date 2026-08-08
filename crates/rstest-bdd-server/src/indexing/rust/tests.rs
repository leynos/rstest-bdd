//! Tests for Rust step definition indexing.

use rstest::rstest;

use super::*;

/// Assert a step's keyword, pattern, and whether the pattern was inferred.
fn assert_step(index: &RustStepFileIndex, name: &str, expected: ExpectedStep<'_>) {
    let step = expect_step(index, name);
    assert_eq!(
        step.keyword, expected.keyword,
        "unexpected keyword for `{name}`"
    );
    assert_eq!(
        step.pattern, expected.pattern,
        "unexpected pattern for `{name}`"
    );
    assert_eq!(
        step.pattern_inferred, expected.inferred,
        "unexpected inference for `{name}`"
    );
}

/// The keyword and pattern a step definition is expected to carry.
#[derive(Clone, Copy)]
struct ExpectedStep<'a> {
    keyword: StepType,
    pattern: &'a str,
    inferred: bool,
}

/// Locate an indexed step definition by function name.
fn expect_step<'a>(index: &'a RustStepFileIndex, name: &str) -> &'a IndexedStepDefinition {
    let Some(step) = index
        .step_definitions
        .iter()
        .find(|step| step.function.name == name)
    else {
        panic!("expected an indexed step for `{name}`");
    };
    step
}

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

    for (name, expected) in [
        (
            "has_pattern",
            ExpectedStep {
                keyword: StepType::Given,
                pattern: "a message",
                inferred: false,
            },
        ),
        (
            "inferred_from_name",
            ExpectedStep {
                keyword: StepType::When,
                pattern: "inferred from name",
                inferred: true,
            },
        ),
        (
            "inferred_from_whitespace",
            ExpectedStep {
                keyword: StepType::Then,
                pattern: "inferred from whitespace",
                inferred: true,
            },
        ),
        (
            "empty_pattern",
            ExpectedStep {
                keyword: StepType::Given,
                pattern: "",
                inferred: false,
            },
        ),
        (
            "qualified_attribute",
            ExpectedStep {
                keyword: StepType::When,
                pattern: "qualified",
                inferred: false,
            },
        ),
    ] {
        assert_step(&index, name, expected);
    }
}

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
        "fn docstring_std(docstring: std::string::String) {}\n",
        "\n",
        "#[when]\n",
        "fn docstring_alloc(docstring: alloc::string::String) {}\n",
        "\n",
        "#[when]\n",
        "fn docstring_wrong_type(docstring: &str) {}\n",
    );

    let index = index_rust_source(PathBuf::from("steps.rs"), source).expect("index rust source");

    assert_param_attrs_step(&index);

    assert_expectations(&index, "uses_param_names", true, false);
    for function in ["uses_param_attrs", "docstring_std", "docstring_alloc"] {
        assert_expectations(&index, function, false, true);
    }
    assert_expectations(&index, "docstring_wrong_type", false, false);
}

/// Assert that `#[datatable]` and `docstring` parameter attributes are indexed.
fn assert_param_attrs_step(index: &RustStepFileIndex) {
    let step = expect_step(index, "uses_param_attrs");
    assert_eq!(step.parameters.len(), 2, "expected two indexed parameters");
    assert!(
        step.parameters.iter().any(|param| param.is_datatable),
        "expected a datatable parameter"
    );
    assert!(
        step.parameters.iter().any(|param| param.is_docstring),
        "expected a docstring parameter"
    );
}

/// Assert what a step expects to receive from the feature file.
///
/// `expects_table` is only checked when `true`, because several cases only
/// constrain the docstring expectation.
fn assert_expectations(
    index: &RustStepFileIndex,
    name: &str,
    expects_table: bool,
    expects_docstring: bool,
) {
    let step = expect_step(index, name);
    if expects_table {
        assert!(step.expects_table, "`{name}` should expect a data table");
    }
    assert_eq!(
        step.expects_docstring, expects_docstring,
        "unexpected docstring expectation for `{name}`"
    );
}

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
        vec!["outer".to_owned(), "inner".to_owned()]
    );
    assert_eq!(step.function.name, "nested_step");
}

#[test]
fn returns_error_when_multiple_step_attributes_present() {
    let source = concat!(
        "use rstest_bdd_macros::{given, when};\n",
        "\n",
        "#[given(\"a\")]\n",
        "#[when(\"b\")]\n",
        "fn conflicting_step() {}\n",
    );

    let err = index_rust_source(PathBuf::from("steps.rs"), source)
        .expect_err("expected indexing to fail");

    match err {
        RustStepIndexError::MultipleStepAttributes { function } => {
            assert_eq!(function, "conflicting_step");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[rstest]
#[case(
    concat!(
        "use rstest_bdd_macros::given;\n",
        "\n",
        "#[given(123)]\n",
        "fn invalid_args() {}\n",
    ),
    "invalid_args"
)]
#[case(
    concat!(
        "use rstest_bdd_macros::given;\n",
        "\n",
        "#[given(foo = 42)]\n",
        "fn invalid_named_args() {}\n",
    ),
    "invalid_named_args"
)]
fn returns_error_when_step_attribute_arguments_are_invalid(
    #[case] source: &'static str,
    #[case] expected_function: &'static str,
) {
    let err = match index_rust_source(PathBuf::from("steps.rs"), source) {
        Ok(index) => panic!(
            "expected indexing to fail, but got {} step definitions",
            index.step_definitions.len()
        ),
        Err(err) => err,
    };

    match err {
        RustStepIndexError::InvalidStepAttributeArguments {
            function,
            attribute,
            message,
        } => {
            assert_eq!(function, expected_function);
            assert_eq!(attribute, "given");
            assert!(
                !message.trim().is_empty(),
                "expected an explanatory error message"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
