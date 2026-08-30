//! Behavioural test covering struct-based step arguments.

use proptest::prelude::*;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, StepArgs, given, scenario, then, when};

#[derive(Default, ScenarioState)]
struct CartState {
    cart: Slot<CartInput>,
}

#[derive(Clone, Debug, StepArgs)]
struct CartInput {
    quantity: u32,
    item: String,
    price: f32,
}

/// Product input with a `:string`-hinted `name` field to verify quote stripping in `#[step_args]`.
#[derive(Clone, Debug, StepArgs)]
struct ProductInput {
    name: String,
    price: f32,
}

#[derive(Debug, PartialEq, StepArgs)]
struct ReorderedTransfer {
    recipient: String,
    #[step_args(placeholder = "sender")]
    from: String,
    amount: u64,
}

#[derive(Debug, PartialEq, StepArgs)]
struct NormalizedAmount {
    #[step_args(trim, parse_with = parse_cents)]
    amount: u64,
}

fn parse_cents(raw: &str) -> Result<u64, std::num::ParseIntError> {
    raw.strip_suffix(" cents").unwrap_or(raw).parse()
}

#[derive(Default, ScenarioState)]
struct ProductState {
    product: Slot<ProductInput>,
}

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn cart_state() -> CartState { CartState::default() }

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn product_state() -> ProductState { ProductState::default() }

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

/// Step that captures a quoted product name using `:string` hint with `#[step_args]`.
#[given("a product named {name:string} priced at ${price:f32}")]
fn set_product(#[step_args] product: ProductInput, product_state: &ProductState) {
    product_state.product.set(product);
}

/// Verify the product was captured with quotes stripped from the name.
#[then("the product summary shows {name:string} at ${price:f32}")]
#[expect(
    clippy::expect_used,
    reason = "behaviour test panics with clear message when state missing"
)]
fn product_summary_matches(#[step_args] expected: ProductInput, product_state: &ProductState) {
    let actual = product_state
        .product
        .get()
        .expect("product state should be populated before verification");
    assert_eq!(
        actual.name, expected.name,
        "product name should match without quotes"
    );
    assert_eq!(
        actual.price.to_bits(),
        expected.price.to_bits(),
        "product price should match"
    );
}

#[scenario(path = "tests/features/struct_step_args.feature")]
fn struct_step_args(#[from(cart_state)] _cart_state: CartState) {}

#[scenario(
    path = "tests/features/struct_step_args.feature",
    name = "String hints with step_args struct"
)]
fn struct_step_args_with_string_hint(#[from(product_state)] _product_state: ProductState) {}

#[test]
fn struct_step_args_reports_parse_failure() {
    let Err(err) = <CartInput as rstest_bdd::step_args::StepArgs>::from_captures(vec![
        "invalid".into(),
        "widget".into(),
        "1.99".into(),
    ]) else {
        panic!("invalid quantity should trigger StepArgsError");
    };
    let expected = rstest_bdd::step_args::StepArgsError::parse_failure("quantity", "invalid");
    assert_eq!(err.to_string(), expected.to_string());
}

#[test]
fn step_args_bind_named_captures_independently_of_field_order() {
    let transfer = <ReorderedTransfer as rstest_bdd::StepArgs>::from_named_captures(vec![
        rstest_bdd::step_args::StepCapture {
            name: "sender",
            value: String::from("Alice"),
        },
        rstest_bdd::step_args::StepCapture {
            name: "amount",
            value: String::from("42"),
        },
        rstest_bdd::step_args::StepCapture {
            name: "recipient",
            value: String::from("Bob"),
        },
    ])
    .expect("named captures should populate the transfer");
    assert_eq!(
        transfer,
        ReorderedTransfer {
            recipient: String::from("Bob"),
            from: String::from("Alice"),
            amount: 42,
        }
    );
}

proptest! {
    #[test]
    fn named_capture_order_does_not_change_step_args_binding(
        captures in prop::sample::select(vec![
            [("sender", "Alice"), ("amount", "42"), ("recipient", "Bob")],
            [("sender", "Alice"), ("recipient", "Bob"), ("amount", "42")],
            [("amount", "42"), ("sender", "Alice"), ("recipient", "Bob")],
            [("amount", "42"), ("recipient", "Bob"), ("sender", "Alice")],
            [("recipient", "Bob"), ("sender", "Alice"), ("amount", "42")],
            [("recipient", "Bob"), ("amount", "42"), ("sender", "Alice")],
        ]),
    ) {
        let captures = captures
            .into_iter()
            .map(|(name, value)| rstest_bdd::StepCapture {
                name,
                value: String::from(value),
            })
            .collect();
        let transfer = <ReorderedTransfer as rstest_bdd::StepArgs>::from_named_captures(captures)
            .expect("every capture permutation should populate the transfer");

        prop_assert_eq!(
            transfer,
            ReorderedTransfer {
                recipient: String::from("Bob"),
                from: String::from("Alice"),
                amount: 42,
            },
        );
    }
}

#[test]
fn step_args_trim_before_custom_parsing() {
    let amount = <NormalizedAmount as rstest_bdd::StepArgs>::from_named_captures(vec![
        rstest_bdd::step_args::StepCapture {
            name: "amount",
            value: String::from(" 42 cents "),
        },
    ])
    .expect("trimmed capture should parse through the custom parser");
    assert_eq!(amount, NormalizedAmount { amount: 42 });
}
