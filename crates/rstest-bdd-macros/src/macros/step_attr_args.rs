//! Parsing for arguments shared by the three step attribute macros.

use syn::parse::{Parse, ParseStream};

use crate::return_classifier::ReturnOverride;

/// Parsed arguments for step attribute macros.
///
/// Supports an optional step pattern literal and an optional return override hint.
pub(super) struct StepAttrArgs {
    /// Stores the internal `pattern` value.
    pub(super) pattern: Option<syn::LitStr>,
    /// Stores the internal `return_override` value.
    pub(super) return_override: Option<ReturnOverride>,
}

impl Parse for StepAttrArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self {
                pattern: None,
                return_override: None,
            });
        }

        if input.peek(syn::Ident) {
            if let Some(result) = try_parse_expr_syntax(input)? {
                return Ok(result);
            }
        }

        if input.peek(syn::LitStr) {
            let pattern: syn::LitStr = input.parse()?;
            let return_override = parse_optional_return_override(input)?;
            return Ok(Self {
                pattern: Some(pattern),
                return_override,
            });
        }

        let return_override = Some(parse_return_override(input)?);
        if !input.is_empty() {
            return Err(input.error("unexpected tokens in step attribute"));
        }
        Ok(Self {
            pattern: None,
            return_override,
        })
    }
}

/// Parses cucumber-rs compatible `expr = "pattern"` syntax when present.
fn try_parse_expr_syntax(input: ParseStream<'_>) -> syn::Result<Option<StepAttrArgs>> {
    let fork = input.fork();
    let Ok(ident) = fork.parse::<syn::Ident>() else {
        return Ok(None);
    };

    if ident != "expr" || !fork.peek(syn::Token![=]) {
        return Ok(None);
    }

    let _: syn::Ident = input.parse()?;
    input.parse::<syn::Token![=]>()?;
    let pattern: syn::LitStr = input.parse()?;
    let return_override = parse_optional_return_override(input)?;

    Ok(Some(StepAttrArgs {
        pattern: Some(pattern),
        return_override,
    }))
}

/// Parses an optional override and rejects any remaining attribute tokens.
fn parse_optional_return_override(input: ParseStream<'_>) -> syn::Result<Option<ReturnOverride>> {
    let return_override = if input.is_empty() {
        None
    } else {
        input.parse::<syn::Token![,]>()?;
        Some(parse_return_override(input)?)
    };

    if !input.is_empty() {
        return Err(input.error("unexpected tokens in step attribute"));
    }

    Ok(return_override)
}

/// Parses a return override hint.
fn parse_return_override(input: ParseStream<'_>) -> syn::Result<ReturnOverride> {
    let ident: syn::Ident = input.parse()?;
    match ident.to_string().as_str() {
        "result" => Ok(ReturnOverride::Result),
        "value" => Ok(ReturnOverride::Value),
        _ => Err(syn::Error::new_spanned(
            ident,
            "expected `result` or `value`",
        )),
    }
}
