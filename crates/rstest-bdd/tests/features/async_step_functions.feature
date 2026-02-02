Feature: Async step functions

  Scenario: Async scenario runs async step bodies
    Given a counter state starts at 0
    When an async step increments the state
    And a sync step increments the state
    Then the state value is 2

  Scenario: Sync scenario can execute async steps via blocking fallback
    Given a counter state starts at 0
    When an async step increments the state
    And a sync step increments the state
    Then the state value is 2
