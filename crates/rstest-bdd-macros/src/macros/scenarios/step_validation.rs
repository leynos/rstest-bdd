//! Compile-time step validation for generated scenario tests.
//!
//! The three entry points are mutually exclusive: `strict-compile-time-validation`
//! implies `compile-time-validation`, so the strict gate must be listed first.

use crate::parsing::feature::ParsedStep;

/// Validate generated scenarios in strict mode: missing steps are hard errors.
#[cfg(feature = "strict-compile-time-validation")]
pub(super) fn validate_steps_compile_time(
    steps: &[ParsedStep],
    libraries: Option<&[Box<str>]>,
) -> Result<(), syn::Error> {
    libraries.map_or_else(
        || crate::validation::steps::validate_steps_exist(steps, true),
        |libraries| crate::validation::steps::validate_steps_exist_in_scope(steps, libraries, true),
    )
}

/// Validate generated scenarios with missing steps reported as warnings.
#[cfg(all(
    feature = "compile-time-validation",
    not(feature = "strict-compile-time-validation")
))]
pub(super) fn validate_steps_compile_time(
    steps: &[ParsedStep],
    libraries: Option<&[Box<str>]>,
) -> Result<(), syn::Error> {
    libraries.map_or_else(
        || crate::validation::steps::validate_steps_exist(steps, false),
        |libraries| {
            crate::validation::steps::validate_steps_exist_in_scope(steps, libraries, false)
        },
    )
}

/// No-validation fallback: step resolution happens entirely at runtime.
#[cfg(not(feature = "compile-time-validation"))]
pub(super) fn validate_steps_compile_time(
    steps: &[ParsedStep],
    libraries: Option<&[Box<str>]>,
) -> Result<(), syn::Error> {
    let _ = (steps, libraries);
    Ok(())
}
