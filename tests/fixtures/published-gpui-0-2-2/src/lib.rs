//! Compile-only coverage for the documented published GPUI stateful steps.

use std::cell::RefCell;

use gpui::{AppContext as _, VisualContext as _};
use rstest_bdd_macros::{given, then, when};

#[derive(Default)]
struct CounterView {
    value: usize,
}

impl CounterView {
    fn new(_view_cx: &mut gpui::Context<Self>) -> Self { Self::default() }
}

impl gpui::Render for CounterView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _view_cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::Empty
    }
}

#[derive(Default)]
struct ScenarioState {
    entity: Option<gpui::Entity<CounterView>>,
    window: Option<gpui::AnyWindowHandle>,
}

thread_local! {
    static SCENARIO_STATE: RefCell<ScenarioState> =
        RefCell::new(ScenarioState::default());
}

fn with_state<R>(callback: impl FnOnce(&mut ScenarioState) -> R) -> R {
    SCENARIO_STATE.with(|state| callback(&mut state.borrow_mut()))
}

fn reset_state_before_assignment() {
    SCENARIO_STATE.with(|state| *state.borrow_mut() = ScenarioState::default());
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

#[when("the view is updated through a reconstructed visual context")]
fn view_is_updated_through_reconstructed_visual_context(
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    let (entity, window) = current_handles();
    let mut visual_context = gpui::VisualTestContext::from_window(window, context);
    let value = visual_context.update_entity(&entity, |view, _view_cx| {
        view.value += 1;
        view.value
    });
    assert_eq!(value, 1);
}

#[then("the durable handles still identify the updated view")]
fn durable_handles_identify_the_updated_view(
    #[from(rstest_bdd_harness_context)] context: &mut gpui::TestAppContext,
) {
    let (entity, window) = current_handles();
    let visual_context = gpui::VisualTestContext::from_window(window, context);

    assert_eq!(
        visual_context.read_entity(&entity, |view, _app| view.value),
        1
    );
}
