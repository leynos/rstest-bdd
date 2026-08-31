//! Property tests for tolerant Rust step-definition indexing.

use proptest::prelude::*;

use super::super::*;

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
        Just(GeneratedStepItem::Invalid)
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
    fn preserves_valid_steps_and_diagnostics_in_traversal_order(items in generated_step_items()) {
        let mut generated = GeneratedRustSource::default();
        generated.append_items(&items, 0);
        let GeneratedRustSource { source, expected_step_names, expected_diagnostic_names, .. } = generated;
        let result = index_rust_source(PathBuf::from("generated.rs"), &source)
            .expect("generated source is valid Rust");
        let indexed_step_names: Vec<_> = result.index.step_definitions.iter()
            .map(|step| step.function.name.clone()).collect();
        let diagnostic_names: Vec<_> = result.diagnostics.iter().filter_map(|diagnostic| match diagnostic {
            RustStepIndexDiagnostic::MultipleStepAttributes { function } => Some(function.clone()),
            RustStepIndexDiagnostic::InvalidStepAttributeArguments { .. } => None,
        }).collect();
        prop_assert_eq!(indexed_step_names, expected_step_names);
        prop_assert_eq!(diagnostic_names, expected_diagnostic_names);
    }
}
