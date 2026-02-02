//! Test support helpers for scenario runtime code generation.

use crate::codegen::scenario::ScenarioReturnKind;
use syn::visit::Visit;

use super::super::generators::generate_skip_handler;
use super::RuntimeFunction;

/// Return the identifier of the final segment in a `syn::Path`.
///
/// Returns `None` when the path has no segments (for example, if it was parsed
/// from an empty token stream).
pub(super) fn path_last_ident(path: &syn::Path) -> Option<&syn::Ident> {
    path.segments.last().map(|seg| &seg.ident)
}

#[expect(clippy::panic, reason = "test helper panics for clearer failures")]
pub(super) fn extract_path(expr: &syn::Expr) -> &syn::Path {
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
pub(super) fn assert_path_ends_with_module_function(
    path: &syn::Path,
    module: &str,
    function: &str,
) {
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
pub(super) fn assert_path_is_execution_execute_step(path: &syn::Path) {
    assert_path_ends_with_module_function(path, "execution", "execute_step");
}

/// Assert that a path ends with `execution::ExecutionError`.
pub(super) fn assert_path_is_execution_error(path: &syn::Path) {
    assert_path_ends_with_module_function(path, "execution", "ExecutionError");
}

/// Assert that a path ends with `execution::execute_step_async`.
pub(super) fn assert_path_is_execution_execute_step_async(path: &syn::Path) {
    assert_path_ends_with_module_function(path, "execution", "execute_step_async");
}

#[expect(clippy::panic, reason = "test helper panics for clearer failures")]
pub(super) fn find_function_by_name<'a>(file: &'a syn::File, name: &str) -> &'a syn::ItemFn {
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

pub(super) fn count_method_calls_in_block(block: &syn::Block, method_name: &str) -> usize {
    let mut finder = MethodCallFinder {
        name: method_name.to_string(),
        count: 0,
    };
    finder.visit_block(block);
    finder.count
}

pub(super) fn find_call_in_block(
    block: &syn::Block,
    name: RuntimeFunction,
) -> Option<&syn::ExprCall> {
    let mut finder = CallFinder {
        name: name.call_name().to_string(),
        found: None,
    };
    finder.visit_block(block);
    finder.found
}

#[expect(clippy::panic, reason = "test helper panics for clearer failures")]
pub(super) fn parse_skip_handler(return_kind: ScenarioReturnKind) -> syn::ExprIf {
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

pub(super) fn collect_returns(block: &syn::Block) -> Vec<&syn::ExprReturn> {
    let mut finder = ReturnFinder {
        returns: Vec::new(),
    };
    finder.visit_block(block);
    finder.returns
}

pub(super) fn is_ok_unit_expr(expr: &syn::Expr) -> bool {
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
