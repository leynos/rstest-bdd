//! Domain types for scenario code generation.
//!
//! These types provide semantic wrappers around string-based data structures
//! to improve code clarity and type safety in scenario code generation.

#![expect(
    clippy::expl_impl_clone_on_copy,
    reason = "base_newtype! generates paired Copy and Clone impls we cannot alter"
)]

use newt_hype::base_newtype;

macro_rules! string_wrapper {
    ($(#[$meta:meta])* $name:ident, $base:ident) => {
        base_newtype!($base);

        $(#[$meta])*
        #[doc = ""]
        #[doc = " # Examples"]
        #[doc = ""]
        #[doc = concat!(
            "```rust,ignore\n",
            "let value = ",
            stringify!($name),
            "::new(\"Given I have 5 items\");\n",
            "assert_eq!(value.as_str(), \"Given I have 5 items\");\n",
            "```"
        )]
        pub(crate) type $name = $base<String>;

        impl $name {
            /// Returns the wrapper contents as a string slice.
            pub(crate) fn as_str(&self) -> &str {
                self.as_ref()
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                <Self as AsRef<String>>::as_ref(self).as_str()
            }
        }
    };
}

macro_rules! vec_string_wrapper {
    ($(#[$meta:meta])* $name:ident, $base:ident) => {
        base_newtype!($base);

        $(#[$meta])*
        #[doc = ""]
        #[doc = " # Examples"]
        #[doc = ""]
        #[doc = concat!(
            "```rust,ignore\n",
            "let values = vec![\"count\".to_string()];\n",
            "let wrapper = ",
            stringify!($name),
            "::new(values);\n",
            "assert_eq!(wrapper.as_slice(), &[\"count\".to_string()]);\n",
            "```"
        )]
        pub(crate) type $name = $base<Vec<String>>;

        impl $name {
            /// Returns the wrapper contents as a slice.
            pub(crate) fn as_slice(&self) -> &[String] {
                self.as_ref()
            }
        }

        impl AsRef<[String]> for $name {
            fn as_ref(&self) -> &[String] {
                <Self as AsRef<Vec<String>>>::as_ref(self).as_slice()
            }
        }
    };
}

string_wrapper!(
    /// Wraps step text content.
    StepText,
    StepTextBase
);

vec_string_wrapper!(
    /// Wraps Examples table column names.
    ExampleHeaders,
    ExampleHeadersBase
);

vec_string_wrapper!(
    /// Wraps Examples table row values.
    ExampleRow,
    ExampleRowBase
);

string_wrapper!(
    /// Wraps multiline documentation strings.
    Docstring,
    DocstringBase
);
