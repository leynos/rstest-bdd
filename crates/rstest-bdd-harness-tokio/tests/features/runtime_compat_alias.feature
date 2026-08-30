Feature: Explicit Tokio harness
  Scenario: Explicit Tokio harness executes synchronous steps
    Given an explicit Tokio harness counter initialized to 0
    When the explicit Tokio harness counter is incremented synchronously
    Then the explicit Tokio harness counter value is 1
