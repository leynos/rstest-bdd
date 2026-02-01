//! Tests for runtime scaffolding code generation.

use super::generators::{
    generate_async_step_executor, generate_skip_decoder, generate_skip_handler,
    generate_step_executor,
};
use crate::codegen::scenario::ScenarioReturnKind;
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

#[expect(clippy::panic, reason = "test helper panics for clearer failures")]
fn parse_skip_handler(return_kind: ScenarioReturnKind) -> syn::ExprIf {
    let stmt: syn::Stmt = syn::parse2(generate_skip_handler(return_kind))
        .unwrap_or_else(|err| panic!("expected skip handler to parse as a statement: {err}"));
    match stmt {
        syn::Stmt::Expr(syn::Expr::If(expr_if), _) => expr_if,
        other => panic!("expected if expression in skip handler, got {other:?}"),
    }
}

struct ReturnFinder<'ast> {
    returns: Vec<&'ast syn::ExprReturn>,
}

impl<'ast> Visit<'ast> for ReturnFinder<'ast> {
    fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
        self.returns.push(node);
    }
}

fn collect_returns(block: &syn::Block) -> Vec<&syn::ExprReturn> {
    let mut finder = ReturnFinder {
        returns: Vec::new(),
    };
    finder.visit_block(block);
    finder.returns
}

fn is_ok_unit_expr(expr: &syn::Expr) -> bool {
    let syn::Expr::Call(call) = expr else {
        return false;
    };
    let path = match call.func.as_ref() {
        syn::Expr::Path(expr_path) => &expr_path.path,
        _ => return false,
    };
    if path_last_ident(path).map(syn::Ident::to_string).as_deref() != Some("Ok") {
        return false;
    }
    if call.args.len() != 1 {
        return false;
    }
    matches!(call.args.first(), Some(syn::Expr::Tuple(tuple)) if tuple.elems.is_empty())
}

/// Assert that generated step executor code delegates to `rstest_bdd::execution::execute_step`.
///
/// This helper validates the architecture where generated code is a thin wrapper
/// that delegates to runtime functions, rather than containing inline implementation.
///
/// # Arguments
///
/// * `tokens` - The generated token stream to parse
/// * `function_name` - The name of the function to find in the generated code
/// * `description` - A human-readable description for error messages
#[expect(
    clippy::panic,
    reason = "test helper panics for clearer failure messages"
)]
fn assert_step_executor_delegates_to_runtime(
    tokens: proc_macro2::TokenStream,
    function_name: &str,
    description: &str,
) {
    let file: syn::File = syn::parse2(tokens)
        .unwrap_or_else(|e| panic!("{description}: failed to parse tokens: {e}"));

    let item = find_function_by_name(&file, function_name);

    let execute_step_call = find_call_in_block(&item.block, "execute_step")
        .unwrap_or_else(|| panic!("{description}: expected call to execute_step"));

    let func_path = extract_path(execute_step_call.func.as_ref());
    assert_path_is_execution_execute_step(func_path);

    assert_eq!(
        execute_step_call.args.len(),
        2,
        "{description}: execute_step should receive StepExecutionRequest reference and ctx"
    );
}

/// Verify that the generated step executor delegates to `rstest_bdd::execution::execute_step`.
///
/// This test validates the architecture where the generated code is a thin wrapper
/// that delegates to runtime functions, rather than containing inline implementation.
#[test]
fn execute_single_step_delegates_to_runtime() {
    assert_step_executor_delegates_to_runtime(
        generate_step_executor(),
        "__rstest_bdd_execute_single_step",
        "sync step executor",
    );
}

/// Verify that the generated async step executor delegates to `rstest_bdd::execution::execute_step`.
///
/// This mirrors `execute_single_step_delegates_to_runtime` but for the async helper
/// `__rstest_bdd_process_async_step`, ensuring it stays a thin wrapper over the runtime.
#[test]
fn execute_async_step_delegates_to_runtime() {
    assert_step_executor_delegates_to_runtime(
        generate_async_step_executor(),
        "__rstest_bdd_process_async_step",
        "async step executor",
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

fn assert_skip_handler_returns(
    return_kind: ScenarioReturnKind,
    empty_message: &str,
    predicate: impl Fn(&syn::ExprReturn) -> bool,
    predicate_message: &str,
) {
    let if_expr = parse_skip_handler(return_kind);
    let returns = collect_returns(&if_expr.then_branch);
    assert!(!returns.is_empty(), "{empty_message}");
    assert!(
        returns.iter().all(|ret| predicate(ret)),
        "{predicate_message}"
    );
}

#[test]
fn skip_handler_returns_unit_for_unit_scenarios() {
    assert_skip_handler_returns(
        ScenarioReturnKind::Unit,
        "expected skip handler to include a return for unit scenarios",
        |ret| ret.expr.is_none(),
        "unit skip handler should only use a bare return",
    );
}

#[test]
fn skip_handler_returns_ok_for_fallible_scenarios() {
    assert_skip_handler_returns(
        ScenarioReturnKind::ResultUnit,
        "expected skip handler to include a return for fallible scenarios",
        |ret| ret.expr.as_ref().is_some_and(|expr| is_ok_unit_expr(expr)),
        "fallible skip handler should only return Ok(())",
    );
}
