//! What a caller tells a preparation, and what it reports back.
//!
//! [`Sources`] is the input side: a family name, a variant label and a hash of
//! the inputs it builds. [`PreparationSummary`] is the output side: what one
//! run of that family's preparation commands actually did, for the log line.
//! Both live beside [`Preparation`](super::Preparation) rather than inside it
//! because they are the two ends of the same exchange and are never used
//! together by the same code.

/// What a preparation builds, as the caller describes it.
///
/// These three travel together: a family is named, it builds a variant, and
/// what it builds is identified by a hash of its inputs. Grouping them keeps
/// [`Preparation::new`](super::Preparation::new) readable and stops a caller
/// from pairing one family's name with another's input hash.
#[derive(Debug, Clone)]
pub struct Sources {
    /// Stable kebab-case family name, used in the stamp file name and logs.
    pub family: String,
    /// Caller-supplied label for the feature set or tool variant, such as
    /// `all-features` or the resolved Clippy version.
    pub variant: String,
    /// Caller-supplied hash of the fixture sources and lockfiles.
    pub inputs: String,
}

impl Sources {
    /// Name a family, its variant, and the hash of the inputs it builds.
    #[must_use]
    pub fn new(
        family: impl Into<String>,
        variant: impl Into<String>,
        inputs: impl Into<String>,
    ) -> Self {
        Self {
            family: family.into(),
            variant: variant.into(),
            inputs: inputs.into(),
        }
    }
}

/// What one family's preparation did, for the log line.
#[derive(Debug, Default, Clone, Copy)]
pub struct PreparationSummary {
    /// Number of `Compiling` lines the nested builds emitted.
    pub compiled_units: usize,
}
