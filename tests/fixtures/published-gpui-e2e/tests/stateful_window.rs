//! End-to-end coverage for stateful scenarios against published GPUI.
//!
//! The root workspace resolves GPUI through a stable-compatible vendored shim.
//! This standalone nightly fixture instead runs packaged rstest-bdd artefacts
//! against crates.io `gpui 0.2.2`, proving the executable harness boundary and
//! the documented stateful call shapes together.

use std::cell::RefCell;

use gpui::{AppContext as _, VisualContext as _};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serial_test::serial;

/// Holds the counter value rendered by the published-GPUI test window.
#[derive(Default)]
struct CounterView {
    /// Stores the value that the reconstructed visual context increments.
    value: usize,
}

impl CounterView {
    /// Creates an empty counter view for a newly opened GPUI window.
    fn new(_view_context: &mut gpui::Context<Self>) -> Self { Self::default() }
}

impl gpui::Render for CounterView {
    /// Renders no elements because this fixture verifies state rather than UI output.
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _view_context: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::Empty
    }
}

/// Stores durable handles and reset observations for one stateful scenario.
#[derive(Default)]
struct ScenarioState {
    /// Retains the counter entity created by the Given step.
    entity: Option<gpui::Entity<CounterView>>,
    /// Retains the window handle paired with the counter entity.
    window: Option<gpui::AnyWindowHandle>,
    /// Records how many windows the current scenario opened.
    opened_window_count: usize,
}

thread_local! {
    /// Holds isolated state for the scenario executing on the current test thread.
    static SCENARIO_STATE: RefCell<ScenarioState> = RefCell::new(ScenarioState::default());
}

/// Runs an operation with the thread-local state for the active scenario.
fn with_state<R>(operation: impl FnOnce(&mut ScenarioState) -> R) -> R {
    SCENARIO_STATE.with(|state| operation(&mut state.borrow_mut()))
}

/// Clears scenario state before the Given step assigns fresh durable handles.
fn reset_state_before_assignment() {
    SCENARIO_STATE.with(|state| *state.borrow_mut() = ScenarioState::default());
}

/// Clears scenario state after the cleanup fixture finishes.
fn reset_state_after_scenario() {
    SCENARIO_STATE.with(|state| *state.borrow_mut() = ScenarioState::default());
}

/// Clears thread-local state when the scenario cleanup fixture is dropped.
struct ScenarioStateCleanup;

impl Drop for ScenarioStateCleanup {
    /// Resets the state after each scenario, including assertion failures.
    fn drop(&mut self) { reset_state_after_scenario(); }
}

/// Initializes scenario state and returns the guard that resets it afterwards.
#[fixture]
fn scenario_state_cleanup() -> ScenarioStateCleanup {
    reset_state_before_assignment();
    ScenarioStateCleanup
}

/// Returns the durable entity and window handles stored by the Given step.
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

/// Opens a published-GPUI window and stores its durable handles for later steps.
#[given("a fresh GPUI window is opened")]
fn fresh_gpui_window_is_opened(
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    let stale_window_count = with_state(|state| usize::from(state.window.is_some()));
    reset_state_before_assignment();

    let (entity, visual_context) =
        context.add_window_view(|_window, view_context| CounterView::new(view_context));
    let window = visual_context.window_handle();
    let opened_window_count = context.windows().len();

    with_state(|state| {
        state.entity = Some(entity);
        state.window = Some(window);
        state.opened_window_count = opened_window_count;
    });

    assert_eq!(
        stale_window_count, 0,
        "reset-before-assignment should remove stale scenario state"
    );
}

/// Increments the stored counter through a reconstructed published visual context.
#[when("the view is updated through a reconstructed visual context")]
fn view_is_updated_through_reconstructed_visual_context(
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    let (entity, window) = current_handles();
    let mut visual_context = gpui::VisualTestContext::from_window(window, context);
    let updated_value: usize = visual_context.update_entity(&entity, |view, _view_context| {
        view.value += 1;
        view.value
    });

    assert_eq!(
        updated_value, 1,
        "the published context should update the view"
    );
}

/// Verifies that reconstructed published-GPUI handles identify the updated view.
#[then("the durable handles still identify the updated view")]
fn durable_handles_identify_the_updated_view(
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    let (entity, window) = current_handles();
    let entity_id = entity.entity_id();
    let cloned_entity = entity.clone();
    let visual_context = gpui::VisualTestContext::from_window(window, context);
    let counter_value: usize = visual_context.read_entity(&entity, |view, _app| view.value);

    assert_eq!(
        counter_value, 1,
        "the reconstructed context should read the update"
    );
    assert_eq!(
        cloned_entity.entity_id(),
        entity_id,
        "cloning an Entity must preserve its durable identity"
    );
    assert_eq!(
        visual_context.window_handle().window_id(),
        window.window_id(),
        "the reconstructed visual context must retain the stored window identity"
    );
}

/// Verifies that the current scenario contains only its freshly assigned handles.
#[then("no stale handles from a previous scenario remain")]
fn no_stale_handles_from_previous_scenario_remain() {
    with_state(|state| {
        assert!(
            state.entity.is_some() && state.window.is_some(),
            "current scenario should assign fresh handles after reset"
        );
        assert_eq!(
            state.opened_window_count, 1,
            "the harness should provide one fresh published-GPUI window"
        );
    });
}

/// Executes the scenario that reconstructs a visual context from durable handles.
#[scenario(
    path = "tests/features/stateful_window.feature",
    name = "Reconstruct visual context from durable handles",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
)]
#[serial]
fn scenario_reconstructs_visual_context_from_durable_handles(
    #[from(scenario_state_cleanup)] _cleanup: ScenarioStateCleanup,
) {
}

/// Executes the scenario that proves a second window starts from reset state.
#[scenario(
    path = "tests/features/stateful_window.feature",
    name = "Opening a second GPUI window starts from reset state",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
)]
#[serial]
fn scenario_opening_second_window_starts_from_reset_state(
    #[from(scenario_state_cleanup)] _cleanup: ScenarioStateCleanup,
) {
}
