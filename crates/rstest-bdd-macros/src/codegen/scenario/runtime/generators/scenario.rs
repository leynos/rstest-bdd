//! Scenario-level code generators.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::return_classifier::ReturnKind;

/// Generates the `__RstestBddScenarioReportGuard` struct that records scenario
/// results on drop if not already recorded.
///
/// The generated struct implements a drop guard that records passed scenarios
/// automatically when the test completes without explicit recording.
///
/// # Generated code
///
/// ```text
/// struct __RstestBddScenarioReportGuard {
///     recorded: bool,
///     feature_path: &'static str,
///     scenario_name: &'static str,
///     line: u32,
///     tags: ScenarioTags,
/// }
///
/// impl __RstestBddScenarioReportGuard {
///     fn new(...) -> Self { ... }
///     fn mark_recorded(&mut self) { ... }
///     fn tags(&self) -> &[String] { ... }
///     fn take_tags(&mut self) -> ScenarioTags { ... }
/// }
///
/// impl Drop for __RstestBddScenarioReportGuard {
///     fn drop(&mut self) { /* record if not already recorded */ }
/// }
/// ```
pub(in crate::codegen::scenario::runtime) fn generate_scenario_guard() -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        struct __RstestBddScenarioReportGuard {
            recorded: bool,
            feature_path: &'static str,
            scenario_name: &'static str,
            line: u32,
            tags: #path::reporting::ScenarioTags,
        }

        impl __RstestBddScenarioReportGuard {
            fn new(
                feature_path: &'static str,
                scenario_name: &'static str,
                line: u32,
                tags: #path::reporting::ScenarioTags,
            ) -> Self {
                Self {
                    recorded: false,
                    feature_path,
                    scenario_name,
                    line,
                    tags,
                }
            }

            fn mark_recorded(&mut self) {
                self.recorded = true;
            }

            fn tags(&self) -> &[String] {
                self.tags.as_ref()
            }

            fn take_tags(&mut self) -> #path::reporting::ScenarioTags {
                std::mem::take(&mut self.tags)
            }
        }

        impl Drop for __RstestBddScenarioReportGuard {
            fn drop(&mut self) {
                if !self.recorded && !std::thread::panicking() {
                    let tags = self.take_tags();
                    let metadata = #path::reporting::ScenarioMetadata::new(
                        self.feature_path,
                        self.scenario_name,
                        self.line,
                        tags,
                    );
                    #path::reporting::record(#path::reporting::ScenarioRecord::from_metadata(
                        metadata,
                        #path::reporting::ScenarioStatus::Passed,
                    ));
                }
            }
        }
    }
}

/// Generates the skip handler code that processes skipped scenarios.
///
/// The generated code handles scenarios that were skipped during execution,
/// recording bypassed steps, updating the scenario guard, and optionally
/// panicking if `fail_on_skipped` is enabled without `@allow-skipped`.
///
/// # Generated code
///
/// ```text
/// if let Some(message) = __rstest_bdd_skipped {
///     // Check fail_on_skipped configuration
///     // Record bypassed steps if diagnostics enabled
///     // Mark scenario as recorded
///     // Record skip status
///     // Panic if forced failure
/// }
/// ```
pub(in crate::codegen::scenario::runtime) fn generate_skip_handler(
    return_kind: ReturnKind,
) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    let return_stmt = match return_kind {
        ReturnKind::ResultUnit | ReturnKind::ResultValue => quote! { return Ok(()); },
        _ => quote! { return; },
    };
    quote! {
        if let Some(__rstest_bdd_message) = __rstest_bdd_skipped {
            let __rstest_bdd_fail_on_skipped_enabled = #path::config::fail_on_skipped();
            let __rstest_bdd_forced_failure = __rstest_bdd_fail_on_skipped_enabled && !__rstest_bdd_allow_skipped;
            if #path::diagnostics_enabled() {
                if let Some(__rstest_bdd_start) = __rstest_bdd_skipped_at {
                    let __rstest_bdd_bypassed = __rstest_bdd_steps
                        .iter()
                        .enumerate()
                        .skip(__rstest_bdd_start + 1)
                        .map(|(_, (__rstest_bdd_kw, __rstest_bdd_txt, _, _))| (*__rstest_bdd_kw, *__rstest_bdd_txt));
                    #path::record_bypassed_steps_with_tags(
                        __RSTEST_BDD_FEATURE_PATH,
                        __RSTEST_BDD_SCENARIO_NAME,
                        __RSTEST_BDD_SCENARIO_LINE,
                        __rstest_bdd_scenario_guard.tags(),
                        __rstest_bdd_message.as_deref(),
                        __rstest_bdd_bypassed,
                    );
                }
            }
            __rstest_bdd_scenario_guard.mark_recorded();
            let __rstest_bdd_scenario_tags_owned = __rstest_bdd_scenario_guard.take_tags();
            let __rstest_bdd_skip_details = #path::reporting::SkippedScenario::new(
                __rstest_bdd_message.clone(),
                __rstest_bdd_allow_skipped,
                __rstest_bdd_forced_failure,
            );
            let __rstest_bdd_metadata = #path::reporting::ScenarioMetadata::new(
                __RSTEST_BDD_FEATURE_PATH,
                __RSTEST_BDD_SCENARIO_NAME,
                    __RSTEST_BDD_SCENARIO_LINE,
                    __rstest_bdd_scenario_tags_owned,
                );
            #path::reporting::record(#path::reporting::ScenarioRecord::from_metadata(
                __rstest_bdd_metadata,
                #path::reporting::ScenarioStatus::Skipped(__rstest_bdd_skip_details),
            ));
            if __rstest_bdd_forced_failure {
                let __rstest_bdd_detail = __rstest_bdd_message.unwrap_or_else(|| "scenario skipped".to_string());
                panic!(
                    "Scenario skipped with fail_on_skipped enabled: {}\n(feature: {}, scenario: {})",
                    __rstest_bdd_detail,
                    __RSTEST_BDD_FEATURE_PATH,
                    __RSTEST_BDD_SCENARIO_NAME
                );
            }
            #return_stmt
        }
    }
}
