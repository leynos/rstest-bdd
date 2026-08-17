//! Step execution context, fixture access, and step return overrides.
//!
//! `StepContext` stores named fixture references plus a map of last-seen step
//! results keyed by fixture name. Returned values must be `'static` so they
//! can be boxed. When exactly one fixture matches a returned type, its name
//! records the override (last write wins); ambiguous matches leave fixtures
//! untouched.
//!
//! Borrowing is guard-based (ADR-012): borrow methods take `&self`, so
//! guards for **distinct** fixtures may be held concurrently — including
//! multiple mutable guards — without tripping `E0499`/`E0502`. Conflicting
//! borrows of the **same** fixture surface as
//! [`FixtureBorrowError::AlreadyBorrowed`] from the `try_*` methods rather
//! than panicking.
//!
//! # World lifecycle contract
//!
//! A fresh `StepContext` is constructed for every scenario run by the
//! generated test, and the owned fixture cells backing it live in that test
//! function's body. They are dropped when the scenario finishes — whether it
//! passes, fails (unwinds), or is skipped — so no fixture state leaks across
//! scenario boundaries and no caller-side reset discipline is required.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;

mod entry;
mod error;
mod guards;
mod insert_outcome;
#[cfg(test)]
mod tests;

use entry::FixtureEntry;
pub use error::FixtureBorrowError;
pub use guards::{FixtureRef, FixtureRefMut};
pub use insert_outcome::InsertOutcome;

/// Reserved fixture key used for harness-provided context.
///
/// Harness-backed scenarios insert `HarnessAdapter::Context` into
/// [`StepContext`] under this key. Step functions can then request the context
/// fixture by naming a parameter `rstest_bdd_harness_context` or by using
/// `#[from(rstest_bdd_harness_context)]`.
pub const RSTEST_BDD_HARNESS_CONTEXT_FIXTURE: &str = rstest_bdd_policy::HARNESS_CONTEXT_FIXTURE;

/// Context passed to step functions containing references to requested fixtures.
///
/// This is constructed by the `#[scenario]` macro for each step invocation. Use
/// [`insert_owned`](Self::insert_owned) when a fixture should be shared
/// mutably across steps; step functions may then request `&mut T` and mutate
/// world state without resorting to interior mutability wrappers.
///
/// Distinct fixtures can be borrowed mutably at the same time:
///
/// ```
/// use rstest_bdd::StepContext;
///
/// let mut ctx = StepContext::default();
/// let first = StepContext::owned_cell(1_u32);
/// let second = StepContext::owned_cell(String::from("hi"));
/// ctx.insert_owned::<u32>("first", &first);
/// ctx.insert_owned::<String>("second", &second);
///
/// let mut a = ctx.try_borrow_mut::<u32>("first").expect("first fixture");
/// let mut b = ctx.try_borrow_mut::<String>("second").expect("second fixture");
/// *a += 1;
/// b.push('!');
/// assert_eq!(*a, 2);
/// assert_eq!(b.as_str(), "hi!");
/// ```
///
/// # Examples
///
/// ```
/// use rstest_bdd::StepContext;
///
/// let mut ctx = StepContext::default();
/// let value = 42;
/// ctx.insert("my_fixture", &value);
/// let owned = StepContext::owned_cell(String::from("hi"));
/// ctx.insert_owned::<String>("owned", &owned);
///
/// let retrieved: Option<&i32> = ctx.get("my_fixture");
/// assert_eq!(retrieved, Some(&42));
/// {
///     let mut suffix = ctx.borrow_mut::<String>("owned").expect("owned fixture");
///     suffix.value_mut().push('!');
/// }
/// drop(ctx);
/// let owned = owned.into_inner();
/// let owned: String = *owned
///     .downcast::<String>()
///     .expect("fixture should downcast to String");
/// assert_eq!(owned, "hi!");
/// ```
#[derive(Default)]
pub struct StepContext<'a> {
    fixtures: HashMap<&'static str, FixtureEntry<'a>>,
    values: HashMap<&'static str, RefCell<Box<dyn Any>>>,
}
impl<'a> StepContext<'a> {
    /// Create an owned fixture cell for use with [`insert_owned`](Self::insert_owned).
    ///
    /// This helper boxes the provided value and erases its concrete type so it
    /// can back a mutable fixture. Callers must retain the returned cell for as
    /// long as the context references it.
    #[must_use]
    pub fn owned_cell<T: Any>(value: T) -> RefCell<Box<dyn Any>> {
        RefCell::new(Box::new(value))
    }

    /// Insert a fixture reference by name.
    pub fn insert<T: Any>(&mut self, name: &'static str, value: &'a T) {
        self.fixtures.insert(name, FixtureEntry::shared(value));
    }

    /// Insert a fixture backed by a `RefCell<Box<dyn Any>>`, enabling mutable borrows.
    ///
    /// A runtime type check ensures the stored value matches the requested `T`
    /// so mismatches are surfaced immediately instead of silently failing at
    /// borrow time.
    ///
    /// # Panics
    ///
    /// Panics when the provided cell does not currently store a value of type
    /// `T`, because continuing would render the fixture un-borrowable at run
    /// time.
    pub fn insert_owned<T: Any>(&mut self, name: &'static str, cell: &'a RefCell<Box<dyn Any>>) {
        let guard = cell.borrow();
        let actual = guard.as_ref().type_id();
        assert!(
            actual == TypeId::of::<T>(),
            "insert_owned: stored value type ({actual:?}) does not match requested {:?} for fixture '{name}'",
            TypeId::of::<T>()
        );
        self.fixtures.insert(name, FixtureEntry::owned::<T>(cell));
    }

    // ------------------------------------------------------------------
    // Harness-context wrappers (ADR-007).
    //
    // These thin wrappers hard-code the reserved
    // `RSTEST_BDD_HARNESS_CONTEXT_FIXTURE` key over the generic fixture API.
    // They are deliberate API surface, not dead code: the insert side is
    // emitted by macro-generated harness scenarios, and the borrow side is
    // the supported typed-extraction surface for adapters and step code.
    // Do not add further wrappers here for new generic access patterns
    // unless generated code or the documented step-authoring path needs
    // them; see ADR-007 ("Phase 2 convention: StepContext mapping").
    // ------------------------------------------------------------------

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

    /// Retrieve a shared fixture reference by name and type.
    ///
    /// Only fixtures inserted with [`insert`](Self::insert) are served:
    /// mutable (`insert_owned`) fixtures and step-returned override values
    /// live behind interior mutability and must be accessed through the
    /// guard-based [`borrow_ref`](Self::borrow_ref) /
    /// [`try_borrow`](Self::try_borrow) API instead.
    #[must_use]
    pub fn get<T: Any>(&'a self, name: &str) -> Option<&'a T> {
        self.fixtures.get(name)?.shared_value()?.downcast_ref::<T>()
    }

    /// Borrow a fixture by name, keeping the guard alive until dropped.
    ///
    /// Step-returned override values take precedence over fixtures of the
    /// same name. Returns `None` when the fixture is missing, stores a
    /// different type, or is currently borrowed mutably; use
    /// [`try_borrow`](Self::try_borrow) to distinguish these cases.
    #[must_use]
    pub fn borrow_ref<'b, T: Any>(&'b self, name: &str) -> Option<FixtureRef<'b, T>>
    where
        'a: 'b,
    {
        self.try_borrow(name).ok()
    }

    /// Borrow a fixture mutably by name.
    ///
    /// Takes `&self`, so distinct mutable fixture guards may coexist.
    /// Step-returned overrides take precedence and remain mutable even when the
    /// underlying fixture was inserted by shared reference. Returns `None` when
    /// missing, wrong type, already borrowed, or shared without an applicable
    /// override; use [`try_borrow_mut`](Self::try_borrow_mut) for details.
    #[must_use]
    pub fn borrow_mut<'b, T: Any>(&'b self, name: &str) -> Option<FixtureRefMut<'b, T>>
    where
        'a: 'b,
    {
        self.try_borrow_mut(name).ok()
    }

    /// Returns an iterator over the names of all available fixtures.
    ///
    /// This method is useful for diagnostic purposes, such as generating error
    /// messages that list which fixtures are available when a required fixture
    /// is missing.
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
    /// let names: Vec<_> = ctx.available_fixtures().collect();
    /// assert!(names.contains(&"my_fixture"));
    /// ```
    pub fn available_fixtures(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.fixtures.keys().copied()
    }

    /// Insert a prior step's value, overriding a fixture only when exactly one
    /// fixture has the same type; otherwise the value is ignored.
    ///
    /// [`InsertOutcome`] distinguishes the three results a bare `Option`
    /// previously conflated: a recorded override (carrying any displaced
    /// value), no matching fixture type, and an ambiguous match. Ambiguity also
    /// logs a warning, with an `eprintln!` fallback when logging is disabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{InsertOutcome, StepContext};
    ///
    /// let fixture = 1_u32;
    /// let mut ctx = StepContext::default();
    /// ctx.insert("number", &fixture);
    ///
    /// let first = ctx.insert_value(Box::new(5_u32));
    /// assert!(matches!(first, InsertOutcome::Inserted(None)));
    /// let second = ctx.insert_value(Box::new(7_u32));
    /// assert!(matches!(second, InsertOutcome::Inserted(Some(_))));
    ///
    /// let unmatched = ctx.insert_value(Box::new("text"));
    /// assert!(matches!(unmatched, InsertOutcome::NoMatch));
    /// ```
    pub fn insert_value(&mut self, value: Box<dyn Any>) -> InsertOutcome {
        let ty = value.as_ref().type_id();
        let mut matches = self
            .fixtures
            .iter()
            .filter_map(|(&name, entry)| (entry.type_id == ty).then_some(name));
        let Some(name) = matches.next() else {
            return InsertOutcome::NoMatch;
        };
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
            if warn_logging_is_disabled() {
                eprintln!("{message}");
            }
            return InsertOutcome::AmbiguousIgnored;
        }
        InsertOutcome::Inserted(
            self.values
                .insert(name, RefCell::new(value))
                .map(RefCell::into_inner),
        )
    }

    /// Borrow a fixture by name, reporting the failure reason on error.
    ///
    /// Step-returned override values take precedence over fixtures of the
    /// same name.
    ///
    /// # Errors
    ///
    /// - [`FixtureBorrowError::NotFound`] when no fixture or override is
    ///   registered under `name`.
    /// - [`FixtureBorrowError::TypeMismatch`] when the entry stores a
    ///   different type than `T`.
    /// - [`FixtureBorrowError::AlreadyBorrowed`] when a mutable guard for
    ///   the same fixture is alive.
    pub fn try_borrow<'b, T: Any>(
        &'b self,
        name: &str,
    ) -> Result<FixtureRef<'b, T>, FixtureBorrowError>
    where
        'a: 'b,
    {
        match self.values.get(name) {
            Some(cell) => entry::borrow_cell(cell, name),
            None => self.fixture_entry(name)?.try_borrow::<T>(name),
        }
    }

    /// Borrow a fixture mutably by name, reporting the failure reason on error.
    ///
    /// Takes `&self`, so distinct mutable fixtures can be borrowed concurrently;
    /// only same-fixture conflicts fail. Step-returned overrides take precedence
    /// and remain mutable even when their fixture was inserted with [`insert`](Self::insert).
    ///
    /// # Errors
    ///
    /// - [`FixtureBorrowError::NotFound`] when neither fixture nor override exists under `name`.
    /// - [`FixtureBorrowError::TypeMismatch`] when the entry stores a different type than `T`.
    /// - [`FixtureBorrowError::AlreadyBorrowed`] when any same-fixture guard is alive.
    /// - [`FixtureBorrowError::NotMutable`] when a shared fixture has no applicable
    ///   step-returned override.
    pub fn try_borrow_mut<'b, T: Any>(
        &'b self,
        name: &str,
    ) -> Result<FixtureRefMut<'b, T>, FixtureBorrowError>
    where
        'a: 'b,
    {
        match self.values.get(name) {
            Some(cell) => entry::borrow_cell_mut(cell, name),
            None => self.fixture_entry(name)?.try_borrow_mut::<T>(name),
        }
    }

    /// Look up the storage entry for `name`, reporting
    /// [`FixtureBorrowError::NotFound`] when no fixture is registered.
    fn fixture_entry(&self, name: &str) -> Result<&FixtureEntry<'a>, FixtureBorrowError> {
        self.fixtures
            .get(name)
            .ok_or_else(|| FixtureBorrowError::not_found(name))
    }
}

/// Report whether `log::warn!` output is currently discarded.
///
/// Kept as a named predicate so the mirrored `eprintln!` call site stays a
/// simple two-branch condition.
fn warn_logging_is_disabled() -> bool {
    !log::log_enabled!(log::Level::Warn)
}
