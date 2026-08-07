//! Storage entries backing `StepContext` fixtures.
//!
//! Each [`FixtureEntry`] records how a fixture was inserted (shared
//! reference or owned `RefCell`) together with its `TypeId`, and implements
//! the guard-based borrow operations over that storage (ADR-012). The
//! [`borrow_cell`] / [`borrow_cell_mut`] primitives are the single home for
//! "borrow a type-erased cell and downcast" — both owned fixtures and
//! step-returned override values borrow through them. The shared/mutable
//! pair is irreducible: `std::cell` exposes shared and mutable borrowing
//! through the distinct `Ref`/`RefMut` types.

use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};

use super::error::FixtureBorrowError;
use super::guards::{FixtureRef, FixtureRefMut};

pub(super) struct FixtureEntry<'a> {
    kind: FixtureKind<'a>,
    pub(super) type_id: TypeId,
}

enum FixtureKind<'a> {
    Shared(&'a dyn Any),
    Mutable(&'a RefCell<Box<dyn Any>>),
}

impl<'a> FixtureEntry<'a> {
    pub(super) fn shared<T: Any>(value: &'a T) -> Self {
        Self {
            kind: FixtureKind::Shared(value),
            type_id: TypeId::of::<T>(),
        }
    }

    pub(super) fn owned<T: Any>(cell: &'a RefCell<Box<dyn Any>>) -> Self {
        Self {
            kind: FixtureKind::Mutable(cell),
            type_id: TypeId::of::<T>(),
        }
    }

    /// Return the shared reference backing this entry, when it was inserted
    /// by shared reference. Owned (mutable) entries return `None`.
    pub(super) fn shared_value(&self) -> Option<&'a dyn Any> {
        match self.kind {
            FixtureKind::Shared(value) => Some(value),
            FixtureKind::Mutable(_) => None,
        }
    }

    pub(super) fn try_borrow<T: Any>(
        &self,
        name: &str,
    ) -> Result<FixtureRef<'_, T>, FixtureBorrowError> {
        self.check_type_id::<T>(name)?;
        match self.kind {
            FixtureKind::Shared(value) => value
                .downcast_ref::<T>()
                .map(FixtureRef::shared)
                .ok_or_else(|| FixtureBorrowError::type_mismatch(name)),
            FixtureKind::Mutable(cell) => borrow_cell(cell, name),
        }
    }

    pub(super) fn try_borrow_mut<T: Any>(
        &self,
        name: &str,
    ) -> Result<FixtureRefMut<'_, T>, FixtureBorrowError> {
        self.check_type_id::<T>(name)?;
        match self.kind {
            FixtureKind::Shared(_) => Err(FixtureBorrowError::not_mutable(name)),
            FixtureKind::Mutable(cell) => borrow_cell_mut(cell, name),
        }
    }

    fn check_type_id<T: Any>(&self, name: &str) -> Result<(), FixtureBorrowError> {
        if self.type_id == TypeId::of::<T>() {
            Ok(())
        } else {
            Err(FixtureBorrowError::type_mismatch(name))
        }
    }
}

/// Borrow a type-erased cell immutably and downcast its contents to `T`.
///
/// Canonical shared-borrow primitive for `RefCell<Box<dyn Any>>` storage:
/// owned fixtures and step-returned overrides both resolve through it. A
/// live mutable guard yields [`FixtureBorrowError::AlreadyBorrowed`]; a
/// failed downcast yields [`FixtureBorrowError::TypeMismatch`].
pub(super) fn borrow_cell<'b, T: Any>(
    cell: &'b RefCell<Box<dyn Any>>,
    name: &str,
) -> Result<FixtureRef<'b, T>, FixtureBorrowError> {
    let guard = cell
        .try_borrow()
        .map_err(|_| FixtureBorrowError::already_borrowed(name))?;
    Ref::filter_map(guard, |boxed| boxed.downcast_ref::<T>())
        .map(FixtureRef::borrowed)
        .map_err(|_| FixtureBorrowError::type_mismatch(name))
}

/// Borrow a type-erased cell mutably and downcast its contents to `T`.
///
/// Mutable counterpart of [`borrow_cell`]; any live guard for the same cell
/// yields [`FixtureBorrowError::AlreadyBorrowed`].
pub(super) fn borrow_cell_mut<'b, T: Any>(
    cell: &'b RefCell<Box<dyn Any>>,
    name: &str,
) -> Result<FixtureRefMut<'b, T>, FixtureBorrowError> {
    let guard = cell
        .try_borrow_mut()
        .map_err(|_| FixtureBorrowError::already_borrowed(name))?;
    RefMut::filter_map(guard, |boxed| boxed.downcast_mut::<T>())
        .map(FixtureRefMut::borrowed)
        .map_err(|_| FixtureBorrowError::type_mismatch(name))
}
