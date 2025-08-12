use rstest_bdd_macros::given;

struct User {
    name: String,
}

#[given("user data")]
fn step_with_struct(User { name }: User) {}

fn main() {}
