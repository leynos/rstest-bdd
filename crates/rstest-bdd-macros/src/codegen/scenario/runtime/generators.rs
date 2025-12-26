//! Individual code generators for runtime scaffolding components.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Generates the `__rstest_bdd_execute_single_step` function that looks up
/// and runs a step, handling fixture validation and skip encoding.
#[expect(
    clippy::too_many_lines,
    reason = "single function contains all step execution logic with inlined helpers"
)]
pub(super) fn generate_step_executor() -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        #[expect(
            clippy::too_many_arguments,
            reason = "helper mirrors generated step inputs to keep panic messaging intact",
        )]
        fn __rstest_bdd_execute_single_step(
            index: usize,
            keyword: #path::StepKeyword,
            text: &str,
            docstring: Option<&str>,
            table: Option<&[&[&str]]>,
            ctx: &mut #path::StepContext,
            feature_path: &str,
            scenario_name: &str,
        ) -> Result<Option<Box<dyn std::any::Any>>, String> {
            fn validate_required_fixtures(
                step: &#path::Step,
                ctx: &#path::StepContext,
                text: &str,
                feature_path: &str,
                scenario_name: &str,
            ) {
                if step.fixtures.is_empty() {
                    return;
                }

                let available: std::collections::HashSet<&str> =
                    ctx.available_fixtures().collect();
                let missing: Vec<_> = step.fixtures
                    .iter()
                    .copied()
                    .filter(|f| !available.contains(f))
                    .collect();

                if !missing.is_empty() {
                    let mut available_list: Vec<_> = available.into_iter().collect();
                    available_list.sort_unstable();
                    panic!(
                        concat!(
                            "Step '{}' (defined at {}:{}) requires fixtures {:?}, ",
                            "but the following are missing: {:?}\n",
                            "Available fixtures from scenario: {:?}\n",
                            "(feature: {}, scenario: {})",
                        ),
                        text,
                        step.file,
                        step.line,
                        step.fixtures,
                        missing,
                        available_list,
                        feature_path,
                        scenario_name,
                    );
                }
            }

            fn encode_skip_message(message: Option<String>) -> String {
                message.map_or_else(
                    || SKIP_NONE_PREFIX.to_string(),
                    |msg| {
                        let mut encoded = String::with_capacity(1 + msg.len());
                        encoded.push(SKIP_SOME_PREFIX);
                        encoded.push_str(&msg);
                        encoded
                    },
                )
            }

            fn is_skipped(result: &Result<#path::StepExecution, #path::StepError>) -> bool {
                matches!(result, Ok(#path::StepExecution::Skipped { .. }))
            }

            if let Some(step) = #path::find_step_with_metadata(keyword, #path::StepText::from(text)) {
                validate_required_fixtures(&step, ctx, text, feature_path, scenario_name);

                let result = (step.run)(ctx, text, docstring, table);

                if is_skipped(&result) {
                    if let Ok(#path::StepExecution::Skipped { message }) = result {
                        return Err(encode_skip_message(message));
                    }
                }

                match result {
                    Ok(#path::StepExecution::Continue { value }) => Ok(value),
                    Err(err) => {
                        panic!(
                            "Step failed at index {}: {} {} - {}\n(feature: {}, scenario: {})",
                            index,
                            keyword.as_str(),
                            text,
                            err,
                            feature_path,
                            scenario_name
                        );
                    }
                    // SAFETY: Skipped case handled above via is_skipped predicate
                    Ok(#path::StepExecution::Skipped { .. }) => unreachable!(),
                }
            } else {
                panic!(
                    "Step not found at index {}: {} {} (feature: {}, scenario: {})",
                    index,
                    keyword.as_str(),
                    text,
                    feature_path,
                    scenario_name
                );
            }
        }
    }
}

/// Generates the `__rstest_bdd_decode_skip_message` function that decodes
/// skip messages from their encoded format.
pub(super) fn generate_skip_decoder() -> TokenStream2 {
    quote! {
        fn __rstest_bdd_decode_skip_message(encoded: String) -> Option<String> {
            fn is_skip_none(c: Option<char>) -> bool {
                matches!(c, Some(prefix) if prefix == SKIP_NONE_PREFIX)
            }

            fn is_skip_some(c: Option<char>) -> bool {
                matches!(c, Some(prefix) if prefix == SKIP_SOME_PREFIX)
            }

            let first = encoded.chars().next();

            if is_skip_none(first) {
                return None;
            }

            if is_skip_some(first) {
                let prefix_len = first.expect("checked above").len_utf8();
                return Some(encoded[prefix_len..].to_string());
            }

            Some(encoded)
        }
    }
}

/// Generates the `__RstestBddScenarioReportGuard` struct that records scenario
/// results on drop if not already recorded.
pub(super) fn generate_scenario_guard() -> TokenStream2 {
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

/// Generates the step executor loop that iterates over steps and handles results.
pub(super) fn generate_step_executor_loop(
    keyword_tokens: &[TokenStream2],
    values: &[TokenStream2],
    docstrings: &[TokenStream2],
    tables: &[TokenStream2],
) -> TokenStream2 {
    quote! {
        let __rstest_bdd_steps = [#((#keyword_tokens, #values, #docstrings, #tables)),*];
        for (__rstest_bdd_index, (__rstest_bdd_keyword, __rstest_bdd_text, __rstest_bdd_docstring, __rstest_bdd_table)) in __rstest_bdd_steps.iter().copied().enumerate() {
            match __rstest_bdd_execute_single_step(
                __rstest_bdd_index,
                __rstest_bdd_keyword,
                __rstest_bdd_text,
                __rstest_bdd_docstring,
                __rstest_bdd_table,
                &mut ctx,
                __RSTEST_BDD_FEATURE_PATH,
                __RSTEST_BDD_SCENARIO_NAME,
            ) {
                Ok(__rstest_bdd_value) => {
                    if let Some(__rstest_bdd_val) = __rstest_bdd_value {
                        let _ = ctx.insert_value(__rstest_bdd_val);
                    }
                }
                Err(__rstest_bdd_encoded) => {
                    __rstest_bdd_skipped = Some(__rstest_bdd_decode_skip_message(__rstest_bdd_encoded));
                    __rstest_bdd_skipped_at = Some(__rstest_bdd_index);
                    break;
                }
            }
        }
    }
}

/// Generates the skip handler code that processes skipped scenarios.
pub(super) fn generate_skip_handler() -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
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
            return;
        }
    }
}
