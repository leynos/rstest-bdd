//! ADR-007 harness-context accessors.
//!
//! The five methods here hard-code the reserved
//! [`RSTEST_BDD_HARNESS_CONTEXT_FIXTURE`] key over the generic fixture API.
//! They are deliberate API surface, not dead code: the insert side is emitted
//! by macro-generated harness scenarios, and the borrow side is the supported
//! typed-extraction surface for adapters and step code.
//!
//! They were moved out of `context/mod.rs` unchanged, to keep that module under
//! the workspace's 400-line cap. The original in-place note said not to add
//! further wrappers here for new generic access patterns unless generated code
//! or the documented step-authoring path needs them; moving the block is how
//! that closed boundary stays visible instead of becoming one more region of a
//! file that has no room left.
//!
//! The `impl` block is separate from the one in `mod.rs`, which is why the
//! methods are `pub` here rather than `pub(crate)`: they are the same inherent
//! methods either way, and splitting the block must not change their
//! visibility.

use std::{any::Any, cell::RefCell};

use super::{RSTEST_BDD_HARNESS_CONTEXT_FIXTURE, StepContext};
use crate::context::guards::{FixtureRef, FixtureRefMut};

impl<'a> StepContext<'a> {
    /// Insert harness-provided context using the reserved fixture key.
    ///
    /// Part of the ADR-007 harness-context contract. This shared-reference
    /// variant exists for adapters that keep ownership of their context;
    /// macro-generated code uses
    /// [`insert_owned_harness_context`](Self::insert_owned_harness_context).
    pub fn insert_harness_context<T: Any>(&mut self, context: &'a T) {
        self.insert(RSTEST_BDD_HARNESS_CONTEXT_FIXTURE, context);
    }

    /// Insert owned harness-provided context using the reserved fixture key.
    ///
    /// Part of the ADR-007 harness-context contract. This is the variant
    /// emitted by macro-generated harness scenarios (see
    /// `codegen/scenario/runtime/harness.rs` in `rstest-bdd-macros`), which
    /// wrap the adapter's `HarnessAdapter::Context` in an owned cell so
    /// steps can borrow it mutably.
    pub fn insert_owned_harness_context<T: Any>(&mut self, cell: &'a RefCell<Box<dyn Any>>) {
        self.insert_owned::<T>(RSTEST_BDD_HARNESS_CONTEXT_FIXTURE, cell);
    }

    /// Retrieve harness-provided context by type when it is stored by shared reference.
    ///
    /// This delegates to [`get`](Self::get), which returns `None` for mutable
    /// (`insert_owned`) fixture entries. The macro-generated harness path
    /// currently inserts context with
    /// [`insert_owned_harness_context`](Self::insert_owned_harness_context)
    /// under [`RSTEST_BDD_HARNESS_CONTEXT_FIXTURE`], so callers should use
    /// [`borrow_harness_context`](Self::borrow_harness_context) for that path.
    #[must_use]
    pub fn harness_context<T: Any>(&'a self) -> Option<&'a T> {
        self.get(RSTEST_BDD_HARNESS_CONTEXT_FIXTURE)
    }

    /// Borrow harness-provided context by type.
    ///
    /// Part of the ADR-007 harness-context contract: the supported typed
    /// read accessor for context stored by
    /// [`insert_owned_harness_context`](Self::insert_owned_harness_context).
    #[must_use]
    pub fn borrow_harness_context<'b, T: Any>(&'b self) -> Option<FixtureRef<'b, T>>
    where
        'a: 'b,
    {
        self.borrow_ref(RSTEST_BDD_HARNESS_CONTEXT_FIXTURE)
    }

    /// Borrow harness-provided context mutably by type.
    ///
    /// Part of the ADR-007 harness-context contract: the supported typed
    /// mutable accessor for context stored by
    /// [`insert_owned_harness_context`](Self::insert_owned_harness_context).
    #[must_use]
    pub fn borrow_harness_context_mut<'b, T: Any>(&'b self) -> Option<FixtureRefMut<'b, T>>
    where
        'a: 'b,
    {
        self.borrow_mut(RSTEST_BDD_HARNESS_CONTEXT_FIXTURE)
    }
}
