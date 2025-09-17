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
            log::warn!(
                concat!(
                    "Ambiguous fixture override: more than one fixture matches ",
                    "type_id {:?}. Override ignored."
                ),
                ty
            );
            #[expect(
                clippy::print_stderr,
                reason = "surface ambiguous overrides when logging is disabled"
            )]
            if !log::log_enabled!(log::Level::Warn) {
                eprintln!(
                    concat!(
                        "Ambiguous fixture override: more than one fixture matches ",
                        "type_id {:?}. Override ignored."
                    ),
                    ty
                );
            }
            return None;
        }
        self.values.insert(name, value)
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
