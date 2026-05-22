//! Behavioural coverage for stateful GPUI window scenarios.
//!
//! These tests exercise the GPUI harness path used by generated BDD scenarios:
//! a `GpuiHarness` scenario receives one `TestAppContext`, creates a window,
//! stores durable `Entity<T>` and `AnyWindowHandle` values in scenario state,
//! and reconstructs `VisualTestContext` from those handles in later steps.
//!
//! The module also documents the reset protocol expected by stateful scenarios.
//! Thread-local scenario state is cleared before fresh handle assignment and by
//! a fixture guard at scenario teardown, so success, failure, and skip paths do
//! not leak stale handles into the next serial GPUI scenario.
#![cfg(feature = "native-gpui-tests")]

use rstest::fixture;
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
    reset_state_after_scenario();
}

fn reset_state_after_scenario() {
    SCENARIO_STATE.with(|state| *state.borrow_mut() = ScenarioState::default());
}

#[derive(Clone, Debug)]
struct ScenarioStateCleanup;

impl Drop for ScenarioStateCleanup {
    fn drop(&mut self) {
        reset_state_after_scenario();
    }
}

#[fixture]
fn scenario_state_cleanup() -> ScenarioStateCleanup {
    reset_state_before_assignment();
    ScenarioStateCleanup
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

#[test]
fn update_entity_returns_not_found_for_unknown_handle() {
    let mut context_with_window = gpui::TestAppContext::single();
    let (stale_entity, _visual_context) =
        context_with_window.add_window_view(|_context| CounterView::default());

    let mut unrelated_context = gpui::TestAppContext::single();
    let (_entity, visual_context) =
        unrelated_context.add_window_view(|_context| CounterView::default());
    let mut unrelated_visual_context = gpui::VisualTestContext::from_window(
        visual_context.window_handle(),
        &mut unrelated_context,
    )
    .unwrap_or_else(|| panic!("fresh window handle should reconstruct visual context"));

    let result = unrelated_visual_context.update_entity(stale_entity, |view| {
        view.value += 1;
        panic!("stale entity handle should not invoke the update closure");
    });

    assert_eq!(
        result,
        Err(gpui::EntityError::NotFound {
            id: stale_entity.id()
        })
    );
}

#[test]
fn entity_error_display_snapshot() {
    let error = gpui::EntityError::NotFound { id: 42 };

    insta::assert_snapshot!(format!("{error}"));
}

#[test]
fn visual_test_context_from_window_returns_none_for_foreign_handle() {
    let mut context_with_window = gpui::TestAppContext::single();
    let (_entity, visual_context) =
        context_with_window.add_window_view(|_context| CounterView::default());
    let foreign_window = visual_context.window_handle();

    let mut unrelated_context = gpui::TestAppContext::single();
    let (_entity, unrelated_visual_context) =
        unrelated_context.add_window_view(|_context| CounterView::default());
    assert_eq!(
        foreign_window.id(),
        unrelated_visual_context.window_handle().id(),
        "this regression must prove equal numeric ids from different contexts do not match"
    );
    assert!(
        gpui::VisualTestContext::from_window(foreign_window, &mut unrelated_context).is_none(),
        "from_window should reject handles from a different TestAppContext"
    );
}

#[test]
fn entity_access_is_rejected_from_the_wrong_window() {
    let mut context = gpui::TestAppContext::single();
    let (first_entity, first_visual_context) =
        context.add_window_view(|_context| CounterView::default());
    let (_second_entity, second_visual_context) =
        context.add_window_view(|_context| CounterView::default());

    assert_eq!(
        first_visual_context.read_entity(first_entity, |view| view.value),
        Some(0),
        "the owning visual context should be able to read its own entity"
    );

    let mut second_visual_context =
        gpui::VisualTestContext::from_window(second_visual_context.window_handle(), &mut context)
            .unwrap_or_else(|| panic!("second window handle should reconstruct visual context"));
    let update_result = second_visual_context.update_entity(first_entity, |view| {
        view.value += 1;
        panic!("cross-window entity updates should not invoke the update closure");
    });

    assert_eq!(
        update_result,
        Err(gpui::EntityError::NotFound {
            id: first_entity.id()
        }),
        "a visual context should reject entity handles owned by another window"
    );
    assert_eq!(
        second_visual_context.read_entity(first_entity, |view| view.value),
        None,
        "a visual context should not read entity handles owned by another window"
    );
}

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
