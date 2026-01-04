//! Tests for runtime scaffolding code generation.

use super::generators::generate_step_executor;
use syn::visit::Visit;

/// Return the identifier of the final segment in a `syn::Path`.
///
/// Returns `None` when the path has no segments (for example, if it was parsed
/// from an empty token stream).
fn path_last_ident(path: &syn::Path) -> Option<&syn::Ident> {
    path.segments.last().map(|seg| &seg.ident)
}

/// Return the identifier of the segment before the final segment in a `syn::Path`.
///
/// Returns `None` when the path contains fewer than two segments.
fn path_second_last_ident(path: &syn::Path) -> Option<&syn::Ident> {
    path.segments.iter().rev().nth(1).map(|seg| &seg.ident)
}

#[expect(clippy::panic, reason = "test helper panics for clearer failures")]
fn extract_call(expr: &syn::Expr) -> &syn::ExprCall {
    match expr {
        syn::Expr::Call(call) => call,
        other => panic!("expected call expression, got {other:?}"),
    }
}

#[expect(clippy::panic, reason = "test helper panics for clearer failures")]
fn extract_path(expr: &syn::Expr) -> &syn::Path {
    match expr {
        syn::Expr::Path(expr_path) => &expr_path.path,
        other => panic!("expected path expression, got {other:?}"),
    }
}

fn assert_path_ends_with(path: &syn::Path, expected: &str, context: &str) {
    assert_eq!(
        path_last_ident(path).map(syn::Ident::to_string).as_deref(),
        Some(expected),
        "{context}",
    );
}

#[expect(
    clippy::expect_used,
    reason = "test helper uses expect for clearer failures"
)]
fn find_execute_single_step_function(file: &syn::File) -> &syn::ItemFn {
    file.items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(f) if f.sig.ident == "__rstest_bdd_execute_single_step" => Some(f),
            _ => None,
        })
        .expect("expected __rstest_bdd_execute_single_step function")
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

#[expect(
    clippy::indexing_slicing,
    reason = "indexing is guarded by explicit arg length assertions"
)]
fn assert_steptext_from_wrapper(find_step_call: &syn::ExprCall) {
    let args: Vec<_> = find_step_call.args.iter().collect();
    let steptext_call = extract_call(args[1]);
    let steptext_func_path = extract_path(steptext_call.func.as_ref());
    assert_path_ends_with(steptext_func_path, "from", "expected StepText::from(...)");
    assert_eq!(
        path_second_last_ident(steptext_func_path)
            .map(syn::Ident::to_string)
            .as_deref(),
        Some("StepText"),
        "expected StepText::from(...)",
    );

    let inner_args: Vec<_> = steptext_call.args.iter().collect();
    assert_eq!(inner_args.len(), 1, "expected StepText::from(text)");
    let inner_path = extract_path(inner_args[0]);
    assert_path_ends_with(inner_path, "text", "expected StepText::from(text)");
}

/// Verify that a named function is defined as an inner function.
fn assert_has_inner_function(stmts: &[syn::Stmt], name: &str) {
    let found = stmts.iter().any(|stmt| match stmt {
        syn::Stmt::Item(syn::Item::Fn(f)) => f.sig.ident == name,
        _ => false,
    });
    assert!(found, "expected inner function '{name}' to be defined");
}

/// Check if an expression is a reference to a specific identifier (e.g., `&step`).
fn is_reference_to_ident(expr: &syn::Expr, name: &str) -> bool {
    let syn::Expr::Reference(ref_expr) = expr else {
        return false;
    };
    let syn::Expr::Path(path_expr) = ref_expr.expr.as_ref() else {
        return false;
    };
    path_expr.path.is_ident(name)
}

/// Find the index of the first statement matching the given predicate.
fn find_statement_index<F>(stmts: &[syn::Stmt], predicate: F) -> Option<usize>
where
    F: Fn(&syn::Stmt) -> bool,
{
    stmts.iter().position(predicate)
}

/// Find the index of a statement containing a call to a named function.
fn find_call_statement_index(stmts: &[syn::Stmt], func_name: &str) -> Option<usize> {
    find_statement_index(stmts, |stmt| {
        let syn::Stmt::Expr(syn::Expr::Call(call), _) = stmt else {
            return false;
        };
        let syn::Expr::Path(path_expr) = call.func.as_ref() else {
            return false;
        };
        path_expr.path.is_ident(func_name)
    })
}

/// Find the index of a statement containing a match expression.
fn find_match_statement_index(stmts: &[syn::Stmt]) -> Option<usize> {
    find_statement_index(stmts, |stmt| {
        matches!(stmt, syn::Stmt::Expr(syn::Expr::Match(_), _))
    })
}

#[test]
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test parses generated tokens and uses expect for clearer failures"
)]
fn execute_single_step_looks_up_steps_with_steptext_from() {
    // Parse the generated helper tokens so we can assert on the AST structure,
    // keeping this test resilient to formatting-only changes.
    let file: syn::File =
        syn::parse2(generate_step_executor()).expect("generate_step_executor parses as a file");
    let item = find_execute_single_step_function(&file);

    // Validate that inner helper functions are defined inside execute_single_step
    assert_has_inner_function(&item.block.stmts, "validate_required_fixtures");
    assert_has_inner_function(&item.block.stmts, "encode_skip_message");

    let find_step_call = find_call_in_block(&item.block, "find_step_with_metadata")
        .expect("expected call to find_step_with_metadata");
    let func_path = extract_path(find_step_call.func.as_ref());
    assert_path_ends_with(
        func_path,
        "find_step_with_metadata",
        "expected to call find_step_with_metadata(...)",
    );
    assert_eq!(
        find_step_call.args.len(),
        2,
        "expected find_step_with_metadata(keyword, text)"
    );
    assert_steptext_from_wrapper(find_step_call);

    let stmts = &item.block.stmts;

    // Verify validate_required_fixtures is called with &step as the first argument
    let validate_idx = find_call_statement_index(stmts, "validate_required_fixtures")
        .expect("expected validate_required_fixtures call in then branch");
    let validate_stmt = stmts
        .get(validate_idx)
        .expect("validate_idx should be valid");
    let syn::Stmt::Expr(syn::Expr::Call(validate_call), _) = validate_stmt else {
        panic!("expected call statement");
    };
    let first_arg = validate_call
        .args
        .first()
        .expect("validate_required_fixtures should have arguments");
    assert!(
        is_reference_to_ident(first_arg, "step"),
        "first argument to validate_required_fixtures should be &step"
    );

    // Verify step execution (match expression) appears after validate_required_fixtures
    let match_idx =
        find_match_statement_index(stmts).expect("expected match expression in then branch");
    assert!(
        validate_idx < match_idx,
        "validate_required_fixtures (index {validate_idx}) should be called before step execution (index {match_idx})"
    );
}
