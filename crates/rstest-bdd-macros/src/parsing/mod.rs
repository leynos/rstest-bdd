//! Shared parsers used by the procedural macros.
//!
//! The `feature` module loads `.feature` files and lifts their steps into the
//! strongly typed [`ScenarioData`](feature::ScenarioData) structures consumed by
//! code generation. `examples` normalizes scenario outlines, while `tags`
//! handles compile-time tag-expression filtering so the macros can decide which
//! scenarios to expand. The `placeholder` module provides substitution of
//! `<placeholder>` tokens in scenario outline step text.

pub(crate) mod examples;
pub(crate) mod feature;
pub(crate) mod placeholder;
pub(crate) mod tags;
