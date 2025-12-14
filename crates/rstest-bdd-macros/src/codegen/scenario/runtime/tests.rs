//! Tests for runtime scaffolding code generation.

use super::execute_single_step;

fn path_last_ident(path: &syn::Path) -> Option<&syn::Ident> {
    path.segments.last().map(|seg| &seg.ident)
}

fn path_second_last_ident(path: &syn::Path) -> Option<&syn::Ident> {
    path.segments.iter().rev().nth(1).map(|seg| &seg.ident)
}

#[test]
fn execute_single_step_looks_up_steps_with_steptext_from() {
    let item: syn::ItemFn = match syn::parse2(execute_single_step()) {
        Ok(item) => item,
        Err(err) => panic!("execute_single_step parses as a function: {err}"),
    };
    let expr_if = item
        .block
        .stmts
        .iter()
        .find_map(|stmt| match stmt {
            syn::Stmt::Expr(syn::Expr::If(expr_if), _) => Some(expr_if),
            _ => None,
        })
        .unwrap_or_else(|| panic!("contains an if expression"));
    let expr_let = match expr_if.cond.as_ref() {
        syn::Expr::Let(expr_let) => expr_let,
        other => panic!("expected if-let condition, got {other:?}"),
    };
    let find_step_call = match expr_let.expr.as_ref() {
        syn::Expr::Call(call) => call,
        other => panic!("expected call expression, got {other:?}"),
    };
    let func_path = match find_step_call.func.as_ref() {
        syn::Expr::Path(expr_path) => &expr_path.path,
        other => panic!("expected path expression, got {other:?}"),
    };
    assert_eq!(
        path_last_ident(func_path)
            .map(syn::Ident::to_string)
            .as_deref(),
        Some("find_step"),
        "expected to call find_step(...)",
    );

    let args: Vec<_> = find_step_call.args.iter().collect();
    assert_eq!(args.len(), 2, "expected find_step(keyword, text)");

    let second_arg = args.get(1).map_or_else(
        || panic!("expected second argument for find_step(keyword, text)"),
        |arg| *arg,
    );
    let steptext_call = match second_arg {
        syn::Expr::Call(call) => call,
        other => panic!("expected StepText::from(text), got {other:?}"),
    };
    let steptext_func_path = match steptext_call.func.as_ref() {
        syn::Expr::Path(expr_path) => &expr_path.path,
        other => panic!("expected path expression, got {other:?}"),
    };
    assert_eq!(
        path_last_ident(steptext_func_path)
            .map(syn::Ident::to_string)
            .as_deref(),
        Some("from"),
        "expected StepText::from(...)",
    );
    assert_eq!(
        path_second_last_ident(steptext_func_path)
            .map(syn::Ident::to_string)
            .as_deref(),
        Some("StepText"),
        "expected StepText::from(...)",
    );

    let inner_args: Vec<_> = steptext_call.args.iter().collect();
    assert_eq!(inner_args.len(), 1, "expected StepText::from(text)");
    let inner_arg = inner_args.first().map_or_else(
        || panic!("expected StepText::from(text) argument"),
        |arg| *arg,
    );
    let inner_path = match inner_arg {
        syn::Expr::Path(expr_path) => &expr_path.path,
        other => panic!("expected text identifier, got {other:?}"),
    };
    assert_eq!(
        path_last_ident(inner_path)
            .map(syn::Ident::to_string)
            .as_deref(),
        Some("text"),
        "expected StepText::from(text)",
    );
}
