//! Compile-time tests for the procedural macros.

use std::borrow::Cow;
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

#[test]
fn step_macros_compile() {
    let t = trybuild::TestCases::new();
    t.pass("tests/fixtures/step_macros.rs");
    t.pass("tests/fixtures/step_macros_unicode.rs");
    t.pass("tests/fixtures/scenario_single_match.rs");
    // `scenarios!` should succeed when the directory exists.
    // t.pass("tests/fixtures/scenarios_autodiscovery.rs");
    t.compile_fail("tests/fixtures/scenario_missing_file.rs");
    t.compile_fail("tests/fixtures/step_macros_invalid_identifier.rs");
    t.compile_fail("tests/fixtures/step_tuple_pattern.rs");
    t.compile_fail("tests/fixtures/step_struct_pattern.rs");
    t.compile_fail("tests/fixtures/step_nested_pattern.rs");
    t.compile_fail("tests/ui/datatable_wrong_type.rs");
    t.compile_fail("tests/ui/datatable_duplicate.rs");
    t.compile_fail("tests/ui/datatable_duplicate_attr.rs");
    t.compile_fail("tests/ui/datatable_after_docstring.rs");
    t.compile_fail("tests/ui/placeholder_missing_param.rs");
    t.compile_fail("tests/ui/implicit_fixture_missing.rs");
    t.compile_fail("tests/ui/placeholder_missing_params.rs");
    t.compile_fail("tests/fixtures/scenarios_missing_dir.rs");
    if cfg!(feature = "strict-compile-time-validation") {
        t.compile_fail("tests/fixtures/scenario_missing_step.rs");
        t.compile_fail("tests/fixtures/scenario_out_of_order.rs");
    } else {
        t.pass("tests/fixtures/scenario_missing_step.rs");
        t.pass("tests/fixtures/scenario_out_of_order.rs");
        compile_fail_missing_step_warning(&t);
    }
    if cfg!(feature = "compile-time-validation") {
        t.compile_fail("tests/fixtures/scenario_ambiguous_step.rs");
    }
}

type Normaliser = fn(&str) -> String;

fn compile_fail_missing_step_warning(t: &trybuild::TestCases) {
    compile_fail_with_normalised_output(
        t,
        "tests/fixtures/scenario_missing_step_warning.rs",
        &[strip_nightly_macro_backtrace_hint],
    );
}

fn compile_fail_with_normalised_output(
    t: &trybuild::TestCases,
    test_path: &str,
    normalisers: &[Normaliser],
) {
    match panic::catch_unwind(AssertUnwindSafe(|| t.compile_fail(test_path))) {
        Ok(()) => (),
        Err(panic) => {
            if normalised_outputs_match(test_path, normalisers).unwrap_or(false) {
                return;
            }

            panic::resume_unwind(panic);
        }
    }
}

fn normalised_outputs_match(test_path: &str, normalisers: &[Normaliser]) -> io::Result<bool> {
    let actual_path = wip_stderr_path(test_path);
    let expected_path = expected_stderr_path(test_path);
    let actual = fs::read_to_string(&actual_path)?;
    let expected = fs::read_to_string(&expected_path)?;

    if apply_normalisers(&actual, normalisers) == apply_normalisers(&expected, normalisers) {
        let _ = fs::remove_file(actual_path);
        return Ok(true);
    }

    Ok(false)
}

fn wip_stderr_path(test_path: &str) -> PathBuf {
    let Some(file_name) = Path::new(test_path).file_name() else {
        panic!("trybuild test path must include file name");
    };
    let mut path = PathBuf::from(file_name);
    path.set_extension("stderr");
    Path::new("target/tests/wip").join(path)
}

fn expected_stderr_path(test_path: &str) -> PathBuf {
    let mut path = PathBuf::from(test_path);
    path.set_extension("stderr");
    path
}

fn apply_normalisers<'a>(text: &'a str, normalisers: &[Normaliser]) -> Cow<'a, str> {
    let mut value = Cow::Borrowed(text);
    for normalise in normalisers {
        value = Cow::Owned(normalise(value.as_ref()));
    }
    value
}

fn strip_nightly_macro_backtrace_hint(text: &str) -> String {
    text.replace(
        " (in Nightly builds, run with -Z macro-backtrace for more info)",
        "",
    )
}
