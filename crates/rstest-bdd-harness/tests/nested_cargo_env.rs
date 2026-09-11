//! Unit tests for the shared nested-Cargo helper.
//!
//! Spawned-process tests are gated to Unix following the convention in
//! `binary_test_support_cargo.rs`; Windows runners cannot reliably spawn the
//! stub commands used here under nextest.

#![cfg(unix)]
use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
    process::Command,
    time::Duration,
};

use rstest_bdd_harness::nested_cargo::{
    CapturedOutput,
    cargo_command,
    describe_env,
    env_from_vars,
    filtered_command,
    run_bounded_with_timeout,
};

fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)))
        .collect()
}

fn fallback() -> PathBuf { PathBuf::from("/tmp/fallback-target") }

/// Look up one environment entry on a built command.
fn cmd_env(cmd: &Command, key: &str) -> Option<OsString> {
    cmd.get_envs()
        .find(|(name, _)| name.to_string_lossy() == key)
        .and_then(|(_, value)| value.map(OsString::from))
}

#[test]
fn strips_makeflags_pkg_and_llvm_cov_variables() {
    let env = env_from_vars(
        vars(&[
            ("CARGO_MAKEFLAGS", "-j4"),
            ("CARGO_PKG_NAME", "outer"),
            ("CARGO_LLVM_COV", "1"),
            ("CARGO_LLVM_COV_TARGET_DIR", "/cov"),
            ("CARGO_LLVM_COV_SHOW_MISSING", "1"),
            ("CARGO_TARGET_DIR", "/shared-target"),
            ("PATH", "/usr/bin"),
        ]),
        Some(&fallback()),
    );
    assert!(!env.contains_key("CARGO_MAKEFLAGS"));
    assert!(!env.contains_key("CARGO_PKG_NAME"));
    assert!(!env.contains_key("CARGO_LLVM_COV"));
    assert!(!env.contains_key("CARGO_LLVM_COV_TARGET_DIR"));
    assert!(!env.contains_key("CARGO_LLVM_COV_SHOW_MISSING"));
    assert_eq!(
        env.get("CARGO_TARGET_DIR"),
        Some(OsStr::new("/shared-target"))
    );
    assert_eq!(env.get("PATH"), Some(OsStr::new("/usr/bin")));
}

#[test]
fn falls_back_to_caller_target_directory_when_unset() {
    let env = env_from_vars(vars(&[("PATH", "/usr/bin")]), Some(&fallback()));
    assert_eq!(
        env.get("CARGO_TARGET_DIR"),
        Some(OsStr::new("/tmp/fallback-target"))
    );
}

#[test]
#[should_panic(expected = "no CARGO_TARGET_DIR")]
fn panics_without_target_dir_or_fallback() {
    let _ = env_from_vars(vars(&[("PATH", "/usr/bin")]), None);
}

#[test]
fn redirects_inherited_profile_file_to_unique_child_path() {
    let env = env_from_vars(
        vars(&[
            ("CARGO_TARGET_DIR", "/shared-target"),
            ("LLVM_PROFILE_FILE", "/cov/parent-%m.profraw"),
        ]),
        Some(&fallback()),
    );
    let redirected = env.get("LLVM_PROFILE_FILE").expect("redirected profile");
    let redirected = redirected.to_string_lossy();
    assert!(
        redirected.starts_with(std::env::temp_dir().to_string_lossy().as_ref()),
        "redirected profile must live in the scratch root: {redirected}"
    );
    assert!(redirected.contains("nested-"));
    assert!(!redirected.contains("/cov/parent"));
    assert!(
        redirected.contains("%p"),
        "profile pattern must stay per-process: {redirected}"
    );
}

#[test]
fn keeps_parent_profile_file_when_not_inherited() {
    let env = env_from_vars(
        vars(&[("CARGO_TARGET_DIR", "/shared-target")]),
        Some(&fallback()),
    );
    assert!(!env.contains_key("LLVM_PROFILE_FILE"));
}

#[test]
fn redirected_profile_paths_are_unique_per_call() {
    let base = vars(&[
        ("CARGO_TARGET_DIR", "/shared-target"),
        ("LLVM_PROFILE_FILE", "/cov/parent.profraw"),
    ]);
    let first = env_from_vars(base.clone(), Some(&fallback()))
        .get("LLVM_PROFILE_FILE")
        .expect("first")
        .to_os_string();
    let second = env_from_vars(base, Some(&fallback()))
        .get("LLVM_PROFILE_FILE")
        .expect("second")
        .to_os_string();
    assert_ne!(first, second, "concurrent children need distinct profiles");
}

#[test]
fn apply_to_replaces_the_whole_child_environment() {
    let env = env_from_vars(
        vars(&[
            ("CARGO_TARGET_DIR", "/shared-target"),
            ("RUSTFLAGS", "-Dwarnings"),
        ]),
        Some(&fallback()),
    );
    let mut cmd = Command::new("true");
    cmd.env("LEFTOVER", "stale");
    env.apply_to(&mut cmd);
    let recorded: Vec<_> = cmd.get_envs().collect();
    assert!(recorded.contains(&(
        &OsString::from("RUSTFLAGS"),
        Some(&OsString::from("-Dwarnings"))
    )));
    assert!(
        !recorded
            .iter()
            .any(|(key, _)| key == &OsString::from("LEFTOVER"))
    );
}

#[test]
fn cargo_command_pins_cwd_and_environment() {
    let env = env_from_vars(
        vars(&[("CARGO_TARGET_DIR", "/shared-target")]),
        Some(&fallback()),
    );
    let cwd = std::env::temp_dir();
    let cmd = cargo_command(&env, &cwd);
    assert_eq!(cmd.get_current_dir(), Some(cwd.as_path()));
    assert_eq!(
        cmd.get_program(),
        OsStr::new(env!("CARGO")),
        "the nested cargo must come from the CARGO compile-time variable"
    );
    assert_eq!(
        cmd_env(&cmd, "CARGO_TARGET_DIR").as_deref(),
        Some(OsStr::new("/shared-target"))
    );
    assert_eq!(cmd_env(&cmd, "CARGO_MAKEFLAGS"), None);
}

#[test]
fn filtered_command_targets_the_program_and_cwd() {
    let env = env_from_vars(
        vars(&[("CARGO_TARGET_DIR", "/shared-target")]),
        Some(&fallback()),
    );
    let program = PathBuf::from("/usr/bin/env");
    let cwd = std::env::temp_dir();
    let cmd = filtered_command(&program, &env, &cwd);
    assert_eq!(cmd.get_program(), program.as_os_str());
    assert_eq!(cmd.get_current_dir(), Some(cwd.as_path()));
    assert_eq!(
        cmd_env(&cmd, "CARGO_TARGET_DIR").as_deref(),
        Some(OsStr::new("/shared-target"))
    );
}

#[test]
fn describe_env_lists_sorted_key_value_pairs() {
    let env = env_from_vars(
        vars(&[
            ("ZZ_LAST", "1"),
            ("CARGO_TARGET_DIR", "/shared-target"),
            ("AA_FIRST", "0"),
        ]),
        Some(&fallback()),
    );
    let description = describe_env(&env);
    let mut lines = description.lines();
    assert!(
        lines
            .next()
            .expect("sorted env is non-empty")
            .starts_with("AA_FIRST=")
    );
    assert!(
        lines
            .next()
            .expect("sorted env is non-empty")
            .starts_with("CARGO_TARGET_DIR=")
    );
    assert!(
        lines
            .next()
            .expect("sorted env is non-empty")
            .starts_with("ZZ_LAST=")
    );
    assert_eq!(lines.next(), None);
}

#[cfg(unix)]
#[test]
fn captured_output_combines_streams_and_preserves_status() {
    let captured = CapturedOutput {
        status: true,
        stdout: "out".to_owned(),
        stderr: "err".to_owned(),
    };
    assert!(captured.status);
    assert_eq!(captured.combined(), "stdout:\nout\nstderr:\nerr");
    assert_eq!(captured.to_string(), captured.combined());
    let mut failing = Command::new("sh");
    failing.args(["-c", "echo bytes; exit 3"]);
    let raw = run_bounded_with_timeout(&mut failing, Duration::from_secs(10))
        .expect("sh is available on this platform");
    let from_raw = CapturedOutput::from(raw);
    assert!(!from_raw.status);
    assert_eq!(from_raw.stdout, "bytes\n");
    assert_eq!(from_raw.stderr, "");
}

#[cfg(unix)]
#[test]
fn run_bounded_captures_output_and_status() {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "echo nested-out; echo nested-err >&2; exit 0"]);
    let output = run_bounded_with_timeout(&mut cmd, Duration::from_secs(10))
        .expect("sh is available on this platform");
    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("nested-out"),
        "stdout must be captured: {:?}",
        output.stdout
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("nested-err"),
        "stderr must be captured: {:?}",
        output.stderr
    );
}

#[cfg(unix)]
#[test]
fn run_bounded_reports_timeout_with_captured_output() {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "echo partial; sleep 30"]);
    let error = run_bounded_with_timeout(&mut cmd, Duration::from_millis(200))
        .expect_err("the child outlives the bound");
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    let message = error.to_string();
    assert!(message.contains("wall-clock bound"), "{message}");
    assert!(
        message.contains("partial"),
        "captured stdout must be attached: {message}"
    );
}

#[cfg(unix)]
#[test]
fn run_bounded_surfaces_spawn_failures() {
    let mut cmd = Command::new("definitely-not-a-real-binary-3f8a");
    let error = run_bounded_with_timeout(&mut cmd, Duration::from_secs(1))
        .expect_err("the child cannot spawn");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}
