//! Behavioural coverage for stateful GPUI window scenarios.
#![cfg(feature = "native-gpui-tests")]

use rstest_bdd_macros::{given, scenario, then, when};
use serial_test::serial;
use std::cell::RefCell;

#[derive(Clone, Debug, Default)]
struct CounterView {
    value: usize,
}

#[derive(Clone, Debug, Default)]
struct ScenarioState {
    entity: Option<gpui::Entity<CounterView>>,
    window: Option<gpui::AnyWindowHandle>,
    opened_window_count: usize,
}

thread_local! {
    static SCENARIO_STATE: RefCell<ScenarioState> =
        RefCell::new(ScenarioState::default());
}

fn reset_state_before_assignment() {
    // Reset before assigning the next scenario's handles so a reused serial
    // test thread cannot observe handles left by a failed or skipped scenario.
    SCENARIO_STATE.with(|state| *state.borrow_mut() = ScenarioState::default());
}

fn with_state<R>(operation: impl FnOnce(&mut ScenarioState) -> R) -> R {
    SCENARIO_STATE.with(|state| operation(&mut state.borrow_mut()))
}

fn current_handles() -> (gpui::Entity<CounterView>, gpui::AnyWindowHandle) {
    with_state(|state| {
        let entity = state
            .entity
            .unwrap_or_else(|| panic!("scenario should have stored an entity handle"));
        let window = state
            .window
            .unwrap_or_else(|| panic!("scenario should have stored a window handle"));
        (entity, window)
    })
}

#[given("a fresh GPUI window is opened")]
fn fresh_gpui_window_is_opened(
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    let stale_window_count = with_state(|state| usize::from(state.window.is_some()));
    reset_state_before_assignment();

    let (entity, visual_context) = context.add_window_view(|_context| CounterView::default());
    let window = visual_context.window_handle();

    with_state(|state| {
        state.entity = Some(entity);
        state.window = Some(window);
        state.opened_window_count = context.windows().len();
    });

    assert_eq!(
        stale_window_count, 0,
        "reset-before-assignment should remove stale scenario state"
    );
}

#[when("the view is updated through a reconstructed visual context")]
fn view_is_updated_through_reconstructed_visual_context(
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    let (entity, window) = current_handles();
    let mut visual_context = gpui::VisualTestContext::from_window(window, context)
        .unwrap_or_else(|| panic!("stored window handle should reconstruct visual context"));
    assert_eq!(
        visual_context.update_entity(entity, |view| view.value += 1),
        Ok(())
    );
}

#[then("the durable handles still identify the updated view")]
fn durable_handles_identify_the_updated_view(
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    let (entity, window) = current_handles();
    let visual_context = gpui::VisualTestContext::from_window(window, context)
        .unwrap_or_else(|| panic!("stored window handle should reconstruct visual context"));

    assert_eq!(
        visual_context.read_entity(entity, |view| view.value),
        Some(1)
    );
}

#[then("no stale handles from a previous scenario remain")]
fn no_stale_handles_from_previous_scenario_remain() {
    with_state(|state| {
        assert!(
            state.entity.is_some() && state.window.is_some(),
            "current scenario should assign fresh handles after reset"
        );
        assert_eq!(
            state.opened_window_count, 1,
            "fresh context should expose exactly one window"
        );
    });
}

#[scenario(
    path = "tests/features/stateful_window.feature",
    name = "Reconstruct visual context from durable handles",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
)]
#[serial]
fn scenario_reconstructs_visual_context_from_durable_handles() {}

#[scenario(
    path = "tests/features/stateful_window.feature",
    name = "Opening a second GPUI window starts from reset state",
    harness = rstest_bdd_harness_gpui::GpuiHarness,
)]
#[serial]
fn scenario_opening_second_window_starts_from_reset_state() {}
