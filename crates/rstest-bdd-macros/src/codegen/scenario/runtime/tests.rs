//! Tests for runtime scaffolding code generation.

use super::execute_single_step;

fn path_last_ident(path: &syn::Path) -> Option<&syn::Ident> {
    path.segments.last().map(|seg| &seg.ident)
}

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
        .expect("contains an if expression")
}

fn extract_let_from_cond(cond: &syn::Expr) -> &syn::ExprLet {
    match cond {
        syn::Expr::Let(expr_let) => expr_let,
        other => panic!("expected if-let condition, got {other:?}"),
    }
}

fn extract_call(expr: &syn::Expr) -> &syn::ExprCall {
    match expr {
        syn::Expr::Call(call) => call,
        other => panic!("expected call expression, got {other:?}"),
    }
}

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

#[test]
#[expect(
    clippy::expect_used,
    reason = "test parses generated tokens and uses expect for clearer failures"
)]
#[expect(
    clippy::indexing_slicing,
    reason = "indexing is guarded by explicit arg length assertions"
)]
fn execute_single_step_looks_up_steps_with_steptext_from() {
    let item: syn::ItemFn =
        syn::parse2(execute_single_step()).expect("execute_single_step parses as a function");
    let expr_if = extract_if_expr(&item.block.stmts);
    let expr_let = extract_let_from_cond(expr_if.cond.as_ref());
    let find_step_call = extract_call(expr_let.expr.as_ref());
    let func_path = extract_path(find_step_call.func.as_ref());
    assert_path_ends_with(func_path, "find_step", "expected to call find_step(...)");

    let args: Vec<_> = find_step_call.args.iter().collect();
    assert_eq!(args.len(), 2, "expected find_step(keyword, text)");

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
