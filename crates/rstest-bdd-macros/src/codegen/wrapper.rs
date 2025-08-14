//! Generation of wrapper functions for step definitions.

use super::keyword_to_token;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Fixture argument extracted from a step function.
pub(crate) struct FixtureArg {
    pub(crate) pat: syn::Ident,
    pub(crate) name: syn::Ident,
    pub(crate) ty: syn::Type,
}

/// Non-fixture argument extracted from a step function.
pub(crate) struct StepArg {
    pub(crate) pat: syn::Ident,
    pub(crate) ty: syn::Type,
}

/// Data table argument extracted from a step function.
pub(crate) struct DataTableArg {
    pub(crate) pat: syn::Ident,
}

/// Doc string argument extracted from a step function.
pub(crate) struct DocStringArg {
    pub(crate) pat: syn::Ident,
}

/// Collections of arguments extracted from a step function.
///
/// This container groups fixture, step, data table, and doc string
/// references so functions can operate on ordered parameters without
/// juggling multiple slices.
pub(crate) struct ArgumentCollections<'a> {
    pub(crate) fixtures: &'a [FixtureArg],
    pub(crate) step_args: &'a [StepArg],
    pub(crate) datatable: Option<&'a DataTableArg>,
    pub(crate) docstring: Option<&'a DocStringArg>,
}

/// Argument ordering as declared in the step function signature.
pub(crate) enum CallArg {
    Fixture(usize),
    StepArg(usize),
    DataTable,
    DocString,
}

/// Processes and stores arguments extracted from a step function.
///
/// This helper owns collections for fixture, step, data table, and doc
/// string arguments. Call [`process_argument`] for each parameter in order,
/// then use [`into_parts`] to retrieve the populated vectors.
///
/// # Examples
/// ```rust,ignore
/// use syn::parse_quote;
/// let mut arg: syn::PatType = parse_quote!(value: i32);
/// let mut processor = ArgumentProcessor::new();
/// processor.process_argument(&mut arg).unwrap();
/// let (fixtures, step_args, datatable, docstring, order) = processor.into_parts();
/// assert!(fixtures.is_empty());
/// assert_eq!(step_args.len(), 1);
/// assert!(datatable.is_none());
/// assert!(docstring.is_none());
/// assert_eq!(order.len(), 1);
/// ```
struct ArgumentProcessor {
    fixtures: Vec<FixtureArg>,
    step_args: Vec<StepArg>,
    datatable: Option<DataTableArg>,
    docstring: Option<DocStringArg>,
    call_order: Vec<CallArg>,
}

impl ArgumentProcessor {
    /// Create an empty processor ready to collect arguments.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let processor = ArgumentProcessor::new();
    /// assert!(processor.fixtures.is_empty());
    /// ```
    fn new() -> Self {
        Self {
            fixtures: Vec::new(),
            step_args: Vec::new(),
            datatable: None,
            docstring: None,
            call_order: Vec::new(),
        }
    }

    /// Analyse and store a single function argument.
    ///
    /// This inspects attributes and patterns to determine whether the
    /// parameter refers to a fixture, a regular step argument, a data table or
    /// a doc string, preserving the original declaration order.
    ///
    /// # Examples
    /// ```rust,ignore
    /// use syn::parse_quote;
    /// let mut arg: syn::PatType = parse_quote!(value: i32);
    /// let mut processor = ArgumentProcessor::new();
    /// processor.process_argument(&mut arg).unwrap();
    /// ```
    fn process_argument(&mut self, arg: &mut syn::PatType) -> syn::Result<()> {
        let fixture_name = extract_fixture_name(&mut arg.attrs);
        let pat = validate_and_extract_pattern(&arg.pat)?;
        let ty = (*arg.ty).clone();

        if let Some(name) = fixture_name {
            let idx = self.fixtures.len();
            self.fixtures.push(FixtureArg { pat, name, ty });
            self.call_order.push(CallArg::Fixture(idx));
        } else if is_datatable_arg(&self.datatable, &pat, &ty) {
            validate_datatable_ordering(self.docstring.as_ref(), &arg.pat)?;
            self.datatable = Some(DataTableArg { pat });
            self.call_order.push(CallArg::DataTable);
        } else if is_docstring_arg(&self.docstring, &pat, &ty) {
            self.docstring = Some(DocStringArg { pat });
            self.call_order.push(CallArg::DocString);
        } else {
            let idx = self.step_args.len();
            self.step_args.push(StepArg { pat, ty });
            self.call_order.push(CallArg::StepArg(idx));
        }
        Ok(())
    }

    /// Decompose the processor into its collected argument lists.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let processor = ArgumentProcessor::new();
    /// let (fixtures, step_args, datatable, docstring, order) = processor.into_parts();
    /// assert!(fixtures.is_empty());
    /// assert!(step_args.is_empty());
    /// assert!(datatable.is_none());
    /// assert!(docstring.is_none());
    /// assert!(order.is_empty());
    /// ```
    #[expect(
        clippy::type_complexity,
        reason = "method mirrors extract_args return type"
    )]
    fn into_parts(
        self,
    ) -> (
        Vec<FixtureArg>,
        Vec<StepArg>,
        Option<DataTableArg>,
        Option<DocStringArg>,
        Vec<CallArg>,
    ) {
        (
            self.fixtures,
            self.step_args,
            self.datatable,
            self.docstring,
            self.call_order,
        )
    }
}

/// Recursively collect identifiers bound by a pattern.
///
/// Step parameters currently require a single, plain identifier so the
/// generated wrapper can name the argument. Destructuring patterns are parsed
/// to surface a clearer error message until full support is implemented.
fn extract_identifiers_from_pattern(pat: &syn::Pat) -> syn::Result<Vec<syn::Ident>> {
    match pat {
        syn::Pat::Ident(pat_ident) => Ok(vec![pat_ident.ident.clone()]),
        syn::Pat::Tuple(pat_tuple) => pat_tuple
            .elems
            .iter()
            .map(extract_identifiers_from_pattern)
            .collect::<syn::Result<Vec<_>>>()
            .map(|v| v.into_iter().flatten().collect()),
        syn::Pat::Struct(pat_struct) => pat_struct
            .fields
            .iter()
            .map(|f| extract_identifiers_from_pattern(&f.pat))
            .collect::<syn::Result<Vec<_>>>()
            .map(|v| v.into_iter().flatten().collect()),
        syn::Pat::TupleStruct(pat_tuple_struct) => pat_tuple_struct
            .elems
            .iter()
            .map(extract_identifiers_from_pattern)
            .collect::<syn::Result<Vec<_>>>()
            .map(|v| v.into_iter().flatten().collect()),
        syn::Pat::Paren(pat_paren) => extract_identifiers_from_pattern(&pat_paren.pat),
        _ => Err(syn::Error::new_spanned(
            pat,
            "unsupported pattern type - only identifiers, tuples, structs, and tuple structs are supported",
        )),
    }
}

/// Remove the `from` attribute from a parameter and return the fixture name.
///
/// # Examples
/// ```rust,ignore
/// # use syn::{parse_quote, Attribute};
/// let mut attrs: Vec<Attribute> = vec![parse_quote!(#[from(foo)])];
/// let name = extract_fixture_name(&mut attrs);
/// assert_eq!(name.unwrap().to_string(), "foo");
/// assert!(attrs.is_empty());
/// ```
fn extract_fixture_name(attrs: &mut Vec<syn::Attribute>) -> Option<syn::Ident> {
    let mut fixture_name = None;
    attrs.retain(|a| {
        if a.path().is_ident("from") {
            fixture_name = a.parse_args::<syn::Ident>().ok();
            false
        } else {
            true
        }
    });
    fixture_name
}

/// Validate that the pattern is a single bare identifier and return it.
///
/// # Examples
/// ```rust,ignore
/// # use syn::parse_quote;
/// let pat: syn::Pat = parse_quote!(value);
/// let ident = validate_and_extract_pattern(&pat).unwrap();
/// assert_eq!(ident.to_string(), "value");
/// ```
fn validate_and_extract_pattern(pat: &syn::Pat) -> syn::Result<syn::Ident> {
    let pat_idents = extract_identifiers_from_pattern(pat)?;
    if pat_idents.len() != 1 || !matches!(pat, syn::Pat::Ident(_)) {
        return Err(syn::Error::new_spanned(
            pat,
            format!(
                "complex destructuring patterns are not yet supported - pattern resolves to {} identifiers but a single bare identifier is required. Found pattern: `{}`",
                pat_idents.len(),
                pat.to_token_stream()
            ),
        ));
    }
    #[expect(clippy::expect_used, reason = "length checked above")]
    Ok(pat_idents
        .into_iter()
        .next()
        .expect("pattern resolves to exactly one identifier"))
}

/// Ensure a datatable parameter appears before any docstring parameter.
///
/// # Examples
/// ```rust,ignore
/// # use syn::parse_quote;
/// let pat: syn::Pat = parse_quote!(datatable);
/// let doc = Some(DocStringArg { pat: parse_quote!(docstring) });
/// assert!(validate_datatable_ordering(&doc, &pat).is_err());
/// ```
fn validate_datatable_ordering(
    existing_docstring: Option<&DocStringArg>,
    pat: &syn::Pat,
) -> syn::Result<()> {
    if existing_docstring.is_some() {
        return Err(syn::Error::new_spanned(
            pat,
            "datatable must be declared before docstring to match Gherkin ordering",
        ));
    }
    Ok(())
}

/// Extract fixture and step arguments from a function signature.
#[expect(clippy::type_complexity, reason = "return type defined by API")]
// FIXME: https://github.com/leynos/rstest-bdd/issues/54
pub(crate) fn extract_args(
    func: &mut syn::ItemFn,
) -> syn::Result<(
    Vec<FixtureArg>,
    Vec<StepArg>,
    Option<DataTableArg>,
    Option<DocStringArg>,
    Vec<CallArg>,
)> {
    let mut processor = ArgumentProcessor::new();

    for input in &mut func.sig.inputs {
        let syn::FnArg::Typed(arg) = input else {
            return Err(syn::Error::new_spanned(input, "methods not supported"));
        };

        processor.process_argument(arg)?;
    }

    Ok(processor.into_parts())
}

fn is_vec_vec_string(ty: &syn::Type) -> bool {
    is_vec_of(ty, |inner| is_vec_of(inner, is_string_type))
}

fn is_vec_of<F>(ty: &syn::Type, check_inner: F) -> bool
where
    F: FnOnce(&syn::Type) -> bool,
{
    use syn::{GenericArgument, PathArguments, Type};

    let Type::Path(tp) = ty else { return false };
    let Some(seg) = tp.path.segments.last() else {
        return false;
    };
    if seg.ident != "Vec" {
        return false;
    }
    let PathArguments::AngleBracketed(args) = &seg.arguments else {
        return false;
    };
    let Some(GenericArgument::Type(inner_ty)) = args.args.first() else {
        return false;
    };

    check_inner(inner_ty)
}

fn is_string_type(ty: &syn::Type) -> bool {
    matches!(
        ty,
        syn::Type::Path(tp) if tp.path.segments.last().is_some_and(|seg| seg.ident == "String")
    )
}

/// Determines if a function parameter should be treated as a docstring argument.
///
/// Detection relies on the parameter being named `docstring` and having the
/// concrete type `String`. Renaming the parameter or using a type alias will
/// prevent detection.
///
/// # Examples
/// ```rust,ignore
/// # use proc_macro2::Span;
/// # use syn::{parse_quote, Ident};
/// let ty: syn::Type = parse_quote! { String };
/// let name = Ident::new("docstring", Span::call_site());
/// let none = None;
/// assert!(is_docstring_arg(&none, &name, &ty));
/// ```
#[expect(clippy::ref_option, reason = "signature defined by requirements")]
// FIXME: https://github.com/leynos/rstest-bdd/issues/54
fn is_docstring_arg(
    existing_docstring: &Option<DocStringArg>,
    param_name: &syn::Ident,
    param_type: &syn::Type,
) -> bool {
    existing_docstring.is_none() && param_name == "docstring" && is_string_type(param_type)
}

/// Determines if a function parameter should be treated as a datatable argument.
///
/// Detection relies on the parameter being named `datatable` and having the
/// concrete type `Vec<Vec<String>>`. Renaming the parameter or using a type alias
/// will prevent detection.
///
/// # Examples
/// ```rust,ignore
/// # use proc_macro2::Span;
/// # use syn::{parse_quote, Ident};
/// let ty: syn::Type = parse_quote! { Vec<Vec<String>> };
/// let name = Ident::new("datatable", Span::call_site());
/// let none = None;
/// assert!(is_datatable_arg(&none, &name, &ty));
/// ```
#[expect(clippy::ref_option, reason = "signature defined by requirements")]
// FIXME: https://github.com/leynos/rstest-bdd/issues/54
fn is_datatable_arg(
    existing_datatable: &Option<DataTableArg>,
    param_name: &syn::Ident,
    param_type: &syn::Type,
) -> bool {
    existing_datatable.is_none() && param_name == "datatable" && is_vec_vec_string(param_type)
}

/// Generate declaration for a data table argument.
fn gen_datatable_decl(
    datatable: Option<&DataTableArg>,
    pattern: &syn::LitStr,
) -> Option<TokenStream2> {
    datatable.map(|DataTableArg { pat }| {
        quote! {
            let #pat: Vec<Vec<String>> = _table
                .ok_or_else(|| format!("Step '{}' requires a data table", #pattern))?
                .iter()
                .map(|row| row.iter().map(|cell| cell.to_string()).collect())
                .collect();
        }
    })
}

/// Generate declaration for a doc string argument.
///
/// Step functions require an owned `String`, so the wrapper copies the block.
fn gen_docstring_decl(
    docstring: Option<&DocStringArg>,
    pattern: &syn::LitStr,
) -> Option<TokenStream2> {
    docstring.map(|DocStringArg { pat }| {
        quote! {
            let #pat: String = _docstring
                .ok_or_else(|| format!("Step '{}' requires a doc string", #pattern))?
                .to_owned();
        }
    })
}

/// Configuration required to generate a wrapper.
pub(crate) struct WrapperConfig<'a> {
    pub(crate) ident: &'a syn::Ident,
    pub(crate) fixtures: &'a [FixtureArg],
    pub(crate) step_args: &'a [StepArg],
    pub(crate) datatable: Option<&'a DataTableArg>,
    pub(crate) docstring: Option<&'a DocStringArg>,
    pub(crate) pattern: &'a syn::LitStr,
    pub(crate) keyword: rstest_bdd::StepKeyword,
    pub(crate) call_order: &'a [CallArg],
}

/// Generate declarations for fixture values.
///
/// Non-reference fixtures must implement [`Clone`] because wrappers clone
/// them to hand ownership to the step function.
fn gen_fixture_decls(fixtures: &[FixtureArg], ident: &syn::Ident) -> Vec<TokenStream2> {
    fixtures
        .iter()
        .map(|FixtureArg { pat, name, ty }| {
            let lookup_ty = if let syn::Type::Reference(r) = ty {
                &*r.elem
            } else {
                ty
            };
            let clone_suffix = if matches!(ty, syn::Type::Reference(_)) {
                quote! {}
            } else {
                quote! { .cloned() }
            };
            quote! {
                let #pat: #ty = ctx
                    .get::<#lookup_ty>(stringify!(#name))
                    #clone_suffix
                    .ok_or_else(|| format!(
                        "Missing fixture '{}' of type '{}' for step function '{}'",
                        stringify!(#name),
                        stringify!(#lookup_ty),
                        stringify!(#ident)
                    ))?;
            }
        })
        .collect()
}

/// Generate code to parse step arguments from regex captures.
fn gen_step_parses(
    step_args: &[StepArg],
    captured: &[TokenStream2],
    pattern: &syn::LitStr,
) -> Vec<TokenStream2> {
    step_args
        .iter()
        .zip(captured.iter().enumerate())
        .map(|(StepArg { pat, ty }, (idx, capture))| {
            let raw_ident = format_ident!("__raw{}", idx);
            quote! {
                let #raw_ident = #capture.unwrap_or_else(|| {
                    panic!(
                        "pattern '{}' missing capture for argument '{}'",
                        #pattern,
                        stringify!(#pat),
                    )
                });
                let #pat: #ty = (#raw_ident).parse().unwrap_or_else(|_| {
                    panic!(
                        "failed to parse argument '{}' of type '{}' from pattern '{}' with captured value: '{:?}'",
                        stringify!(#pat),
                        stringify!(#ty),
                        #pattern,
                        #raw_ident,
                    )
                });
            }
        })
        .collect()
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Generate unique identifiers for the wrapper components.
///
/// Returns identifiers for the wrapper function, fixture array constant, and
/// pattern constant.
///
/// # Examples
/// ```rust,ignore
/// # use syn::Ident;
/// # use proc_macro2::Span;
/// let ident = Ident::new("step_fn", Span::call_site());
/// let (w, c, p) = generate_wrapper_identifiers(&ident, 1);
/// assert!(w.to_string().contains("step_fn"));
/// ```
fn generate_wrapper_identifiers(
    ident: &syn::Ident,
    id: usize,
) -> (proc_macro2::Ident, proc_macro2::Ident, proc_macro2::Ident) {
    let wrapper_ident = format_ident!("__rstest_bdd_wrapper_{}_{}", ident, id);
    let ident_upper = ident.to_string().to_uppercase();
    let const_ident = format_ident!("__RSTEST_BDD_FIXTURES_{}_{}", ident_upper, id);
    let pattern_ident = format_ident!("__RSTEST_BDD_PATTERN_{}_{}", ident_upper, id);
    (wrapper_ident, const_ident, pattern_ident)
}

/// Generate the `StepPattern` constant used by a wrapper.
///
/// # Examples
/// ```rust,ignore
/// # use syn::parse_quote;
/// # use proc_macro2::Span;
/// # let pattern = parse_quote!("^foo$");
/// # let pattern_ident = proc_macro2::Ident::new("PAT", Span::call_site());
/// let tokens = generate_wrapper_signature(&pattern, &pattern_ident);
/// assert!(tokens.to_string().contains("StepPattern"));
/// ```
fn generate_wrapper_signature(
    pattern: &syn::LitStr,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    quote! {
        static #pattern_ident: rstest_bdd::StepPattern =
            rstest_bdd::StepPattern::new(#pattern);
    }
}

/// Generate declarations and parsing logic for wrapper arguments.
///
/// The returned tuple contains fixture declarations, step argument parsers,
/// optional data table handling, and optional doc string handling.
///
/// # Examples
/// ```rust,ignore
/// # use syn::parse_quote;
/// # let ident = syn::Ident::new("step", proc_macro2::Span::call_site());
/// # let pattern = parse_quote!("pattern");
/// # let config = WrapperConfig {
/// #     ident: &ident,
/// #     fixtures: &[],
/// #     step_args: &[],
/// #     datatable: None,
/// #     docstring: None,
/// #     pattern: &pattern,
/// #     keyword: rstest_bdd::StepKeyword::Given,
/// #     call_order: &[],
/// # };
/// let result = generate_argument_processing(&config);
/// assert!(result.0.is_empty());
/// ```
fn generate_argument_processing(
    config: &WrapperConfig<'_>,
) -> (
    Vec<TokenStream2>,
    Vec<TokenStream2>,
    Option<TokenStream2>,
    Option<TokenStream2>,
) {
    let declares = gen_fixture_decls(config.fixtures, config.ident);
    let captured: Vec<_> = config
        .step_args
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            let index = syn::Index::from(idx + 1); // skip full match at index 0
            quote! { captures.get(#index).map(|m| m.as_str()) }
        })
        .collect();
    let step_arg_parses = gen_step_parses(config.step_args, &captured, config.pattern);
    let datatable_decl = gen_datatable_decl(config.datatable, config.pattern);
    let docstring_decl = gen_docstring_decl(config.docstring, config.pattern);
    (declares, step_arg_parses, datatable_decl, docstring_decl)
}

/// Collect argument identifiers in the order declared by the step function.
///
/// # Examples
/// ```rust,ignore
/// # let args = ArgumentCollections {
/// #     fixtures: &[],
/// #     step_args: &[],
/// #     datatable: None,
/// #     docstring: None,
/// # };
/// let idents = collect_ordered_arguments(&[], &args);
/// assert!(idents.is_empty());
/// ```
fn collect_ordered_arguments<'a>(
    call_order: &'a [CallArg],
    args: &ArgumentCollections<'a>,
) -> Vec<&'a syn::Ident> {
    call_order
        .iter()
        .map(|arg| match arg {
            CallArg::Fixture(i) =>
            {
                #[expect(
                    clippy::indexing_slicing,
                    reason = "indices validated during extraction"
                )]
                &args.fixtures[*i].pat
            }
            CallArg::StepArg(i) =>
            {
                #[expect(
                    clippy::indexing_slicing,
                    reason = "indices validated during extraction"
                )]
                &args.step_args[*i].pat
            }
            CallArg::DataTable =>
            {
                #[expect(clippy::expect_used, reason = "variant guarantees presence")]
                &args
                    .datatable
                    .expect("datatable present in call_order but not configured")
                    .pat
            }
            CallArg::DocString =>
            {
                #[expect(clippy::expect_used, reason = "variant guarantees presence")]
                &args
                    .docstring
                    .expect("docstring present in call_order but not configured")
                    .pat
            }
        })
        .collect()
}

/// Assemble the final wrapper function using prepared components.
///
/// # Examples
/// ```rust,ignore
/// # use syn::parse_quote;
/// # let ident = syn::Ident::new("step", proc_macro2::Span::call_site());
/// # let pattern = parse_quote!("pattern");
/// let tokens = assemble_wrapper_function(
///     &proc_macro2::Ident::new("wrapper", proc_macro2::Span::call_site()),
///     &proc_macro2::Ident::new("PAT", proc_macro2::Span::call_site()),
///     (vec![], vec![], None, None),
///     vec![],
///     &pattern,
///     &ident,
/// );
/// assert!(tokens.to_string().contains("fn"));
/// ```
fn assemble_wrapper_function(
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
    arg_processing: (
        Vec<TokenStream2>,
        Vec<TokenStream2>,
        Option<TokenStream2>,
        Option<TokenStream2>,
    ),
    arg_idents: &[&syn::Ident],
    pattern: &syn::LitStr,
    ident: &syn::Ident,
) -> TokenStream2 {
    let (declares, step_arg_parses, datatable_decl, docstring_decl) = arg_processing;
    quote! {
        fn #wrapper_ident(
            ctx: &rstest_bdd::StepContext<'_>,
            text: &str,
            _docstring: Option<&str>,
            _table: Option<&[&[&str]]>,
        ) -> Result<(), String> {
            use std::panic::{catch_unwind, AssertUnwindSafe};

            let captures = #pattern_ident
                .regex()
                .captures(text)
                .ok_or_else(|| format!(
                    "Step text '{}' does not match pattern '{}'",
                    text,
                    #pattern
                ))?;

            #(#declares)*
            #(#step_arg_parses)*
            #datatable_decl
            #docstring_decl

            catch_unwind(AssertUnwindSafe(|| {
                #ident(#(#arg_idents),*);
                Ok(())
            }))
            .map_err(|e| format!(
                "Panic in step '{}', function '{}': {:?}",
                #pattern,
                stringify!(#ident),
                e
            ))?
        }
    }
}

/// Generate the wrapper function body and pattern constant.
///
/// # Examples
/// ```rust,ignore
/// # use syn::parse_quote;
/// # use proc_macro2::Ident;
/// # let ident = Ident::new("step", proc_macro2::Span::call_site());
/// # let config = WrapperConfig {
/// #     ident: &ident,
/// #     fixtures: &[],
/// #     step_args: &[],
/// #     datatable: None,
/// #     docstring: None,
/// #     pattern: &parse_quote!(""),
/// #     keyword: rstest_bdd::StepKeyword::Given,
/// #     call_order: &[],
/// # };
/// # let (wrapper_ident, _, pattern_ident) = generate_wrapper_identifiers(config.ident, 0);
/// let tokens = generate_wrapper_body(&config, &wrapper_ident, &pattern_ident);
/// assert!(tokens.to_string().contains("fn"));
/// ```
fn generate_wrapper_body(
    config: &WrapperConfig<'_>,
    wrapper_ident: &proc_macro2::Ident,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    let WrapperConfig {
        ident,
        fixtures,
        step_args,
        datatable,
        docstring,
        pattern,
        call_order,
        ..
    } = *config;

    let signature = generate_wrapper_signature(pattern, pattern_ident);
    let arg_processing = generate_argument_processing(config);
    let collections = ArgumentCollections {
        fixtures,
        step_args,
        datatable,
        docstring,
    };
    let arg_idents = collect_ordered_arguments(call_order, &collections);
    let wrapper_fn = assemble_wrapper_function(
        wrapper_ident,
        pattern_ident,
        arg_processing,
        &arg_idents,
        pattern,
        ident,
    );

    quote! {
        #signature
        #wrapper_fn
    }
}
/// Generate fixture registration and inventory code for the wrapper.
///
/// # Examples
/// ```rust,ignore
/// # use syn::parse_quote;
/// # use proc_macro2::Ident;
/// # let ident = Ident::new("step", proc_macro2::Span::call_site());
/// # let config = WrapperConfig {
/// #     ident: &ident,
/// #     fixtures: &[],
/// #     step_args: &[],
/// #     datatable: None,
/// #     docstring: None,
/// #     pattern: &parse_quote!(""),
/// #     keyword: rstest_bdd::StepKeyword::Given,
/// #     call_order: &[],
/// # };
/// # let (wrapper_ident, const_ident, pattern_ident) = generate_wrapper_identifiers(config.ident, 0);
/// let tokens = generate_registration_code(&config, &pattern_ident, &wrapper_ident, &const_ident);
/// assert!(tokens.to_string().contains("rstest_bdd"));
/// ```
fn generate_registration_code(
    config: &WrapperConfig<'_>,
    pattern_ident: &proc_macro2::Ident,
    wrapper_ident: &proc_macro2::Ident,
    const_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    let WrapperConfig {
        fixtures, keyword, ..
    } = config;
    let fixture_names: Vec<_> = fixtures
        .iter()
        .map(|FixtureArg { name, .. }| {
            let s = name.to_string();
            quote! { #s }
        })
        .collect();
    let fixture_len = fixture_names.len();
    let keyword_token = keyword_to_token(*keyword);
    quote! {
        const #const_ident: [&'static str; #fixture_len] = [#(#fixture_names),*];
        const _: [(); #fixture_len] = [(); #const_ident.len()];

        rstest_bdd::step!(@pattern #keyword_token, &#pattern_ident, #wrapper_ident, &#const_ident);
    }
}

/// Generate the wrapper function and inventory registration.
pub(crate) fn generate_wrapper_code(config: &WrapperConfig<'_>) -> TokenStream2 {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let (wrapper_ident, const_ident, pattern_ident) =
        generate_wrapper_identifiers(config.ident, id);
    let body = generate_wrapper_body(config, &wrapper_ident, &pattern_ident);
    let registration =
        generate_registration_code(config, &pattern_ident, &wrapper_ident, &const_ident);

    quote! {
        #body
        #registration
    }
}
