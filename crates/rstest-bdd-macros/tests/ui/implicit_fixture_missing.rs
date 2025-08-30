use rstest_bdd_macros::given;

#[given("a step with an implicit fixture")]
fn step(_missing: u32) {}

fn main() {}
