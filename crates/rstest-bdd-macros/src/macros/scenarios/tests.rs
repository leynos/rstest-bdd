//! Unit tests for the `scenarios!` macro entry point.

use std::{collections::HashSet, fs, path::Path};

use proc_macro2::Span;
use quote::quote;
use rstest_bdd_policy::RuntimeMode;
#[cfg(feature = "compile-time-validation")]
use serial_test::serial;
use syn::LitStr;
use tempfile::TempDir;

use super::{
    ScenarioTestContext,
    expand_scenarios_tokens,
    macro_args::ScenariosArgs,
    process_scenario,
};
#[cfg(feature = "compile-time-validation")]
use crate::{
    StepKeyword,
    validation::steps::{
        clear_registered_steps_for_tests,
        register_step,
        register_step_in_library,
    },
};
use crate::{
    codegen::SharedAdapterResolutions,
    parsing::{feature::parse_and_load_feature, tags::TagExpression},
};

fn write_feature(contents: &str) -> std::io::Result<(TempDir, std::path::PathBuf)> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("example.feature");
    fs::write(&path, contents)?;
    Ok((temp, path))
}

fn scenarios_args(dir: &Path) -> ScenariosArgs {
    ScenariosArgs {
        dir: LitStr::new(&dir.display().to_string(), Span::call_site()),
        tag_filter: None,
        fixtures: Vec::new(),
        runtime: RuntimeMode::Sync,
        harness: None,
        attributes: None,
        libraries: None,
    }
}

/// Shared inputs for constructing a scenario test context.
#[derive(Clone, Copy)]
struct TestContextArgs<'a> {
    /// The feature file being processed.
    feature_path: &'a Path,
    /// Optional filter limiting processed scenarios.
    tag_filter: Option<&'a TagExpression>,
    /// Optional closed scope for compile-time validation.
    library_validation_names: Option<&'a [Box<str>]>,
    /// Resolved runtime adapter information.
    resolutions: &'a SharedAdapterResolutions,
}

fn test_context(args: TestContextArgs<'_>) -> ScenarioTestContext<'_> {
    let TestContextArgs {
        feature_path,
        tag_filter,
        library_validation_names,
        resolutions,
    } = args;
    let scope = Box::leak(Box::new(quote!(::rstest_bdd::StepScope::global())));
    ScenarioTestContext {
        feature_stem: "example",
        rel_path: feature_path,
        tag_filter,
        fixtures: &[],
        runtime: RuntimeMode::Sync,
        harness: None,
        attributes: None,
        effective_harness: None,
        scope,
        library_validation_names,
        resolutions,
    }
}

#[test]
#[cfg_attr(feature = "compile-time-validation", serial)]
fn expand_scenarios_generates_module_for_valid_features() -> Result<(), String> {
    #[cfg(feature = "compile-time-validation")]
    clear_registered_steps_for_tests();
    let (temp, feature_path) =
        write_feature("Feature: Example\nScenario: works\nGiven the system works\n")
            .map_err(|error| error.to_string())?;
    #[cfg(feature = "compile-time-validation")]
    register_step(
        StepKeyword::Given,
        &LitStr::new("the system works", Span::call_site()),
    );
    let tokens = expand_scenarios_tokens(scenarios_args(temp.path())).to_string();
    let expected_feature_path = LitStr::new(&feature_path.display().to_string(), Span::call_site());
    let expected_feature_path_tokens = quote!(#expected_feature_path).to_string();

    assert!(tokens.contains("mod "), "{tokens}");
    assert!(tokens.contains("_scenarios"), "{tokens}");
    assert!(tokens.contains("StepScope :: global ()"));
    assert!(tokens.contains(&expected_feature_path_tokens), "{tokens}");
    assert!(!tokens.contains("compile_error"));
    #[cfg(feature = "compile-time-validation")]
    clear_registered_steps_for_tests();
    Ok(())
}

#[test]
fn expand_scenarios_preserves_selected_library_scope() -> Result<(), String> {
    let (temp, _) = write_feature("Feature: Example\nScenario: works\nGiven the system works\n")
        .map_err(|error| error.to_string())?;
    let mut args = scenarios_args(temp.path());
    args.libraries = Some(vec![
        syn::parse_quote!(accounts),
        syn::parse_quote!(filesystem),
    ]);
    let tokens = expand_scenarios_tokens(args).to_string();

    assert!(tokens.contains("StepScope :: new"));
    assert!(tokens.contains("__RSTEST_BDD_STEP_LIBRARY_accounts"));
    assert!(tokens.contains("__RSTEST_BDD_STEP_LIBRARY_filesystem"));
    Ok(())
}

#[test]
fn expand_scenarios_preserves_invalid_tag_diagnostic() -> Result<(), String> {
    let (_, feature_path) =
        write_feature("Feature: Example\n").map_err(|error| error.to_string())?;
    let mut args = scenarios_args(feature_path.as_path());
    args.tag_filter = Some(LitStr::new("@", Span::call_site()));
    let tokens = expand_scenarios_tokens(args).to_string();

    assert!(tokens.contains("compile_error"));
    assert!(tokens.contains("expected tag name after '@'"), "{tokens}");
    Ok(())
}

#[test]
fn expand_scenarios_normalizes_missing_directory_errors() -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|error| error.to_string())?;
    let missing = temp.path().join("missing");
    let tokens = expand_scenarios_tokens(scenarios_args(missing.as_path())).to_string();

    assert!(tokens.contains("compile_error"));
    assert!(tokens.contains("failed to read directory"));
    assert!(tokens.contains("directory not found"));
    Ok(())
}

#[test]
fn process_scenario_skips_filtered_out_scenarios() -> Result<(), String> {
    let (_temp, feature_path) =
        write_feature("Feature: Example\n@ignored\nScenario: ignored\nGiven the system works\n")
            .map_err(|error| error.to_string())?;
    let feature =
        parse_and_load_feature(feature_path.as_path()).map_err(|error| error.to_string())?;
    let filter = TagExpression::parse("@selected").map_err(|error| error.to_string())?;
    let resolutions = SharedAdapterResolutions::resolve(None, None);
    let context = test_context(TestContextArgs {
        feature_path: feature_path.as_path(),
        tag_filter: Some(&filter),
        library_validation_names: None,
        resolutions: &resolutions,
    });
    let mut names = HashSet::new();

    let result =
        process_scenario(&feature, 0, &context, &mut names).map_err(|error| error.to_string())?;
    assert!(result.is_none());
    Ok(())
}

#[test]
#[cfg(feature = "compile-time-validation")]
#[serial]
fn process_scenario_returns_validation_errors_without_generating_a_test() -> Result<(), String> {
    clear_registered_steps_for_tests();
    let (_temp, feature_path) =
        write_feature("Feature: Example\nScenario: ambiguous\nGiven the system works\n")
            .map_err(|error| error.to_string())?;
    let feature =
        parse_and_load_feature(feature_path.as_path()).map_err(|error| error.to_string())?;
    let pattern = LitStr::new("the system works", Span::call_site());
    register_step_in_library(StepKeyword::Given, &pattern, "accounts");
    register_step_in_library(StepKeyword::Given, &pattern, "accounts");
    let libraries = vec![Box::<str>::from("accounts")];
    let resolutions = SharedAdapterResolutions::resolve(None, None);
    let context = test_context(TestContextArgs {
        feature_path: feature_path.as_path(),
        tag_filter: None,
        library_validation_names: Some(&libraries),
        resolutions: &resolutions,
    });
    let mut names = HashSet::new();

    let result = process_scenario(&feature, 0, &context, &mut names);
    let error = result
        .err()
        .ok_or_else(|| "ambiguous selected definitions should fail validation".to_owned())?
        .to_string();
    assert!(error.contains("compile_error"));
    assert!(error.contains("Ambiguous step definition"));
    assert!(names.is_empty());
    clear_registered_steps_for_tests();
    Ok(())
}

#[test]
#[cfg_attr(feature = "compile-time-validation", serial)]
fn process_scenario_generates_a_test_when_validation_succeeds() -> Result<(), String> {
    #[cfg(feature = "compile-time-validation")]
    clear_registered_steps_for_tests();
    let (_temp, feature_path) =
        write_feature("Feature: Example\nScenario: works\nGiven the system works\n")
            .map_err(|error| error.to_string())?;
    let feature =
        parse_and_load_feature(feature_path.as_path()).map_err(|error| error.to_string())?;
    #[cfg(feature = "compile-time-validation")]
    register_step(
        StepKeyword::Given,
        &LitStr::new("the system works", Span::call_site()),
    );
    let resolutions = SharedAdapterResolutions::resolve(None, None);
    let context = test_context(TestContextArgs {
        feature_path: feature_path.as_path(),
        tag_filter: None,
        library_validation_names: None,
        resolutions: &resolutions,
    });
    let mut names = HashSet::new();

    let generated = process_scenario(&feature, 0, &context, &mut names)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "valid scenario should generate a test".to_owned())?
        .to_string();
    assert!(generated.contains("fn example_works"));
    #[cfg(feature = "compile-time-validation")]
    clear_registered_steps_for_tests();
    Ok(())
}

#[cfg(unix)]
mod unix {
    //! Unix-only symlink discovery coverage.

    use std::{fs, os::unix::fs::symlink, path::Path};

    use tempfile::tempdir;

    use super::super::feature_discovery::collect_feature_files;

    #[test]
    fn collects_symlinked_feature_files_without_following_directory_loops() {
        let temp = tempdir().expect("test setup should succeed");
        let features_root = temp.path().join("features");
        fs::create_dir_all(features_root.join("nested")).expect("test setup should succeed");

        let feature_path = features_root.join("nested/example.feature");
        fs::write(&feature_path, "Feature: Example\n").expect("test setup should succeed");

        let symlink_path = features_root.join("symlink.feature");
        symlink(&feature_path, &symlink_path).expect("test setup should succeed");

        let relative_symlink_path = features_root.join("relative_link.feature");
        symlink(Path::new("nested/example.feature"), &relative_symlink_path)
            .expect("test setup should succeed");

        let loop_dir = features_root.join("loop");
        symlink(&features_root, &loop_dir).expect("test setup should succeed");

        let files =
            collect_feature_files(features_root.as_path()).expect("test setup should succeed");

        let mut expected = vec![feature_path, symlink_path, relative_symlink_path];
        expected.sort();
        assert_eq!(files, expected);
    }
}

#[cfg(not(unix))]
mod non_unix {
    //! Portable configuration coverage when Unix symlinks are unavailable.

    #[test]
    fn collects_symlinked_feature_files_without_following_directory_loops() {
        assert!(cfg!(not(unix)));
    }
}
