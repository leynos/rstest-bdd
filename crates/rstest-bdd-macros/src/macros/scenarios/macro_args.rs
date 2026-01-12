//! Parses arguments supplied to the `scenarios!` macro.
//!
//! Accepts either a positional directory literal or the `dir = "..."` and
//! `path = "..."` named arguments alongside an optional `tags = "..."` filter,
//! an optional `fixtures = [name: Type, ...]` list, and an optional
//! `runtime = "..."` mode selection.
//! The parser enforces that each input appears at most once, mirroring both
//! accepted spellings in duplicate and missing-argument diagnostics so users
//! immediately see which synonym needs adjusting.

use syn::LitStr;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

/// Runtime mode for scenario test execution.
///
/// This enum mirrors [`rstest_bdd::execution::RuntimeMode`] in the runtime crate.
/// The duplication is necessary because proc-macro crates cannot depend on
/// runtime crates at compile time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RuntimeMode {
    /// Synchronous execution (default).
    #[default]
    Sync,
    /// Tokio current-thread runtime (`#[tokio::test(flavor = "current_thread")]`).
    TokioCurrentThread,
}

impl RuntimeMode {
    /// Returns `true` if this mode requires async test generation.
    pub fn is_async(self) -> bool {
        matches!(self, Self::TokioCurrentThread)
    }

    /// Returns a hint for which test attributes to generate.
    ///
    /// This provides a clean abstraction for test attribute decisions,
    /// keeping the policy centralised in `RuntimeMode`.
    pub fn test_attribute_hint(self) -> TestAttributeHint {
        match self {
            Self::Sync => TestAttributeHint::RstestOnly,
            Self::TokioCurrentThread => TestAttributeHint::RstestWithTokioCurrentThread,
        }
    }
}

/// Hint for which test attributes the macro layer should generate.
///
/// This enum mirrors [`rstest_bdd::execution::TestAttributeHint`] and provides
/// a clean abstraction for compile-time test attribute decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TestAttributeHint {
    /// Generate only `#[rstest::rstest]`.
    RstestOnly,
    /// Generate `#[rstest::rstest]` and `#[tokio::test(flavor = "current_thread")]`.
    RstestWithTokioCurrentThread,
}

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
}

enum ScenariosArg {
    Dir(LitStr),
    Tags(LitStr),
    Fixtures(Vec<FixtureSpec>),
    Runtime(RuntimeMode),
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
                let content;
                syn::bracketed!(content in input);
                let specs = Punctuated::<FixtureSpec, Comma>::parse_terminated(&content)?;
                Ok(Self::Fixtures(specs.into_iter().collect()))
            } else if ident == "runtime" {
                let value: LitStr = input.parse()?;
                let mode = match value.value().as_str() {
                    "tokio-current-thread" => RuntimeMode::TokioCurrentThread,
                    other => {
                        return Err(syn::Error::new(
                            value.span(),
                            format!(
                                "unknown runtime `{other}`; \
                                 supported: \"tokio-current-thread\""
                            ),
                        ));
                    }
                };
                Ok(Self::Runtime(mode))
            } else {
                Err(input.error("expected `dir`, `path`, `tags`, `fixtures`, or `runtime`"))
            }
        }
    }
}

impl Parse for ScenariosArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let args = Punctuated::<ScenariosArg, Comma>::parse_terminated(input)?;
        let mut dir = None;
        let mut tag_filter = None;
        let mut fixtures = None;
        let mut runtime = None;

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
                    if fixtures.is_some() {
                        return Err(input.error("duplicate `fixtures` argument"));
                    }
                    fixtures = Some(specs);
                }
                ScenariosArg::Runtime(mode) => {
                    if runtime.is_some() {
                        return Err(input.error("duplicate `runtime` argument"));
                    }
                    runtime = Some(mode);
                }
            }
        }

        let dir = dir.ok_or_else(|| input.error("`dir` (or `path`) argument is required"))?;

        Ok(Self {
            dir,
            tag_filter,
            fixtures: fixtures.unwrap_or_default(),
            runtime: runtime.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code uses infallible unwraps and indexed access for clarity"
)]
mod tests {
    use super::{FixtureSpec, RuntimeMode, ScenariosArgs};
    use quote::quote;
    use syn::parse_quote;

    fn parse_scenarios_args(tokens: proc_macro2::TokenStream) -> syn::Result<ScenariosArgs> {
        syn::parse2(tokens)
    }

    fn parse_fixture_spec(tokens: proc_macro2::TokenStream) -> syn::Result<FixtureSpec> {
        syn::parse2(tokens)
    }

    fn type_to_string(ty: &syn::Type) -> String {
        quote!(#ty).to_string()
    }

    /// Assert that parsing fails and the error message contains the expected keyword.
    fn assert_parse_error_contains(result: syn::Result<ScenariosArgs>, expected_keyword: &str) {
        match result {
            Ok(_) => panic!("parsing should fail"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains(expected_keyword),
                    "error message should contain '{expected_keyword}': {msg}"
                );
            }
        }
    }

    /// Assert that fixture spec parsing fails and the error exists.
    fn assert_fixture_parse_fails(tokens: proc_macro2::TokenStream) {
        assert!(parse_fixture_spec(tokens).is_err(), "parsing should fail");
    }

    // Tests for FixtureSpec parsing

    #[test]
    fn fixture_spec_parses_simple_type() {
        let spec: FixtureSpec = parse_fixture_spec(parse_quote!(world: TestWorld)).unwrap();
        assert_eq!(spec.name.to_string(), "world");
        assert!(type_to_string(&spec.ty).contains("TestWorld"));
    }

    #[test]
    fn fixture_spec_parses_generic_type() {
        let spec: FixtureSpec =
            parse_fixture_spec(parse_quote!(counter: RefCell<CounterWorld>)).unwrap();
        assert_eq!(spec.name.to_string(), "counter");
        let ty_str = type_to_string(&spec.ty);
        assert!(ty_str.contains("RefCell"));
        assert!(ty_str.contains("CounterWorld"));
    }

    #[test]
    fn fixture_spec_parses_path_type() {
        let spec: FixtureSpec =
            parse_fixture_spec(parse_quote!(db: std::sync::Arc<Database>)).unwrap();
        assert_eq!(spec.name.to_string(), "db");
    }

    #[test]
    fn fixture_spec_rejects_missing_colon() {
        assert_fixture_parse_fails(parse_quote!(world TestWorld));
    }

    #[test]
    fn fixture_spec_rejects_missing_type() {
        assert_fixture_parse_fails(parse_quote!(world:));
    }

    // Tests for ScenariosArgs parsing

    #[test]
    fn scenarios_args_parses_positional_dir() {
        let args: ScenariosArgs = parse_scenarios_args(parse_quote!("tests/features")).unwrap();
        assert_eq!(args.dir.value(), "tests/features");
        assert!(args.tag_filter.is_none());
        assert!(args.fixtures.is_empty());
    }

    #[test]
    fn scenarios_args_parses_named_dir() {
        let args: ScenariosArgs =
            parse_scenarios_args(parse_quote!(dir = "tests/features")).unwrap();
        assert_eq!(args.dir.value(), "tests/features");
    }

    #[test]
    fn scenarios_args_parses_named_path() {
        let args: ScenariosArgs =
            parse_scenarios_args(parse_quote!(path = "tests/features")).unwrap();
        assert_eq!(args.dir.value(), "tests/features");
    }

    #[test]
    fn scenarios_args_parses_with_tags() {
        let args: ScenariosArgs =
            parse_scenarios_args(parse_quote!("tests/features", tags = "@fast")).unwrap();
        assert_eq!(args.dir.value(), "tests/features");
        assert_eq!(args.tag_filter.as_ref().unwrap().value(), "@fast");
    }

    #[test]
    fn scenarios_args_parses_single_fixture() {
        let args: ScenariosArgs =
            parse_scenarios_args(parse_quote!("tests/features", fixtures = [world: TestWorld]))
                .unwrap();
        assert_eq!(args.fixtures.len(), 1);
        assert_eq!(args.fixtures[0].name.to_string(), "world");
    }

    #[test]
    fn scenarios_args_parses_multiple_fixtures() {
        let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
            "tests/features",
            fixtures = [world: TestWorld, db: Database]
        ))
        .unwrap();
        assert_eq!(args.fixtures.len(), 2);
        assert_eq!(args.fixtures[0].name.to_string(), "world");
        assert_eq!(args.fixtures[1].name.to_string(), "db");
    }

    #[test]
    fn scenarios_args_parses_all_arguments() {
        let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
            "tests/features",
            tags = "@smoke",
            fixtures = [world: TestWorld]
        ))
        .unwrap();
        assert_eq!(args.dir.value(), "tests/features");
        assert_eq!(args.tag_filter.as_ref().unwrap().value(), "@smoke");
        assert_eq!(args.fixtures.len(), 1);
    }

    #[test]
    fn scenarios_args_allows_arguments_in_any_order() {
        let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
            fixtures = [world: TestWorld],
            tags = "@smoke",
            dir = "tests/features"
        ))
        .unwrap();
        assert_eq!(args.dir.value(), "tests/features");
        assert_eq!(args.tag_filter.as_ref().unwrap().value(), "@smoke");
        assert_eq!(args.fixtures.len(), 1);
    }

    #[test]
    fn scenarios_args_rejects_missing_dir() {
        let result = parse_scenarios_args(parse_quote!(tags = "@fast"));
        assert_parse_error_contains(result, "dir");
    }

    #[test]
    fn scenarios_args_rejects_duplicate_dir() {
        let result = parse_scenarios_args(parse_quote!(dir = "a", path = "b"));
        assert_parse_error_contains(result, "duplicate");
    }

    #[test]
    fn scenarios_args_rejects_duplicate_tags() {
        let result = parse_scenarios_args(parse_quote!("tests/features", tags = "@a", tags = "@b"));
        assert_parse_error_contains(result, "duplicate");
    }

    #[test]
    fn scenarios_args_rejects_duplicate_fixtures() {
        let result = parse_scenarios_args(parse_quote!(
            "tests/features",
            fixtures = [a: A],
            fixtures = [b: B]
        ));
        assert_parse_error_contains(result, "duplicate");
    }

    #[test]
    fn scenarios_args_rejects_unknown_argument() {
        let result = parse_scenarios_args(parse_quote!("tests/features", unknown = "value"));
        assert!(result.is_err());
    }

    #[test]
    fn scenarios_args_parses_empty_fixtures() {
        let args: ScenariosArgs =
            parse_scenarios_args(parse_quote!("tests/features", fixtures = [])).unwrap();
        assert!(args.fixtures.is_empty());
    }

    #[test]
    fn scenarios_args_parses_fixtures_with_trailing_comma() {
        let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
            "tests/features",
            fixtures = [world: TestWorld,]
        ))
        .unwrap();
        assert_eq!(args.fixtures.len(), 1);
    }

    // Tests for runtime argument parsing

    #[test]
    fn scenarios_args_defaults_to_sync_runtime() {
        let args: ScenariosArgs = parse_scenarios_args(parse_quote!("tests/features"))
            .expect("parse_scenarios_args should succeed");
        assert_eq!(args.runtime, RuntimeMode::Sync);
        assert!(!args.runtime.is_async());
    }

    #[test]
    fn scenarios_args_parses_runtime_tokio_current_thread() {
        let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
            "tests/features",
            runtime = "tokio-current-thread"
        ))
        .expect("parse_scenarios_args should succeed");
        assert_eq!(args.runtime, RuntimeMode::TokioCurrentThread);
        assert!(args.runtime.is_async());
    }

    #[test]
    fn scenarios_args_parses_runtime_with_other_arguments() {
        let args: ScenariosArgs = parse_scenarios_args(parse_quote!(
            "tests/features",
            tags = "@async",
            runtime = "tokio-current-thread",
            fixtures = [world: TestWorld]
        ))
        .expect("parse_scenarios_args should succeed");
        assert_eq!(args.dir.value(), "tests/features");
        assert_eq!(
            args.tag_filter
                .as_ref()
                .expect("tag_filter should be set")
                .value(),
            "@async"
        );
        assert_eq!(args.runtime, RuntimeMode::TokioCurrentThread);
        assert_eq!(args.fixtures.len(), 1);
    }

    #[test]
    fn scenarios_args_rejects_unknown_runtime() {
        let result =
            parse_scenarios_args(parse_quote!("tests/features", runtime = "unknown-runtime"));
        assert_parse_error_contains(result, "unknown runtime");
    }

    #[test]
    fn scenarios_args_rejects_duplicate_runtime() {
        let result = parse_scenarios_args(parse_quote!(
            "tests/features",
            runtime = "tokio-current-thread",
            runtime = "tokio-current-thread"
        ));
        assert_parse_error_contains(result, "duplicate");
    }

    // Tests for RuntimeMode::test_attribute_hint

    #[test]
    fn runtime_mode_sync_returns_rstest_only_hint() {
        use super::TestAttributeHint;
        assert_eq!(
            RuntimeMode::Sync.test_attribute_hint(),
            TestAttributeHint::RstestOnly
        );
    }

    #[test]
    fn runtime_mode_tokio_current_thread_returns_rstest_with_tokio_hint() {
        use super::TestAttributeHint;
        assert_eq!(
            RuntimeMode::TokioCurrentThread.test_attribute_hint(),
            TestAttributeHint::RstestWithTokioCurrentThread
        );
    }
}
