//! Scenario return type classification helpers.

use crate::return_classifier::{ReturnKind, classify_return_type};

const FALLIBLE_SCENARIO_RETURN_ERROR: &str =
    "#[scenario] bodies must return () or a unit Result/StepResult";

pub(super) fn classify_scenario_return(
    sig: &syn::Signature,
) -> Result<crate::codegen::scenario::ScenarioReturnKind, syn::Error> {
    match classify_return_type(&sig.output, None)? {
        ReturnKind::Unit => Ok(crate::codegen::scenario::ScenarioReturnKind::Unit),
        ReturnKind::ResultUnit => Ok(crate::codegen::scenario::ScenarioReturnKind::ResultUnit),
        ReturnKind::Value | ReturnKind::ResultValue => Err(scenario_return_error(sig)),
    }
}

fn scenario_return_error(sig: &syn::Signature) -> syn::Error {
    syn::Error::new_spanned(&sig.output, FALLIBLE_SCENARIO_RETURN_ERROR)
}
