//! Strongly typed metadata for generated scenarios.
#![expect(
    clippy::expl_impl_clone_on_copy,
    reason = "base_newtype! generates paired Copy and Clone impls we cannot alter"
)]

//! Scenario metadata wrappers shared across macro code generation.
use newt_hype::base_newtype;

macro_rules! metadata_string {
    ($(#[$meta:meta])* $name:ident, $base:ident) => {
        base_newtype!($base);

        $(#[$meta])*
        pub(crate) type $name = $base<String>;

        impl $name {
            pub(crate) fn as_str(&self) -> &str {
                self.as_ref()
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                $base::new(value.to_string())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                <Self as AsRef<String>>::as_ref(self).as_str()
            }
        }
    };
}

metadata_string!(
    /// Path to a feature file on disk.
    FeaturePath,
    FeaturePathBase
);

metadata_string!(
    /// Name of an individual Gherkin scenario.
    ScenarioName,
    ScenarioNameBase
);
