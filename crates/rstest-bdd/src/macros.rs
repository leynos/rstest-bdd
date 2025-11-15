//! Public macro helpers exported by `rstest-bdd`.
//!
//! The macros live in a dedicated module to keep `lib.rs` small and focused on
//! type exports. They remain available at the crate root via `#[macro_export]`.

/// Skip the current scenario with an optional message.
///
/// Step or hook functions may invoke the macro to stop executing the remaining
/// steps. When the [`config::fail_on_skipped`](crate::config::fail_on_skipped)
/// flag is enabled, scenarios without the `@allow_skipped` tag panic after the
/// last executed step instead of being recorded as skipped.
#[macro_export]
macro_rules! skip {
    () => {{
        $crate::__rstest_bdd_request_current_skip(None)
    }};
    ($msg:expr $(,)?) => {{
        $crate::__rstest_bdd_request_current_skip(Some(Into::<String>::into($msg)))
    }};
    ($fmt:expr, $($arg:tt)*) => {{
        $crate::__rstest_bdd_request_current_skip(Some(format!($fmt, $($arg)*)))
    }};
}

/// Assert that a [`Result`] is `Ok` and unwrap it.
///
/// Panics with a message including the error when the value is an `Err`.
///
/// Note: Formatting the error in the panic message requires the error type to
/// implement [`std::fmt::Display`].
///
/// # Examples
/// ```
/// use rstest_bdd::assert_step_ok;
///
/// let res: Result<(), &str> = Ok(());
/// assert_step_ok!(res);
/// ```
#[macro_export]
macro_rules! assert_step_ok {
    ($expr:expr $(,)?) => {
        match $expr {
            Ok(value) => value,
            Err(e) => $crate::panic_localized!("assert-step-ok-panic", error = e),
        }
    };
}

/// Assert that a [`Result`] is `Err` and unwrap the error.
///
/// Optionally asserts that the error's display contains a substring.
///
/// Note: The `(expr, "substring")` form requires the error type to
/// implement [`std::fmt::Display`] so it can be converted to a string for
/// matching.
///
/// # Examples
/// ```
/// use rstest_bdd::assert_step_err;
///
/// let err: Result<(), &str> = Err("boom");
/// let e = assert_step_err!(err, "boom");
/// assert_eq!(e, "boom");
/// ```
///
/// Single-argument form:
/// ```
/// use rstest_bdd::assert_step_err;
///
/// let err: Result<(), &str> = Err("boom");
/// let e = assert_step_err!(err);
/// assert_eq!(e, "boom");
/// ```
#[macro_export]
macro_rules! assert_step_err {
    ($expr:expr $(,)?) => {
        match $expr {
            Ok(_) => $crate::panic_localized!("assert-step-err-success"),
            Err(e) => e,
        }
    };
    ($expr:expr, $msg:expr $(,)?) => {
        match $expr {
            Ok(_) => $crate::panic_localized!("assert-step-err-success"),
            Err(e) => {
                let __rstest_bdd_display = e.to_string();
                let __rstest_bdd_msg: &str = $msg.as_ref();
                assert!(
                    __rstest_bdd_display.contains(__rstest_bdd_msg),
                    "{}",
                    $crate::localization::message_with_args(
                        "assert-step-err-missing-substring",
                        |args| {
                            args.set("display", __rstest_bdd_display.clone());
                            args.set("expected", __rstest_bdd_msg.to_string());
                        },
                    )
                );
                e
            }
        }
    };
}

/// Assert that a [`StepExecution`](crate::StepExecution) represents a skipped
/// outcome.
///
/// Returns the optional skip message so callers can inspect it further. Supply
/// `message = "substring"` to assert that the reason contains a specific
/// fragment, or `message_absent = true` to assert that no message was
/// provided.
///
/// # Examples
/// ```
/// use rstest_bdd::{assert_step_skipped, StepExecution};
///
/// let message = assert_step_skipped!(
///     StepExecution::skipped(Some("pending dependency".into())),
///     message = "pending",
/// );
/// assert_eq!(message, Some("pending dependency".into()));
/// ```
#[macro_export]
macro_rules! assert_step_skipped {
    ($expr:expr $(,)?) => {{
        match $expr {
            $crate::StepExecution::Skipped { message } => message,
            $crate::StepExecution::Continue { .. } => {
                $crate::panic_localized!("assert-skip-not-skipped", target = "step execution",)
            }
        }
    }};
    ($expr:expr, message = $value:expr $(,)?) => {{
        let __rstest_bdd_message = $crate::assert_step_skipped!($expr);
        let __rstest_bdd_expected: String = ::std::convert::Into::into($value);
        $crate::__rstest_bdd_expect_skip_message_contains(
            __rstest_bdd_message.as_deref(),
            __rstest_bdd_expected.as_str(),
            "step execution",
        );
        __rstest_bdd_message
    }};
    ($expr:expr, message_absent = $value:expr $(,)?) => {{
        let __rstest_bdd_message = $crate::assert_step_skipped!($expr);
        if $value {
            $crate::__rstest_bdd_expect_skip_message_absent(
                __rstest_bdd_message.as_deref(),
                "step execution",
            );
        }
        __rstest_bdd_message
    }};
    ($expr:expr, $($rest:tt)+) => {{
        compile_error!("unsupported assert_step_skipped! arguments; expected `message = ...`");
    }};
}

/// Assert that a [`ScenarioStatus`](crate::reporting::ScenarioStatus) recorded
/// a skipped outcome.
///
/// Returns a cloned [`SkippedScenario`](crate::reporting::SkippedScenario)
/// describing the skip. Provide optional filters to assert message contents,
/// whether skipping was allowed, and whether it forced the run to fail.
///
/// # Examples
/// ```
/// use rstest_bdd::assert_scenario_skipped;
/// use rstest_bdd::reporting::{ScenarioStatus, SkippedScenario};
///
/// let status = ScenarioStatus::Skipped(SkippedScenario::new(
///     Some("pending upstream".into()),
///     true,
///     false,
/// ));
/// let details = assert_scenario_skipped!(
///     status,
///     message = "pending",
///     allow_skipped = true,
/// );
/// assert!(details.allow_skipped());
/// ```
#[macro_export]
macro_rules! assert_scenario_skipped {
    ($status:expr $(, $key:ident = $value:expr )* $(,)?) => {{
        let __rstest_bdd_status = &$status;
        let __rstest_bdd_details = match __rstest_bdd_status {
            $crate::reporting::ScenarioStatus::Skipped(details) => details.clone(),
            _ => {
                $crate::panic_localized!(
                    "assert-skip-not-skipped",
                    target = "scenario status",
                )
            }
        };
        $(
            $crate::__rstest_bdd_assert_scenario_detail!(
                &__rstest_bdd_details,
                $key,
                $value
            );
        )*
        __rstest_bdd_details
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rstest_bdd_assert_scenario_detail {
    ($details:expr, message, None) => {{
        $crate::__rstest_bdd_expect_skip_message_absent($details.message(), "scenario status");
    }};
    ($details:expr, message, Some($value:expr)) => {{
        let __rstest_bdd_expected: String = ::std::convert::Into::into($value);
        $crate::__rstest_bdd_expect_skip_message_contains(
            $details.message(),
            __rstest_bdd_expected.as_str(),
            "scenario status",
        );
    }};
    ($details:expr, message, $value:expr) => {{
        let __rstest_bdd_expected: String = ::std::convert::Into::into($value);
        $crate::__rstest_bdd_expect_skip_message_contains(
            $details.message(),
            __rstest_bdd_expected.as_str(),
            "scenario status",
        );
    }};
    ($details:expr, message_absent, $value:expr) => {{
        if $value {
            $crate::__rstest_bdd_expect_skip_message_absent($details.message(), "scenario status");
        }
    }};
    ($details:expr, allow_skipped, $value:expr) => {{
        let __rstest_bdd_expected_bool: bool = $value;
        $crate::__rstest_bdd_expect_skip_flag(
            $details.allow_skipped(),
            __rstest_bdd_expected_bool,
            "scenario status",
            "allow_skipped",
        );
    }};
    ($details:expr, forced_failure, $value:expr) => {{
        let __rstest_bdd_expected_bool: bool = $value;
        $crate::__rstest_bdd_expect_skip_flag(
            $details.forced_failure(),
            __rstest_bdd_expected_bool,
            "scenario status",
            "forced_failure",
        );
    }};
    ($details:expr, $other:ident, $value:expr) => {{
        let _ = &$value;
        compile_error!(concat!(
            "unsupported key for assert_scenario_skipped!: ",
            stringify!($other),
            "; supported keys: message, message_absent, allow_skipped, forced_failure",
        ));
    }};
}
