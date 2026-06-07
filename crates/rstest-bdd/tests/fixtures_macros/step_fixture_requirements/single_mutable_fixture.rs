//! Compile-pass fixture for a generated single mutable fixture borrow.

use rstest_bdd::{
    StepContext, StepFixtureRequirements, StepKeyword,
    execution::{StepExecutionRequest, execute_step},
};
use rstest_bdd_macros::given;

#[given("single mutable fixture")]
fn single_mutable_fixture(some_fixture: &mut SomeFixture) {
    some_fixture.value = 42;
}

struct SomeFixture {
    value: u32,
}

fn main() {
    let mut ctx = StepContext::default();
    let fixture = StepContext::owned_cell(SomeFixture { value: 0 });
    ctx.insert_owned::<SomeFixture>("some_fixture", &fixture);

    let requirements = rstest_bdd::iter::<StepFixtureRequirements>
        .into_iter()
        .find(|entry| {
            entry.keyword == StepKeyword::Given
                && entry.pattern.as_str() == "single mutable fixture"
        })
        .expect("generated submit block should register fixture requirements")
        .requirements;

    assert_eq!(requirements.len(), 1);
    assert_eq!(requirements[0].name, "some_fixture");
    assert_eq!(requirements[0].ty, "SomeFixture");

    let request = StepExecutionRequest {
        index: 0,
        keyword: StepKeyword::Given,
        text: "single mutable fixture",
        docstring: None,
        table: None,
        feature_path: "ui.feature",
        scenario_name: "single mutable fixture",
    };
    execute_step(&request, &mut ctx).expect("step should execute");

    let fixture = fixture.into_inner();
    let fixture = fixture
        .downcast::<SomeFixture>()
        .expect("fixture should remain SomeFixture");
    assert_eq!(fixture.value, 42);
}
