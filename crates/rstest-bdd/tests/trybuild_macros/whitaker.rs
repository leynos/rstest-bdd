//! Whitaker lint-gate regression tests for the trybuild macro suite.

use camino::{Utf8Path, Utf8PathBuf};
use std::env;
use std::fs;
use std::process::{Command, Output};

#[test]
fn whitaker_lint_gate_accepts_clean_and_rejects_panicking_fixtures() {
    if !cargo_dylint_available() {
        return;
    }

    let Ok(temp) = tempfile::tempdir() else {
        panic!("temp directory should be created");
    };
    let Ok(temp_dir) = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()) else {
        panic!("temp directory path must be valid UTF-8");
    };
    let clean_manifest = write_whitaker_fixture(
        &temp_dir.join("clean"),
        "pub fn clean_value(value: Option<u32>) -> u32 {\n\
             let Some(number) = value else {\n\
                 panic!(\"missing value\");\n\
             };\n\
             number\n\
         }\n",
    );
    let rejected_manifest = write_whitaker_fixture(
        &temp_dir.join("rejected"),
        &[
            "pub fn rejected_value(value: Option<u32>) -> u32 {\n",
            "    value.",
            "unwrap_or_else",
            "(||",
            "panic!",
            "(\"missing value\"))\n",
            "}\n",
        ]
        .concat(),
    );

    let clean = run_whitaker_lint_fixture(&clean_manifest);
    assert!(
        clean.status.success(),
        "clean Whitaker fixture should pass\n{}",
        command_output(&clean),
    );

    let rejected = run_whitaker_lint_fixture(&rejected_manifest);
    assert!(
        !rejected.status.success(),
        "panicking Whitaker fixture should fail\n{}",
        command_output(&rejected),
    );
    let output = command_output(&rejected);
    assert!(
        output.contains("no_unwrap_or_else_panic"),
        "Whitaker lint name should appear in output\n{output}",
    );
    assert!(
        output.contains("unwrap_or_else"),
        "Whitaker output should identify the banned call\n{output}",
    );
}

#[test]
fn whitaker_artifact_paths_are_toolchain_scoped() {
    let target_dir_a = make_variable("WHITAKER_TARGET_DIR", "nightly-test-a");
    let target_dir_b = make_variable("WHITAKER_TARGET_DIR", "nightly-test-b");
    let toolchain_stamp_a = make_variable("WHITAKER_TOOLCHAIN_STAMP", "nightly-test-a");
    let build_stamp_a = make_variable("WHITAKER_BUILD_STAMP", "nightly-test-a");

    assert_ne!(
        target_dir_a, target_dir_b,
        "changing WHITAKER_TOOLCHAIN must select a different target dir",
    );
    assert!(
        target_dir_a.contains("v0.2.5-nightly-test-a-target"),
        "target dir should include tag and toolchain: {target_dir_a}",
    );
    assert!(
        toolchain_stamp_a.starts_with(&target_dir_a),
        "toolchain stamp should live under toolchain-scoped target dir",
    );
    assert!(
        build_stamp_a.starts_with(&target_dir_a),
        "build stamp should live under toolchain-scoped target dir",
    );
}

fn cargo_dylint_available() -> bool {
    match Command::new("cargo")
        .arg("dylint")
        .arg("--version")
        .output()
    {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

fn write_whitaker_fixture(crate_dir: &Utf8Path, lib_rs: &str) -> Utf8PathBuf {
    let src_dir = crate_dir.join("src");
    let manifest_path = crate_dir.join("Cargo.toml");
    let Some(fixture_name) = crate_dir.file_name() else {
        panic!("Whitaker fixture directory should include a name");
    };
    if let Err(err) = fs::create_dir_all(src_dir.as_std_path()) {
        panic!("failed to create Whitaker fixture source directory: {err}");
    }
    if let Err(err) = fs::write(
        manifest_path.as_std_path(),
        format!(
            "[package]\n\
             name = \"whitaker-rust-fixture-{fixture_name}\"\n\
             version = \"0.0.0\"\n\
             edition = \"2024\"\n\
             \n\
             [lib]\n\
             path = \"src/lib.rs\"\n"
        ),
    ) {
        panic!("failed to write Whitaker fixture manifest: {err}");
    }
    if let Err(err) = fs::write(src_dir.join("lib.rs").as_std_path(), lib_rs) {
        panic!("failed to write Whitaker fixture source: {err}");
    }
    manifest_path
}

fn run_whitaker_lint_fixture(manifest_path: &Utf8Path) -> Output {
    let repo_root = repo_root();
    let Some(fixture_dir) = manifest_path.parent() else {
        panic!("Whitaker fixture manifest should have a parent directory");
    };
    let Some(fixture_name) = fixture_dir.file_name() else {
        panic!("Whitaker fixture directory should include a name");
    };
    let target_dir = repo_root.join(format!("target/tests/whitaker_ui/{fixture_name}"));
    match Command::new("make")
        .current_dir(repo_root.as_std_path())
        .env("CARGO_TARGET_DIR", target_dir.as_str())
        .arg("--no-print-directory")
        .arg("lint-whitaker")
        .arg(format!(
            "CARGO_FLAGS=--manifest-path {manifest_path} --all-targets"
        ))
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to run make lint-whitaker: {err}"),
    }
}

fn make_variable(variable: &str, toolchain: &str) -> String {
    let repo_root = repo_root();
    let eval = "print-%: ; @printf \"%s\\n\" \"$($*)\"";
    let output = match Command::new("make")
        .current_dir(repo_root.as_std_path())
        .arg("--no-print-directory")
        .arg("--eval")
        .arg(eval)
        .arg(format!("print-{variable}"))
        .arg(format!("WHITAKER_TOOLCHAIN={toolchain}"))
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to query Makefile variable {variable}: {err}"),
    };
    assert!(
        output.status.success(),
        "make variable query failed for {variable}\n{}",
        command_output(&output),
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn repo_root() -> Utf8PathBuf {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(crate_parent) = manifest_dir.parent() else {
        panic!("crate manifest directory should have a parent");
    };
    let Some(repo_root) = crate_parent.parent() else {
        panic!("crate parent should have a repository parent");
    };
    repo_root.to_path_buf()
}

fn command_output(output: &Output) -> String {
    format!(
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}
