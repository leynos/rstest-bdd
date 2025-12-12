//! Helpers for signalling that a scenario should be skipped.
//!
//! The [`skip!`](crate::skip!) macro triggers a panic carrying a [`SkipRequest`]
//! payload. Step wrappers intercept that panic, convert it into a skipped
//! outcome, and stop executing subsequent steps. When the `fail_on_skipped`
//! configuration flag is enabled scenarios without an `@allow_skipped` tag
//! panic after the final step instead of being marked as skipped.

use std::cell::RefCell;
use std::fmt;
use std::marker::PhantomData;
use std::panic;
use std::rc::Rc;
use std::thread::{self, ThreadId};

thread_local! {
    static SCOPE_STACK: RefCell<Vec<ScopeEntry>> = const { RefCell::new(Vec::new()) };
}

/// Internal marker carried by the panic that requests the scenario to be
/// skipped.
#[derive(Debug)]
pub struct SkipRequest {
    message: Option<String>,
}

impl SkipRequest {
    /// Create a new skip request with an optional message.
    #[must_use]
    pub fn new(message: Option<String>) -> Self {
        Self { message }
    }

    /// Consume the request, returning the original message.
    #[must_use]
    pub fn into_message(self) -> Option<String> {
        self.message
    }

    /// Panic with this skip request.
    #[track_caller]
    pub fn raise(message: Option<String>) -> ! {
        panic::resume_unwind(Box::new(Self::new(message)));
    }
}

/// Describes where a skip request originated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_enums,
    reason = "Only step and hook scopes are meaningful for skip tracking"
)]
pub enum ScopeKind {
    /// Skip invoked from a step definition.
    Step,
    /// Skip invoked from a hook function.
    Hook,
}

impl ScopeKind {
    const fn describe(self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Hook => "hook",
        }
    }
}

/// Metadata describing the current execution scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScopeMetadata {
    kind: ScopeKind,
    name: &'static str,
    file: &'static str,
    line: u32,
}

impl ScopeMetadata {
    /// Construct metadata for a scope entry.
    #[must_use]
    pub const fn new(kind: ScopeKind, name: &'static str, file: &'static str, line: u32) -> Self {
        Self {
            kind,
            name,
            file,
            line,
        }
    }

    fn describe(&self) -> (&'static str, &'static str, u32) {
        (self.kind.describe(), self.name, self.line)
    }
}

/// RAII guard that marks the current thread as executing a step or hook.
#[derive(Debug)]
pub struct StepScopeGuard {
    metadata: ScopeMetadata,
    thread: ThreadId,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl StepScopeGuard {
    /// Enter a scope represented by the provided metadata.
    #[must_use]
    pub fn enter(metadata: ScopeMetadata) -> Self {
        Self {
            metadata,
            thread: thread::current().id(),
            _not_send_or_sync: PhantomData,
        }
    }

    fn register(&self) {
        SCOPE_STACK.with(|stack| {
            stack.borrow_mut().push(ScopeEntry {
                metadata: self.metadata,
                thread: self.thread,
            });
        });
    }
}

impl Drop for StepScopeGuard {
    fn drop(&mut self) {
        SCOPE_STACK.with(|stack| {
            let mut stack = stack.borrow_mut();
            let matches = stack
                .pop()
                .is_some_and(|entry| entry.metadata == self.metadata);
            debug_assert!(matches, "scope stack must contain matching entry");
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ScopeEntry {
    metadata: ScopeMetadata,
    thread: ThreadId,
}

impl ScopeEntry {
    fn ensure_thread(&self) -> Result<(), ScopeError> {
        let current = thread::current().id();
        if self.thread == current {
            return Ok(());
        }
        Err(ScopeError::WrongThread {
            expected: self.thread,
            actual: current,
            metadata: self.metadata,
        })
    }
}

#[derive(Debug)]
enum ScopeError {
    WrongThread {
        expected: ThreadId,
        actual: ThreadId,
        metadata: ScopeMetadata,
    },
}

impl fmt::Display for ScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongThread {
                expected,
                actual,
                metadata,
            } => {
                let (scope, name, line) = metadata.describe();
                write!(
                    f,
                    "rstest_bdd::skip! may only run on the thread executing the {scope} '{}'\
                     (defined at {}:{}). Expected thread id {:?} but {:?} attempted to invoke it.",
                    name, metadata.file, line, expected, actual,
                )
            }
        }
    }
}

/// Enter a new execution scope. Used by generated step/hook wrappers.
#[doc(hidden)]
#[must_use]
pub fn enter_scope(
    kind: ScopeKind,
    name: &'static str,
    file: &'static str,
    line: u32,
) -> StepScopeGuard {
    let guard = StepScopeGuard::enter(ScopeMetadata::new(kind, name, file, line));
    guard.register();
    guard
}

/// Validate the current thread and raise a skip request.
#[cfg(test)]
#[doc(hidden)]
pub fn request_skip(scope: &StepScopeGuard, message: Option<String>) -> ! {
    let entry = ScopeEntry {
        metadata: scope.metadata,
        thread: scope.thread,
    };
    entry.ensure_thread().unwrap_or_else(|err| panic!("{err}"));
    SkipRequest::raise(message);
}

fn with_current_scope<F, R>(callback: F) -> R
where
    F: FnOnce(&ScopeEntry) -> R,
{
    let entry = SCOPE_STACK.with(|stack| stack.borrow().last().copied());
    let Some(scope_entry) = entry else {
        panic!("rstest_bdd::skip! may only be used inside a step or hook generated by rstest-bdd");
    };
    callback(&scope_entry)
}

/// Raise a skip request using the innermost registered scope, panicking when absent.
#[doc(hidden)]
pub fn request_current_skip(message: Option<String>) -> ! {
    with_current_scope(|scope| {
        scope.ensure_thread().unwrap_or_else(|err| panic!("{err}"));
        SkipRequest::raise(message)
    })
}

impl fmt::Display for SkipRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.message {
            Some(msg) => f.write_str(msg),
            None => f.write_str("scenario skipped"),
        }
    }
}

/// Panic with a [`SkipRequest`] payload to indicate the current scenario should
/// be skipped.
///
/// This function underpins the [`skip!`](crate::skip!) macro and is intentionally
/// public so behavioural tests can trigger skips without importing the macro.
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::panic::{self, UnwindSafe};

    fn with_test_scope<F: FnOnce()>(body: F) {
        let guard = enter_scope(ScopeKind::Step, "test_scope", file!(), line!());
        body();
        drop(guard);
    }

    #[test]
    fn request_skip_raises_panic() {
        let result = panic::catch_unwind(|| SkipRequest::raise(Some("skip".to_string())));
        assert!(result.is_err(), "request_skip should panic");
    }

    #[rstest]
    #[case::without_message(|| with_test_scope(|| crate::skip!()), None)]
    #[case::single_argument(
        || with_test_scope(|| crate::skip!("maintenance window")),
        Some("maintenance window")
    )]
    #[case::formatted(
        || with_test_scope(|| {
            let detail = "service";
            crate::skip!("{detail} pending", detail = detail);
        }),
        Some("service pending")
    )]
    #[case::formatted_trailing_comma(
        || with_test_scope(|| {
            let detail = "service";
            crate::skip!("{detail} pending", detail = detail,);
        }),
        Some("service pending")
    )]
    fn skip_macro_records_expected_message<F>(
        #[case] trigger: F,
        #[case] expected: Option<&'static str>,
    ) where
        F: FnOnce() + UnwindSafe,
    {
        let result = panic::catch_unwind(trigger);
        let Err(payload) = result else {
            panic!("skip! should raise a panic payload");
        };
        let Ok(request) = payload.downcast::<SkipRequest>() else {
            panic!("payload should downcast to SkipRequest");
        };
        assert_eq!(
            request.into_message(),
            expected.map(ToString::to_string),
            "skip! should produce the expected optional message",
        );
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "test asserts join success and panic on thread mismatch"
    )]
    fn request_skip_complains_when_thread_changes() {
        let mut guard = enter_scope(ScopeKind::Step, "thread_check", file!(), line!());
        let other_id = std::thread::spawn(|| thread::current().id())
            .join()
            .expect("thread id");
        guard.thread = other_id;
        let result = panic::catch_unwind(|| request_skip(&guard, Some("msg".into())));
        let payload = result.expect_err("request_skip should panic on thread mismatch");
        let rendered = payload
            .downcast::<String>()
            .map(|msg| *msg)
            .or_else(|payload| payload.downcast::<&'static str>().map(|s| s.to_string()))
            .unwrap_or_else(|_| panic!("panic payload should be a string"));
        assert!(
            rendered.contains("rstest_bdd::skip! may only run on the thread"),
            "panic message should describe thread restrictions: {rendered}",
        );
    }

    #[test]
    fn skip_without_scope_panics() {
        let result = panic::catch_unwind(|| crate::skip!());
        assert!(result.is_err(), "skip should panic without a scope");
    }

    #[test]
    fn helper_functions_can_skip() {
        fn nested_helper() {
            crate::skip!("helper triggered skip");
        }

        fn helper() {
            nested_helper();
        }

        let result = panic::catch_unwind(|| with_test_scope(helper));
        assert!(result.is_err(), "helper skip should raise panic payload");
    }
}
