use std::path::PathBuf;
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

impl From<MacroFixtureCase> for PathBuf {
    fn from(value: MacroFixtureCase) -> Self {
        value.0.into()
    }
}

owned_str_newtype!(UiFixtureCase);

impl From<UiFixtureCase> for PathBuf {
    fn from(value: UiFixtureCase) -> Self {
        value.0.into()
    }
}

borrowed_str_newtype!(NormaliserInput);

borrowed_str_newtype!(FixturePathLine);

borrowed_str_newtype!(FixtureTestPath);

borrowed_str_newtype!(FixtureStderr);
