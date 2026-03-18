Feature: Runtime compatibility alias
  Scenario: Tokio runtime alias executes synchronous steps
    Given a runtime alias counter initialized to 0
    When the runtime alias counter is incremented synchronously
    Then the runtime alias counter value is 1
