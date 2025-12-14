//! Helpers that generate the runtime scaffolding for scenario tests.

mod types;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use types::{CodeComponents, ScenarioLiterals, ScenarioLiteralsInput, TokenAssemblyContext};
pub(crate) use types::{ProcessedSteps, TestTokensConfig};

pub(crate) fn execute_single_step() -> TokenStream2 {
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
            tags: #path::reporting::ScenarioTags,
        }

        impl ScenarioReportGuard {
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

        impl Drop for ScenarioReportGuard {
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

pub(crate) fn generate_step_executor_loop(
    keyword_tokens: &[TokenStream2],
    values: &[TokenStream2],
    docstrings: &[TokenStream2],
    tables: &[TokenStream2],
) -> TokenStream2 {
    quote! {
        let steps = [#((#keyword_tokens, #values, #docstrings, #tables)),*];
        for (index, (keyword, text, docstring, table)) in steps.iter().copied().enumerate() {
            match execute_single_step(
                index,
                keyword,
                text,
                docstring,
                table,
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
                    #path::record_bypassed_steps_with_tags(
                        FEATURE_PATH,
                        SCENARIO_NAME,
                        SCENARIO_LINE,
                        scenario_guard.tags(),
                        message.as_deref(),
                        bypassed,
                    );
                }
            }
            scenario_guard.mark_recorded();
            let scenario_tags_owned = scenario_guard.take_tags();
            let skip_details = #path::reporting::SkippedScenario::new(
                message.clone(),
                allow_skipped,
                forced_failure,
            );
            let metadata = #path::reporting::ScenarioMetadata::new(
                FEATURE_PATH,
                SCENARIO_NAME,
                SCENARIO_LINE,
                scenario_tags_owned,
            );
            #path::reporting::record(#path::reporting::ScenarioRecord::from_metadata(
                metadata,
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

fn create_scenario_literals(input: ScenarioLiteralsInput<'_>) -> ScenarioLiterals {
    let allow_literal = syn::LitBool::new(input.allow_skipped, proc_macro2::Span::call_site());
    let feature_literal =
        syn::LitStr::new(input.feature_path.as_str(), proc_macro2::Span::call_site());
    let scenario_literal =
        syn::LitStr::new(input.scenario_name.as_str(), proc_macro2::Span::call_site());
    let scenario_line_literal = syn::LitInt::new(
        &input.scenario_line.to_string(),
        proc_macro2::Span::call_site(),
    );
    let tag_literals = input
        .tags
        .iter()
        .map(|tag| syn::LitStr::new(tag, proc_macro2::Span::call_site()))
        .collect();

    ScenarioLiterals {
        allow_literal,
        feature_literal,
        scenario_literal,
        scenario_line_literal,
        tag_literals,
    }
}

fn generate_code_components(processed_steps: &ProcessedSteps) -> CodeComponents {
    let ProcessedSteps {
        keyword_tokens,
        values,
        docstrings,
        tables,
    } = processed_steps;

    let step_executor = execute_single_step();
    let skip_decoder = generate_skip_decoder();
    let scenario_guard = generate_scenario_guard();
    let step_executor_loop =
        generate_step_executor_loop(keyword_tokens, values, docstrings, tables);
    let skip_handler = generate_skip_handler();

    CodeComponents {
        step_executor,
        skip_decoder,
        scenario_guard,
        step_executor_loop,
        skip_handler,
    }
}

pub(crate) fn generate_test_tokens(
    config: TestTokensConfig<'_>,
    ctx_prelude: impl Iterator<Item = TokenStream2>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
    ctx_postlude: impl Iterator<Item = TokenStream2>,
) -> TokenStream2 {
    let TestTokensConfig {
        processed_steps,
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

    let literals = create_scenario_literals(ScenarioLiteralsInput {
        feature_path,
        scenario_name,
        scenario_line,
        tags,
        allow_skipped,
    });

    let components = generate_code_components(&processed_steps);
    let block_tokens = quote! { #block };
    let context =
        TokenAssemblyContext::new(&ctx_prelude, &ctx_inserts, &ctx_postlude, &block_tokens);

    assemble_test_tokens(literals, components, context)
}

fn assemble_test_tokens(
    literals: ScenarioLiterals,
    components: CodeComponents,
    context: TokenAssemblyContext<'_>,
) -> TokenStream2 {
    let TokenAssemblyContext {
        ctx_prelude,
        ctx_inserts,
        ctx_postlude,
        block,
    } = context;
    let ScenarioLiterals {
        allow_literal,
        feature_literal,
        scenario_literal,
        scenario_line_literal,
        tag_literals,
    } = literals;

    let CodeComponents {
        step_executor,
        skip_decoder,
        scenario_guard,
        step_executor_loop,
        skip_handler,
    } = components;

    let path = crate::codegen::rstest_bdd_path();
    quote! {
        const FEATURE_PATH: &str = #feature_literal;
        const SCENARIO_NAME: &str = #scenario_literal;
        const SCENARIO_LINE: u32 = #scenario_line_literal;
        static SCENARIO_TAGS: std::sync::LazyLock<#path::reporting::ScenarioTags> =
            std::sync::LazyLock::new(|| {
                std::sync::Arc::<[String]>::from(vec![#(#tag_literals.to_string()),*])
            });
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
            SCENARIO_TAGS.clone(),
        );
        let mut skipped: Option<Option<String>> = None;
        let mut skipped_at: Option<usize> = None;
        #step_executor_loop
        #skip_handler
        #(#ctx_postlude)*
        #block
    }
}
