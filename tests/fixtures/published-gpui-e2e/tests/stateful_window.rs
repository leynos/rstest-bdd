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

#[derive(Default)]
struct CounterView {
    value: usize,
}

impl CounterView {
    fn new(_view_context: &mut gpui::Context<Self>) -> Self { Self::default() }
}

impl gpui::Render for CounterView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _view_context: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::Empty
    }
}

#[derive(Default)]
struct ScenarioState {
    entity: Option<gpui::Entity<CounterView>>,
    window: Option<gpui::AnyWindowHandle>,
    opened_window_count: usize,
}

thread_local! {
    static SCENARIO_STATE: RefCell<ScenarioState> = RefCell::new(ScenarioState::default());
}

fn with_state<R>(operation: impl FnOnce(&mut ScenarioState) -> R) -> R {
    SCENARIO_STATE.with(|state| operation(&mut state.borrow_mut()))
}

fn reset_state_before_assignment() {
    SCENARIO_STATE.with(|state| *state.borrow_mut() = ScenarioState::default());
}

fn reset_state_after_scenario() {
    SCENARIO_STATE.with(|state| *state.borrow_mut() = ScenarioState::default());
}

struct ScenarioStateCleanup;

impl Drop for ScenarioStateCleanup {
    fn drop(&mut self) { reset_state_after_scenario(); }
}

#[fixture]
fn scenario_state_cleanup() -> ScenarioStateCleanup {
    reset_state_before_assignment();
    ScenarioStateCleanup
}

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
