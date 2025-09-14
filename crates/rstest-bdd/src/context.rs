//! Step execution context and fixture access.
//! This module provides `StepContext`, a simple type-indexed store that the
//! scenario runner uses to pass fixtures into step functions.

use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Context passed to step functions containing references to requested fixtures.
///
/// This is constructed by the `#[scenario]` macro for each step invocation.
///
/// # Examples
///
/// ```
/// use rstest_bdd::StepContext;
///
/// let mut ctx = StepContext::default();
/// let value = 42;
/// ctx.insert("my_fixture", &value);
///
/// let retrieved: Option<&i32> = ctx.get("my_fixture");
/// assert_eq!(retrieved, Some(&42));
/// ```
#[derive(Default)]
pub struct StepContext<'a> {
    pub(crate) fixtures: HashMap<&'static str, &'a dyn Any>,
    values: HashMap<TypeId, Box<dyn Any>>,
}

impl<'a> StepContext<'a> {
    /// Insert a fixture reference by name.
    pub fn insert<T: Any>(&mut self, name: &'static str, value: &'a T) {
        self.fixtures.insert(name, value);
    }

    /// Retrieve a fixture reference by name and type.
    ///
    /// Values returned from prior `#[when]` steps override fixtures of the same
    /// type. This enables a functional style where step return values feed into
    /// later assertions without having to define ad-hoc fixtures.
    #[must_use]
    pub fn get<T: Any>(&self, name: &str) -> Option<&T> {
        if let Some(val) = self.values.get(&TypeId::of::<T>()) {
            return val.downcast_ref::<T>();
        }
        self.fixtures.get(name)?.downcast_ref::<T>()
    }

    /// Insert a value produced by a prior step.
    pub fn insert_value(&mut self, value: Box<dyn Any>) {
        let ty = value.as_ref().type_id();
        self.values.insert(ty, value);
    }
}
