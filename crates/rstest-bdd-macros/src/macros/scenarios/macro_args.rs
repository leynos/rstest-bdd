//! Parses arguments supplied to the `scenarios!` macro.
//!
//! Accepts either a positional directory literal or the `dir = "..."` and
//! `path = "..."` named arguments alongside an optional `tags = "..."` filter.
//! The parser enforces that each input appears at most once, mirroring both
//! accepted spellings in duplicate and missing-argument diagnostics so users
//! immediately see which synonym needs adjusting.

use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::LitStr;

pub(super) struct ScenariosArgs {
    pub(super) dir: LitStr,
    pub(super) tag_filter: Option<LitStr>,
}

enum ScenariosArg {
    Dir(LitStr),
    Tags(LitStr),
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
            } else {
                Err(input.error("expected `dir`, `path`, or `tags`"))
            }
        }
    }
}

impl Parse for ScenariosArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let args = Punctuated::<ScenariosArg, Comma>::parse_terminated(input)?;
        let mut dir = None;
        let mut tag_filter = None;

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
            }
        }

        let dir = dir.ok_or_else(|| input.error("`dir` (or `path`) is required"))?;

        Ok(Self { dir, tag_filter })
    }
}
