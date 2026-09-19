//! Tests for `ScenarioPlan`, `StepInvocation`, and their builder.

use crate::{StepKeyword, runner::ScenarioPlanBuilder};

/// The macro path must not copy: text and tags reach the plan as
/// `Cow::Borrowed` over the original `'static` literal.
///
/// Observed by pointer identity, because an accessor returning `&str` cannot
/// distinguish a borrow from a copy of equal contents. A builder that allocated
/// a `String` for either would leave the pointers unequal.
#[test]
fn macro_path_allocates_no_step_text() {
    const TEXT: &str = "a calculator";
    const TAG: &str = "@allow_skipped";
    const NAME: &str = "Add two numbers";
    const SOURCE: &str = "notes/arithmetic.md";

    let plan = ScenarioPlanBuilder::new(NAME, SOURCE)
        .tag(TAG)
        .step_at(StepKeyword::Given, TEXT, 43)
        .build();

    let step = plan.steps().first().expect("one step was added");
    assert!(
        std::ptr::eq(step.text().as_ptr(), TEXT.as_ptr()),
        "step text must borrow the static literal, not copy it",
    );

    let tag = plan.tags().next().expect("one tag was added");
    assert!(
        std::ptr::eq(tag.as_ptr(), TAG.as_ptr()),
        "tag must borrow the static literal, not copy it",
    );

    assert!(
        std::ptr::eq(plan.name().as_ptr(), NAME.as_ptr()),
        "the scenario name must borrow the static literal, not copy it",
    );
}
