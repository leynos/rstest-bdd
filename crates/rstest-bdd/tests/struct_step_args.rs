//! Behavioural test covering struct-based step arguments.

use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{given, scenario, then, when, ScenarioState, StepArgs};

#[derive(Default, ScenarioState)]
struct CartState {
    cart: Slot<CartInput>,
}

#[derive(Clone, StepArgs)]
struct CartInput {
    quantity: u32,
    item: String,
    price: f32,
}

#[fixture]
fn cart_state() -> CartState {
    CartState::default()
}

#[given("a cart containing {quantity:u32} {item} at ${price:f32}")]
fn a_cart_with_items(#[step_args] details: CartInput, cart_state: &CartState) {
    cart_state.cart.set(details);
}

#[when("I replace the cart contents with {quantity:u32} {item} at ${price:f32}")]
fn replace_cart(#[step_args] details: CartInput, cart_state: &CartState) {
    cart_state.cart.set(details);
}

#[then("the cart summary shows {quantity:u32} {item} at ${price:f32}")]
#[expect(
    clippy::expect_used,
    reason = "behaviour test panics with clear message when state missing"
)]
fn cart_summary_matches(#[step_args] expected: CartInput, cart_state: &CartState) {
    let actual = cart_state
        .cart
        .get()
        .expect("cart state should be populated before verification");
    let CartInput {
        quantity: actual_quantity,
        item: actual_item,
        price: actual_price,
    } = actual;
    let CartInput {
        quantity: expected_quantity,
        item: expected_item,
        price: expected_price,
    } = expected;
    assert_eq!(actual_quantity, expected_quantity);
    assert_eq!(actual_item, expected_item);
    assert_eq!(actual_price.to_bits(), expected_price.to_bits());
}

#[scenario(path = "tests/features/struct_step_args.feature")]
fn struct_step_args(cart_state: CartState) {
    let _ = cart_state;
}
