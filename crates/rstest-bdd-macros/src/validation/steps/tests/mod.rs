//! Tests for step-definition validation: missing/single/ambiguous outcomes and registry behaviour.
// Intentionally left without file-wide lint suppressions; add per-function #[expect(...)] where
// needed.
use camino::Utf8PathBuf;
use rstest::rstest;
use serial_test::serial;
use tempfile::{tempdir, tempdir_in};

use super::{
    crate_id::{canonicalize_out_dir, normalize_crate_id},
    *,
};

mod support;
use self::support::{
    TempWorkingDir,
    assert_bullet_count,
    clear_registry,
    create_dir_all_cap,
    create_test_step,
    temp_working_dir,
};

#[rstest]
#[case::basic("a step", "a step")]
#[case::placeholder("I have {item}", "I have apples")]
#[case::typed("number {n:u32}", "number 42")]
#[serial]
fn validates_step_patterns(#[case] pattern: &str, #[case] test_text: &str) {
    clear_registry();
    register_step(
        StepKeyword::Given,
        &syn::LitStr::new(pattern, proc_macro2::Span::call_site()),
    );
    let steps = [create_test_step(StepKeyword::Given, test_text)];
    assert!(validate_steps_exist(&steps, true).is_ok());
    assert!(validate_steps_exist(&steps, false).is_ok());
}

#[rstest]
#[case::missing_step(None, "missing")]
#[case::foreign_crate_step(Some(("a step", "other")), "a step")]
#[serial]
fn validates_strict_mode_errors(
    #[case] foreign_step: Option<(&str, &str)>,
    #[case] step_text: &str,
) {
    clear_registry();
    if let Some((pattern, crate_id)) = foreign_step {
        register_step_for_crate(StepKeyword::Given, pattern, crate_id);
    }
    let steps = [create_test_step(StepKeyword::Given, step_text)];
    assert!(validate_steps_exist(&steps, true).is_err());
    assert!(validate_steps_exist(&steps, false).is_ok());
}

#[rstest]
#[case::literal("a step", "a step", "a step")]
#[case::placeholder("I have {item}", "I have {n:u32}", "I have 1")]
#[serial]
fn errors_when_step_ambiguous(
    #[case] pattern_a: &str,
    #[case] pattern_b: &str,
    #[case] text: &str,
) {
    clear_registry();
    let lit_a = syn::LitStr::new(pattern_a, proc_macro2::Span::call_site());
    let lit_b = syn::LitStr::new(pattern_b, proc_macro2::Span::call_site());
    register_step(StepKeyword::Given, &lit_a);
    register_step(StepKeyword::Given, &lit_b);
    let steps = [create_test_step(StepKeyword::Given, text)];
    let err = match validate_steps_exist(&steps, false) {
        Err(e) => e.to_string(),
        Ok(()) => panic!("expected ambiguous step error"),
    };
    assert!(err.contains("Ambiguous step definition"));
    assert!(err.contains(pattern_a));
    assert!(err.contains(pattern_b));
    assert_bullet_count(&err, 2);
    assert!(validate_steps_exist(&steps, true).is_err());
}

#[rstest]
#[serial]
fn aborts_on_invalid_step_pattern() {
    clear_registry();
    // proc-macro-error3 panics outside macro contexts; assert expected message
    let Err(err) = std::panic::catch_unwind(|| {
        register_step(
            StepKeyword::Given,
            &syn::LitStr::new("unclosed {", proc_macro2::Span::call_site()),
        );
    }) else {
        panic!("expected invalid step pattern to abort");
    };
    let msg = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err.downcast_ref::<&str>().copied());
    let Some(msg) = msg else {
        panic!("panic payload must be a string");
    };
    assert!(
        msg.contains("proc-macro-error3 API cannot be used outside of `entry_point` invocation")
    );

    assert!(rstest_bdd_patterns::build_regex_from_pattern("unclosed {").is_err());
}

#[test]
#[serial]
fn errors_when_step_matches_three_definitions() {
    clear_registry();
    let lit_a = syn::LitStr::new("I have {item}", proc_macro2::Span::call_site());
    let lit_b = syn::LitStr::new("I have {n:u32}", proc_macro2::Span::call_site());
    let lit_c = syn::LitStr::new("I have 1", proc_macro2::Span::call_site());
    register_step(StepKeyword::Given, &lit_a);
    register_step(StepKeyword::Given, &lit_b);
    register_step(StepKeyword::Given, &lit_c);
    let steps = [create_test_step(StepKeyword::Given, "I have 1")];
    let err = match validate_steps_exist(&steps, false) {
        Err(e) => e.to_string(),
        Ok(()) => panic!("expected ambiguous step error"),
    };
    assert!(err.contains("Ambiguous step definition"));
    assert!(err.contains("I have {item}"));
    assert!(err.contains("I have {n:u32}"));
    assert!(err.contains("I have 1"));
    assert_bullet_count(&err, 3);
    assert!(validate_steps_exist(&steps, true).is_err());
}

#[test]
#[serial]
fn scoped_validation_uses_only_selected_libraries() {
    clear_registry();
    let account = syn::LitStr::new("the domain is empty", proc_macro2::Span::call_site());
    let filesystem = syn::LitStr::new("the domain is empty", proc_macro2::Span::call_site());
    register_step_in_library(StepKeyword::Given, &account, "accounts");
    register_step_in_library(StepKeyword::Given, &filesystem, "filesystem");
    let steps = [create_test_step(StepKeyword::Given, "the domain is empty")];
    let accounts = vec![Box::<str>::from("accounts")];

    assert!(validate_steps_exist_in_scope(&steps, &accounts, true).is_ok());
}

fn scoped_validation_error(definitions: &[(&str, &str)], selected_libraries: &[&str]) -> String {
    clear_registry();
    for (library, pattern) in definitions {
        let pattern = syn::LitStr::new(pattern, proc_macro2::Span::call_site());
        register_step_in_library(StepKeyword::Given, &pattern, library);
    }
    let steps = [create_test_step(StepKeyword::Given, "the domain is empty")];
    let libraries = selected_libraries
        .iter()
        .map(|library| Box::<str>::from(*library))
        .collect::<Vec<_>>();

    validate_steps_exist_in_scope(&steps, &libraries, true)
        .expect_err("scoped validation fixture must produce an error")
        .to_string()
}

#[test]
#[serial]
fn scoped_validation_reports_unselected_library_hints() {
    let error = scoped_validation_error(
        &[
            ("accounts", "a different account step"),
            ("filesystem", "the domain is empty"),
        ],
        &["accounts"],
    );
    assert!(error.contains("Selected libraries: [accounts]"));
    assert!(error.contains("unselected libraries"));
    assert!(error.contains("filesystem"));
}

#[test]
#[serial]
fn scoped_validation_reports_equal_candidates_without_precedence() {
    let error = scoped_validation_error(
        &[
            ("accounts", "the domain is empty"),
            ("filesystem", "the domain is empty"),
        ],
        &["accounts", "filesystem"],
    );
    assert!(error.contains("Ambiguous step definition"));
    assert!(error.contains("Selected libraries: [accounts, filesystem]"));
    assert!(error.contains("accounts"));
    assert!(error.contains("filesystem"));
}

#[test]
#[serial]
fn global_registration_populates_global_and_scoped_indexes() -> Result<(), String> {
    clear_registry();
    let crate_id = "global-registration-test";
    register_step_for_crate(StepKeyword::Given, "the global step", crate_id);

    let registry = REGISTERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let defs = registry
        .get(normalize_crate_id(crate_id).as_ref())
        .ok_or_else(|| "global registration should create a crate definition entry".to_owned())?;
    assert_eq!(defs.patterns(StepKeyword::Given).len(), 1);
    assert_eq!(
        defs.scoped_by_kw
            .get(&(Box::<str>::from("rstest_bdd::global"), StepKeyword::Given))
            .map(Vec::len),
        Some(1)
    );
    Ok(())
}

#[test]
#[serial]
fn named_library_registration_avoids_global_index() -> Result<(), String> {
    clear_registry();
    let pattern = syn::LitStr::new("the named step", proc_macro2::Span::call_site());
    register_step_in_library(StepKeyword::Given, &pattern, "accounts");

    let registry = REGISTERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let defs = registry
        .get(current_crate_id())
        .ok_or_else(|| "named registration should create a crate definition entry".to_owned())?;
    assert!(defs.patterns(StepKeyword::Given).is_empty());
    assert_eq!(
        defs.scoped_by_kw
            .get(&(Box::<str>::from("accounts"), StepKeyword::Given))
            .map(Vec::len),
        Some(1)
    );
    Ok(())
}

#[test]
fn normalizes_crate_id_without_out_dir_component() {
    assert_eq!(normalize_crate_id("my_crate").as_ref(), "my_crate");
}

#[cfg(windows)]
#[test]
fn normalizes_windows_drive_letter_out_dir() {
    let id = normalize_crate_id("demo:C:/a/b");
    assert_eq!(id.as_ref(), "demo:C:/a/b");
}

#[serial]
#[test]
fn normalizes_relative_out_dir_paths() {
    let temp = tempdir_in(".").expect("test setup should succeed");
    let abs = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| {
            format!(
                "temporary directory should be valid UTF-8: {}",
                path.display()
            )
        })
        .expect("test setup should succeed");
    let cwd = std::env::current_dir().expect("test setup should succeed");
    let cwd = Utf8PathBuf::from_path_buf(cwd)
        .map_err(|path| {
            format!(
                "current directory should be valid UTF-8: {}",
                path.display()
            )
        })
        .expect("test setup should succeed");
    let relative = abs.strip_prefix(&cwd).expect("test setup should succeed");
    let crate_id = format!("demo:./{}", relative.as_str());
    let normalized = normalize_crate_id(&crate_id);
    let canonical_abs = abs
        .as_path()
        .canonicalize_utf8()
        .unwrap_or_else(|_| abs.clone());
    let expected = format!("demo:{}", canonical_abs.as_str());
    assert_eq!(normalized.as_ref(), expected);
}

#[test]
fn leaves_unresolvable_out_dir_paths_unchanged() {
    let temp = tempdir().expect("create temp directory");
    let missing = temp.path().join("missing");
    let missing = Utf8PathBuf::from_path_buf(missing).expect("path should be valid UTF-8");
    let crate_id = format!("demo:{}", missing.as_str());
    let normalized = normalize_crate_id(&crate_id);
    assert_eq!(normalized.as_ref(), crate_id);
}

#[rstest]
#[serial]
fn canonicalize_out_dir_resolves_relative_components(
    temp_working_dir: std::io::Result<TempWorkingDir>,
) {
    let temp_working_dir = temp_working_dir.expect("test setup should succeed");
    let nested_dir = temp_working_dir.join("nested");
    create_dir_all_cap(nested_dir.as_path()).expect("test setup should succeed");
    let nested = temp_working_dir.join("nested/.");
    let canonical = canonicalize_out_dir(nested.as_path());
    let expected_dir = temp_working_dir.path().join("nested");
    let expected = expected_dir
        .as_path()
        .canonicalize_utf8()
        .unwrap_or_else(|_| expected_dir.clone());

    assert_eq!(canonical, expected);
    assert!(
        canonical.is_absolute(),
        "canonical path should be absolute: {canonical}"
    );
}

#[cfg(unix)]
#[test]
fn canonicalize_out_dir_resolves_symlinks() {
    let temp = tempdir().expect("create temp directory");
    let base = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .expect("temporary directory should be valid UTF-8");
    let target = base.join("target");
    create_dir_all_cap(target.as_path()).expect("create target directory for canonicalization");
    let link = base.join("link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(target.as_std_path(), link.as_std_path())
        .expect("create symlink to target"); // replace with cap-std when available

    let canonical = canonicalize_out_dir(link.as_path());
    let expected = target
        .as_path()
        .canonicalize_utf8()
        .unwrap_or_else(|_| target.clone());

    assert_eq!(canonical, expected);
}

#[test]
fn canonicalize_out_dir_returns_original_when_unresolvable() {
    let temp = tempdir().expect("create temp directory");
    let missing = temp.path().join("missing");
    let missing = Utf8PathBuf::from_path_buf(missing).expect("path should be valid UTF-8");
    assert_eq!(canonicalize_out_dir(missing.as_path()), missing);
}

#[serial]
#[test]
fn canonicalizes_equivalent_crate_paths_in_registry() {
    clear_registry();
    let temp = tempdir().expect("test setup should succeed");
    let abs = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| {
            format!(
                "temporary directory should be valid UTF-8: {}",
                path.display()
            )
        })
        .expect("test setup should succeed");
    let crate_id = format!("demo:{}", abs.as_str());
    let alt_id = format!("demo:{}/.", abs.as_str());

    register_step_for_crate(StepKeyword::Given, "first pattern", &crate_id);
    register_step_for_crate(StepKeyword::Given, "second pattern", &alt_id);

    let registry = REGISTERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(
        registry.len(),
        1,
        "expected canonical crate IDs to share entry"
    );
    let (stored_id, defs) = registry
        .iter()
        .next()
        .ok_or("expected at least one crate entry")
        .expect("test setup should succeed");
    let expected_id = normalize_crate_id(&crate_id);
    assert_eq!(stored_id.as_ref(), expected_id.as_ref());

    let patterns = defs.patterns(StepKeyword::Given);
    assert_eq!(patterns.len(), 2, "expected both patterns to be stored");
    drop(registry);
    clear_registry();
}
