//! Parses arguments supplied to the `scenarios!` macro.
//!
//! Accepts either a positional directory literal or the `dir = "..."` and
//! `path = "..."` named arguments alongside an optional `tags = "..."` filter,
//! an optional `fixtures = [name: Type, ...]` list, and an optional
//! `runtime = "..."` mode selection.
//! The parser enforces that each input appears at most once, mirroring both
//! accepted spellings in duplicate and missing-argument diagnostics so users
//! immediately see which synonym needs adjusting.

pub(crate) use rstest_bdd_policy::{RuntimeMode, TestAttributeHint};
use syn::LitStr;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

/// A single fixture specification: `name: Type`.
#[derive(Clone, Debug)]
pub(super) struct FixtureSpec {
    pub(super) name: syn::Ident,
    pub(super) ty: syn::Type,
}

impl Parse for FixtureSpec {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name: syn::Ident = input.parse()?;
        input.parse::<syn::token::Colon>()?;
        let ty: syn::Type = input.parse()?;
        Ok(Self { name, ty })
    }
}

pub(super) struct ScenariosArgs {
    pub(super) dir: LitStr,
    pub(super) tag_filter: Option<LitStr>,
    pub(super) fixtures: Vec<FixtureSpec>,
    pub(super) runtime: RuntimeMode,
    pub(super) harness: Option<syn::Path>,
    pub(super) attributes: Option<syn::Path>,
}

enum ScenariosArg {
    Dir(LitStr),
    Tags(LitStr),
    Fixtures(Vec<FixtureSpec>),
    Runtime(RuntimeMode),
    Harness(syn::Path),
    Attributes(syn::Path),
}

impl Parse for ScenariosArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(LitStr) {
            Ok(Self::Dir(input.parse()?))
        } else {
            let ident: syn::Ident = input.parse()?;
            input.parse::<syn::token::Eq>()?;
            parse_named_arg(&ident, input)
        }
    }
}

/// Parse a named argument based on its identifier.
fn parse_named_arg(ident: &syn::Ident, input: ParseStream<'_>) -> syn::Result<ScenariosArg> {
    match ident.to_string().as_str() {
        "dir" | "path" => Ok(ScenariosArg::Dir(input.parse()?)),
        "tags" => Ok(ScenariosArg::Tags(input.parse()?)),
        "fixtures" => parse_fixtures_arg(input),
        "runtime" => parse_runtime_arg(input),
        "harness" => Ok(ScenariosArg::Harness(input.parse()?)),
        "attributes" => Ok(ScenariosArg::Attributes(input.parse()?)),
        _ => Err(input.error(
            "expected `dir`, `path`, `tags`, `fixtures`, `runtime`, `harness`, or `attributes`",
        )),
    }
}

/// Parse the fixtures argument: `fixtures = [name: Type, ...]`
fn parse_fixtures_arg(input: ParseStream<'_>) -> syn::Result<ScenariosArg> {
    let content;
    syn::bracketed!(content in input);
    let specs = Punctuated::<FixtureSpec, Comma>::parse_terminated(&content)?;
    Ok(ScenariosArg::Fixtures(specs.into_iter().collect()))
}

/// Parse the runtime argument: `runtime = "tokio-current-thread"`
fn parse_runtime_arg(input: ParseStream<'_>) -> syn::Result<ScenariosArg> {
    let value: LitStr = input.parse()?;
    let mode = parse_runtime_mode(&value)?;
    Ok(ScenariosArg::Runtime(mode))
}

/// Parse a runtime mode string into a `RuntimeMode` enum.
fn parse_runtime_mode(value: &LitStr) -> syn::Result<RuntimeMode> {
    match value.value().as_str() {
        "tokio-current-thread" => Ok(RuntimeMode::TokioCurrentThread),
        other => Err(syn::Error::new(
            value.span(),
            format!("unknown runtime `{other}`; supported: \"tokio-current-thread\""),
        )),
    }
}

/// Assign `value` to `slot` if empty, or return a duplicate-argument error.
fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    label: &str,
    input: ParseStream<'_>,
) -> syn::Result<()> {
    if slot.is_some() {
        return Err(input.error(format!("duplicate `{label}` argument")));
    }
    *slot = Some(value);
    Ok(())
}

/// Process each parsed argument and populate the corresponding field.
#[expect(
    clippy::type_complexity,
    reason = "flat tuple avoids a single-use struct"
)]
fn process_args(
    args: Punctuated<ScenariosArg, Comma>,
    input: ParseStream<'_>,
) -> syn::Result<(
    Option<LitStr>,
    Option<LitStr>,
    Option<Vec<FixtureSpec>>,
    Option<RuntimeMode>,
    Option<syn::Path>,
    Option<syn::Path>,
)> {
    let mut dir = None;
    let mut tag_filter = None;
    let mut fixtures = None;
    let mut runtime = None;
    let mut harness = None;
    let mut attributes = None;

    for arg in args {
        match arg {
            ScenariosArg::Dir(lit) => set_once(&mut dir, lit, "dir/path", input)?,
            ScenariosArg::Tags(lit) => set_once(&mut tag_filter, lit, "tags", input)?,
            ScenariosArg::Fixtures(specs) => {
                set_once(&mut fixtures, specs, "fixtures", input)?;
            }
            ScenariosArg::Runtime(mode) => set_once(&mut runtime, mode, "runtime", input)?,
            ScenariosArg::Harness(p) => set_once(&mut harness, p, "harness", input)?,
            ScenariosArg::Attributes(p) => {
                set_once(&mut attributes, p, "attributes", input)?;
            }
        }
    }

    Ok((dir, tag_filter, fixtures, runtime, harness, attributes))
}

impl Parse for ScenariosArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let args = Punctuated::<ScenariosArg, Comma>::parse_terminated(input)?;
        let (dir, tag_filter, fixtures, runtime, harness, attributes) = process_args(args, input)?;

        let dir = dir.ok_or_else(|| input.error("`dir` (or `path`) argument is required"))?;

        Ok(Self {
            dir,
            tag_filter,
            fixtures: fixtures.unwrap_or_default(),
            runtime: runtime.unwrap_or_default(),
            harness,
            attributes,
        })
    }
}

#[cfg(test)]
mod tests;
