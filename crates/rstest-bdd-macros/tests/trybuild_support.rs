use std::borrow::Cow;
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

pub type Normaliser = fn(&str) -> String;

#[derive(Clone, Copy, Debug)]
pub struct FixturePath<'a> {
    raw: &'a str,
}

impl<'a> FixturePath<'a> {
    #[must_use]
    pub fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    #[must_use]
    pub fn as_str(self) -> &'a str {
        self.raw
    }

    #[must_use]
    pub fn expected_stderr_path(self) -> PathBuf {
        let mut path = PathBuf::from(self.raw);
        path.set_extension("stderr");
        path
    }

    /// Return the path to the wip stderr file for this fixture.
    ///
    /// # Panics
    ///
    /// Panics if the fixture path does not include a file name.
    #[must_use]
    pub fn wip_stderr_path(self) -> PathBuf {
        let Some(file_name) = Path::new(self.raw).file_name() else {
            panic!("trybuild test path must include file name");
        };
        let mut path = PathBuf::from(file_name);
        path.set_extension("stderr");
        Path::new("target/tests/wip").join(path)
    }
}

impl<'a> From<&'a str> for FixturePath<'a> {
    fn from(raw: &'a str) -> Self {
        FixturePath::new(raw)
    }
}

impl AsRef<str> for FixturePath<'_> {
    fn as_ref(&self) -> &str {
        self.raw
    }
}

#[cfg(not(feature = "strict-compile-time-validation"))]
pub fn compile_fail_with_normalised_output(
    t: &trybuild::TestCases,
    test_path: FixturePath<'_>,
    normalisers: &[Normaliser],
) {
    run_compile_fail_with_normalised_output(
        || t.compile_fail(test_path.as_str()),
        test_path,
        normalisers,
    );
}

/// Run a compile-fail assertion and reconcile trybuild outputs.
///
/// # Panics
///
/// Panics if reading or normalising the output fixtures fails.
pub fn run_compile_fail_with_normalised_output<F>(
    compile_fail: F,
    test_path: FixturePath<'_>,
    normalisers: &[Normaliser],
) where
    F: FnOnce(),
{
    match panic::catch_unwind(AssertUnwindSafe(compile_fail)) {
        Ok(()) => (),
        Err(panic) => {
            match normalised_outputs_match(test_path, normalisers) {
                Ok(true) => return,
                Ok(false) => (),
                Err(error) => panic!("failed to normalise trybuild outputs: {error}"),
            }

            panic::resume_unwind(panic);
        }
    }
}

/// Compare the normalised stderr outputs for a fixture.
///
/// # Errors
///
/// Returns an error if the fixture stderr files cannot be read.
pub fn normalised_outputs_match(
    test_path: FixturePath<'_>,
    normalisers: &[Normaliser],
) -> io::Result<bool> {
    let actual_path = test_path.wip_stderr_path();
    let expected_path = test_path.expected_stderr_path();
    let actual = fs::read_to_string(&actual_path)?;
    let expected = fs::read_to_string(&expected_path)?;

    if apply_normalisers(&actual, normalisers) == apply_normalisers(&expected, normalisers) {
        let _ = fs::remove_file(actual_path);
        return Ok(true);
    }

    Ok(false)
}

pub fn apply_normalisers<'a>(text: &'a str, normalisers: &[Normaliser]) -> Cow<'a, str> {
    let mut value = Cow::Borrowed(text);
    for normalise in normalisers {
        value = Cow::Owned(normalise(value.as_ref()));
    }
    value
}

pub fn normalise_fixture_paths(text: &str) -> String {
    let normalised_lines = text
        .lines()
        .map(normalise_fixture_path_line)
        .collect::<Vec<_>>();
    let separator = char::from(0x0A);
    let separator_str = separator.to_string();
    let mut normalised = normalised_lines.join(&separator_str);
    if text.ends_with(separator) {
        normalised.push(separator);
    }
    normalised
}

#[must_use]
pub fn normalise_fixture_path_line(line: &str) -> String {
    const ARROW: &str = "-->";

    let Some((prefix, remainder)) = line.split_once(ARROW) else {
        return line.to_owned();
    };

    let trimmed = remainder.trim_start();
    if trimmed.is_empty() || !trimmed.contains(".rs") {
        return line.to_owned();
    }

    let mut parts = trimmed.splitn(2, ':');
    let path = parts.next().unwrap_or(trimmed);
    let suffix = parts.next();

    let file_name = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(path);

    let mut rebuilt = format!("{prefix}{ARROW} ");
    rebuilt.push('$');
    rebuilt.push_str("DIR/");
    rebuilt.push_str(file_name);
    if let Some(rest) = suffix {
        if !rest.is_empty() {
            rebuilt.push(':');
            rebuilt.push_str(rest);
        }
    }

    rebuilt
}

#[must_use]
pub fn strip_nightly_macro_backtrace_hint(text: &str) -> String {
    text.replace(
        " (in Nightly builds, run with -Z macro-backtrace for more info)",
        "",
    )
}
