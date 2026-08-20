//! Tests for Rust step definition indexing.

use super::*;
use proptest::prelude::*;
use rstest::rstest;

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

    for function in ["uses_param_attrs", "docstring_std", "docstring_alloc"] {
        let step = index
            .step_definitions
            .iter()
            .find(|step| step.function.name == function)
            .expect("expected step");
        assert!(step.expects_docstring);
    }

    let docstring_wrong_type = index
        .step_definitions
        .iter()
        .find(|step| step.function.name == "docstring_wrong_type")
        .expect("expected step");
    assert!(!docstring_wrong_type.expects_docstring);
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
        vec!["outer".to_string(), "inner".to_string()]
    );
    assert_eq!(step.function.name, "nested_step");
}

#[test]
fn reports_multiple_step_attributes_without_discarding_valid_steps() {
    let source = concat!(
        "use rstest_bdd_macros::{given, when};\n",
        "\n",
        "#[given(\"a\")]\n",
        "#[when(\"b\")]\n",
        "fn conflicting_step() {}\n",
        "\n",
        "#[given(\"valid\")]\n",
        "fn valid_step() {}\n",
    );

    let result = index_rust_source(PathBuf::from("steps.rs"), source)
        .expect("source syntax should remain indexable");
    assert_eq!(result.step_definitions.len(), 1);

    match result.diagnostics.as_slice() {
        [RustStepIndexDiagnostic::MultipleStepAttributes { function }] => {
            assert_eq!(function, "conflicting_step");
        }
        other => panic!("unexpected diagnostics: {other:?}"),
    }
}

#[derive(Clone, Debug)]
enum GeneratedStepItem {
    Valid,
    Invalid,
    InlineModule(Vec<Self>),
}

#[derive(Clone, Copy, Debug)]
enum GeneratedStepKind {
    Valid,
    Invalid,
}

#[derive(Default)]
struct GeneratedRustSource {
    source: String,
    next_function: usize,
    next_module: usize,
    expected_step_names: Vec<String>,
    expected_diagnostic_names: Vec<String>,
}

impl GeneratedRustSource {
    fn append_items(&mut self, items: &[GeneratedStepItem], indentation: usize) {
        for item in items {
            match item {
                GeneratedStepItem::Valid => self.append_step(indentation, GeneratedStepKind::Valid),
                GeneratedStepItem::Invalid => {
                    self.append_step(indentation, GeneratedStepKind::Invalid);
                }
                GeneratedStepItem::InlineModule(items) => {
                    self.append_inline_module(items, indentation);
                }
            }
        }
    }

    fn append_step(&mut self, indentation: usize, kind: GeneratedStepKind) {
        let name = match kind {
            GeneratedStepKind::Valid => format!("valid_step_{}", self.next_function),
            GeneratedStepKind::Invalid => format!("invalid_step_{}", self.next_function),
        };
        self.next_function += 1;

        match kind {
            GeneratedStepKind::Valid => self.expected_step_names.push(name.clone()),
            GeneratedStepKind::Invalid => self.expected_diagnostic_names.push(name.clone()),
        }

        self.append_indentation(indentation);
        match kind {
            GeneratedStepKind::Valid => self.source.push_str("#[given(\"valid\")]\n"),
            GeneratedStepKind::Invalid => {
                self.source.push_str("#[given(\"first\")]\n");
                self.append_indentation(indentation);
                self.source.push_str("#[when(\"second\")]\n");
            }
        }
        self.append_indentation(indentation);
        self.source.push_str("fn ");
        self.source.push_str(&name);
        self.source.push_str("() {}\n");
    }

    fn append_inline_module(&mut self, items: &[GeneratedStepItem], indentation: usize) {
        let name = format!("module_{}", self.next_module);
        self.next_module += 1;
        self.append_indentation(indentation);
        self.source.push_str("mod ");
        self.source.push_str(&name);
        self.source.push_str(" {\n");
        self.append_items(items, indentation + 4);
        self.append_indentation(indentation);
        self.source.push_str("}\n");
    }

    fn append_indentation(&mut self, indentation: usize) {
        self.source.push_str(&" ".repeat(indentation));
    }
}

fn generated_step_items() -> impl Strategy<Value = Vec<GeneratedStepItem>> {
    let leaf = prop_oneof![
        Just(GeneratedStepItem::Valid),
        Just(GeneratedStepItem::Invalid),
    ];
    prop::collection::vec(
        leaf.prop_recursive(2, 16, 3, |inner| {
            prop::collection::vec(inner, 1..=3).prop_map(GeneratedStepItem::InlineModule)
        }),
        1..=8,
    )
}

proptest! {
    #[test]
    fn preserves_valid_steps_and_diagnostics_in_traversal_order(
        items in generated_step_items(),
    ) {
        let mut generated = GeneratedRustSource::default();
        generated.append_items(&items, 0);
        let GeneratedRustSource {
            source,
            expected_step_names,
            expected_diagnostic_names,
            ..
        } = generated;

        let result = index_rust_source(PathBuf::from("generated.rs"), &source)
            .expect("generated source is valid Rust");
        let indexed_step_names: Vec<_> = result
            .index
            .step_definitions
            .iter()
            .map(|step| step.function.name.clone())
            .collect();
        let diagnostic_names: Vec<_> = result
            .diagnostics
            .iter()
            .filter_map(|diagnostic| match diagnostic {
                RustStepIndexDiagnostic::MultipleStepAttributes { function } => {
                    Some(function.clone())
                }
                RustStepIndexDiagnostic::InvalidStepAttributeArguments { .. } => None,
            })
            .collect();

        prop_assert_eq!(indexed_step_names, expected_step_names);
        prop_assert_eq!(diagnostic_names, expected_diagnostic_names);
    }
}

#[rstest]
#[case(
    concat!(
        "use rstest_bdd_macros::given;\n",
        "\n",
        "#[given(123)]\n",
        "fn invalid_args() {}\n",
        "\n",
        "#[given(\"valid\")]\n",
        "fn valid_step() {}\n",
    ),
    "invalid_args"
)]
#[case(
    concat!(
        "use rstest_bdd_macros::given;\n",
        "\n",
        "#[given(foo = 42)]\n",
        "fn invalid_named_args() {}\n",
        "\n",
        "#[given(\"valid\")]\n",
        "fn valid_step() {}\n",
    ),
    "invalid_named_args"
)]
fn reports_invalid_step_attribute_arguments_without_discarding_valid_steps(
    #[case] source: &'static str,
    #[case] expected_function: &'static str,
) {
    let result = index_rust_source(PathBuf::from("steps.rs"), source)
        .expect("source syntax should remain indexable");

    let indexed_names: Vec<_> = result
        .index
        .step_definitions
        .iter()
        .map(|step| step.function.name.as_str())
        .collect();
    assert_eq!(indexed_names, ["valid_step"]);

    match result.diagnostics.as_slice() {
        [
            RustStepIndexDiagnostic::InvalidStepAttributeArguments {
                function,
                attribute,
                message,
            },
        ] => {
            assert_eq!(function, expected_function);
            assert_eq!(*attribute, "given");
            assert!(
                !message.trim().is_empty(),
                "expected an explanatory error message"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
