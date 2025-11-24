//! Tests for step-definition validation: missing/single/ambiguous outcomes and registry behaviour.
// Intentionally left without file-wide lint suppressions; add per-function #[expect(...)] where needed.
use super::crate_id::{canonicalise_out_dir, normalise_crate_id};
use super::*;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::{fixture, rstest};
use serial_test::serial;
use tempfile::{tempdir, tempdir_in};

fn clear_registry() {
    #[expect(clippy::expect_used, reason = "registry lock must panic if poisoned")]
    REGISTERED.lock().expect("step registry poisoned").clear();
}

fn create_test_step(keyword: StepKeyword, text: &str) -> ParsedStep {
    ParsedStep {
        keyword,
        text: text.to_string(),
        docstring: None,
        table: None,
        #[cfg(feature = "compile-time-validation")]
        span: proc_macro2::Span::call_site(),
    }
}

fn assert_bullet_count(err: &str, expected: usize) {
    let bullet_count = err
        .lines()
        .filter(|l| l.trim_start().starts_with("- "))
        .count();
    assert_eq!(bullet_count, expected, "expected {expected} bullet matches");
}

struct TempWorkingDir {
    _temp: tempfile::TempDir,
    path: Utf8PathBuf,
    original_cwd: Utf8PathBuf,
}

impl TempWorkingDir {
    fn new(temp: tempfile::TempDir, path: Utf8PathBuf, original_cwd: Utf8PathBuf) -> Self {
        Self {
            _temp: temp,
            path,
            original_cwd,
        }
    }

    fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }
}

impl Drop for TempWorkingDir {
    #[expect(
        clippy::expect_used,
        reason = "restoring the working directory must succeed for cleanup"
    )]
    fn drop(&mut self) {
        std::env::set_current_dir(self.original_cwd.as_std_path())
            .expect("restore current directory");
    }
}

fn should_skip_creation(path: &Utf8Path) -> bool {
    path.as_str().is_empty() || path == Utf8Path::new(".")
}

fn ensure_parent_exists(path: &Utf8Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if parent != path {
            create_dir_all_cap(parent)?;
        }
    }

    Ok(())
}

fn with_dir<T>(
    path: &Utf8Path,
    op: impl FnOnce(&Dir, &Utf8Path) -> std::io::Result<T>,
) -> std::io::Result<T> {
    let authority = ambient_authority();
    if let Some(parent) = path.parent() {
        if should_skip_creation(parent) {
            let dir = Dir::open_ambient_dir(Utf8Path::new("."), authority)?;
            let target = path.file_name().map_or(path, Utf8Path::new);
            return op(&dir, target);
        }

        let dir = Dir::open_ambient_dir(parent, authority)?;
        let target = path.file_name().map_or(path, Utf8Path::new);
        return op(&dir, target);
    }

    let dir = Dir::open_ambient_dir(Utf8Path::new("."), authority)?;
    op(&dir, path)
}

fn create_single_dir(path: &Utf8Path) -> std::io::Result<()> {
    with_dir(path, |dir, target| {
        dir.create_dir(target).or_else(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(error)
            }
        })
    })
}

fn create_dir_all_cap(path: &Utf8Path) -> std::io::Result<()> {
    if should_skip_creation(path) {
        return Ok(());
    }

    if path.file_name().is_none() {
        return ensure_parent_exists(path);
    }

    ensure_parent_exists(path)?;
    create_single_dir(path)?;

    Ok(())
}

#[expect(
    clippy::expect_used,
    reason = "temporary directory setup relies on explicit failure messages for clarity"
)]
fn temp_working_dir_inner() -> TempWorkingDir {
    let original = std::env::current_dir().expect("obtain current directory");
    let original =
        Utf8PathBuf::from_path_buf(original).expect("current directory should be valid UTF-8");
    let temp = tempdir().expect("create temp directory");
    std::env::set_current_dir(temp.path()).expect("set current directory for test");

    let temp_path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .expect("temporary path should be valid UTF-8");
    TempWorkingDir::new(temp, temp_path, original)
}

#[fixture]
fn temp_working_dir() -> TempWorkingDir {
    temp_working_dir_inner()
}

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
    // proc-macro-error panics outside macro contexts; assert expected message
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
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap_or_else(|| panic!("panic payload must be a string"));
    assert!(msg.contains("proc-macro-error API cannot be used outside of `entry_point` invocation"));

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
fn normalises_crate_id_without_out_dir_component() {
    assert_eq!(normalise_crate_id("my_crate").as_ref(), "my_crate");
}

#[cfg(windows)]
#[test]
fn normalises_windows_drive_letter_out_dir() {
    let id = normalise_crate_id("demo:C:/a/b");
    assert_eq!(id.as_ref(), "demo:C:/a/b");
}

#[test]
#[serial]
#[expect(
    clippy::expect_used,
    reason = "test arranges filesystem state with explicit expect messages"
)]
fn normalises_relative_out_dir_paths() {
    let temp = tempdir_in(".").expect("create temp dir in current directory");
    let abs = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .expect("temporary directory should be valid UTF-8");
    let cwd = std::env::current_dir().expect("obtain current directory");
    let cwd = Utf8PathBuf::from_path_buf(cwd).expect("current directory should be valid UTF-8");
    let relative = abs
        .strip_prefix(&cwd)
        .expect("temporary directory to reside under current directory");
    let crate_id = format!("demo:./{}", relative.as_str());
    let normalised = normalise_crate_id(&crate_id);
    let canonical_abs = abs
        .as_path()
        .canonicalize_utf8()
        .unwrap_or_else(|_| abs.clone());
    let expected = format!("demo:{}", canonical_abs.as_str());
    assert_eq!(normalised.as_ref(), expected);
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test documents fallback behaviour with explicit expect messaging"
)]
fn leaves_unresolvable_out_dir_paths_unchanged() {
    let temp = tempdir().expect("create temp directory");
    let missing = temp.path().join("missing");
    let missing = Utf8PathBuf::from_path_buf(missing).expect("path should be valid UTF-8");
    let crate_id = format!("demo:{}", missing.as_str());
    let normalised = normalise_crate_id(&crate_id);
    assert_eq!(normalised.as_ref(), crate_id);
}

#[rstest]
#[serial]
#[expect(
    clippy::expect_used,
    reason = "test builds nested directories using explicit expect messaging"
)]
fn canonicalise_out_dir_resolves_relative_components(temp_working_dir: TempWorkingDir) {
    create_dir_all_cap(Utf8Path::new("nested"))
        .expect("create nested directory for canonicalisation");
    let nested = Utf8Path::new("nested/.");
    let canonical = canonicalise_out_dir(nested);
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
#[expect(
    clippy::expect_used,
    reason = "symlink setup uses expect to surface filesystem failures"
)]
fn canonicalise_out_dir_resolves_symlinks() {
    let temp = tempdir().expect("create temp directory");
    let base = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .expect("temporary directory should be valid UTF-8");
    let target = base.join("target");
    create_dir_all_cap(target.as_path()).expect("create target directory for canonicalisation");
    let link = base.join("link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(target.as_std_path(), link.as_std_path())
        .expect("create symlink to target"); // replace with cap-std when available

    let canonical = canonicalise_out_dir(link.as_path());
    let expected = target
        .as_path()
        .canonicalize_utf8()
        .unwrap_or_else(|_| target.clone());

    assert_eq!(canonical, expected);
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test asserts fallback path handling with explicit expect messaging"
)]
fn canonicalise_out_dir_returns_original_when_unresolvable() {
    let temp = tempdir().expect("create temp directory");
    let missing = temp.path().join("missing");
    let missing = Utf8PathBuf::from_path_buf(missing).expect("path should be valid UTF-8");
    assert_eq!(canonicalise_out_dir(missing.as_path()), missing);
}

#[test]
#[serial]
#[expect(
    clippy::expect_used,
    reason = "registry fixture wiring relies on explicit expect diagnostics"
)]
fn canonicalises_equivalent_crate_paths_in_registry() {
    clear_registry();
    let temp = tempdir().expect("create temp directory");
    let abs = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .expect("temporary directory should be valid UTF-8");
    let crate_id = format!("demo:{}", abs.as_str());
    let alt_id = format!("demo:{}/.", abs.as_str());

    register_step_for_crate(StepKeyword::Given, "first pattern", &crate_id);
    register_step_for_crate(StepKeyword::Given, "second pattern", &alt_id);

    let registry = REGISTERED.lock().expect("step registry poisoned");
    assert_eq!(
        registry.len(),
        1,
        "expected canonical crate IDs to share entry"
    );
    let (stored_id, defs) = registry
        .iter()
        .next()
        .expect("expected at least one crate entry");
    let expected_id = normalise_crate_id(&crate_id);
    assert_eq!(stored_id.as_ref(), expected_id.as_ref());

    let patterns = defs.patterns(StepKeyword::Given);
    assert_eq!(patterns.len(), 2, "expected both patterns to be stored");
    drop(registry);
    clear_registry();
}
