use rstest_bdd_macros::given;

#[given("numbers {a} then {b} and finally {c}")]
fn missing_multi() {}

#[given("value {v:u32}")]
fn type_mismatch(_v: String) {}

#[given("sum {x} and {y}")]
fn extra_param(_x: u32, _y: u32, _z: u32) {}

fn main() {}
