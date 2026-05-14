//! Compile-pass fixture for generated fixture requirement registration.

use rstest_bdd::{StepContext, StepFixtureRequirements, StepKeyword};
use rstest_bdd_macros::given;

#[given("a generated fixture requirement")]
fn generated_fixture_requirement(_db: &DbPool) {}

struct DbPool;

fn main() {
    let mut ctx = StepContext::default();
    let db = DbPool;
    ctx.insert("db", &db);

    let requirements = rstest_bdd::iter::<StepFixtureRequirements>
        .into_iter()
        .find(|entry| {
            entry.keyword == StepKeyword::Given
                && entry.pattern.as_str() == "a generated fixture requirement"
        })
        .expect("generated submit block should register fixture requirements")
        .requirements;

    assert_eq!(requirements.len(), 1);
    assert_eq!(requirements[0].name, "db");
    assert_eq!(requirements[0].ty, "DbPool");
}
