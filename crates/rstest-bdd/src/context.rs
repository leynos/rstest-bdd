//! Step execution context, fixture access, and typed return storage.
//! `StepContext` stores named fixture references and a type-indexed map for
//! values returned from step functions. Values must be `'static` so they can be
//! boxed. When exactly one fixture matches a returned type, that value replaces
//! the original fixture (last write wins); otherwise the fixture remains.

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
    pub(crate) fixtures: HashMap<&'static str, (&'a dyn Any, TypeId)>,
    values: HashMap<&'static str, Box<dyn Any>>,
}

impl<'a> StepContext<'a> {
    /// Insert a fixture reference by name.
    pub fn insert<T: Any>(&mut self, name: &'static str, value: &'a T) {
        self.fixtures.insert(name, (value, TypeId::of::<T>()));
    }

    /// Retrieve a fixture reference by name and type.
    ///
    /// Values returned from prior `#[when]` steps override fixtures of the same
    /// type when that type is unique among fixtures. This enables a functional
    /// style where step return values feed into later assertions without having
    /// to define ad-hoc fixtures.
    #[must_use]
    pub fn get<T: Any>(&self, name: &str) -> Option<&T> {
        if let Some(val) = self.values.get(name) {
            return val.downcast_ref::<T>();
        }
        self.fixtures.get(name)?.0.downcast_ref::<T>()
    }

    /// Insert a value produced by a prior step.
    /// The value overrides a fixture only if exactly one fixture has the same
    /// type; otherwise it is ignored to avoid ambiguity.
    pub fn insert_value(&mut self, value: Box<dyn Any>) {
        let ty = value.as_ref().type_id();
        let candidates: Vec<_> = self
            .fixtures
            .iter()
            .filter_map(|(&name, &(_, t))| (t == ty).then_some(name))
            .collect();
        if let [name] = candidates.as_slice() {
            self.values.insert(*name, value);
        }
    }
}
