//! File staging utilities for trybuild support.

#[path = "staging/implementation.rs"]
mod implementation;

#[cfg(windows)]
pub(crate) use implementation::stage_unrelatable_feature_root;
pub(crate) use implementation::{
    macros_fixture,
    stage_target_root_snapshots,
    trybuild_target_directory,
    ui_fixture,
};
