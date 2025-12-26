//! Tests for runtime scaffolding code generation.

use super::generate_step_executor;

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

#[expect(
    clippy::expect_used,
    reason = "test helper uses expect for clearer failures"
)]
fn extract_if_expr(stmts: &[syn::Stmt]) -> &syn::ExprIf {
    stmts
        .iter()
        .find_map(|stmt| match stmt {
            syn::Stmt::Expr(syn::Expr::If(expr_if), _) => Some(expr_if),
            _ => None,
        })
        .expect("expected statements to contain an if expression")
}

#[expect(clippy::panic, reason = "test helper panics for clearer failures")]
fn extract_let_from_cond(cond: &syn::Expr) -> &syn::ExprLet {
    match cond {
        syn::Expr::Let(expr_let) => expr_let,
        other => panic!("expected if-let condition, got {other:?}"),
    }
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

#[expect(clippy::panic, reason = "test helper panics for clearer failures")]
fn extract_stmt_call(stmt: &syn::Stmt) -> &syn::ExprCall {
    match stmt {
        // syn 2.x uses Stmt::Expr(Expr, Option<Token![;]>) for both semicolon and no-semicolon
        syn::Stmt::Expr(syn::Expr::Call(call), _) => call,
        other => panic!("expected call statement, got {other:?}"),
    }
}

fn is_path_ident(expr: &syn::Expr, name: &str) -> bool {
    matches!(expr, syn::Expr::Path(p) if p.path.is_ident(name))
}

fn is_reference_to_ident(expr: &syn::Expr, name: &str) -> bool {
    matches!(expr, syn::Expr::Reference(r) if matches!(&*r.expr, syn::Expr::Path(p) if p.path.is_ident(name)))
}

#[expect(
    clippy::expect_used,
    reason = "test helper uses expect for clearer failures"
)]
fn find_execute_single_step_function(file: &syn::File) -> &syn::ItemFn {
    file.items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(f) if f.sig.ident == "execute_single_step" => Some(f),
            _ => None,
        })
        .expect("expected execute_single_step function")
}

fn assert_find_step_with_metadata_call(expr_if: &syn::ExprIf) -> &syn::ExprCall {
    let expr_let = extract_let_from_cond(expr_if.cond.as_ref());
    let find_step_call = extract_call(expr_let.expr.as_ref());
    let func_path = extract_path(find_step_call.func.as_ref());
    assert_path_ends_with(
        func_path,
        "find_step_with_metadata",
        "expected to call find_step_with_metadata(...)",
    );

    let args: Vec<_> = find_step_call.args.iter().collect();
    assert_eq!(
        args.len(),
        2,
        "expected find_step_with_metadata(keyword, text)"
    );

    find_step_call
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

#[expect(
    clippy::indexing_slicing,
    reason = "indexing is guarded by explicit arg length assertions"
)]
fn assert_validate_required_fixtures_call(if_body_stmts: &[syn::Stmt]) {
    let validate_call = extract_stmt_call(&if_body_stmts[0]);
    let validate_path = extract_path(validate_call.func.as_ref());
    assert_path_ends_with(
        validate_path,
        "validate_required_fixtures",
        "expected first statement to call validate_required_fixtures",
    );

    let validate_args: Vec<_> = validate_call.args.iter().collect();
    assert_eq!(
        validate_args.len(),
        5,
        "expected validate_required_fixtures(&step, ctx, text, feature_path, scenario_name)"
    );
    assert!(
        is_reference_to_ident(validate_args[0], "step"),
        "expected first arg to be &step"
    );
    assert!(
        is_path_ident(validate_args[1], "ctx"),
        "expected second arg to be ctx"
    );
    assert!(
        is_path_ident(validate_args[2], "text"),
        "expected third arg to be text"
    );
    assert!(
        is_path_ident(validate_args[3], "feature_path"),
        "expected fourth arg to be feature_path"
    );
    assert!(
        is_path_ident(validate_args[4], "scenario_name"),
        "expected fifth arg to be scenario_name"
    );
}

#[expect(
    clippy::indexing_slicing,
    reason = "indexing is guarded by explicit arg length assertions"
)]
fn assert_run_step_call(if_body_stmts: &[syn::Stmt]) {
    let run_step_call = extract_stmt_call(&if_body_stmts[1]);
    let run_step_path = extract_path(run_step_call.func.as_ref());
    assert_path_ends_with(
        run_step_path,
        "run_step",
        "expected second statement to call run_step",
    );

    let run_step_args: Vec<_> = run_step_call.args.iter().collect();
    assert_eq!(
        run_step_args.len(),
        9,
        "expected run_step(index, keyword, text, docstring, table, ctx, feature_path, scenario_name, &step)"
    );
    assert!(
        is_path_ident(run_step_args[0], "index"),
        "expected first arg to be index"
    );
    assert!(
        is_path_ident(run_step_args[1], "keyword"),
        "expected second arg to be keyword"
    );
    assert!(
        is_path_ident(run_step_args[2], "text"),
        "expected third arg to be text"
    );
    assert!(
        is_path_ident(run_step_args[3], "docstring"),
        "expected fourth arg to be docstring"
    );
    assert!(
        is_path_ident(run_step_args[4], "table"),
        "expected fifth arg to be table"
    );
    assert!(
        is_path_ident(run_step_args[5], "ctx"),
        "expected sixth arg to be ctx"
    );
    assert!(
        is_path_ident(run_step_args[6], "feature_path"),
        "expected seventh arg to be feature_path"
    );
    assert!(
        is_path_ident(run_step_args[7], "scenario_name"),
        "expected eighth arg to be scenario_name"
    );
    assert!(
        is_reference_to_ident(run_step_args[8], "step"),
        "expected ninth arg to be &step"
    );
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test parses generated tokens and uses expect for clearer failures"
)]
fn execute_single_step_looks_up_steps_with_steptext_from() {
    // Parse the generated helper tokens so we can assert on the AST structure,
    // keeping this test resilient to formatting-only changes.
    let file: syn::File =
        syn::parse2(generate_step_executor()).expect("generate_step_executor parses as a file");
    let item = find_execute_single_step_function(&file);

    // Validate the `if let Some(step) = find_step_with_metadata(...)` guard
    let expr_if = extract_if_expr(&item.block.stmts);
    let find_step_call = assert_find_step_with_metadata_call(expr_if);
    assert_steptext_from_wrapper(find_step_call);

    // Validate the if body contains validate_required_fixtures and run_step calls
    let if_body_stmts = &expr_if.then_branch.stmts;
    assert!(
        if_body_stmts.len() >= 2,
        "expected at least 2 statements in if body for validate_required_fixtures and run_step"
    );
    assert_validate_required_fixtures_call(if_body_stmts);
    assert_run_step_call(if_body_stmts);
}
