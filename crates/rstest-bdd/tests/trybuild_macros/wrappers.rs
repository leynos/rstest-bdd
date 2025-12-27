//! Wrapper newtypes used by the trybuild macro fixtures and normaliser helpers.
//! They centralise UTF-8 conversions so tests can work with camino paths and
//! expose standard-path views when talking to trybuild.
use camino::{Utf8Path, Utf8PathBuf};
use std::path::Path as StdPath;
use the_newtype::Newtype;

macro_rules! owned_str_newtype {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Newtype, PartialEq)]
        pub(crate) struct $name(pub(crate) String);

        impl ::std::ops::Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.0.as_str()
            }
        }

        impl ::std::convert::AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.0.as_str()
            }
        }

        impl<'a> ::std::convert::From<&'a str> for $name {
            fn from(value: &'a str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

macro_rules! borrowed_str_newtype {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Newtype, PartialEq)]
        pub(crate) struct $name<'a>(pub(crate) &'a str);

        impl<'a> ::std::ops::Deref for $name<'a> {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.0
            }
        }

        impl<'a> ::std::convert::AsRef<str> for $name<'a> {
            fn as_ref(&self) -> &str {
                self.0
            }
        }

        impl<'a> ::std::convert::From<&'a str> for $name<'a> {
            fn from(value: &'a str) -> Self {
                Self(value)
            }
        }
    };
}

owned_str_newtype!(MacroFixtureCase);

impl From<MacroFixtureCase> for Utf8PathBuf {
    fn from(value: MacroFixtureCase) -> Self {
        Self::from(value.0)
    }
}

impl AsRef<StdPath> for MacroFixtureCase {
    fn as_ref(&self) -> &StdPath {
        Utf8Path::new(self.0.as_str()).as_std_path()
    }
}

owned_str_newtype!(UiFixtureCase);

impl From<UiFixtureCase> for Utf8PathBuf {
    fn from(value: UiFixtureCase) -> Self {
        Self::from(value.0)
    }
}

impl AsRef<StdPath> for UiFixtureCase {
    fn as_ref(&self) -> &StdPath {
        Utf8Path::new(self.0.as_str()).as_std_path()
    }
}

borrowed_str_newtype!(NormaliserInput);

borrowed_str_newtype!(FixturePathLine);

borrowed_str_newtype!(FixtureTestPath);

borrowed_str_newtype!(FixtureStderr);

/// Normalises fixture paths in trybuild error output by stripping directory
/// prefixes, making assertions platform-independent.
pub(crate) fn normalise_fixture_paths(input: NormaliserInput<'_>) -> String {
    let text = input.as_ref();
    let mut normalised = text
        .lines()
        .map(|line| normalise_fixture_path_line(FixturePathLine::from(line)))
        .collect::<Vec<_>>()
        .join("\n");
    if text.ends_with('\n') {
        normalised.push('\n');
    }
    normalised
}

fn normalise_fixture_path_line(line: FixturePathLine<'_>) -> String {
    const ARROW: &str = "-->";
    let value = line.as_ref();
    let Some((prefix, remainder)) = value.split_once(ARROW) else {
        return value.to_owned();
    };
    let trimmed = remainder.trim_start();
    if trimmed.is_empty() || !trimmed.contains(".rs") {
        return value.to_owned();
    }
    let mut parts = trimmed.splitn(2, ':');
    let path = parts.next().unwrap_or(trimmed);
    let suffix = parts.next();
    let file_name = Utf8Path::new(path).file_name().unwrap_or(path);
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

/// Strips nightly-only macro backtrace hints from compiler output.
pub(crate) fn strip_nightly_macro_backtrace_hint(input: NormaliserInput<'_>) -> String {
    input.as_ref().replace(
        " (in Nightly builds, run with -Z macro-backtrace for more info)",
        "",
    )
}
