//! Tests for runtime scaffolding code generation.

use super::generators::{
    generate_async_step_executor, generate_skip_decoder, generate_step_executor,
};
use syn::visit::Visit;

/// Return the identifier of the final segment in a `syn::Path`.
///
/// Returns `None` when the path has no segments (for example, if it was parsed
/// from an empty token stream).
fn path_last_ident(path: &syn::Path) -> Option<&syn::Ident> {
    path.segments.last().map(|seg| &seg.ident)
}

#[expect(clippy::panic, reason = "test helper panics for clearer failures")]
fn extract_path(expr: &syn::Expr) -> &syn::Path {
    match expr {
        syn::Expr::Path(expr_path) => &expr_path.path,
        other => panic!("expected path expression, got {other:?}"),
    }
}

/// Assert that a path ends with `{module}::{function}`.
///
/// This is more robust than string matching as it checks specific path segments
/// rather than substring containment, ensuring the test remains valid even if
/// module paths change as long as the architectural intent is preserved.
#[expect(
    clippy::indexing_slicing,
    reason = "indices are bounds-checked by the preceding assert"
)]
fn assert_path_ends_with_module_function(path: &syn::Path, module: &str, function: &str) {
    let segments: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    let len = segments.len();

    assert!(
        len >= 2,
        "expected path with at least 2 segments ({module}::{function}), got: {segments:?}"
    );

    assert_eq!(
        segments[len - 2],
        module,
        "expected second-to-last segment to be '{module}', got path: {segments:?}"
    );

    assert_eq!(
        segments[len - 1],
        function,
        "expected last segment to be '{function}', got path: {segments:?}"
    );
}

/// Assert that a path ends with `execution::execute_step`.
fn assert_path_is_execution_execute_step(path: &syn::Path) {
    assert_path_ends_with_module_function(path, "execution", "execute_step");
}

/// Assert that a path ends with `execution::decode_skip_message`.
fn assert_path_is_execution_decode_skip_message(path: &syn::Path) {
    assert_path_ends_with_module_function(path, "execution", "decode_skip_message");
}

#[expect(clippy::panic, reason = "test helper panics for clearer failures")]
fn find_function_by_name<'a>(file: &'a syn::File, name: &str) -> &'a syn::ItemFn {
    file.items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(f) if f.sig.ident == name => Some(f),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected {name} function"))
}

struct CallFinder<'ast> {
    name: String,
    found: Option<&'ast syn::ExprCall>,
}

impl<'ast> Visit<'ast> for CallFinder<'ast> {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if self.found.is_some() {
            return;
        }
        if let syn::Expr::Path(expr_path) = node.func.as_ref() {
            if path_last_ident(&expr_path.path)
                .map(syn::Ident::to_string)
                .as_deref()
                == Some(self.name.as_str())
            {
                self.found = Some(node);
                return;
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

fn find_call_in_block<'a>(block: &'a syn::Block, name: &str) -> Option<&'a syn::ExprCall> {
    let mut finder = CallFinder {
        name: name.to_string(),
        found: None,
    };
    finder.visit_block(block);
    finder.found
}

/// Verify that the generated step executor delegates to `rstest_bdd::execution::execute_step`.
///
/// This test validates the architecture where the generated code is a thin wrapper
/// that delegates to runtime functions, rather than containing inline implementation.
#[test]
#[expect(
    clippy::expect_used,
    reason = "test parses generated tokens and uses expect for clearer failures"
)]
fn execute_single_step_delegates_to_runtime() {
    // Parse the generated helper tokens so we can assert on the AST structure,
    // keeping this test resilient to formatting-only changes.
    let file: syn::File =
        syn::parse2(generate_step_executor()).expect("generate_step_executor parses as a file");
    let item = find_function_by_name(&file, "__rstest_bdd_execute_single_step");

    // The generated function should delegate to rstest_bdd::execution::execute_step
    let execute_step_call =
        find_call_in_block(&item.block, "execute_step").expect("expected call to execute_step");
    let func_path = extract_path(execute_step_call.func.as_ref());

    // Assert the path ends with execution::execute_step using segment-based check
    assert_path_is_execution_execute_step(func_path);

    // Verify 2 arguments are passed: the StepExecutionRequest struct reference and ctx
    assert_eq!(
        execute_step_call.args.len(),
        2,
        "execute_step should receive StepExecutionRequest reference and ctx"
    );
}

/// Verify that the generated async step executor delegates to `rstest_bdd::execution::execute_step`.
///
/// This mirrors `execute_single_step_delegates_to_runtime` but for the async helper
/// `__rstest_bdd_process_async_step`, ensuring it stays a thin wrapper over the runtime.
#[test]
#[expect(
    clippy::expect_used,
    reason = "test parses generated tokens and uses expect for clearer failures"
)]
fn execute_async_step_delegates_to_runtime() {
    // Parse the generated async step executor tokens
    let file: syn::File = syn::parse2(generate_async_step_executor())
        .expect("generate_async_step_executor parses as a file");

    let item = find_function_by_name(&file, "__rstest_bdd_process_async_step");

    // The generated function should delegate to rstest_bdd::execution::execute_step
    let execute_step_call =
        find_call_in_block(&item.block, "execute_step").expect("expected call to execute_step");
    let func_path = extract_path(execute_step_call.func.as_ref());

    // Assert the path ends with execution::execute_step using segment-based check
    assert_path_is_execution_execute_step(func_path);

    // Verify 2 arguments are passed: the StepExecutionRequest struct reference and ctx
    assert_eq!(
        execute_step_call.args.len(),
        2,
        "execute_step should receive StepExecutionRequest reference and ctx"
    );
}

/// Verify that the skip decoder delegates to `rstest_bdd::execution::decode_skip_message`.
#[test]
#[expect(
    clippy::expect_used,
    reason = "test parses generated tokens and uses expect for clearer failures"
)]
fn skip_decoder_delegates_to_runtime() {
    let file: syn::File =
        syn::parse2(generate_skip_decoder()).expect("generate_skip_decoder parses as a file");

    let item = find_function_by_name(&file, "__rstest_bdd_decode_skip_message");

    // Verify it calls decode_skip_message from execution module
    let decode_call = find_call_in_block(&item.block, "decode_skip_message")
        .expect("expected call to decode_skip_message");
    let func_path = extract_path(decode_call.func.as_ref());

    // Assert the path ends with execution::decode_skip_message using segment-based check
    assert_path_is_execution_decode_skip_message(func_path);
}
