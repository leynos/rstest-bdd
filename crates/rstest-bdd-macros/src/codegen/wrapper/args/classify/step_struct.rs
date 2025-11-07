//! Step-struct classifier helpers keep the main classifier module small.

use std::collections::HashSet;

use quote::ToTokens;

use super::{Arg, ExtractedArgs};

pub(crate) fn extract_step_struct_attribute(arg: &mut syn::PatType) -> syn::Result<bool> {
    super::extract_flag_attribute(arg, "step_args")
}

pub(crate) fn classify_step_struct(
    st: &mut ExtractedArgs,
    arg: &syn::PatType,
    placeholders: &mut HashSet<String>,
) -> syn::Result<()> {
    let syn::Pat::Ident(pat_ident) = arg.pat.as_ref() else {
        return Err(syn::Error::new_spanned(
            &arg.pat,
            "#[step_args] requires a simple identifier pattern",
        ));
    };
    let pat = &pat_ident.ident;
    let ty = &arg.ty;
    let check = |condition: bool, span: &dyn ToTokens, msg: &str| {
        if condition {
            Err(syn::Error::new_spanned(span, msg))
        } else {
            Ok(())
        }
    };
    check(
        st.step_struct_idx.is_some(),
        arg,
        "only one #[step_args] parameter is permitted per step",
    )?;
    check(
        st.step_args().next().is_some(),
        arg,
        "#[step_args] cannot be combined with named step arguments",
    )?;
    check(
        placeholders.is_empty(),
        arg,
        "#[step_args] requires at least one placeholder in the pattern",
    )?;
    check(
        arg.attrs.iter().any(|a| a.path().is_ident("from")),
        arg,
        "#[step_args] cannot be combined with #[from]",
    )?;
    check(
        matches!(ty.as_ref(), syn::Type::Reference(_)),
        ty.as_ref(),
        "#[step_args] parameters must own their struct type",
    )?;
    let idx = st.push(Arg::StepStruct {
        pat: pat.clone(),
        ty: ty.as_ref().clone(),
    });
    st.step_struct_idx = Some(idx);
    st.blocked_placeholders.clone_from(placeholders);
    placeholders.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::{Span, TokenStream as TokenStream2};
    use quote::quote;
    use syn::{parse_quote, FnArg, Ident};

    fn placeholder_set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    fn pat(tokens: TokenStream2) -> syn::PatType {
        match syn::parse2::<FnArg>(tokens) {
            Ok(FnArg::Typed(arg)) => arg,
            Ok(FnArg::Receiver(_)) => panic!("expected typed argument"),
            Err(err) => panic!("failed to parse argument: {err}"),
        }
    }

    #[test]
    fn classifies_step_struct_and_clears_placeholders() {
        let mut extracted = ExtractedArgs::default();
        let mut placeholders = placeholder_set(&["value"]);
        let arg = pat(quote!(#[step_args] args: Args));

        match classify_step_struct(&mut extracted, &arg, &mut placeholders) {
            Ok(()) => {}
            Err(err) => panic!("classification should succeed: {err}"),
        }

        assert!(placeholders.is_empty());
        assert!(matches!(
            extracted.args.as_slice(),
            [Arg::StepStruct { .. }]
        ));
    }

    #[test]
    fn rejects_duplicate_step_structs() {
        let mut extracted = ExtractedArgs::default();
        extracted.step_struct_idx = Some(extracted.push(Arg::StepStruct {
            pat: Ident::new("existing", Span::call_site()),
            ty: parse_quote!(Args),
        }));
        let mut placeholders = placeholder_set(&["value"]);
        let arg = pat(quote!(#[step_args] args: Args));

        let Err(err) = classify_step_struct(&mut extracted, &arg, &mut placeholders) else {
            panic!("duplicate #[step_args] should error");
        };
        assert!(err
            .to_string()
            .contains("only one #[step_args] parameter is permitted per step"));
    }

    #[test]
    fn rejects_mix_with_named_arguments() {
        let mut extracted = ExtractedArgs::default();
        extracted.push(Arg::Step {
            pat: Ident::new("value", Span::call_site()),
            ty: parse_quote!(String),
        });
        let mut placeholders = placeholder_set(&["value"]);
        let arg = pat(quote!(#[step_args] args: Args));

        let Err(err) = classify_step_struct(&mut extracted, &arg, &mut placeholders) else {
            panic!("mixing #[step_args] with named args should error");
        };
        assert!(err
            .to_string()
            .contains("#[step_args] cannot be combined with named step arguments"));
    }

    #[test]
    fn rejects_missing_placeholders() {
        let mut extracted = ExtractedArgs::default();
        let mut placeholders = HashSet::new();
        let arg = pat(quote!(#[step_args] args: Args));

        let Err(err) = classify_step_struct(&mut extracted, &arg, &mut placeholders) else {
            panic!("missing placeholders should error");
        };
        assert!(err
            .to_string()
            .contains("#[step_args] requires at least one placeholder"));
    }

    #[test]
    fn rejects_with_from_attribute() {
        let mut extracted = ExtractedArgs::default();
        let mut placeholders = placeholder_set(&["value"]);
        let arg = pat(quote!(#[step_args] #[from] args: Args));

        let Err(err) = classify_step_struct(&mut extracted, &arg, &mut placeholders) else {
            panic!("#[step_args] with #[from] should error");
        };
        assert!(err
            .to_string()
            .contains("#[step_args] cannot be combined with #[from]"));
    }

    #[test]
    fn rejects_reference_types() {
        let mut extracted = ExtractedArgs::default();
        let mut placeholders = placeholder_set(&["value"]);
        let arg = pat(quote!(#[step_args] args: &Args));

        let Err(err) = classify_step_struct(&mut extracted, &arg, &mut placeholders) else {
            panic!("#[step_args] references should error");
        };
        assert!(err
            .to_string()
            .contains("#[step_args] parameters must own their struct type"));
    }
}
