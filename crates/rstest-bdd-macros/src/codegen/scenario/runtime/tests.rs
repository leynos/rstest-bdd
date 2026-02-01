//! Tests for runtime scaffolding code generation.

use super::generators::{
    generate_async_step_executor, generate_skip_extractor, generate_skip_handler,
    generate_step_executor,
};
use crate::codegen::scenario::ScenarioReturnKind;
use rstest::rstest;
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

/// Assert that a path ends with `execution::ExecutionError`.
fn assert_path_is_execution_error(path: &syn::Path) {
    assert_path_ends_with_module_function(path, "execution", "ExecutionError");
}

/// Assert that a path ends with `execution::execute_step_async`.
fn assert_path_is_execution_execute_step_async(path: &syn::Path) {
    assert_path_ends_with_module_function(path, "execution", "execute_step_async");
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

/// Visitor to find method calls by name.
struct MethodCallFinder {
    name: String,
    count: usize,
}

impl<'ast> Visit<'ast> for MethodCallFinder {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == self.name {
            self.count += 1;
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn count_method_calls_in_block(block: &syn::Block, method_name: &str) -> usize {
    let mut finder = MethodCallFinder {
        name: method_name.to_string(),
        count: 0,
    };
    finder.visit_block(block);
    finder.count
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

/// Expectations for a generated step executor helper.
#[derive(Clone, Copy)]
struct StepExecutorExpectation<'a> {
    function_name: &'a str,
    description: &'a str,
    runtime_function: &'a str,
    should_be_async: bool,
}

impl<'a> StepExecutorExpectation<'a> {
    fn new(
        function_name: &'a str,
        description: &'a str,
        runtime_function: &'a str,
        should_be_async: bool,
    ) -> Self {
        Self {
            function_name,
            description,
            runtime_function,
            should_be_async,
        }
    }
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
    expectation: StepExecutorExpectation<'_>,
) {
    let file: syn::File = syn::parse2(tokens)
        .unwrap_or_else(|e| panic!("{}: failed to parse tokens: {e}", expectation.description));

    let item = find_function_by_name(&file, expectation.function_name);

    if expectation.should_be_async {
        assert!(
            item.sig.asyncness.is_some(),
            "{}: expected {} to be async",
            expectation.description,
            expectation.function_name
        );
    } else {
        assert!(
            item.sig.asyncness.is_none(),
            "{}: expected {} to be non-async",
            expectation.description,
            expectation.function_name
        );
    }

    let execute_step_call = find_call_in_block(&item.block, expectation.runtime_function)
        .unwrap_or_else(|| {
            panic!(
                "{}: expected call to {}",
                expectation.description, expectation.runtime_function
            )
        });

    let func_path = extract_path(execute_step_call.func.as_ref());
    match expectation.runtime_function {
        "execute_step" => assert_path_is_execution_execute_step(func_path),
        "execute_step_async" => assert_path_is_execution_execute_step_async(func_path),
        other => panic!(
            "{}: unexpected runtime function name: {other}",
            expectation.description
        ),
    }

    assert_eq!(
        execute_step_call.args.len(),
        2,
        "{}: {} should receive StepExecutionRequest reference and ctx",
        expectation.description,
        expectation.runtime_function
    );
}

/// Whether to generate and validate sync or async executor code.
#[derive(Debug, Clone, Copy)]
enum ExecutorType {
    Sync,
    Async,
}

impl ExecutorType {
    fn generate(self) -> proc_macro2::TokenStream {
        match self {
            Self::Sync => generate_step_executor(),
            Self::Async => generate_async_step_executor(),
        }
    }

    fn expectation(self) -> StepExecutorExpectation<'static> {
        match self {
            Self::Sync => StepExecutorExpectation::new(
                "__rstest_bdd_execute_single_step",
                "sync step executor",
                "execute_step",
                false,
            ),
            Self::Async => StepExecutorExpectation::new(
                "__rstest_bdd_process_async_step",
                "async step executor",
                "execute_step_async",
                true,
            ),
        }
    }
}

/// Verify that generated step executors remain thin wrappers over the runtime.
///
/// This parameterised test covers both synchronous and asynchronous executor
/// variants, ensuring they delegate to the appropriate `rstest_bdd::execution`
/// function without embedding inline implementation details.
#[rstest]
#[case(ExecutorType::Sync)]
#[case(ExecutorType::Async)]
fn step_executor_delegates_to_runtime(#[case] executor_type: ExecutorType) {
    assert_step_executor_delegates_to_runtime(
        executor_type.generate(),
        executor_type.expectation(),
    );
}

/// Verify that the skip extractor references `rstest_bdd::execution::ExecutionError`.
///
/// The generated `__rstest_bdd_extract_skip_message` function accepts an
/// `ExecutionError` reference and calls its `is_skip()` and `skip_message()`
/// methods to extract skip information.
#[test]
#[expect(
    clippy::expect_used,
    reason = "test parses generated tokens and uses expect for clearer failures"
)]
fn skip_extractor_references_execution_error() {
    let file: syn::File =
        syn::parse2(generate_skip_extractor()).expect("generate_skip_extractor parses as a file");

    let item = find_function_by_name(&file, "__rstest_bdd_extract_skip_message");

    // Verify the function signature references ExecutionError
    // The function takes a reference to ExecutionError as its parameter
    let inputs = &item.sig.inputs;
    assert_eq!(inputs.len(), 1, "expected single parameter");

    let param = inputs.first().expect("expected first parameter");
    if let syn::FnArg::Typed(pat_type) = param {
        // The type should be a reference to a path ending in ExecutionError
        if let syn::Type::Reference(type_ref) = pat_type.ty.as_ref() {
            if let syn::Type::Path(type_path) = type_ref.elem.as_ref() {
                assert_path_is_execution_error(&type_path.path);
            } else {
                panic!("expected path type inside reference");
            }
        } else {
            panic!("expected reference type for parameter");
        }
    } else {
        panic!("expected typed parameter");
    }

    // Verify the function body calls is_skip() and skip_message() on the error parameter
    let is_skip_calls = count_method_calls_in_block(&item.block, "is_skip");
    assert!(
        is_skip_calls >= 1,
        "expected at least one call to is_skip(), found {is_skip_calls}"
    );

    let skip_message_calls = count_method_calls_in_block(&item.block, "skip_message");
    assert!(
        skip_message_calls >= 1,
        "expected at least one call to skip_message(), found {skip_message_calls}"
    );
}

#[expect(clippy::panic, reason = "test helper panics for clearer failures")]
fn assert_skip_handler_returns(
    return_kind: ScenarioReturnKind,
    empty_message: &str,
    predicate: impl Fn(&syn::ExprReturn) -> bool,
    predicate_message: &str,
) {
    let if_expr = parse_skip_handler(return_kind);
    let returns = collect_returns(&if_expr.then_branch);
    let panic_with_message = |message: &str| panic!("{message}");

    if returns.is_empty() {
        panic_with_message(empty_message);
    }
    if !returns.iter().all(|ret| predicate(ret)) {
        panic_with_message(predicate_message);
    }
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
