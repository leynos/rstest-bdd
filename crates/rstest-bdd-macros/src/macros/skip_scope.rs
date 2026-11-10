//! Generated skip-scope guard insertion for step functions.

use syn::parse_quote;

/// Wrap a step body so skip handling retains the calling step's source context.
pub(super) fn inject_skip_scope(func: &mut syn::ItemFn) {
    let path = crate::codegen::rstest_bdd_path();
    let ident = &func.sig.ident;
    let scope_init: syn::Stmt = parse_quote! {
        #[expect(unused_variables, reason = "RAII guard, only Drop matters")]
        let __rstest_bdd_step_scope_guard = #path::__rstest_bdd_enter_scope(
            #path::__rstest_bdd_scope_kind::Step,
            stringify!(#ident),
            file!(),
            line!(),
        );
    };
    let original_stmts = func.block.stmts.clone();
    *func.block = parse_quote!({
        #scope_init
        #(#original_stmts)*
    });
}
