use rstest_bdd_macros::given;

#[given("a number {value}")]
fn step(other: u32) {}

fn main() {}
