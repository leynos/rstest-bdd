//! Step execution context, fixture access, and step return overrides.
//! `StepContext` stores named fixture references plus a map of last-seen step
//! results keyed by fixture name. Returned values must be `'static` so they can
//! be boxed. When exactly one fixture matches a returned type, its name records
//! the override (last write wins); ambiguous matches leave fixtures untouched.

use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;

/// Context passed to step functions containing references to requested fixtures.
///
/// This is constructed by the `#[scenario]` macro for each step invocation. Use
/// [`insert_owned`](Self::insert_owned) when a fixture should be shared
/// mutably across steps; step functions may then request `&mut T` and mutate
/// world state without resorting to interior mutability wrappers.
///
/// # Examples
///
/// ```
/// use rstest_bdd::StepContext;
///
/// let mut ctx = StepContext::default();
/// let value = 42;
/// ctx.insert("my_fixture", &value);
/// let owned = std::cell::RefCell::new(Box::new(String::from("hi")));
/// ctx.insert_owned("owned", &owned);
///
/// let retrieved: Option<&i32> = ctx.get("my_fixture");
/// assert_eq!(retrieved, Some(&42));
/// {
///     let mut suffix = ctx.borrow_mut::<String>("owned").expect("owned fixture");
///     suffix.value_mut().push('!');
/// }
/// assert_eq!(*owned.into_inner(), "hi!");
/// ```
#[derive(Default)]
pub struct StepContext<'a> {
    fixtures: HashMap<&'static str, FixtureEntry<'a>>,
    values: HashMap<&'static str, Box<dyn Any>>,
}

struct FixtureEntry<'a> {
    kind: FixtureKind<'a>,
    type_id: TypeId,
}

enum FixtureKind<'a> {
    Shared(&'a dyn Any),
    Mutable(&'a dyn Any), // stores &RefCell<Box<T>>
}

impl<'a> StepContext<'a> {
    /// Insert a fixture reference by name.
    pub fn insert<T: Any>(&mut self, name: &'static str, value: &'a T) {
        self.fixtures.insert(name, FixtureEntry::shared(value));
    }

    /// Insert a fixture backed by a `RefCell<Box<T>>`, enabling mutable borrows.
    pub fn insert_owned<T: Any>(&mut self, name: &'static str, cell: &'a RefCell<Box<T>>) {
        self.fixtures.insert(name, FixtureEntry::owned(cell));
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
        match self.fixtures.get(name)?.kind {
            FixtureKind::Shared(value) => value.downcast_ref::<T>(),
            FixtureKind::Mutable(_) => None,
        }
    }

    /// Borrow a fixture by name, keeping the guard alive until dropped.
    pub fn borrow_ref<T: Any>(&self, name: &str) -> Option<FixtureRef<'_, T>> {
        if let Some(val) = self.values.get(name) {
            return val.downcast_ref::<T>().map(FixtureRef::Shared);
        }
        self.fixtures.get(name)?.borrow_ref::<T>()
    }

    /// Borrow a fixture mutably by name.
    ///
    /// # Panics
    ///
    /// The underlying fixtures use `RefCell` for interior mutability. Attempting
    /// to borrow the same fixture mutably while an existing mutable guard is
    /// alive will panic via `RefCell::borrow_mut`. Callers must drop guards
    /// before requesting another mutable borrow of the same fixture.
    pub fn borrow_mut<T: Any>(&'a mut self, name: &str) -> Option<FixtureRefMut<'a, T>> {
        if let Some(val) = self.values.get_mut(name) {
            return val.downcast_mut::<T>().map(FixtureRefMut::Override);
        }
        self.fixtures.get(name)?.borrow_mut::<T>()
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
            .filter_map(|(&name, entry)| (entry.type_id == ty).then_some(name));
        let name = matches.next()?;
        if matches.next().is_some() {
            let message =
                crate::localization::message_with_args("step-context-ambiguous-override", |args| {
                    args.set("type_id", format!("{ty:?}"));
                });
            log::warn!("{message}");
            #[expect(
                clippy::print_stderr,
                reason = "surface ambiguous overrides when logging is disabled"
            )]
            if !log::log_enabled!(log::Level::Warn) {
                eprintln!("{message}");
            }
            return None;
        }
        self.values.insert(name, value)
    }
}

impl<'a> FixtureEntry<'a> {
    fn shared<T: Any>(value: &'a T) -> Self {
        Self {
            kind: FixtureKind::Shared(value),
            type_id: TypeId::of::<T>(),
        }
    }

    fn owned<T: Any>(cell: &'a RefCell<Box<T>>) -> Self {
        Self {
            kind: FixtureKind::Mutable(cell),
            type_id: TypeId::of::<T>(),
        }
    }

    /// Helper to borrow from a mutable fixture, delegating the borrow operation
    /// to the provided closure.
    fn borrow_mutable<T: Any, R>(
        &self,
        borrow_fn: impl FnOnce(&'a RefCell<Box<T>>) -> R,
    ) -> Option<R> {
        if self.type_id != TypeId::of::<T>() {
            return None;
        }
        match self.kind {
            FixtureKind::Mutable(cell_any) => {
                let cell = cell_any.downcast_ref::<RefCell<Box<T>>>()?;
                Some(borrow_fn(cell))
            }
            FixtureKind::Shared(_) => None,
        }
    }

    fn borrow_ref<T: Any>(&self) -> Option<FixtureRef<'_, T>> {
        match self.kind {
            FixtureKind::Shared(value) => value.downcast_ref::<T>().map(FixtureRef::Shared),
            FixtureKind::Mutable(_) => self.borrow_mutable(|cell| {
                let guard = cell.borrow();
                let mapped = Ref::map(guard, Box::as_ref);
                FixtureRef::Borrowed(mapped)
            }),
        }
    }

    fn borrow_mut<T: Any>(&self) -> Option<FixtureRefMut<'_, T>> {
        self.borrow_mutable(|cell| {
            let guard = cell.borrow_mut();
            let mapped = RefMut::map(guard, Box::as_mut);
            FixtureRefMut::Borrowed(mapped)
        })
    }
}
/// Borrowed fixture reference that keeps any underlying `RefCell` borrow alive
/// for the duration of a step.
pub enum FixtureRef<'a, T> {
    /// Reference bound directly to a shared fixture.
    Shared(&'a T),
    /// Borrow guard taken from a backing `RefCell`.
    Borrowed(Ref<'a, T>),
}

impl<T> FixtureRef<'_, T> {
    /// Access the borrowed value as an immutable reference.
    #[must_use]
    pub fn value(&self) -> &T {
        match self {
            Self::Shared(value) => value,
            Self::Borrowed(guard) => guard,
        }
    }
}

impl<T> AsRef<T> for FixtureRef<'_, T> {
    fn as_ref(&self) -> &T {
        self.value()
    }
}

/// Borrowed mutable fixture reference tied to the lifetime of the step borrow.
pub enum FixtureRefMut<'a, T> {
    /// Mutable reference produced by a prior step override.
    Override(&'a mut T),
    /// Borrow guard obtained from the underlying `RefCell`.
    Borrowed(RefMut<'a, T>),
}

impl<T> FixtureRefMut<'_, T> {
    /// Access the borrowed value mutably.
    #[must_use]
    pub fn value_mut(&mut self) -> &mut T {
        match self {
            Self::Override(value) => value,
            Self::Borrowed(guard) => guard,
        }
    }
}

impl<T> AsMut<T> for FixtureRefMut<'_, T> {
    fn as_mut(&mut self) -> &mut T {
        self.value_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    struct NoopLogger;

    impl log::Log for NoopLogger {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            true
        }
        fn log(&self, _: &log::Record<'_>) {}
        fn flush(&self) {}
    }

    static LOGGER: NoopLogger = NoopLogger;
    static INIT_LOGGER: Once = Once::new();

    fn ensure_logger() {
        INIT_LOGGER.call_once(|| {
            let _ = log::set_logger(&LOGGER);
            log::set_max_level(log::LevelFilter::Warn);
        });
    }

    #[test]
    fn borrow_mut_returns_mutable_fixture() {
        let cell = RefCell::new(Box::new(String::from("seed")));
        let mut ctx = StepContext::default();
        ctx.insert_owned("text", &cell);

        {
            let Some(mut value) = ctx.borrow_mut::<String>("text") else {
                panic!("mutable fixture should exist");
            };
            value.as_mut().push_str("ing");
        }
        assert_eq!(*cell.into_inner(), "seeding");
    }

    #[test]
    fn borrow_mut_returns_none_for_shared_fixture() {
        let fixture = 5;
        let mut ctx = StepContext::default();
        ctx.insert("number", &fixture);
        assert!(ctx.borrow_mut::<i32>("number").is_none());
    }

    #[test]
    #[expect(clippy::expect_used, reason = "tests require explicit panic messages")]
    fn insert_value_overrides_unique_fixture() {
        ensure_logger();
        let fixture = 1u32;
        let mut ctx = StepContext::default();
        ctx.insert("number", &fixture);

        let first = ctx.insert_value(Box::new(5u32));
        assert!(
            first.is_none(),
            "first override should have no previous value"
        );

        let second = ctx
            .insert_value(Box::new(7u32))
            .expect("expected previous override to be returned");
        let previous = second
            .downcast::<u32>()
            .expect("override should downcast to u32");
        assert_eq!(*previous, 5);

        let current = ctx
            .get::<u32>("number")
            .expect("retrieved fixture should exist");
        assert_eq!(*current, 7);
    }

    #[test]
    fn insert_value_returns_none_when_type_ambiguous() {
        ensure_logger();
        let first = 1u32;
        let second = 2u32;
        let mut ctx = StepContext::default();
        ctx.insert("one", &first);
        ctx.insert("two", &second);

        let result = ctx.insert_value(Box::new(5u32));
        assert!(result.is_none(), "ambiguous overrides must be ignored");
        assert_eq!(ctx.get::<u32>("one"), Some(&1));
        assert_eq!(ctx.get::<u32>("two"), Some(&2));
    }

    #[test]
    fn insert_value_returns_none_when_type_missing() {
        ensure_logger();
        let text = "fixture";
        let mut ctx = StepContext::default();
        ctx.insert("text", &text);

        let result = ctx.insert_value(Box::new(5u32));
        assert!(result.is_none(), "missing fixture should skip override");
        assert!(ctx.get::<u32>("text").is_none());
    }
}
