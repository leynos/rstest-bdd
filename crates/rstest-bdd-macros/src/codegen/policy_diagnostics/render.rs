//! Render findings into the channel-neutral values the emitters carry.
//!
//! Warnings and errors are built here, away from the boundary type, so the
//! decision (findings), the wording (messages) and the transport (channels)
//! stay separately readable. Errors suppress warnings at the boundary
//! (decision D9); this module only builds what it is asked for.

use proc_macro2::TokenStream as TokenStream2;

#[cfg(not(rstest_bdd_nightly))]
use super::emit::Marker;
use super::{
    DiagnosticState,
    PolicyInputs,
    SlotClass,
    classify,
    emit::RenderedWarning,
    message,
    model::Framework,
    resolution::emitted_attributes,
};
use crate::codegen::CrateResolver;

/// Turn the findings into channel-neutral warnings.
///
/// A finding can hold only for an evidenced slot, whose path is by definition
/// present; a missing slot therefore means the model and the stored inputs
/// disagree, and dropping the warning keeps the renderer total without a
/// panic.
pub(super) fn warnings(state: &DiagnosticState) -> Vec<RenderedWarning> {
    [
        harness_warning(state),
        attributes_warning(state),
        conflict_warning(state),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Build the W003 warning, spanned at the `attributes` path.
fn conflict_warning(state: &DiagnosticState) -> Option<RenderedWarning> {
    let conflict = state.findings.conflicting_frameworks?;
    let ident = state.slots.attributes.as_ref()?;
    let matching_policy = canonical_policy_path(state.resolver, conflict.harness);
    let harness_needs_fixing = matches!(state.inputs.harness(), SlotClass::Evidenced(_));
    Some(RenderedWarning {
        message: message::conflicting_selectors(
            conflict.harness.display_name(),
            conflict.attributes.display_name(),
            &matching_policy,
            harness_needs_fixing,
        ),
        span: ident.span(),
        #[cfg(not(rstest_bdd_nightly))]
        marker: Marker::ConflictingHarnessAndPolicy,
    })
}

/// Build the errors this boundary must report, if any.
pub(super) fn errors(state: &DiagnosticState) -> Option<TokenStream2> {
    if !state.findings.runtime_alias_with_harness {
        return None;
    }
    let ident = state.slots.harness.as_ref()?;
    let canonical = match state.inputs.harness() {
        SlotClass::Evidenced(framework) => Some(canonical_adapter_path(
            state.resolver,
            framework,
            &ident.to_string(),
        )),
        SlotClass::Absent | SlotClass::Recognized(_) | SlotClass::Foreign => None,
    };
    let error = syn::Error::new(
        ident.span(),
        message::runtime_alias_with_harness(canonical.as_deref()),
    );
    Some(error.into_compile_error())
}

/// Build the harness slot's warning, W002 taking the place of W001.
fn harness_warning(state: &DiagnosticState) -> Option<RenderedWarning> {
    let ident = state.slots.harness.as_ref()?;
    let SlotClass::Evidenced(framework) = state.inputs.harness() else {
        return None;
    };
    let written = ident.to_string();
    let canonical_harness = canonical_adapter_path(state.resolver, framework, &written);
    let canonicalized = state
        .inputs
        .with_harness(SlotClass::Recognized(framework.into()));
    let dropped = lost_attributes(state.inputs, canonicalized);
    let message = if state.findings.missing_attribute_policy {
        let canonical_policy = canonical_policy_path(state.resolver, framework);
        message::missing_attribute_policy(&canonical_harness, &canonical_policy, &dropped)
    } else {
        message::unrecognized_adapter_path(&written, &canonical_harness, &dropped)
    };
    #[cfg(not(rstest_bdd_nightly))]
    let marker = if state.findings.missing_attribute_policy {
        Marker::MissingAttributePolicy
    } else {
        Marker::UnrecognizedAdapterPath
    };
    Some(RenderedWarning {
        message,
        span: ident.span(),
        #[cfg(not(rstest_bdd_nightly))]
        marker,
    })
}

/// Build the attributes slot's warning, which is always W001.
fn attributes_warning(state: &DiagnosticState) -> Option<RenderedWarning> {
    let ident = state.slots.attributes.as_ref()?;
    let SlotClass::Evidenced(framework) = state.inputs.attributes() else {
        return None;
    };
    let written = ident.to_string();
    let canonical = canonical_adapter_path(state.resolver, framework, &written);
    let canonicalized = state
        .inputs
        .with_attributes(SlotClass::Recognized(framework.into()));
    let dropped = lost_attributes(state.inputs, canonicalized);
    Some(RenderedWarning {
        message: message::unrecognized_adapter_path(&written, &canonical, &dropped),
        span: ident.span(),
        #[cfg(not(rstest_bdd_nightly))]
        marker: Marker::UnrecognizedAdapterPath,
    })
}

/// The attributes the canonical spelling adds and the actual expansion omits.
///
/// A user-written executor attribute suppresses every "was not applied" claim,
/// because the generator deduplicates against it (decision D31).
fn lost_attributes(actual: PolicyInputs, canonical: PolicyInputs) -> Vec<&'static str> {
    if actual.has_user_test_attr() {
        return Vec::new();
    }
    emitted_attributes(canonical).difference(emitted_attributes(actual))
}

/// A resolver-rooted canonical adapter path for the type the user wrote.
fn canonical_adapter_path(
    resolver: &dyn CrateResolver,
    framework: Framework,
    written: &str,
) -> String {
    format!("{}::{written}", canonical_root(resolver, framework))
}

/// A resolver-rooted canonical attribute-policy path for one framework.
fn canonical_policy_path(resolver: &dyn CrateResolver, framework: Framework) -> String {
    format!(
        "{}::{}",
        canonical_root(resolver, framework),
        framework.attribute_policy_type()
    )
}

/// The crate root a suggested path must use.
///
/// The resolver's answer keeps a renamed dependency compilable: a crate that
/// depends on `tok = { package = "rstest-bdd-harness-tokio" }` must be told to
/// write `tok::TokioHarness`, not the name it does not have.
fn canonical_root(resolver: &dyn CrateResolver, framework: Framework) -> String {
    let spec = classify::adapter_spec(framework);
    resolver.resolved_root(spec.package_name).map_or_else(
        || spec.default_crate_name.to_owned(),
        |ident| ident.to_string(),
    )
}
