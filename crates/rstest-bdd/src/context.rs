//! Step execution context, fixture access, and step return overrides.
//! `StepContext` stores named fixture references plus a map of last-seen step
//! results keyed by fixture name. Returned values must be `'static` so they can
//! be boxed. When exactly one fixture matches a returned type, its name records
//! the override (last write wins); ambiguous matches leave fixtures untouched.

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
    pub fn get<T: Any>(&'a self, name: &str) -> Option<&'a T> {
        if let Some(val) = self.values.get(name) {
            return val.downcast_ref::<T>();
        }
        self.fixtures.get(name)?.0.downcast_ref::<T>()
    }

    /// Insert a value produced by a prior step.
    /// The value overrides a fixture only if exactly one fixture has the same
    /// type; otherwise it is ignored to avoid ambiguity.
    ///
    /// Returns the previous override for that fixture when one existed.
    pub fn insert_value(&mut self, value: Box<dyn Any>) -> Option<Box<dyn Any>> {
        let ty = value.as_ref().type_id();
        let mut matches = self
            .fixtures
            .iter()
            .filter_map(|(&name, &(_, t))| (t == ty).then_some(name));
        let name = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        self.values.insert(name, value)
    }
}
