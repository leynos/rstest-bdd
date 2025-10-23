//! Parses the arguments passed to the `scenarios!` macro,
//! supporting both positional directory literals and named
//! `tags` filters.

use syn::LitStr;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

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
                        return Err(input.error("duplicate directory argument"));
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

        let dir = dir.ok_or_else(|| input.error("directory argument is required"))?;

        Ok(Self { dir, tag_filter })
    }
}
