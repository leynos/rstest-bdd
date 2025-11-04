//! Behavioural test covering struct-based step arguments.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when, StepArgs};

#[derive(Clone, Default)]
struct Cart {
    quantity: u32,
    item: String,
    price: f32,
}

impl Cart {
    fn set(&mut self, details: &CartInput) {
        self.quantity = details.quantity;
        self.item = details.item.clone();
        self.price = details.price;
    }
}

#[derive(StepArgs)]
struct CartInput {
    quantity: u32,
    item: String,
    price: f32,
}

#[fixture]
fn cart() -> Cart {
    Cart::default()
}

#[given("a cart containing {quantity:u32} {item} at ${price:f32}")]
fn a_cart_with_items(#[step_args] details: CartInput, cart: &mut Cart) {
    cart.set(&details);
}

#[when("I replace the cart contents with {quantity:u32} {item} at ${price:f32}")]
fn replace_cart(#[step_args] details: CartInput, cart: &mut Cart) {
    cart.set(&details);
}

#[then("the cart summary shows {quantity:u32} {item} at ${price:f32}")]
fn cart_summary_matches(#[step_args] expected: CartInput, cart: &Cart) {
    assert_eq!(cart.quantity, expected.quantity);
    assert_eq!(cart.item, expected.item);
    assert!((cart.price - expected.price).abs() < f32::EPSILON);
}

#[scenario(path = "tests/features/struct_step_args.feature")]
fn struct_step_args(cart: Cart) {
    let _ = cart;
}
