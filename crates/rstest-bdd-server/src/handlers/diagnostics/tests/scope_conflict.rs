//! Publication boundary tests for conflicting closed step-library scopes.
//!
//! Scope resolution is a pure query, so a read must never log. The warning is
//! published once by the indexing mutation that observes the conflict. These
//! tests pin both halves of that contract: the read paths stay silent, and a
//! repeated observation of the same conflict does not republish.
//!
//! `tracing-subscriber` is a normal dependency of this crate rather than a
//! test-only capture helper, so the harness is built from the `MakeWriter`
//! trait directly.

use std::{
    io,
    path::Path,
    sync::{Arc, Mutex},
};

use tracing_subscriber::fmt::writer::MakeWriter;

use super::*;
use crate::{
    handlers::{handle_definition, handle_implementation},
    lsp::{
        GotoDefinitionParams,
        PartialResultParams,
        Position,
        TextDocumentIdentifier,
        TextDocumentPositionParams,
        Url,
        WorkDoneProgressParams,
    },
    test_support::{TestScenario, index_file, write_workspace_file},
};

/// Substring that identifies the scope-conflict warning.
const CONFLICT_MESSAGE: &str = "conflicting closed step-library scopes";

/// Feature file whose bindings the two Rust sources disagree about.
const FEATURE_SOURCE: &str = "Feature: test\n  Scenario: s\n    Given the domain is empty\n";

/// Rust source that binds [`FEATURE_SOURCE`] to the `accounts` library scope.
const ACCOUNTS_SOURCE: &str = concat!(
    "#[step_library]\n",
    "mod accounts {\n",
    "    #[given(\"the domain is empty\")]\n",
    "    fn empty() {}\n",
    "}\n",
    "#[scenario(path = \"test.feature\", libraries = [accounts])]\n",
    "fn bind() {}\n",
);

/// Rust source that binds the same feature to the `filesystem` scope.
///
/// Saving this after [`ACCOUNTS_SOURCE`] is what creates the conflict.
const FILESYSTEM_SOURCE: &str = concat!(
    "#[step_library]\n",
    "mod filesystem {\n",
    "    #[given(\"the domain is empty\")]\n",
    "    fn empty() {}\n",
    "}\n",
    "#[scenario(path = \"test.feature\", libraries = [filesystem])]\n",
    "fn bind() {}\n",
);

/// Collects everything a `tracing` subscriber writes for one scope.
#[derive(Clone, Default)]
struct WarningSink {
    /// Bytes written by every writer handed out for this sink.
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl WarningSink {
    /// Return everything captured so far as lossy UTF-8.
    fn captured(&self) -> String {
        let bytes = self
            .buffer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Count how many captured lines mention the scope conflict.
    fn conflict_count(&self) -> usize { self.captured().matches(CONFLICT_MESSAGE).count() }
}

/// One write handle onto a [`WarningSink`].
struct SinkWriter {
    /// Shared buffer the enclosing sink reads.
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl io::Write for SinkWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // A poisoned lock still holds every byte written before the panic, and
        // this is the test's own observation channel, so recover it rather than
        // fail the assertion for an unrelated reason.
        self.buffer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

impl<'a> MakeWriter<'a> for WarningSink {
    type Writer = SinkWriter;

    fn make_writer(&'a self) -> Self::Writer {
        SinkWriter {
            buffer: Arc::clone(&self.buffer),
        }
    }
}

/// Run `body` with every warning captured into `sink`.
fn with_captured_warnings<T>(sink: &WarningSink, body: impl FnOnce() -> T) -> T {
    // Colour codes would interleave with the field names this test asserts on.
    let subscriber = tracing_subscriber::fmt()
        .with_writer(sink.clone())
        .with_ansi(false)
        .without_time()
        .with_max_level(tracing::Level::WARN)
        .finish();
    tracing::subscriber::with_default(subscriber, body)
}

/// Build a scenario whose feature is bound to exactly one library scope.
fn single_scope_scenario(scenario_builder: ScenarioBuilder) -> TestScenario {
    scenario_builder
        .with_feature("test.feature", FEATURE_SOURCE)
        .with_rust_steps("accounts.rs", ACCOUNTS_SOURCE)
        .build()
}

/// Write and index the second binding, creating the conflict.
///
/// # Panics
///
/// Panics if the source cannot be written to the test workspace.
fn add_conflicting_binding(scenario: &mut TestScenario) {
    let path = write_workspace_file(&scenario.dir, "filesystem.rs", FILESYSTEM_SOURCE);
    index_file(&mut scenario.state, &path);
}

/// Return the cursor position on the first step of one Rust file.
///
/// # Panics
///
/// Panics when the file contributed no compiled step, which would mean the
/// scenario setup failed rather than that the case under test failed.
fn first_step_position(scenario: &TestScenario, rust_path: &Path) -> Position {
    let Some(step) = scenario
        .state
        .step_registry()
        .steps_for_file(rust_path)
        .first()
    else {
        panic!("no compiled step for {}", rust_path.display());
    };
    Position::new(step.attribute_span.function_line, 0)
}

/// Build the parameters for a go-to-definition request.
///
/// In the `lsp-types` version `async-lsp` resolves,
/// `GotoImplementationParams = GotoTypeDefinitionParams = GotoDefinitionParams`
/// are all aliases of one struct, so a single constructor serves both requests.
fn goto_params(uri: Url, position: Position) -> GotoDefinitionParams {
    GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    }
}

#[rstest]
fn read_only_queries_stay_silent_about_scope_conflicts(scenario_builder: ScenarioBuilder) {
    let mut scenario = single_scope_scenario(scenario_builder);
    let accounts_path = scenario.dir.path().join("accounts.rs");
    let feature_path = scenario.dir.path().join("test.feature");
    let sink = WarningSink::default();

    with_captured_warnings(&sink, || {
        // Establish the conflict inside the capture so its baseline is known.
        add_conflicting_binding(&mut scenario);
        let baseline = sink.conflict_count();
        assert_eq!(baseline, 1, "the indexing mutation must publish once");

        let Ok(accounts_uri) = Url::from_file_path(&accounts_path) else {
            panic!("accounts source path is not a file URI");
        };
        let Ok(feature_uri) = Url::from_file_path(&feature_path) else {
            panic!("feature path is not a file URI");
        };
        let position = first_step_position(&scenario, &accounts_path);

        let definition = handle_definition(&scenario.state, &goto_params(accounts_uri, position));
        assert!(
            definition.is_ok(),
            "definition query failed: {definition:?}"
        );

        let implementation = handle_implementation(
            &scenario.state,
            &goto_params(feature_uri, Position::new(2, 4)),
        );
        assert!(
            implementation.is_ok(),
            "implementation query failed: {implementation:?}"
        );

        let Some(feature_index) = scenario.state.feature_index(&feature_path) else {
            panic!("feature index missing");
        };
        let feature_diagnostics =
            compute_unimplemented_step_diagnostics(&scenario.state, feature_index);
        assert!(
            !feature_diagnostics.is_empty(),
            "a conflicting scope must leave the feature step unresolved"
        );

        let rust_diagnostics = compute_unused_step_diagnostics(&scenario.state, &accounts_path);
        assert!(
            !rust_diagnostics.is_empty(),
            "a conflicting scope must not resolve Rust definitions as used"
        );

        assert_eq!(
            sink.conflict_count(),
            baseline,
            "read-only queries must not publish scope warnings, captured: {}",
            sink.captured()
        );
    });
}

#[rstest]
fn indexing_mutation_publishes_one_structured_warning(scenario_builder: ScenarioBuilder) {
    let mut scenario = single_scope_scenario(scenario_builder);
    let sink = WarningSink::default();

    with_captured_warnings(&sink, || {
        add_conflicting_binding(&mut scenario);
    });

    let captured = sink.captured();
    assert_eq!(
        captured.matches(CONFLICT_MESSAGE).count(),
        1,
        "one conflict must publish exactly one warning, captured: {captured}"
    );
    assert!(
        captured.contains("resolve-feature-step-scope"),
        "the warning must carry its structured operation field, captured: {captured}"
    );
    assert!(
        captured.contains("conflicting-closed-scopes"),
        "the warning must carry its structured failure category, captured: {captured}"
    );
    assert!(
        captured.contains("test.feature"),
        "the warning must name the affected feature file, captured: {captured}"
    );
}

#[rstest]
fn repeated_observation_does_not_republish(scenario_builder: ScenarioBuilder) {
    let mut scenario = single_scope_scenario(scenario_builder);
    let sink = WarningSink::default();

    with_captured_warnings(&sink, || {
        add_conflicting_binding(&mut scenario);
        scenario.state.report_indexed_scope_conflicts();
        scenario.state.report_indexed_scope_conflicts();
        scenario.state.report_indexed_scope_conflicts();
    });

    assert_eq!(
        sink.conflict_count(),
        1,
        "re-observing an unchanged conflict must stay silent, captured: {}",
        sink.captured()
    );
}
