Feature: Struct-based step arguments

  Scenario: Replacing cart items via structured arguments
    Given a cart containing 2 pumpkins at $4.50
    When I replace the cart contents with 5 squash at $7.25
    Then the cart summary shows 5 squash at $7.25
