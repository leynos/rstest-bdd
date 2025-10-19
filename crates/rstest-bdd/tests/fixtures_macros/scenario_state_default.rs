use rstest_bdd::Slot;
use rstest_bdd::ScenarioState as _;
use rstest_bdd_macros::ScenarioState;

#[derive(Default, ScenarioState)]
struct DerivedState {
    value: Slot<u32>,
}

fn assert_reset() {
    let state = DerivedState::default();
    assert!(state.value.is_empty());
    state.reset();
}

fn main() {
    assert_reset();
}
