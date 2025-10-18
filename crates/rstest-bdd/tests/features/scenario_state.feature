Feature: Scenario state slots
  The Slot<T> helper stores values without bespoke RefCell boilerplate.

  Scenario: Recording a single value
    Given an empty cart state
    When I record the value 42
    Then the recorded value is 42

  Scenario: Clearing stored values
    Given an empty cart state
    When I record the value 17
    And I clear the cart state
    Then no value is stored
