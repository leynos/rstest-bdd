Feature: Tokio harness integration

  Scenario: Tokio runtime is active during step execution
    Given the Tokio runtime is active
    When a Tokio handle is obtained
    Then the handle confirms current-thread execution

  Scenario: Tokio harness with attribute policy
    Given the Tokio runtime is active
    When a Tokio handle is obtained
    Then the handle confirms current-thread execution
