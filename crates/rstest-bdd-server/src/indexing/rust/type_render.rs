//! Type rendering helpers for Rust step indexing.
//!
//! The language server stores type information as strings for display and
//! diagnostics. We avoid `quote` here by rendering common `syn::Type` shapes
//! directly, falling back to `Debug` output for rarely used syntaxes.

use std::fmt::Write;

use syn::{Expr, GenericArgument, Path, PathArguments, ReturnType, Type, TypeParamBound};

pub(super) fn render_type(ty: &Type) -> String {
    match ty {
        Type::Path(type_path) => render_path(&type_path.path),
        Type::BareFn(bare_fn) => render_bare_fn(bare_fn),
        Type::Reference(type_ref) => render_reference(type_ref),
        Type::Tuple(tuple) => render_tuple(tuple),
        Type::Slice(slice) => format!("[{}]", render_type(&slice.elem)),
        Type::Array(array) => {
            let len = render_expr(&array.len);
            format!("[{}; {len}]", render_type(&array.elem))
        }
        Type::Paren(paren) => format!("({})", render_type(&paren.elem)),
        Type::Group(group) => render_type(&group.elem),
        Type::Ptr(ptr) => {
            let mut rendered = String::from("*");
            rendered.push_str(if ptr.mutability.is_some() {
                "mut "
            } else {
                "const "
            });
            rendered.push_str(&render_type(&ptr.elem));
            rendered
        }
        Type::TraitObject(trait_object) => render_trait_object(trait_object),
        Type::Never(_) => "!".to_string(),
        other => format!("{other:?}"),
    }
}

fn render_reference(type_ref: &syn::TypeReference) -> String {
    let mut rendered = String::from("&");
    if let Some(lifetime) = &type_ref.lifetime {
        rendered.push('\'');
        rendered.push_str(&lifetime.ident.to_string());
        rendered.push(' ');
    }
    if type_ref.mutability.is_some() {
        rendered.push_str("mut ");
    }
    rendered.push_str(&render_type(&type_ref.elem));
    rendered
}

fn render_tuple(tuple: &syn::TypeTuple) -> String {
    if tuple.elems.is_empty() {
        return "()".to_string();
    }

    let elems = tuple
        .elems
        .iter()
        .map(render_type)
        .collect::<Vec<_>>()
        .join(", ");
    if tuple.elems.len() == 1 {
        format!("({elems},)")
    } else {
        format!("({elems})")
    }
}

/// Render the unsafety and ABI prefix for a bare function type.
fn render_fn_prefix(unsafety: Option<&syn::token::Unsafe>, abi: Option<&syn::Abi>) -> String {
    let mut prefix = String::new();
    if unsafety.is_some() {
        prefix.push_str("unsafe ");
    }
    if let Some(abi) = abi {
        prefix.push_str("extern ");
        if let Some(name) = &abi.name {
            let _ = write!(prefix, "{:?} ", name.value());
        }
    }
    prefix
}

/// Render variadic parameters suffix if present.
fn render_variadic(variadic: Option<&syn::BareVariadic>, has_inputs: bool) -> String {
    if variadic.is_some() {
        if has_inputs {
            ", ...".to_string()
        } else {
            "...".to_string()
        }
    } else {
        String::new()
    }
}

fn render_bare_fn(bare_fn: &syn::TypeBareFn) -> String {
    let mut rendered = render_fn_prefix(bare_fn.unsafety.as_ref(), bare_fn.abi.as_ref());
    rendered.push_str("fn(");
    let inputs = bare_fn
        .inputs
        .iter()
        .map(|arg| render_type(&arg.ty))
        .collect::<Vec<_>>()
        .join(", ");
    rendered.push_str(&inputs);
    rendered.push_str(&render_variadic(
        bare_fn.variadic.as_ref(),
        !bare_fn.inputs.is_empty(),
    ));
    rendered.push(')');

    if let ReturnType::Type(_, ty) = &bare_fn.output {
        rendered.push_str(" -> ");
        rendered.push_str(&render_type(ty));
    }

    rendered
}

fn render_trait_object(trait_object: &syn::TypeTraitObject) -> String {
    let bounds = trait_object
        .bounds
        .iter()
        .map(|bound| match bound {
            TypeParamBound::Trait(trait_bound) => {
                let mut rendered = String::new();
                if let syn::TraitBoundModifier::Maybe(_) = trait_bound.modifier {
                    rendered.push('?');
                }
                rendered.push_str(&render_path(&trait_bound.path));
                rendered
            }
            TypeParamBound::Lifetime(lifetime) => format!("'{}", lifetime.ident),
            other => format!("{other:?}"),
        })
        .collect::<Vec<_>>()
        .join(" + ");

    if bounds.is_empty() {
        "dyn".to_string()
    } else {
        format!("dyn {bounds}")
    }
}

fn render_path(path: &Path) -> String {
    let mut rendered = String::new();
    if path.leading_colon.is_some() {
        rendered.push_str("::");
    }

    for (idx, segment) in path.segments.iter().enumerate() {
        if idx > 0 {
            rendered.push_str("::");
        }
        rendered.push_str(&segment.ident.to_string());
        rendered.push_str(&render_path_arguments(&segment.arguments));
    }

    rendered
}

fn render_path_arguments(arguments: &PathArguments) -> String {
    match arguments {
        PathArguments::None => String::new(),
        PathArguments::AngleBracketed(angle_bracketed) => {
            let args = angle_bracketed
                .args
                .iter()
                .map(render_generic_argument)
                .collect::<Vec<_>>()
                .join(", ");
            format!("<{args}>")
        }
        PathArguments::Parenthesized(parenthesized) => {
            let inputs = parenthesized
                .inputs
                .iter()
                .map(render_type)
                .collect::<Vec<_>>()
                .join(", ");
            let output = match &parenthesized.output {
                ReturnType::Default => String::new(),
                ReturnType::Type(_, ty) => format!(" -> {}", render_type(ty)),
            };
            format!("({inputs}){output}")
        }
    }
}

fn render_generic_argument(argument: &GenericArgument) -> String {
    match argument {
        GenericArgument::Type(ty) => render_type(ty),
        GenericArgument::Lifetime(lifetime) => format!("'{}", lifetime.ident),
        GenericArgument::Const(expr) => render_expr(expr),
        other => format!("{other:?}"),
    }
}

fn render_lit(lit: &syn::Lit) -> String {
    match lit {
        syn::Lit::Int(lit) => lit.to_string(),
        syn::Lit::Bool(lit) => lit.value.to_string(),
        syn::Lit::Char(lit) => format!("{:?}", lit.value()),
        syn::Lit::Str(lit) => format!("{:?}", lit.value()),
        other => format!("{other:?}"),
    }
}

fn render_expr(expr: &Expr) -> String {
    match expr {
        Expr::Lit(expr_lit) => render_lit(&expr_lit.lit),
        Expr::Path(expr_path) => render_path(&expr_path.path),
        other => format!("{other:?}"),
    }
}
