//! Scenario return type classification helpers.

use crate::return_classifier::{ReturnKind, classify_return_type};

const FALLIBLE_SCENARIO_RETURN_ERROR: &str =
    "#[scenario] bodies must return () or a unit Result/StepResult";

pub(super) fn classify_scenario_return(
    sig: &syn::Signature,
) -> Result<crate::codegen::scenario::ScenarioReturnKind, syn::Error> {
    let return_kind = classify_return_type(&sig.output, None)?;
    if is_supported_scenario_return(return_kind) {
        Ok(map_scenario_return_kind(return_kind))
    } else {
        Err(scenario_return_error(sig))
    }
}

fn is_supported_scenario_return(return_kind: ReturnKind) -> bool {
    matches!(return_kind, ReturnKind::Unit | ReturnKind::ResultUnit)
}

fn map_scenario_return_kind(
    return_kind: ReturnKind,
) -> crate::codegen::scenario::ScenarioReturnKind {
    match return_kind {
        ReturnKind::Unit => crate::codegen::scenario::ScenarioReturnKind::Unit,
        ReturnKind::ResultUnit => crate::codegen::scenario::ScenarioReturnKind::ResultUnit,
        ReturnKind::Value | ReturnKind::ResultValue => {
            unreachable!("unsupported scenario return kind: {return_kind:?}");
        }
    }
}

fn scenario_return_error(sig: &syn::Signature) -> syn::Error {
    syn::Error::new_spanned(&sig.output, FALLIBLE_SCENARIO_RETURN_ERROR)
}
