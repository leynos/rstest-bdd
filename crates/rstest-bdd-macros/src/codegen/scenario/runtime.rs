//! Helpers that generate the runtime scaffolding for scenario tests.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::codegen::scenario::{FeaturePath, ScenarioName};

/// Grouped tokens for scenario steps.
pub(crate) struct ProcessedSteps {
    pub(crate) keyword_tokens: Vec<TokenStream2>,
    pub(crate) values: Vec<TokenStream2>,
    pub(crate) docstrings: Vec<TokenStream2>,
    pub(crate) tables: Vec<TokenStream2>,
}

/// Configuration for generating test tokens.
pub(crate) struct TestTokensConfig<'a> {
    pub(crate) processed_steps: ProcessedSteps,
    pub(crate) feature_path: &'a FeaturePath,
    pub(crate) scenario_name: &'a ScenarioName,
    pub(crate) scenario_line: u32,
    pub(crate) tags: &'a [String],
    pub(crate) block: &'a syn::Block,
    pub(crate) allow_skipped: bool,
}

pub(crate) fn execute_single_step(
    feature_path: &FeaturePath,
    scenario_name: &ScenarioName,
) -> TokenStream2 {
    let _ = feature_path.as_str();
    let _ = scenario_name.as_str();
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        #[expect(
            clippy::too_many_arguments,
            reason = "helper mirrors generated step inputs to keep panic messaging intact",
        )]
        fn execute_single_step(
            index: usize,
            keyword: #path::StepKeyword,
            text: &str,
            docstring: Option<&str>,
            table: Option<&[&[&str]]>,
            ctx: &mut #path::StepContext,
            feature_path: &str,
            scenario_name: &str,
        ) -> Result<Option<Box<dyn std::any::Any>>, String> {
            if let Some(f) = #path::find_step(keyword, text.into()) {
                match f(ctx, text, docstring, table) {
                    Ok(#path::StepExecution::Continue { value }) => Ok(value),
                    Ok(#path::StepExecution::Skipped { message }) => {
                        let encoded = message.map_or_else(
                            || SKIP_NONE_PREFIX.to_string(),
                            |msg| {
                                let mut encoded = String::with_capacity(1 + msg.len());
                                encoded.push(SKIP_SOME_PREFIX);
                                encoded.push_str(&msg);
                                encoded
                            },
                        );
                        Err(encoded)
                    }
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

fn generate_skip_decoder() -> TokenStream2 {
    quote! {
        fn decode_skip_message(encoded: String) -> Option<String> {
            match encoded.chars().next() {
                Some(prefix) if prefix == SKIP_NONE_PREFIX => None,
                Some(prefix) if prefix == SKIP_SOME_PREFIX => {
                    let prefix_len = prefix.len_utf8();
                    Some(encoded[prefix_len..].to_string())
                }
                _ => Some(encoded),
            }
        }
    }
}

fn generate_scenario_guard() -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        struct ScenarioReportGuard {
            recorded: bool,
            feature_path: &'static str,
            scenario_name: &'static str,
            line: u32,
            tags: &'static [&'static str],
        }

        impl ScenarioReportGuard {
            fn new(
                feature_path: &'static str,
                scenario_name: &'static str,
                line: u32,
                tags: &'static [&'static str],
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
        }

        impl Drop for ScenarioReportGuard {
            fn drop(&mut self) {
                if !self.recorded && !std::thread::panicking() {
                    #path::reporting::record(#path::reporting::ScenarioRecord::new(
                        self.feature_path,
                        self.scenario_name,
                        self.line,
                        self.tags
                            .iter()
                            .map(|tag| tag.to_string())
                            .collect::<Vec<_>>(),
                        #path::reporting::ScenarioStatus::Passed,
                    ));
                }
            }
        }
    }
}

pub(crate) fn generate_step_executor_loop(
    keyword_tokens: &[TokenStream2],
    values: &[TokenStream2],
    docstrings: &[TokenStream2],
    tables: &[TokenStream2],
) -> TokenStream2 {
    quote! {
        let steps = [#((#keyword_tokens, #values, #docstrings, #tables)),*];
        for (index, (keyword, text, docstring, table)) in steps.iter().enumerate() {
            match execute_single_step(
                index,
                *keyword,
                *text,
                *docstring,
                *table,
                &mut ctx,
                FEATURE_PATH,
                SCENARIO_NAME,
            ) {
                Ok(value) => {
                    if let Some(val) = value {
                        let _ = ctx.insert_value(val);
                    }
                }
                Err(encoded) => {
                    skipped = Some(decode_skip_message(encoded));
                    skipped_at = Some(index);
                    break;
                }
            }
        }
    }
}

fn generate_skip_handler() -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        if let Some(message) = skipped {
            let fail_on_skipped_enabled = #path::config::fail_on_skipped();
            let forced_failure = fail_on_skipped_enabled && !allow_skipped;
            if #path::diagnostics_enabled() {
                if let Some(start) = skipped_at {
                    let bypassed = steps
                        .iter()
                        .enumerate()
                        .skip(start + 1)
                        .map(|(_, (keyword, text, _, _))| (*keyword, *text));
                    #path::record_bypassed_steps(
                        FEATURE_PATH,
                        SCENARIO_NAME,
                        SCENARIO_LINE,
                        SCENARIO_TAGS
                            .iter()
                            .map(|tag| tag.to_string())
                            .collect::<Vec<_>>(),
                        message.as_deref(),
                        bypassed,
                    );
                }
            }
            scenario_guard.mark_recorded();
            let skip_details = #path::reporting::SkippedScenario::new(
                message.clone(),
                allow_skipped,
                forced_failure,
            );
            #path::reporting::record(#path::reporting::ScenarioRecord::new(
                FEATURE_PATH,
                SCENARIO_NAME,
                SCENARIO_LINE,
                SCENARIO_TAGS
                    .iter()
                    .map(|tag| tag.to_string())
                    .collect::<Vec<_>>(),
                #path::reporting::ScenarioStatus::Skipped(skip_details),
            ));
            if forced_failure {
                let detail = message.unwrap_or_else(|| "scenario skipped".to_string());
                panic!(
                    "Scenario skipped with fail_on_skipped enabled: {}\n(feature: {}, scenario: {})",
                    detail,
                    FEATURE_PATH,
                    SCENARIO_NAME
                );
            }
            return;
        }
    }
}

pub(crate) fn generate_test_tokens(
    config: TestTokensConfig<'_>,
    ctx_prelude: impl Iterator<Item = TokenStream2>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
    ctx_postlude: impl Iterator<Item = TokenStream2>,
) -> TokenStream2 {
    let TestTokensConfig {
        processed_steps:
            ProcessedSteps {
                keyword_tokens,
                values,
                docstrings,
                tables,
            },
        feature_path,
        scenario_name,
        scenario_line,
        tags,
        block,
        allow_skipped,
    } = config;
    let ctx_prelude: Vec<_> = ctx_prelude.collect();
    let ctx_inserts: Vec<_> = ctx_inserts.collect();
    let ctx_postlude: Vec<_> = ctx_postlude.collect();

    let path = crate::codegen::rstest_bdd_path();
    let allow_literal = syn::LitBool::new(allow_skipped, proc_macro2::Span::call_site());
    let feature_literal = syn::LitStr::new(feature_path.as_str(), proc_macro2::Span::call_site());
    let scenario_literal = syn::LitStr::new(scenario_name.as_str(), proc_macro2::Span::call_site());
    let scenario_line_literal =
        syn::LitInt::new(&scenario_line.to_string(), proc_macro2::Span::call_site());
    let tag_literals: Vec<_> = tags
        .iter()
        .map(|tag| syn::LitStr::new(tag, proc_macro2::Span::call_site()))
        .collect();
    let step_executor = execute_single_step(feature_path, scenario_name);
    let skip_decoder = generate_skip_decoder();
    let scenario_guard = generate_scenario_guard();
    let step_executor_loop =
        generate_step_executor_loop(&keyword_tokens, &values, &docstrings, &tables);
    let skip_handler = generate_skip_handler();
    quote! {
        const FEATURE_PATH: &str = #feature_literal;
        const SCENARIO_NAME: &str = #scenario_literal;
        const SCENARIO_LINE: u32 = #scenario_line_literal;
        const SCENARIO_TAGS: &[&str] = &[#(#tag_literals),*];
        const SKIP_NONE_PREFIX: char = '\u{0}';
        const SKIP_SOME_PREFIX: char = '\u{1}';
        #step_executor
        #skip_decoder
        #scenario_guard

        let allow_skipped: bool = #allow_literal;
        #(#ctx_prelude)*
        let mut ctx = {
            let mut ctx = #path::StepContext::default();
            #(#ctx_inserts)*
            ctx
        };

        let mut scenario_guard = ScenarioReportGuard::new(
            FEATURE_PATH,
            SCENARIO_NAME,
            SCENARIO_LINE,
            SCENARIO_TAGS,
        );
        let mut skipped: Option<Option<String>> = None;
        let mut skipped_at: Option<usize> = None;
        #step_executor_loop
        #skip_handler
        #(#ctx_postlude)*
        #block
    }
}
