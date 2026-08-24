//! Classifier helpers assign each function parameter to an [`Arg`] variant.
//!
//! The `extract_args` pipeline runs these classifiers in order until one claims
//! ownership of a parameter, ensuring future extensions only need to append a
//! new function to the list rather than editing the control flow. Attribute
//! validation lives here so the pipeline can provide precise diagnostics while
//! keeping the orchestration layer slim.

use std::collections::HashSet;

use super::{Arg, ExtractedArgs, normalize_param_name};

mod step_struct;
mod type_shape;

pub(super) use fixture_or_step::classify_fixture_or_step;
pub(super) use step_struct::{classify_step_struct, extract_step_struct_attribute};
use type_shape::{is_docstring_canonical, is_type_seq, should_classify_as_datatable};

/// Test whether `ty` is `CachedTable`, the lazily converted table representation.
///
/// Shared with the wider crate because table binding code outside this module
/// must distinguish a `CachedTable` parameter from a raw `Vec<Vec<String>>` one.
/// It lives here rather than alongside the other shape predicates in
/// [`type_shape`] so that `classify::is_cached_table` remains its path.
pub(crate) fn is_cached_table(ty: &syn::Type) -> bool { is_type_seq(ty, &["CachedTable"]) }

/// Diagnostic for a `datatable` parameter declared with an unsupported type.
const DATATABLE_TYPE_ERROR: &str = concat!(
    "parameter named `datatable` must have type `Vec<Vec<String>>` or `CachedTable` ",
    "(or use `#[datatable]` with a type that implements `TryFrom<Vec<Vec<String>>>`)",
);
/// Diagnostic for a duplicate or wrongly typed `docstring` parameter.
const DOCSTRING_TYPE_ERROR: &str =
    "only one docstring parameter is permitted and it must have type `String`";
/// Diagnostic for `#[datatable]` applied to the reserved `docstring` parameter.
const DATATABLE_ON_DOCSTRING_ERROR: &str =
    "parameter `docstring` cannot be annotated with #[datatable]";
/// Diagnostic for a second `DataTable` parameter in one signature.
///
/// Shared with `crate::macros` so the help text stays aligned with the
/// emitted error.
pub(crate) const DUPLICATE_DATATABLE_ERROR: &str = "only one DataTable parameter is permitted";
/// Diagnostic for a `DataTable` parameter declared after a `DocString`.
const DATATABLE_AFTER_DOCSTRING_ERROR: &str =
    "DataTable must be declared before DocString to match Gherkin ordering";
/// Diagnostic for combining `#[datatable]` with `#[from]`.
const DATATABLE_WITH_FROM_ERROR: &str = "#[datatable] cannot be combined with #[from]";

/// Mutable state threaded through the classification pipeline.
///
/// Bundles the accumulator of classified arguments (`extracted`) with the set
/// of step-pattern placeholders not yet claimed by a parameter
/// (`placeholders`). Classifiers that match a placeholder remove it from the
/// set, so each placeholder binds at most one parameter; whatever remains
/// after classification represents unbound placeholders.
pub(super) struct ClassificationContext<'a> {
    /// Accumulated classification results, appended to in place.
    pub(super) extracted: &'a mut ExtractedArgs,
    /// Step-pattern placeholders still awaiting a matching parameter;
    /// classifiers remove entries as they claim them.
    pub(super) placeholders: &'a mut HashSet<String>,
}

impl<'a> ClassificationContext<'a> {
    /// Borrow an accumulator and a placeholder set for one classification run.
    ///
    /// Holds both mutably for the context's lifetime, so a caller cannot read
    /// either while classifiers are still claiming parameters. Neither argument
    /// is copied: mutations made through the context are visible to the caller
    /// once it is dropped.
    pub(super) fn new(
        extracted: &'a mut ExtractedArgs,
        placeholders: &'a mut HashSet<String>,
    ) -> Self {
        Self {
            extracted,
            placeholders,
        }
    }
}

/// Detect and strip a marker attribute (e.g. `#[datatable]`) from `arg`.
///
/// Mutates `arg.attrs` in place, removing every attribute whose path is
/// `attr_name` so the generated wrapper does not re-emit it. Returns whether
/// the attribute was present.
///
/// # Errors
///
/// Returns an error when the attribute carries arguments (the marker form
/// takes none) or appears more than once on the same parameter.
///
/// # Examples
///
/// Given the parameter as written in a step function, and `attr_name`
/// `"datatable"`:
///
/// ```text
/// #[datatable] table: CachedTable   => Ok(true);  `arg.attrs` left empty
/// table: CachedTable                => Ok(false); `arg.attrs` unchanged
/// #[datatable(1)] table: MyTable    => Err: "`#[datatable]` does not take arguments"
/// #[datatable] #[datatable] t: T    => Err: "duplicate `#[datatable]` attribute"
/// ```
pub(super) fn extract_flag_attribute(arg: &mut syn::PatType, attr_name: &str) -> syn::Result<bool> {
    let mut found = false;
    let mut duplicate = false;
    let mut err_attr: Option<syn::Attribute> = None;
    arg.attrs.retain(|a| {
        if a.path().is_ident(attr_name) {
            if found {
                duplicate = true;
            }
            found = true;
            if a.meta.require_path_only().is_err() {
                err_attr = Some(a.clone());
            }
            false
        } else {
            true
        }
    });
    if let Some(attr) = err_attr {
        return Err(syn::Error::new_spanned(
            attr,
            format!("`#[{attr_name}]` does not take arguments"),
        ));
    }
    if duplicate {
        return Err(syn::Error::new_spanned(
            &arg.pat,
            format!("duplicate `#[{attr_name}]` attribute"),
        ));
    }
    Ok(found)
}

/// Outcome of a successful [`match_named_flag`] call.
struct FlagMatch {
    /// Whether the match came from the marker attribute rather than the
    /// canonical name/type shape. Callers use this to apply rules that only
    /// bind to the explicit form, such as rejecting `#[datatable]` on the
    /// reserved `docstring` parameter.
    via_attr: bool,
}

/// The two ways a reserved parameter may be recognized, plus its diagnostic.
///
/// Parameterizes [`match_named_flag`] so `DataTable` and `DocString` share one
/// matching routine: each supplies its own marker attribute (or none), its
/// canonical identifier, the type predicate for the canonical form, and the
/// error text used when the name matches but the type does not.
struct FlagMatchConfig<F>
where
    F: Fn(&syn::Ident, &syn::Type) -> bool,
{
    /// Marker attribute that opts a parameter in explicitly, or `None` for a
    /// parameter kind recognized only by its canonical name and type. When
    /// present it is stripped from the parameter during matching.
    attr_name: Option<&'static str>,
    /// Reserved identifier for this parameter kind, used to detect a
    /// right-name/wrong-type parameter and report `wrong_type_msg`.
    canonical_name: &'static str,
    /// Predicate recognizing the canonical name-and-type shape.
    canonical_check: F,
    /// Diagnostic emitted when the parameter uses `canonical_name` but
    /// `canonical_check` rejects its type.
    wrong_type_msg: &'static str,
}

impl<F> FlagMatchConfig<F>
where
    F: Fn(&syn::Ident, &syn::Type) -> bool,
{
    /// Bundle the matching rules for one reserved parameter kind.
    fn new(
        attr_name: Option<&'static str>,
        canonical_name: &'static str,
        canonical_check: F,
        wrong_type_msg: &'static str,
    ) -> Self {
        Self {
            attr_name,
            canonical_name,
            canonical_check,
            wrong_type_msg,
        }
    }
}

/// Recognize a reserved parameter by marker attribute or canonical shape.
///
/// Checks the marker attribute first, stripping it from `arg` in place when
/// `config.attr_name` is set — a mutation that persists even when this function
/// subsequently returns an error. Then tests the canonical name-and-type shape.
/// A parameter matching either way yields `Some`, recording in
/// [`FlagMatch::via_attr`] which route matched.
///
/// Returns `Ok(None)` when the parameter is unrelated to this kind, leaving it
/// for the next classifier.
///
/// # Errors
///
/// Returns an error when the parameter uses the reserved `canonical_name` but
/// its type fails `canonical_check` — a right-name/wrong-type parameter is
/// reported as `config.wrong_type_msg` rather than silently falling through to
/// be treated as a fixture. Also propagates marker-attribute validation
/// failures from [`extract_flag_attribute`], such as a marker carrying
/// arguments or appearing twice.
fn match_named_flag<F>(
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
    config: FlagMatchConfig<F>,
) -> syn::Result<Option<FlagMatch>>
where
    F: Fn(&syn::Ident, &syn::Type) -> bool,
{
    let FlagMatchConfig {
        attr_name,
        canonical_name,
        canonical_check,
        wrong_type_msg,
    } = config;

    let via_attr = if let Some(name) = attr_name {
        extract_flag_attribute(arg, name)?
    } else {
        false
    };
    let canonical = canonical_check(pat, ty);
    if via_attr || canonical {
        Ok(Some(FlagMatch { via_attr }))
    } else if pat == canonical_name {
        Err(syn::Error::new_spanned(arg, wrong_type_msg))
    } else {
        Ok(None)
    }
}

/// Classify `arg` as the step’s `DataTable` parameter, if it matches.
///
/// A parameter matches when it is annotated `#[datatable]` or when it is the
/// canonical `datatable: Vec<Vec<String>>` / `datatable: CachedTable` shape.
/// On a match the argument is recorded in `st` (setting `st.datatable_idx`)
/// and the `#[datatable]` marker attribute is stripped from `arg` in place
/// via [`extract_flag_attribute`].
///
/// - `st` — classification accumulator, mutated on success.
/// - `arg` — the parameter being classified; its attribute list is mutated in place even when
///   validation subsequently fails.
/// - `pat` / `ty` — the parameter's identifier and type, pre-extracted by the caller.
///
/// Returns `Ok(true)` when the parameter was claimed, `Ok(false)` to let the
/// next classifier try.
///
/// # Errors
///
/// Returns an error when the parameter is named `datatable` with an
/// unsupported type, when `#[datatable]` is applied to the reserved
/// `docstring` parameter, when a `DataTable` was already classified, when the
/// `DataTable` appears after a `DocString` (Gherkin ordering), or when a
/// matched `DataTable` parameter also carries `#[from]`. The `#[from]`
/// rejection applies to any matched `DataTable`, whether it matched via the
/// `#[datatable]` marker or the canonical `datatable` shape, because `#[from]`
/// has no meaning for a table binding.
///
/// # Examples
///
/// Given the parameter as written in a step function:
///
/// ```text
/// datatable: Vec<Vec<String>>       => Ok(true);  recorded as DataTable
/// #[datatable] rows: CachedTable    => Ok(true);  marker stripped, recorded
/// name: String                      => Ok(false); left for the next classifier
/// datatable: u32                    => Err: unsupported `datatable` type
/// #[from(x)] datatable: CachedTable => Err: `#[from]` rejected on a DataTable
/// ```
pub(super) fn classify_datatable(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
) -> syn::Result<bool> {
    let has_from = arg.attrs.iter().any(|a| a.path().is_ident("from"));
    let match_result = match_named_flag(
        arg,
        pat,
        ty,
        FlagMatchConfig::new(
            Some("datatable"),
            "datatable",
            should_classify_as_datatable,
            DATATABLE_TYPE_ERROR,
        ),
    )?;
    let Some(flag_match) = match_result else {
        return Ok(false);
    };
    if flag_match.via_attr && pat == "docstring" {
        return Err(syn::Error::new_spanned(arg, DATATABLE_ON_DOCSTRING_ERROR));
    }
    if st.datatable_idx.is_some() {
        return Err(syn::Error::new_spanned(arg, DUPLICATE_DATATABLE_ERROR));
    }
    if st.docstring_idx.is_some() {
        return Err(syn::Error::new_spanned(
            arg,
            DATATABLE_AFTER_DOCSTRING_ERROR,
        ));
    }
    if has_from {
        return Err(syn::Error::new_spanned(arg, DATATABLE_WITH_FROM_ERROR));
    }
    let idx = st.push(Arg::DataTable {
        pat: pat.clone(),
        ty: ty.clone(),
    });
    st.datatable_idx = Some(idx);
    Ok(true)
}

/// Classify `arg` as the step’s `DocString` parameter, if it matches.
///
/// A parameter matches when it is the canonical `docstring: String` shape.
/// On a match the argument is recorded in `st` (setting `st.docstring_idx`).
/// Unlike [`classify_datatable`] there is no marker attribute, so `arg` is
/// not mutated.
///
/// Returns `Ok(true)` when the parameter was claimed, `Ok(false)` to let the
/// next classifier try.
///
/// # Errors
///
/// Returns an error when a parameter named `docstring` has a type other than
/// `String`, or when a `DocString` parameter was already classified.
///
/// # Examples
///
/// Given the parameter as written in a step function:
///
/// ```text
/// docstring: String   => Ok(true);  recorded as DocString
/// name: String        => Ok(false); left for the next classifier
/// docstring: u32      => Err: docstring parameter must have type `String`
/// ```
pub(super) fn classify_docstring(
    st: &mut ExtractedArgs,
    arg: &mut syn::PatType,
    pat: &syn::Ident,
    ty: &syn::Type,
) -> syn::Result<bool> {
    let match_result = match_named_flag(
        arg,
        pat,
        ty,
        FlagMatchConfig::new(
            None,
            "docstring",
            is_docstring_canonical,
            DOCSTRING_TYPE_ERROR,
        ),
    )?;
    if match_result.is_none() {
        return Ok(false);
    }
    if st.docstring_idx.is_some() {
        return Err(syn::Error::new_spanned(arg, DOCSTRING_TYPE_ERROR));
    }
    let idx = st.push(Arg::DocString { pat: pat.clone() });
    st.docstring_idx = Some(idx);
    Ok(true)
}
#[cfg(test)]
mod prop_tests;
#[cfg(test)]
mod tests;

mod fixture_or_step;
