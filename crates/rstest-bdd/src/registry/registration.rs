//! Inventory registration for step definitions.
//!
//! The exported [`step!`] macro attaches source metadata to every registered
//! definition so library selection can remain lexical and deterministic.

/// Register a step definition with the inventory registry.
///
/// The macro accepts sync-only and explicit async handlers. Every expansion
/// records the defining Rust module path, allowing the registry to associate
/// the step with its nearest `#[step_library]` declaration.
#[macro_export]
macro_rules! step {
    (
        @pattern $keyword:expr, $pattern:expr, $handler:path, $async_handler:path,
        $fixtures:expr, $mode:expr
    ) => {
        const _: () = {
            $crate::submit! {
                $crate::Step {
                    module_path: module_path!(), keyword: $keyword, pattern: $pattern,
                    run: $handler, run_async: $async_handler, execution_mode: $mode,
                    fixtures: $fixtures, file: file!(), line: line!(),
                }
            }
        };
    };
    (@pattern $keyword:expr, $pattern:expr, $handler:path, $fixtures:expr, $mode:expr) => {
        const _: () = {
            fn __rstest_bdd_auto_async<'ctx, 'fixtures>(
                ctx: &'ctx mut $crate::StepContext<'fixtures>, text: &'ctx str,
                docstring: ::core::option::Option<&'ctx str>,
                table: ::core::option::Option<&'ctx [&'ctx [&'ctx str]]>,
            ) -> $crate::StepFuture<'ctx> {
                ::std::boxed::Box::pin(::std::future::ready($handler(ctx, text, docstring, table)))
            }
            $crate::submit! {
                $crate::Step {
                    module_path: module_path!(), keyword: $keyword, pattern: $pattern,
                    run: $handler, run_async: __rstest_bdd_auto_async, execution_mode: $mode,
                    fixtures: $fixtures, file: file!(), line: line!(),
                }
            }
        };
    };
    ($keyword:expr, $pattern:expr, $handler:path, & $fixtures:expr, mode = $mode:expr $(,)?) => {
        const _: () = {
            static PATTERN: $crate::StepPattern = $crate::StepPattern::new($pattern);
            $crate::step!(@pattern $keyword, &PATTERN, $handler, &$fixtures, $mode);
        };
    };
    ($keyword:expr, $pattern:expr, $handler:path, & $fixtures:expr) => {
        const _: () = {
            static PATTERN: $crate::StepPattern = $crate::StepPattern::new($pattern);
            $crate::step!(@pattern $keyword, &PATTERN, $handler, &$fixtures, $crate::StepExecutionMode::Both);
        };
    };
    ($keyword:expr, $pattern:expr, $handler:path, $async_handler:path, $fixtures:expr, mode = $mode:expr $(,)?) => {
        const _: () = {
            static PATTERN: $crate::StepPattern = $crate::StepPattern::new($pattern);
            $crate::step!(@pattern $keyword, &PATTERN, $handler, $async_handler, $fixtures, $mode);
        };
    };
    ($keyword:expr, $pattern:expr, $handler:path, $async_handler:path, $fixtures:expr) => {
        const _: () = {
            static PATTERN: $crate::StepPattern = $crate::StepPattern::new($pattern);
            $crate::step!(
                @pattern $keyword, &PATTERN, $handler, $async_handler, $fixtures,
                $crate::StepExecutionMode::Both
            );
        };
    };
}
