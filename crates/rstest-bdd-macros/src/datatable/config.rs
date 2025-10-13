use proc_macro2::Ident;
use syn::{ExprPath, Type};

use super::rename::RenameRule;

pub(crate) struct StructConfig {
    pub(crate) rename_rule: Option<RenameRule>,
}

pub(crate) enum DefaultValue {
    Trait,
    Function(ExprPath),
}

#[derive(Clone)]
pub(crate) enum Accessor {
    Column { name: String },
    Index { position: usize },
}

pub(crate) struct FieldConfig {
    pub(crate) accessor: Accessor,
    pub(crate) optional: bool,
    pub(crate) default: Option<DefaultValue>,
    pub(crate) parse_with: Option<ExprPath>,
    pub(crate) truthy: bool,
    pub(crate) trim: bool,
}

impl FieldConfig {
    pub(crate) fn new(accessor: Accessor) -> Self {
        Self {
            accessor,
            optional: false,
            default: None,
            parse_with: None,
            truthy: false,
            trim: false,
        }
    }
}

pub(crate) struct FieldSpec {
    pub(crate) ident: Option<Ident>,
    pub(crate) ty: Type,
    pub(crate) inner_ty: Type,
    pub(crate) config: FieldConfig,
}
