use rstest_bdd_macros::given;

#[given("coordinates")]
fn step_with_tuple((x, y): (i32, i32)) {}

fn main() {}
