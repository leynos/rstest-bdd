//! Minimal `#[gpui::test]` implementation for the rstest-bdd workspace.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Error, FnArg, ItemFn, PatType, Signature, Type, TypeReference, parse::Nothing,
    parse_macro_input,
};

/// Runs a test inside the workspace GPUI shim.
///
/// The annotated function may be synchronous or asynchronous, may take zero or
/// more `&gpui::TestAppContext` parameters, and may return any type that
/// implements [`std::process::Termination`], such as `()` or
/// `Result<(), E>`.
///
/// The generated wrapper preserves the declared test name in
/// `TestAppContext::test_function_name()`.
///
/// # Examples
///
/// ```rust,no_run
/// #[gpui::test]
/// fn renders_a_view(context: &gpui::TestAppContext) {
///     assert_eq!(context.test_function_name(), Some("renders_a_view"));
/// }
/// ```
///
/// ```rust,no_run
/// #[gpui::test]
/// async fn saves_state(context: &gpui::TestAppContext) -> Result<(), &'static str> {
///     assert_eq!(context.test_function_name(), Some("saves_state"));
///     Ok(())
/// }
/// ```
///
/// # Errors
///
/// Emits a compile error when the function is generic, uses a receiver
/// parameter, or declares parameters other than references to
/// `gpui::TestAppContext`.
#[proc_macro_attribute]
pub fn test(args: TokenStream, input: TokenStream) -> TokenStream {
    let _ = parse_macro_input!(args as Nothing);
    let mut function = parse_macro_input!(input as ItemFn);

    if let Err(error) = validate_signature(&function.sig) {
        return error.to_compile_error().into();
    }

    let outer_attrs = std::mem::take(&mut function.attrs);
    let outer_name = function.sig.ident.clone();
    let inner_name = format_ident!("__{outer_name}");
    function.sig.ident = inner_name.clone();

    let context_setup = match build_context_setup(&function.sig, &outer_name) {
        Ok(tokens) => tokens,
        Err(error) => return error.to_compile_error().into(),
    };
    let ContextSetup {
        setup,
        args,
        teardown,
    } = context_setup;

    let call = if function.sig.asyncness.is_some() {
        quote! {
            let executor = gpui::BackgroundExecutor::new(std::sync::Arc::new(dispatcher.clone()));
            gpui::assert_test_outcome(executor.block_test(#inner_name(#(#args),*)));
        }
    } else {
        quote! {
            gpui::assert_test_outcome(#inner_name(#(#args),*));
        }
    };

    let expanded = quote! {
        #(#outer_attrs)*
        #[test]
        fn #outer_name() {
            #function

            gpui::run_test(1, &[], 0, &mut |dispatcher, _seed| {
                #(#setup)*
                #call
                #(#teardown)*
            }, None);
        }
    };

    expanded.into()
}

struct ContextSetup {
    setup: Vec<proc_macro2::TokenStream>,
    args: Vec<proc_macro2::TokenStream>,
    teardown: Vec<proc_macro2::TokenStream>,
}

fn validate_signature(signature: &Signature) -> syn::Result<()> {
    if !signature.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &signature.generics,
            "gpui::test does not support generic functions in this workspace",
        ));
    }

    for input in &signature.inputs {
        let FnArg::Typed(argument) = input else {
            return Err(Error::new_spanned(
                input,
                "gpui::test does not support receiver parameters",
            ));
        };

        validate_argument(argument)?;
    }

    Ok(())
}

fn validate_argument(argument: &PatType) -> syn::Result<()> {
    let Type::Reference(reference) = argument.ty.as_ref() else {
        return Err(Error::new_spanned(
            argument,
            "gpui::test only supports &TestAppContext parameters in this workspace",
        ));
    };

    validate_context_reference(reference)
}

fn validate_context_reference(reference: &TypeReference) -> syn::Result<()> {
    let Type::Path(path) = reference.elem.as_ref() else {
        return Err(Error::new_spanned(
            reference,
            "gpui::test only supports references to TestAppContext",
        ));
    };

    let Some(last_segment) = path.path.segments.last() else {
        return Err(Error::new_spanned(path, "expected a concrete path"));
    };

    if last_segment.ident == "TestAppContext" {
        Ok(())
    } else {
        Err(Error::new_spanned(
            reference,
            "gpui::test only supports &TestAppContext parameters in this workspace",
        ))
    }
}

fn build_context_setup(
    signature: &Signature,
    declared_name: &syn::Ident,
) -> syn::Result<ContextSetup> {
    let mut setup = Vec::new();
    let mut args = Vec::new();
    let mut teardown = Vec::new();

    for (index, input) in signature.inputs.iter().enumerate() {
        let FnArg::Typed(argument) = input else {
            unreachable!("validated above");
        };

        validate_argument(argument)?;

        let binding = format_ident!("cx_{index}");
        setup.push(quote! {
            let mut #binding = gpui::TestAppContext::build(
                dispatcher.clone(),
                Some(stringify!(#declared_name)),
            );
        });
        args.push(quote!(&mut #binding));
        teardown.push(quote! {
            dispatcher.run_until_parked();
            #binding.executor().forbid_parking();
            #binding.quit();
            dispatcher.run_until_parked();
        });
    }

    Ok(ContextSetup {
        setup,
        args,
        teardown,
    })
}
