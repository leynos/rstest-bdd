//! The classified user arguments for one expansion boundary.
//!
//! A configuration is one of two shapes because the two macros expose
//! different inputs: `#[scenario]` knows its signature and the attributes the
//! author wrote, while `scenarios!` knows only whether the deprecated runtime
//! alias was supplied.
//!
//! The two user-attribute facts are separate on purpose. `rstest` accepts an
//! `async` test when any attribute's last segment is `test`, but the generator
//! deduplicates only against the *same* framework's attribute, so `#[test]`
//! satisfies the first and not the second.

use rstest_bdd_policy::RuntimeMode;

use super::model::SlotClass;

/// The explicit user arguments at one expansion boundary.
///
/// The harness field is the *explicit* `harness = ...` argument, never the
/// path that the deprecated `runtime` alias generates, because supplying the
/// alias is not supplying a harness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PolicyInputs {
    /// One `#[scenario]` expansion.
    Scenario {
        /// Classification of the explicit `harness = ...` argument.
        harness: SlotClass,
        /// Classification of the explicit `attributes = ...` argument.
        attributes: SlotClass,
        /// Whether the annotated function is `async`.
        is_async: bool,
        /// Whether the function already carries an executor attribute.
        has_executor_attr: bool,
        /// Whether the function already carries `#[gpui::test]`.
        has_gpui_test_attr: bool,
    },
    /// One `scenarios!` expansion.
    Scenarios {
        /// Classification of the explicit `harness = ...` argument.
        harness: SlotClass,
        /// Classification of the explicit `attributes = ...` argument.
        attributes: SlotClass,
        /// Whether the deprecated `runtime` alias was supplied.
        runtime_alias: bool,
    },
}

impl PolicyInputs {
    /// Which explicit path the harness slot holds.
    pub(crate) fn harness(self) -> SlotClass {
        match self {
            Self::Scenario { harness, .. } | Self::Scenarios { harness, .. } => harness,
        }
    }

    /// Which explicit path the attributes slot holds.
    pub(crate) fn attributes(self) -> SlotClass {
        match self {
            Self::Scenario { attributes, .. } | Self::Scenarios { attributes, .. } => attributes,
        }
    }

    /// The runtime the selected hint falls back to.
    pub(crate) fn attribute_runtime(self) -> RuntimeMode {
        if self.implies_tokio_runtime() {
            RuntimeMode::TokioCurrentThread
        } else {
            RuntimeMode::Sync
        }
    }

    /// Whether the caller asked for a Tokio runtime by any spelling.
    ///
    /// `#[scenario]` asks with an `async fn`; `scenarios!` asks with the
    /// deprecated alias.
    fn implies_tokio_runtime(self) -> bool {
        match self {
            Self::Scenario { is_async, .. } => is_async,
            Self::Scenarios { runtime_alias, .. } => runtime_alias,
        }
    }

    /// Whether the generated test function is `async`.
    ///
    /// `scenarios!` reproduces `resolve_effective_runtime`: the alias forces a
    /// synchronous body unless an explicit harness is also present.
    pub(crate) fn executes_async(self) -> bool {
        match self {
            Self::Scenario { is_async, .. } => is_async,
            Self::Scenarios {
                harness,
                runtime_alias,
                ..
            } => runtime_alias && !matches!(harness, SlotClass::Absent),
        }
    }

    /// Whether a user-written attribute already satisfies `rstest`'s rule.
    ///
    /// `scenarios!` generates its functions, so a user cannot write one there.
    pub(crate) fn has_executor_attr(self) -> bool {
        match self {
            Self::Scenario {
                has_executor_attr, ..
            } => has_executor_attr,
            Self::Scenarios { .. } => false,
        }
    }

    /// Whether the user already wrote `#[gpui::test]`.
    ///
    /// Distinct from [`Self::has_executor_attr`]: `#[test]` satisfies `rstest`
    /// but does not deduplicate a GPUI attribute, so it must not suppress a
    /// claim about the GPUI attribute.
    pub(crate) fn has_gpui_test_attr(self) -> bool {
        match self {
            Self::Scenario {
                has_gpui_test_attr, ..
            } => has_gpui_test_attr,
            Self::Scenarios { .. } => false,
        }
    }

    /// The same configuration with the harness slot replaced.
    pub(crate) fn with_harness(self, harness: SlotClass) -> Self {
        let mut parts = self.parts();
        parts.harness = harness;
        Self::from_parts(parts)
    }

    /// The same configuration with the attributes slot replaced.
    pub(crate) fn with_attributes(self, attributes: SlotClass) -> Self {
        let mut parts = self.parts();
        parts.attributes = attributes;
        Self::from_parts(parts)
    }

    /// The same scenario with the executor-attribute flag set.
    #[cfg(test)]
    pub(crate) fn with_executor_attr(self, has_executor_attr: bool) -> Self {
        let mut parts = self.parts();
        parts.has_executor_attr = has_executor_attr;
        Self::from_parts(parts)
    }

    /// The same configuration with the deprecated alias cleared.
    #[cfg(test)]
    pub(crate) fn without_runtime_alias(self) -> Self {
        let mut parts = self.parts();
        parts.shape = Shape::Scenarios {
            runtime_alias: false,
        };
        Self::from_parts(parts)
    }

    /// Every field this configuration carries, whichever shape it is.
    ///
    /// One rebuilder over this record replaces four near-identical `match`
    /// arms per accessor: each shape fills only the fields it has.
    fn parts(self) -> Parts {
        match self {
            Self::Scenario {
                harness,
                attributes,
                is_async,
                has_executor_attr,
                has_gpui_test_attr,
            } => Parts {
                harness,
                attributes,
                shape: Shape::Scenario { is_async },
                has_executor_attr,
                has_gpui_test_attr,
            },
            Self::Scenarios {
                harness,
                attributes,
                runtime_alias,
            } => Parts {
                harness,
                attributes,
                shape: Shape::Scenarios { runtime_alias },
                has_executor_attr: false,
                has_gpui_test_attr: false,
            },
        }
    }

    /// Rebuild the shape this record describes.
    fn from_parts(parts: Parts) -> Self {
        match parts.shape {
            Shape::Scenario { is_async } => Self::Scenario {
                harness: parts.harness,
                attributes: parts.attributes,
                is_async,
                has_executor_attr: parts.has_executor_attr,
                has_gpui_test_attr: parts.has_gpui_test_attr,
            },
            Shape::Scenarios { runtime_alias } => Self::Scenarios {
                harness: parts.harness,
                attributes: parts.attributes,
                runtime_alias,
            },
        }
    }
}

/// The shape-specific fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    /// A `#[scenario]` expansion.
    Scenario {
        /// Whether the annotated function is `async`.
        is_async: bool,
    },
    /// A `scenarios!` expansion.
    Scenarios {
        /// Whether the deprecated `runtime` alias was supplied.
        runtime_alias: bool,
    },
}

/// The fields of one configuration, shape included.
#[derive(Clone, Copy)]
struct Parts {
    /// Classification of the explicit `harness` argument.
    harness: SlotClass,
    /// Classification of the explicit `attributes` argument.
    attributes: SlotClass,
    /// The shape-specific fields.
    shape: Shape,
    /// Whether the function already carries an executor attribute.
    has_executor_attr: bool,
    /// Whether the function already carries `#[gpui::test]`.
    has_gpui_test_attr: bool,
}
