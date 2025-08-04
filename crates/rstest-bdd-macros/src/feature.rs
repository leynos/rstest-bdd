//! Feature file parsing and processing.

use gherkin::{Feature, GherkinEnv};
use proc_macro::TokenStream;
use std::path::{Path, PathBuf};

pub(crate) fn parse_and_load_feature(path: &Path) -> Result<Feature, TokenStream> {
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        let err = syn::Error::new(
            proc_macro2::Span::call_site(),
            "CARGO_MANIFEST_DIR is not set. This variable is normally provided by Cargo. Ensure the macro runs within a Cargo build context.",
        );
        return Err(err.to_compile_error().into());
    };
    let feature_path = PathBuf::from(manifest_dir).join(path);
    Feature::parse_path(&feature_path, GherkinEnv::default()).map_err(|err| {
        let msg = format!("failed to parse feature file: {err}");
        syn::Error::new(proc_macro2::Span::call_site(), msg)
            .to_compile_error()
            .into()
    })
}

/// Rows parsed from a `Scenario Outline` examples table.
#[derive(Clone)]
pub(crate) struct ExampleTable {
    pub(crate) headers: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
}
