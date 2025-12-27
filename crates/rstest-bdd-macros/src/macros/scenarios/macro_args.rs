//! Parses arguments supplied to the `scenarios!` macro.
//!
//! Accepts either a positional directory literal or the `dir = "..."` and
//! `path = "..."` named arguments alongside an optional `tags = "..."` filter
//! and an optional `fixtures = [...]` list of fixture names to inject.
//! The parser enforces that each input appears at most once, mirroring both
//! accepted spellings in duplicate and missing-argument diagnostics so users
//! immediately see which synonym needs adjusting.

use syn::LitStr;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

/// Parsed fixture specification from `fixtures = [foo, bar]`.
pub(super) struct FixtureSpec {
    pub(super) name: syn::Ident,
    pub(super) ty: syn::Type,
}

pub(super) struct ScenariosArgs {
    pub(super) dir: LitStr,
    pub(super) tag_filter: Option<LitStr>,
    pub(super) fixtures: Vec<FixtureSpec>,
}

enum ScenariosArg {
    Dir(LitStr),
    Tags(LitStr),
    Fixtures(Vec<FixtureSpec>),
}

/// Parse a single fixture specification: `name: Type`.
fn parse_fixture_spec(input: ParseStream<'_>) -> syn::Result<FixtureSpec> {
    let name: syn::Ident = input.parse()?;
    input.parse::<syn::Token![:]>()?;
    let ty: syn::Type = input.parse()?;
    Ok(FixtureSpec { name, ty })
}

/// Parse a bracketed list of fixture specifications: `[foo: Foo, bar: Bar]`.
fn parse_fixture_list(input: ParseStream<'_>) -> syn::Result<Vec<FixtureSpec>> {
    let content;
    syn::bracketed!(content in input);
    let specs: Punctuated<FixtureSpec, Comma> =
        content.parse_terminated(parse_fixture_spec, Comma)?;
    Ok(specs.into_iter().collect())
}

impl Parse for ScenariosArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(LitStr) {
            Ok(Self::Dir(input.parse()?))
        } else {
            let ident: syn::Ident = input.parse()?;
            input.parse::<syn::token::Eq>()?;
            if ident == "dir" || ident == "path" {
                Ok(Self::Dir(input.parse()?))
            } else if ident == "tags" {
                Ok(Self::Tags(input.parse()?))
            } else if ident == "fixtures" {
                Ok(Self::Fixtures(parse_fixture_list(input)?))
            } else {
                Err(input.error("expected `dir`, `path`, `tags`, or `fixtures`"))
            }
        }
    }
}

impl Parse for ScenariosArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let args = Punctuated::<ScenariosArg, Comma>::parse_terminated(input)?;
        let mut dir = None;
        let mut tag_filter = None;
        let mut fixtures = Vec::new();

        for arg in args {
            match arg {
                ScenariosArg::Dir(lit) => {
                    if dir.is_some() {
                        return Err(input.error("duplicate `dir`/`path` argument"));
                    }
                    dir = Some(lit);
                }
                ScenariosArg::Tags(lit) => {
                    if tag_filter.is_some() {
                        return Err(input.error("duplicate `tags` argument"));
                    }
                    tag_filter = Some(lit);
                }
                ScenariosArg::Fixtures(specs) => {
                    if !fixtures.is_empty() {
                        return Err(input.error("duplicate `fixtures` argument"));
                    }
                    fixtures = specs;
                }
            }
        }

        let dir = dir.ok_or_else(|| input.error("`dir` (or `path`) argument is required"))?;

        Ok(Self {
            dir,
            tag_filter,
            fixtures,
        })
    }
}
