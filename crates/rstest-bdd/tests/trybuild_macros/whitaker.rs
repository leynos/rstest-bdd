//! Whitaker lint-gate regression tests for the trybuild macro suite.

use std::{
    env,
    fs,
    process::{Command, Output},
};

use camino::{Utf8Path, Utf8PathBuf};

#[test]
fn whitaker_lint_gate_accepts_clean_and_rejects_panicking_fixtures() {
    if !cargo_dylint_available() || !whitaker_available() {
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
        concat!(
            "//! Clean Whitaker fixture crate.\npub fn clean_value(value: Option<u32>) -> u32 {\nlet ",
            "Some(number) = value else {\npanic!(\"missing value\");\n};\nnumber\n}\n",
        ),
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

fn cargo_dylint_available() -> bool {
    command_succeeds(Command::new("cargo").arg("dylint").arg("--version"))
}

fn whitaker_available() -> bool { command_succeeds(Command::new("whitaker").arg("--version")) }

fn command_succeeds(command: &mut Command) -> bool {
    match command.output() {
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
            "[package]\nname = \"whitaker-rust-fixture-{fixture_name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/lib.rs\"\n"
        ),
    ) {
        panic!("failed to write Whitaker fixture manifest: {err}");
    }
    if let Err(err) = fs::write(src_dir.join("lib.rs").as_std_path(), lib_rs) {
        panic!("failed to write Whitaker fixture source: {err}");
    }
    manifest_path
}

/// Variables `cargo llvm-cov` exports to its test processes.
///
/// A nested nightly build that inherits them is instrumented by the nightly's
/// LLVM, so it writes raw profiles in a newer or older format than the stable
/// `llvm-profdata` merging them, and the whole coverage merge fails
/// ("no profile can be merged"). The lint gate fixture needs none of them.
const COVERAGE_ENV: [&str; 5] = [
    "LLVM_PROFILE_FILE",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_LLVM_COV",
    "CARGO_LLVM_COV_SHOW_ENV",
    "CARGO_LLVM_COV_TARGET_DIR",
];

/// Builds the `make lint-whitaker` command with coverage instrumentation removed.
fn whitaker_lint_command(repo_root: &Utf8Path) -> Command {
    let mut command = Command::new("make");
    command.current_dir(repo_root.as_std_path());
    for name in COVERAGE_ENV {
        command.env_remove(name);
    }
    command
}

#[test]
fn the_whitaker_fixture_command_inherits_no_coverage_instrumentation() {
    let command = whitaker_lint_command(Utf8Path::new("."));
    // The two variables that make a child build write profiles into the
    // parent's merge directory, named here rather than read from the list the
    // command is built from, so dropping one from that list fails this test.
    let required = ["LLVM_PROFILE_FILE", "CARGO_ENCODED_RUSTFLAGS"];
    for name in required.into_iter().chain(COVERAGE_ENV) {
        let removed = command
            .get_envs()
            .any(|(key, value)| key == name && value.is_none());
        assert!(removed, "{name} must be removed from the fixture command");
    }
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
    match whitaker_lint_command(&repo_root)
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
