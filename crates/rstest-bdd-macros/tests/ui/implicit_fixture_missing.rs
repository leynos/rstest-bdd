use rstest_bdd_macros::given;

#[given("step with implicit fixture")]
fn step(_number: u32) {}

compile_error!("missing implicit fixture");

fn main() {}
