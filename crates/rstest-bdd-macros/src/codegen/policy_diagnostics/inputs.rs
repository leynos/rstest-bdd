//! The classified user arguments for one expansion boundary.
//!
//! A configuration is one of two shapes because the two macros expose
//! different inputs: `#[scenario]` knows its signature and attributes,
//! `scenarios!` knows only whether the deprecated runtime alias was supplied.

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
        has_user_test_attr: bool,
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
    /// deprecated alias. Reading the question once, rather than nesting a
    /// conditional inside each shape, keeps the two spellings visibly the same
    /// policy.
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
    pub(crate) fn has_user_test_attr(self) -> bool {
        match self {
            Self::Scenario {
                has_user_test_attr, ..
            } => has_user_test_attr,
            Self::Scenarios { .. } => false,
        }
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
                has_user_test_attr,
            } => Parts {
                harness,
                attributes,
                shape: Shape::Scenario {
                    is_async,
                    has_user_test_attr,
                },
            },
            Self::Scenarios {
                harness,
                attributes,
                runtime_alias,
            } => Parts {
                harness,
                attributes,
                shape: Shape::Scenarios { runtime_alias },
            },
        }
    }

    /// The same configuration with the harness slot replaced.
    pub(crate) fn with_harness(self, harness: SlotClass) -> Self {
        Self::from_parts(self.parts().with_harness(harness))
    }

    /// The same configuration with the attributes slot replaced.
    pub(crate) fn with_attributes(self, attributes: SlotClass) -> Self {
        Self::from_parts(self.parts().with_attributes(attributes))
    }

    /// The same scenario with the user-attribute flag set.
    #[cfg(test)]
    pub(crate) fn with_user_test_attr(self, value: bool) -> Self {
        Self::from_parts(self.parts().with(ShapeEdit::UserTestAttr(value)))
    }

    /// The same configuration with the deprecated alias cleared.
    #[cfg(test)]
    pub(crate) fn without_runtime_alias(self) -> Self {
        Self::from_parts(self.parts().with(ShapeEdit::RuntimeAlias(false)))
    }

    /// Rebuild the shape this record describes.
    fn from_parts(parts: Parts) -> Self {
        match parts.shape {
            Shape::Scenario {
                is_async,
                has_user_test_attr,
            } => Self::Scenario {
                harness: parts.harness,
                attributes: parts.attributes,
                is_async,
                has_user_test_attr,
            },
            Shape::Scenarios { runtime_alias } => Self::Scenarios {
                harness: parts.harness,
                attributes: parts.attributes,
                runtime_alias,
            },
        }
    }
}

/// The shape-specific fields, so a rebuilder never has to match twice.
#[derive(Clone, Copy)]
enum Shape {
    /// A `#[scenario]` expansion.
    Scenario {
        /// Whether the annotated function is `async`.
        is_async: bool,
        /// Whether the function already carries an executor attribute.
        has_user_test_attr: bool,
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
}

impl Parts {
    /// The same parts with the harness slot replaced.
    fn with_harness(mut self, harness: SlotClass) -> Self {
        self.harness = harness;
        self
    }

    /// The same parts with the attributes slot replaced.
    fn with_attributes(mut self, attributes: SlotClass) -> Self {
        self.attributes = attributes;
        self
    }

    /// The same parts with a test-only shape field replaced.
    ///
    /// `edit` names the field and its new value; a shape that does not carry
    /// that field comes back unchanged, which is why one setter serves both
    /// test-only accessors.
    #[cfg(test)]
    fn with(self, edit: ShapeEdit) -> Self {
        match (self.shape, edit) {
            (Shape::Scenario { is_async, .. }, ShapeEdit::UserTestAttr(has_user_test_attr)) => {
                Self {
                    shape: Shape::Scenario {
                        is_async,
                        has_user_test_attr,
                    },
                    ..self
                }
            }
            (Shape::Scenarios { .. }, ShapeEdit::RuntimeAlias(runtime_alias)) => Self {
                shape: Shape::Scenarios { runtime_alias },
                ..self
            },
            (shape, _) => Self { shape, ..self },
        }
    }
}

/// One test-only shape-field replacement.
#[cfg(test)]
#[derive(Clone, Copy)]
enum ShapeEdit {
    /// Set the user-attribute flag of a scenario.
    UserTestAttr(bool),
    /// Set the deprecated-alias flag of a `scenarios!` expansion.
    RuntimeAlias(bool),
}
