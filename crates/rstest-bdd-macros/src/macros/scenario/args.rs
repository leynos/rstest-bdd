//! Argument parsing for `#[scenario]` covering required `path`, mutually
//! exclusive `index`/`name` selectors, optional tag filters, and optional
//! harness adapter and attribute policy paths. Reports duplicates and
//! conflicts with combined `syn::Error`s.

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
    pub(super) harness: Option<syn::Path>,
    pub(super) attributes: Option<syn::Path>,
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
    Harness(syn::Path),
    Attributes(syn::Path),
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
            } else if ident == "harness" {
                Ok(Self::Harness(input.parse()?))
            } else if ident == "attributes" {
                Ok(Self::Attributes(input.parse()?))
            } else {
                Err(input
                    .error("expected `path`, `index`, `name`, `tags`, `harness`, or `attributes`"))
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
        let mut harness = None;
        let mut attributes = None;

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
                ScenarioArg::Harness(p) => {
                    if harness.is_some() {
                        return Err(input.error("duplicate `harness` argument"));
                    }
                    harness = Some(p);
                }
                ScenarioArg::Attributes(p) => {
                    if attributes.is_some() {
                        return Err(input.error("duplicate `attributes` argument"));
                    }
                    attributes = Some(p);
                }
            }
        }

        let path = path.ok_or_else(|| input.error("`path` argument is required"))?;

        Ok(Self {
            path,
            selector,
            tag_filter,
            harness,
            attributes,
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

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code uses infallible unwraps for clarity"
)]
mod tests {
    use super::ScenarioArgs;
    use quote::quote;

    fn parse_scenario_args(tokens: proc_macro2::TokenStream) -> syn::Result<ScenarioArgs> {
        syn::parse2(tokens)
    }

    fn assert_parse_error_contains(result: syn::Result<ScenarioArgs>, expected_keyword: &str) {
        match result {
            Ok(_) => panic!("parsing should fail"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains(expected_keyword),
                    "error should contain '{expected_keyword}': {msg}"
                );
            }
        }
    }

    #[test]
    fn parses_harness_argument() {
        let args = parse_scenario_args(quote!(
            path = "test.feature",
            harness = rstest_bdd_harness::StdHarness
        ))
        .unwrap();
        assert_eq!(args.path.value(), "test.feature");
        let harness = args.harness.expect("harness should be set");
        let harness_str = quote!(#harness).to_string();
        assert!(
            harness_str.contains("StdHarness"),
            "should contain StdHarness: {harness_str}"
        );
    }

    #[test]
    fn parses_attributes_argument() {
        let args = parse_scenario_args(quote!(
            path = "test.feature",
            attributes = rstest_bdd_harness::DefaultAttributePolicy
        ))
        .unwrap();
        let attr_policy = args.attributes.expect("attributes should be set");
        let attr_str = quote!(#attr_policy).to_string();
        assert!(
            attr_str.contains("DefaultAttributePolicy"),
            "should contain DefaultAttributePolicy: {attr_str}"
        );
    }

    #[test]
    fn parses_harness_and_attributes_together() {
        let args = parse_scenario_args(quote!(
            path = "test.feature",
            harness = my::Harness,
            attributes = my::Policy
        ))
        .unwrap();
        assert!(args.harness.is_some());
        assert!(args.attributes.is_some());
    }

    #[test]
    fn parses_harness_with_all_other_arguments() {
        let args = parse_scenario_args(quote!(
            path = "test.feature",
            name = "My scenario",
            tags = "@fast",
            harness = my::Harness,
            attributes = my::Policy
        ))
        .unwrap();
        assert_eq!(args.path.value(), "test.feature");
        assert!(args.selector.is_some());
        assert!(args.tag_filter.is_some());
        assert!(args.harness.is_some());
        assert!(args.attributes.is_some());
    }

    #[test]
    fn defaults_harness_and_attributes_to_none() {
        let args = parse_scenario_args(quote!(path = "test.feature")).unwrap();
        assert!(args.harness.is_none());
        assert!(args.attributes.is_none());
    }

    #[test]
    fn rejects_duplicate_harness() {
        let result = parse_scenario_args(quote!(
            path = "test.feature",
            harness = a::H,
            harness = b::H
        ));
        assert_parse_error_contains(result, "duplicate");
    }

    #[test]
    fn rejects_duplicate_attributes() {
        let result = parse_scenario_args(quote!(
            path = "test.feature",
            attributes = a::P,
            attributes = b::P
        ));
        assert_parse_error_contains(result, "duplicate");
    }

    #[test]
    fn rejects_unknown_argument() {
        let result = parse_scenario_args(quote!(path = "test.feature", unknown = "value"));
        assert!(result.is_err());
    }
}
