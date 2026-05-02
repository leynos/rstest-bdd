Feature: Tokio harness scenarios macro defaults

  Scenario: Tokio scenarios macro uses harness-led defaults
    Given the Tokio runtime is active
    When a Tokio handle is obtained
    Then the handle confirms current-thread execution
