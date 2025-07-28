Feature: Using fixtures

  Scenario: Step accesses fixture value
    Given the number 42 is available as a fixture
    When the step function retrieves the number
    Then the fixture value matches expectations
