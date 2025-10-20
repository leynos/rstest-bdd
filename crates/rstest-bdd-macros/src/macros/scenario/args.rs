use proc_macro2::Span;
use syn::{
    LitInt, LitStr,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
};

pub(super) struct ScenarioArgs {
    pub(super) path: LitStr,
    pub(super) selector: Option<ScenarioSelector>,
    pub(super) tag_filter: Option<LitStr>,
}

pub(super) enum ScenarioSelector {
    Index { value: usize, span: Span },
    Name { value: String, span: Span },
}

enum ScenarioArg {
    Path(LitStr),
    Index(LitInt),
    Name(LitStr),
    Tags(LitStr),
}

impl Parse for ScenarioArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(LitStr) {
            let lit: LitStr = input.parse()?;
            Ok(Self::Path(lit))
        } else {
            let ident: syn::Ident = input.parse()?;
            input.parse::<syn::token::Eq>()?;
            if ident == "path" {
                Ok(Self::Path(input.parse()?))
            } else if ident == "index" {
                Ok(Self::Index(input.parse()?))
            } else if ident == "name" {
                Ok(Self::Name(input.parse()?))
            } else if ident == "tags" {
                Ok(Self::Tags(input.parse()?))
            } else {
                Err(input.error("expected `path`, `index`, `name`, or `tags`"))
            }
        }
    }
}

impl Parse for ScenarioArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let args = Punctuated::<ScenarioArg, Comma>::parse_terminated(input)?;
        let mut path = None;
        let mut selector = None;
        let mut tag_filter = None;

        for arg in args {
            match arg {
                ScenarioArg::Path(lit) => {
                    if path.is_some() {
                        return Err(input.error("duplicate `path` argument"));
                    }
                    path = Some(lit);
                }
                ScenarioArg::Index(i) => {
                    if let Some(existing) = &selector {
                        return Err(selector_conflict_error(
                            existing,
                            SelectorKind::Index,
                            i.span(),
                        ));
                    }
                    let value = i.base10_parse()?;
                    selector = Some(ScenarioSelector::Index {
                        value,
                        span: i.span(),
                    });
                }
                ScenarioArg::Name(lit) => {
                    if let Some(existing) = &selector {
                        return Err(selector_conflict_error(
                            existing,
                            SelectorKind::Name,
                            lit.span(),
                        ));
                    }
                    selector = Some(ScenarioSelector::Name {
                        value: lit.value(),
                        span: lit.span(),
                    });
                }
                ScenarioArg::Tags(lit) => {
                    if tag_filter.is_some() {
                        return Err(input.error("duplicate `tags` argument"));
                    }
                    tag_filter = Some(lit);
                }
            }
        }

        let path = path.ok_or_else(|| input.error("`path` is required"))?;

        Ok(Self {
            path,
            selector,
            tag_filter,
        })
    }
}

enum SelectorKind {
    Index,
    Name,
}

fn selector_conflict_error(
    existing: &ScenarioSelector,
    new_kind: SelectorKind,
    new_span: Span,
) -> syn::Error {
    match (existing, new_kind) {
        (ScenarioSelector::Index { .. }, SelectorKind::Index) => {
            syn::Error::new(new_span, "duplicate `index` argument")
        }
        (ScenarioSelector::Name { .. }, SelectorKind::Name) => {
            syn::Error::new(new_span, "duplicate `name` argument")
        }
        (ScenarioSelector::Index { span, .. }, SelectorKind::Name) => {
            let mut err = syn::Error::new(
                new_span,
                "`name` cannot be combined with `index`; choose one selector",
            );
            err.combine(syn::Error::new(
                *span,
                "`index` cannot be combined with `name`",
            ));
            err
        }
        (ScenarioSelector::Name { span, .. }, SelectorKind::Index) => {
            let mut err = syn::Error::new(new_span, "`index` cannot be combined with `name`");
            err.combine(syn::Error::new(
                *span,
                "`name` cannot be combined with `index`; choose one selector",
            ));
            err
        }
    }
}
