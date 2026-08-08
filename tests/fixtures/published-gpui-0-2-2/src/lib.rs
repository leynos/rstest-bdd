//! Compile-only coverage for the documented published GPUI stateful steps.
//!
//! `make check-published-gpui` runs `cargo check` over this crate; nothing here
//! is ever executed, because published `gpui` pulls in the graphics and
//! windowing stack that the `vendor/gpui` shim exists to avoid. The gate is
//! therefore compilation: each step's bindings are annotated so the published
//! call shapes and return types documented in `docs/users-guide.md` fail the
//! build if they drift. The `assert_eq!` calls record the expected runtime
//! semantics for readers; the executable proof of those semantics lives in
//! `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`, against the shim.

use std::cell::RefCell;

use gpui::{AppContext as _, VisualContext as _};
use rstest_bdd_macros::{given, then, when};

/// Minimal published-GPUI view whose state the documented steps mutate.
#[derive(Default)]
struct CounterView {
    /// Counter incremented by the `#[when]` step and asserted by `#[then]`.
    value: usize,
}

impl CounterView {
    /// Builds the view from the published view context handed to
    /// `add_window_view`.
    fn new(_view_cx: &mut gpui::Context<Self>) -> Self {
        Self::default()
    }
}

impl gpui::Render for CounterView {
    /// Renders nothing; the fixture only exercises entity access, not layout.
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _view_cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::Empty
    }
}

/// Durable handles shared across the steps of one scenario.
#[derive(Default)]
struct ScenarioState {
    /// Typed handle to the stored view; `Clone`, not `Copy`, when published.
    entity: Option<gpui::Entity<CounterView>>,
    /// Identifies the window, so a fresh `VisualTestContext` can be rebuilt.
    window: Option<gpui::AnyWindowHandle>,
}

thread_local! {
    /// Scenario state, kept thread-local because steps cannot borrow both
    /// `&mut TestAppContext` and a shared mutable world under the v0.6 API.
    static SCENARIO_STATE: RefCell<ScenarioState> =
        RefCell::new(ScenarioState::default());
}

/// Runs `callback` against the current thread's scenario state.
fn with_state<R>(callback: impl FnOnce(&mut ScenarioState) -> R) -> R {
    SCENARIO_STATE.with(|state| callback(&mut state.borrow_mut()))
}

/// Clears stale handles before a `#[given]` stores fresh ones.
fn reset_state_before_assignment() {
    SCENARIO_STATE.with(|state| *state.borrow_mut() = ScenarioState::default());
}

/// Returns the stored handles, panicking if a step ran out of order.
fn current_handles() -> (gpui::Entity<CounterView>, gpui::AnyWindowHandle) {
    with_state(|state| {
        let Some(entity) = state.entity.clone() else {
            panic!("scenario should have stored an entity handle");
        };
        let Some(window) = state.window else {
            panic!("scenario should have stored a window handle");
        };
        (entity, window)
    })
}

/// Opens a window and stores only the durable handles it yields.
#[given("a fresh GPUI window is opened")]
fn fresh_gpui_window_is_opened(
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    let stale_window_count = with_state(|state| usize::from(state.window.is_some()));
    reset_state_before_assignment();

    let (entity, visual_context) =
        context.add_window_view(|_window, view_cx| CounterView::new(view_cx));
    let window = visual_context.window_handle();

    with_state(|state| {
        state.entity = Some(entity);
        state.window = Some(window);
    });

    assert_eq!(
        stale_window_count, 0,
        "reset-before-assignment should remove stale scenario state"
    );
}

/// Rebuilds the visual context and asserts the published `update_entity`
/// returns the callback value directly rather than wrapping it.
#[when("the view is updated through a reconstructed visual context")]
fn view_is_updated_through_reconstructed_visual_context(
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    let (entity, window) = current_handles();
    let mut visual_context = gpui::VisualTestContext::from_window(window, context);
    // The explicit `usize` binding is the load-bearing check here: the fixture
    // is never executed, so it is the type, not the assertion, that proves
    // published `update_entity` returns the callback value directly. The
    // vendored shim returns `Result<usize, _>` and would fail to compile.
    let value: usize = visual_context.update_entity(&entity, |view, _view_cx| {
        view.value += 1;
        view.value
    });
    assert_eq!(value, 1);
}

/// Confirms the stored handles still resolve to the mutated view.
#[then("the durable handles still identify the updated view")]
fn durable_handles_identify_the_updated_view(
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    let (entity, window) = current_handles();
    let visual_context = gpui::VisualTestContext::from_window(window, context);
    // As above, the binding's type is what the compile-only check verifies.
    let value: usize = visual_context.read_entity(&entity, |view, _app| view.value);

    assert_eq!(value, 1);
}
