//! Wrapper newtypes for trybuild fixtures and normaliser helpers, providing UTF-8-aware conversions for the trybuild harness.
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
