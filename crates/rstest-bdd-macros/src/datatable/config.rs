//! Shared configuration structures used across datatable derive helpers.
//!
//! The types in this module capture parsed attribute state so the expanders
//! can focus on generating the final token streams.

use proc_macro2::Ident;
use syn::{ExprPath, Type};

use super::rename::RenameRule;
use crate::named_fields::{NamedFieldSpec, ScalarConversion};

/// Internal data used by the macros implementation.
pub(crate) struct StructConfig {
    /// Optional rule used to derive datatable column names.
    pub(crate) rename_rule: Option<RenameRule>,
}

/// Describes how a missing datatable value is supplied.
pub(crate) enum DefaultValue {
    /// Use the field type's `Default` implementation.
    Trait,
    /// Call the configured function to produce the value.
    Function(ExprPath),
}

/// Selects a datatable cell by name or zero-based position.
#[derive(Clone)]
pub(crate) enum Accessor {
    /// Look up a cell by its column name.
    Column {
        /// Column name used for lookup.
        name: String,
    },
    /// Look up a cell by its zero-based position.
    Index {
        /// Zero-based column position used for lookup.
        position: usize,
    },
}

/// Internal data used by the macros implementation.
pub(crate) struct FieldConfig {
    /// Cell selector used when parsing the field.
    pub(crate) accessor: Accessor,
    /// Whether a missing cell should produce `None`.
    pub(crate) optional: bool,
    /// Optional fallback used when the cell is missing.
    pub(crate) default: Option<DefaultValue>,
    /// Shared normalization and scalar-conversion policy.
    pub(crate) conversion: ScalarConversion,
}

impl FieldConfig {
    /// Creates field configuration with all optional behaviours disabled.
    pub(crate) fn new(accessor: Accessor) -> Self {
        Self {
            accessor,
            optional: false,
            default: None,
            conversion: ScalarConversion::plain(),
        }
    }
}

/// Parsed metadata used to generate one datatable field binding.
pub(crate) struct FieldSpec {
    /// Field identifier, when the source field is named.
    pub(crate) ident: Option<Ident>,
    /// Declared field type.
    pub(crate) ty: Type,
    /// Inner type used for optional field parsing.
    pub(crate) inner_ty: Type,
    /// Parsed field attributes controlling conversion.
    pub(crate) config: FieldConfig,
    /// Shared metadata for named fields; tuple fields intentionally omit this.
    pub(crate) named: Option<NamedFieldSpec>,
}
