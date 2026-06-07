//! Compile-pass fixture for generated all-immutable fixture borrows.

use rstest_bdd::{
    StepContext, StepFixtureRequirements, StepKeyword,
    execution::{StepExecutionRequest, execute_step},
};
use rstest_bdd_macros::given;

#[given("all immutable fixtures")]
fn all_immutable_fixtures(_first: &FirstFixture, _second: &SecondFixture) {}

struct FirstFixture;
struct SecondFixture;

fn main() {
    let mut ctx = StepContext::default();
    let first = FirstFixture;
    let second = SecondFixture;
    ctx.insert("first", &first);
    ctx.insert("second", &second);

    let requirements = rstest_bdd::iter::<StepFixtureRequirements>
        .into_iter()
        .find(|entry| {
            entry.keyword == StepKeyword::Given
                && entry.pattern.as_str() == "all immutable fixtures"
        })
        .expect("generated submit block should register fixture requirements")
        .requirements;

    assert_eq!(requirements.len(), 2);
    assert_eq!(requirements[0].name, "first");
    assert_eq!(requirements[0].ty, "FirstFixture");
    assert_eq!(requirements[1].name, "second");
    assert_eq!(requirements[1].ty, "SecondFixture");

    let request = StepExecutionRequest {
        index: 0,
        keyword: StepKeyword::Given,
        text: "all immutable fixtures",
        docstring: None,
        table: None,
        feature_path: "ui.feature",
        scenario_name: "all immutable fixtures",
    };
    execute_step(&request, &mut ctx).expect("step should execute");
}
