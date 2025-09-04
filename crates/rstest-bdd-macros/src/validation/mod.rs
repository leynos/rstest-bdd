//! Validation helpers for parsing macros.

pub(crate) mod examples;
pub(crate) mod parameters;
#[cfg(feature = "compile-time-validation")]
#[cfg_attr(docsrs, doc(cfg(feature = "compile-time-validation")))]
pub(crate) mod steps;
